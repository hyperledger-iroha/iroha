---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: Developer-sdk-index
العنوان: Guias de SDK da SoraFS
Sidebar_label: دليل SDK
الوصف: Trechos por linguagem para integrar artefatos da SoraFS.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sorafs/developer/sdk/index.md`. Mantenha ambas as copias sincronzadas.
:::

استخدم هذا المركز لمرافقة المساعدين للغة التي ترافق سلسلة الأدوات SoraFS.
بالنسبة للمقتطفات الخاصة بـ Rust، ولـ [Rust SDK snippets](./developer-sdk-rust.md).

## مساعدين للغة- **Python** - `sorafs_multi_fetch_local` (اختبارات الدخان يتم إجراؤها محليًا) e
  `sorafs_gateway_fetch` (تمارين البوابة E2E) الآن قبل `telemetry_region`
  اختياري لتجاوز `transport_policy`
  (`"soranet-first"`، `"soranet-strict"` أو `"direct-only"`)، قم بتدوير المقابض
  الطرح يفعل CLI. عند استخدام الوكيل QUIC المحلي، `sorafs_gateway_fetch` retorna o
  بيان المتصفح في `local_proxy_manifest` لتمرير الخصيتين إلى حزمة الثقة
  لمكيفات الملاحة.
- **JavaScript** - `sorafsMultiFetchLocal` مساعدة أو مساعدة في بايثون، إرجاع
  بايتات الحمولة واستئناف الاستقبال أثناء ممارسة `sorafsGatewayFetch`
  البوابات Torii، مجموعة بيانات الوكيل المحلي وعرض تجاوزات الرسائل
  القياس عن بعد/النقل بواسطة CLI.
- **Rust** - يمكن للخدمات تشغيل أو جدولة مباشرة عبر
  `sorafs_car::multi_fetch`; راجع المرجع
  [مقتطفات Rust SDK](./developer-sdk-rust.md) لمساعدي إثبات التدفق
  integracao do orquestrador.
- **Android** - `HttpClientTransport.sorafsGatewayFetch(...)` إعادة استخدام منفذ HTTP
  هل Torii وهورا `GatewayFetchOptions`. الجمع بين كوم
  `ClientConfig.Builder#setSorafsGatewayUri` وتلميح تحميل PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) عند إجراء التحميلات
  ابحث في بعض الطرق عن PQ.

## لوحة النتائج ومقابض السياسة

مساعدي بايثون (`sorafs_multi_fetch_local`) وجافا سكريبت
(`sorafsMultiFetchLocal`) عرض لوحة النتائج للجدولة باستخدام القياس عن بعد
بيلو CLI:- ثنائيات الإنتاج المؤهلة أو لوحة النتائج من خلال اللوحة؛ تعريفنا `use_scoreboard=True`
  (أو للحصول على المدخلات `telemetry`) لإعادة إنتاج التركيبات التي يستمدها منها المساعد
  لقد تم التفكير في المثبتين من خلال Metadados من الإعلانات واللقطات
  أحدث القياسات عن بعد.
- قم بتعريف `return_scoreboard=True` لتلقي العملات المحسوبة مع الإيصالات
  من قطعة، يسمح لك بتسجيل تشخيصات التقاط CI.
- استخدم المصفوفات `deny_providers` أو `boost_providers` لجذب الأقران أو الإضافة
  `priority_delta` عند تحديد جدولة أو تحديد.
- الحفاظ على وضعية الوضع `"soranet-first"` حتى تتمكن من إعداد الرجوع إلى إصدار سابق؛
  Forneca `"direct-only"` فقط عند الامتثال للامتثال يتطلب تجنب المرحلات
  أو جرب SNNet-5a الاحتياطي، واحتفظ بـ `"soranet-strict"` للطيارين PQ فقط
  كوم aprovacao دي Goveranca.
- مساعدو البوابة تمامًا في معرض `scoreboardOutPath` و`scoreboardNowUnixSecs`.
  قم بتعريف `scoreboardOutPath` للاستمرار في حساب لوحة النتائج (إظهار العلم
  `--scoreboard-out` do CLI) حتى يكون `cargo xtask sorafs-adoption-check` صالحًا
  أدوات SDK الاصطناعية، واستخدام `scoreboardNowUnixSecs` عند تحديد التركيبات
  valor `assume_now` estavel para metadados reproduziveis. لا يوجد مساعد دي جافا سكريبت،
  يمكنك تحديد `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`؛
  عند حذف التسمية، فإنها تشتق `region:<telemetryRegion>` (احتياطي لـ `sdk:js`).يقوم مساعد Python بإصدار `telemetry_source="sdk:python"` تلقائيًا عندما
  استمر في لوحة النتائج واحتفظ بالوصفات الضمنية ذات الإعاقة.

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```