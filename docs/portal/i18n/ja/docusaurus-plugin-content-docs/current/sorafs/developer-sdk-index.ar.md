---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: developer-sdk-index
title: أدلة SDK لـ SoraFS
sidebar_label: أدلة SDK
description: مقتطفات خاصة بكل لغة لدمج آرتيفاكتات SoraFS.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/developer/sdk/index.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

استخدم هذا المحور لتتبع المساعدات الخاصة بكل لغة التي تُشحن مع سلسلة أدوات SoraFS.
للمقتطفات الخاصة بـ Rust انتقل إلى [مقتطفات Rust SDK](./developer-sdk-rust.md).

## مساعدات اللغات

- **Python** — `sorafs_multi_fetch_local` (اختبارات دخان للمُنسِّق المحلي) و
  `sorafs_gateway_fetch` (تمارين E2E للبوابة) يقبلان الآن `telemetry_region` اختياريًا
  مع تجاوز `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` أو `"direct-only"`)، بما يعكس أزرار الإطلاق في
  CLI. عند تشغيل Proxy QUIC محلي، يعيد `sorafs_gateway_fetch` مانيفست المتصفح تحت
  `local_proxy_manifest` حتى تتمكن الاختبارات من تمرير trust bundle إلى محولات المتصفح.
- **JavaScript** — يعكس `sorafsMultiFetchLocal` مساعد Python ويعيد بايتات الحمولة وملخصات
  الإيصالات، بينما يمارس `sorafsGatewayFetch` بوابات Torii، ويمرر مانيفستات proxy المحلية،
  ويعرض نفس تجاوزات التليمترية/النقل الموجودة في CLI.
- **Rust** — يمكن للخدمات تضمين المُجدول مباشرةً عبر `sorafs_car::multi_fetch`؛ راجع
  [مقتطفات Rust SDK](./developer-sdk-rust.md) لمساعدات proof-stream وتكامل المُنسِّق.
- **Android** — يعيد `HttpClientTransport.sorafsGatewayFetch(…)` استخدام مُنفّذ HTTP الخاص
  بـ Torii ويلتزم بـ `GatewayFetchOptions`. ادمجه مع
  `ClientConfig.Builder#setSorafsGatewayUri` ومع تلميح رفع PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) عندما يجب أن تلتزم الرفوعات
  بمسارات PQ فقط.

## مفاتيح scoreboard والسياسات

يعرّض كل من مساعد Python (`sorafs_multi_fetch_local`) وJavaScript
(`sorafsMultiFetchLocal`) لوحة scoreboard الواعية بالتليمترية التي يستخدمها CLI:

- تمكّن الثنائيات الإنتاجية scoreboard افتراضيًا؛ اضبط `use_scoreboard=True`
  (أو وفّر إدخالات `telemetry`) عند إعادة تشغيل fixtures حتى يستخلص المساعد ترتيب
  المزوّدين الموزون من بيانات adverts ولقطات التليمترية الحديثة.
- اضبط `return_scoreboard=True` لتلقي الأوزان المحسوبة مع إيصالات الـ chunk حتى تتمكن
  سجلات CI من التقاط التشخيصات.
- استخدم مصفوفتَي `deny_providers` أو `boost_providers` لرفض الأقران أو إضافة
  `priority_delta` عندما يختار المُجدول المزوّدين.
- حافظ على الوضع الافتراضي `"soranet-first"` ما لم تكن تجهّز لخفض المستوى؛ قدّم
  `"direct-only"` فقط عندما يتعين على منطقة امتثال تجنّب المرحلات أو عند تدريب
  ارتداد SNNet-5a، واحجز `"soranet-strict"` لطيارين PQ-only بموافقة الحوكمة.
- تعرض مساعدات البوابة أيضًا `scoreboardOutPath` و`scoreboardNowUnixSecs`. اضبط
  `scoreboardOutPath` لحفظ لوحة scoreboard المحسوبة (يعكس علم CLI `--scoreboard-out`)
  حتى يتمكن `cargo xtask sorafs-adoption-check` من التحقق من آرتيفاكتات SDK، واستخدم
  `scoreboardNowUnixSecs` عندما تحتاج fixtures إلى قيمة `assume_now` ثابتة لبيانات
  وصفية قابلة لإعادة الإنتاج. في مساعد JavaScript يمكنك أيضًا ضبط
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`؛ وعند حذف الملصق
  يشتق `region:<telemetryRegion>` (مع fallback إلى `sdk:js`). يصدر مساعد Python تلقائيًا
  `telemetry_source="sdk:python"` كلما حفظ لوحة scoreboard ويُبقي البيانات الوصفية
  الضمنية معطّلة.

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
