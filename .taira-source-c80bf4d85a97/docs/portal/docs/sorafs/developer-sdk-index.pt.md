---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ffb6e472741a35b5564ab50172a5d25cc7beaa55cff3f1b2d927c6da9baf033f
source_last_modified: "2025-11-15T05:19:35.608986+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: developer-sdk-index
title: Guias de SDK da SoraFS
sidebar_label: Guias de SDK
description: Trechos por linguagem para integrar artefatos da SoraFS.
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/developer/sdk/index.md`. Mantenha ambas as copias sincronizadas.
:::

Use este hub para acompanhar os helpers por linguagem que acompanham a toolchain da SoraFS.
Para snippets especificos de Rust, va para [Rust SDK snippets](./developer-sdk-rust.md).

## Helpers por linguagem

- **Python** - `sorafs_multi_fetch_local` (smoke tests do orquestrador local) e
  `sorafs_gateway_fetch` (exercicios E2E de gateway) agora aceitam um `telemetry_region`
  opcional mais um override de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`), espelhando os knobs de
  rollout do CLI. Quando um proxy QUIC local sobe, `sorafs_gateway_fetch` retorna o
  browser manifest em `local_proxy_manifest` para que os testes passem o trust bundle
  para adaptadores de navegador.
- **JavaScript** - `sorafsMultiFetchLocal` espelha o helper de Python, retornando
  bytes de payload e resumos de recibos, enquanto `sorafsGatewayFetch` exercita
  gateways Torii, encadeia manifests de proxy local e expoe os mesmos overrides
  de telemetria/transporte do CLI.
- **Rust** - servicos podem embutir o scheduler diretamente via
  `sorafs_car::multi_fetch`; veja a referencia de
  [Rust SDK snippets](./developer-sdk-rust.md) para helpers de proof-stream e
  integracao do orquestrador.
- **Android** - `HttpClientTransport.sorafsGatewayFetch(...)` reutiliza o executor HTTP
  do Torii e honra `GatewayFetchOptions`. Combine com
  `ClientConfig.Builder#setSorafsGatewayUri` e o hint de upload PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) quando uploads precisarem
  ficar em caminhos somente PQ.

## Scoreboard e knobs de politica

Os helpers de Python (`sorafs_multi_fetch_local`) e JavaScript
(`sorafsMultiFetchLocal`) expoem o scoreboard do scheduler com telemetria usado
pelo CLI:

- Binarios de producao habilitam o scoreboard por padrao; defina `use_scoreboard=True`
  (ou forneca entradas `telemetry`) ao reproduzir fixtures para que o helper derive
  a ordenacao ponderada de provedores a partir de metadados de advert e snapshots
  recentes de telemetria.
- Defina `return_scoreboard=True` para receber os pesos calculados junto com recibos
  de chunk, permitindo que logs de CI capturem diagnosticos.
- Use arrays `deny_providers` ou `boost_providers` para rejeitar peers ou adicionar
  `priority_delta` quando o scheduler seleciona provedores.
- Mantenha a postura padrao `"soranet-first"` a menos que esteja preparando um downgrade;
  forneca `"direct-only"` apenas quando uma regiao de compliance precisar evitar relays
  ou ao ensaiar o fallback SNNet-5a, e reserve `"soranet-strict"` para pilotos PQ-only
  com aprovacao de governanca.
- Helpers de gateway tambem expoem `scoreboardOutPath` e `scoreboardNowUnixSecs`.
  Defina `scoreboardOutPath` para persistir o scoreboard calculado (espelha o flag
  `--scoreboard-out` do CLI) para que `cargo xtask sorafs-adoption-check` valide
  artefatos de SDK, e use `scoreboardNowUnixSecs` quando fixtures precisarem de um
  valor `assume_now` estavel para metadados reproduziveis. No helper de JavaScript,
  voce tambem pode definir `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  quando o label e omitido, ele deriva `region:<telemetryRegion>` (fallback para `sdk:js`).
  O helper de Python emite automaticamente `telemetry_source="sdk:python"` quando
  persiste um scoreboard e mantem metadados implicitos desabilitados.

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
