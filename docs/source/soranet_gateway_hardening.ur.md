---
lang: ur
direction: rtl
source: docs/source/soranet_gateway_hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1a7a7fb86b2d307aea1b367c9c83a09b19e24cea3f5f4ccd29937fcae3d80997
source_last_modified: "2025-11-21T15:11:47.334996+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- docs/source/soranet_gateway_hardening.md کا اردو ترجمہ -->

# SoraGlobal Gateway ہارڈننگ (SNNet-15H)

ہارڈننگ ہیلپر Gateway بلڈز کو پروموٹ کرنے سے پہلے سکیورٹی/پرائیویسی شواہد جمع کرتا ہے۔

## کمانڈ
- `cargo xtask soranet-gateway-hardening --sbom <path> --vuln-report <path> --hsm-policy <path> --sandbox-profile <path> --data-retention-days 30 --log-retention-days 30 --out artifacts/soranet/gateway_hardening`

## آؤٹ پٹس
- `gateway_hardening_summary.json` — ہر ان پٹ (SBOM، vuln رپورٹ، HSM پالیسی، sandbox پروفائل) کے لیے اسٹیٹس اور ریٹینشن سگنل۔ غائب ان پٹس پر `warn` یا `error` دکھایا جاتا ہے۔
- `gateway_hardening_summary.md` — گورننس پیکٹس کے لیے انسان دوست خلاصہ۔

## قبولیت نوٹس
- SBOM اور vuln رپورٹس موجود ہونا ضروری ہیں؛ غائب ان پٹس اسٹیٹس کم کرتے ہیں۔
- 30 دن سے زیادہ ریٹینشن پر `warn` لگتا ہے؛ GA سے پہلے مزید سخت ڈیفالٹس فراہم کریں۔
- خلاصہ آرٹیفیکٹس کو GAR/SOC ریویوز اور واقعہ رن بکس کے ساتھ بطور منسلکات استعمال کریں۔

</div>
