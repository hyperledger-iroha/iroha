---
lang: zh-hans
direction: ltr
source: docs/source/coordination_llm_prompts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cc5499372cc9b188384254f0bf05386d81a1a57e0388d74ad2ae698e0ab9945e
source_last_modified: "2025-12-29T18:16:35.936935+00:00"
translation_last_reviewed: 2026-02-07
---

# LLM Coordination Prompts

## Purpose

These prompt templates help engineers quickly gather clarifications from @mtakemiya
when roadmap items leave open questions. Copy one of the sections below into the
LLM thread, replace the bracketed placeholders, and include relevant file or
line references so context stays anchored.

## Architecture Or Design Decisions

````markdown
We need clarification on an open design point from the roadmap.

**Context**
- Feature/phase: [e.g., Kaigi Privacy Phase 3 — Relay Overlay]
- Current implementation state: [short summary of what exists today]
- Blocking question(s):
  1. [First question]
  2. [Second question, if any]

**Constraints we already know**
- Determinism requirements: [notes]
- Performance/telemetry targets: [notes]
- Security assumptions: [notes]

Could you provide the expected decision or additional constraints so we can
finish the implementation?
````

## Configuration Or Operator Guidance

````markdown
We are documenting configuration/operator guidance and need input.

**Topic**: [e.g., release artifact selection between Iroha 2 and 3]
**Current draft**: [link or summary of doc/code]

Questions:
1. [How should operators choose between options?]
2. [What safeguards/telemetry should they check?]

Any specific wording or runbook steps you would like us to include?
````

## Cryptography Or Protocol Primitives

````markdown
Before implementing the next cryptographic/protocol task, we need domain input.

**Roadmap item**: [e.g., Repo/PvP settlement circuits]
**Existing materials reviewed**: [spec references or code paths]

Clarifications requested:
- [Key question about curves/parameters/message layout]
- [Fallback or testing expectations]

Are there mandatory references or acceptance criteria we must observe?
````

## Test Vectors Or Fixtures

````markdown
We are preparing tests/fixtures for [feature]. Could you confirm the expected
vectors or provide guidance?

**Implementation snapshot**: [branch or file summary]
**Needed vectors**:
- [Vector or scenario]
- [Another scenario, if relevant]

Do we have canonical test data, or should we synthesise vectors using the
current spec? Please confirm so we can keep CI deterministic.
````

## Release Engineering Or Process

````markdown
Clarification needed on release/coordination steps for [feature or milestone].

**Current plan**: [brief summary of build matrix, packaging, sign-off, etc.]
**Open questions**:
1. [Approval flow or owner question]
2. [Artifact naming/hash requirements]

Let us know the expectations so we can document the release runbook correctly.
````
