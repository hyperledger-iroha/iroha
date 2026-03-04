---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sns/local-to-global-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14277cc1a10b7514c5b7234bd5637c0ae39e634d6066ca9a45ba58cf2ecfbf63
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ar
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# مجموعة ادوات عناوين Local -> Global

تعكس هذه الصفحة `docs/source/sns/local_to_global_toolkit.md` من المستودع الاحادي. وهي تجمع ادوات CLI و runbooks المطلوبة لبند خارطة الطريق **ADDR-5c**.

## نظرة عامة

- `scripts/address_local_toolkit.sh` يلف CLI الخاص بـ `iroha` لانتاج:
  - `audit.json` -- خرج منظم من `iroha tools address audit --format json`.
  - `normalized.txt` -- literals IH58 (المفضل) / compressed (`sora`) (الخيار الثاني) محولة لكل selector من نطاق Local.
- استخدم السكربت مع لوحة ingest للعناوين (`dashboards/grafana/address_ingest.json`)
  وقواعد Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) لاثبات ان cutover Local-8 /
  Local-12 امن. راقب لوحات التصادم Local-8 و Local-12 والتنبيهات
  `AddressLocal8Resurgence`, `AddressLocal12Collision`, و `AddressInvalidRatioSlo` قبل
  ترقية تغييرات manifest.
- ارجع الى [Address Display Guidelines](address-display-guidelines.md) و
  [Address Manifest runbook](../../../source/runbooks/address_manifest_ops.md) لسياق UX واستجابة الحوادث.

## الاستخدام

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

الخيارات:

- `--format compressed (`sora`)` لخروج `sora...` بدلا من IH58.
- `--no-append-domain` لاصدار literals بدون نطاق.
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
   `iroha tools address normalize --fail-on-warning --only-local` الى CI حتى تفشل
   محاولات التراجع قبل الوصول الى production.

راجع المستند المصدر لمزيد من التفاصيل وقوائم الادلة و release-note snippet
الذي يمكنك اعادة استخدامه عند اعلان cutover للعملاء.
