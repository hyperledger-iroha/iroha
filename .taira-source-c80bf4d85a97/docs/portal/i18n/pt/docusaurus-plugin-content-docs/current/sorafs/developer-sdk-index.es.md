---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice-sdk do desenvolvedor
título: Guias do SDK de SoraFS
sidebar_label: Guias do SDK
descrição: Fragmentos específicos por idioma para integrar artefatos de SoraFS.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/developer/sdk/index.md`. Mantenha ambas as cópias sincronizadas.
:::

Use este hub para seguir os auxiliares de idioma que você envia com o conjunto de ferramentas de SoraFS.
Para snippets específicos de Rust e [Rust SDK snippets](./developer-sdk-rust.md).

## Ajudantes por idioma

- **Python** — `sorafs_multi_fetch_local` (testes de humor do orquestrador local) e
  `sorafs_gateway_fetch` (exercícios E2E do gateway) agora aceita um `telemetry_region`
  opcional mais uma substituição de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`), refletindo os botões de
  implementação da CLI. Quando um proxy QUIC local é gerado,
  `sorafs_gateway_fetch` devolve o manifesto do navegador em
  `local_proxy_manifest` para que os testes sejam entregues no pacote confiável aos adaptadores
  do navegador.
- **JavaScript** — `sorafsMultiFetchLocal` reflete o auxiliar do Python, devolvendo
  bytes de carga útil e resumos de recibos, enquanto `sorafsGatewayFetch` é emitido
  gateways de Torii, encadena manifestos de proxy local e expõe as substituições incorretas
  de telemetria/transporte que o CLI.
- **Rust** — os serviços podem incrustar o agendador diretamente via
  `sorafs_car::multi_fetch`; consulte a referência de
  [Rust SDK snippets](./developer-sdk-rust.md) para auxiliares de streaming de prova e integração
  do orquestrador.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` reutiliza o ejetor HTTP
  de Torii e respeito `GatewayFetchOptions`. Combinado com
  `ClientConfig.Builder#setSorafsGatewayUri` e a dica de subida PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) quando as subidas forem interrompidas
  um solo de rutas PQ.

## Placar e botões de política

Os ajudantes de Python (`sorafs_multi_fetch_local`) e JavaScript
(`sorafsMultiFetchLocal`) expõe o placar do agendador com telemetria usada
pela CLI:- Os binários de produção habilitam o placar por defeito; estabelecer
  `use_scoreboard=True` (ou fornece entradas de `telemetry`) para reprodução
  fixtures para que o ajudante derive a ordem ponderada dos provedores a partir de
  metadados de anúncios e instantâneos de telemetria recentes.
- Establece `return_scoreboard=True` para receber os pesos calculados junto com os
  recibos de chunk e permitir que os logs de CI capturem diagnósticos.
- Use arreglos `deny_providers` ou `boost_providers` para rechazar peers ou adicionar um
  `priority_delta` quando o agendador seleciona provedores.
- Mantenha a postura predeterminada `"soranet-first"` salva que prepara um downgrade;
  fornece `"direct-only"` somente quando uma região de conformidade deve evitar relés
  ou ensayar o fallback SNNet-5a, e reserve `"soranet-strict"` para pilotos somente PQ
  com aprovação de governo.
- Os ajudantes do gateway também expõem `scoreboardOutPath` e `scoreboardNowUnixSecs`.
  Configure `scoreboardOutPath` para persistir o cálculo do placar (refleja a bandeira
  `--scoreboard-out` da CLI) para que `cargo xtask sorafs-adoption-check` valide artefatos
  de SDK, e usa `scoreboardNowUnixSecs` quando os fixtures exigem um valor estável de
  `assume_now` para metadados reproduzíveis. Um auxiliar de JavaScript também pode
  estabilizador `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`; quando se omite
  a etiqueta deriva `region:<telemetryRegion>` (com substituto para `sdk:js`). El ajudante de
  Python emite automaticamente `telemetry_source="sdk:python"` quando persiste um placar
  e mantenha desativados os metadados implícitos.

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