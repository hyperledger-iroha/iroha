---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: vista previa-comentarios-w1-plan
título: Plano de verificación previa de paquetes W1
etiqueta_barra lateral: Plano W1
descripción: Tarefas, responsaveis y checklist de evidencia para a coorte de preview de parceiros.
---

| Artículo | Detalles |
| --- | --- |
| Onda | W1 - Parceiros e integradores Torii |
| Janela alvo | Q2 2025 semana 3 |
| Etiqueta de artefato (planejado) | `preview-2025-04-12` |
| Problema con el rastreador | `DOCS-SORA-Preview-W1` |

## Objetivos

1. Garantía aprobada legalmente y de gobierno para los términos de vista previa de paquetes.
2. Preparar el proxy Pruébalo e instantáneas de telemetría usadas sin paquete de convite.
3. Actualizar el artefato de vista previa verificado por suma de comprobación y los resultados de las sondas.
4. Finalizar la lista de participantes y las plantillas de solicitud antes de enviar dos invitaciones.

##Desdobramento de tarefas| identificación | Tarefa | Responsavel | Prazo | Estado | Notas |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obter aprobación legal para o adendo dos termos de vista previa | Líder de Docs/DevRel -> Legal | 2025-04-05 | Concluido | Boleto legal `DOCS-SORA-Preview-W1-Legal` aprobado en 2025-04-05; PDF anexo al rastreador. |
| W1-P2 | Capturar janela de staging do proxy Pruébalo (2025-04-10) y validar a saude do proxy | Documentos/DevRel + Operaciones | 2025-04-06 | Concluido | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` ejecutado el 2025-04-06; transcripción de CLI y archivos `.env.tryit-proxy.bak`. |
| W1-P3 | Construir artefato de vista previa (`preview-2025-04-12`), rodar `scripts/preview_verify.sh` + `npm run probe:portal`, archivo descriptor/checksums | Portal TL | 2025-04-08 | Concluido | Artefato e logs de verificación armados em `artifacts/docs_preview/W1/preview-2025-04-12/`; Saida de sonda anexada ao tracker. |
| W1-P4 | Revisar formularios de ingesta de paquetes (`DOCS-SORA-Preview-REQ-P01...P08`), confirmar contactos y NDAs | Enlace de gobernanza | 2025-04-07 | Concluido | As oito solicitacoes aprovadas (as duas ultimas em 2025-04-11); aprovacoes linkadas no tracker. |
| W1-P5 | Redigir o convite (basado en `docs/examples/docs_preview_invite_template.md`), definir `<preview_tag>` e `<request_ticket>` para cada paquete | Líder de Docs/DevRel | 2025-04-08 | Concluido | Rascunho do convite enviado el 2025-04-12 15:00 UTC junto con enlaces de artefato. |

## Lista de verificación de verificación previa> Dice: montó `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` para ejecutar automáticamente los pasos 1-5 (compilación, verificación de suma de comprobación, sonda del portal, verificador de enlaces y actualización del proxy Pruébelo). El script registra un registro JSON que puede anexar al problema del rastreador.

1. `npm run build` (com `DOCS_RELEASE_TAG=preview-2025-04-12`) para regenerar `build/checksums.sha256` e `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` y archiva `build/link-report.json` al lado del descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (o pasa el objetivo adecuado a través de `--tryit-target`); confirme `.env.tryit-proxy` actualizado y guarde `.bak` para revertir.
6. Actualizar el problema W1 con caminos de registros (suma de verificación del descriptor, dijo la sonda, mudanca sin proxy Pruébelo e instantáneas Grafana).

## Lista de verificación de evidencia

- [x] Aprovacao legal assinada (PDF o enlace del ticket) anexada a `DOCS-SORA-Preview-W1`.
- [x] Capturas de pantalla de Grafana para `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor y registro de suma de comprobación de `preview-2025-04-12` armados en `artifacts/docs_preview/W1/`.
- [x] Tabela de roster de convites con `invite_sent_at` preenchido (ver log W1 sin tracker).
- [x] Artefatos de feedback reflejados en [`preview-feedback/w1/log.md`](./log.md) con una línea por parceiro (actualizado en 2025-04-26 con datos de roster/telemetria/issues).

Actualizar este plano conforme como tarefas avancarem; o tracker o referencia para manter o roadmap auditavel.

## Flujo de comentarios1. Para cada revisor, duplicar o plantilla em
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   Preencher os metadados y armazenar una copia completa em
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Resumir convites, checkpoints de telemetria e issues abertos dentro do log vivo em
   [`preview-feedback/w1/log.md`](./log.md) para que los revisores de gobierno possam rever toda a onda
   Sem sair do repositorio.
3. Quando os exports de Knowledge-Check ou Surveys chegarem, anexar no caminho de artefatos indicado no log
   e vincular o emitir el rastreador.