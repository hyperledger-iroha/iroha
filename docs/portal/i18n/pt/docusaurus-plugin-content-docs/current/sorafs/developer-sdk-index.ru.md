---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice-sdk do desenvolvedor
título: Руководства no SDK SoraFS
sidebar_label: Adicionar ao SDK
description: Use trechos para integração de artefatos SoraFS.
---

:::nota História Canônica
:::

Use este recurso, use ajudantes de linguagem, use o conjunto de ferramentas SoraFS.
Para os fragmentos do Rust SDK, execute-os em [snippets do SDK do Rust](./developer-sdk-rust.md).

## Ajudantes de Языковые

- **Python** — `sorafs_multi_fetch_local` (testes de fumaça локального оркестратора) e
  `sorafs_gateway_fetch` (gateway E2E упражнения) теперь принимают опциональный
  `telemetry_region` substituição adicional para `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`), botões de implementação adicionais
  CLI. Como usar o proxy QUIC local, `sorafs_gateway_fetch` é válido
  manifesto do navegador em `local_proxy_manifest`, esses testes podem ser transferidos para pacote confiável
  Adaptador portátil.
- **JavaScript** — `sorafsMultiFetchLocal` usa o auxiliar Python, gerando bytes de carga útil
  e resumos квитанций, тогда как `sorafsGatewayFetch` упражняет gateways Torii,
  прокидывает manifesta proxy local e раскрывает те же substituições de telemetria/transporte,
  isso e CLI.
- **Rust** — сервисы могут встраивать agendador напрямую через `sorafs_car::multi_fetch`;
  sim. [Rust SDK snippets](./developer-sdk-rust.md) para auxiliares de fluxo de prova e integração
  orquestrador.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` executor HTTP Torii
  e use `GatewayFetchOptions`. Combinar com
  `ClientConfig.Builder#setSorafsGatewayUri` e dica de upload PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`).

## Painel de avaliação e botões de política

Ajudantes Python (`sorafs_multi_fetch_local`) e JavaScript (`sorafsMultiFetchLocal`)
выставляют placar do agendador com reconhecimento de telemetria, используемый CLI:- Produção бинари включают placar по умолчанию; instalar `use_scoreboard=True`
  (ou передайте entradas `telemetry`) при проигрывании fixtures, чтобы helper выводил
  взвешенный порядок провайдеров из anúncios de metadados e instantâneos de telemetria instantâneos.
- Установите `return_scoreboard=True`, чтобы получать рассчитанные веса вместе с pedaço de recibos,
  позволяя CI логам фиксировать диагностику.
- Use `deny_providers` ou `boost_providers` para obter pares ou добавления
  `priority_delta`, este agendador é testado.
- Сохраняйте позу `"soranet-first"` по умолчанию, если только не готовите downgrade;
  указывайте `"direct-only"` лишь когда conformidade região обязан избегать relés или при
  репетиции fallback SNNet-5a, e резервируйте `"soranet-strict"` para pilotos somente PQ com
  governança одобрением.
- Auxiliares de gateway também usam `scoreboardOutPath` e `scoreboardNowUnixSecs`.
  Use `scoreboardOutPath` para obter o placar de segurança (sоответствует флагу
  CLI `--scoreboard-out`), itens `cargo xtask sorafs-adoption-check` podem ser validados
  SDK артефакты, и используйте `scoreboardNowUnixSecs`, когда fixtures требуется
  Defina o valor `assume_now` para obter metadados. No ajudante JavaScript pode ser
  instalação completa `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  nenhum rótulo está disponível, no `region:<telemetryRegion>` (substituto para `sdk:js`). Ajudante Python
  автоматически пишет `telemetry_source="sdk:python"` para painel de avaliação e держит
  metadados implícitos выключенными.

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