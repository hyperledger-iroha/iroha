---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Relatório de calibração de moderação de IA (2026-02)
resumo: Dataset de base de calibração, limiares e placar para o primeiro release de governança MINFO-1.
---

# Relatório de calibração de moderação de IA - Fevereiro 2026

Este relatorio contratou os artefatos de calibração iniciais para **MINFO-1**. Ó
dataset, o manifesto e o placar foram produzidos em 2026-02-05, revisados pelo
conselho do Ministério em 2026-02-10 e ancorados no DAG de governação na altura
`912044`.

## Manifesto do conjunto de dados

- **Referência do conjunto de dados:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Lesma:** `ai-moderation-calibration-202602`
- **Entradas:** manifesto 480, bloco 12.800, metadados 920, áudio 160
- **Combinação de rótulos:** seguro 68%, suspeito 19%, escalada 13%
- **Resumo do artefato:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribuição:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

O manifesto completo fica em `docs/examples/ai_moderation_calibration_manifest_202602.json`
e contem a assinatura de governança e o hash do runner capturado no momento do
lançamento.

## Resumo do placar

As calibrações rodaram com opset 17 e o pipeline de seed determinística. Ó
JSON completo do placar (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
registrar os hashes e resumos de telemetria; a tabela abaixo destaca as métricas mais
importantes.

| Modelo (família) | Brier | CEE | AUROC | Precisão@Quarentena | Recall@Escalar |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Segurança (visão) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Segurança (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Conjunto perceptivo (perceptual) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Métricas combinadas: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. A distribuição de
vereditos na janela de calibração foi passado 91,2%, quarentena 6,8%,
escalate 2,0%, homologado com as expectativas de política registradas no resumo do
manifestar. O backlog de falsos positivos encontrados em zero, e o drift score (7,1%)
ficou bem abaixo dos limites de alerta de 20%.

## Limites e aprovação

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Moção de governança: `MINFO-2026-02-07`
- Assinado por `ministry-council-seat-03` em `2026-02-10T11:33:12Z`

CI armazenou o pacote assinado em `artifacts/ministry/ai_moderation/2026-02/`
junto com os binários do moderation runner. O digest do manifesto e os hashes do
O placar acima deve ser referenciado durante auditorias e apelações.

## Painéis e alertas

SREs de moderação devem importar o dashboard Grafana em
`dashboards/grafana/ministry_moderation_overview.json` e as regras de alerta do
Prometheus em `dashboards/alerts/ministry_moderation_rules.yml` (uma cobertura de
testes fica em `dashboards/alerts/tests/ministry_moderation_rules.test.yml`). Esses
artefatos emitem alertas para barracas de ingestão, picos de deriva e crescimento da fila de
quarentena, atendendo aos requisitos de monitoramento indicados na
[Especificação do AI Moderation Runner](../../ministry/ai-moderation-runner.md).