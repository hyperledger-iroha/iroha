---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/norito-streaming-roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 76f831f12d2776407bd0d697578cbdb6ceedc3a9833d4fa4a6cf0dbbd7c024ed
source_last_modified: "2025-11-14T04:43:20.918615+00:00"
translation_last_reviewed: 2026-01-30
---

Le backlog suivant capture les elements de travail a court terme que le Streaming Working Group suit pour la diffusion audio/video Norito. Les valeurs sont mises a jour en meme temps que `status.md` et partagees ici pour les utilisateurs du portail qui preferent une vue exploitable par machine.

```json
[
  {
    "id": "NSC-28b",
    "title": "Enforce 10ms A/V sync tolerance",
    "owner": "Streaming Runtime TL",
    "priority": "streaming runtime",
    "status": "planning",
    "dependencies": [
      "telemetry onboarding"
    ],
    "notes": "Draft telemetry signal spec and schedule validator instrumentation review."
  },
  {
    "id": "NSC-30a",
    "title": "Relay incentive & reputation framework",
    "owner": "Economics WG",
    "priority": "economics",
    "status": "planning",
    "dependencies": [
      "telemetry metrics"
    ],
    "notes": "Workshop scheduled for 2026-03-09; prepare incentive whitepaper outline."
  },
  {
    "id": "NSC-37a",
    "title": "Finalize ZK ticket schema",
    "owner": "ZK Working Group",
    "priority": "zk",
    "status": "planning",
    "dependencies": [
      "schema review"
    ],
    "notes": "Circulate schema draft and book review with Core Host / Streaming leads."
  },
  {
    "id": "NSC-42",
    "title": "Codec legal & patent review",
    "owner": "Legal & Standards",
    "priority": "codec",
    "status": "done",
    "dependencies": [],
    "notes": "Completed: counsel sign-off recorded in docs/source/soranet/nsc-42-legal.md; CABAC stays opt-in via ENABLE_CABAC + [streaming.codec], trellis remains disabled, bundled rANS is behind ENABLE_RANS_BUNDLES."
  },
  {
    "id": "NSC-55",
    "title": "Validate rANS tables and patent posture",
    "owner": "Codec Team",
    "priority": "codec",
    "status": "done",
    "dependencies": [],
    "notes": "Completed: deterministic rANS tables + generator checked in with benches/reports per nsc55 plan."
  }
]
```
