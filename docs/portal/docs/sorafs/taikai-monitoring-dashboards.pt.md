---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/taikai-monitoring-dashboards.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6049b1c4fb42bbfbeaa7fa8f3549c5b7beac1a3e8baec45c0c0ce52f0c3baa2e
source_last_modified: "2025-11-14T09:52:13.533271+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Dashboards de monitoramento Taikai
description: Resumo do portal dos boards Grafana viewer/cache que sustentam a evidencia SN13-C.
---

A prontidao do Taikai routing-manifest (TRM) depende de dois boards Grafana e de
seus alerts associados. Esta pagina espelha os destaques de
`dashboards/grafana/taikai_viewer.json`, `dashboards/grafana/taikai_cache.json`, e
`dashboards/alerts/taikai_viewer_rules.yml` para que reviewers possam acompanhar
sem clonar o repositorio.

## Viewer dashboard (`taikai_viewer.json`)

- **Live edge e latencia:** Os panels visualizam os histogramas de latencia p95/p99
  (`taikai_ingest_segment_latency_ms`, `taikai_ingest_live_edge_drift_ms`) por
  cluster/stream. Observe p99 > 900 ms ou drift > 1.5 s (dispara o alert
  `TaikaiLiveEdgeDrift`).
- **Erros de segmentos:** Desdobra `taikai_ingest_segment_errors_total{reason}`
  para expor decode failures, lineage replay attempts ou manifest mismatches.
  Anexe screenshots a incidentes SN13-C quando este panel subir acima da banda
  "warning".
- **Saude do viewer e CEK:** Panels derivados de metricas `taikai_viewer_*`
  acompanham idade de rotacao do CEK, mix de PQ guard, contagens de rebuffer e
  roll-ups de alerts. O panel de CEK aplica o SLA de rotacao que governance
  revisa antes de aprovar novos aliases.
- **Snapshot de telemetria de aliases:** A tabela `/status -> telemetry.taikai_alias_rotations`
  fica no board para que operadores confirmem digests de manifest antes de anexar
  evidencia de governance.

## Cache dashboard (`taikai_cache.json`)

- **Pressao por tier:** Panels chartam `sorafs_taikai_cache_{hot,warm,cold}_occupancy`
  e `sorafs_taikai_cache_promotions_total`. Use para ver se uma rotacao TRM
  esta sobrecarregando tiers especificos.
- **Negacoes de QoS:** `sorafs_taikai_qos_denied_total` aparece quando a pressao
  de cache forca throttling; anote o drill log sempre que a taxa sair de zero.
- **Utilizacao de egress:** Ajuda a confirmar que os exits SoraFS acompanham os
  viewers Taikai quando janelas CMAF rotacionam.

## Alerts e captura de evidencia

- As regras de paging ficam em `dashboards/alerts/taikai_viewer_rules.yml` e
  mapeiam um-para-um com os panels acima (`TaikaiLiveEdgeDrift`,
  `TaikaiIngestFailure`, `TaikaiCekRotationLag`, proof-health warnings). Garanta
  que cada cluster de production conecte isso ao Alertmanager.
- Snapshots/screenshots capturados durante drills devem ser armazenados em
  `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` junto com spool files e o JSON
  `/status`. Use `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`
  para anexar a execucao ao drill log compartilhado.
- Quando dashboards mudarem, inclua o digest SHA-256 do arquivo JSON na descricao
  do PR do portal para que auditores possam casar a pasta Grafana gerenciada com
  a versao do repo.

## Evidence bundle checklist

Revisoes SN13-C esperam que cada drill ou incidente envie os mesmos artefacts
listados no Taikai anchor runbook. Capture-os na ordem abaixo para que o bundle
fique pronto para a revisao de governance:

1. Copie os arquivos mais recentes `taikai-anchor-request-*.json`,
   `taikai-trm-state-*.json` e `taikai-lineage-*.json` de
   `config.da_ingest.manifest_store_dir/taikai/`. Esses artefacts de spool provam
   qual routing manifest (TRM) e qual janela de lineage estavam ativos. O helper
   `cargo xtask taikai-anchor-bundle --spool <dir> --copy-dir <out> --out <out>/anchor_bundle.json [--signing-key <ed25519>]`
   copiara os spool files, emitira hashes e opcionalmente assinara o resumo.
2. Registre o output de `/v2/status` filtrado para
   `.telemetry.taikai_alias_rotations[]` e salve ao lado dos spool files.
   Reviewers comparam o `manifest_digest_hex` reportado e os limites da janela
   com o estado de spool copiado.
3. Exporte Prometheus snapshots para as metricas listadas acima e tire screenshots
   dos dashboards viewer/cache com filtros relevantes de cluster/stream visiveis.
   Coloque o JSON/CSV bruto e os screenshots na pasta de artefacts.
4. Inclua IDs de incidentes do Alertmanager (se houver) que referenciem as regras
   de `dashboards/alerts/taikai_viewer_rules.yml` e anote se eles auto-encerraram
   quando a condicao foi limpa.

Armazene tudo em `artifacts/sorafs_taikai/drills/<YYYYMMDD>/` para que audits de
Drill e revisoes de governance SN13-C possam obter um unico arquivo.

## Cadencia de drills e logging

- Execute o Taikai anchor drill na primeira terca-feira de cada mes as 15:00 UTC.
  O cronograma mantem a evidencia fresca antes do governance sync do SN13.
- Depois de capturar os artefacts acima, anexe a execucao ao ledger compartilhado
  com `scripts/telemetry/log_sorafs_drill.sh --scenario taikai-anchor`. O helper
  emite a entrada JSON exigida por `docs/source/sorafs/runbooks-index.md`.
- Linke os artefacts arquivados na entrada do runbook index e escale quaisquer
  alerts falhados ou regressions de dashboard em ate 48 horas via o canal Media
  Platform WG/SRE.
- Mantenha o set de screenshots de resumo do drill (latencia, drift, erros,
  rotacao de CEK, pressao de cache) junto ao spool bundle para que operadores
  possam mostrar exatamente como os dashboards se comportaram durante o ensaio.

Consulte o [Taikai Anchor Runbook](./taikai-anchor-runbook.md) para o procedimento
completo de Sev 1 e o checklist de evidencia. Esta pagina so captura a orientacao
especifica de dashboards que SN13-C exige antes de sair do status de progresso.
