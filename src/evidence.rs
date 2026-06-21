//! Seneca Phase A — the tamper-evident evidence log (the wedge).
//!
//! A host-side, append-only log of agent activity that a compromised *guest*
//! cannot forge, alter, or truncate without detection. Each record is hash-chained
//! to the previous one; the chain head is signed by a key held on the HOST (never
//! given to the guest). `verify` recomputes the chain, checks the signed head, and
//! reports either integrity or the first point of divergence.
//!
//! In production the signing key lives in a TPM/KMS and the log is written by a
//! host-side recorder watching the guest's only tap. Here the key is a host-side
//! file and `record` is the entrypoint the recorder calls — so the cryptographic
//! property (the actual thesis) is provable locally, without KVM.
//!
//! Threat model boundary: the guest may tamper with the LOG bytes; it can never
//! reach the `.key` or `.head` (those are host-side). The demo/tests model exactly
//! that — they edit the log only.

use std::fs;
use std::path::PathBuf;

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const GENESIS: [u8; 32] = [0u8; 32];

#[derive(Serialize, Deserialize)]
struct Record {
    seq: u64,
    prev_hash: String, // hex
    host_time: String, // host clock, outside guest control
    event: String,
    payload: String,
    hash: String, // hex; recomputed (not trusted) on verify
}

#[derive(Serialize, Deserialize)]
struct Head {
    seq: u64,
    head_hash: String, // hex
    sig: String,       // hex, Ed25519 over head_msg(seq, head_hash)
}

// ---- helpers ---------------------------------------------------------------

fn e<E: std::fmt::Display>(x: E) -> String {
    x.to_string()
}
fn side(logpath: &str, ext: &str) -> PathBuf {
    PathBuf::from(format!("{}.{}", logpath, ext))
}
fn hex_encode(b: &[u8]) -> String {
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push_str(&format!("{:02x}", x));
    }
    s
}
fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}
fn hex32(s: &str) -> Option<[u8; 32]> {
    unhex(s)?.try_into().ok()
}
fn hex64(s: &str) -> Option<[u8; 64]> {
    unhex(s)?.try_into().ok()
}

fn now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{}", ms)
}

/// Length-prefixed canonical hash of a record — unambiguous regardless of field
/// contents (no JSON-whitespace or delimiter ambiguity).
fn record_hash(seq: u64, prev: &[u8; 32], host_time: &str, event: &str, payload: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"SENECAv1");
    h.update(seq.to_le_bytes());
    h.update(prev);
    for f in [host_time, event, payload] {
        h.update((f.len() as u64).to_le_bytes());
        h.update(f.as_bytes());
    }
    h.finalize().into()
}

fn head_msg(seq: u64, hash: &[u8; 32]) -> Vec<u8> {
    let mut m = seq.to_le_bytes().to_vec();
    m.extend_from_slice(hash);
    m
}

fn load_or_create_key(logpath: &str) -> Result<SigningKey, String> {
    let keypath = side(logpath, "key");
    if keypath.exists() {
        let bytes = fs::read(&keypath).map_err(e)?;
        let arr: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| "host key file is corrupt".to_string())?;
        return Ok(SigningKey::from_bytes(&arr));
    }
    let mut seed = [0u8; 32];
    getrandom::getrandom(&mut seed).map_err(|_| "rng failed".to_string())?;
    let sk = SigningKey::from_bytes(&seed);
    fs::write(&keypath, seed).map_err(e)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&keypath, fs::Permissions::from_mode(0o600));
    }
    fs::write(side(logpath, "pub"), hex_encode(sk.verifying_key().as_bytes())).map_err(e)?;
    Ok(sk)
}

fn load_pub(logpath: &str) -> Result<VerifyingKey, String> {
    let s = fs::read_to_string(side(logpath, "pub")).map_err(|_| "no public key for this log".to_string())?;
    let arr = hex32(s.trim()).ok_or("public key file is malformed")?;
    VerifyingKey::from_bytes(&arr).map_err(|_| "public key is invalid".to_string())
}

fn read_head(logpath: &str) -> Result<Option<Head>, String> {
    let p = side(logpath, "head");
    if !p.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_str(&fs::read_to_string(&p).map_err(e)?).map_err(e)?))
}

fn write_head(logpath: &str, key: &SigningKey, seq: u64, hash: &[u8; 32]) -> Result<(), String> {
    let sig = key.sign(&head_msg(seq, hash));
    let h = Head {
        seq,
        head_hash: hex_encode(hash),
        sig: hex_encode(&sig.to_bytes()),
    };
    fs::write(side(logpath, "head"), serde_json::to_vec(&h).map_err(e)?).map_err(e)
}

// ---- commands --------------------------------------------------------------

/// Append one event to the evidence log and re-sign the chain head.
pub fn record(logpath: &str, event: &str, payload: &str) -> Result<(), String> {
    let key = load_or_create_key(logpath)?;
    let (prev_seq, prev) = match read_head(logpath)? {
        Some(h) => (
            Some(h.seq),
            hex32(&h.head_hash).ok_or("signed head is malformed")?,
        ),
        None => (None, GENESIS),
    };
    let seq = prev_seq.map(|s| s + 1).unwrap_or(0);
    let host_time = now();
    let hash = record_hash(seq, &prev, &host_time, event, payload);

    let rec = Record {
        seq,
        prev_hash: hex_encode(&prev),
        host_time,
        event: event.to_string(),
        payload: payload.to_string(),
        hash: hex_encode(&hash),
    };
    let line = serde_json::to_string(&rec).map_err(e)? + "\n";
    use std::io::Write;
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logpath)
        .map_err(e)?;
    f.write_all(line.as_bytes()).map_err(e)?;

    write_head(logpath, &key, seq, &hash)?;
    Ok(())
}

