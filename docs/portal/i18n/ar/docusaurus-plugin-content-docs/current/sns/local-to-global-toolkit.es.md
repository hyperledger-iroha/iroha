---
lang: ar
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مجموعة التوجيهات المحلية -> العالمية

تعكس هذه الصفحة `docs/source/sns/local_to_global_toolkit.md` من المستودع الأحادي. قم بتضمين مساعدي CLI ودفاتر التشغيل المطلوبة لعنصر خريطة الطريق **ADDR-5c**.

## السيرة الذاتية

- `scripts/address_local_toolkit.sh` يغلف CLI `iroha` لإنتاج:
  - `audit.json` - هيكل البنية الخارجة من `iroha tools address audit --format json`.
  - `normalized.txt` - تحويل حرفي I105 (مفضل) / مضغوط (`sora`) (الخيار الأفضل الثاني) لكل محدد نطاق محلي.
- دمج النص مع لوحة معلومات إدخال الاتجاهات (`dashboards/grafana/address_ingest.json`)
  وأنظمة Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) لاختبار القطع Local-8 /
  محلي - 12 آمن. شاهد لوحات التصادم Local-8 و Local-12 والتنبيهات
  `AddressLocal8Resurgence`، `AddressLocal12Collision`، و`AddressInvalidRatioSlo` قبل
  المروج تغيير البيان.
- مرجع إلى [إرشادات عرض العنوان](address-display-guidelines.md) إلخ
  [دليل بيان العنوان](../../../source/runbooks/address_manifest_ops.md) لسياق تجربة المستخدم والاستجابة للحوادث.

## أوسو

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

الخيارات:

- `--format I105` للخروج `sora...` في مكان I105.
- `domainless output (default)` للإصدار الحرفي بدون ملكية.
- `--audit-only` لحذف خطوة التحويل.
- `--allow-errors` لمتابعة المسح عند ظهور ملفات مشوهة (تتزامن مع سلوك CLI).يصف النص مسارات القطع الأثرية في نهاية عملية الإخراج. Adjunta ambos archives أ
تذكرة إدارة التغييرات جنبًا إلى جنب مع لقطة شاشة Grafana التي تختبرها
اكتشافات محلية 8 وتصادمات محلية 12 لمدة > = 30 يومًا.

## التكامل CI

1. قم بتشغيل البرنامج النصي في مهمة مخصصة وإخراج نتائجه.
2. يدمج الحظر عند تحديد `audit.json` للمحددات المحلية (`domain.kind = local12`).
   في شجاعتك المعيبة `true` (تجاوز منفردًا `false` في مجموعات التطوير/اختبار al
   الانحدارات التشخيصية) والموافقة عليها
   `iroha tools address normalize` a CI لهذه الأغراض
   تراجع الانحدار قبل أن يصبح إنتاجًا.

قم بمراجعة المستند بشكل فعال للحصول على المزيد من التفاصيل وقوائم الأدلة والمقتطفات
ملاحظات الإصدار التي يمكنك إعادة استخدامها للإعلان عن عملية القطع للعملاء.