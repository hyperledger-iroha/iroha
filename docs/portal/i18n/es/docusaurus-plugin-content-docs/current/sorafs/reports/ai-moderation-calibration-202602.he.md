---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 31fe86afc1471673471393db8dafa7a921de50635508bf68b311dd2d5f44e6c4
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Informe de calibración de moderación de IA - Febrero 2026

Este informe empaqueta los artefactos de calibración inaugurales para **MINFO-1**. El
dataset, manifest y scoreboard se produjeron el 2026-02-05, fueron revisados por el
consejo del Ministerio el 2026-02-10 y se anclaron en el DAG de gobernanza en la altura
`912044`.

## Manifiesto del dataset

- **Dataset reference:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **Entries:** manifest 480, chunk 12,800, metadata 920, audio 160
- **Label mix:** safe 68%, suspect 19%, escalate 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

El manifiesto completo vive en `docs/examples/ai_moderation_calibration_manifest_202602.json`
y contiene la firma de gobernanza más el hash del runner capturado en el momento del
release.

## Resumen del scoreboard

Las calibraciones se ejecutaron con opset 17 y el pipeline de semillas determinísticas. El
JSON completo del scoreboard (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
registra los hashes y digests de telemetry; la tabla siguiente destaca las métricas más
importantes.

| Modelo (familia) | Brier | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Safety (vision) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (multimodal) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| Perceptual ensemble (perceptual) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

Métricas combinadas: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. La distribución de
veredictos en la ventana de calibración fue pass 91.2%, quarantine 6.8%,
escalate 2.0%, coincidiendo con las expectativas de política registradas en el
resumen del manifest. El backlog de falsos positivos se mantuvo en cero y el
drift score (7.1%) quedó muy por debajo del umbral de alerta del 20%.

## Umbrales y aprobación

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Governance motion: `MINFO-2026-02-07`
- Signed by `ministry-council-seat-03` at `2026-02-10T11:33:12Z`

CI almacenó el bundle firmado en `artifacts/ministry/ai_moderation/2026-02/`
junto con los binarios del moderation runner. El digest del manifest y los hashes
del scoreboard anteriores deben referenciarse durante auditorías y apelaciones.

## Dashboards y alertas

Los SRE de moderación deben importar el dashboard de Grafana en
`dashboards/grafana/ministry_moderation_overview.json` y las reglas de alertas de
Prometheus en `dashboards/alerts/ministry_moderation_rules.yml` (la cobertura de tests
vive en `dashboards/alerts/tests/ministry_moderation_rules.test.yml`). Estos
artefactos emiten alertas por bloqueos de ingesta, picos de drift y crecimiento de
la cola de quarantine, cumpliendo los requisitos de monitoreo indicados en la
[AI Moderation Runner Specification](../../ministry/ai-moderation-runner.md).
