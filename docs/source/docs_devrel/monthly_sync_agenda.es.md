---
lang: es
direction: ltr
source: docs/source/docs_devrel/monthly_sync_agenda.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2f89131efc0c79ddf63d71a25c04029014ba58393fb6336e676181322bc5066
source_last_modified: "2026-01-03T18:08:00.500077+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Agenda de sincronización mensual de Docs/DevRel

Esta agenda formaliza la sincronización mensual de Docs/DevRel a la que se hace referencia en todos
`roadmap.md` (consulte “Agregar revisión de personal de localización a Docs/DevRel mensuales
sync”) y el plan Android AND5 i18n. Úselo como lista de verificación canónica, y
actualícelo cada vez que los entregables de la hoja de ruta agreguen o retiren elementos de la agenda.

## Cadencia y Logística

- **Frecuencia:** mensual (normalmente el segundo jueves, 16:00 UTC)
- **Duración:** 45 minutos + Hang-back opcional de 15 minutos para inmersiones profundas
- **Ubicación:** Zoom (`https://meet.sora.dev/docs-devrel-sync`) con compartido
  notas en HackMD o `docs/source/docs_devrel/minutes/<yyyy-mm>.md`
- **Audiencia:** Gerente de Docs/DevRel (presidente), ingenieros de Docs, localización
  administrador de programas, SDK DX TL (Android, Swift, JS), documentos de producto, lanzamiento
  Delegado de ingeniería, Observadores de soporte/QA
- **Facilitador:** Administrador de Docs/DevRel; designar un escribano rotativo que
  confirmar las actas en el repositorio dentro de las 24 horas

## Lista de verificación previa al trabajo

| Propietario | Tarea | Artefacto |
|-------|------|----------|
| Escribano | Cree el archivo de notas del mes (`docs/source/docs_devrel/minutes/<yyyy-mm>.md`) usando la siguiente plantilla. | Archivo de notas |
| Localización PM | Actualice `docs/source/sdk/android/i18n_plan.md#translation-status` y el registro de personal; llenar previamente las decisiones propuestas. | plano i18n |
| DX TL | Ejecute `ci/check_android_docs_i18n.sh` o `scripts/sync_docs_i18n.py --dry-run` y adjunte resúmenes para su discusión. | Artefactos de IC |
| Herramientas de documentos | Exporte resúmenes `docs/i18n/manifest.json` + lista de tickets pendientes de `docs/source/sdk/android/i18n_requests/`. | Manifiesto y resumen del ticket |
| Soporte/Liberación | Reúna cualquier escalación que requiera acción de Docs/DevRel (por ejemplo, invitaciones de vista previa pendientes, bloqueo de comentarios de los revisores). | Status.md o documento de escalada |

## Bloques de agenda1. **Pasar lista y objetivos (5min)**
   - Confirmar quórum, escribano y logística.
   - Resalte cualquier incidente urgente (interrupción de la vista previa de documentos, bloqueo de localización).
2. **Revisión de la dotación de personal de localización (15 min)**
   - Revisar el inicio de sesión de decisiones de personal.
     `docs/source/sdk/android/i18n_plan.md#staffing-decision-log`.
   - Confirmar el estado de las órdenes de compra abiertas (`DOCS-L10N-*`) y la cobertura provisional.
   - Comparar la salida de actualización de CI con la tabla de estado de traducción; llamar a cualquier
     documento cuyo SLA local (>5 días hábiles) se incumplirá antes del próximo
     sincronización.
   - Decidir si se requiere escalamiento (Operaciones de Producto, Finanzas, contratista
     gestión). Registre la decisión tanto en el registro de dotación de personal como en el informe mensual.
     minutos, incluido propietario + fecha de vencimiento.
   - Si la dotación de personal es saludable, documente la confirmación para que la acción de la hoja de ruta pueda
     regrese a 🈺/🈴 con evidencia.
3. **Actualizaciones de documentos/hoja de ruta (10 min)**
   - Estado del trabajo del portal DOCS-SORA, proxy Try-It y publicación SoraFS
     preparación.
   - Resalte la deuda de documentos o los revisores necesarios para los trenes de lanzamiento actuales.
4. **Lo más destacado del SDK (10 min)**
   - Preparación de documentos Android AND5/AND7, paridad Swift IOS5, progreso JS GA.
   - Capture elementos compartidos o diferencias de esquemas que afectarán a los documentos.
