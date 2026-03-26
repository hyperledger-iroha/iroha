---
lang: ar
direction: rtl
source: docs/portal/docs/sns/local-to-global-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Kit de enderecos محلي -> عالمي

هذه الصفحة مخصصة `docs/source/sns/local_to_global_toolkit.md` من خلال الريبو الأحادي. تم تجميع مساعدي CLI وكتب التشغيل الرائعة من خلال عنصر خريطة الطريق **ADDR-5c**.

## فيساو جيرال

- `scripts/address_local_toolkit.sh` مغلف بـ CLI `iroha` للمنتج:
  - `audit.json`--مؤسسة البناء `iroha tools address audit --format json`.
  - `normalized.txt` - حرف i105 (مفضل) / مضغوط (`sora`) (أفضل خيار ثاني) محول لكل محدد نطاق محلي.
- الجمع بين البرنامج النصي ولوحة التحكم في إدخال enderecos (`dashboards/grafana/address_ingest.json`)
  كما يتم إصلاح Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) للتأكد من قطع Local-8 /
  محلي-12 وآمن. راقب آلام القولون Local-8 و Local-12 والتنبيهات
  `AddressLocal8Resurgence`، `AddressLocal12Collision`، و`AddressInvalidRatioSlo` قبل
  المروج mudancas دي البيان.
- راجع كـ [إرشادات عرض العنوان](address-display-guidelines.md) e o
  [دليل بيان العنوان](../../../source/runbooks/address_manifest_ops.md) لسياق تجربة المستخدم والاستجابة للحوادث.

## أوسو

```bash
scripts/address_local_toolkit.sh       --input fixtures/address/local_digest_examples.txt       --output-dir artifacts/address_migration       --network-prefix 753       --format i105
```

Opcoes:

- `--format i105` لـ صيدا `sora...` em vez de i105.
- `domainless output (default)` لإصدار أدبي بدون ملكية.
- `--audit-only` لبدء المحادثة.
- `--allow-errors` لمواصلة التباين عند ظهور تشوهات الخطوط (مثل سلوك CLI).يستخرج البرنامج النصي طريقتين من الأعمال الفنية في نهاية التنفيذ. Anexe os dois arquivos ao
تذكرة gestao de mudancas الخاصة بك مع لقطة شاشة Grafana التي لا تتوصل إلى الصفر
أجهزة الكشف المحلية-8 وصفر الكوليسو المحلية-12 لمدة> = 30 يوما.

## إنتيجراكاو CI

1. قم بالكتابة في وظيفة مخصصة وحسدها كما تقول.
2. يقوم Bloqueie بدمج محددات تقرير `audit.json` المحلية (`domain.kind = local12`).
   لا توجد شجاعة في `true` (لذا تم التغيير لـ `false` في مجموعات التطوير/الاختبار للتشخيص
   التراجعات) والإضافة
   `iroha tools address normalize` ao CI للتراجع
   falhem antes de chegar a producao.

شاهد الخط المستند لمزيد من التفاصيل وقوائم الأدلة المرجعية ومقتطف من
ملاحظات الإصدار التي يمكنك إعادة استخدامها أو الإعلان عن التحويل للعملاء.