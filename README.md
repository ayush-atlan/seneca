<!-- logo: drop a file at docs/assets/logo.png (square, ~200px) -->
<p align="center">
 
<img width="200" height="200" alt="Pi7_Gemini_Generated_Image_6s4dh96s4dh96s4d-Photoroom" src="https://github.com/user-attachments/assets/254858fa-f3bc-4899-bde6-2af5a14a9fa6" />

</p>



<h1 align="center">Seneca</h1>
<p align="center">
  <b>Run a whole swarm of AI agents as one unit — gang-scheduled, budgeted together,<br>
  and freezable in a heartbeat.</b>
</p>

<p align="center">
  <a href="#"><img alt="status" src="https://img.shields.io/badge/status-design%20stage-orange"></a>
  <a href="#"><img alt="built with rust" src="https://img.shields.io/badge/built%20with-Rust-000?logo=rust"></a>
  <a href="#"><img alt="firecracker" src="https://img.shields.io/badge/runtime-Firecracker-ff9900?logo=amazonaws&logoColor=white"></a>
  <a href="#"><img alt="phase" src="https://img.shields.io/badge/build-phase%202-blue"></a>
  <a href="#"><img alt="license" src="https://img.shields.io/badge/license-TBD-lightgrey"></a>
</p>

<p align="center">
  <a href="#what">What</a> ·
  <a href="#why">Why</a> ·
  <a href="#how-it-works">How it works</a> ·
  <a href="#quickstart">Quickstart</a> ·
  <a href="#the-pieces">The pieces</a> ·
  <a href="#docs">Docs</a> ·
  <a href="#status">Status</a>
</p>

---

## What

Seneca runs each AI agent in its own **microVM** (a tiny, fast, isolated virtual computer, via
[Firecracker](https://firecracker-microvm.github.io/)) and manages the whole **swarm** of them as a
single unit. Because it owns the machines, it can snapshot an agent's *entire running state — memory
included* — and pause or resume it in under a second.

## Why

| | Coordination frameworks | Single-sandbox vendors | **Seneca** |
|---|:---:|:---:|:---:|
| Coordinate agents | ✅ | — | ✅ |
| Strong isolation per agent | — | ✅ | ✅ |
| Snapshot live memory | ❌ | per box | ✅ |
| **Govern the swarm as one unit** | ❌ | ❌ | ✅ |

One snapshot lever, four superpowers — applied to the **whole team at once**:

- 🧩 **Gang-schedule** — the swarm starts all-or-nothing; never half-launched.
- 💰 **One shared budget** — compute + tokens + API spend, with a hard stop.
- ⏸️ **Approval-pause for free** — snapshot the team, resume days later exactly where it left off.
- 🧊 **Freeze, don't kill** — snapshot-and-quarantine a misbehaving swarm with full evidence intact.

## How it works

```
        CLI ──▶ Control Plane ──▶ Node Agent ──▶ [microVM][microVM][microVM]
                (decides when)    (does it)          └── one swarm ──┘
                                       │
                                  Snapshot Store  (pause · resume · freeze)
```

The orchestrator is the only thing that touches the machines, so a pause, a budget stop, or a freeze
applies to the entire swarm — not one box at a time. See [docs/overview.md](docs/overview.md).

## Quickstart

```bash
# build (Linux + KVM + a `firecracker` binary needed to boot real VMs)
cargo build --release

seneca up swarm.example.json   # boot the swarm, all-or-nothing
seneca freeze demo             # pause + snapshot the whole swarm to disk
seneca resume demo             # restore it, mid-execution
seneca status                  # running vs frozen
seneca down demo               # destroy it
```

## The pieces

- **Control plane** — remembers every swarm, its budget, and its state.
- **Node agent** — boots microVMs and performs snapshot / pause / resume.
- **Snapshot store** — the frozen machine states behind pause, resume, and freeze.
- **Guest** — the agent, running untrusted inside its microVM.
- **CLI** — launch · freeze · resume · down · status.

## Docs

- [Overview](docs/overview.md) · the full plain-language story
- [Glossary](docs/glossary.md) · every term explained for a newcomer
- [Architecture](docs/architecture.md) · the parts and how they connect
- [Build plan](docs/build-plan.md) · phased implementation
- [Prior art](docs/prior-art.md) · what exists, and where Seneca's wedge sits
- [Limitations](docs/limitations.md) · honest limits and gotchas
- [Whitepaper](whitepaper/seneca.tex) · in-depth architecture (LaTeX)

## Status

Design stage, building in phases. **Phase 1** (all-or-nothing swarm boot) and **Phase 2**
(freeze/resume) are implemented in `src/main.rs`. Not production-ready.
