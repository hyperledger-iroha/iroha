---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/soranet/pq-ratchet-runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2382eafbfebb7102edeb035ab8d3f897cee6fb049a9bf42d3baf8abf1d83958
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
id: pq-ratchet-runbook
lang: pt
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canonica
Esta pagina espelha `docs/source/soranet/pq_ratchet_runbook.md`. Mantenha ambas as copias sincronizadas.
:::

## Proposito

Este runbook guia a sequencia do simulacro para a politica de anonimato post-quantum (PQ) em estagios do SoraNet. Operadores ensaiam promocao (Stage A -> Stage B -> Stage C) e a despromocao controlada de volta a Stage B/A quando a oferta de PQ cai. O simulacro valida hooks de telemetria (`sorafs_orchestrator_policy_events_total`, `sorafs_orchestrator_brownouts_total`, `sorafs_orchestrator_pq_ratio_*`) e coleta artefatos para o log de rehearsal de incidentes.

## Prerequisitos

- Ultimo binario `sorafs_orchestrator` com capability-weighting (commit igual ou posterior ao reference do drill mostrado em `docs/source/soranet/reports/pq_ratchet_validation.md`).
- Acesso ao stack Prometheus/Grafana que serve `dashboards/grafana/soranet_pq_ratchet.json`.
- Snapshot nominal do guard directory. Busque e valide uma copia antes do simulacro:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

Se o source directory publicar apenas JSON, re-encode para Norito binary com `soranet-directory build` antes de rodar os helpers de rotacao.

- Capture metadata e pre-stage artefatos de rotacao do issuer com o CLI:

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- Janela de mudanca aprovada pelos times on-call de networking e observability.

## Passos de promocao

1. **Stage audit**

   Registre o stage inicial:

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   Espere `anon-guard-pq` antes da promocao.

2. **Promova para Stage B (Majority PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - Aguarde >=5 minutos para manifests serem atualizados.
   - No Grafana (dashboard `SoraNet PQ Ratchet Drill`) confirme que o painel "Policy Events" mostra `outcome=met` para `stage=anon-majority-pq`.
   - Capture um screenshot ou JSON do painel e anexe ao log de incidentes.

3. **Promova para Stage C (Strict PQ)**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - Verifique que os histogramas `sorafs_orchestrator_pq_ratio_*` tendem a 1.0.
   - Confirme que o contador de brownout permanece plano; caso contrario, siga os passos de despromocao.

## Despromocao / brownout drill

1. **Induza uma escassez sintetica de PQ**

   Desative relays PQ no ambiente playground reduzindo o guard directory a entradas classicas apenas, depois recarregue o cache do orchestrator:

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **Observe a telemetria de brownout**

   - Dashboard: o painel "Brownout Rate" sobe acima de 0.
   - PromQL: `sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` deve reportar `anonymity_outcome="brownout"` com `anonymity_reason="missing_majority_pq"`.

3. **Despromova para Stage B / Stage A**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   Se a oferta de PQ ainda for insuficiente, despromova para `anon-guard-pq`. O simulacro termina quando os contadores de brownout estabilizam e as promocoes podem ser reaplicadas.

4. **Restaure o guard directory**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## Telemetria e artefatos

- **Dashboard:** `dashboards/grafana/soranet_pq_ratchet.json`
- **Alertas Prometheus:** garanta que o alerta de brownout de `sorafs_orchestrator_policy_events_total` fique abaixo do SLO configurado (&lt;5% em qualquer janela de 10 minutos).
- **Incident log:** anexe trechos de telemetria e notas do operador em `docs/examples/soranet_pq_ratchet_fire_drill.log`.
- **Captura assinada:** use `cargo xtask soranet-rollout-capture` para copiar o drill log e o scoreboard para `artifacts/soranet_pq_rollout/<timestamp>/`, calcular digests BLAKE3 e produzir um `rollout_capture.json` assinado.

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

Anexe os metadata gerados e a assinatura ao pacote de governance.

## Rollback

Se o simulacro revelar escassez real de PQ, permanece em Stage A, notifique o Networking TL e anexe as metricas coletadas junto com os diffs do guard directory ao incident tracker. Use o export do guard directory capturado anteriormente para restaurar o servico normal.

:::tip Cobertura de regressao
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` fornece a validacao sintetica que sustenta este simulacro.
:::
