---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Relatório de calibração de moderação de IA (2026-02)
resumo: MINFO-1 کے پہلے lançamento de governança کے لئے conjunto de dados de calibração de linha de base, limites e painel de avaliação۔
---

# Relatório de calibração de moderação de IA - Março de 2026

یہ رپورٹ **MINFO-1** کے لئے ابتدائی artefatos de calibração کو پیک کرتی ہے۔ conjunto de dados, manifesto e placar
2026-02-05 کو تیار کیے گئے، 2026-02-10 کو Conselho ministerial نے ریویو کیا، اور governança DAG میں altura
`912044` پر âncora کیے گئے۔

## Manifesto do conjunto de dados

- **Referência do conjunto de dados:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Lesma:** `ai-moderation-calibration-202602`
- **Entradas:** manifesto 480, bloco 12.800, metadados 920, áudio 160
- **Combinação de rótulos:** seguro 68%, suspeito 19%, escalada 13%
- **Resumo do artefato:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribuição:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

مکمل manifesto `docs/examples/ai_moderation_calibration_manifest_202602.json` میں موجود ہے
اور اس میں assinatura de governança کے ساتھ liberação کے وقت hash de corredor capturado شامل ہے۔

## Resumo do placar

Calibrações opset 17 do pipeline de sementes determinísticas Usando o placar JSON
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) hashes e resumos de telemetria ریکارڈ کرتا ہے؛
نیچے دی گئی tabela e métricas دکھاتی ہے۔

| Modelo (família) | Brier | CEE | AUROC | Precisão@Quarentena | Recall@Escalar |
| ------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Segurança (visão) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Segurança (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Conjunto perceptivo (perceptual) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Métricas combinadas: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Janela de calibração Distribuição de veredicto
passar 91,2%, quarentena 6,8%, escalar 2,0% تھا، جو resumo do manifesto میں درج expectativas de política سے corresponder کرتا ہے۔
Backlog falso positivo صفر رہا، اور pontuação de desvio (7,1%) Limite de alerta de 20% سے کافی نیچے تھا۔

## Limites e aprovação

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Moção de governança: `MINFO-2026-02-07`
- Assinado por `ministry-council-seat-03` em `2026-02-10T11:33:12Z`

CI نے pacote assinado کو `artifacts/ministry/ai_moderation/2026-02/` میں binários do executor de moderação کے ساتھ محفوظ کیا۔
اوپر دیے گئے manifest digest اور scoreboard hashes کو auditorias اور apelos کے دوران referir کرنا ضروری ہے۔

## Painéis e alertas

SREs de moderação no painel Grafana
Regras de alerta `dashboards/grafana/ministry_moderation_overview.json` e Prometheus
`dashboards/alerts/ministry_moderation_rules.yml` امپورٹ کرنے چاہئیں
(cobertura de teste `dashboards/alerts/tests/ministry_moderation_rules.test.yml` میں ہے)۔ یہ artefatos
barracas de ingestão, picos de desvio e fila de quarentena کی crescimento کے لئے alertas emitidos کرتے ہیں, اور
[Especificação do AI Moderation Runner] (../../ministry/ai-moderation-runner.md) Monitoramento de monitoramento
requisitos