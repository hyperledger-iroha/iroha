---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pq-ratchet-runbook
título: Simulação PQ Ratchet SoraNet
sidebar_label: Catraca Runbook PQ
descrição: Etapas de ensaio de plantão para promover ou retroceder a política de anonimato PQ e validar a telemetria determinada.
---

:::nota Fonte canônica
Esta página reflete `docs/source/soranet/pq_ratchet_runbook.md`. Gardez les duas cópias alinhadas apenas com o retrato do antigo conjunto de documentação.
:::

## Objetivo

Este runbook guia a sequência de exercícios de simulação para a política de anonimato pós-quântico (PQ) do SoraNet. Os operadores repetem a promoção (Estágio A -> Estágio B -> Estágio C) e a retrogradação é controlada para o Estágio B/A quando a oferta PQ é baixada. A broca valida os ganchos de telemetria (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) e coleta os artefatos para o registro do ensaio do incidente.

## Pré-requisito

- Dernier binário `sorafs_orchestrator` com ponderação de capacidade (commit igual ou posterior à referência da broca em `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Acesse a pilha Prometheus/Grafana que é `dashboards/grafana/soranet_pq_ratchet.json`.
- Instantâneo nominal do diretório de guarda. Recupere e verifique uma cópia antes da broca:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Se o diretório de origem não for publicado como JSON, codifique-o novamente no binário Norito com `soranet-directory build` antes de executar os auxiliares de rotação.

- Capture os metadados e pré-instale os artefatos de rotação do emissor com a CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Fenetre de mudança aprovada pelas equipes de rede de plantão e observabilidade.

## Etapas de promoção

1. **Auditoria de etapa**

   Registre o estágio de partida:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Attendez `anon-guard-pq` promoção de vanguarda.

2. **Promoção versus Estágio B (Maioria PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Compareça >=5 minutos para que os manifestos sejam aprovados.
   - No Grafana (painel `SoraNet PQ Ratchet Drill`) confirme que o painel "Eventos de Política" aparece `outcome=met` para `stage=anon-majority-pq`.
   - Capture uma captura de tela ou o JSON do painel e anexe o registro do incidente.

3. **Promoção versão C (PQ estrito)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifique se os histogramas `sorafs_orchestrator_pq_ratio_*` tendem à versão 1.0.
   - Confirme que o compteur brownout reste plat; sinon suivez les etapes de retrogradation.

## Broca de retrogradação / brownout

1. **Induire une penurie PQ synthetique**

   Desative os relés PQ no ambiente de playground, seguindo o diretório de proteção em suas entradas clássicas e depois recarregue o orquestrador de cache:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observador da queda de energia da telemetria**

   - Dashboard: o painel "Brownout Rate" fica acima de 0.
   -PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` faça o repórter `anonymity_outcome="brownout"` com `anonymity_reason="missing_majority_pq"`.

3. **Retrógrado versão Estágio B / Estágio A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Se a oferta PQ estiver insuficiente, retrogradez ver `anon-guard-pq`. A broca termina quando os compteurs se estabilizam e as promoções podem ser reaplicadas.

4. **Diretório Restaurer le guard**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetria e artefatos- **Painel:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** certifique-se de que o alerta de brownout para `sorafs_orchestrator_policy_events_total` permaneça na configuração do SLO (&lt;5% em toda a janela de 10 minutos).
- **Registro de incidentes:** adiciona trechos de telemetria e notas operadas em `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Capturar signatário:** use `cargo xtask soranet-rollout-capture` para copiar o log de perfuração e o placar em `artifacts/soranet_pq_rollout/<timestamp>/`, calcule os resumos BLAKE3 e produza um sinal `rollout_capture.json`.

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

Adicione os metadados gerados e a assinatura do dossiê de governança.

## Reversão

Se a broca revelar uma verdadeira penúria PQ, descanse no Estágio A, notifique o Networking TL e anexe as métricas coletadas, assim como as diferenças do diretório de proteção no rastreador de incidentes. Use a exportação do guard directory capture plus para restaurar o serviço normalmente.

:::tip Cobertura de regressão
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fornece a validação sintética que é necessária para esta broca.
:::