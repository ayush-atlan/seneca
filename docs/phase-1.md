# Phase 1 — The Tamper-Evident Evidence Log

This is the one part of Seneca that is built today (`src/evidence.rs`). It is the wedge: a record of
what an agent did that the agent **cannot forge, erase, or truncate** without it being detectable.

Written for someone new — terms are explained as they appear.

## The property we guarantee

> Even if an AI agent completely takes over its own machine, it still cannot change the record of what
> it did without that tampering being caught.

A few terms first:

- **Guest** — the agent and everything running inside its virtual machine. We assume it is fully
  compromised and hostile.
- **Host** — the trusted machine that runs the guest's VM from the outside. The guest can't reach the
  host's files or keys.
- **Hash** — a short fingerprint of some data (we use SHA-256). Change one byte of the data and the
  fingerprint changes completely. You can't work backwards from the fingerprint to fake matching data.

## How it works (three ideas)

1. **Hash chain.** Each log record includes the hash of the record before it. So the records form a
   chain: record #5 "points at" #4, which points at #3, and so on. Alter record #3 and its hash
   changes, which breaks the link #4 was relying on — the break is visible.

2. **Host-held signing key.** A **signing key** is a secret that lets the holder produce a signature
   only they could make (we use Ed25519). Seneca keeps this key on the **host**, never in the guest.
   After each new record, the host signs the chain's current tip (the "head"). Because the guest never
   has the key, it cannot produce a valid signature for a version of history it rewrote.

3. **Verify.** `seneca verify` rebuilds the chain from scratch and checks the signed head. It reports
   either "intact" or the exact record where things first diverge.

Together these cover the three ways to cheat:

| Attack | Caught by |
|---|---|
| Alter a record | hash chain breaks at that record |
| Insert / remove a record in the middle | sequence numbers + chain don't line up |
| Drop records off the end (truncate) | the signed head expects more records than remain |
| Rewrite history and re-sign | impossible — the signing key is on the host |

## Record format

Each line of the log is one record:

```
{ seq, prev_hash, host_time, event, payload, hash }
```

`hash` is computed over the record's fields **plus** `prev_hash`, using a length-prefixed encoding so
there's no ambiguity about where one field ends. `verify` always recomputes this — it never trusts the
stored value.

## Commands

```
seneca record <log> <event> <payload>   # append an event (the entrypoint the host recorder calls)
seneca verify  <log>                     # rebuild the chain, check the signed head, report divergence
seneca timeline <log>                    # print the action history in order
seneca demo                              # the proof, end to end
```

`record` keeps three sidecar files next to the log: `.key` (the host signing key, `0600`, never given
to the guest), `.pub` (its public half, used by `verify`), and `.head` (the latest signed head).

## The demo (the only test that matters)

```
cargo run -- demo
```

It records a normal agent session including a blocked exfiltration attempt, verifies the log is intact,
then **edits the log to hide the exfil** (simulating a compromised guest) and verifies again:

```
caught: DIVERGENCE at seq 3: record contents were altered (hash mismatch)
```

The guest could not hide what it did. That is the whole thesis in one run. A self-check
(`cargo test`) asserts both tampering and truncation are detected.

## What Phase 1 does NOT do (later phases)

- **Capture the events itself.** A real deployment needs a host-side recorder watching the guest's
  single network exit (a tap device + default-deny firewall) to feed events into `record`. That needs
  a Linux/KVM host; `record` is the seam it plugs into. *(Phase A network recorder / Phase B broker.)*
- **External anchoring.** Periodically publishing the head to an outside immutable store, so even a
  later *host* compromise can only rewrite a bounded window. *(Phase D.)*
- **Hardware-backed keys.** Holding the signing key in a TPM/KMS instead of a file. *(Phase B.)*
- **Forensic freeze.** Snapshotting a misbehaving VM into a sealed evidence bundle. *(Phase C.)*

Phase 1 deliberately proves the cryptography — the part the thesis stands or falls on — first.
