---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : développeur-sdk-index
titre : Kit SDK pour SoraFS
sidebar_label : Kit SDK
description: مقتطفات خاصة بكل لغة لدمج آرتيفاكتات SoraFS.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/developer/sdk/index.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

استخدم هذا المحور لتتبع المساعدات الخاصة بكل لغة التي تُشحن مع سلسلة أدوات SoraFS.
Les applications pour Rust sont disponibles dans [SDK Rust] (./developer-sdk-rust.md).

## مساعدات اللغات- **Python** — `sorafs_multi_fetch_local` (اختبارات دخان للمُنسِّق المحلي) et
  `sorafs_gateway_fetch` (pour E2E) pour `telemetry_region` اختياريًا
  مع تجاوز `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` et `"direct-only"`), vous avez besoin de plus d'informations
  CLI. Utilisez Proxy QUIC pour utiliser `sorafs_gateway_fetch`.
  `local_proxy_manifest` est un paquet de confiance pour les applications de confiance.
- **JavaScript** — Logiciel `sorafsMultiFetchLocal` pour Python et les logiciels malveillants
  Paramètres du proxy `sorafsGatewayFetch` et Torii et proxy proxy
  ويعرض نفس تجاوزات التليمترية/النقل الموجودة في CLI.
- **Rust** — يمكن للخدمات تضمين المُجدول مباشرةً عبر `sorafs_car::multi_fetch`؛ راجع
  [مقتطفات Rust SDK](./developer-sdk-rust.md) pour proof-stream et مُنسِّق.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` prend en charge la connexion HTTP
  Pour Torii et pour `GatewayFetchOptions`. ادمجه مع
  `ClientConfig.Builder#setSorafsGatewayUri` et PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) عندما يجب أن تلتزم الرفوعات
  بمسارات PQ فقط.

## مفاتيح scoreboard والسياسات

Utilisez Python (`sorafs_multi_fetch_local`) et JavaScript
(`sorafsMultiFetchLocal`) Tableau de bord pour la CLI :- تمكّن الثنائيات الإنتاجية tableau de bord افتراضيًا؛ Article `use_scoreboard=True`
  (أو وفّر إدخالات `telemetry`) عند إعادة تشغيل luminaires حتى يستخلص المساعد ترتيب
  المزوّدين الموزون من بيانات annonces et ولقطات التليمترية الحديثة.
- اضبط `return_scoreboard=True` لتلقي الأوزان المحسوبة مع إيصالات الـ chunk حتى تتمكن
  سجلات CI من التقاط التشخيصات.
- استخدم مصفوفتَي `deny_providers` et `boost_providers` لرفض الأقران أو إضافة
  `priority_delta` عندما يختار المُجدول المزوّدين.
- حافظ على الوضع الافتراضي `"soranet-first"` ما لم تكن تجهّز لخفض المستوى؛ قدّم
  `"direct-only"` فقط عندما يتعين على منطقة امتثال تجنّب المرحلات أو عند تدريب
  Utilisez SNNet-5a et `"soranet-strict"` pour PQ uniquement.
- تعرض مساعدات البوابة أيضًا `scoreboardOutPath` et `scoreboardNowUnixSecs`. اضبط
  `scoreboardOutPath` Tableau de bord pour tableau de bord (يعكس علم CLI `--scoreboard-out`)
  Utilisez `cargo xtask sorafs-adoption-check` pour le SDK et le SDK.
  `scoreboardNowUnixSecs` Luminaires pour luminaires `assume_now` Luminaires pour luminaires
  وصفية قابلة لإعادة الإنتاج. La version JavaScript est également disponible.
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`؛ وعند حذف الملصق
  Utilisez `region:<telemetryRegion>` (pour solution de secours, `sdk:js`). يصدر مساعد Python تلقائيًا
  `telemetry_source="sdk:python"` Tableau de bord et tableau de bord
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