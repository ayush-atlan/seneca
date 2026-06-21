# Seneca

**The flight recorder for AI agents — prove what your agent did, tamper-evidently.**

AI agents now take real actions with real credentials. After an incident the question isn't "should it have?" — it's *"what exactly did it do, and can we prove the record wasn't forged by the compromised agent itself?"*

Seneca records agent activity **outside the guest**, in a hash-chained log signed by a host-held key. If an agent fully compromises its own VM, it still **cannot forge, erase, or truncate the evidence** without detection.

> Not another sandbox or policy gateway. Isolation and enforcement are table stakes. Seneca's wedge is **guest-unforgeable evidence**.

## How it works

- **Hash chain** — each record links to the previous; altering one breaks the chain.
- **Host-held signing key** — the chain head is signed by a key the guest never has, so tampering can't be re-signed.
- **`verify`** — recomputes the chain and pinpoints the first divergence (edit, insertion, or truncation).

## Try it

```bash
cargo run -- demo        # compromised guest tries to hide an exfil; Seneca catches it
```
```
seneca record <log> <event> <payload>   # append an event (the host recorder's entrypoint)
seneca verify <log>                      # check integrity / report divergence
seneca timeline <log>                    # reconstruct the action history
```

## Status

Phase A (the wedge) is built and tested: tamper-evident log + `verify` + the compromise demo (`src/evidence.rs`).
The microVM harness (`up`/`freeze`/`resume`) exists for the host-side recorder and forensic-freeze phases.

| Phase | What | State |
|---|---|---|
| A | Tamper-evident evidence log + verify | ✅ |
| B | Scoped egress broker + credential brokering | next |
| C | Forensic freeze → sealed evidence bundle | — |
| D | Verification surface (timeline, anchors, divergence) | partial |

Design stage; not production-ready. Full thesis: [whitepaper/seneca.tex](whitepaper/seneca.tex).

## Build

```bash
cargo build --release && cargo test
```
