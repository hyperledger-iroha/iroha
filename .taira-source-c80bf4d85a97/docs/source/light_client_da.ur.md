---
lang: ur
direction: rtl
source: docs/source/light_client_da.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6561551b6f00fb37b8e41fc5ade61206d7bd9323ab8e089f3dd5d5cfdfc0fd53
source_last_modified: "2026-01-03T18:07:57.770085+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# لائٹ کلائنٹ ڈیٹا کی دستیابی کا نمونہ

Sumeragi v2 uses reliable broadcast internally for consensus data availability.
The public first-release Torii API does not expose per-session chunk sampling,
delivery inspection, or global collector-plan endpoints, and there is no
dedicated Torii sampling configuration.

For operator visibility, use `/v1/sumeragi/telemetry` for aggregate RBC backlog
and availability fields and `/v1/sumeragi/status` for the compact consensus
state. These endpoints provide operational telemetry, not light-client data-
availability proofs.
