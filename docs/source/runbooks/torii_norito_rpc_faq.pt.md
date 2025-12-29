---
lang: pt
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969f24fd53af5e5714eb4b257dc086b3990550a5085c9464805f689c7d8ee324
source_last_modified: "2025-12-14T09:53:36.247266+00:00"
translation_last_reviewed: 2025-12-28
---

# FAQ operacional Norito-RPC

Esta FAQ destila os knobs de rollout/rollback, telemetria e artefatos de
 evidência referenciados nos itens **NRPC-2** e **NRPC-4** para que operadores
 tenham uma única página durante canaries, brownouts ou drills de incidente.
 Trate como porta de entrada dos handoffs de plantão; os procedimentos detalhados
 continuam em `docs/source/torii/norito_rpc_rollout_plan.md` e
`docs/source/runbooks/torii_norito_rpc_canary.md`.

## 1. Knobs de configuração

| Caminho | Propósito | Valores permitidos / notas |
|------|---------|------------------------|
| `torii.transport.norito_rpc.enabled` | Chave liga/desliga rígida do transporte Norito. | `true` mantém os handlers HTTP registrados; `false` os desativa independentemente do stage. |
| `torii.transport.norito_rpc.require_mtls` | Exigir TLS mútuo para endpoints Norito. | Padrão `true`. Desative apenas em pools de staging isolados. |
| `torii.transport.norito_rpc.allowed_clients` | Allowlist de contas de serviço / tokens API autorizados. | Forneça CIDRs, hashes de token ou OIDC client IDs conforme o deployment. |
| `torii.transport.norito_rpc.stage` | Stage de rollout anunciado aos SDKs. | `disabled` (rejeita Norito, força JSON), `canary` (somente allowlist, telemetria reforçada), `ga` (padrão para cada cliente autenticado). |
| `torii.preauth_scheme_limits.norito_rpc` | Concorrência por esquema + orçamento de burst. | Espelhe as chaves usadas nos throttles HTTP/WS (ex.: `max_in_flight`, `rate_per_sec`). Aumentar o cap sem atualizar Alertmanager derrota o guardrail. |
| `transport.norito_rpc.*` em `docs/source/config/client_api.md` | Overrides voltados ao cliente (CLI / SDK discovery). | Use `cargo xtask client-api-config diff` para inspecionar mudanças pendentes antes de enviá‑las ao Torii. |

**Fluxo de brownout recomendado**

1. Defina `torii.transport.norito_rpc.stage=disabled`.
2. Mantenha `enabled=true` para que probes/testes de alerta continuem exercitando os handlers.
3. Troque `torii.preauth_scheme_limits.norito_rpc.max_in_flight` para zero se
   precisar parar imediatamente (por exemplo, enquanto aguarda propagação de config).
4. Atualize o log do operador e anexe o digest da nova config ao relatório de stage.

## 2. Checklists operacionais

