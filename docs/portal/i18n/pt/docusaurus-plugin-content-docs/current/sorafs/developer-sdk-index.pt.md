---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice-sdk do desenvolvedor
título: Guias de SDK da SoraFS
sidebar_label: Guias do SDK
description: Trechos por linguagem para integrar artefatos do SoraFS.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/developer/sdk/index.md`. Mantenha ambas as cópias sincronizadas.
:::

Use este hub para acompanhar os ajudantes por linguagem que acompanham a cadeia de ferramentas do SoraFS.
Para trechos específicos de Rust, vá para [Rust SDK snippets](./developer-sdk-rust.md).

## Ajudantes de linguagem

- **Python** - `sorafs_multi_fetch_local` (testes de fumaça do orquestrador local) e
  `sorafs_gateway_fetch` (exercícios E2E de gateway) agora aceito um `telemetry_region`
  opcional mais uma substituição de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`), espelhando os botões de
  implementação da CLI. Quando um proxy QUIC local sobe, `sorafs_gateway_fetch` retorna o
  browser manifest em `local_proxy_manifest` para que os testes passem o trust bundle
  para adaptadores de navegador.
- **JavaScript** - `sorafsMultiFetchLocal` espelha o helper de Python, retornando
  bytes de payload e resumos de recibos, enquanto `sorafsGatewayFetch` exercita
  gateways Torii, encadeia manifestos de proxy local e expoe os mesmos overrides
  de telemetria/transporte do CLI.
- **Rust** - serviços podem embutir o agendador diretamente via
  `sorafs_car::multi_fetch`; veja a referência de
  [Rust SDK snippets](./developer-sdk-rust.md) para auxiliares de prova-stream e
  integração do orquestrador.
- **Android** - `HttpClientTransport.sorafsGatewayFetch(...)` reutiliza o executor HTTP
  do Torii e honra `GatewayFetchOptions`. Combinar com
  `ClientConfig.Builder#setSorafsGatewayUri` e a dica de upload PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) quando uploads precisamrem
  ficar em caminhos somente PQ.

## Placar e botões de política

Os ajudantes de Python (`sorafs_multi_fetch_local`) e JavaScript
(`sorafsMultiFetchLocal`) expoem o placar do agendador com telemetria usado
pelo CLI:- Binários de produção habilitados o placar por padrão; definição `use_scoreboard=True`
  (ou forneca entradas `telemetry`) ao reproduzir fixtures para que o helper derive
  a ordenação ponderada de provedores a partir de metadados de anúncios e snapshots
  recentes de telemetria.
- Defina `return_scoreboard=True` para receber os pesos calculados junto com recibos
  de chunk, permitindo que logs de CI capturem diagnósticos.
- Use arrays `deny_providers` ou `boost_providers` para rejeitar peers ou adicionar
  `priority_delta` quando o agendador seleciona provedores.
- Mantenha a postura padrão `"soranet-first"` a menos que esteja preparando um downgrade;
  forneca `"direct-only"` apenas quando uma região de conformidade precisar evitar relés
  ou ao ensaiar o fallback SNNet-5a, e reserve `"soranet-strict"` para pilotos PQ-only
  com aprovação de governança.
- Helpers de gateway também expoem `scoreboardOutPath` e `scoreboardNowUnixSecs`.
  Defina `scoreboardOutPath` para persistir o cálculo do placar (espelha o flag
  `--scoreboard-out` do CLI) para que `cargo xtask sorafs-adoption-check` seja válido
  artistas de SDK, e usam `scoreboardNowUnixSecs` quando fixtures precisamrem de um
  valor `assume_now` estavel para metadados reproduziveis. Sem ajudante de JavaScript,
  você também pode definir `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  quando o rótulo é omitido, ele deriva `region:<telemetryRegion>` (fallback para `sdk:js`).
  O ajudante do Python emite automaticamente `telemetry_source="sdk:python"` quando
  persistir um placar e manter metadados implícitos desabilitados.

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