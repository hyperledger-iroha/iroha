---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ratchet-runbook
título: Simulacro de PQ Ratchet de SoraNet
sidebar_label: Runbook do PQ Ratchet
descrição: Passos de ensaio para proteger a promoção ou degradação da política de anonimato PQ escalonada com validação de telemetria determinista.
---

:::nota Fonte canônica
Esta página reflete `docs/source/soranet/pq_ratchet_runbook.md`. Mantenha ambas as cópias sincronizadas.
:::

## Propósito

Este runbook guia a sequência do simulacro para a política de anonimato pós-quântica (PQ) escalonada do SoraNet. Os operadores pensam tanto na promoção (Estágio A -> Estágio B -> Estágio C) quanto na degradação controlada de retorno ao Estágio B/A quando ocorre o fornecimento PQ. O simulacro valida os ganchos de telemetria (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) e coleta artefatos para o registro de ensaio de incidentes.

## Pré-requisitos

- Último binário `sorafs_orchestrator` com ponderação de capacidade (comprometido após a referência do simulacro mostrado em `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Acesse a pilha de Prometheus/Grafana que serve `dashboards/grafana/soranet_pq_ratchet.json`.
- Instantâneo nominal do diretório del guard. Trae e verifique uma cópia antes do simulacro:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Se o diretório de origem for apenas publicado em JSON, codifique-o novamente para o binário Norito com `soranet-directory build` antes de executar os auxiliares de rotação.

- Captura de metadados e pré-estágio de artefatos de rotação do emissor com CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Ventana de mudança aprovada pelos equipamentos de plantão de networking e observabilidade.

## Passos de promoção

1. **Auditoria de etapa**

   Registre a etapa inicial:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espera `anon-guard-pq` antes de promocionar.

2. **Promociona a Fase B (Maioria PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Espera >=5 minutos para que os manifestos refresquem.
   - Em Grafana (painel `SoraNet PQ Ratchet Drill`) confirma que o painel "Eventos de Política" mostra `outcome=met` para `stage=anon-majority-pq`.
   - Capture uma captura de tela do painel JSON e adicione-a ao registro de incidentes.

3. **Promociona o Estágio C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifique se os histogramas `sorafs_orchestrator_pq_ratio_*` estão em 1.0.
   - Confirme que o contador de queda de energia permanece plano; se não, siga os passos de degradação.

## Simulacro de degradação / brownout

1. **Induza uma fuga sintética de PQ**

   Deshabilita relés PQ no entorno do playground gravando o diretório de guarda para entradas clássicas apenas, depois recarregando o cache do orquestrador:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observe a telemetria de queda de energia**

   - Dashboard: o painel "Brownout Rate" está acima de 0.
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` deve relatar `anonymity_outcome="brownout"` com `anonymity_reason="missing_majority_pq"`.

3. **Degradação a Estágio B / Estágio A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Se o fornecimento PQ for insuficiente, degradará a `anon-guard-pq`. O simulacro termina quando os contadores de queda de energia se estabilizam e as promoções podem ser reaplicadas.

4. **Diretório Restaura el guard**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetria e artefatos- **Painel:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** certifique-se de que o alerta de brownout de `sorafs_orchestrator_policy_events_total` seja mantido por baixo do SLO configurado (&lt;5% em qualquer janela de 10 minutos).
- **Registro de incidentes:** adiciona trechos de telemetria e notas do operador a `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Captura firmada:** usa `cargo xtask soranet-rollout-capture` para copiar o log de perfuração e o placar em `artifacts/soranet_pq_rollout/<timestamp>/`, calcular digests BLAKE3 e produzir um `rollout_capture.json` firmado.

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

Junta-se aos metadados gerados e à firma do pacote de governança.

## Reversão

Se o simulacro descobrir uma fuga real de PQ, permanece no Estágio A, notifica o Networking TL e adiciona as métricas coletadas junto com as diferenças do diretório de guarda ao rastreador de incidentes. Use a exportação do diretório de proteção capturado anteriormente para restaurar o serviço normal.

:::tip Cobertura de regressão
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fornece a validação sintética que respalda este simulacro.
:::