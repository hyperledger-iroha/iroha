---
lang: ar
direction: rtl
source: docs/source/soranet_gateway_hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1a7a7fb86b2d307aea1b367c9c83a09b19e24cea3f5f4ccd29937fcae3d80997
source_last_modified: "2025-11-21T15:11:47.334996+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/soranet_gateway_hardening.md -->

# تقسية بوابة SoraGlobal ‏(SNNet-15H)

يجمع مساعد التقسية أدلة الأمان/الخصوصية قبل ترقية إصدارات الـ Gateway.

## الأمر
- `cargo xtask soranet-gateway-hardening --sbom <path> --vuln-report <path> --hsm-policy <path> --sandbox-profile <path> --data-retention-days 30 --log-retention-days 30 --out artifacts/soranet/gateway_hardening`

## المخرجات
- `gateway_hardening_summary.json` — حالة لكل مُدخل (SBOM، تقرير الثغرات، سياسة HSM، ملف sandbox) إضافةً إلى إشارة الاحتفاظ. المُدخلات المفقودة تُظهر `warn` أو `error`.
- `gateway_hardening_summary.md` — ملخص مقروء للبشر لحزم الحوكمة.

## ملاحظات القبول
- يجب أن تتوفر تقارير SBOM والثغرات؛ المُدخلات المفقودة تُخفض الحالة.
- الاحتفاظ لأكثر من 30 يوماً يضع علامة `warn` للمراجعة؛ وفر قيماً افتراضية أكثر تشدداً قبل GA.
- استخدم ملخصات المخرجات كمرفقات لمراجعات GAR/SOC وكتب تشغيل الحوادث.

</div>
