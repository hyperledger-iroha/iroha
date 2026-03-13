---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 39cbd5e448c8a868c50401c15466d7159cb08ad7be52dafb9e9dc66d5bba979d
source_last_modified: "2025-11-17T18:34:13.669847+00:00"
translation_last_reviewed: 2026-01-01
---

# Cronograma de adocao do Norito-RPC

> As notas canonicas de planejamento vivem em `docs/source/torii/norito_rpc_adoption_schedule.md`.  
> Esta copia do portal resume as expectativas de rollout para autores de SDK, operadores e revisores.

## Objetivos

- Alinhar cada SDK (Rust CLI, Python, JavaScript, Swift, Android) no transporte binario Norito-RPC antes do toggle de producao AND4.
- Manter gates de fase, bundles de evidencia e hooks de telemetria deterministas para que a governanca possa auditar o rollout.
- Tornar trivial capturar evidencia de fixtures e canary com os helpers compartilhados citados no roadmap NRPC-4.

## Cronograma de fases

| Fase | Janela | Escopo | Criterios de saida |
|------|--------|--------|--------------------|
| **P0 - Paridade de laboratorio** | Q2 2025 | Suites smoke de Rust CLI + Python executam `/v2/norito-rpc` em CI, helper JS passa testes unitarios, harness mock Android exercita transportes duais. | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` e `javascript/iroha_js/test/noritoRpcClient.test.js` verdes em CI; harness Android conectado ao `./gradlew test`. |
| **P1 - Preview de SDK** | Q3 2025 | Bundle compartilhado de fixtures commitado, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` grava logs + JSON em `artifacts/norito_rpc/`, flags opcionais de transporte Norito expostos nos samples de SDK. | Manifeste de fixtures assinado, updates de README mostram uso opt-in, API de preview Swift disponivel atras do flag IOS2. |
| **P2 - Staging / preview AND4** | Q1 2026 | Pools Torii de staging preferem Norito, clientes AND4 preview Android e suites de paridade IOS2 Swift usam por padrao o transporte binario, dashboard de telemetria `dashboards/grafana/torii_norito_rpc_observability.json` populado. | `docs/source/torii/norito_rpc_stage_reports.md` captura o canary, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` passa, replay do harness mock Android captura casos de sucesso/erro. |
| **P3 - GA em producao** | Q4 2026 | Norito vira o transporte padrao para todos os SDKs; JSON fica como fallback de brownout. Jobs de release arquivam artefatos de paridade a cada tag. | Checklist de release agrega o output smoke Norito para Rust/JS/Python/Swift/Android; thresholds de alerta para SLOs de taxa de erro Norito vs JSON aplicados; `status.md` e notas de release citam evidencia GA. |

## Entregaveis de SDK e hooks de CI

- **Rust CLI e harness de integracao** - estender os testes smoke `iroha_cli pipeline` para forcar o transporte Norito quando `cargo xtask norito-rpc-verify` estiver disponivel. Proteger com `cargo test -p integration_tests -- norito_streaming` (lab) e `cargo xtask norito-rpc-verify` (staging/GA), guardando artefatos em `artifacts/norito_rpc/`.
- **SDK Python** - deixar o smoke de release (`python/iroha_python/scripts/release_smoke.sh`) por padrao em Norito RPC, manter `run_norito_rpc_smoke.sh` como entrada de CI e documentar a paridade em `python/iroha_python/README.md`. Alvo de CI: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **SDK JavaScript** - estabilizar `NoritoRpcClient`, deixar helpers de governance/query por padrao em Norito quando `toriiClientConfig.transport.preferred === "norito_rpc"`, e capturar samples end-to-end em `javascript/iroha_js/recipes/`. CI deve rodar `npm test` e o job dockerizado `npm run test:norito-rpc` antes de publicar; provenance envia logs smoke Norito para `javascript/iroha_js/artifacts/`.
- **SDK Swift** - ligar o transporte Norito bridge atras do flag IOS2, espelhar a cadencia de fixtures e garantir que a suite de paridade Connect/Norito rode nos lanes Buildkite referenciados em `docs/source/sdk/swift/index.md`.
- **SDK Android** - clientes AND4 preview e o harness mock Torii adotam Norito, com telemetria de retry/backoff documentada em `docs/source/sdk/android/networking.md`. O harness compartilha fixtures com outros SDKs via `scripts/run_norito_rpc_fixtures.sh --sdk android`.

## Evidencia e automacao

- `scripts/run_norito_rpc_fixtures.sh` envolve `cargo xtask norito-rpc-verify`, captura stdout/stderr e emite `fixtures.<sdk>.summary.json` para que os donos de SDK tenham um artefato determinista para anexar ao `status.md`. Use `--sdk <label>` e `--out artifacts/norito_rpc/<stamp>/` para manter os bundles de CI organizados.
- `cargo xtask norito-rpc-verify` impone paridade de hash de schema (`fixtures/norito_rpc/schema_hashes.json`) e falha se Torii retornar `X-Iroha-Error-Code: schema_mismatch`. Acompanhe cada falha com uma captura de fallback JSON para depuracao.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` e `dashboards/grafana/torii_norito_rpc_observability.json` definem os contratos de alerta para NRPC-2. Rode o script apos cada edicao do dashboard e guarde a saida do `promtool` no bundle canary.
- `docs/source/runbooks/torii_norito_rpc_canary.md` descreve os drills de staging e producao; atualize quando hashes de fixtures ou gates de alerta mudarem.

## Checklist para revisores

Antes de marcar um marco NRPC-4, confirme:

1. Os hashes do bundle de fixtures mais recente coincidem com `fixtures/norito_rpc/schema_hashes.json` e o artefato de CI correspondente fica registrado em `artifacts/norito_rpc/<stamp>/`.
2. Os README de SDK e as docs do portal descrevem como forcar o fallback JSON e citam o transporte Norito por padrao.
3. Os dashboards de telemetria mostram paineis de taxa de erro dual-stack com links de alerta, e o dry run do Alertmanager (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) esta anexado ao tracker.
4. O cronograma de adocao aqui coincide com a entrada do tracker (`docs/source/torii/norito_rpc_tracker.md`) e o roadmap (NRPC-4) referencia o mesmo bundle de evidencia.

Manter disciplina no cronograma deixa o comportamento cross-SDK previsivel e permite que a governanca audite a adocao do Norito-RPC sem solicitacoes sob medida.
