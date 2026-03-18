---
lang: ar
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# محلي -> عالمي ٹول کٹ

هذه صفحة `docs/source/sns/local_to_global_toolkit.md`. هذه خريطة الطريق **ADDR-5c** هي عبارة عن مساعدين ودليل تشغيل لـ CLI.

##جائزہ

- `scripts/address_local_toolkit.sh` `iroha` CLI لالتفاف الكرتا والتقاطها:
  - `audit.json` -- `iroha tools address audit --format json` كإخراج منظم ۔
  - `normalized.txt` -- ہر محدد المجال المحلي کے لیے I105 (ترجیحی) / مضغوط (`sora`، ثاني أفضل) حرفي۔
- اسکرپٹ کو عنوان لوحة التحكم (`dashboards/grafana/address_ingest.json`)
  ويتم استخدام قواعد Alertmanager (`dashboards/alerts/address_ingest_rules.yml`)
  Local-8 / Local-12 قطع ثابت ثابت. لوحات تصادم محلية 8 و محلية 12 و
  تنبيهات `AddressLocal8Resurgence` و`AddressLocal12Collision` و`AddressInvalidRatioSlo`
  يتم الترويج لبدائل البيان بشكل واضح.
- تجربة المستخدم والاستجابة للحوادث [إرشادات عرض العنوان](address-display-guidelines.md) و
  [دليل بيان العنوان](../../../source/runbooks/address_manifest_ops.md) .

##استخدام

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

اختیارات:

- `--format i105` I105 بجائے `sora...` إخراج .
- `domainless output (default)` هذه مجرد كلمات حرفية.
- تم تحديد خطوة التحويل `--audit-only`.
- `--allow-errors` يقوم بفحص الصفوف المشوهة ويتم فحصها (سلوك CLI).

لقد أصبح السكربت أخيرًا مسارات للقطع الأثرية. دونو فيل
تذكرة إدارة التغيير کے ساتھ منسلک کریں و Grafana لقطة شاشة بھی تشمل کریں جو
>=30 دن تک صفر اكتشافات محلية-8 وصفر تصادمات محلية-12 دکھائے۔

## CI المفقودين1. يتم إنشاء سكربت وظيفة مخصصة ويتم تسجيل المخرجات.
2. الجب `audit.json` سجل المحددات المحلية (`domain.kind = local12`) يدمج روك دي.
   الافتراضي `true` (تم تحديد انحدارات dev/test في وقت `false`) و
   `iroha tools address normalize` الذي يشتمل على تقنية CI
   إنتاج الانحدارات تک پہنچنے سے پہلے فيل ہوں.

مزيد من التفاصيل، وقوائم التحقق من الأدلة، ومقتطف مذكرة الإصدار للحصول على معلومات مفصلة
لذا، سيتم الإعلان عن التخفيض عند إعادة استخدامه.