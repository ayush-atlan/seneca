# Seneca — Limitations & Known Gotchas

Being honest about what a first version would *not* do, and the technical traps that come with
snapshotting machines. If a term is unfamiliar, see the [glossary](glossary.md).

---

## Scope limits of a first version

- **Design stage only.** This repo is docs, not code. Nothing here has been built or measured.
- **Swarm-as-a-unit is the focus; the rest is not.** Seneca deliberately does *not* try to also be a
  taint/IFC system, a credential broker, or a cost-chargeback product — those are
  [already well-covered by others](prior-art.md). Trying to do everything would mean reinventing
  things that exist.
- **Single-cluster assumption.** The first design assumes one cluster of hosts under one control
  plane. Spreading a single swarm across multiple regions or clouds is out of scope.
- **Cooperative agents within a swarm.** The current thesis governs the swarm against the *outside*
  (scheduling, budget, freeze). Strong isolation *between* members of the same swarm (so one
  compromised member can't attack its teammates) is a known concern but not the headline — and the
  network-isolation piece of it overlaps with microsegmentation work others already do.

## Gotchas inherent to snapshotting machines

Snapshotting a full machine (RAM included) is powerful, but restoring or copying a frozen machine has
well-known traps. A real implementation must handle these — they're flagged here so nothing assumes
they're free:

- **Shared randomness.** Two machines restored from the same snapshot start with the *same* internal
  randomness. If left alone, they'd generate identical "random" values — a security bug. On restore,
  the system must **reseed the source of randomness**.
- **Frozen clocks.** A restored machine thinks it's still the moment it was snapshotted. Its **clock
  must be resynced** on resume, or time-based logic misbehaves.
- **Network identity collisions.** Copies of a machine share the same network identity (MAC/IP
  addresses). Restored or forked copies must be **given fresh network identities**, or they'll
  collide on the network.
- **Open connections don't survive.** A live network connection (e.g., an in-progress download) won't
  survive a snapshot/restore. Good restore points are moments when the agent **isn't mid-connection**,
  or the connection must be transparently re-established.

## Cost and performance unknowns

- **Snapshot size.** Snapshotting RAM means snapshots can be large. Storing many of them (for paused
  or frozen swarms) has a real storage cost that a design must account for.
- **Restore latency at scale.** "Sub-second" restore is true per machine; restoring a *large* swarm
  at once, or many swarms together, needs measurement, not assumption.
- **Idle paused swarms.** Approval-pause is "free" in *compute*, but the snapshot still occupies
  storage while it waits. Long-pending approvals accumulate stored state.

## Open questions a build would need to answer

- What stores the control plane's state, and how does it stay consistent across hosts?
- How are snapshots named, deduplicated, and garbage-collected when no longer needed?
- How exactly is a swarm defined and submitted (the shape of the spec the operator writes)?
- What's the policy language for forensic-freeze rules ("freeze everything that touched Y")?

These are intentionally unanswered here — this is the *why* and *what*, not the *how*.
