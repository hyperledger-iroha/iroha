---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: pt
direction: ltr
source: docs/source/runbooks/nexus_lane_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1d9fbcf5301bd36a0bdc8a010ae214f7b1561373237054ab3c8a45a411cf6c0e
source_last_modified: "2025-12-14T09:53:36.241505+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de finalidade de lanes Nexus e oráculo

**Status:** Ativo — atende ao entregável de dashboard/runbook NX-18.  
**Público:** Core Consensus WG, SRE/Telemetry, Release Engineering, líderes de plantão.  
**Escopo:** Cobre SLOs de duração de slot, quórum DA, oráculo e buffer de
liquidação que governam a promessa de finalização em 1 s. Use junto com
`dashboards/grafana/nexus_lanes.json` e os helpers de telemetria em
`scripts/telemetry/`.

## Dashboards

- **Grafana (`dashboards/grafana/nexus_lanes.json`)** — publica o board “Nexus Lane Finality & Oracles”. Os painéis acompanham:
  - `histogram_quantile()` sobre `iroha_slot_duration_ms` (p50/p95/p99) e o gauge da amostra mais recente.
  - `iroha_da_quorum_ratio` e `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` para destacar churn de DA.
  - Superfícies do oráculo: `iroha_oracle_price_local_per_xor`, `iroha_oracle_staleness_seconds`, `iroha_oracle_twap_window_seconds` e `iroha_oracle_haircut_basis_points`.
  - Painel de buffer de liquidação (`iroha_settlement_buffer_xor`) mostrando débitos por lane em tempo real a partir de recibos `LaneBlockCommitment`.
- **Regras de alerta** — reutilizam as cláusulas Slot/DA SLO de `ans3.md`. Pager quando:
  - p95 de duração de slot > 1000 ms por duas janelas consecutivas de 5 m,
  - quórum DA < 0,95 ou `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m]) > 0`,
  - staleness do oráculo > 90 s ou janela TWAP ≠ 60 s configurados,
  - buffer de liquidação < 25 % (soft) / 10 % (hard) quando a métrica estiver ativa.

## Guia rápido de métricas

| Métrica | Alvo / Alerta | Notas |
|--------|----------------|-------|
| `histogram_quantile(0.95, iroha_slot_duration_ms)` | ≤ 1000 ms (hard), 950 ms warning | Use o painel do dashboard ou rode `scripts/telemetry/check_slot_duration.py` (`--json-out artifacts/nx18/slot_summary.json`) contra o export do Prometheus coletado durante caos. |
| `iroha_slot_duration_ms_latest` | Espelha o slot mais recente; investigue se > 1100 ms mesmo quando os quantis parecem OK. | Exporte o valor ao abrir incidentes. |
| `iroha_da_quorum_ratio` | ≥ 0,95 em janela móvel de 30 m. | Derivado de reschedules DA durante commits de bloco. |
| `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` | Deve permanecer 0 fora de rehearsals de caos. | Trate qualquer aumento sustentado como `missing-availability warning`. |

Cada reschedule também dispara um warning no pipeline Torii com `kind = "missing-availability warning"`. Capture esses eventos junto com o pico de métricas para identificar o cabeçalho de bloco afetado, a tentativa de retry e os contadores de requeue sem vasculhar logs de validadores.【crates/iroha_core/src/sumeragi/main_loop.rs:5164】
| `iroha_oracle_staleness_seconds` | ≤ 60 s. Alertar a 75 s. | Indica feeds TWAP de 60 s stale. |
| `iroha_oracle_twap_window_seconds` | Exatamente 60 s ± tolerância de 5 s. | Divergência indica oráculo mal configurado. |
| `iroha_oracle_haircut_basis_points` | Igual ao tier de liquidez do lane (0/25/75 bps). | Escale se haircuts subirem inesperadamente. |
| `iroha_settlement_buffer_xor` | Soft 25 %, hard 10 %. Forçar modo XOR‑only abaixo de 10 %. | O painel expõe débitos micro‑XOR por lane/dataspace; exporte antes de ajustar a política do router. |

## Playbook de resposta

