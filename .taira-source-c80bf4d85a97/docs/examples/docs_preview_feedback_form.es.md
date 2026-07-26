---
lang: es
direction: ltr
source: docs/examples/docs_preview_feedback_form.md
status: complete
translator: manual
source_hash: afb7e51ddc0b7e819f2cbf3888aadf907b0e0010c676cb44af648f9f4818f8f5
source_last_modified: "2025-11-10T19:22:20.036140+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Traducción al español de docs/examples/docs_preview_feedback_form.md (Docs preview feedback form) -->

# Formulario de feedback para el preview de docs (oleada W1)

Usa esta plantilla al recopilar feedback de reviewers de la oleada W1. Duplica el archivo
por partner, rellena los metadatos y guarda la copia completada en
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.

## Metadatos del reviewer

- **ID del partner:** `partner-w1-XX`
- **Ticket de solicitud:** `DOCS-SORA-Preview-REQ-PXX`
- **Invitación enviada (UTC):** `YYYY-MM-DD hh:mm`
- **Checksum reconocido (UTC):** `YYYY-MM-DD hh:mm`
- **Áreas de foco principales:** (por ejemplo _docs del orquestador SoraFS_,
  _flujos ISO de Torii_)

## Confirmaciones de telemetría y artefactos

| Elemento de checklist | Resultado | Evidencia |
| --- | --- | --- |
| Verificación de checksums | ✅ / ⚠️ | Ruta al log (por ejemplo, `build/checksums.sha256`) |
| Smoke test del proxy Try it | ✅ / ⚠️ | Fragmento de `npm run manage:tryit-proxy …` |
| Revisión de dashboard en Grafana | ✅ / ⚠️ | Ruta(s) a captura(s) de pantalla |
| Revisión del informe de probe del portal | ✅ / ⚠️ | `artifacts/docs_preview/.../preflight-summary.json` |

Añade filas para cualquier SLO adicional que el reviewer inspeccione.

## Registro de feedback

| Área | Severidad (info/minor/major/blocker) | Descripción | Fix sugerido o pregunta | Issue de seguimiento |
| --- | --- | --- | --- | --- |
| | | | | |

Haz referencia al issue de GitHub o al ticket interno en la última columna para que el
tracker del preview pueda vincular los elementos de remediación con este formulario.

## Resumen de la encuesta

1. **¿Qué nivel de confianza tienes en las instrucciones de checksums y el proceso de
   invitación?** (1–5)
2. **¿Qué docs fueron más/menos útiles?** (respuesta corta)
3. **¿Encontraste bloqueos al acceder al proxy Try it o a los dashboards de telemetría?**
4. **¿Hace falta contenido adicional de localización o accesibilidad?**
5. **¿Algún comentario adicional antes del GA?**

Recoge respuestas breves y adjunta exportaciones crudas de la encuesta si estás usando un
formulario externo.

## Comprobación de conocimiento

- Puntuación: `__/10`
- Preguntas incorrectas (si las hay): `[#1, #4, …]`
- Acciones de seguimiento (si la puntuación es < 9/10): ¿hay llamada de remediación
  programada? s/n

## Aprobación

- Nombre del reviewer y marca de tiempo:
- Nombre del reviewer de Docs/DevRel y marca de tiempo:

Guarda la copia firmada junto con los artefactos asociados para que quienes auditan
puedan reproducir la oleada sin contexto adicional.
