---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: Developer-sdk-index
العنوان: Руководства по SDK SoraFS
Sidebar_label: Руководства по SDK
الوصف: مقتطفات رائعة لتكامل قطعة أثرية SoraFS.
---

:::note Канонический источник
:::

استخدم هذه الميزة لتتبع مساعدي اللغة، الذين يتم نشرهم باستخدام سلسلة الأدوات SoraFS.
للمقتطفات الخاصة بـ Rust، انتقل إلى [Rust SDK snippets](./developer-sdk-rust.md).

## مساعدين رائعين- **Python** — `sorafs_multi_fetch_local` (اختبارات الدخان للمدير المحلي) و
  `sorafs_gateway_fetch` (إدارة البوابة E2E) الخطوة الأولى اختيارية
  `telemetry_region` بالإضافة إلى التجاوز لـ `transport_policy`
  (`"soranet-first"`، `"soranet-strict"` أو `"direct-only"`)، مقابض الطرح الخارجية
  سطر الأوامر. عندما يتم تحديث وكيل QUIC المحلي، يتم إلغاء `sorafs_gateway_fetch`
  بيان المتصفح في `local_proxy_manifest`، حتى تتمكن من إجراء الاختبارات قبل حزمة الثقة
  محول بروزرنيم.
- **JavaScript** — `sorafsMultiFetchLocal` يزيل مساعد Python، وينتج بايتات الحمولة
  وملخصات الحياة، حيث يقوم `sorafsGatewayFetch` بتشغيل بوابات Torii،
  يعرض الوكيل المحلي ويكشف عن تجاوزات القياس عن بعد/النقل،
  هذا وCLI.
- **Rust** — يمكن للخدمات إنشاء برنامج جدولة عبر `sorafs_car::multi_fetch`;
  سم. [مقتطفات Rust SDK](./developer-sdk-rust.md) لمساعدي إثبات الدفق والتكامل
  أوركيستراتورا.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` ينقل منفذ Torii HTTP
  وتعلم `GatewayFetchOptions`. الجمع بين س
  `ClientConfig.Builder#setSorafsGatewayUri` وتلميح تحميل PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) عندما يتم توفير المزيد من البيانات لـ PQ-only.

## لوحة النتائج ومقابض السياسة

مساعدو بايثون (`sorafs_multi_fetch_local`) وجافا سكريبت (`sorafsMultiFetchLocal`)
يمكنك الحصول على لوحة نتائج الجدولة المدركة للقياس عن بعد، باستخدام CLI:- يتضمن الإنتاج الثنائي لوحة النتائج في حالة التحسن؛ تثبيت `use_scoreboard=True`
  (أو قم بإعادة إدخالات `telemetry`) عند تثبيت التركيبات، لتتمكن من توصيلها بالمساعد
  أحدث مزودي إعلانات البيانات الوصفية ولقطات القياس عن بعد.
- تثبيت `return_scoreboard=True` للحصول على إيصالات متعددة في وقت واحد،
  يسمح بإصلاح شريط التشخيص.
- استخدم `deny_providers` أو `boost_providers` بكميات كبيرة لإلغاء استنساخ الأقران أو الإضافة
  `priority_delta`، عندما يتم تحديد المجدول.
- قم بالاشتراك في الوضع `"soranet-first"` للترقية، إذا لم تقم بالرجوع إلى الإصدار السابق كثيرًا؛
  تأكد من `"direct-only"` عندما تقوم منطقة الامتثال باختيار المرحلات أو قبل ذلك
  التكرارات الاحتياطية SNNet-5a، والحجز على `"soranet-strict"` للطيارين PQ فقط
  одобением الحكم.
- مساعدي البوابة يكملون أيضًا `scoreboardOutPath` و`scoreboardNowUnixSecs`.
  أضف `scoreboardOutPath` للحفاظ على لوحة النتائج الرائعة (علم رائع)
  CLI `--scoreboard-out`)، يمكن التحقق من صحة `cargo xtask sorafs-adoption-check`
  عناصر SDK، وتستخدم `scoreboardNowUnixSecs`، عندما تحتاج التركيبات
  الاسم الثابت `assume_now` لبيانات التعريف المتصلة. يمكن استخدام مساعد جافا سكريبت
  يتم تثبيته بشكل إضافي `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  إذا تم إلغاء التسمية، على `region:<telemetryRegion>` (الاحتياطي لـ `sdk:js`). مساعد بايثونالرسالة التلقائية `telemetry_source="sdk:python"` مع لوحة النتائج والنتائج
  استثناءات البيانات الوصفية الضمنية.

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