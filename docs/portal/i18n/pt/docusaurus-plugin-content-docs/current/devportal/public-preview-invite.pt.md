---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Playbook de convites do preview público

## Objetivos do programa

Este manual explica como anunciar e executar a visualização pública assim que o
fluxo de trabalho de onboarding de revisores está ativo. Ele mantém o roadmap DOCS-SORA honesto ao
garanta que cada convite saia com artefactos verificáveis, orientação de segurança e um
caminho claro de feedback.

- **Público:** lista com curadoria de membros da comunidade, parceiros e mantenedores que colaboraram a
  política de uso aceitavel do preview.
- **Limites:** tamanho de onda padrão <= 25 revisores, janela de acesso de 14 dias, resposta
  incidentes em 24h.

## Checklist do portão de lançamento

Conclua estas tarefas antes de enviar qualquer convite:

1. Últimos artistas de visualização enviados no CI (`docs-portal-preview`,
   manifesto de checksum, descritor, pacote SoraFS).
2. `npm run --prefix docs/portal serve` (gateado por checksum) testado na mesma tag.
3. Tickets de onboarding de revisores aprovados e termos a onda de convites.
4. Documentos de segurança, observabilidade e incidentes validados
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulário de feedback ou modelo de emissão preparado (inclui campos de severidade,
   passos de reprodução, capturas de tela e informações de ambiente).
6. Cópia do anúncio revisado por Docs/DevRel + Governance.

## Pacote de convite

Cada convite deve incluir:

1. **Artefatos selecionados** - Forneca links para o manifesto/plano SoraFS ou para os artistas
   GitHub, mais o manifesto de checksum e o descritor. Referência explícita ao comando
   de verificação para que os revisores possam executá-lo antes de subir o site.
2. **Instruções de serviço** - Inclui o comando de visualização gateado por checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Lembretes de segurança** - Informe que os tokens expiram automaticamente, links não devem ser
   compartilhados e incidentes devem ser reportados imediatamente.
4. **Canal de feedback** - Link para o modelo/formulário e esclareça expectativas de tempo de resposta.
5. **Datas do programa** - Informe dados de início/fim, horário comercial ou sincronizações, e a próxima
   janela de atualização.

O e-mail de exemplo em
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
cobre esses requisitos. Atualizar os placeholders (dados, URLs, contatos)
antes de enviar.

## Exportar o host de visualização

Então promova o host de preview quando o onboarding estiver completo e o ticket de mudança estiver
aprovado. Veja o [guia de exposição do host de preview](./preview-host-exposure.md) para os passos
ponta a ponta de construir/publicar/verificar usados nesta estação.

1. **Build e empacotamento:** Marque o release tag e produza artefatos deterministas.

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

   O script de pino gravado `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   e `portal.dns-cutover.json` em `artifacts/sorafs/`. Anexe esses arquivos a onda de convites
   para que cada revisor possa verificar os mesmos bits.

2. **Publicar o alias de visualização:** Rode o comando sem `--skip-submit`
   (forneca `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, e uma prova de alias emitida
   pela governança). O script vai amarrar o manifesto a `docs-preview.sora` e emitir
   `portal.manifest.submit.summary.json` mais `portal.pin.report.json` para o pacote de evidências.

3. **Testar o deploy:** Confirme que o alias resolve e que o checksum corresponde à tag
   antes de enviar convites.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantenha `npm run serve` (`scripts/serve-verified-preview.mjs`) a mão como substituto para
   que os revisores podem subir uma cópia local se a borda da visualização falhar.

## Timeline de comunicação

| Diâmetro | Ação | Proprietário |
| --- | --- | --- |
| D-3 | Finalizar cópia do convite, atualizar artefatos, simulação de verificação | Documentos/DevRel |
| D-2 | Sign-off de governança + ticket de mudança | Documentos/DevRel + Governança |
| D-1 | Enviar convites usando o template, atualizar tracker com lista de destinatários | Documentos/DevRel |
| D | Chamada de kickoff / horário comercial, monitorar dashboards de telemetria | Documentos/DevRel + plantão |
| D+7 | Digest de feedback no meio da onda, triagem de problemas bloqueadores | Documentos/DevRel |
| D+14 | Fechar a onda, revogar o acesso temporário, publicar resumo em `status.md` | Documentos/DevRel |

## Rastreamento de acesso e telemetria

1. Cadastre cada destinatário, carimbo de data/hora do convite e dados de revogação com o
   visualizar o registrador de feedback (veja
   [`preview-feedback-log`](./preview-feedback-log)) para que cada onda compartilhe o mesmo
   rastro de evidências:

   ```bash
   # Adicione um novo evento de convite a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```Os eventos suportados são `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened`, e `access-revoked`. O log fica em
   `artifacts/docs_portal_preview/feedback_log.json` por padrão; anexo ao ticket da
   onda de convites junto com os formulários de consentimento. Use o ajudante de
   resumo para produzir um roll-up auditável antes da nota de encerramento:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   O resumo JSON enumera convites por onda, destinos abertos, contagens de
   feedback e o timestamp do evento mais recente. Ó ajudante e apoiado por
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   portanto o mesmo fluxo de trabalho pode rodar localmente ou em CI. Use o modelo de resumo em
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   ao publicar o resumo da onda.
2. Tague os dashboards de telemetria com o `DOCS_RELEASE_TAG` usado na onda para que
   picos podem ser correlacionados com as coortes de convite.
3. Rode `npm run probe:portal -- --expect-release=<tag>` após o deploy para confirmar que
   o ambiente de visualização anuncia os metadados corretos de lançamento.
4. Registre qualquer incidente no modelo do runbook e vincule-se a coorte.

## Feedback e fechamento

1. Adicione feedback em um documento compartilhado ou quadro de problemas. Marque os itens com
   `docs-preview/<wave>` para que os proprietários do roadmap os encontrem facilmente.
2. Use um resumo do preview logger para preencher o relatorio da onda, depois
   resumo a coorte em `status.md` (participantes, principais achados, correções planejadas) e
   atualize `roadmap.md` se o marco DOCS-SORA mudou.
3. Siga os passos de desligamento de
   [`reviewer-onboarding`](./reviewer-onboarding.md): acesso revogado, solicitações de arquivo e
   agradeca os participantes.
4. Prepare uma próxima onda atualizando artefatos, reexecutando as portas de checksum e
   atualizando o template de convite com novos dados.

Aplique este playbook de forma consistente mantem o programa de preview auditável e
oferece ao Docs/DevRel um caminho repetivel para escalar convites à medida que o portal
se aproxima de GA.