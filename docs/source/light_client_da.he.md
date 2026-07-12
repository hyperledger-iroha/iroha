<!-- Hebrew translation of docs/source/light_client_da.md -->

---
lang: he
direction: rtl
source: docs/source/light_client_da.md
status: complete
translator: manual
---

<div dir="rtl">

# דגימת זמינות נתונים ללייט-קליינטים

Sumeragi v2 uses reliable broadcast internally for consensus data availability.
The public first-release Torii API does not expose per-session chunk sampling,
delivery inspection, or global collector-plan endpoints, and there is no
dedicated Torii sampling configuration.

For operator visibility, use `/v1/sumeragi/telemetry` for aggregate RBC backlog
and availability fields and `/v1/sumeragi/status` for the compact consensus
state. These endpoints provide operational telemetry, not light-client data-
availability proofs.

</div>
