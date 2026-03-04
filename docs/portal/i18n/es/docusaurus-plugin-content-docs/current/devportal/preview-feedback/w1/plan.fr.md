---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w1-plan
título: Plan de verificación previa de partenaires W1
sidebar_label: Plan W1
descripción: Taches, responsables y checklist de preuve pour la cohorte de previa partenaires.
---

| Elemento | Detalles |
| --- | --- |
| Vago | W1 - Partenaires e integradores Torii |
| Ventana cible | Q2 2025 semana 3 |
| Etiqueta de artefacto (planificar) | `preview-2025-04-12` |
| Rastreador de problemas | `DOCS-SORA-Preview-W1` |

## Objetivos

1. Obtener las aprobaciones legales y de gobierno para los términos de vista previa de las partes.
2. Prepare el proxy Pruébelo y utilice las instantáneas de telemetría en el paquete de invitación.
3. Rafraichir l'artefacto de vista previa verificar par checksum et les resultats de probes.
4. Finalice la lista de socios y las plantillas de demanda antes del envío de invitaciones.

## Decoupage de taches| identificación | Taché | Responsable | Echeance | Estatuto | Notas |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obtener la aprobación legal para el complemento de términos de vista previa | Líder de Docs/DevRel -> Legal | 2025-04-05 | Terminar | Billete legal `DOCS-SORA-Preview-W1-Legal` válido el 2025-04-05; PDF adjunto al rastreador. |
| W1-P2 | Capture la ventana de puesta en escena del proxy Pruébelo (2025-04-10) y valide la seguridad del proxy | Documentos/DevRel + Operaciones | 2025-04-06 | Terminar | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` ejecute le 2025-04-06; transcripción CLI + `.env.tryit-proxy.bak` archivos. |
| W1-P3 | Construir el artefacto de vista previa (`preview-2025-04-12`), ejecutador `scripts/preview_verify.sh` + `npm run probe:portal`, descriptor de archivador/sumas de verificación | Portal TL | 2025-04-08 | Terminar | Artefacto + registros de verificación de existencias sous `artifacts/docs_preview/W1/preview-2025-04-12/`; Salida de sonda adjunta al rastreador. |
| W1-P4 | Revoir les formulaires d'intake partenaires (`DOCS-SORA-Preview-REQ-P01...P08`), contactos de confirmación + NDA | Enlace de gobernanza | 2025-04-07 | Terminar | Les huit demandes approuvees (les deux dernieres le 2025-04-11); aprobaciones liees dans le tracker. |
| W1-P5 | Rediger le texte d'invitation (base sur `docs/examples/docs_preview_invite_template.md`), defina `<preview_tag>` e `<request_ticket>` para cada partenaire | Líder de Docs/DevRel | 2025-04-08 | Terminar | Brouillon d'invitation enviado le 2025-04-12 15:00 UTC con les gravámenes d'artefact. |

## Lista de verificación previa al vuelo> Astucia: lanza `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` para ejecutar automáticamente las etapas 1-5 (compilación, suma de verificación de verificación, prueba del portal, verificador de enlaces y puesta en marcha del día del proxy Pruébelo). El script registra un registro JSON unido al rastreador.

1. `npm run build` (con `DOCS_RELEASE_TAG=preview-2025-04-12`) para regenerar `build/checksums.sha256` e `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` y archivador `build/link-report.json` a cote du descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (o proporcione la información adecuada a través de `--tryit-target`); Confirme el `.env.tryit-proxy` durante un día y conserve el `.bak` para revertirlo.
6. Agregue un día al problema W1 con los caminos de registros (suma de verificación del descriptor, sonda de salida, cambio del proxy Pruébelo y instantáneas Grafana).

## Lista de verificación anterior

- [x] Aprobación legal del firmante (PDF ou lien du ticket) agregado a `DOCS-SORA-Preview-W1`.
- [x] Capturas de pantalla Grafana para `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor y registro de suma de comprobación `preview-2025-04-12` almacenados en `artifacts/docs_preview/W1/`.
- [x] Cuadro de lista de invitaciones con mensajes `invite_sent_at` (ver el registro W1 del rastreador).
- [x] Artefacts de feedback repris dans [`preview-feedback/w1/log.md`](./log.md) avec une ligne par partenaire (mis a jour 2025-04-26 avec roster/telemetria/issues).

Mettre a jour ce plan a mesure de l'avancement; El rastreador es una referencia para garantizar que la hoja de ruta sea auditable.

## Flujo de retroalimentación1. Para cada revisor, dupliquer le template dans
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   Remplir les metadonnees et stocker la copie complete sous
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Invitaciones de currículum, puntos de control de telemetría y problemas abiertos en el registro vivo
   [`preview-feedback/w1/log.md`](./log.md) para que los revisores gobiernen puissent rejouer la vague
   sin abandonar el depósito.
3. Cuando las exportaciones de conocimiento-verificación o sondajes llegan, les unen en el camino de artefacto nota en el registro
   et lier l'issue du tracker.