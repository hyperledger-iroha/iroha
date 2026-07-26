---
lang: ar
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20dd2db9bb7af3c40b35154b927df8000cb8c0d6c8e6190e71fb3e6491403149
source_last_modified: "2025-12-19T22:33:53.968499+00:00"
translation_last_reviewed: 2026-01-01
---

# مجموعة ادوات عناوين Local -> Global

تعكس هذه الصفحة `docs/source/sns/local_to_global_toolkit.md` من المستودع الاحادي. وهي تجمع ادوات CLI و runbooks المطلوبة لبند خارطة الطريق **ADDR-5c**.

## نظرة عامة

- `scripts/address_local_toolkit.sh` يلف CLI الخاص بـ `iroha` لانتاج:
  - `audit.json` -- خرج منظم من `iroha tools address audit --format json`.
  - `normalized.txt` -- literals i105 (المفضل) / i105 (الخيار الثاني) محولة لكل selector من نطاق Local.
- استخدم السكربت مع لوحة ingest للعناوين (`dashboards/grafana/address_ingest.json`)
  وقواعد Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) لاثبات ان cutover Local-8 /
  Local-12 امن. راقب لوحات التصادم Local-8 و Local-12 والتنبيهات
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, و `AddressInvalidRatioSlo` قبل
  ترقية تغييرات manifest.
- ارجع الى [Address Display Guidelines](address-display-guidelines.md) و
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) لسياق UX واستجابة الحوادث.

## الاستخدام

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

الخيارات:

- `--format i105` لخروج `sora...` بدلا من i105.
- `domainless output (default)` لاصدار literals بدون نطاق.
- `--audit-only` لتخطي خطوة التحويل.
- `--allow-errors` للاستمرار عند ظهور صفوف تالفة (مطابق لسلوك CLI).

يطبع السكربت مسارات artefact في نهاية التشغيل. ارفق كلا الملفين مع
تذكرة change-management ومع لقطة Grafana التي تثبت صفر
اكتشافات Local-8 وصفر تصادمات Local-12 لمدة >=30 يوما.

## تكامل CI

1. شغل السكربت في job مخصص وارفع المخرجات.
2. احظر عمليات الدمج عندما يبلغ `audit.json` عن Local selectors (`domain.kind = local12`).
   على القيمة الافتراضية `true` (قم بالتحويل الى `false` فقط على بيئات dev/test عند
   تشخيص التراجعات) واضف
   `iroha tools address normalize` الى CI حتى تفشل
   محاولات التراجع قبل الوصول الى production.

راجع المستند المصدر لمزيد من التفاصيل وقوائم الادلة و release-note snippet
الذي يمكنك اعادة استخدامه عند اعلان cutover للعملاء.
