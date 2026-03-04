---
lang: fr
direction: ltr
source: docs/source/coordination_llm_prompts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cc5499372cc9b188384254f0bf05386d81a1a57e0388d74ad2ae698e0ab9945e
source_last_modified: "2026-01-03T18:08:00.678386+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Invites de coordination LLM

## Objectif

Ces modèles d'invite aident les ingénieurs à recueillir rapidement les éclaircissements de @mtakemiya
lorsque les éléments de la feuille de route laissent des questions ouvertes. Copiez l'une des sections ci-dessous dans le
fil LLM, remplacez les espaces réservés entre crochets et incluez le fichier ou le fichier pertinent.
références de ligne pour que le contexte reste ancré.

## Décisions d'architecture ou de conception

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

## Configuration ou guide de l'opérateur

````markdown
We are documenting configuration/operator guidance and need input.

**Topic**: [e.g., release artifact selection between Iroha 2 and 3]
**Current draft**: [link or summary of doc/code]

Questions:
1. [How should operators choose between options?]
2. [What safeguards/telemetry should they check?]

Any specific wording or runbook steps you would like us to include?
````

## Primitives de cryptographie ou de protocole

````markdown
Before implementing the next cryptographic/protocol task, we need domain input.

**Roadmap item**: [e.g., Repo/PvP settlement circuits]
**Existing materials reviewed**: [spec references or code paths]

Clarifications requested:
- [Key question about curves/parameters/message layout]
- [Fallback or testing expectations]

Are there mandatory references or acceptance criteria we must observe?
````

## Test de vecteurs ou d'appareils

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

## Ingénierie ou processus de publication

````markdown
Clarification needed on release/coordination steps for [feature or milestone].

**Current plan**: [brief summary of build matrix, packaging, sign-off, etc.]
**Open questions**:
1. [Approval flow or owner question]
2. [Artifact naming/hash requirements]

Let us know the expectations so we can document the release runbook correctly.
````