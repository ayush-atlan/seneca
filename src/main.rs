//! Seneca Phases 1-2: boot a swarm of Firecracker microVMs, and freeze/resume
//! the whole swarm.
//!
//! A swarm is a JSON file listing members.
//!   up <spec>    boot one microVM per member, all-or-nothing
//!   freeze <n>   pause + snapshot every member to disk, then stop them (0 running)
//!   resume <n>   restore every member from its snapshot and resume
//!   down <n>     destroy a swarm (kill + delete its state and snapshots)
//!   status       show each swarm: running (alive count) or frozen
//!
//! Cold boot uses Firecracker's `--config-file` (no API calls needed). Snapshot
//! and restore *do* need the API, so VMs run with an `--api-sock`; we drive that
//! socket by shelling out to `curl --unix-socket` -- no HTTP crate required.
//!
//! ponytail: no networking yet (no tap devices) and no forking -- so the classic
//! snapshot hazards do NOT apply here yet; see the block in `resume`. No
//! control-plane/scheduler service: state is a file under .seneca/. Requires
//! `curl` and (for real VMs) Linux + KVM + a `firecracker` binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;

const BOOT_ARGS: &str = "console=ttyS0 reboot=k panic=1 pci=off";
// ponytail: fixed liveness heuristic; swap for a readiness probe if boots are slow.
const LIVENESS_WAIT: Duration = Duration::from_millis(1000);
const USAGE: &str =
    "usage: seneca up <swarm.json> | freeze <name> | resume <name> | down <name> | status";

// ---- swarm spec (input) ----------------------------------------------------

#[derive(Deserialize)]
struct Spec {
    name: String,
    members: Vec<Member>,
}

#[derive(Deserialize)]
struct Member {
    name: String,
    kernel: String,
    rootfs: String,
    #[serde(default = "one")]
    vcpus: u32,
    #[serde(default = "default_mem")]
    mem_mib: u32,
}
fn one() -> u32 {
    1
}
fn default_mem() -> u32 {
    256
}

// ---- swarm state (persisted under .seneca/<name>.json) ---------------------

#[derive(Serialize, Deserialize)]
struct SwarmState {
    name: String,
    frozen: bool,
    members: Vec<MemberState>,
}

#[derive(Serialize, Deserialize)]
struct MemberState {
    name: String,
    pid: u32,         // 0 when frozen/stopped
    sock: String,     // Firecracker API socket
    snapshot: String, // "" until frozen
    mem: String,      // "" until frozen
}

// ---- paths / env -----------------------------------------------------------

