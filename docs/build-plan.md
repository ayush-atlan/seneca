# Seneca — Phased Build Plan

Build order is risk order: prove the snapshot lever works before building anything around it. Each
phase ships something runnable and is useless to skip. No control plane, scheduler, or snapshot store
as separate services until a phase actually needs one — for a single host they're a binary, a loop,
and a directory.

> Stack: Rust. Firecracker is driven through its existing REST API over a unix socket — we do not
> wrap or reinvent it.

---

## Phase 0 — Spike: prove the lever (no Seneca code)

**Goal:** snapshot one Firecracker microVM, kill it, restore it, confirm it resumes mid-execution
(RAM intact). Pure shell + Firecracker's snapshot API.

**Done when:** a VM counting in a loop is snapshotted, destroyed, restored, and continues counting
from where it stopped — in under a second.

If this phase is hard or slow, the whole thesis is wrong. Find out now, in a day, with zero code.

---

## Phase 1 — A swarm is a list (boot all-or-nothing)

**Goal:** one binary reads a swarm spec (a JSON file: list of members + resources) and boots a
microVM per member. If any member fails to boot, tear all of them down. That's the entire
gang-scheduling rule.

- Swarm spec = a JSON file. No spec language, no API.
- "Scheduler" = boot-all-or-kill-all loop. Co-scheduling theory is YAGNI on one host.
- microVM lifecycle = calls to the Firecracker API over its UDS.

**Done when:** `seneca up swarm.json` boots N VMs together or none, and `seneca down` stops them.
A bad member in the spec leaves zero VMs running.

---

## Phase 2 — Freeze and resume the whole swarm

**Goal:** snapshot every member, stop them, restore them later to the same state. This single
mechanism is *both* approval-pause and forensic freeze — same code, different trigger. Build it once.

- Snapshot store = a directory of per-member snapshot files. No object store, no content-addressing
  until storage actually hurts.
- **Not optional (security/correctness):** on restore, reseed each VM's randomness, resync its clock,
  and reassign MAC/IP. Skipping these is a real bug, not a simplification — handle them here.

**Done when:** `seneca freeze swarm` stops a running swarm to disk at zero compute cost, and
`seneca resume swarm` brings every member back mid-execution with fresh entropy, correct time, and no
network collisions.

---

## Phase 3 — One shared budget, hard stop

**Goal:** the swarm shares one budget; exhausting it stops the swarm.

- Budget = a counter the agents report spend to (compute seconds, tokens, API spend), checked against
  a limit. A file or in-memory map, not a metering service.
- Hard stop = when the counter crosses the limit, trigger Phase 2's stop. Reuse, don't rebuild.

**Done when:** a swarm given a small budget is stopped automatically once combined spend crosses it,
and the three numbers are attributable to the swarm.

---

## Phase 4 — Operator surface + the parts that pull their weight

**Goal:** make it usable, and only now split out anything that's grown painful.

- CLI verbs: `up / down / freeze / resume / status / freeze-by <rule>`. The forensic "freeze
  everything that touched Y" rule lands here, on top of Phase 2.
- Split the single binary into control-plane / node-agent **only if** you actually run more than one
  host. Until then it's premature. `// ponytail: single binary; split when host #2 appears.`
- Persistent state store, snapshot GC, multi-host coordination: add each the first time its absence
  bites, not before.

**Done when:** an operator can run, pause-for-approval, budget, and forensically freeze a swarm from
the CLI on one host.

---

## What's deliberately deferred (and when to add it)

- **Multi-host / cross-region** → when one host can't hold the swarms you run.
- **Object-store snapshot backend + GC** → when the snapshot directory gets expensive.
- **Inter-member isolation / microsegmentation** → when swarms run mutually untrusted members; overlaps
  with existing tools, so likely *integrate* rather than build (see [prior-art.md](prior-art.md)).
- **Taint/IFC, credential brokering, dry-run, chargeback UIs** → not Seneca's wedge; out of scope unless
  the thesis changes.

## Verification spine (runs from Phase 1 on)

One end-to-end check, grown each phase: boot a 3-member swarm → freeze it → resume it → confirm all
three resumed mid-execution with fresh entropy/time/network → exhaust its budget → confirm auto-stop.
If that script passes, Seneca does what it claims.
