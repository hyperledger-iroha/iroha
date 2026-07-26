---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Manual de convites da pré-visualização pública

## Objetivos do programa

Este manual explica como anunciar e executar a visualização pública uma vez que o
fluxo de trabalho de integração de revisores está ativo. Mantenha a honestidade do roteiro DOCS-SORA al
certifique-se de que cada convite seja enviado com artefatos verificáveis, guia de segurança e um
caminho claro de feedback.

- **Público:** lista com curadoria de membros da comunidade, parceiros e mantenedores que
  firmar a política de uso aceitável da visualização.
- **Limites:** tamano de ola por defeito <= 25 revisores, ventana de acesso de 14 dias, resposta
  incidentes em 24h.

## Checklist de portão de lançamento

Complete estas tarefas antes de enviar qualquer convite:

1. Últimos artefatos de visualização carregados em CI (`docs-portal-preview`,
   manifesto de checksum, descritor, pacote SoraFS).
2. `npm run --prefix docs/portal serve` (gateado por checksum) verificado na mesma tag.
3. Tickets de onboarding de revisores aprovados e incluídos na janela de convites.
4. Documentos de segurança, observação e incidentes validados
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulário de feedback ou planta de emissão qualificada (incluye campos de severidade,
   passos de reprodução, capturas de tela e informações de ambiente).
6. Texto do anúncio revisado por Docs/DevRel + Governance.

## Pacote de convite

Cada convite deve incluir:

1. **Artefatos selecionados** - Proporciona links ao manifesto/plano de SoraFS ou a los
   artefatos do GitHub, mas o manifesto da soma de verificação e o descritor. Referência do comando
   de verificação explicitamente para que os revisores possam executá-lo antes de levantar
   o local.
2. **Instruções de serviço** - Inclui o comando de visualização gateado por checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Registros de segurança** - Indica que os tokens expiram automaticamente, os links
   não devemos compartilhar e os incidentes devem ser relatados imediatamente.
4. **Canal de feedback** - Enlaza la plantilla/formulario e esclareça expectativas de tempo de resposta.
5. **Fechas do programa** - Proporciona datas de início/fim, horário comercial ou sincronizações, e a proximidade
   janela de atualização.

O e-mail de exibição em
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
preencha esses requisitos. Atualizar placeholders (fechas, URLs, contatos)
antes de enviar.

## Expor o host de visualização

Apenas promova o host de visualização uma vez que a integração esteja completa e o ticket de mudança
está aprovado. Consulte o [guia de exposição do host de visualização](./preview-host-exposure.md)
para os passos de ponta a ponta de construir/publicar/verificar usados nesta seção.

1. **Construir e empaquetado:** Marcar a etiqueta de lançamento e produzir artefatos deterministas.

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
   ```

   O script do pino descreve `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   e `portal.dns-cutover.json` abaixo de `artifacts/sorafs/`. Adjunta esses arquivos à la ola de
   convites para que cada revisor possa verificar os bits errados.

2. **Publicar o alias de visualização:** Repetir o comando sin `--skip-submit`
   (proporcional `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, e o teste de alias emitido
   por governo). O script enlazara o manifesto a `docs-preview.sora` e emitirá
   `portal.manifest.submit.summary.json` mas `portal.pin.report.json` para o pacote de evidências.

3. **Provar o despliegue:** Confirme que o alias foi resolvido e que a soma de verificação coincide com a tag
   antes de enviar convites.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Manter `npm run serve` (`scripts/serve-verified-preview.mjs`) a mão como substituto para
   que os revisores podem levantar uma cópia local se a borda da visualização falhar.

## Linha do tempo de comunicações

| Diâmetro | Ação | Proprietário |
| --- | --- | --- |
| D-3 | Finalizar cópia do convite, atualizar artefatos, simulação de verificação | Documentos/DevRel |
| D-2 | Assinatura de governo + bilhete de mudança | Documentos/DevRel + Governança |
| D-1 | Enviar convites usando a planta, atualizar rastreador com lista de destinos | Documentos/DevRel |
| D | Chamada de kickoff/horário de atendimento, monitoramento de dashboards de telemetria | Documentos/DevRel + plantão |
| D+7 | Digest de feedback de metade de ola, triagem de problemas bloqueadores | Documentos/DevRel |
| D+14 | Cerrar ola, revogar acesso temporal, publicar currículo em `status.md` | Documentos/DevRel |

## Acompanhamento de acesso e telemetria1. Registre cada destinatário, carimbo de data e hora do convite e data de revogação com o
   visualizar o registrador de feedback (ver
   [`preview-feedback-log`](./preview-feedback-log)) para que cada uma compartilhe o mesmo
   rastro de evidência:

   ```bash
   # Agrega un nuevo evento de invitacion a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Os eventos suportados são `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened`, e `access-revoked`. El log vive en
   `artifacts/docs_portal_preview/feedback_log.json` por defeito; adicional ao ticket de
   a ola de convites junto com os formulários de consentimento. Use o ajudante de
   resumo para produzir um currículo auditável antes da nota de cierre:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   O resumo JSON enumera convites por ola, destinos abertos, conteúdos de
   feedback e o carimbo de data/hora do evento mais recente. El helper esta respaldado por
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Assim, o mesmo fluxo de trabalho pode ser executado localmente ou em CI. Use a planta de digestão em
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   ao publicar a recapitulação da ola.
2. Etiqueta os painéis de telemetria com o `DOCS_RELEASE_TAG` usado para a tela para que
   os picos podem ser correlacionados com as coortes de convite.
3. Execute `npm run probe:portal -- --expect-release=<tag>` após a implantação para confirmar
   que o ambiente de visualização anuncia os metadados corretos de lançamento.
4. Registre qualquer incidente na planta do runbook e coloque-o na coorte.

## Feedback e fechamento

1. Agregar feedback em um documento compartilhado ou tabela de problemas. Itens de etiqueta com
   `docs-preview/<wave>` para que os proprietários do roteiro possam consultá-los facilmente.
2. Use o resumo da saída do registrador de visualização para obter o relatório da ola, depois retomar
   la cohorte en `status.md` (participantes, hallazgos principais, correções planejadas) e
   atualiza `roadmap.md` se a mudança DOCS-SORA.
3. Siga os passos de desligamento de
   [`reviewer-onboarding`](./reviewer-onboarding.md): acesso revogado, solicitações de arquivo e
   agradecer aos participantes.
4. Prepare o seguinte ou refrescando artefatos, reexecutando os portões de checksum e
   atualizando a planta de convite com novas datas.

Aplique este manual de forma consistente para manter o programa de visualização auditável e
le do Docs/DevRel é uma forma repetitiva de escalar convites na medida em que o portal se
sobre um GA.