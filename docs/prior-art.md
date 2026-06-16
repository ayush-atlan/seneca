# Seneca — Prior Art (the honest landscape)

A project like this is only credible if it's honest about what already exists. This page maps the
landscape: what's **already taken** (so Seneca shouldn't claim it as new), and exactly **where
Seneca's wedge sits**. We checked these before settling on the thesis.

> **Short version:** almost every agent-security idea you can think of is already being built by
> someone. The one genuinely under-served spot — and the one only a microVM owner can fill — is
> *treating the swarm as the unit and using full-machine snapshot as the lever.* That's Seneca.

---

## Already taken — do NOT claim these as novel

| Idea | Who's already doing it |
|---|---|
| **Taint tracking / information-flow control** for agents (track where data came from, limit actions accordingly) | Microsoft Research's *Fides* system — ["Securing AI Agents with Information-Flow Control"](https://arxiv.org/abs/2505.23643); Simon Willison's widely-cited ["lethal trifecta"](https://simonw.substack.com/p/the-lethal-trifecta-for-ai-agents) framing; [neuro-symbolic taint analysis](https://www.geordie.ai/resources/the-new-attack-surface-why-ai-agents-need-taint-analysis) |
| **JIT credential brokering** (the sandbox never holds real secrets; a proxy injects them at the wire) | [Browserbase](https://www.browserbase.com/blog/what-is-firecracker) substitutes credentials at the host-side network interface; [Infisical](https://infisical.com/blog/credential-brokering-for-ai-agents) has a credential-broker writeup/product |
| **Host-side chokepoint** (everything the agent does flows through one host-controlled proxy) | Standard practice among serious sandbox vendors (Browserbase, the Kubernetes-native enforcement crowd) |
| **Deterministic record-replay** for debugging agents | Crowded: [debugg.ai](https://debugg.ai/resources/taming-heisenbugs-deterministic-replay-sandboxes-code-debugging-ai), `rr`-style harnesses, multiple 2026 writeups |
| **Plan-vs-apply / dry-run** before executing side effects | Becoming standard: [data443 "pre-execution control layer"](https://data443.com/blog/controlling-ai-actions-pre-execution-control-layer/), dry-run agent patterns |
| **Cost attribution / chargeback / budget enforcement** for agents | A whole category already: [Prefactor](https://prefactor.tech/learn/what-is-agent-cost-attribution), [TrueFoundry](https://www.truefoundry.com/blog/llm-cost-attribution-team-budgets) |
| **Microsegmentation** between agents (who may talk to whom) | Mature security category now aimed at agents: [Elisity](https://www.elisity.com/blog/ai-agent-network-security-microsegmentation-2026) |

If Seneca touches any of these, it should *reuse the known approach and say so* — not pretend it
invented it.

## The neighbors, and what they don't do

- **Framework / orchestration tools** — LangGraph, CrewAI, AutoGen. Great at *coordinating* agents
  (who runs next, who messages whom). But they run as ordinary software and **don't own the machine**,
  so they can't snapshot a swarm's live memory, can't enforce true isolation between members, and
  can't guarantee a runaway agent is stopped. Even [TrueFoundry's own analysis](https://www.truefoundry.com/blog/multi-agent-architecture)
  notes that state and governance — not scheduling at the machine layer — are the unsolved pains.
- **Per-sandbox vendors** — E2B, Modal, Browserbase. Excellent at isolating a *single* agent in a
  microVM, and some already snapshot one box. But their **unit is one box** — they don't model a
  group of boxes as a single schedulable, budgeted, freezable team.

## Where Seneca's wedge sits

Seneca lives in the gap between those two neighbors:

> **The swarm is the unit of orchestration, and full-machine snapshot (RAM included) is the lever.**

That combination powers four capabilities — gang-scheduling, one shared budget, free approval-pause,
and forensic freeze — *applied to the whole swarm at once*. Framework tools can't do it because they
don't own the machine; per-sandbox vendors don't do it because they don't model the swarm as a unit.

## Honest caveat about novelty

Even this wedge is **integration novelty, not pure invention**:

- **Gang-scheduling** is decades old in supercomputing and Kubernetes (co-scheduling).
- **Forensic snapshots** are standard in cloud incident response — for long-lived servers.
- **Snapshot/pause/resume** is a Firecracker feature, not something Seneca invents.

What's under-served is *applying these, together, to ephemeral agent swarms as a single governed
unit.* That's a real and defensible place to stand — but Seneca should describe itself as "combining
known-good building blocks into a shape nobody ships yet," not as inventing the building blocks.

The space moves fast, so this page should be revisited periodically — a gap that's open today may
close.

## Sources

- [Securing AI Agents with Information-Flow Control (arXiv)](https://arxiv.org/abs/2505.23643)
- [The lethal trifecta for AI agents — Simon Willison](https://simonw.substack.com/p/the-lethal-trifecta-for-ai-agents)
- [Why AI agents need taint analysis — Geordie AI](https://www.geordie.ai/resources/the-new-attack-surface-why-ai-agents-need-taint-analysis)
- [What is Firecracker? — Browserbase](https://www.browserbase.com/blog/what-is-firecracker)
- [Credential Brokering for AI Agents — Infisical](https://infisical.com/blog/credential-brokering-for-ai-agents)
- [Controlling AI Actions: Pre-Execution Control Layer — data443](https://data443.com/blog/controlling-ai-actions-pre-execution-control-layer/)
- [Deterministic Replay & Sandboxes — debugg.ai](https://debugg.ai/resources/taming-heisenbugs-deterministic-replay-sandboxes-code-debugging-ai)
- [What is Agent Cost Attribution? — Prefactor](https://prefactor.tech/learn/what-is-agent-cost-attribution)
- [LLM Cost Attribution at Scale — TrueFoundry](https://www.truefoundry.com/blog/llm-cost-attribution-team-budgets)
- [Multi-Agent Architecture — TrueFoundry](https://www.truefoundry.com/blog/multi-agent-architecture)
- [AI Agent Network Security & Microsegmentation — Elisity](https://www.elisity.com/blog/ai-agent-network-security-microsegmentation-2026)
