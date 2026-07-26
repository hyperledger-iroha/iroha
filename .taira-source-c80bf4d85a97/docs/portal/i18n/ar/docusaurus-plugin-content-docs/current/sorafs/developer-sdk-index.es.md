---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: Developer-sdk-index
العنوان: Guías de SDK de SoraFS
Sidebar_label: دليل SDK
الوصف: أجزاء محددة باللغة لدمج القطع الأثرية لـ SoraFS.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/developer/sdk/index.md`. احتفظ بنسخ متزامنة.
:::

يُستخدم هذا المركز لتتبع المساعدين باللغة التي يتم إرسالها باستخدام سلسلة الأدوات SoraFS.
لمقتطفات Rust المحددة و[Rust SDK snippets](./developer-sdk-rust.md).

## مساعدون للغة- **Python** — `sorafs_multi_fetch_local` (اختبارات بشرية للمشغل المحلي) ذ
  `sorafs_gateway_fetch` (الأدوات الإلكترونية E2E للبوابة) الآن قبلت `telemetry_region`
  اختياري أكثر من تجاوز `transport_policy`
  (`"soranet-first"`، `"soranet-strict"` أو `"direct-only"`)، تعكس المقابض
  طرح CLI. عندما يكون لديك وكيل QUIC محلي،
  `sorafs_gateway_fetch` يقوم بإعادة بيان المتصفح إلى
  `local_proxy_manifest` لكي تقوم الاختبارات بتعزيز حزمة الثقة للمتكيفين
  ديل نافيغادور.
- **JavaScript** — `sorafsMultiFetchLocal` يعيد تطوير مساعد بايثون
  بايتات الحمولة وملخصات الاستقبال، أثناء تشغيل `sorafsGatewayFetch`
  بوابات Torii، تقوم بجمع بيانات الوكيل المحلي وإظهار التجاوزات نفسها
  القياس عن بعد/النقل الذي CLI.
- **Rust** — يمكن إضافة الخدمات إلى المجدول مباشرة عبر
  `sorafs_car::multi_fetch`; راجع المرجع
  [Rust SDK snippets](./developer-sdk-rust.md) لمساعدي إثبات الدفق والتكامل
  ديل أوركيستادور.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` إعادة استخدام مُخرج HTTP
  دي Torii واحترم `GatewayFetchOptions`. كومبينالو يخدع
  `ClientConfig.Builder#setSorafsGatewayUri` وتلميح دعم PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) عندما يتم رفع مستوى الدعم
  rutas منفردا PQ.

## لوحة النتائج والمقابض السياسية

مساعدو بايثون (`sorafs_multi_fetch_local`) وجافا سكريبت
(`sorafsMultiFetchLocal`) يعرض لوحة النتائج للجدولة باستخدام القياس عن بعد
من خلال CLI:- ثنائيات الإنتاج قادرة على تأهيل لوحة النتائج بشكل خاطئ؛ establece
  `use_scoreboard=True` (نسبة إدخالات `telemetry`) إلى النسخ
  تركيبات لكي يستمد المساعد الأمر من الموردين من الخارج
  أحدث بيانات التعريف للإعلانات ولقطات القياس عن بعد.
- تعيين `return_scoreboard=True` لاستلام البيزوات المحسوبة إلى جانبها
  إيصالات القطعة والسماح لسجلات CI بالتقاط التشخيص.
- الولايات المتحدة الأمريكية أدخلت `deny_providers` أو `boost_providers` لإعادة توجيه النظراء أو إضافة اشتراك
  `priority_delta` عند اختيار المجدول.
- حافظ على الوضعية المحددة مسبقًا `"soranet-first"` التي تستعد للرجوع إلى إصدار أقدم؛
  نسبة `"direct-only"` فقط عندما تمنع منطقة الامتثال من المرحلات
  o رحلة العودة SNNet-5a، وحجز `"soranet-strict"` للطيارين PQ فقط
  مع موافقة الحكومة.
- تم عرض مساعدي البوابة أيضًا على `scoreboardOutPath` و`scoreboardNowUnixSecs`.
  تكوين `scoreboardOutPath` لمواصلة حساب لوحة النتائج (لإعادة العلم)
  `--scoreboard-out` del CLI) لكي يقوم `cargo xtask sorafs-adoption-check` بصلاحية المصنوعات اليدوية
  de SDK، والولايات المتحدة الأمريكية `scoreboardNowUnixSecs` عندما تتطلب التركيبات قيمة ثابتة
  `assume_now` للنسخ الوصفية. يمكن أيضًا استخدام مساعد JavaScript
  استابيسر `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`; cuando se omite
  العلامة المشتقة `region:<telemetryRegion>` (مع الخيار الاحتياطي `sdk:js`). المساعد ديتقوم Python بإصدار `telemetry_source="sdk:python"` تلقائيًا عند الاستمرار في لوحة النتائج
  والحفاظ على البيانات الوصفية الضمنية غير صالحة.

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