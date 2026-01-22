---
lang: pt
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1add138352dabf2433c36c9abac24085af8c7e53dca8bc579d73b37680e470cf
source_last_modified: "2025-11-22T04:59:33.125102+00:00"
translation_last_reviewed: 2026-01-01
---

# Plano de rollback de incentivos de relay

Use este playbook para desabilitar pagamentos automaticos de relay se governance solicitar um
halt ou se os guardrails de telemetria dispararem.

1. **Congelar automacao.** Pare o daemon de incentivos em cada host de orchestrator
   (`systemctl stop soranet-incentives.service` ou o equivalente em container) e confirme que o processo nao esta mais rodando.
2. **Drenar instrucoes pendentes.** Execute
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   para garantir que nao ha instrucoes de payout pendentes. Arquive os payloads Norito resultantes para auditoria.
3. **Revogar aprovacao de governance.** Edite `reward_config.json`, defina
   `"budget_approval_id": null` e redeploy a configuracao com
   `iroha app sorafs incentives service init` (ou `update-config` se o daemon for de longa duracao). O payout engine agora falha fechado com
   `MissingBudgetApprovalId`, portanto o daemon recusa cunhar payouts ate que um novo hash de aprovacao seja restaurado. Registre o commit git e o SHA-256 da config modificada no log do incidente.
4. **Notificar o Parlamento Sora.** Anexe o ledger de payouts drenado, o relatorio shadow-run e um resumo curto do incidente. As atas do Parlamento devem registrar o hash da configuracao revogada e o horario em que o daemon foi parado.
5. **Validacao do rollback.** Mantenha o daemon desabilitado ate que:
   - alertas de telemetria (`soranet_incentives_rules.yml`) estejam verdes por >=24 h,
   - o relatorio de reconciliacao da tesouraria mostre zero transferencias faltantes, e
   - o Parlamento aprove um novo hash de budget.

Assim que governance reemitir um hash de aprovacao de budget, atualize `reward_config.json`
com o novo digest, rode novamente o comando `shadow-run` na telemetria mais recente,
e reinicie o daemon de incentivos.
