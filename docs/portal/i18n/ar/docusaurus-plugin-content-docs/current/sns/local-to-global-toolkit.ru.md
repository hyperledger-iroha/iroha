---
lang: ar
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# أنواع الأدوات المحلية -> العناوين العالمية

هذا الجزء يخرج `docs/source/sns/local_to_global_toolkit.md` من الريبو الأحادي. تتضمن هذه العناصر مساعدات CLI ودفاتر التشغيل، ويجب أن تكون هناك بطاقات مطلوبة **ADDR-5c**.

## ملاحظة

- `scripts/address_local_toolkit.sh` يدعم CLI `iroha` للحصول على:
  - `audit.json`--الهيكل الهيكلي `iroha tools address audit --format json`.
  - `normalized.txt` - حرفية I105 (مفترض) / مضغوطة (`sora`) (اختيار خارجي) لمحدد المجال المحلي.
- استخدم البرنامج النصي مباشرة باستخدام عنوان استيعاب لوحة القيادة (`dashboards/grafana/address_ingest.json`)
  وضبط Alertmanager (`dashboards/alerts/address_ingest_rules.yml`)، للتأكيد على السلامة الشاملة Local-8 /
  محلي-12. لقطات لوحات التصادم Local-8 و Local-12 والتنبيهات
  `AddressLocal8Resurgence`، `AddressLocal12Collision`، و`AddressInvalidRatioSlo` قبل
  بيان توريد البضائع.
- قم بالتواصل مع [إرشادات عرض العنوان](address-display-guidelines.md) و
  [دليل تشغيل بيان العنوان](../../../source/runbooks/address_manifest_ops.md) لتجربة المستخدم وسياق الاستجابة للحوادث.

##الاستخدام

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

الخيارات:

- `--format I105` للبث `sora...` بدلاً من I105.
- `domainless output (default)` для вывода مجردة.
- `--audit-only` لإجراء تحويلات جديدة.
- `--allow-errors` لمواصلة المسح الضوئي باستخدام خراطيش الحبر (المرتبطة بـ CLI).يقوم البرنامج النصي بإدخال القطع الأثرية في نهاية الاستخدام. قم بتقديم ملف c
تذكرة إدارة التغيير вместе с Grafana لقطة شاشة, подтверодающим ноль
Local-8 اصطدام ولا شيء Local-12 تصادم الحد الأدنى ل> = 30 يومًا.

## التكامل CI

1. قم بتثبيت البرنامج النصي في مهمة رائعة وقم بإلغاء تأمين المخرجات.
2. قم بحظر عمليات الدمج عندما يتم استخدام المحددات المحلية (`audit.json`) (`domain.kind = local12`).
   تم التعرف عليه من خلال `true` (اذكر `false` كثيرًا في التطوير/الاختبار من خلال انحدار التشخيص) و
   قم بإضافة `iroha tools address normalize` في CI للرجوع إلى الانحدار
   انتقل إلى الإنتاج.

سم. مستند موجود للتفاصيل، وقائمة الأدلة، ومقتطف مذكرة الإصدار، الذي يمكن أن يكون
الاستفادة من التخفيض للعملاء.