fn state_dir() -> PathBuf {
    // SENECA_STATE_DIR keeps tests hermetic; defaults to .seneca/ for real use.
    std::env::var("SENECA_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".seneca"))
}
fn state_path(name: &str) -> PathBuf {
    state_dir().join(format!("{}.json", name))
}
fn firecracker() -> String {
    std::env::var("FIRECRACKER_BIN").unwrap_or_else(|_| "firecracker".into())
}

fn load_state(name: &str) -> Result<SwarmState, String> {
    serde_json::from_str(
        &fs::read_to_string(state_path(name)).map_err(|_| format!("no such swarm: {}", name))?,
    )
    .map_err(e)
}
fn save_state(s: &SwarmState) -> Result<(), String> {
    fs::write(state_path(&s.name), serde_json::to_vec(s).map_err(e)?).map_err(e)
}

// ---- Firecracker process + API --------------------------------------------

fn fc_config(m: &Member) -> serde_json::Value {
    json!({
        "boot-source": { "kernel_image_path": m.kernel, "boot_args": BOOT_ARGS },
        "drives": [{
            "drive_id": "rootfs", "path_on_host": m.rootfs,
            "is_root_device": true, "is_read_only": false
        }],
        "machine-config": { "vcpu_count": m.vcpus, "mem_size_mib": m.mem_mib },
    })
}

/// Spawn firecracker with an API socket. `cfg` = Some for cold boot from a config
/// file; None for a blank instance that will load a snapshot.
fn spawn_fc(name: &str, workdir: &Path, cfg: Option<&Path>) -> std::io::Result<(Child, PathBuf)> {
    let sock = workdir.join(format!("{}.sock", name));
    let _ = fs::remove_file(&sock); // drop a stale socket from a prior run
    let log = fs::File::create(workdir.join(format!("{}.log", name)))?;
    let errlog = log.try_clone()?;
    let mut cmd = Command::new(firecracker());
    cmd.arg("--api-sock").arg(&sock);
    if let Some(cfg) = cfg {
        cmd.arg("--config-file").arg(cfg);
    }
    let child = cmd.stdout(log).stderr(errlog).spawn()?;
    Ok((child, sock))
}

fn fc_api(sock: &str, method: &str, path: &str, body: &str) -> Result<(), String> {
    let url = format!("http://localhost{}", path);
    let out = Command::new("curl")
        .args([
            "-fsS",
            "--unix-socket",
            sock,
            "-X",
            method,
            url.as_str(),
            "-H",
            "Content-Type: application/json",
            "-d",
            body,
        ])
        .output()
        .map_err(e)?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "firecracker API {} {} failed: {}",
            method,
            path,
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn wait_for_socket(sock: &Path) -> Result<(), String> {
    for _ in 0..40 {
        if sock.exists() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    Err(format!("api socket never appeared: {}", sock.display()))
}

fn kill_pid(pid: u32) {
    if pid != 0 {
        // std has no kill-by-pid; `kill` (SIGTERM) is the lazy native option.
        let _ = Command::new("kill").arg(pid.to_string()).status();
    }
}
fn kill_child(child: &mut Child) {
    let _ = child.kill(); // disposable VMs: SIGKILL is fine for teardown
    let _ = child.wait();
}
fn pid_alive(pid: u32) -> bool {
    pid != 0
        && Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map_or(false, |s| s.success())
}

// ---- commands --------------------------------------------------------------

fn up(spec_path: &str) -> Result<(), String> {
    let spec: Spec = serde_json::from_str(&fs::read_to_string(spec_path).map_err(e)?).map_err(e)?;

    // Validate before booting anything -- cheaper to fail here than mid-launch.
    for m in &spec.members {
        for f in [&m.kernel, &m.rootfs] {
            if !Path::new(f).exists() {
                return Err(format!("missing file for member '{}': {}", m.name, f));
            }
        }
    }

    let workdir = state_dir().join(&spec.name);
    fs::create_dir_all(&workdir).map_err(e)?;

    let mut procs: Vec<(String, Child, PathBuf)> = Vec::new();
    for m in &spec.members {
        let cfg = workdir.join(format!("{}.fc.json", m.name));
        if let Err(err) = fs::write(&cfg, serde_json::to_vec(&fc_config(m)).expect("serializes")) {
            procs.iter_mut().for_each(|(_, c, _)| kill_child(c));
            return Err(format!("write config for '{}': {}", m.name, err));
        }
        match spawn_fc(&m.name, &workdir, Some(&cfg)) {
            Ok((c, sock)) => procs.push((m.name.clone(), c, sock)),
            Err(err) => {
                procs.iter_mut().for_each(|(_, c, _)| kill_child(c));
                return Err(format!("failed to spawn '{}': {}", m.name, err));
            }
        }
    }

    // All-or-nothing: a bad config makes firecracker exit fast. If any member
    // died, tear the whole swarm down so nothing is left half-started.
    std::thread::sleep(LIVENESS_WAIT);
    let mut dead: Vec<String> = Vec::new();
    for (n, c, _) in procs.iter_mut() {
        if matches!(c.try_wait(), Ok(Some(_))) {
            dead.push(n.clone());
        }
    }
    if !dead.is_empty() {
        procs.iter_mut().for_each(|(_, c, _)| kill_child(c));
        return Err(format!(
            "swarm '{}' failed all-or-nothing; members died: {}",
            spec.name,
            dead.join(", ")
        ));
    }

    let members = procs
        .iter()
        .map(|(n, c, sock)| MemberState {
            name: n.clone(),
            pid: c.id(),
            sock: sock.to_string_lossy().into_owned(),
            snapshot: String::new(),
            mem: String::new(),
        })
        .collect();
    save_state(&SwarmState {
        name: spec.name.clone(),
        frozen: false,
        members,
    })?;
    let names: Vec<&str> = procs.iter().map(|(n, _, _)| n.as_str()).collect();
    println!(
        "swarm '{}' up: {} microVMs ({})",
        spec.name,
        procs.len(),
        names.join(", ")
    );
    Ok(()) // Child handles drop here; std does NOT kill on drop, so the VMs keep running.
}

fn freeze(name: &str) -> Result<(), String> {
    let mut s = load_state(name)?;
    if s.frozen {
        return Err(format!("swarm '{}' is already frozen", name));
    }
    let snapdir = state_dir().join(name).join("snapshots");
    fs::create_dir_all(&snapdir).map_err(e)?;

    // Pause + snapshot every member FIRST (nothing killed yet), so a failure
    // partway leaves all members alive and recoverable rather than half-killed.
    let mut shots: Vec<(PathBuf, PathBuf)> = Vec::new();
    for m in &s.members {
        let snap = snapdir.join(format!("{}.vmstate", m.name));
        let mem = snapdir.join(format!("{}.mem", m.name));
        fc_api(&m.sock, "PATCH", "/vm", r#"{"state":"Paused"}"#)?;
        let body = json!({
            "snapshot_type": "Full",
            "snapshot_path": snap.to_string_lossy(),
            "mem_file_path": mem.to_string_lossy(),
        })
        .to_string();
        fc_api(&m.sock, "PUT", "/snapshot/create", &body)?;
        shots.push((snap, mem));
    }

    // All snapshots are on disk -- now stop the processes and record the freeze.
    for (m, (snap, mem)) in s.members.iter_mut().zip(shots) {
        kill_pid(m.pid);
        m.pid = 0;
        m.snapshot = snap.to_string_lossy().into_owned();
        m.mem = mem.to_string_lossy().into_owned();
    }
    s.frozen = true;
    save_state(&s)?;
    println!(
        "swarm '{}' frozen: {} microVMs snapshotted, 0 running",
        name,
        s.members.len()
    );
    Ok(())
}

fn resume(name: &str) -> Result<(), String> {
    let mut s = load_state(name)?;
    if !s.frozen {
        return Err(format!("swarm '{}' is not frozen", name));
    }
    let workdir = state_dir().join(name);

    for m in &mut s.members {
        let (child, sock) = spawn_fc(&m.name, &workdir, None).map_err(e)?;
        wait_for_socket(&sock)?;

        // ponytail: snapshot-restore hazards are deliberately NOT handled here,
        // because none can trigger in this phase:
        //   * reassign MAC/IP  -- no NIC exists yet (networking is a later phase).
        //   * reseed RNG       -- only matters when FORKING one snapshot into
        //                         multiple live VMs; resume restores exactly one.
        //   * resync clock     -- needs a guest agent we don't have; the guest
        //                         must re-sync time on resume (MMDS/vsock signal).
        // Add each the moment its trigger appears (a NIC, a fork verb, a guest
        // agent) -- before then it would be speculative code.
        let body = json!({
            "snapshot_path": m.snapshot,
            "mem_file_path": m.mem,
            "resume_vm": true,
        })
        .to_string();
        fc_api(&sock.to_string_lossy(), "PUT", "/snapshot/load", &body)?;

        m.pid = child.id();
        m.sock = sock.to_string_lossy().into_owned();
        // child drops here without being killed -> resumed VM keeps running.
    }
    s.frozen = false;
    save_state(&s)?;
    println!("swarm '{}' resumed: {} microVMs", name, s.members.len());
    Ok(())
}

fn down(name: &str) -> Result<(), String> {
    let s = load_state(name)?;
    for m in &s.members {
        kill_pid(m.pid);
    }
    let _ = fs::remove_dir_all(state_dir().join(name)); // configs, logs, snapshots
    fs::remove_file(state_path(name)).map_err(e)?;
    println!("swarm '{}' down", name);
    Ok(())
}

fn status() -> Result<(), String> {
    let dir = state_dir();
    if !dir.exists() {
        return Ok(());
    }
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(e)?
        .filter_map(|x| x.ok().map(|d| d.path()))
        .filter(|p| p.is_file() && p.extension().map_or(false, |x| x == "json"))
        .collect();
    files.sort();
    for p in files {
        let s: SwarmState =
            serde_json::from_str(&fs::read_to_string(&p).map_err(e)?).map_err(e)?;
        if s.frozen {
            println!("{}: frozen ({} snapshots)", s.name, s.members.len());
        } else {
            let alive = s.members.iter().filter(|m| pid_alive(m.pid)).count();
            println!("{}: {}/{} alive", s.name, alive, s.members.len());
        }
    }
    Ok(())
}

fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match (args.first().map(String::as_str), args.get(1).map(String::as_str)) {
        (Some("up"), Some(spec)) => up(spec),
        (Some("freeze"), Some(name)) => freeze(name),
        (Some("resume"), Some(name)) => resume(name),
        (Some("down"), Some(name)) => down(name),
        (Some("status"), None) => status(),
        _ => {
            eprintln!("{}", USAGE);
            std::process::exit(2);
        }
    };
    if let Err(msg) = r {
        eprintln!("{}", msg);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Both tests set the SENECA_STATE_DIR env var, which is process-global; this
    // serializes them so parallel `cargo test` can't make them clobber each other.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // Phase 1: all-or-nothing boot, verified with a fake firecracker (no KVM).
    #[test]
    fn all_or_nothing() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = std::env::temp_dir().join(format!("seneca_aon_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        // fake firecracker: exit 1 if the config's rootfs contains "fail"
        // (simulating a boot failure), otherwise behave like a running VM.
        let fc = tmp.join("fake_fc.sh");
        fs::write(
            &fc,
            "#!/bin/sh\ncfg=\"\"\nwhile [ $# -gt 0 ]; do case \"$1\" in --config-file) cfg=\"$2\"; shift;; esac; shift; done\n\
             grep -q 'fail' \"$cfg\" && exit 1\nexec sleep 60\n",
        )
        .unwrap();
        Command::new("chmod").arg("+x").arg(&fc).status().unwrap();

        let kernel = tmp.join("vmlinux");
        fs::write(&kernel, "x").unwrap();
        let ok = tmp.join("ok.ext4");
        fs::write(&ok, "x").unwrap();
        let bad = tmp.join("fail.ext4");
        fs::write(&bad, "x").unwrap();

        std::env::set_var("FIRECRACKER_BIN", &fc);
        std::env::set_var("SENECA_STATE_DIR", tmp.join("state"));

        let kernel_s = kernel.to_str().unwrap();
        let ok_s = ok.to_str().unwrap();
        let write_spec = |file: &str, name: &str, rootfs_b: &str| {
            let p = tmp.join(file);
            fs::write(
                &p,
                serde_json::to_vec(&json!({
                    "name": name,
                    "members": [
                        {"name": "a", "kernel": kernel_s, "rootfs": ok_s},
                        {"name": "b", "kernel": kernel_s, "rootfs": rootfs_b},
                    ]
                }))
                .unwrap(),
            )
            .unwrap();
            p
        };

        // failure: member b fails -> up errors and leaves no state behind.
        let bad_spec = write_spec("bad.json", "broken", bad.to_str().unwrap());
        assert!(up(bad_spec.to_str().unwrap()).is_err());
        assert!(
            !state_path("broken").exists(),
            "no state file after a failed all-or-nothing boot"
        );

        // success: both members boot -> state written, then down removes it.
        let ok_spec = write_spec("ok.json", "good", ok.to_str().unwrap());
        up(ok_spec.to_str().unwrap()).unwrap();
        assert!(state_path("good").exists());
        down("good").unwrap();
        assert!(!state_path("good").exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    // Phase 2: the freeze/resume bookkeeping the API calls depend on. The actual
    // pause/snapshot/load needs real KVM (manual check on a Linux host); here we
    // verify the state model round-trips and the frozen<->running flip persists.
    #[test]
    fn freeze_state_roundtrip() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = std::env::temp_dir().join(format!("seneca_st_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();
        std::env::set_var("SENECA_STATE_DIR", &tmp);

        let running = SwarmState {
            name: "s".into(),
            frozen: false,
            members: vec![MemberState {
                name: "a".into(),
                pid: 123,
                sock: "/tmp/a.sock".into(),
                snapshot: String::new(),
                mem: String::new(),
            }],
        };
        save_state(&running).unwrap();
        let mut loaded = load_state("s").unwrap();
        assert!(!loaded.frozen && loaded.members[0].pid == 123);

        // simulate the freeze transition's effect on state
        loaded.frozen = true;
        loaded.members[0].pid = 0;
        loaded.members[0].snapshot = "/snap/a.vmstate".into();
        loaded.members[0].mem = "/snap/a.mem".into();
        save_state(&loaded).unwrap();

        let frozen = load_state("s").unwrap();
        assert!(frozen.frozen && frozen.members[0].pid == 0);
        assert_eq!(frozen.members[0].snapshot, "/snap/a.vmstate");

        let _ = fs::remove_dir_all(&tmp);
    }
}
