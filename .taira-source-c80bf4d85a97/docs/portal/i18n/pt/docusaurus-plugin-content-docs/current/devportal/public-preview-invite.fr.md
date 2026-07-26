---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Visualização pública do Playbook d'invitation

## Objetivos do programa

Este manual explícito comenta o anúncio e faz com que a visualização pública seja feita uma vez que ele
o fluxo de trabalho de integração dos revisores está ativo. Guarde o roteiro DOCS-SORA honrado em
tenho certeza de que cada convite parte com artefatos verificáveis, remessas de segurança
e um caminho claro para feedback.

- **Público:** lista curada de membros da comunidade, parceiros e mantenedores aqui
  assine a política de uso aceitável na visualização.
- **Plafonds:** taille de vague par defaut <= 25 revisores, acesso de 14 dias,
  resposta a incidentes sous 24 h.

## Checklist de portão de lancemento

Termine estes taches antes de enviar um convite:

1. Derniers artefatos de pré-visualização de cobranças em CI (`docs-portal-preview`,
   manifesto de checksum, descritor, pacote SoraFS).
2. `npm run --prefix docs/portal serve` (gate par checksum) teste na tag meme.
3. Os tickets de integração dos revisores são aprovados e ficam na vaga de convite.
4. Documentos de segurança, observabilidade e incidentes válidos
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulário de feedback ou modelo de preparação de problemas (inclui campos de gravidade,
   etapas de reprodução, capturas de tela e informações ambientais).
6. Cópia do anúncio revisado por Docs/DevRel + Governance.

## Pacote de convite

Cada convite inclui:

1. **Verificações de artefatos** - Forneça garantias sobre o manifesto/plano SoraFS ou os artefatos
   GitHub, além do manifesto de soma de verificação e do descritor. Referenciar explicitamente o comando
   de verificação para que os revisores possam executar o executor antes de lançar o site.
2. **Instruções de serviço** - Inclua o comando preview gatee par checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Rappels securite** - Indica que os tokens expiram automaticamente, que as garantias
   ne doivent pas etre partages, et que les incidents doivent etre signales imediatamente.
4. **Canal de feedback** - Libere o modelo/formulário e esclareça os tempos de resposta.
5. **Datas do programa** - Informe as datas de estreia/fim, horário comercial ou sincronizações, et la prochaine
   janela de atualização.

O exemplo de e-mail em
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
couvre ces exigências. Coloque marcadores de posição no dia (datas, URLs, contatos)
antes do envio.

## Exposer l'hote preview

Não promova a pré-visualização do hotel quando o embarque terminar e o bilhete de alteração for aprovado.
Veja o [guia de exposição do hotel preview](./preview-host-exposure.md) para etapas de ponta a ponta
O build/publish/verify é usado nesta seção.

1. **Construir e embalar:** Marque a etiqueta de liberação e produza os artefatos determinados.

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

   O script de pino escrito `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   e `portal.dns-cutover.json` sob `artifacts/sorafs/`. Joindre ces fichiers a la vague
   um convite para que cada revisor possa verificar os bits dos memes.

2. **Publicar a visualização do alias:** Relancer la commande sans `--skip-submit`
   (quadro `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, e o teste de alias emise
   pela governança). O script é manifestado em `docs-preview.sora` e emet
   `portal.manifest.submit.summary.json` mais `portal.pin.report.json` para o pacote de testes.

3. **Teste a implantação:** Confirme se o alias foi retornado e se a soma de verificação corresponde à tag
   antes de enviar os convites.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Garder `npm run serve` (`scripts/serve-verified-preview.mjs`) sob a principal como substituto para
   que os revisores podem lançar uma cópia local se a visualização da borda estiver flanqueada.

## Cronograma de comunicação

| Jornada | Ação | Proprietário |
| --- | --- | --- |
| D-3 | Finalize a cópia do convite, rafraichir os artefatos, teste de verificação | Documentos/DevRel |
| D-2 | Aprovação de governo + ticket de changement | Documentos/DevRel + Governança |
| D-1 | Envie os convites por meio do modelo, coloque o rastreador no dia com a lista de destinos | Documentos/DevRel |
| D | Chamada inicial / horário comercial, monitore os painéis de telemetria | Documentos/DevRel + plantão |
| D+7 | Resumo de feedback um pouco vago, triagem de problemas bloqueados | Documentos/DevRel |
| D+14 | Fechar a vaga, revogar o acesso temporário, publicar um currículo em `status.md` | Documentos/DevRel |

## Sugerindo acesso e telemetria1. Registre cada destino, carimbo de data e hora do convite e data de revogação com ele
   visualizar o registrador de feedback (ver
   [`preview-feedback-log`](./preview-feedback-log)) para que cada vaga parte do meme
   trace de preuves:

   ```bash
   # Ajouter un nouvel evenement d'invitation a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Os eventos com preço de custo são `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened` e `access-revoked`. Le log se encontrou um
   `artifacts/docs_portal_preview/feedback_log.json` por padrão; joignez-le au ticket de
   vago d'invitation avec les formulaires de consentement. Use o ajudante de resumo
   para produzir um roll-up auditável antes da nota de coagulação:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   O resumo JSON enumera os convites por vaga, os destinos abertos, os
   controles de feedback e carimbo de data / hora do último evento. L'helper repouso em
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Donc le meme workflow pode ser localizado ou em CI. Utilize o modelo de resumo
   em [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   durante a publicação da recapitulação da vaga.
2. Marque os painéis de telemetria com o `DOCS_RELEASE_TAG` para utilizá-los para obter informações precisas
   as fotos podem ser correlacionadas com coortes de convite.
3. Executor `npm run probe:portal -- --expect-release=<tag>` após a implantação para confirmar que
   A visualização do ambiente anuncia os bons metadados de lançamento.
4. Remeter todos os incidentes no modelo do runbook e ligá-los à coorte.

## Feedback e coagulação

1. Adicione comentários em um documento ou quadro de problemas. Marcar os itens com
   `docs-preview/<wave>` para que os proprietários do roteiro possam recuperá-los facilmente.
2. Use o resumo de classificação do registrador de visualização para completar o relacionamento vago e depois
   resumir a coorte em `status.md` (participantes, principais estatísticas, correções anteriores) et
   coloque um dia `roadmap.md` se o jalon DOCS-SORA for alterado.
3. Siga as etapas de desligamento a partir de então
   [`reviewer-onboarding`](./reviewer-onboarding.md): revogar o acesso, arquivar as demandas e
   remercier les participantes.
4. Prepare a cadeia vaga e rafraique os artefatos e relacione as portas da soma de verificação
   e um novo modelo de convite com novas datas.

Aplique este playbook de facon consistente garde le program preview auditable et donne a
Docs/DevRel é mais repetível para fazer grandes convites, na medida em que o portal se aproxima do GA.