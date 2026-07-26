---
lang: es
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Playbook de invitaciones para vista previa pública

## Objetivos del programa

Este playbook explica como anunciar e ejecutar o previa publico asim que o
flujo de trabajo de incorporación de revisores estiver ativo. El mantenimiento de la hoja de ruta DOCS-SORA honesto
garantir que cada convite saia com artefatos verificaveis, orientacao de seguranca e um
camino claro de retroalimentación.

- **Audiencia:** lista curada de miembros de la comunidad, socios y mantenedores que assinaram a
  política de uso aceitavel do vista previa.
- **Limitaciones:** tamanho de onda padrao <= 25 revisores, janela de acesso de 14 días, respuesta
  a incidentes en 24h.

## Lista de verificación de puerta de lancamento

Complete estas tarefas antes de enviar cualquier convite:

1. Últimos artefactos de vista previa enviados na CI (`docs-portal-preview`,
   manifiesto de suma de comprobación, descriptor, paquete SoraFS).
2. `npm run --prefix docs/portal serve` (gateado por checksum) probado sin etiqueta mesmo.
3. Tickets de onboarding de revisores aprovados e vinculados a onda de convites.
4. Docs de seguranca, observabilidade e incidentes validados
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulario de retroalimentación o plantilla de problema preparado (incluye campos de gravedad,
   pasos de reproducción, capturas de pantalla e información de ambiente).
6. Copiar el anuncio revisado por Docs/DevRel + Governance.

##Pacote de conviteCada invitación debe incluir:

1. **Artefatos verificados** - Forneca links para o manifest/plan SoraFS o para os artefatos
   GitHub, más el manifiesto de suma de comprobación y el descriptor. Referencia explícita al comando
   de verificación para que los revisores puedan ejecutarlo antes de subir al sitio.
2. **Instrucciones de servicio** - Incluye el comando de vista previa activado por checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Lembretes de seguranca** - Informe que los tokens caducan automáticamente, los enlaces nao devem ser
   compartilhados e incidentes deben ser reportados inmediatamente.
4. **Canal de retroalimentación** - Linke o template/formulario e esclareca expectativas de tempo de respuesta.
5. **Datas do programa** - Informar datos de inicio/fim, horarios de oficina o sincronizaciones, y aproximaciones
   janela de refrescar.

O correo electrónico de ejemplo em
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
cobre eses requisitos. Actualizar los marcadores de posición del sistema operativo (datos, URL, contactos)
antes de enviar.

## Exportar o host de vista previa

Entonces promova el host de vista previa cuando el onboarding estiver completo y el ticket de mudanca estiver
aprobado. Veja o [guia de exposición del host de vista previa](./preview-host-exposure.md) para los pasos
de extremo a extremo de construir/publicar/verificar usados nesta secao.

1. **Construir y empacotamento:** Marcar la etiqueta de lanzamiento y producir artefatos deterministas.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```El script de pin grava `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   y `portal.dns-cutover.json` en `artifacts/sorafs/`. Anexe esses arquivos a onda de convites
   para que cada revisor pueda verificar os mesmos bits.

2. **Publicar o alias de vista previa:** Rode o comando sem `--skip-submit`
   (forneca `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, y una prueba de alias emitida
   pela gobernancia). O script vai amarrar o manifest a `docs-preview.sora` y emitir
   `portal.manifest.submit.summary.json` más `portal.pin.report.json` para el paquete de evidencias.

3. **Probar la implementación:** Confirme que el alias se resuelva y que la suma de comprobación corresponda a la etiqueta
   antes de enviar invitaciones.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantenha `npm run serve` (`scripts/serve-verified-preview.mjs`) a mao como respaldo para
   que los revisores pueden subir una copia local se o borde de vista previa falhar.

## Cronología de comunicación| Día | Acao | Propietario |
| --- | --- | --- |
| D-3 | Finalizar copia del convite, actualizar artefatos, simulacro de verificacao | Documentos/DevRel |
| D-2 | Aprobación de gobernancia + ticket de mudanza | Documentos/DevRel + Gobernanza |
| D-1 | Enviar invitaciones usando una plantilla, actualizar tracker con lista de destinatarios | Documentos/DevRel |
| D | Llamada inicial / horario de oficina, monitorar paneles de telemetria | Documentos/DevRel + De guardia |
| D+7 | Resumen de comentarios no meio da onda, clasificación de problemas bloqueantes | Documentos/DevRel |
| D+14 | Fechar a onda, revogar acesso temporario, publicar resumen en `status.md` | Documentos/DevRel |

## Seguimiento de acceso y telemetría

1. Registre cada destinatario, marca de tiempo de convite e data de revogacao com o
   registrador de comentarios de vista previa (veja
   [`preview-feedback-log`](./preview-feedback-log)) para que cada onda comparta el mesmo
   rastro de evidencias:

   ```bash
   # Adicione um novo evento de convite a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Los eventos soportados sao `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened`, e `access-revoked`. O log fica em
   `artifacts/docs_portal_preview/feedback_log.json` por padrao; anexo ao ticket da
   onda de invitaciones junto con los formularios de consentimiento. Utilice o ayudante de
   Resumen para producir un roll-up auditavel antes de la nota de encerramento:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```El resumen JSON enumera invitaciones por onda, destinatarios abiertos, contagios de
   comentarios y marca de tiempo del evento más reciente. O ayudante e apoiado por
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Portanto o mesmo flujo de trabajo se puede rodar localmente o en CI. Utilice la plantilla de resumen em
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   ao publicar o resumen de la onda.
2. Tague os paneles de telemetría con o `DOCS_RELEASE_TAG` usado na onda para que
   picos possam ser correlacionados con coortes de convite.
3. Rode `npm run probe:portal -- --expect-release=<tag>` apos o implementar para confirmar que
   o ambiente de vista previa anuncia a metadata correta de lanzamiento.
4. Registre cualquier incidente sin plantilla del runbook y vincule a coorte.

## Comentarios y fechamento

1. Agregue comentarios en un documento compartido o en el tablero de problemas. Marque os itens com
   `docs-preview/<wave>` para que los propietarios encuentren fácilmente la hoja de ruta.
2. Utilice un resumen dicho del registrador de vista previa para preencher o relatorio da onda, después
   resuma a coorte em `status.md` (participantes, principais achados, fixes planejados) e
   atualize `roadmap.md` se o marco DOCS-SORA mudou.
3. Sigue los pasos de offboarding de
   [`reviewer-onboarding`](./reviewer-onboarding.md): revogue acesso, archiva solicitacoes e
   agradeca a los participantes.
4. Prepare una próxima onda atualizando artefatos, reejecutando os gates de checksum e
   actualizando la plantilla de invitación con nuevos datos.Aplique este playbook de forma consistente mantem o programa de vista previa auditable e
Oferece ao Docs/DevRel un camino repetitivo para escalar invita a medida que el portal.
se aproxima a GA.