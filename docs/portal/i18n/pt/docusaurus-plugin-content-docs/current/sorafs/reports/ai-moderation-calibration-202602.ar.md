---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: تقرير معايرة إشراف الذكاء الاصطناعي (2026-02)
resumo: مجموعة بيانات معايرة أساسية وعتبات ولوحة نتائج لأول إصدار حوكمة MINFO-1.
---

# تقرير معايرة إشراف الذكاء الاصطناعي - Fevereiro 2026

يجمع هذا التقرير قطع معايرة البداية لـ **MINFO-1**. Como definir conjunto de dados, manifesto e placar
Em 2026-02-05, وتمت مراجعتها من مجلس الوزارة em 2026-02-10, وتم تثبيتها em DAG الحوكمة
Use o `912044`.

## Manifesto مجموعة البيانات

- **Referência do conjunto de dados:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Lesma:** `ai-moderation-calibration-202602`
- **Entradas:** manifesto 480, bloco 12.800, metadados 920, áudio 160
- **Combinação de rótulos:** seguro 68%, suspeito 19%, escalada 13%
- **Resumo do artefato:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribuição:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Manifesto do manifesto em `docs/examples/ai_moderation_calibration_manifest_202602.json`
ويتضمن توقيع الحوكمة بالإضافة إلى hash الخاص بالـ runner الملتقط وقت الإصدار.

## ملخص placar

تم تشغيل المعايرات باستخدام opset 17 ومسار البذور الحتمي. يسجل JSON para o placar do placar
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) hashes e resumos de telemetria;
Não há problema em usá-lo.

| النموذج (العائلة) | Brier | CEE | AUROC | Precisão@Quarentena | Recall@Escalar |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Segurança (visão) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Segurança (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Conjunto perceptivo (perceptual) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Nomes de referência: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. كان توزيع
A meta é passar 91,2% e quarentena 6,8% e aumentar 2,0%, e aumentar 2,0%
لتوقعات السياسة المسجلة no manifesto. ظل backlog للإيجابيات الكاذبة عند الصفر،
A pontuação de desvio total (7,1%) é de 20%.

## العتبات والمصادقة

-`thresholds.quarantine = 0.42`
-`thresholds.escalate = 0.78`
- Moção de governança: `MINFO-2026-02-07`
- Assinado por `ministry-council-seat-03` em `2026-02-10T11:33:12Z`

Use CI como parte do `artifacts/ministry/ai_moderation/2026-02/` para obter mais informações
corredor de moderação. يجب الرجوع إلى digest الخاص بالـ manifesto e hashes الخاصة بالـ placar
أعلاه أثناء عمليات التدقيق والاستئناف.

## لوحات المتابعة والتنبيهات

Os SREs são usados ​​para Grafana
`dashboards/grafana/ministry_moderation_overview.json` e Prometheus são compatíveis com Prometheus
`dashboards/alerts/ministry_moderation_rules.yml` (nome do dispositivo em questão)
`dashboards/alerts/tests/ministry_moderation_rules.test.yml`). تصدر هذه
القطع تنبيهات لتوقفات ingestão وارتفاعات deriva ونمو طابور quarentena, وتلبي متطلبات
المراقبة المشار إليها في
[Especificação do AI Moderation Runner](../../ministry/ai-moderation-runner.md).