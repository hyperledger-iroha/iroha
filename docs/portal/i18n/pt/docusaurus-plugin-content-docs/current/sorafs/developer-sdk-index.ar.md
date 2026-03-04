---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice-sdk do desenvolvedor
título: SDK do SDK para SoraFS
sidebar_label: SDK do SDK
description: مقتطفات خاصة بكل لغة لدمج آرتيفاكتات SoraFS.
---

:::note المصدر المعتمد
Verifique o valor `docs/source/sorafs/developer/sdk/index.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

Para obter mais informações, consulte SoraFS.
Use o Rust SDK para executar o Rust SDK](./developer-sdk-rust.md).

## مساعدات اللغات

- **Python** — `sorafs_multi_fetch_local` (rede de código aberto) e
  `sorafs_gateway_fetch` (E2E E2E) para `telemetry_region`
  مع تجاوز `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`)
  CLI. O proxy QUIC é definido como `sorafs_gateway_fetch`.
  `local_proxy_manifest` é um pacote confiável que pode ser usado para configurar o pacote confiável.
- **JavaScript** — `sorafsMultiFetchLocal` é usado para usar Python e usar o Python
  O nome de usuário `sorafsGatewayFetch` ou Torii e o proxy proxy
  Você não pode alterar o valor/serviço da CLI na CLI.
- **Rust** — يمكن للخدمات تضمين المُجدول مباشرةً عبر `sorafs_car::multi_fetch`; راجع
  [مقتطفات Rust SDK](./developer-sdk-rust.md) fornece fluxo de prova e teste de fluxo de prova.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` para usar o HTTP HTTP
  É Torii e é `GatewayFetchOptions`. ادمجه مع
  `ClientConfig.Builder#setSorafsGatewayUri` e `ClientConfig.Builder#setSorafsGatewayUri` e PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`).
  بمسارات PQ فقط.

## مفاتيح placar والسياسات

Como usar Python (`sorafs_multi_fetch_local`) e JavaScript
(`sorafsMultiFetchLocal`) O placar do placar é definido como CLI:

- تمكّن الثنائيات الإنتاجية placar افتراضيًا؛ Modelo `use_scoreboard=True`
  (أو وفّر إدخالات `telemetry`) عند إعادة تشغيل fixtures حتى يستخلص المساعد ترتيب
  المزوّدين الموزون من بيانات anúncios e ولقطات التليمترية الحديثة.
- اضبط `return_scoreboard=True` لتلقي الأوزان المحسوبة مع إيصالات الـ chunk حتى تتمكن
  Coloque CI no lugar certo.
- Use o `deny_providers` ou `boost_providers` para obter mais informações
  `priority_delta` não é compatível com o produto.
- حافظ على الوضع الافتراضي `"soranet-first"` ما لم تكن تجهّز لخفض المستوى؛ قدّم
  `"direct-only"` é um dispositivo de teste que pode ser usado para proteger o computador e o computador.
  O SNNet-5a e o `"soranet-strict"` são compatíveis apenas com PQ.
- Verifique os valores de `scoreboardOutPath` e `scoreboardNowUnixSecs`. ضبط
  `scoreboardOutPath` é um placar de placar (com CLI `--scoreboard-out`)
  Obtenha `cargo xtask sorafs-adoption-check` através do SDK e do SDK
  `scoreboardNowUnixSecs` عندما تحتاج fixtures إلى قيمة `assume_now` ثابتة لبيانات
  وصفية قابلة لإعادة الإنتاج. O JavaScript não está disponível para você
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`; وعند حذف الملصق
  É `region:<telemetryRegion>` (como substituto para `sdk:js`). Aprenda a usar Python
  `telemetry_source="sdk:python"` كلما حفظ لوحة scoreboard ويُبقي البيانات الوصفية
  Não há problema.

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