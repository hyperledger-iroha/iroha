---
lang: he
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/norito-streaming-roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f69d3794b9b508ce447d5599bf7f2f7b862bae44fe2f0ad2f6a16837eb313ee8
source_last_modified: "2025-11-21T18:11:12.471911+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: מפת דרכים של Norito Streaming
---

רשימת ה-backlog הבאה מרכזת את פריטי העבודה לטווח הקרוב ש-Streaming Working Group עוקב אחריהם עבור אספקת אודיו/וידאו של Norito. הערכים מתעדכנים לצד `status.md` ומשותפים כאן עבור משתמשי הפורטל שמעדיפים תצוגה ידידותית למכונה.

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
