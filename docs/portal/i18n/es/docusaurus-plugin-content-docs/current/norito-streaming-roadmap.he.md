---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/norito-streaming-roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d46f5db0324e28da5689cd7cd5e0d7c5b81c2460b177034eb8d8ef35af427bec
source_last_modified: "2025-11-14T04:43:20.917824+00:00"
translation_last_reviewed: 2026-01-30
---

El siguiente backlog recoge los elementos de trabajo a corto plazo que el Streaming Working Group sigue para la entrega de audio/video de Norito. Los valores se actualizan junto a `status.md` y se comparten aqui para los usuarios del portal que prefieren una vista amigable para maquinas.

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