- **Canary / staging** — siga `docs/source/runbooks/torii_norito_rpc_canary.md`.
  Esse runbook referencia as mesmas chaves de config e lista os artefatos de
  evidência capturados por `scripts/run_norito_rpc_smoke.sh` +
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh`.
- **Promoção para produção** — execute o template de stage report em
  `docs/source/torii/norito_rpc_stage_reports.md`. Registre o hash da config,
  o hash da allowlist, o digest do bundle de smoke, o hash do export do Grafana
  e o identificador do drill de alertas.
- **Rollback** — volte `stage` para `disabled`, mantenha a allowlist e documente
  a troca no stage report + log do incidente. Quando a causa raiz for corrigida,
  reexecute o checklist canary antes de setar `stage=ga`.

## 3. Telemetria e alertas

| Ativo | Localização | Notas |
|-------|----------|-------|
| Dashboard | `dashboards/grafana/torii_norito_rpc_observability.json` | Acompanha taxa de requisições, códigos de erro, tamanhos de payload, falhas de decodificação e % de adoção. |
| Alertas | `dashboards/alerts/torii_norito_rpc_rules.yml` | Gates `NoritoRpcErrorBudget`, `NoritoRpcDecodeFailures` e `NoritoRpcFallbackSpike`. |
| Script de caos | `scripts/telemetry/test_torii_norito_rpc_alerts.sh` | Falha a CI quando as expressões de alerta divergem. Execute após cada mudança de config. |
| Smoke tests | `python/iroha_python/scripts/run_norito_rpc_smoke.sh`, `cargo xtask norito-rpc-verify` | Inclua logs no bundle de evidência para cada promoção. |

Dashboards devem ser exportados e anexados ao ticket de release (`make
docs-portal-dashboards` na CI) para que on‑call possa reproduzir métricas sem
acesso ao Grafana de produção.

## 4. Perguntas frequentes

**Como permitir um novo SDK durante canary?**  
Adicione a conta de serviço/token em `torii.transport.norito_rpc.allowed_clients`,
recarregue o Torii e registre a mudança em `docs/source/torii/norito_rpc_tracker.md`
sob NRPC-2R. O owner do SDK também deve capturar uma execução de fixtures via
`scripts/run_norito_rpc_fixtures.sh --sdk <label>`.

**O que acontece se a decodificação Norito falhar no meio do rollout?**  
Deixe `stage=canary`, mantenha `enabled=true` e faça triagem via
`torii_norito_decode_failures_total`. Os owners do SDK podem voltar a JSON
omitindo `Accept: application/x-norito`; o Torii continuará servindo JSON até
que o stage volte para `ga`.

**Como provar que o gateway está servindo o manifesto correto?**  
Execute `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario norito-rpc --host
<gateway-host>` para que a sonda registre os headers `Sora-Proof` junto com o
config digest Norito. Anexe a saída JSON ao stage report.

**Onde devo registrar overrides de redaction?**  
Documente cada override temporário na coluna `Notes` do stage report e registre
 o patch de config Norito no controle de mudanças. Overrides expiram
automaticamente no arquivo de config; esta FAQ garante que o plantão lembre de
limpar após incidentes.

Para qualquer dúvida não coberta aqui, escale pelos canais listados no runbook
canary (`docs/source/runbooks/torii_norito_rpc_canary.md`).

## 5. Trecho de release note (follow‑up OPS-NRPC)

O item **OPS-NRPC** exige uma nota de release pronta para que operadores anunciem
o rollout Norito‑RPC de forma consistente. Copie o bloco abaixo no próximo post
de release (substitua os campos entre colchetes) e anexe o bundle de evidência
descrito abaixo.

> **Transporte Torii Norito-RPC** — Envelopes Norito agora são servidos junto com
> a API JSON. O flag `torii.transport.norito_rpc.stage` é entregue configurado em
> **[stage: disabled/canary/ga]** e segue o checklist de rollout por etapas em
> `docs/source/torii/norito_rpc_rollout_plan.md`. Operadores podem optar por sair
> temporariamente definindo `torii.transport.norito_rpc.stage=disabled` enquanto
> mantêm `torii.transport.norito_rpc.enabled=true`; os SDKs voltam para JSON
> automaticamente. Dashboards de telemetria
> (`dashboards/grafana/torii_norito_rpc_observability.json`) e drills de alerta
> (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) permanecem obrigatórios
> antes de elevar o stage, e os artefatos canary/smoke capturados por
> `python/iroha_python/scripts/run_norito_rpc_smoke.sh` devem ser anexados ao
> ticket de release.

Antes de publicar:

1. Substitua o marcador **[stage: …]** pelo stage anunciado no Torii.
2. Vincule o ticket de release ao stage report mais recente em
   `docs/source/torii/norito_rpc_stage_reports.md`.
3. Envie os exports Grafana/Alertmanager acima junto com os hashes do bundle de
   smoke de `scripts/run_norito_rpc_smoke.sh`.

Este trecho mantém o requisito OPS-NRPC de release note atendido sem forçar
commanders de incidente a reformular o status do rollout toda vez.