### Violação de duração de slot
1. Confirme via dashboard + `promql` (p95/p99).  
2. Capture a saída de `scripts/telemetry/check_slot_duration.py --json-out <path>`
   (e o snapshot de métricas) para que revisores CXO verifiquem o gate de 1 s.  
3. Inspecione entradas de RCA: profundidade da fila de mempool, reschedules DA, traces IVM.  
4. Abra incidente, anexe screenshot do Grafana e agende drill de caos se a regressão persistir.

### Degradação de quórum DA
1. Verifique `iroha_da_quorum_ratio` e o contador de reschedule; correlacione com logs `missing-availability warning`.  
2. Se o ratio <0,95, fixe attesters falhando, amplie parâmetros de amostragem ou mude o perfil para modo XOR‑only.  
3. Rode `scripts/telemetry/check_nexus_audit_outcome.py` durante rehearsals de routed‑trace para provar que eventos `nexus.audit.outcome` seguem passando após mitigação.  
4. Arquive bundles de recibos DA com o ticket do incidente.

### Staleness de oráculo / deriva de haircut
1. Use os painéis 5–8 para verificar preço, staleness, janela TWAP e haircut.  
2. Para staleness >90 s: reinicie ou faça failover do feed do oráculo e reexecute o harness de caos.  
3. Para mismatch de haircut: inspecione o perfil de liquidez e mudanças recentes de governança; avise a tesouraria se linhas de swap precisarem de intervenção.

### Alertas de buffer de liquidação
1. Use `iroha_settlement_buffer_xor` (e recibos noturnos) para confirmar headroom antes de ajustar a política do router.  
2. Quando a métrica cruzar um limiar, execute:
   - **Soft breach (<25 %)**: acione a tesouraria, considere usar linhas de swap e registre o alerta.  
   - **Hard breach (<10 %)**: force inclusão XOR‑only, recuse lanes subsidiados e documente em `ops/drill-log.md`.  
3. Consulte `docs/source/settlement_router.md` para alavancas repo/reverse‑repo.

## Evidência e automação

- **CI** — conecte `scripts/telemetry/check_slot_duration.py --json-out artifacts/nx18/slot_summary.json` e `scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` ao workflow de aceitação RC para que cada release candidate entregue o resumo de duração de slot e os resultados de gate DA/oráculo/buffer junto com o snapshot de métricas citado acima. O helper já é invocado por `ci/check_nexus_lane_smoke.sh`.  
- **Paridade de dashboard** — rode `scripts/telemetry/compare_dashboards.py dashboards/grafana/nexus_lanes.json <prod-export.json>` para garantir que o board publicado coincide com exports de staging/prod.  
- **Artefatos de trace** — durante rehearsals TRACE ou drills de caos NX-18, invoque `scripts/telemetry/check_nexus_audit_outcome.py` para arquivar o payload `nexus.audit.outcome` mais recente (`docs/examples/nexus_audit_outcomes/`). Anexe o arquivo e screenshots do Grafana ao drill log.
- **Bundle de evidência de slot** — após gerar o JSON de resumo, rode `scripts/telemetry/bundle_slot_artifacts.py --metrics <prometheus.tgz-extract>/metrics.prom --summary artifacts/nx18/slot_summary.json --out-dir artifacts/nx18` para que o `slot_bundle_manifest.json` resultante capture digests SHA‑256 dos dois artefatos. Suba o diretório como está com o bundle de evidência RC. O pipeline de release executa isso automaticamente (opção `--skip-nexus-lane-smoke`) e copia `artifacts/nx18/` para a saída de release.

## Checklist de manutenção

- Mantenha `dashboards/grafana/nexus_lanes.json` sincronizado com exports do Grafana após cada mudança de esquema; documente edições nos commits referenciando NX-18.  
- Atualize este runbook quando novas métricas (ex.: gauges de buffer de liquidação) ou limiares de alerta chegarem.  
- Registre cada rehearsal de caos (latência de slot, jitter DA, stall de oráculo, esgotamento de buffer) com `scripts/telemetry/log_sorafs_drill.sh --log ops/drill-log.md --program NX-18 --status <status>`.

Seguir este runbook fornece a evidência “dashboards/runbooks de operador” exigida por NX-18 e garante que o SLO de finalização continue aplicável antes do Nexus GA.
