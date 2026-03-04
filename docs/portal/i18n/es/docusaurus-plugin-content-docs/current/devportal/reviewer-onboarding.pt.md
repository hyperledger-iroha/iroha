---
lang: es
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Onboarding de revisores hacer vista previa

## Visao general

DOCS-SORA acompaña un lanzamiento en fases del portal de desarrolladores. Construye con gate de checksum
(`npm run serve`) e fluxos Pruébalo reforcados destravam o proximo marco:
onboarding de revisores validados antes de o vista previa pública se abrirá ampliamente. Esta guia
descreve como coletar solicitudes, verificar elegibilidade, provisionar acesso y fazer offboarding
de participantes com seguranca. Consulta o
[vista previa del flujo de invitación](./preview-invite-flow.md) para el planeamiento de coortes, a
cadencia de invitaciones y exportaciones de telemetría; os passos abaixo focam nas acoes
a tomar quando um revisor ja foi seleccionado.

- **Escopo:** revisores que necesitan acceso a la vista previa de documentos (`docs-preview.sora`,
  compilaciones de páginas de GitHub o paquetes de SoraFS) antes de GA.
- **Foro do escopo:** operadores de Torii o SoraFS (cobertos por sus propios kits de onboarding)
  e implantacoes do portal em producao (ver
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Papeles y requisitos previos| Papel | Objetivos típicos | Artefatos requeridos | Notas |
| --- | --- | --- | --- |
| Mantenedor principal | Verificar novos guías, ejecutar pruebas de humo. | Identificador de GitHub, contacto Matrix, CLA asociado en archivo. | Generalmente ja esta no hay tiempo GitHub `docs-preview`; Aún así registre una solicitud para que el acceso seja auditavel. |
| Revisor socio | Validar fragmentos de SDK o contenido de gobierno antes de publicarlos. | Email corporativo, POC legal, termos de vista previa asinados. | Deve reconhecer requisitos de telemetria + tratamento de datos. |
| Voluntario comunitario | Fornecer feedback de usabilidade sobre guías. | Mango de GitHub, contacto preferido, fuso horario, aceitacao do CoC. | Mantenha coortes pequeñas; priorize revisores que assinaram o acordo de contribuicao. |

Todos los tipos de revisores deben ser:

1. Reconhecer a politica de uso aceitavel para artefactos de vista previa.
2. Ler os apendices de seguranca/observabilidade
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Concordar em ejecutar `docs/portal/scripts/preview_verify.sh` antes de servir qualquer
   instantánea localmente.

## Flujo de ingesta1. Pedir ao solicitante que preencha o
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulario (ou copiar/colar en un problema). Capturar ao menos: identidade, método de contacto,
   Maneja GitHub, datos previstos de revisión y confirmación de que os docs de seguranca foram lidos.
2. Registre una solicitud sin rastreador `docs-preview` (emita GitHub o ticket de gobierno)
   e atribuir un aprobador.
3. Validar requisitos previos:
   - CLA / acuerdo de contribución en archivo (ou referencia de contrato de socio).
   - Reconhecimento de uso aceitavel armazenado na solicitacao.
   - Avaliacao de risco completa (por ejemplo, socios revisores aprovados pelo Legal).
4. O aprovador faz o sign-off na solicitacao e vincula a issues de tracking a qualquer entrada de
   gestión de cambios (ejemplo: `DOCS-SORA-Preview-####`).

## Aprovisionamiento y ferramentas

1. **Compartilhar artefatos** - Fornecer o descriptor + archivo de vista previa más reciente del flujo de trabajo
   de CI o pin SoraFS (artefato `docs-portal-preview`). Lembrar os revisores de ejecutar:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir com cumplimiento de checksum** - Apontar os revisores para o comando com gate de checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Isso reutiliza `scripts/serve-verified-preview.mjs` para que nenhum build nao verificado
   seja iniciado por accidente.3. **Conceder acceso a GitHub (opcional)** - Se revisores precisarem de sucursales nao publicadas,
   Adiciona-los ao time GitHub `docs-preview` durante a revisao e registra a mudanca de membresía
   na solicitaçao.

4. **Comunicar canais de suporte** - Compartir el contacto de guardia (Matrix/Slack) y el procedimiento
   de incidentes de [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetria + feedback** - Lembrar os revisores que analítica anonimizada y coletada
   (ver [`observability`](./observability.md)). Fornecer o formulario de comentarios o plantilla de problema
   citado no convite e registrador o evento com o ayudante
   [`preview-feedback-log`](./preview-feedback-log) para mantener el resumen de la onda actualizado.

## Lista de verificación del revisor

Antes de acceder a la vista previa, los revisores deben completar:

1. Verificar os artefactos bajos (`preview_verify.sh`).
2. Inicie el portal a través de `npm run serve` (o `serve:verified`) para garantizar que o guard de checksum está activo.
3. Ler as notas de seguranca e observabilidade vinculadas acima.
4. Pruebe una consola OAuth/Pruébelo usando el inicio de sesión con código de dispositivo (se aplica) y evite reutilizar tokens de producción.
5. Registrar achados no tracker acordado (issue, doc compartilhado ou formulario) e taguea-los com
   o etiqueta de lanzamiento hacer vista previa.

## Responsabilidades de mantenedores y offboarding| Fase | Acoes |
| --- | --- |
| Inicio | Confirmar que una lista de verificación de admisión está anexada a solicitud, compartir artefatos + instrucciones, agregar una entrada `invite-sent` vía [`preview-feedback-log`](./preview-feedback-log), y agendar una sincronización de meio período se a revisaro durará más de una semana. |
| Monitoreo | Monitorear telemetría de vista previa (procure trafego Try it incomum, falhas de probe) y seguir el runbook de incidentes se algo sospechoso ocorrer. Registrar eventos `feedback-submitted`/`issue-opened` conforme os achados chegarem para manter as metrics da onda precisas. |
| Baja de embarque | Revogar acesso temporario a GitHub o SoraFS, registrar `access-revoked`, archivar una solicitud (incluir resumo de feedback + acoes pendentes), y actualizar el registro de revisores. Solicitar al revisor que elimine las compilaciones locales y anexar el resumen generado a partir de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Utilice el mismo proceso para rotar revisores entre ondas. Manter o rastro no repo (edición + plantillas)
ajuda o DOCS-SORA a permanecer auditavel y permite que un gobierno confirme que o acceso de vista previa
siga los controles documentados.

## Plantillas de invitación y seguimiento- Inicia toda la divulgación con el archivo.
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  La captura del mínimo de idioma legal, instrucciones de suma de verificación de vista previa y una expectativa de que
  revisores reconhecam a politica de uso aceitavel.
- Para editar la plantilla, sustituye los marcadores de posición de `<preview_tag>`, `<request_ticket>` y canales de contacto.
  Guarde una copia del mensaje final sin ticket de admisión para que revisores, aprovadores y auditores puedan
  referenciar o texto exacto enviado.
- Después de enviar o invitar, actualizar un plan de seguimiento o emitir con la marca de tiempo `invite_sent_at` y un dato.
  de encerramento esperado para que o relatorio
  [vista previa del flujo de invitación](./preview-invite-flow.md) se puede identificar una cohorte automáticamente.