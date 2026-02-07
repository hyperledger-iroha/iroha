---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ratchet-runbook
título: Simulacro PQ Ratchet do SoraNet
sidebar_label: Runbook do PQ Ratchet
descrição: Passos de ensaio on-call para promover ou rebaixar a política de anonimato PQ em estágios com validação determinística de telemetria.
---

:::nota Fonte canônica
Esta página espelha `docs/source/soranet/pq_ratchet_runbook.md`. Mantenha ambas as cópias sincronizadas.
:::

## Propósito

Este runbook guia a sequência do simulacro para a política de anonimato pós-quântico (PQ) em estágios do SoraNet. Os operadores ensaiam promoção (Estágio A -> Estágio B -> Estágio C) e a despromoção controlada de volta ao Estágio B/A quando a oferta de PQ cai. O simulador válido de ganchos de telemetria (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) e coleta de artistas para o registro de ensaio de incidentes.

## Pré-requisitos

- Último binário `sorafs_orchestrator` com capacidade-ponderação (commit igual ou posterior à referência do exercício mostrado em `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Acesso à pilha Prometheus/Grafana que atende `dashboards/grafana/soranet_pq_ratchet.json`.
- Snapshot nominal do diretório de guarda. Busque e valide uma cópia antes do simulacro:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Se o diretório de origem publicar apenas JSON, reencode para Norito binário com `soranet-directory build` antes de rodar os ajudantes de rotação.

- Capturar metadados e pré-estágio de artistas de rotação do emissor com o CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Janela de mudança aprovada pelos tempos de plantão de networking e observabilidade.

## Passos de promoção

1. **Auditoria de etapa**

   Registre o estágio inicial:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espere `anon-guard-pq` antes da promoção.

2. **Promoção para Estágio B (Maioria PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Aguarde >=5 minutos para manifestos serem atualizados.
   - No Grafana (dashboard `SoraNet PQ Ratchet Drill`) confirme que o painel "Policy Events" mostra `outcome=met` para `stage=anon-majority-pq`.
   - Capture um screenshot ou JSON do painel e anexe ao log de incidentes.

3. **Promoção para Estágio C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifique se os histogramas `sorafs_orchestrator_pq_ratio_*` tendem a 1.0.
   - Confirme que o contador de queda de energia permanece plano; caso contrário, siga os passos de despromoção.

## Despromocao / broca de brownout

1. **Induza uma escassez sintética de PQ**

   Desative relés PQ no ambiente playground aumentando o guarda diretório a entradas clássicas apenas, depois recarregue o cache do orquestrador:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observar uma telemetria de queda de energia**

   - Dashboard: o painel "Brownout Rate" está acima de 0.
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` deve reportar `anonymity_outcome="brownout"` com `anonymity_reason="missing_majority_pq"`.

3. **Despromova para Estágio B / Estágio A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Se a oferta de PQ ainda for insuficiente, despromova para `anon-guard-pq`. O simulacro termina quando os contadores de brownout se estabilizam e as promoções podem ser reaplicadas.

4. **Restaurar o diretório de guarda**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetria e artistas- **Painel:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** garanta que o alerta de brownout de `sorafs_orchestrator_policy_events_total` fique abaixo do SLO configurado (&lt;5% em qualquer janela de 10 minutos).
- **Registro de incidentes:** trechos de anexo de telemetria e notas do operador em `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Captura assinada:** use `cargo xtask soranet-rollout-capture` para copiar o log de perfuração e o scoreboard para `artifacts/soranet_pq_rollout/<timestamp>/`, calcular digests BLAKE3 e produzir um `rollout_capture.json` contratado.

Exemplo:

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

Anexo dos metadados gerados e assinatura do pacote de governança.

## Reversão

Se o simulacro revelar deficiência real de PQ, permaneça no Estágio A, notifique o Networking TL e anexe as métricas coletadas junto com os diffs do guard directory ao incident tracker. Use o export do guard directory capturado anteriormente para restaurar o serviço normal.

:::tip Cobertura de regressão
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fornece uma validação sintética que sustenta este simulacro.
:::