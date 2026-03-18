---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Fluxo de invitaciones hacer vista previa

## Objetivo

El elemento de la hoja de ruta **DOCS-SORA** destaca la incorporación de revisores y el programa de invitaciones a la vista previa pública como los últimos bloqueadores antes de que el portal salga de la versión beta. Esta página descreve como abrir cada onda de convites, quais artefatos devem ser enviados antes de mandar convites e como provar que o fluxo e auditavel. Utilice junto com:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) para o manejo por revisor.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) para garantías de suma de comprobación.
- [`devportal/observability`](./observability.md) para exportaciones de telemetría y ganchos de alertas.

##Plano de ondas| Onda | Audiencia | Criterios de entrada | Criterios de dicha | Notas |
| --- | --- | --- | --- | --- |
| **W0 - Núcleo de mantenedores** | Mantenedores de Docs/SDK validando conteudo do dia um. | Time GitHub `docs-portal-preview` poblado, puerta de suma de comprobación `npm run serve` verde, Alertmanager silencioso por 7 días. | Todos los documentos P0 revisados, backlog tagueado, sin incidentes bloqueadores. | Usado para validar o fluxo; Sin correo electrónico de invitación, apenas compartilhar os artefactos de vista previa. |
| **W1 - Socios** | Operadores SoraFS, integradores Torii, revisores de gobierno sob NDA. | W0 encerrado, termos legales aprobados, proxy Try-it em staging. | Sign-off dos partners coletado (issue ou formulario assinado), telemetria mostra =2 publicaciones de documentación enviadas a través del canal de vista previa sin reversión. | Limitar invitaciones simultáneas (<=25) y agrupar semanalmente. |

Documente qual onda esta activa em `status.md` y no tracker de solicitudes de vista previa para que agobernanca veja o estado rapidamente.

## Lista de verificación de verificación previaConclua estos acoes **antes** de agendar convites para uma onda:

1. **Artefatos de CI disponibles**
   - Último `docs-portal-preview` + descriptor enviado por `.github/workflows/docs-portal-preview.yml`.
   - Pin de SoraFS anotado en `docs/portal/docs/devportal/deploy-guide.md` (descriptor de corte presente).
2. **Cumplimiento de la suma de control**
   - `docs/portal/scripts/serve-verified-preview.mjs` invocado a través de `npm run serve`.
   - Instrucciones de `scripts/preview_verify.sh` probadas en macOS + Linux.
3. **Línea base de telemetría**
   - `dashboards/grafana/docs_portal.json` mostra trafego Pruébalo saudavel e o alerta `docs.preview.integrity` esta verde.
   - Último apéndice de `docs/portal/docs/devportal/observability.md` actualizado con enlaces de Grafana.
4. **Artefatos de gobierno**
   - Problema para invitar al rastreador pronta (uma problema por onda).
   - Plantilla de registro de revisores copiada (ver [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Aprovacoes legais e de SRE requeridas anexadas a issues.

Regístrese para concluir la verificación previa sin rastreador de invitaciones antes de enviar cualquier correo electrónico.

##Etapas do fluxo1. **Seleccionar candidatos**
   - Puxar da planilha de espera ou fila de socios.
   - Garantir que cada candidato tenga o plantilla de solicitud completa.
2. **Aprovar acceso**
   - Atribuir un aprobador a un problema de seguimiento de invitaciones.
   - Verificar prerequisitos (CLA/contrato, uso aceitavel, brief de seguranca).
3. **Enviar invitaciones**
   - Preencher os placeholders de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, contactos).
   - Anexar el descriptor + hash del archivo, URL de preparación para Pruébalo y canales de soporte.
   - Guardar el correo electrónico final (o la transcripción de Matrix/Slack) en caso de problema.
4. **Acompañar la incorporación**
   - Actualizar el rastreador de invitaciones con `invite_sent_at`, `expected_exit_at`, y el estado (`pending`, `active`, `complete`, `revoked`).
   - Linkar a solicitudcao de entrada do revisor para auditabilidade.
5. **Monitorear telemetría**
   - Observador `docs.preview.session_active` y alertas `TryItProxyErrors`.
   - Abrir un incidente se a telemetría desviar do baseline y registrar o resultado al lado de la entrada de convite.
6. **Coletar feedback y encerrar**
   - Encerrar convites quando o feedback chegar ou `expected_exit_at` expirar.
   - Atualizar a issues da onda com um resumo curto (achados, incidentes, proximas acoes) antes de pasar para a proxima coorte.

## Evidencia e informes| Artefacto | Onde armazenar | Cadencia de actualización |
| --- | --- | --- |
| Problema con el rastreador de invitaciones | Proyecto GitHub `docs-portal-preview` | Actualizar apos cada convite. |
| Exportar la lista de revisores | Registro vinculado en `docs/portal/docs/devportal/reviewer-onboarding.md` | Semanal. |
| Instantáneas de telemetría | `docs/source/sdk/android/readiness/dashboards/<date>/` (reutilizar paquete de telemetría) | Por onda + apos incidentes. |
| Resumen de comentarios | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (criar pasta por onda) | Dentro de 5 días apos a saya da onda. |
| Nota de reunión de gobierno | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Preencher antes de cada sincronización DOCS-SORA. |

Ejecute `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
apos cada lote para producir un digest legivel por maquina. Anexe o JSON renderizado a problema da onda para que los revisores de gobierno confirmen como contagios de convite sin reproducir todo el registro.

Anexe una lista de evidencias a `status.md` siempre que una onda terminar para que una entrada do roadmap possa ser actualizada rápidamente.

## Criterios de reversión y pausa

Pause o fluxo de convites (e notifique agobernanza) quando qualquer um dos itens abaixo ocorrer:

- Incidente de proxy Pruébelo que exigiu rollback (`npm run manage:tryit-proxy`).
- Fadiga de alertas: >3 páginas de alerta para puntos finales apenas de vista previa en 7 días.
- Falta de cumplimiento: convite enviado sem termos assinados ou sem registrador o template de solicitacao.
- Riesgo de integridad: falta de coincidencia de suma de comprobación detectada por `scripts/preview_verify.sh`.Retome apos documentar a remediacao no invite tracker y confirme que el tablero de telemetría está funcionando por pelo menos 48 horas.