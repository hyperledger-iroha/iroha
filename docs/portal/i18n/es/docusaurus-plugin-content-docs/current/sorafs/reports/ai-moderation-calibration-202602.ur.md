---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Informe de calibración de moderación de IA (2026-02)
resumen: MINFO-1 کے پہلے lanzamiento de gobernanza کے لئے conjunto de datos de calibración de referencia, umbrales اور marcador۔
---

# Informe de calibración de moderación de IA: enero de 2026

یہ رپورٹ **MINFO-1** کے لئے ابتدائی artefactos de calibración کو پیک کرتی ہے۔ conjunto de datos, manifiesto y marcador
2026-02-05 کو تیار کیے گئے، 2026-02-10 کو Consejo ministerial نے ریویو کیا، اور gobernanza DAG میں altura
`912044` پر ancla کیے گئے۔

## Manifiesto del conjunto de datos

- **Referencia del conjunto de datos:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Babosa:** `ai-moderation-calibration-202602`
- **Entradas:** manifiesto 480, fragmento 12.800, metadatos 920, audio 160
- **Mezcla de etiquetas:** seguro 68%, sospechoso 19%, escalado 13%
- **Resumen de artefactos:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribución:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

Manifiesto مکمل `docs/examples/ai_moderation_calibration_manifest_202602.json` میں موجود ہے
اور اس میں firma de gobernanza کے ساتھ lanzamiento کے وقت hash de corredor capturado شامل ہے۔

## Resumen del marcador

Calibraciones opset 17 اور tubería de semillas determinista کے ساتھ چلائی گئیں۔ Marcador JSON
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) hashes اور resúmenes de telemetría ریکارڈ کرتا ہے؛
نیچے دی گئی tabla de métricas دکھاتی ہے۔| Modelo (familia) | Zarzo | CEPE | AURÓC | Precisión@Cuarentena | Recordar@Escalar |
| ------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Seguridad (visión) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Seguridad (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Conjunto perceptual (perceptivo) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Métricas combinadas: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. Ventana de calibración میں distribución de veredicto
pasar 91,2%, cuarentena 6,8%, escalar 2,0% تھا، جو resumen manifiesto میں درج expectativas políticas سے coinciden کرتا ہے۔
Acumulación de falsos positivos صفر رہا، اور puntuación de deriva (7,1%) Umbral de alerta del 20% سے کافی نیچے تھا۔

## Umbrales y aprobación

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Moción de gobernanza: `MINFO-2026-02-07`
- Firmado por `ministry-council-seat-03` en `2026-02-10T11:33:12Z`

CI نے paquete firmado کو `artifacts/ministry/ai_moderation/2026-02/` میں binarios del corredor de moderación کے ساتھ محفوظ کیا۔
اوپر دیے گئے resumen de manifiesto اور hashes del marcador کو auditorías اور apelaciones کے دوران referir کرنا ضروری ہے۔

## Paneles y alertasModeración SRE en el panel Grafana
Reglas de alerta `dashboards/grafana/ministry_moderation_overview.json` y Prometheus
`dashboards/alerts/ministry_moderation_rules.yml` امپورٹ کرنے چاہئیں
(cobertura de prueba `dashboards/alerts/tests/ministry_moderation_rules.test.yml` میں ہے)۔ یہ artefactos
puestos de ingesta, picos de deriva اور cola de cuarentena کی crecimiento کے لئے alertas emitidas کرتے ہیں، اور
[Especificación del corredor de moderación AI](../../ministry/ai-moderation-runner.md) Monitoreo de میں بیان کردہ
requisitos پوری کرتے ہیں۔