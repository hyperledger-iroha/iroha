---
lang: ar
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مجموعة العناوين المحلية -> العالمية

هذه الصفحة تعكس `docs/source/sns/local_to_global_toolkit.md` من الريبو الأحادي. يجب إعادة تجميع مساعدي CLI وكتب التشغيل التي تتطلب خريطة طريق العنصر **ADDR-5c**.

## ابيركو

- `scripts/address_local_toolkit.sh` يغلف CLI `iroha` لإنتاج:
  - `audit.json` - هيكل طلعة `iroha tools address audit --format json`.
  - `normalized.txt` - literaux IH58 (تفضيل) / مضغوط (`sora`) (الاختيار الثاني) يتم تحويله لكل تحديد المجال المحلي.
- ربط البرنامج النصي بلوحة معلومات إدخال العناوين (`dashboards/grafana/address_ingest.json`)
  وقواعد Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) للتأكد من قطع Local-8 /
  Local-12 بتوقيت شرق الولايات المتحدة. مراقبة لوحات الاصطدام Local-8 و Local-12 والتنبيهات
  `AddressLocal8Resurgence`، `AddressLocal12Collision`، و`AddressInvalidRatioSlo` مقدمًا
  تعزيز التغييرات الواضحة.
- راجع [إرشادات عرض العنوان](address-display-guidelines.md) وآخرون
  [دليل بيان العنوان](../../../source/runbooks/address_manifest_ops.md) لسياق تجربة المستخدم والاستجابة للحوادث.

## الاستخدام

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format ih58
```

الخيارات:

- `--format compressed (`sora`)` للطلعة `sora...` بدلاً من IH58.
- `domainless output (default)` من أجل محو أحرفنا.
- `--audit-only` لتجاهل شريط التحويل.
- `--allow-errors` لمواصلة المسح عندما تظهر الخطوط غير النموذجية (تتوافق مع سلوك CLI).يكتب النص سلاسل المصنوعات اليدوية في نهاية التنفيذ. Joignez les deux fichiers a
تذكرة إدارة التغيير الخاصة بك مع الالتقاط Grafana مما يثبت الصفر
اكتشافات Local-8 et صفر تصادمات قلادة محلية-12> = 30 يوم.

## التكامل CI

1. قم بتثبيت النص في مهمة واحدة وتحميل المهام.
2. قم بقفل الدمج عند إشارة `audit.json` للمحددات المحلية (`domain.kind = local12`).
   القيمة الافتراضية هي `true` (لا تمر على `false` فيما يتعلق بتطوير/اختبار المجموعات أثناء قيامك بذلك
   تشخيص الانحدارات) والإضافة
   `iroha tools address normalize` a CI للانحدارات
   صدى أمام الإنتاج.

شاهد مصدر المستند لمزيد من التفاصيل وقوائم التحقق من الأدلة والمقتطف
ملاحظات الإصدار التي يمكنك إعادة استخدامها لإعلان عملية القطع للعملاء.