5. **Revisión de acción y estacionamiento (5min)**
   - Revisar elementos abiertos de la sincronización anterior; confirmar cierres.
   - Registrar nuevas acciones en el archivo de notas con propietarios y plazos explícitos.

## Plantilla de revisión de dotación de personal de localización

Incluya la siguiente tabla en las actas de cada mes:

| Ubicación | Capacidad (ETC) | Compromisos y PO | Riesgos / Escaladas | Decisión y propietario |
|--------|----------------|-------------------|---------------------|------------------|
| Japón | por ejemplo, 0,5 contratista + 0,1 copia de seguridad de documentos | PO `DOCS-L10N-4901` (en espera de firma) | “Contrato no firmado antes del 4-03-2026” | “Escalar a operaciones de productos: @docs-devrel, fecha de vencimiento el 2 de marzo de 2026” |
| ÉL | por ejemplo, 0.1 Ingeniero de documentos | La rotación ingresa al PTO 2026-03-18 | “Necesita un revisor de respaldo” | “@docs-lead identificará la copia de seguridad antes del 5 de marzo de 2026” |

Registre también una breve narración que cubra:

- **Perspectiva del SLA:** Se espera que cualquier médico no cumpla el SLA de cinco días hábiles y el
  mitigación (intercambiar prioridad, contratar un proveedor de respaldo, etc.).
- **Estado de los tickets y los activos:** Entradas destacadas en
  `docs/source/sdk/android/i18n_requests/` y si las capturas de pantalla/activos son
  listo para traductores.

### Registro de revisión de dotación de personal de localización

- **Minutos:** Copie la tabla de personal + narrativa en
  `docs/source/docs_devrel/minutes/<yyyy-mm>.md` (todas las configuraciones regionales reflejan la
  Actas en inglés a través de archivos localizados en el mismo directorio). Vincular la entrada
  volver a la agenda (`docs/source/docs_devrel/monthly_sync_agenda.md`) así que
  la gobernanza puede rastrear evidencia.
- **plan i18n:** Actualizar el registro de decisiones de personal y la tabla de estado de traducción
  en `docs/source/sdk/android/i18n_plan.md` inmediatamente después de la reunión.
- **Estado:** Cuando las decisiones de personal afecten las puertas de la hoja de ruta, agregue una entrada breve en
  `status.md` (sección Docs/DevRel) que hace referencia al archivo de minutas y al plan i18n
  actualizar.

## Plantilla de actas

Copie este esqueleto en `docs/source/docs_devrel/minutes/<yyyy-mm>.md`:

```markdown
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Docs/DevRel Monthly Sync — 2026-03-12

## Attendees
- Chair: …
- Scribe: …
- Participants: …

## Agenda Notes
1. Roll call & objectives — …
2. Localization staffing review — include table + narrative.
3. Docs/roadmap updates — …
4. SDK highlights — …
5. Action review & parking lot — …

## Decisions & Actions
| Item | Owner | Due | Notes |
|------|-------|-----|-------|
| JP contractor PO follow-up | @docs-devrel-manager | 2026-03-02 | Example entry |
```Publique las notas a través de relaciones públicas poco después de la reunión y vincúlelas desde `status.md`.
al hacer referencia a riesgos o decisiones de personal.

## Expectativas de seguimiento

1. **Minutos comprometidos:** dentro de las 24 horas (`docs/source/docs_devrel/minutes/`).
2. **Plan i18n actualizado:** ajuste el registro de personal y la tabla de traducción para
   reflejar nuevos compromisos o escaladas.
3. **Entrada Status.md:** resuma cualquier decisión de alto riesgo para mantener la hoja de ruta.
   en sincronía.
4. **Escalaciones archivadas:** cuando la revisión requiera una escalada, crear/actualizar
   el ticket relevante (por ejemplo, operaciones de producto, aprobación de finanzas, incorporación de proveedores)
   y haga referencia a él tanto en las actas como en el plano i18n.

Siguiendo esta agenda, el requisito de la hoja de ruta para incluir la localización
Las revisiones de personal en la sincronización mensual de Docs/DevRel siguen siendo auditables y posteriores.
Los equipos siempre saben dónde encontrar la evidencia.