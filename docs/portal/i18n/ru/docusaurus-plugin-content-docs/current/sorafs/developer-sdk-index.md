---
id: developer-sdk-index
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Канонический источник
:::

Используйте этот хаб, чтобы отслеживать language helpers, поставляемые с toolchain SoraFS.
Для Rust-специфичных сниппетов переходите к [Rust SDK snippets](./developer-sdk-rust.md).

## Языковые helpers

- **Python** — `sorafs_multi_fetch_local` (smoke tests локального оркестратора) и
  `sorafs_gateway_fetch` (gateway E2E упражнения) теперь принимают опциональный
  `telemetry_region` плюс override для `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` или `"direct-only"`), отражая rollout knobs
  CLI. Когда локальный QUIC proxy поднимается, `sorafs_gateway_fetch` возвращает
  browser manifest в `local_proxy_manifest`, чтобы тесты могли передать trust bundle
  браузерным адаптерам.
- **JavaScript** — `sorafsMultiFetchLocal` отражает Python helper, возвращая payload bytes
  и summaries квитанций, тогда как `sorafsGatewayFetch` упражняет Torii gateways,
  прокидывает manifests локального proxy и раскрывает те же telemetry/transport overrides,
  что и CLI.
- **Rust** — сервисы могут встраивать scheduler напрямую через `sorafs_car::multi_fetch`;
  см. [Rust SDK snippets](./developer-sdk-rust.md) для proof-stream helpers и интеграции
  оркестратора.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` переиспользует Torii HTTP executor
  и учитывает `GatewayFetchOptions`. Комбинируйте с
  `ClientConfig.Builder#setSorafsGatewayUri` и PQ upload hint
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) когда загрузки должны идти по PQ-only путям.

## Scoreboard и policy knobs

Python (`sorafs_multi_fetch_local`) и JavaScript (`sorafsMultiFetchLocal`) helpers
выставляют telemetry-aware scheduler scoreboard, используемый CLI:

- Production бинари включают scoreboard по умолчанию; установите `use_scoreboard=True`
  (или передайте `telemetry` entries) при проигрывании fixtures, чтобы helper выводил
  взвешенный порядок провайдеров из metadata adverts и свежих telemetry snapshots.
- Установите `return_scoreboard=True`, чтобы получать рассчитанные веса вместе с chunk receipts,
  позволяя CI логам фиксировать диагностику.
- Используйте массивы `deny_providers` или `boost_providers` для отклонения peers или добавления
  `priority_delta`, когда scheduler выбирает провайдеров.
- Сохраняйте позу `"soranet-first"` по умолчанию, если только не готовите downgrade;
  указывайте `"direct-only"` лишь когда compliance регион обязан избегать relays или при
  репетиции fallback SNNet-5a, и резервируйте `"soranet-strict"` для PQ-only пилотов с
  одобрением governance.
- Gateway helpers также раскрывают `scoreboardOutPath` и `scoreboardNowUnixSecs`.
  Задайте `scoreboardOutPath` для сохранения вычисленного scoreboard (соответствует флагу
  CLI `--scoreboard-out`), чтобы `cargo xtask sorafs-adoption-check` мог валидировать
  SDK артефакты, и используйте `scoreboardNowUnixSecs`, когда fixtures требуется
  стабильное значение `assume_now` для воспроизводимых metadata. В JavaScript helper можно
  дополнительно установить `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  если label опущен, он выводит `region:<telemetryRegion>` (fallback на `sdk:js`). Python helper
  автоматически пишет `telemetry_source="sdk:python"` при сохранении scoreboard и держит
  implicit metadata выключенными.

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
