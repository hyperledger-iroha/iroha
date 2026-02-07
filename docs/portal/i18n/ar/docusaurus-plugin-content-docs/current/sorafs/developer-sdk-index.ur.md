---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: Developer-sdk-index
العنوان: أدلة SoraFS SDK
Sidebar_label: أدلة SDK
الوصف: تدمج القطع الأثرية SoraFS مقتطفات کرنے کے لیے زبان مخصوص۔
---

:::ملاحظة مستند ماخذ
:::

يستخدم المركز سلسلة أدوات SoraFS التي تعمل على تتبع مساعدي اللغة.
مقتطفات Rust مخصوص کے لیے [Rust SDK snippets](./developer-sdk-rust.md) دیکھیں.

## مساعدو اللغة- **Python** — `sorafs_multi_fetch_local` (اختبارات دخان المنسق المحلي) اور
  `sorafs_gateway_fetch` (تمارين البوابة E2E) اب اختياري `telemetry_region` اور
  `transport_policy` تجاوز قبول کرتے ہیں
  (`"soranet-first"`، `"soranet-strict"` أو `"direct-only"`)، بما في ذلك مقابض طرح CLI.
  جب وكيل QUIC المحلي چلتا ہے تو `sorafs_gateway_fetch` بيان المتصفح کو
  `local_proxy_manifest` اتصال إنترنت واختبار حزمة الثقة لمحولات المتصفح
  پہنشا سكي..
- **JavaScript** — `sorafsMultiFetchLocal` Python helper ومرآة للكرتا، بايتات الحمولة
  وملخصات الاستلام واپس كرتا ہے، جبکہ `sorafsGatewayFetch` Torii بوابات کو تمرين کرتا ہے،
  تظهر بيانات الوكيل المحلي في مؤشر ترابط كرتا ہے، وكشف تجاوزات النقل/القياس عن بعد لـ CLI عن کرتا ہے.
- **Rust** — جدولة الخدمات التي تم تضمينها لـ `sorafs_car::multi_fetch`؛
  مساعدات إثبات الدفق وتكامل المنسق لـ [Rust SDK snippets](./developer-sdk-rust.md) موصى به.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` Torii إعادة استخدام منفذ HTTP
  `GatewayFetchOptions` شرف كرتا. هو`ClientConfig.Builder#setSorafsGatewayUri` و
  تلميح تحميل PQ (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) وهو يجمع بين التحميلات
  إن مسارات PQ فقط ضرورية للغاية.

## لوحة النتائج ومقابض السياسة

تساعد Python (`sorafs_multi_fetch_local`) وJavaScript (`sorafsMultiFetchLocal`) واجهة سطر الأوامر (CLI)
لوحة نتائج الجدولة المدركة للقياس عن بعد والتي تكشف عن النتائج:- لوحة النتائج الافتراضية لثنائيات الإنتاج، قم بتمكين البطاقة؛ إعادة المباريات کرتے الوقت
  `use_scoreboard=True` (أو `telemetry` الإدخالات) هذا هو مساعد الإعلان عن البيانات التعريفية والقياس عن بعد الحديث
  اللقطات سے اشتقاق ترتيب المزود المرجح کرے۔
- `return_scoreboard=True` تعيين إيصالات قطع الأوزان المحسوبة وسجلات البريد الإلكتروني وCI
  التقاط التشخيص کر سكای۔
- `deny_providers` أو `boost_providers` تستخدم المصفوفات رفض أقرانها أو `priority_delta`
  أضف ہو جب موفري الجدولة حدد کرے۔
- الوضع الافتراضي `"soranet-first"` لم يتم الانتهاء منه بعد؛ `"direct-only"` ينفق تب دي
  منطقة الامتثال التي تقوم بترحيل بروفة احتياطية أو SNNet-5a و`"soranet-strict"`
  يحصل طيارو PQ فقط على موافقة الإدارة كاحتياطي ثابت.
- يقوم مساعدو البوابة `scoreboardOutPath` و`scoreboardNowUnixSecs` بكشف البطاقة. `scoreboardOutPath`
  تعيين استمرار لوحة النتائج المحسوبة (علامة CLI `--scoreboard-out` للتخطيط) و
  التحقق من صحة عناصر `cargo xtask sorafs-adoption-check` SDK، و`scoreboardNowUnixSecs`
  تحتوي التركيبات على بيانات وصفية قابلة للتكرار ذات قيمة `assume_now` مستقرة. مساعد جافا سكريبت
  تم تعيين `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` أيضًا على ما هو محدد؛ إذا التسمية حذفت ہو تو
  و`region:<telemetryRegion>` تشتق كرتا ہے (الاحتياطي `sdk:js`). مساعد بايثون جب لوحة النتائج تستمر في كرتا ہے لك
  يقوم `telemetry_source="sdk:python"` بإصدار بيانات التعريف الضمنية وتعطيلها.```python
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