---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: Developer-sdk-index
العنوان: الدليل SDK لـ SoraFS
Sidebar_label: الدليل SDK
description: مقتطفات خاصة بكل لغة لدمج آرتيفاكتات SoraFS.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/developer/sdk/index.md`. احرص على جميع النسختين متزامنتين إلى أن يتم إيقاف تشغيل مجموعة Sphinx القديمة.
:::

استخدم هذا المحور لتتبع المساعدات الخاصة بكل اللغات التي تُشحن مع سلسلة الأدوات SoraFS.
انتقل إلى [مقتطفات Rust SDK](./developer-sdk-rust.md).

##مساعدات اللغات- **Python** — `sorafs_multi_fetch_local` (اختبارات دخان للمُنسِّق المحلي) و
  `sorafs_gateway_fetch` (تمارين E2E للبوابة) يقبلان الآن `telemetry_region` اختياريًا
  مع تجاوز `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` أو `"direct-only"`)، بما في ذلك تأكيد الأزرار الجديدة في
  سطر الأوامر. عند تشغيل Proxy QUIC المحلي، يعيد `sorafs_gateway_fetch` تحت إصدار مانيفيست
  `local_proxy_manifest` حتى النجاح في بناء حزمة الثقة إلى محولات الإصدارات.
- **JavaScript** — يعكس `sorafsMultiFetchLocal` مساعد بايثون ويعيد بايتات الحمولة وملخصات
  الإيصالات، بينما تحت `sorafsGatewayFetch` بوابات Torii، ويمر مانيفيستات الوكيل المحلية،
  ويعرض نفس تجاوزات التليميترية/النقل الموجود في CLI.
- **Rust** — يمكن الخدمات تشمل المجدول عبر `sorafs_car::multi_fetch`؛ إعادة النظر
  [مقتطفات Rust SDK](./developer-sdk-rust.md) لمساعدات إثبات التدفق وتكامل المُنسِّق.
- **Android** — أعاد `HttpClientTransport.sorafsGatewayFetch(…)` استخدام مُنفّذ HTTP الخاص
  بـ Torii ويلتزم بـ `GatewayFetchOptions`. ادمجه مع
  `ClientConfig.Builder#setSorafsGatewayUri` ومع تلميح رفع PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) عندما يجب أن تلتزم بالوحدات
  بمسارات PQ فقط.

## لوحة النتائج والسياسات

يتواجد كل من مساعد Python (`sorafs_multi_fetch_local`) وJavaScript
(`sorafsMultiFetchLocal`) لوحة النتائج الواعية بالتليمترية التي تستخدمها CLI:- لوجود ثنائيات جديدة للوحة النتائج افتراضيًا؛ اضبط `use_scoreboard=True`
  (أو توافر التدفقات `telemetry`) عند إعادة تشغيل التجهيزات حتى يستخرج المساعد المساعدة
  المحاسبين الموزون من بيانات الإعلانات ولقطات التليميترية الحديثة.
- اضبط `return_scoreboard=True` نهائيا الأوزان المحسوبة مع إيصالات الـ Chunk حتى الصفر
  سجلات CI من التقاط التشخيصات.
- استخدم مصفوفتي `deny_providers` أو `boost_providers` لرفض الأقران أو إضافة
  `priority_delta` عندما يختار المُنظم المُنظم.
- حافظ على الوضع الافتراضي `"soranet-first"` ما لم تكن معداتّز لخفض المستوى؛ و
  `"direct-only"` فقط عندما يتعين على منطقة اين تجنّب المرحلات أو عند التدريب
  ارتداد SNNet-5a، واحجز `"soranet-strict"` لطيارين PQ-only بموافقة ال تور.
- تم عرض مساعدات البوابة أيضًا `scoreboardOutPath` و`scoreboardNowUnixSecs`. اضبط
  `scoreboardOutPath` حفظ لوحة النتائج المحفوظة (يعكس علم CLI `--scoreboard-out`)
  حتى `cargo xtask sorafs-adoption-check` من التحقق من آرتيفاكتات SDK،
  `scoreboardNowUnixSecs` عندما تحتاج التركيبات إلى القيمة `assume_now` ضبط لبيانات
  غير قابلة للوصفية للإنتاج. في مساعد JavaScript يمكنك أيضًا ضبط
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`؛ أنت تحذف الملصق
  يشتق `region:<telemetryRegion>` (مع الرجوع إلى `sdk:js`). يتلقى مساعد بايثون للبيع
  `telemetry_source="sdk:python"` كلما حفظ لوحة النتائج وبقي البيانات الوصفية
  الضمنية معطلة.

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