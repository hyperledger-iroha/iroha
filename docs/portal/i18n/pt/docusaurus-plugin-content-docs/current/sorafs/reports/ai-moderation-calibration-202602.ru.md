---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Отчет о калибровке модерации ИИ (2026-02)
resumo: Базовый калибровочный набор данных, пороги и placar para первого релиза управления MINFO-1.
---

# Отчет о калибровке модерации ИИ - fevereiro de 2026

Isso foi criado para fornecer calibrações de artefatos para **MINFO-1**. Data,
manifesto e placar были сформированы 2026-02-05, рассмотрены советом
Ministro 2026-02-10 e aprovado no DAG de governança em seu `912044`.

## Манифест датасета

- **Referência do conjunto de dados:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Lesma:** `ai-moderation-calibration-202602`
- **Entradas:** manifesto 480, bloco 12.800, metadados 920, áudio 160
- **Combinação de rótulos:** seguro 68%, suspeito 19%, escalada 13%
- **Resumo do artefato:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribuição:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Manifesto de política enviado para `docs/examples/ai_moderation_calibration_manifest_202602.json`
e é possível atualizar, um também hash runner, instalado no momento
релиза.

## Placar da Suécia

Калибровки запускались с opset 17 e definindo o pipeline de sementes. JSON JSON
placar (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
содержит hashes e digere telemetria; A tabela não contém métricas de classe.

| Modelo (семейство) | Brier | CEE | AUROC | Precisão@Quarentena | Recall@Escalar |
| ----------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Segurança (visão) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Segurança (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Conjunto perceptivo (perceptual) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Métricas suecas: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Expressão
вердиктов в окне калибровки составило passa 91,2%, quarentena 6,8%,
escalar 2,0%, что соответствует ожиданиям политики в сводке manifesto.
No entanto, uma pontuação de desvio (7,1%) é melhor
далеко ниже порога тревоги 20%.

## Por favor, значения e согласование

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Moção de governança: `MINFO-2026-02-07`
- Assinado por `ministry-council-seat-03` em `2026-02-10T11:33:12Z`

CI сохранила подписанный pacote em `artifacts/ministry/ai_moderation/2026-02/`
вместе с бинарями corredor de moderação. Указанные выше digest manifest e hashes
placar должны использоваться при аудитах и аpelляциях.

## Дашборды e алерты

SRE para moderação de importação do painel Grafana do painel
`dashboards/grafana/ministry_moderation_overview.json` e alerta de alerta Prometheus
em `dashboards/alerts/ministry_moderation_rules.yml` (use um teste em
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`). Эти артефакты
Gerar alertas sobre ingestão de bloqueio, desvio de velocidade e interrupção da atividade
quarentena, você pode monitorar o monitoramento, colocá-lo em
[Especificação do AI Moderation Runner](../../ministry/ai-moderation-runner.md).