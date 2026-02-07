---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice-sdk do desenvolvedor
título: Guias do SDK SoraFS
sidebar_label: guias do SDK
description: Artefatos SoraFS integram کرنے کے لیے زبان مخصوص snippets۔
---

:::nota مستند ماخذ
:::

اس hub کو استعمال کریں تاکہ Cadeia de ferramentas SoraFS کے ساتھ آنے والے rastreamento de ajudantes de linguagem ہو سکیں۔
Rust مخصوص snippets کے لیے [Rust SDK snippets](./developer-sdk-rust.md) دیکھیں۔

## Ajudantes de idioma

- **Python** — `sorafs_multi_fetch_local` (testes de fumaça do orquestrador local)
  `sorafs_gateway_fetch` (exercícios de gateway E2E) ou `telemetry_region` opcional
  Substituição `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` ou `"direct-only"`) بالکل CLI rollout knobs کی طرح۔
  جب proxy QUIC local چلتا ہے تو Manifesto do navegador `sorafs_gateway_fetch` کو
  `local_proxy_manifest` میں واپس کرتا ہے تاکہ testa pacote confiável e adaptadores de navegador تک
  پہنچا سکیں۔
- **JavaScript** — `sorafsMultiFetchLocal` Python helper e espelho کرتا ہے, bytes de carga útil
  اور resumos de recibos واپس کرتا ہے، جبکہ `sorafsGatewayFetch` Torii gateways کو exercício کرتا ہے،
  manifestos de proxy local کو thread کرتا ہے، اور CLI جیسے substituições de telemetria/transporte exposição کرتا ہے۔
- **Rust** — agendador de serviços کو براہ راست `sorafs_car::multi_fetch` کے ذریعے incorporar کر سکتے ہیں؛
  auxiliares de fluxo de prova e integração do orquestrador کے لیے [Rust SDK snippets](./developer-sdk-rust.md) دیکھیں۔
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` Torii Reutilização de executor HTTP کرتا ہے اور
  `GatewayFetchOptions` کو honra کرتا ہے۔ `ClientConfig.Builder#setSorafsGatewayUri` `ClientConfig.Builder#setSorafsGatewayUri`
  Dica de upload PQ (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) کے ساتھ combinar کریں جب uploads
  کو Caminhos somente PQ پر رکھنا ضروری ہو۔

## Painel de avaliação e botões de política

Python (`sorafs_multi_fetch_local`) e JavaScript (`sorafsMultiFetchLocal`) helpers CLI
placar do agendador com reconhecimento de telemetria کو expor کرتے ہیں:- Padrão do placar de binários de produção طور پر ativar کرتے ہیں؛ replay dos jogos
  `use_scoreboard=True` (entradas `telemetry`) دیں تاکہ metadados de anúncio auxiliar e telemetria recente
  instantâneos سے derivação ponderada do pedido do provedor کرے۔
- `return_scoreboard=True` set کریں تاکہ pesos computados chunk recibos کے ساتھ مل سکیں اور logs CI
  captura de diagnóstico کر سکیں۔
- `deny_providers` یا `boost_providers` matrizes استعمال کریں تاکہ peers rejeitam ہوں یا `priority_delta`
  adicionar ou provedores de agendadores selecionar کرے۔
- Postura padrão `"soranet-first"` برقرار رکھیں جب تک estágio de downgrade نہ ہو؛ `"direct-only"` صرف تب دیں
  جب região de conformidade کو relés سے بچنا ہو یا ensaio de fallback SNNet-5a ہو، اور `"soranet-strict"`
  کو Pilotos somente PQ کے لیے aprovação de governança کے ساتھ reserva کریں۔
- Auxiliares de gateway `scoreboardOutPath` e `scoreboardNowUnixSecs` بھی expor کرتے ہیں۔ `scoreboardOutPath`
  set کریں تاکہ placar computado persist ہو (CLI `--scoreboard-out` flag کی طرح) اور
  Artefatos do SDK `cargo xtask sorafs-adoption-check` validam
  fixtures کو metadados reproduzíveis کے لیے valor `assume_now` estável چاہیے ہو۔ Ajudante JavaScript
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` بھی set کیے جا سکتے ہیں؛ اگر rótulo omitir ہو تو
  E `region:<telemetryRegion>` deriva کرتا ہے (fallback `sdk:js`). Ajudante Python para persistir placar
  `telemetry_source="sdk:python"` خودکار طور پر emitir کرتا ہے اور metadados implícitos کو desativado رکھتا ہے۔

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