---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/norito-streaming-roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8d2f490f9c614c8c95eed352c4636c66937e351418fbf19c1b7753e69f3a3ca4
source_last_modified: "2025-11-14T04:43:20.919297+00:00"
translation_last_reviewed: 2026-01-30
---

ذیل کا backlog وہ قریبی مدتی کام کی اشیاء دکھاتا ہے جنہیں Streaming Working Group Norito audio/video delivery کے لئے ٹریک کرتا ہے۔ اقدار `status.md` کے ساتھ اپ ڈیٹ ہوتی ہیں اور یہاں ان پورٹل صارفین کے لئے شیئر کی جاتی ہیں جو machine-friendly view کو ترجیح دیتے ہیں۔

```json
[
  {
    "id": "NSC-28b",
    "title": "Enforce ±10ms A/V sync tolerance",
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
