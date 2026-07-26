---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Informe de calibración de moderación IA (2026-02)
resumen: Conjunto de datos de calibración de base, señales y marcador para la primera versión de gobierno MINFO-1.
---

# Informe de calibración de moderación IA - Febrero 2026

Este informe reagrupa los artefactos de calibración inauguraux para **MINFO-1**. le
conjunto de datos, el manifiesto y el marcador ont été produits le 2026-02-05, revus par
le conseil du ministère le 2026-02-10, et ancrés dans le DAG de gouvernance à la
altanería `912044`.

## Manifiesto del conjunto de datos

- **Referencia del conjunto de datos:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Babosa:** `ai-moderation-calibration-202602`
- **Entradas:** manifiesto 480, fragmento 12.800, metadatos 920, audio 160
- **Mezcla de etiquetas:** seguro 68%, sospechoso 19%, escalado 13%
- **Resumen de artefactos:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribución:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

El manifiesto completo se encuentra en `docs/examples/ai_moderation_calibration_manifest_202602.json`
et contient la firma de gobierno así como el hash du runner capturado en
momento de la liberación.

## Resumen del marcador

Las calibraciones se realizan con el ajuste 17 y la tubería de granos determinada. le
JSON completo del marcador (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
consignar hashes y resúmenes de telemetría; le tableau ci-dessous met en avant les
Métricas más importantes.| Modelo (familia) | Zarzo | CEPE | AURÓC | Precisión@Cuarentena | Recordar@Escalar |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Seguridad (visión) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Seguridad (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Conjunto perceptual (perceptivo) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Métricas combinadas: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. La distribucion
los veredictos sobre la ventana de calibración était pasan el 91,2%, la cuarentena el 6,8%,
escalar 2.0%, ce qui correspond aux attentes de politique indiquées dans le
currículum del manifiesto. La acumulación de falsos positivos está restada a cero y la puntuación de deriva
(7,1%) esté bien en dessous du seuil d'alerte de 20%.

## Seuils y validación

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Moción de gobernanza: `MINFO-2026-02-07`
- Firmado por `ministry-council-seat-03` en `2026-02-10T11:33:12Z`

CI almacenado en el paquete firmado en `artifacts/ministry/ai_moderation/2026-02/`
aux côtés des binaires du moderation runner. El resumen del manifiesto y los hashes
du scoreboard ci-dessus doivent être référencés lors des audits et des recurs.

## Paneles y alertasEl SRE de moderación debe importar el tablero Grafana situado en
`dashboards/grafana/ministry_moderation_overview.json` y las reglas de alerta
Prometheus en `dashboards/alerts/ministry_moderation_rules.yml` (la cobertura
de pruebas se encuentran en `dashboards/alerts/tests/ministry_moderation_rules.test.yml`).
Estos artefactos emiten alertas para bloqueos de ingestión y fotos de deriva.
y el croissance de la cuarentena de archivos, satisfaisant les exigences de monitoreo
Se menciona en la [Especificación del corredor de moderación de IA] (../../ministry/ai-moderation-runner.md).