/// Recompute the chain, validate the signed head, and report integrity or the
/// first point of divergence.
pub fn verify(logpath: &str) -> Result<(), String> {
    let vpub = load_pub(logpath)?;
    let head = read_head(logpath)?.ok_or("no signed head — nothing to verify")?;
    let data = fs::read_to_string(logpath).map_err(e)?;

    let mut prev = GENESIS;
    let mut expected_seq = 0u64;
    let mut last_hash = GENESIS;
    for (i, line) in data.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let r: Record = serde_json::from_str(line)
            .map_err(|_| format!("DIVERGENCE at line {}: record is not parseable (altered)", i + 1))?;
        if r.seq != expected_seq {
            return Err(format!(
                "DIVERGENCE at line {}: seq {} (expected {}) — record inserted or removed",
                i + 1,
                r.seq,
                expected_seq
            ));
        }
        let stored_prev = hex32(&r.prev_hash).ok_or("malformed prev_hash")?;
        if stored_prev != prev {
            return Err(format!("DIVERGENCE at seq {}: prev_hash does not chain", r.seq));
        }
        let h = record_hash(r.seq, &prev, &r.host_time, &r.event, &r.payload);
        if hex_encode(&h) != r.hash {
            return Err(format!(
                "DIVERGENCE at seq {}: record contents were altered (hash mismatch)",
                r.seq
            ));
        }
        prev = h;
        last_hash = h;
        expected_seq += 1;
    }

    if head.seq + 1 != expected_seq {
        return Err(format!(
            "TRUNCATION: signed head is seq {} but log holds {} record(s) — records were dropped",
            head.seq, expected_seq
        ));
    }
    let head_hash = hex32(&head.head_hash).ok_or("malformed head hash")?;
    if head_hash != last_hash {
        return Err("DIVERGENCE: signed head does not match the recomputed chain (log altered)".to_string());
    }
    let sig = Signature::from_bytes(&hex64(&head.sig).ok_or("malformed head signature")?);
    vpub
        .verify(&head_msg(head.seq, &head_hash), &sig)
        .map_err(|_| "signed head signature is INVALID (head was forged)".to_string())?;

    println!(
        "OK — {} record(s), chain intact, head signature valid (seq 0..={})",
        expected_seq, head.seq
    );
    Ok(())
}

/// Reconstruct the action history from the host-captured evidence.
pub fn timeline(logpath: &str) -> Result<(), String> {
    let data = fs::read_to_string(logpath).map_err(e)?;
    for line in data.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let r: Record = serde_json::from_str(line).map_err(e)?;
        println!("#{:<4} t={:<14} {:<22} {}", r.seq, r.host_time, r.event, r.payload);
    }
    Ok(())
}

/// The only demo that matters: a compromised guest tries to hide an exfil; the
/// host record still proves it, and verification catches the tampering.
pub fn demo() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("seneca_demo_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(e)?;
    let logbuf = dir.join("run.log");
    let log = logbuf.to_str().ok_or("bad temp path")?;

    println!("1. Host-side recorder logs the agent's activity (key is host-held, never in the guest):");
    record(log, "session.start", "agent run-7")?;
    record(log, "tool.call", "search query=\"q3 revenue\"")?;
    record(log, "egress.allow", "api.stripe.com 1.2KB")?;
    record(log, "egress.DENY", "evil-exfil.net 4096B — customer PII blocked")?;
    record(log, "session.end", "agent run-7")?;
    timeline(log)?;

    println!("\n2. Verify the untampered evidence:");
    verify(log)?;

    println!("\n3. A fully compromised guest edits the log to hide the exfil attempt...");
    let tampered = fs::read_to_string(log)
        .map_err(e)?
        .replace("evil-exfil.net 4096B — customer PII blocked", "cdn.example.com 4096B");
    fs::write(log, tampered).map_err(e)?;

    println!("4. Verify again — Seneca detects it (the guest can't re-sign the host-held head):");
    match verify(log) {
        Ok(()) => {
            let _ = fs::remove_dir_all(&dir);
            return Err("DEMO FAILED: tampering was NOT detected".to_string());
        }
        Err(msg) => println!("   caught: {}", msg),
    }

    let _ = fs::remove_dir_all(&dir);
    println!("\nThesis property holds: the guest could not forge or erase the evidence.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tamper_and_truncation_are_detected() {
        let dir = std::env::temp_dir().join(format!("seneca_ev_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // 1. Tampering a record is detected.
        let p = dir.join("run.log");
        let log = p.to_str().unwrap();
        record(log, "session.start", "run-1").unwrap();
        record(log, "egress.DENY", "evil-exfil.net 4096B").unwrap();
        record(log, "session.end", "run-1").unwrap();
        verify(log).unwrap(); // intact

        let hidden = fs::read_to_string(log).unwrap().replace("evil-exfil.net", "cdn.example.com");
        fs::write(log, hidden).unwrap();
        assert!(verify(log).is_err(), "record tampering must be detected");

        // 2. Truncation (dropping the tail) is detected via the signed head.
        let p2 = dir.join("run2.log");
        let log2 = p2.to_str().unwrap();
        record(log2, "a", "1").unwrap();
        record(log2, "b", "2").unwrap();
        verify(log2).unwrap();
        let mut lines: Vec<String> = fs::read_to_string(log2)
            .unwrap()
            .lines()
            .map(str::to_string)
            .collect();
        lines.pop();
        fs::write(log2, lines.join("\n") + "\n").unwrap();
        assert!(verify(log2).is_err(), "truncation must be detected");

        let _ = fs::remove_dir_all(&dir);
    }
}
