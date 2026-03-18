---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Vista previa del flujo de invitación

## Objetivo

El elemento de la hoja de ruta **DOCS-SORA** cita la incorporación de los lectores y el programa de invitaciones en vista previa pública como los últimos bloqueadores antes de la salida de la versión beta. Esta página decrit comment ouvrir cada vague d'invitations, quels artefactos doivent etre livres avant d'envoyer les invites et comment prouver que le flux est auditable. Utilisez-la avec:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) para la gestión por el selector.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para las garantías de suma de comprobación.
- [`devportal/observability`](./observability.md) para exportar telemetría y ganchos de alerta.

## Plan de vagos| Vago | Audiencia | Criterios de entrada | Criterios de salida | Notas |
| --- | --- | --- | --- | --- |
| **W0 - Núcleo de mantenedores** | Mantenedores Docs/SDK válidos para el contenido del día. | Equipo GitHub `docs-portal-preview` para personas, suma de comprobación de puerta `npm run serve` en verde, Alertmanager silencioso durante 7 días. | Todos los documentos P0 relus, tareas pendientes, incidentes aucun bloquant. | Ejecute una validación del flujo; Al enviar una invitación por correo electrónico, solo podrá compartir la vista previa de los artefactos. |
| **W1 - Socios** | Operadores SoraFS, integradores Torii, relecteurs gouvernance sous NDA. | W0 termine, términos jurídicos aprueban, proxy Try-it y puesta en escena. | Los socios de aprobación se reúnen (emiten o firman el formulario), los relojes de telemetría =2 libera expedientes de documentos a través de vista previa de canalización sin reversión. | El limitador invita a concurrentes (<=25) y el lote cada semana. |

Documentez quelle vague est active dans `status.md` et dans le tracker des demandes preview afin que la gouvernance voie le statut d'un coup d'oeil.

## Lista de verificación previa al vuelo

Terminez estas acciones **antes** de planificar invitaciones para un vago:1. **Artefactos CI disponibles**
   - Dernier `docs-portal-preview` + descriptor de carga par `.github/workflows/docs-portal-preview.yml`.
   - Pin SoraFS nota en `docs/portal/docs/devportal/deploy-guide.md` (descriptor de corte presente).
2. **Aplicación de suma de comprobación**
   - Invocación `docs/portal/scripts/serve-verified-preview.mjs` a través de `npm run serve`.
   - Instrucciones `scripts/preview_verify.sh` probadas en macOS + Linux.
3. **Telemetría de referencia**
   - `dashboards/grafana/docs_portal.json` montre un trafic Pruébelo de forma segura y la alerta `docs.preview.integrity` está en verde.
   - El último anexo de `docs/portal/docs/devportal/observability.md` se encuentra en el día con los gravámenes Grafana.
4. **Gobernanza de artefactos**
   - Issue du invite tracker prete (une issues por vague).
   - Plantilla de registro de copia de los lectores (voir [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprobaciones legales y requisitos de la SRE agregados a la emisión.

Registre la verificación previa en el rastreador de invitaciones antes de enviar el último correo electrónico.

## Cintas del flujo1. **Seleccionador de candidatos**
   - Tirer depuis la liste d'attente ou la file partners.
   - S'assurer que cada candidato tiene una plantilla de demanda completa.
2. **Aprobar el acceso**
   - Asignador de un aprobador de la emisión del rastreador de invitaciones.
   - Verificador de requisitos previos (CLA/contrato, uso aceptable, breve seguridad).
3. **Enviar invitaciones**
   - Completar los marcadores de posición de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contactos).
   - Combine el descriptor + el hash de archivo, la URL de preparación Pruébelo y los canales de soporte.
   - Stocker l'email final (o transcripción Matrix/Slack) en el número.
4. **Continuar con la incorporación**
   - Mettre un día el rastreador de invitaciones con `invite_sent_at`, `expected_exit_at` y el estado (`pending`, `active`, `complete`, `revoked`).
   - Lier la demande d'entree du recteur pour auditabilite.
5. **Supervisor de telemetria**
   - Vigilante `docs.preview.session_active` y alertas `TryItProxyErrors`.
   - Abra un incidente si la telemetría se realiza desde la línea base y registre el resultado en la cuenta de entrada de la invitación.
6. **Recopilar comentarios y clasificar**
   - Cierre las invitaciones cuando lleguen los comentarios o que `expected_exit_at` esté atento.
   - Mettre a jour l'issue de vague avec un currículum judicial (constatas, incidentes, acciones prochaines) antes de pasar a la cohorte siguiente.

## Pruebas e informes| Artefacto | Nuestro stocker | Cadencia de puesta de sol |
| --- | --- | --- |
| Rastreador de problemas de invitación | Proyecto GitHub `docs-portal-preview` | Mettre un día después de cada invitación. |
| Exportación del roster lecteurs | Registre lie dans `docs/portal/docs/devportal/reviewer-onboarding.md` | Hebdomador. |
| Telemetría instantáneas | `docs/source/sdk/android/readiness/dashboards/<date>/` (reutilizar el paquete de telemetría) | Par vague + apres incidentes. |
| Comentarios resumidos | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (creer un expediente por vago) | En los 5 días siguientes a la salida de vague. |
| Nota de reunión de gobierno | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Remplir avant cada control de sincronización DOCS-SORA. |

Lancez `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
Después de cada lote para producir un resumen lisible por la máquina. Utilice el JSON para generar un problema vago para que los lectores confirmen las cuentas de invitaciones sin recargar todo el registro.

Joignez la liste de preuves a `status.md` a chaque fin de vague afin que l'entree roadmap puisse etre mise a day rapidement.

## Criterios de reversión y pausa

Ponga en pausa el flujo de invitaciones (y notifique al gobierno) lorsque l'un des cas suivants survient:- Proxy de incidentes Pruébelo y no es necesario realizar una reversión (`npm run manage:tryit-proxy`).
- Fatiga de alertas: >3 páginas de alerta para los endpoints con vista previa solo durante 7 días.
- Tarjeta electrónica de conformidad: invitación enviada sin términos firmados o sin registro de la plantilla de demanda.
- Riesgo de integración: no coincide la suma de comprobación detectada según `scripts/preview_verify.sh`.

Reprenez only after avoir documente the remediation dans le invite tracker and confirme que la telemetría del tablero está estable al menos 48 horas.