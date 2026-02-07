---
lang: zh-hant
direction: ltr
source: docs/portal/docs/norito-streaming-roadmap.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito Streaming Roadmap
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

以下待辦事項記錄了流式處理中的近期工作項目
Norito 音頻/視頻傳輸的組軌道。值同時更新
`status.md` 並在此分享給喜歡機器友好的門戶消費者
視圖。

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