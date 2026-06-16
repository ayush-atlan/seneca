# Seneca

> **Seneca orchestrates whole swarms of AI agents as one unit — gang-scheduled, budgeted together,
> and freezable in a heartbeat — by running each agent in a Firecracker microVM it can snapshot,
> pause, and resume on demand.**

Seneca is a design-stage project. This repository currently contains **documentation only** — no
code yet. Start here, then read [`docs/overview.md`](docs/overview.md).

---

## The problem

People are building **AI agents**: programs that use a large language model (an LLM, like Claude)
to act on their own — browse the web, read files, call other software. Increasingly they don't run
one agent, they run a **swarm**: a team of agents working together on one job (one researches, one
writes, one checks the work).

Running a swarm in a company is awkward today because the tools split into two camps, and neither
governs the swarm as a single thing:

- **Framework tools** (LangGraph, CrewAI, AutoGen) are great at *coordinating* agents — who talks to
  whom, who does what. But they run on top of ordinary processes and have no real control over the
  machine underneath. They can't pause a whole team's live memory or truly isolate one agent from
  another.
- **Sandbox vendors** (E2B, Modal, Browserbase) give each agent a strong, isolated box to run in —
  but they manage **one box at a time**. They have no concept of "this group of boxes is one team
  that should be scheduled, budgeted, and frozen together."

So the **swarm as a unit** falls through the gap.

## The idea (Seneca's thesis)

Seneca picks the **swarm** as the thing it orchestrates, and runs each agent inside a **Firecracker
microVM** — a tiny, fast, isolated virtual machine. (See the [glossary](docs/glossary.md) for any
unfamiliar term.) Owning the microVMs gives Seneca one superpower the framework tools can never have:

> **It can take a sub-second snapshot of an agent's *entire* machine state — including its live
> memory (RAM) — and pause or resume it on demand.**

That single lever unlocks four capabilities, applied to the **whole swarm at once**:

1. **Gang-schedule it.** The swarm starts all-or-nothing — it never half-launches with some members
   missing and stalls waiting for the rest.
2. **Budget it together.** One shared budget (compute, LLM tokens, external API spend) across the
   whole team, with a hard stop when it's hit — not a separate untracked bill per agent.
3. **Pause it for approval — for free.** When the swarm hits a step that needs a human "yes,"
   Seneca **snapshots and sets it aside at zero ongoing cost**, then resumes it exactly where it
   left off when the human responds — even days later.
4. **Freeze it, don't kill it.** If something goes wrong, Seneca can **snapshot-and-quarantine**
   every agent matching a rule ("freeze everything that touched data source Y") — preserving full
   live memory for investigation — instead of killing the agents and losing all the evidence.

Framework tools can't snapshot a swarm's memory. Per-sandbox vendors don't treat a swarm as one
unit. Seneca does both. (For an honest map of what's already out there, see
[`docs/prior-art.md`](docs/prior-art.md).)

## The pieces (one line each)

- **Control plane** — the brain: remembers every swarm, its budget, and its state.
- **Swarm scheduler** — decides when a whole swarm can start, and enforces all-or-nothing placement.
- **Node agent** — the hands on each host: starts/stops microVMs, wires up networking.
- **Snapshot store** — keeps the frozen machine images used for pause, resume, and forensic freeze.
- **Guest** — the agent itself, running inside a microVM.
- **CLI** — the operator's remote control: launch a swarm, pause it, freeze it, inspect it.

See [`docs/architecture.md`](docs/architecture.md) for how they fit together.

## Status

Design stage. Docs only, no code. The goal of this repo right now is to make the idea clear and
honest enough to decide whether to build it.

## Documentation

- [Overview](docs/overview.md) — the full plain-language story.
- [Glossary](docs/glossary.md) — every term explained for a newcomer.
- [Architecture](docs/architecture.md) — the parts and how they connect.
- [Prior art](docs/prior-art.md) — what already exists, and where Seneca's wedge sits.
- [Limitations](docs/limitations.md) — honest limits and known gotchas.
