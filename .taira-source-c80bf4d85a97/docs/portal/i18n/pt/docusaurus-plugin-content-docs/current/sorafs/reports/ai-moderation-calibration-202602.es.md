---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Informe de calibração de moderação de IA (2026-02)
resumo: Conjunto de dados de calibração base, guarda-chuvas e placar para o primeiro lançamento de governo MINFO-1.
---

# Informe de calibração de moderação de IA - Fevereiro de 2026

Este informa o pacote dos artefatos de calibração inaugurais para **MINFO-1**. El
conjunto de dados, manifesto e placar foram produzidos em 05/02/2026, revisados ​​pelo
conselho do Ministério em 10/02/2026 e anúncio no DAG de governo em altura
`912044`.

## Manifestado no conjunto de dados

- **Referência do conjunto de dados:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Lesma:** `ai-moderation-calibration-202602`
- **Entradas:** manifesto 480, bloco 12.800, metadados 920, áudio 160
- **Combinação de rótulos:** seguro 68%, suspeito 19%, escalada 13%
- **Resumo do artefato:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribuição:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

O manifesto completo vive em `docs/examples/ai_moderation_calibration_manifest_202602.json`
e contém a firma de governança além do hash do corredor capturado no momento do
lançamento.

## Resumo do placar

As calibrações são executadas com o opset 17 e o pipeline de sementes determinísticas. El
JSON completo do placar (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
registrar os hashes e resumos de telemetria; a tabela a seguir destaca as considerações mais detalhadas
importantes.

| Modelo (família) | Brier | CEE | AUROC | Precisão@Quarentena | Recall@Escalar |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Segurança (visão) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Segurança (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Conjunto perceptivo (perceptual) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Métricas combinadas: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. A distribuição de
vereditos na janela de calibração foram aprovados em 91,2%, quarentena em 6,8%,
escalar 2,0%, coincidindo com as expectativas de políticas registradas no
resumo do manifesto. O backlog de falsos positivos se mantuvo em zero e o
pontuação de desvio (7,1%) que foi muito inferior ao umbral de alerta de 20%.

## Umbrais e aprovação

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Moção de governança: `MINFO-2026-02-07`
- Assinado por `ministry-council-seat-03` em `2026-02-10T11:33:12Z`

CI armazenou o pacote firmado em `artifacts/ministry/ai_moderation/2026-02/`
junto com os binários do corredor de moderação. O resumo do manifesto e os hashes
O placar anterior deve ser referenciado durante audiências e apelações.

## Painéis e alertas

O SRE de moderação deve importar o painel do Grafana em
`dashboards/grafana/ministry_moderation_overview.json` e as regras de alerta de
Prometheus e `dashboards/alerts/ministry_moderation_rules.yml` (a cobertura de testes
vive em `dashboards/alerts/tests/ministry_moderation_rules.test.yml`). Estos
artefatos emitem alertas sobre bloqueios de ingestão, picos de deriva e crescimento de
la cola de quarentena, cumprindo os requisitos de monitoramento indicados na la
[Especificação do AI Moderation Runner](../../ministry/ai-moderation-runner.md).