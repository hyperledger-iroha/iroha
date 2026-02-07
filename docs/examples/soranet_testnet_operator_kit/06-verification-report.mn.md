---
lang: mn
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-12-29T18:16:35.092552+00:00"
translation_last_reviewed: 2026-02-07
---

## Operator Verification Report (Phase T0)

- Operator name: ______________________
- Relay descriptor ID: ______________________
- Submission date (UTC): ___________________
- Contact email / matrix: ___________________

### Checklist Summary

| Item | Completed (Y/N) | Notes |
|------|-----------------|-------|
| Hardware & network validated | | |
| Compliance block applied | | |
| Admission envelope verified | | |
| Guard rotation smoke test | | |
| Telemetry scraped & dashboards live | | |
| Brownout drill executed | | |
| PoW ticket success within target | | |

### Metrics Snapshot

- PQ ratio (`sorafs_orchestrator_pq_ratio`): ________
- Downgrade count last 24h: ________
- Average circuit RTT (p95): ________ ms
- PoW median solve time: ________ ms

### Attachments

Please attach:

1. Relay support bundle hash (`sha256`): __________________________
2. Dashboard screenshots (PQ ratio, circuit success, PoW histogram).
3. Signed drill bundle (`drills-signed.json` + signer public key hex and attachments).
4. SNNet-10 metrics report (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`).

### Operator Signature

I certify the above information is accurate and all required steps have been
completed.

Signature: _________________________  Date: ___________________
