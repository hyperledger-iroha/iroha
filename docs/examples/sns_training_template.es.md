---
lang: es
direction: ltr
source: docs/examples/sns_training_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd9da5045f5f40dbc31837145ad13bf79b4d751b0803c0b6d69bab49885ed1b4
source_last_modified: "2025-11-15T09:17:21.371048+00:00"
translation_last_reviewed: 2026-01-01
---

# Plantilla de diapositivas de entrenamiento SNS

Este esquema Markdown refleja las diapositivas que los facilitadores deben adaptar para sus cohortes por idioma. Copia estas secciones en Keynote/PowerPoint/Google Slides y localiza los puntos, capturas y diagramas segun sea necesario.

## Diapositiva de titulo
- Programa: "Sora Name Service onboarding"
- Subtitulo: especifica sufijo + ciclo (ej., `.sora - 2026-03`)
- Presentadores + afiliaciones

## Orientacion de KPI
- Captura o embed de `docs/portal/docs/sns/kpi-dashboard.md`
- Lista de puntos explicando filtros de sufijo, tabla ARPU, rastreador de freeze
- Llamados para exportar PDF/CSV

## Ciclo de vida del manifest
- Diagrama: registrar -> Torii -> governance -> DNS/gateway
- Pasos que referencian `docs/source/sns/registry_schema.md`
- Ejemplo de extracto de manifest con anotaciones

## Drills de disputa y freeze
- Diagrama de flujo para intervencion del guardian
- Checklist que referencia `docs/source/sns/governance_playbook.md`
- Linea de tiempo de ticket de freeze de ejemplo

## Captura de anexos
- Snippet de comando que muestra `cargo xtask sns-annex ... --portal-entry ...`
- Recordatorio de archivar JSON de Grafana en `artifacts/sns/regulatory/<suffix>/<cycle>/`
- Enlace a `docs/source/sns/reports/.<suffix>/<cycle>.md`

## Siguientes pasos
- Enlace de feedback de entrenamiento (ver `docs/examples/sns_training_eval_template.md`)
- Handles de canal Slack/Matrix
- Fechas de proximos hitos
