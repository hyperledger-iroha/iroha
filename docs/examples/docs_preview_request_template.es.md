---
lang: es
direction: ltr
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-11-10T20:01:03.610024+00:00"
translation_last_reviewed: 2026-01-01
---

# Solicitud de acceso de vista previa del portal de docs (Plantilla)

Usa esta plantilla cuando captures los detalles del reviewer antes de otorgar acceso al
entorno de preview publico. Copia el markdown en un issue o formulario de solicitud y
reemplaza los placeholders.

```markdown
## Resumen de solicitud
- Solicitante: <nombre completo / org>
- Usuario de GitHub: <username>
- Contacto preferido: <email/Matrix/Signal>
- Region y zona horaria: <UTC offset>
- Fechas propuestas de inicio / fin: <YYYY-MM-DD -> YYYY-MM-DD>
- Tipo de reviewer: <Core maintainer | Partner | Community volunteer>

## Checklist de cumplimiento
- [ ] Firmo la politica de uso aceptable de la preview (link).
- [ ] Reviso `docs/portal/docs/devportal/security-hardening.md`.
- [ ] Reviso `docs/portal/docs/devportal/incident-runbooks.md`.
- [ ] Confirmo recoleccion de telemetria y analitica anonimizada (si/no).
- [ ] Alias de SoraFS solicitado (si/no). Nombre del alias: `<docs-preview-???>`

## Necesidades de acceso
- URL(s) de preview: <https://docs-preview.sora.link/...>
- Scopes de API requeridos: <Torii read-only | Try it sandbox | none>
- Contexto adicional (tests de SDK, foco de revision de docs, etc.):
  <detalles aqui>

## Aprobacion
- Reviewer (maintainer): <nombre + fecha>
- Ticket de governance / solicitud de cambio: <link>
```

---

## Preguntas especificas para la comunidad (W2+)
- Motivacion para acceso de preview (una oracion):
- Foco principal de revision (SDK, governance, Norito, SoraFS, otro):
- Compromiso semanal de tiempo y ventana de disponibilidad (UTC):
- Necesidades de localizacion o accesibilidad (si/no + detalles):
- Codigo de Conducta de la Comunidad + addendum de uso aceptable de preview reconocido (si/no):
