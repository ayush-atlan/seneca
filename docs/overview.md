# Seneca — Overview

This is the full plain-language story of what Seneca is and why it's built the way it is. If you hit
a word you don't know, check the [glossary](glossary.md) — every term is explained there.

---

## 1. Start with the new shape of the problem: swarms

A year or two ago, an "AI agent" meant one program calling a language model in a loop. Now teams run
**swarms** — several agents working together on one job. A common pattern: a "planner" agent breaks
the work into pieces, a few "worker" agents do the pieces in parallel, and a "reviewer" agent checks
the result before it's accepted.

A swarm is a *team*, and that changes what you need from your infrastructure. You no longer want to
manage agents one at a time. You want to manage the **team as a unit**:

- Start the whole team together, or not at all.
- Give the whole team **one budget**, not five separate untracked bills.
- Pause the whole team when a human needs to approve a step.
- If something goes wrong, contain the whole team for investigation.

Today's tools don't do this, for a reason worth understanding.

## 2. Why today's tools leave a gap

There are two families of tools, and the swarm-as-a-unit falls between them.

**Framework tools** (LangGraph, CrewAI, AutoGen) are about *coordination* — they decide which agent
runs next and how agents pass messages. They're useful, but they run as ordinary software on top of
the operating system. They don't *own* the machine each agent runs on, so they can't do machine-level
things: they can't freeze an agent's live memory, can't truly wall one agent off from another, and
can't guarantee a runaway agent is actually stopped.

**Sandbox vendors** (E2B, Modal, Browserbase) solve the isolation problem well: they give each agent
a strong, locked-down box (often a microVM — see below) so untrusted agent code can't escape and hurt
anything. But their unit is **one box**. They have no idea that five of their boxes are actually one
team that should rise and fall together.

So nobody governs the **swarm** itself. That's the gap Seneca aims at.

## 3. The tool Seneca is built on: Firecracker microVMs

A **virtual machine (VM)** is a fake computer running inside a real one — isolated, so what happens
inside mostly can't touch the host (the real machine). **Firecracker** is an open-source tool from
AWS that makes very small, very fast VMs called **microVMs**: they boot in well under a second and
use little memory. That speed and lightness is exactly what you want for lots of short-lived,
untrusted agent jobs — each agent gets its own disposable mini-computer.

Crucially, because Firecracker owns the whole machine, it can do something ordinary software can't:

> **Take a snapshot — a complete frozen copy of the machine, including its live memory (RAM) — and
> later restore it to the exact same state, in well under a second.**

Think of it like a video game save state: not just "the files on disk," but the *entire running
machine* paused mid-thought, ready to resume as if no time passed.

This one ability is the foundation of everything Seneca offers.

## 4. The four things snapshot unlocks (applied to the whole swarm)

### a) Gang-scheduling: start as a team or not at all

**Gang-scheduling** is an old idea from supercomputing: if a job needs five machines, you launch all
five together or none — you don't start three and let them sit idle waiting for the other two.

Seneca applies this to swarms. A reviewer agent with no workers to review is wasted money. So Seneca
only starts a swarm when it can place *every* member; otherwise it waits. The team rises together.

### b) One shared budget for the team

Agents spend money in three different "currencies" at once: **compute** (the machines they run on),
**tokens** (LLM usage — language models are billed by the amount of text they process), and
**external API spend** (calls to paid services). Because Seneca runs the whole swarm, it can pool
these into **one budget for the team** and enforce a **hard stop** when the limit is reached — so a
swarm can't quietly run up a surprise bill, and you can see which team is spending what.

### c) Approval-pause for free

Agents often need a human to approve a sensitive step ("OK to send this email to the customer?").
The naive way is to keep the agents running while they wait — burning money, sometimes for hours.

Seneca instead **snapshots the swarm and sets it aside at zero ongoing cost**. The machines stop
consuming resources entirely. When the human says yes (even three days later), Seneca **restores the
snapshot and the swarm continues exactly where it paused**, as if no time passed. Human-in-the-loop
stops being an engineering headache and becomes a simple setting.

### d) Forensic freeze: contain, don't destroy

When something goes wrong with normal infrastructure, the reflex is to kill the misbehaving
processes. But killing an agent **destroys the evidence** — you lose what it was thinking and doing
at the moment things went wrong.

Seneca offers **freeze instead of kill**: with one command you can snapshot-and-quarantine every
agent matching a rule — *"freeze everything that touched data source Y in the last hour"* — and the
full live state (memory, disk, recent network activity) is preserved for investigation. You contain
the blast radius **and** keep the evidence. Afterward you can study the frozen swarm at leisure, or
even resume it in a sealed environment to see what it would have done next.

## 5. Why only Seneca can do this

Each of these four is only possible because Seneca **owns the microVMs** and can snapshot full machine
state:

- Framework tools (LangGraph, CrewAI) coordinate agents but can't freeze a swarm's live memory — they
  don't control the machine.
- Per-sandbox vendors (E2B, Modal, Browserbase) can snapshot a single box, but they don't model a
  swarm as one schedulable, budgeted, freezable unit.

Seneca sits exactly at the intersection: **the swarm is the unit, and full-machine snapshot is the
lever.** That intersection is the part of the landscape that's genuinely under-served — see
[prior-art.md](prior-art.md) for the honest details, including what *is* already taken.

## 6. What this overview deliberately leaves out

This is the "why" and the "what." For "how the parts fit," read [architecture.md](architecture.md).
For the honest limits of a first version (and the technical gotchas of snapshotting machines), read
[limitations.md](limitations.md).
