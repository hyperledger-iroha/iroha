---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/ai-moderation-calibration-202602.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Informe de calibración de moderación de IA (2026-02)
resumen: Conjunto de datos de calibración base, umbrales y marcador para el primer lanzamiento de gobernanza MINFO-1.
---

# Informe de calibración de moderación de IA - Febrero 2026

Este informe empaqueta los artefactos de calibración inaugurales para **MINFO-1**. el
conjunto de datos, manifiesto y marcador se produjo el 2026-02-05, fueron revisados por el
consejo del Ministerio el 2026-02-10 y se anclaron en el DAG de gobernanza en la altura
`912044`.

## Manifiesto del conjunto de datos

- **Referencia del conjunto de datos:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Babosa:** `ai-moderation-calibration-202602`
- **Entradas:** manifiesto 480, fragmento 12.800, metadatos 920, audio 160
- **Mezcla de etiquetas:** seguro 68%, sospechoso 19%, escalado 13%
- **Resumen de artefactos:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribución:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

El manifiesto completo vive en `docs/examples/ai_moderation_calibration_manifest_202602.json`
y contiene la firma de gobernanza más el hash del runner capturado en el momento del
liberación.

## Resumen del marcador

Las calibraciones se ejecutaron con opset 17 y el pipeline de semillas determinísticas. el
JSON completo del marcador (`docs/examples/ai_moderation_calibration_scorecard_202602.json`)
registra los hashes y resúmenes de telemetría; la tabla siguiente destaca las métricas más
importantes.| Modelo (familia) | Zarzo | CEPE | AURÓC | Precisión@Cuarentena | Recordar@Escalar |
| --------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Seguridad (visión) | 0,141 | 0,031 | 0,987 | 0,964 | 0,912 |
| LLaVA-1.6 34B Seguridad (multimodal) | 0,118 | 0,028 | 0,978 | 0,942 | 0,904 |
| Conjunto perceptual (perceptual) | 0,162 | 0,047 | 0,953 | 0,883 | 0,861 |

Métricas combinadas: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`. La distribucion de
veredictos en la ventana de calibración fue aprobado 91,2%, cuarentena 6,8%,
escalar 2.0%, coincidiendo con las expectativas de política registradas en el
resumen del manifiesto. El backlog de falsos positivos se mantuvo en cero y el
La puntuación de deriva (7,1%) quedó muy por debajo del umbral de alerta del 20%.

## Umbrales y aprobación

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Moción de gobernanza: `MINFO-2026-02-07`
- Firmado por `ministry-council-seat-03` en `2026-02-10T11:33:12Z`

CI almacenó el paquete firmado en `artifacts/ministry/ai_moderation/2026-02/`
junto con los binarios del moderation runner. El resumen del manifiesto y los hashes
del marcador anterior deben referenciarse durante auditorías y apelaciones.

## Paneles y alertasLos SRE de moderación deben importar el tablero de Grafana en
`dashboards/grafana/ministry_moderation_overview.json` y las reglas de alertas de
Prometheus y `dashboards/alerts/ministry_moderation_rules.yml` (la cobertura de pruebas
vive en `dashboards/alerts/tests/ministry_moderation_rules.test.yml`). estos
artefactos emiten alertas por bloqueos de ingesta, picos de deriva y crecimiento de
la cola de cuarentena, cumpliendo los requisitos de monitoreo indicados en la
[Especificación del corredor de moderación de IA](../../ministry/ai-moderation-runner.md).