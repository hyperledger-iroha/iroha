---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: Developer-sdk-index
العنوان: أدلة SDK SoraFS
Sidebar_label: أدلة SDK
الوصف: إضافات باللغة لدمج العناصر SoraFS.
---

:::ملاحظة المصدر الكنسي
:::

استخدم هذا المركز لمتابعة المساعدين باللغة المكتوبة باستخدام سلسلة الأدوات SoraFS.
من أجل قصاصات Rust، انتقل إلى [Rust SDK snippets](./developer-sdk-rust.md).

## مساعدين على قدم المساواة اللغة- **Python** — `sorafs_multi_fetch_local` (اختبارات دخان الأوركسترا المحلية) وآخرون
  `sorafs_gateway_fetch` (تمارين البوابة E2E) غير مقبولة
  خيار `telemetry_region` بالإضافة إلى إلغاء `transport_policy`
  (`"soranet-first"`، `"soranet-strict"` أو `"direct-only"`)، مرآة للمقابض
  طرح دو CLI. Lorsqu'un proxy QUIC local démarre،
  `sorafs_gateway_fetch` قم بإعادة تصفح البيان عبر
  `local_proxy_manifest` يوضح أن الاختبارات ترسل حزمة الثقة إلى aux
  محولات التنقل.
- **JavaScript** — `sorafsMultiFetchLocal` يعكس مساعدة بايثون، ويفيدنا
  بايتات الحمولة والسيرة الذاتية للرسالة، بينما يتم تنفيذ `sorafsGatewayFetch`
  البوابات Torii، قم بتمرير بيانات الوكيل المحلي وكشف الصور
  يتجاوز télémétrie/transport que le CLI.
- **Rust** — قد تمنع الخدمات جدولة الجدولة مباشرة عبر
  `sorafs_car::multi_fetch` ; راجع المرجع
  [مقتطفات Rust SDK](./developer-sdk-rust.md) للمساعدين في إثبات التدفق وما إلى ذلك
  تكامل الأوركسترا.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` يعيد استخدام منفذ HTTP
  Torii واحترم `GatewayFetchOptions`. الجمع بين لو مع
  `ClientConfig.Builder#setSorafsGatewayUri` ومؤشر التحميل PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) عند إجراء التحميلات
  بقية على chemins PQ-فقط.

## لوحة النتائج ومقابض السياسةمساعدو بايثون (`sorafs_multi_fetch_local`) وجافا سكريبت
(`sorafsMultiFetchLocal`) يعرض لوحة النتائج الخاصة بالجدولة الأساسية عن بعد المستخدمة
قدم المساواة لو CLI :- ثنائيات الإنتاج النشطة في لوحة النتائج بشكل افتراضي؛ تعريف
  `use_scoreboard=True` (أو قم بتزويد الإدخالات `telemetry`) عند إعادة التشغيل
  تركيبات تساعد على استخلاص الطلب من مقدمي الخدمة من جانبهم
  بيانات الإعلانات واللقطات الحديثة عن بعد.
- تحديد `return_scoreboard=True` لتلقي الأوزان المحسوبة مع les
  Reçus de piece afin que les logs CI تلتقط التشخيصات.
- استخدم اللوحات `deny_providers` أو `boost_providers` لرفض النظراء
  أو قم بإضافة `priority_delta` عند جدولة الموفرين المحددين.
- حافظ على الوضع الافتراضي `"soranet-first"` في حالة الرجوع إلى المستوى الأدنى؛ com.fournissez
  `"direct-only"` فقط عند منطقة المطابقة يجب تجنب المرحلات أو
  عند التكرار الاحتياطي لـ SNNet-5a، وحفظ `"soranet-strict"` للطيارين
  PQ فقط مع موافقة الحكم.
- تعرض البوابة المساعدة أيضًا `scoreboardOutPath` و`scoreboardNowUnixSecs`.
  حدد `scoreboardOutPath` للاستمرار في حساب لوحة النتائج (عرض العلم
  CLI `--scoreboard-out`) يوضح أن `cargo xtask sorafs-adoption-check` صالح للعناصر
  SDK، واستخدم `scoreboardNowUnixSecs` عندما تحتاج التركيبات إلى قيمة
  `assume_now` مستقر للمواد القابلة لإعادة الإنتاج. في مساعد جافا سكريبت،
  يمكنك أيضًا تحديد `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ;إذا كانت التسمية مفقودة، فسيتم اشتقاق `region:<telemetryRegion>` (مع البديل الاحتياطي مقابل `sdk:js`).
  يتم تشغيل مساعد Python تلقائيًا `telemetry_source="sdk:python"` كل مرة
  احتفظ بلوحة النتائج واحفظ العناصر المعطلة الضمنية.

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