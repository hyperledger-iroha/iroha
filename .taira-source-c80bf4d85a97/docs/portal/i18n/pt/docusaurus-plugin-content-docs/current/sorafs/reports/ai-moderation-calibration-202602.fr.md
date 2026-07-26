---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Relatório de calibração de moderação IA (2026-02)
resumo: Conjunto de dados de calibração de base, resultados e placar para o primeiro lançamento de governo MINFO-1.
---

# Relatório de calibração de moderação IA - Février 2026

Este relatório reagrupa os artefatos de calibração inaugurados para **MINFO-1**. Le
conjunto de dados, o manifesto e o placar foram produzidos em 05/02/2026, revisados por
le conseil du ministère le 2026-02-10, et ancrés dans le DAG de gouvernance à la
altivez `912044`.

## Manifesto do conjunto de dados

- **Referência do conjunto de dados:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Lesma:** `ai-moderation-calibration-202602`
- **Entradas:** manifesto 480, bloco 12.800, metadados 920, áudio 160
- **Combinação de rótulos:** seguro 68%, suspeito 19%, escalada 13%
- **Resumo do artefato:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribuição:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

O manifesto completo se encontrado em `docs/examples/ai_moderation_calibration_manifest_202602.json`
e contém a assinatura do governo assim como o hash do corredor capturado
momento do lançamento.

## Resumo do placar

As calibrações foram feitas com o conjunto 17 e o pipeline de grãos determinados. Le
JSON completo do placar (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
enviar hashes e resumos de telemetria; le tableau ci-dessous met en avant les
métricas mais importantes.

| Modelo (família) | Brier | CEE | AUROC | Precisão@Quarentena | Recall@Escalar |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Segurança (visão) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Segurança (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Conjunto perceptivo (perceptual) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Métricas combinadas: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. A distribuição
os veredictos na janela de calibração foram aprovados em 91,2%, quarentena em 6,8%,
escalar 2,0%, o que corresponde às atenções de política indicadas no
currículo do manifesto. O acúmulo de falsos positivos é reduzido a zero e a pontuação de desvio
(7,1%) está bem em seu último alerta de 20%.

##Seus e validação

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Moção de governança: `MINFO-2026-02-07`
- Assinado por `ministry-council-seat-03` em `2026-02-10T11:33:12Z`

CI a stocké le bundle signé dans `artifacts/ministry/ai_moderation/2026-02/`
aux côtés des binaires du moderation runner. O resumo do manifesto e os hashes
o placar ci-dessus deve ser referenciado para auditorias e recursos.

## Painéis e alertas

O SRE de modificação deve importar o painel Grafana localizado em
`dashboards/grafana/ministry_moderation_overview.json` e as regras de alerta
Prometheus e `dashboards/alerts/ministry_moderation_rules.yml` (a cobertura
os testes são encontrados em `dashboards/alerts/tests/ministry_moderation_rules.test.yml`).
Esses artefatos emitem alertas para bloqueios de ingestão, fotos de deriva
e o croissance da quarentena de arquivos, satisfazendo as exigências de monitoramento
mencionado na [Especificação do AI Moderation Runner](../../ministry/ai-moderation-runner.md).