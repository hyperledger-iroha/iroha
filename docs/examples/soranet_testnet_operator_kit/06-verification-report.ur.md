---
lang: ur
direction: rtl
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-11-21T14:25:28.923348+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/soranet_testnet_operator_kit/06-verification-report.md کا اردو ترجمہ -->

## آپریٹر ویریفیکیشن رپورٹ (مرحلہ T0)

- آپریٹر نام: ______________________
- Relay descriptor ID: ______________________
- جمع کرانے کی تاریخ (UTC): ___________________
- رابطہ ای میل / matrix: ___________________

### چیک لسٹ خلاصہ

| آئٹم | مکمل (Y/N) | نوٹس |
|------|-----------------|-------|
| Hardware اور نیٹ ورک ویلیڈیٹڈ | | |
| Compliance block لاگو | | |
| Admission envelope ویریفائیڈ | | |
| Guard rotation smoke test | | |
| Telemetry scrape اور dashboards فعال | | |
| Brownout drill مکمل | | |
| PoW tickets کامیابی ہدف کے اندر | | |

### Metrics Snapshot

- PQ ratio (`sorafs_orchestrator_pq_ratio`): ________
- گزشتہ 24h میں downgrade count: ________
- اوسط circuit RTT (p95): ________ ms
- PoW median solve time: ________ ms

### Attachments

براہ کرم منسلک کریں:

1. Relay support bundle hash (`sha256`): __________________________
2. Dashboard اسکرین شاٹس (PQ ratio، circuit success، PoW histogram).
3. Signed drill bundle (`drills-signed.json` + signer public key hex اور attachments).
4. SNNet-10 metrics report (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`).

### آپریٹر دستخط

میں تصدیق کرتا ہوں کہ اوپر دی گئی معلومات درست ہیں اور تمام مطلوبہ اقدامات مکمل ہو چکے ہیں.

دستخط: _________________________  تاریخ: ___________________

</div>
