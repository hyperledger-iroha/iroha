---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice-sdk do desenvolvedor
título: Guias SDK SoraFS
sidebar_label: SDK de guias
descrição: Extratos de idioma para integrar os artefatos SoraFS.
---

:::nota Fonte canônica
:::

Use este hub para seguir os auxiliares no idioma fornecido com o conjunto de ferramentas SoraFS.
Para os snippets Rust, vá para [Rust SDK snippets](./developer-sdk-rust.md).

## Ajudantes por idioma

- **Python** — `sorafs_multi_fetch_local` (testa a fumaça do orquestrador local) e
  `sorafs_gateway_fetch` (exercícios E2E de gateway) aceita desormar um
  Opção `telemetry_region` mais uma substituição de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`), no espelho dos botões de
  implementação da CLI. Lorsqu'un proxy QUIC início local,
  `sorafs_gateway_fetch` reenvia o manifesto do navegador via
  `local_proxy_manifest` para que os testes transmitam o pacote confiável aux
  adaptadores navegador.
- **JavaScript** — `sorafsMultiFetchLocal` reflete o ajudante Python, revelando-os
  bytes de carga útil e os currículos de recursos, tanto que `sorafsGatewayFetch` exerce
  les gateways Torii, passe os manifestos do proxy local e exponha os mesmos
  substitui a telemetria/transporte da CLI.
- **Rust** — os serviços podem embarcar o agendador diretamente via
  `sorafs_car::multi_fetch`; consulte a referência
  [Rust SDK snippets](./developer-sdk-rust.md) para os auxiliares de prova de fluxo e
  a integração do orquestrador.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` reutiliza o executável HTTP
  Torii e respeito `GatewayFetchOptions`. Combine-o com
  `ClientConfig.Builder#setSorafsGatewayUri` e índice de upload PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) quando os uploads forem feitos
  rester sur des chemins somente PQ.

## Placar e botões políticos

Os ajudantes Python (`sorafs_multi_fetch_local`) e JavaScript
(`sorafsMultiFetchLocal`) expõe o placar do agendador baseado no telefone usado
pela CLI:- Os binários de produção ativam o placar por padrão; definição
  `use_scoreboard=True` (ou fornece entradas `telemetry`) ao reproduzir
  fixtures afin que le helper deriva a ordem pondera dos provedores a partir de
  vídeos de anúncios e instantâneos de televisão recentes.
- Defina `return_scoreboard=True` para receber os pesos calculados com os
  recursos de pedaços para que os logs CI capturem os diagnósticos.
- Utilize as tabelas `deny_providers` ou `boost_providers` para rejeitar pares
  ou adicione um `priority_delta` ao selecionar os provedores do agendador.
- Mantenha a postura padrão `"soranet-first"`, salvo em caso de downgrade; Fournissez
  `"direct-only"` apenas em uma região de conformidade, evite os relés ou
  para uma repetição do fallback SNNet-5a, e reserve `"soranet-strict"` para pilotos
  PQ somente com aprovação de governo.
- Os gateways auxiliares expostos também a `scoreboardOutPath` e `scoreboardNowUnixSecs`.
  Defina `scoreboardOutPath` para persistir o cálculo do placar (espelho da bandeira
  CLI `--scoreboard-out`) depois que `cargo xtask sorafs-adoption-check` valida os artefatos
  SDK, e use `scoreboardNowUnixSecs` quando os equipamentos precisarem de um valor
  `assume_now` estável para medições reprodutíveis. No ajudante JavaScript,
  você também pode definir `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ;
  se o rótulo for omitido, o resultado será `region:<telemetryRegion>` (com substituto para `sdk:js`).
  O ajudante Python foi criado automaticamente `telemetry_source="sdk:python"` de cada vez
  persista um placar e guarde as metas implícitas desativadas.

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