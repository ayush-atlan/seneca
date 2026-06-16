# Seneca — Architecture

How the parts of Seneca fit together. This stays at the "boxes and arrows" level — no code. If a term
is unfamiliar, see the [glossary](glossary.md).

> **One-line reminder of the thesis:** Seneca manages a *swarm* of agents as one unit, using
> full-machine snapshots (RAM included) to gang-schedule, budget, pause-for-approval, and freeze it.

---

## The big picture

```
                          ┌──────────────────────── CONTROL PLANE (the brain) ───────────────────────┐
 operator ── CLI ───────▶ │  • remembers every swarm: its members, budget, and state                 │
                          │  • Swarm Scheduler: decides when a whole swarm may start (all-or-nothing) │
                          │  • Budget tracker: one shared budget per swarm, hard stop when hit        │
                          │  • Freeze/pause controller: issues snapshot/quarantine commands           │
                          └───────────────┬───────────────────────────────────────┬──────────────────┘
                                          │ "place this swarm"                     │ "snapshot/restore"
                          ┌───────────────▼───────────── HOST(s) ─────────────────▼──────────────────┐
                          │  NODE AGENT (the hands on each host)                                      │
                          │   • starts/stops Firecracker microVMs                                     │
                          │   • wires up networking for each microVM                                  │
                          │   • performs snapshot / pause / resume on command                         │
                          │                                                                           │
                          │   ┌── microVM ──┐ ┌── microVM ──┐ ┌── microVM ──┐   ← one swarm =         │
                          │   │ guest:      │ │ guest:      │ │ guest:      │     several microVMs    │
                          │   │ planner     │ │ worker      │ │ reviewer    │     managed together    │
                          │   └─────────────┘ └─────────────┘ └─────────────┘                         │
                          └───────────────────────────────────┬───────────────────────────────────────┘
                                                               │ frozen machine images
                                                  ┌────────────▼─────────────┐
                                                  │  SNAPSHOT STORE          │
                                                  │  holds the saved machine │
                                                  │  states (RAM + disk) for │
                                                  │  pause, resume, freeze   │
                                                  └──────────────────────────┘
```

## The pieces, in more detail

### Control plane (the brain)
The part that *decides and remembers*. It holds the authoritative record of every swarm: which agents
are in it, its shared budget and how much is spent, and its current state (starting, running, paused,
frozen). It's where the operator's commands land and where policy lives. It does not run agent code
itself — it directs the parts that do.

Inside the control plane:
- **Swarm scheduler** — the gatekeeper for starting swarms. It only gives the go-ahead when *every*
  member of a swarm can be placed at once (all-or-nothing). This is the gang-scheduling rule.
- **Budget tracker** — adds up the swarm's spend across the three currencies (compute, tokens,
  external API) and enforces a hard stop when the shared limit is reached.
- **Freeze/pause controller** — the thing that turns "pause this swarm for approval" or "freeze
  everything that touched data source Y" into concrete snapshot/quarantine commands for the node
  agents.

### Node agent (the hands on each host)
One runs on every host machine. It owns the Firecracker microVMs on that host: starting and stopping
them, wiring up their networking, and — most importantly — carrying out **snapshot, pause, and
resume** when the control plane asks. The node agent is where Seneca actually touches the machines.

### microVMs and the guest
Each agent in a swarm runs inside its own Firecracker microVM. The software inside (the agent) is the
**guest**, and it's treated as untrusted. A swarm is simply a set of these microVMs that the control
plane tracks and acts on *together* — that "together" is the whole point.

### Snapshot store
Where the frozen machine images live. When a swarm is paused for approval or frozen for forensics, its
full state (RAM + disk) is captured here, so it can be restored later — possibly days later, possibly
in a sealed environment for investigation.

### CLI (the operator's remote control)
The human-facing commands: launch a swarm, check its budget and state, pause it, resume it, freeze a
set of agents by rule, and inspect a frozen swarm. The CLI talks to the control plane.

## How a swarm flows through Seneca

1. **Submit.** An operator (via the CLI) submits a swarm: its member agents, their resource needs, and
   a shared budget.
2. **Schedule (all-or-nothing).** The swarm scheduler waits until every member can be placed, then
   gives the go-ahead. The swarm never half-starts.
3. **Run.** Node agents boot a microVM per member. The budget tracker watches the shared spend.
4. **Pause for approval (optional).** If the swarm hits a step needing human sign-off, the freeze/pause
   controller has the node agents **snapshot the swarm to the snapshot store and stop the microVMs** —
   zero ongoing cost. On approval, they **restore and resume** exactly where it left off.
5. **Hard stop on budget (if needed).** If the shared budget is exhausted, the swarm is stopped.
6. **Forensic freeze (if something goes wrong).** One command snapshots-and-quarantines every agent
   matching a rule, preserving full live state for investigation — rather than killing it and losing
   the evidence.

## Where the "superpower" physically lives

Everything that makes Seneca different traces back to **one capability in one place**: the **node
agent's ability to snapshot/pause/resume a full microVM (RAM included)**, fast. The control plane
decides *when* to use it (schedule, budget-stop, approval-pause, freeze); the node agent *does* it;
the snapshot store *keeps* the result. That's the whole machine.

## What's intentionally not here yet

This describes the shape, not an implementation. Open questions a build would need to settle (state
store choice, multi-host coordination, exactly how snapshots are stored and addressed) are out of
scope for these docs. Known technical gotchas of snapshotting machines are listed in
[limitations.md](limitations.md).
