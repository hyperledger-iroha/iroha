---
lang: pt
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_canary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44aa7b618a377a4a62f3be210ce6b9ccc6906b7791c4bbf6d3f60899fe08e7b8
source_last_modified: "2025-12-14T09:53:36.245431+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de canary Torii Norito-RPC (NRPC-2C)

Este runbook operacionaliza o plano de rollout **NRPC-2** descrevendo como
promover o transporte Norito‑RPC da validação em lab de staging para o stage
“canary” em produção. Deve ser lido junto com:

- [`docs/source/torii/nrpc_spec.md`](../torii/nrpc_spec.md) (contrato do protocolo)
- [`docs/source/torii/norito_rpc_rollout_plan.md`](../torii/norito_rpc_rollout_plan.md)
- [`docs/source/torii/norito_rpc_telemetry.md`](../torii/norito_rpc_telemetry.md)

## Papéis e entradas

| Papel | Responsabilidade |
|------|----------------|
| Torii Platform TL | Aprova deltas de config, assina os smoke tests. |
| NetOps | Aplica mudanças de ingress/envoy e monitora a saúde do pool canary. |
| Liaison de observabilidade | Verifica dashboards/alertas e captura evidências. |
| Platform Ops | Conduz o ticket de mudança, coordena rehearsal de rollback, atualiza trackers. |

Artefatos necessários:

- Patch Norito mais recente de `iroha_config` com `transport.norito_rpc.stage = "canary"` e
  `transport.norito_rpc.allowed_clients` preenchido.
- Snippet de config Envoy/Nginx que preserve `Content-Type: application/x-norito` e
  imponha o perfil mTLS dos clientes canary (`defaults/torii_ingress_mtls.yaml`).
- Allowlist de tokens (YAML ou manifesto Norito) para clientes canary.
- URL de Grafana + token API para `dashboards/grafana/torii_norito_rpc_observability.json`.
- Acesso ao harness de smoke de paridade
  (`python/iroha_python/scripts/run_norito_rpc_smoke.sh`) e ao script de drill de alertas
  (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`).

## Checklist pré‑voo

1. **Spec freeze confirmado.** Garanta que o hash de `docs/source/torii/nrpc_spec.md`
   corresponde ao último release assinado e que não há PRs pendentes tocando o header/layout Norito.
2. **Validação de config.** Execute
   ```bash
   cargo xtask validate-config --config <patch.json> --schema client_api
   ```
   para confirmar que as novas entradas `transport.norito_rpc.*` são parseadas.
3. **Caps de esquema.** Configure `torii.preauth_scheme_limits.norito_rpc` de forma
   conservadora (ex.: 25 conexões concorrentes) para que chamadas binárias não
   esgotem o tráfego JSON.
4. **Rehearsal de ingress.** Aplique o patch de Envoy em staging, rode o teste negativo
   (`cargo test -p iroha_torii -- norito_ingress`) e confirme que headers removidos
   são rejeitados com HTTP 415.
5. **Sanidade de telemetria.** Em staging, execute `scripts/telemetry/test_torii_norito_rpc_alerts.sh
   --env staging --dry-run` e anexe o bundle de evidência gerado.
6. **Inventário de tokens.** Verifique que a allowlist canary inclui pelo menos
   dois operadores por região; armazene o manifesto em `artifacts/norito_rpc/<YYYYMMDD>/allowlist.json`.
7. **Ticketing.** Abra o ticket de mudança com janela start/end, plano de rollback e
   links para este runbook mais a evidência de telemetria.

## Procedimento de promoção canary

1. **Aplicar patch de config.**
   - Faça rollout do delta `iroha_config` (stage=`canary`, allowlist preenchida,
     limites de esquema configurados) via admissão.
   - Reinicie ou hot‑reload o Torii, confirmando que o patch foi reconhecido via
     logs `torii.config.reload`.
2. **Atualizar ingress.**
   - Implante a config Envoy/Nginx habilitando roteamento do header Norito/perfil mTLS
     para o pool canary.
   - Verifique que respostas `curl -vk --cert <client.pem>` incluem os headers
     Norito `X-Iroha-Error-Code` quando esperado.
3. **Smoke tests.**
   - Execute `python/iroha_python/scripts/run_norito_rpc_smoke.sh --profile canary`
     a partir do bastion canary. Capture transcrições JSON + Norito e guarde em
     `artifacts/norito_rpc/<YYYYMMDD>/smoke/`.
   - Registre hashes em `docs/source/torii/norito_rpc_stage_reports.md`.
4. **Observar telemetria.**
   - Observe `torii_active_connections_total{scheme="norito_rpc"}` e
     `torii_request_duration_seconds_bucket{scheme="norito_rpc"}` por pelo menos 30 minutos.
   - Exporte o dashboard do Grafana via API e anexe ao ticket de mudança.
5. **Rehearsal de alertas.**
   - Execute `scripts/telemetry/test_torii_norito_rpc_alerts.sh --env canary` para
     injetar envelopes Norito malformados; garanta que o Alertmanager registre o
     incidente sintético e limpe automaticamente.
6. **Captura de evidência.**
   - Atualize `docs/source/torii/norito_rpc_stage_reports.md` com:
     - Digest de config
     - Hash do manifesto de allowlist
     - Timestamp do smoke test
     - Checksum do export do Grafana
     - ID do drill de alertas
   - Envie artefatos para `artifacts/norito_rpc/<YYYYMMDD>/`.

## Monitoramento e critérios de saída

Permaneça em canary até que todas as condições abaixo sejam verdadeiras por ≥72 horas:

- Taxa de erro (`torii_request_failures_total{scheme="norito_rpc"}`) ≤1 % e sem picos
  sustentados em `torii_norito_decode_failures_total`.
- Paridade de latência (`p95` Norito vs JSON) dentro de 10 %.
- Dashboard de alertas silencioso exceto drills programados.
- Operadores na allowlist enviam relatórios de paridade sem mismatch de esquema.

Documente o status diário no ticket de mudança e capture snapshots em
`docs/source/status/norito_rpc_canary_log.md` (se existir).

## Procedimento de rollback

1. Volte `transport.norito_rpc.stage` para `"disabled"` e limpe `allowed_clients`;
   aplique via admissão.
2. Remova o route/mTLS stanza de Envoy/Nginx, recarregue proxies e confirme que
   novas conexões Norito são recusadas.
3. Revogue tokens canary (ou desative credenciais bearer) para derrubar sessões ativas.
4. Acompanhe `torii_active_connections_total{scheme="norito_rpc"}` até chegar a zero.
5. Reexecute o harness de smoke apenas‑JSON para assegurar a funcionalidade base.
6. Crie um stub de post‑mortem em `docs/source/postmortems/norito_rpc_rollback.md`
   dentro de 24 horas e atualize o ticket de mudança com resumo de impacto + métricas.

## Pós‑canary

Quando os critérios de saída forem satisfeitos:

1. Atualize `docs/source/torii/norito_rpc_stage_reports.md` com a recomendação de GA.
2. Adicione uma entrada em `status.md` resumindo resultados do canary e bundles de evidência.
3. Notifique os leads de SDK para que mudem fixtures de staging para Norito nas paridades.
4. Prepare o patch de config GA (stage=`ga`, removendo allowlist) e agende a
   promoção conforme o plano NRPC-2.

Seguir este runbook garante que cada promoção canary colete a mesma evidência,
mantenha rollback determinístico e satisfaça os critérios de aceitação NRPC-2.
