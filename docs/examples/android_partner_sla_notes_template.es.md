---
lang: es
direction: ltr
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f1d3e5d2b42f7e6f9c2de4f2be56b6994b4b88b109f70edc7e6f04ec0f3465ac
source_last_modified: "2025-11-12T08:32:28.349523+00:00"
translation_last_reviewed: 2026-01-01
---

# Notas de descubrimiento de SLA para partner Android - Plantilla

Usa esta plantilla para cada sesion de descubrimiento de SLA AND8. Guarda la copia completada
bajo `docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md` y adjunta
artefactos de soporte (respuestas del cuestionario, acuses, adjuntos) en el mismo directorio.

```
Partner: <Nombre>                      Fecha: <YYYY-MM-DD>  Hora: <UTC>
Contactos principales: <nombres, roles, email>
Asistentes Android: <Program Lead / Partner Eng / Support Eng / Compliance>
Link de reunion / ticket: <URL o ID>
```

## 1. Agenda y contexto

- Proposito de la sesion (alcance del piloto, ventana de release, expectativas de telemetria).
- Docs de referencia compartidos antes de la llamada (support playbook, calendario de release,
  dashboards de telemetria).

## 2. Panorama de carga

| Tema | Notas |
|------|-------|
| Cargas objetivo / cadenas | |
| Volumen de transacciones esperado | |
| Ventanas criticas de negocio / periodos de blackout | |
| Regimenes regulatorios (GDPR, MAS, FISC, etc.) | |
| Idiomas requeridos / localizacion | |

## 3. Discusion de SLA

| Clase de SLA | Expectativa del partner | Diferencia vs baseline? | Accion requerida |
|--------------|-------------------------|--------------------------|------------------|
| Correccion critica (48 h) | | Si/No | |
| Severidad alta (5 business days) | | Si/No | |
| Mantenimiento (30 days) | | Si/No | |
| Aviso de cutover (60 days) | | Si/No | |
| Cadencia de comunicacion de incidentes | | Si/No | |

Documenta cualquier clausula adicional de SLA solicitada por el partner (p. ej., puente
telefonico dedicado, exportaciones extra de telemetria).

## 4. Requisitos de telemetria y acceso

- Necesidades de acceso Grafana / Prometheus:
- Requisitos de exportacion de logs/traces:
- Expectativas de evidencia offline o dossier:

## 5. Notas de compliance y legales

- Requisitos de notificacion jurisdiccional (estatuto + timing).
- Contactos legales requeridos para updates de incidentes.
- Restricciones de residencia de datos / requisitos de almacenamiento.

## 6. Decisiones y acciones

| Item | Owner | Fecha | Notas |
|------|-------|-------|-------|
| | | | |

## 7. Reconocimiento

- Partner reconocio SLA base? (S/N)
- Metodo de reconocimiento de seguimiento (email / ticket / firma):
- Adjunta email de confirmacion o minutos de la reunion a este directorio antes de cerrar.
