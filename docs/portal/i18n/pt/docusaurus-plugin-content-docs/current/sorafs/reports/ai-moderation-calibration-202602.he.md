---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5a7abd1f1e5292a13a566163f305eef786d97350e16ecee33ae8c08fbe29b4f
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Relatorio de calibracao de moderacao de IA - Fevereiro 2026

Este relatorio empacota os artefatos de calibracao iniciais para **MINFO-1**. O
dataset, o manifest e o scoreboard foram produzidos em 2026-02-05, revisados pelo
conselho do Ministerio em 2026-02-10 e ancorados no DAG de governanca na altura
`912044`.

## Manifest do dataset

- **Dataset reference:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **Entries:** manifest 480, chunk 12,800, metadata 920, audio 160
- **Label mix:** safe 68%, suspect 19%, escalate 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

O manifest completo fica em `docs/examples/ai_moderation_calibration_manifest_202602.json`
e contem a assinatura de governanca e o hash do runner capturado no momento do
release.

## Resumo do scoreboard

As calibracoes rodaram com opset 17 e o pipeline de seed deterministica. O
JSON completo do scoreboard (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
registra os hashes e digests de telemetry; a tabela abaixo destaca as metricas mais
importantes.

| Modelo (familia) | Brier | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Safety (vision) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (multimodal) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| Perceptual ensemble (perceptual) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

Metricas combinadas: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. A distribuicao de
vereditos na janela de calibracao foi pass 91.2%, quarantine 6.8%,
escalate 2.0%, alinhada com as expectativas de politica registradas no resumo do
manifest. O backlog de falsos positivos permaneceu em zero, e o drift score (7.1%)
ficou bem abaixo do limiar de alerta de 20%.

## Thresholds e sign-off

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Governance motion: `MINFO-2026-02-07`
- Signed by `ministry-council-seat-03` at `2026-02-10T11:33:12Z`

CI armazenou o bundle assinado em `artifacts/ministry/ai_moderation/2026-02/`
junto com os binarios do moderation runner. O digest do manifest e os hashes do
scoreboard acima devem ser referenciados durante auditorias e apelacoes.

## Dashboards e alertas

SREs de moderacao devem importar o dashboard Grafana em
`dashboards/grafana/ministry_moderation_overview.json` e as regras de alerta do
Prometheus em `dashboards/alerts/ministry_moderation_rules.yml` (a cobertura de
tests fica em `dashboards/alerts/tests/ministry_moderation_rules.test.yml`). Esses
artefatos emitem alertas para ingest stalls, drift spikes e crescimento da fila de
quarantine, atendendo aos requisitos de monitoramento indicados na
[AI Moderation Runner Specification](../../ministry/ai-moderation-runner.md).
