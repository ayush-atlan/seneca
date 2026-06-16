# Seneca — Glossary

Plain-language definitions of every term used in the Seneca docs. No prior infrastructure knowledge
assumed. Terms are grouped, roughly easiest-first.

---

## The basics

**AI agent**
: A program that uses a large language model to act on its own — deciding what to do next, then doing
it (browsing the web, reading files, calling other software).

**LLM (large language model)**
: The "brain" behind an agent — a model like Claude that reads text and produces text. It's billed by
the amount of text it processes, measured in *tokens*.

**Token**
: The unit LLMs are measured and billed in. Roughly a few characters of text. "This call used 2,000
tokens" means it processed about that much text in and out.

**Swarm**
: A *team* of agents working together on one job — e.g., a planner, several workers, and a reviewer.
Seneca treats the swarm (not the single agent) as the thing it manages.

**Prompt injection**
: An attack where hidden instructions are smuggled into the data an agent reads (like a web page),
hoping the agent obeys them. Not the focus of Seneca's current thesis, but a reason agents are run in
isolated boxes.

## Machines and isolation

**Host**
: The real, physical (or cloud) computer that runs everything — the microVMs and Seneca's own
software.

**VM (virtual machine)**
: A fake computer running inside a real one. It's isolated: what happens inside mostly can't reach the
host. Lets you run untrusted software safely.

**Firecracker**
: An open-source tool from AWS that creates very small, very fast VMs (*microVMs*). It's lightweight
and starts in well under a second, which makes it ideal for running many short-lived agent jobs.

**microVM**
: A small, fast VM made by Firecracker. In these docs, one microVM usually runs one agent.

**Sandbox**
: A safe, isolated box to run untrusted code. Here, a sandbox = one microVM running one agent.

**Guest**
: The software running *inside* a microVM — i.e., the agent. Treated as untrusted (it might be
hijacked), so it's never given more power than necessary.

**Isolation boundary**
: The wall between the untrusted guest and everything else. With microVMs this wall is strong because
it's enforced by the virtual machine, not by trusting the guest to behave.

## Snapshots (the heart of Seneca)

**RAM (memory)**
: The computer's short-term working memory — where a running program keeps what it's currently
thinking about. It's lost when the machine stops... *unless* you snapshot it.

**Snapshot**
: A complete frozen copy of a running machine — including its live RAM — that can be restored later to
the exact same state. Like a video-game save state for a whole computer. Firecracker can do this in
well under a second.

**Pause / resume**
: Stop a microVM in place (pause), then later start it again from the exact same point (resume), using
a snapshot. To the agent, no time passed.

**Restore**
: Bring a snapshot back to life — recreate the running machine from the frozen copy.

**Fork**
: Make a copy of a running machine from a snapshot, so you have two independent copies that can go
their separate ways. (Useful for "what-if" experiments; not a core part of the current thesis.)

## Orchestration and scheduling

**Orchestration**
: The job of deciding *when* and *where* to run things, and managing their whole life (start, pause,
stop). Seneca orchestrates swarms.

**Scheduler**
: The part that decides when a workload can run and on which host. Seneca's scheduler works on whole
swarms.

**Gang-scheduling**
: Launch all members of a group together, or none — never start some and leave them idle waiting for
the rest. Borrowed from supercomputing; Seneca applies it to swarms.

**All-or-nothing placement**
: The result of gang-scheduling: a swarm either gets every machine it needs at once, or it waits — it
never half-starts.

**Control plane / data plane**
: A common split. The *control plane* is the part that decides and remembers (the brain). The *data
plane* is the part that does the actual work. Seneca's control plane tracks swarms and budgets; the
node agents and microVMs are the data plane.

## Money and limits

**Budget**
: A spending limit. Seneca gives a whole swarm one shared budget and stops the swarm when it's hit.

**The three currencies**
: The three ways agents spend money at once — *compute* (machine time), *tokens* (LLM usage), and
*external API spend* (paid third-party services). Seneca tracks all three under one swarm budget.

**Hard stop**
: A limit that actually blocks further spending when reached (versus a "soft" warning that only
notifies). Seneca's budgets are hard stops.

## Networking and operations

**Egress**
: Outbound network traffic — anything an agent tries to send *out* to the internet.

**Approval-pause (human-in-the-loop)**
: Pausing a swarm so a human can approve a sensitive step before it continues. Seneca does this by
snapshotting the swarm at zero ongoing cost and resuming it when the human responds.

**Forensic freeze**
: Snapshot-and-quarantine misbehaving agents (instead of killing them), preserving their full live
state so the incident can be investigated. "Freeze, don't kill."

**Quarantine**
: Isolate something suspicious so it can't cause further harm while it's examined.

**Blast radius**
: How much damage a problem can cause before it's contained. Forensic freeze shrinks it.

## Honest-landscape terms

**Information-flow control (IFC) / taint tracking**
: Tracking where data came from (e.g., "this came from an untrusted web page") and using that to limit
what an agent may do. A real, active research area — and *already well-covered by others*, which is
why it's not Seneca's wedge. See [prior-art.md](prior-art.md).

**Wedge**
: The specific, under-served angle a project bets on to be different. Seneca's wedge is "the swarm as
the unit, with full-machine snapshot as the lever."
