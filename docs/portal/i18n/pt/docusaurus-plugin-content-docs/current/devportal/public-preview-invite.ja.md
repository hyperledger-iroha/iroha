---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/devportal/public-preview-invite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c507adb7745ab5e4930a9d693293529a24e604a8a6a1bc150ec8b05ab44b80a1
source_last_modified: "2025-11-14T04:43:20.114289+00:00"
translation_last_reviewed: 2026-01-30
---

# Playbook de convites do preview publico

## Objetivos do programa

Este playbook explica como anunciar e executar o preview publico assim que o
workflow de onboarding de revisores estiver ativo. Ele mantem o roadmap DOCS-SORA honesto ao
garantir que cada convite saia com artefatos verificaveis, orientacao de seguranca e um
caminho claro de feedback.

- **Audiencia:** lista curada de membros da comunidade, partners e maintainers que assinaram a
  politica de uso aceitavel do preview.
- **Limites:** tamanho de onda padrao <= 25 revisores, janela de acesso de 14 dias, resposta
  a incidentes em 24h.

## Checklist de gate de lancamento

Complete estas tarefas antes de enviar qualquer convite:

1. Ultimos artefatos de preview enviados na CI (`docs-portal-preview`,
   manifest de checksum, descriptor, bundle SoraFS).
2. `npm run --prefix docs/portal serve` (gateado por checksum) testado no mesmo tag.
3. Tickets de onboarding de revisores aprovados e vinculados a onda de convites.
4. Docs de seguranca, observabilidade e incidentes validados
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulario de feedback ou template de issue preparado (inclua campos de severidade,
   passos de reproducao, screenshots e info de ambiente).
6. Copy do anuncio revisada por Docs/DevRel + Governance.

## Pacote de convite

Cada convite deve incluir:

1. **Artefatos verificados** - Forneca links para o manifest/plan SoraFS ou para os artefatos
   GitHub, mais o manifest de checksum e o descriptor. Referencie explicitamente o comando
   de verificacao para que os revisores possam executa-lo antes de subir o site.
2. **Instrucoes de serve** - Inclua o comando de preview gateado por checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Lembretes de seguranca** - Informe que tokens expiram automaticamente, links nao devem ser
   compartilhados e incidentes devem ser reportados imediatamente.
4. **Canal de feedback** - Linke o template/formulario e esclareca expectativas de tempo de resposta.
5. **Datas do programa** - Informe datas de inicio/fim, office hours ou syncs, e a proxima
   janela de refresh.

O email de exemplo em
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
cobre esses requisitos. Atualize os placeholders (datas, URLs, contatos)
antes de enviar.

## Expor o host de preview

So promova o host de preview quando o onboarding estiver completo e o ticket de mudanca estiver
aprovado. Veja o [guia de exposicao do host de preview](./preview-host-exposure.md) para os passos
end-to-end de build/publish/verify usados nesta secao.

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

   O script de pin grava `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   e `portal.dns-cutover.json` em `artifacts/sorafs/`. Anexe esses arquivos a onda de convites
   para que cada revisor possa verificar os mesmos bits.

2. **Publicar o alias de preview:** Rode o comando sem `--skip-submit`
   (forneca `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, e a prova de alias emitida
   pela governanca). O script vai amarrar o manifest a `docs-preview.sora` e emitir
   `portal.manifest.submit.summary.json` mais `portal.pin.report.json` para o bundle de evidencias.

3. **Testar o deployment:** Confirme que o alias resolve e que o checksum corresponde ao tag
   antes de enviar convites.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantenha `npm run serve` (`scripts/serve-verified-preview.mjs`) a mao como fallback para
   que revisores possam subir uma copia local se o edge de preview falhar.

## Timeline de comunicacao

| Dia | Acao | Owner |
| --- | --- | --- |
| D-3 | Finalizar copy do convite, atualizar artefatos, dry-run de verificacao | Docs/DevRel |
| D-2 | Sign-off de governanca + ticket de mudanca | Docs/DevRel + Governance |
| D-1 | Enviar convites usando o template, atualizar tracker com lista de destinatarios | Docs/DevRel |
| D | Kickoff call / office hours, monitorar dashboards de telemetria | Docs/DevRel + On-call |
| D+7 | Digest de feedback no meio da onda, triage de issues bloqueantes | Docs/DevRel |
| D+14 | Fechar a onda, revogar acesso temporario, publicar resumo em `status.md` | Docs/DevRel |

## Tracking de acesso e telemetria

1. Registre cada destinatario, timestamp de convite e data de revogacao com o
   preview feedback logger (veja
   [`preview-feedback-log`](./preview-feedback-log)) para que cada onda compartilhe o mesmo
   rastro de evidencias:

   ```bash
   # Adicione um novo evento de convite a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Os eventos suportados sao `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened`, e `access-revoked`. O log fica em
   `artifacts/docs_portal_preview/feedback_log.json` por padrao; anexe ao ticket da
   onda de convites junto com os formularios de consentimento. Use o helper de
   summary para produzir um roll-up auditavel antes da nota de encerramento:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   O summary JSON enumera convites por onda, destinatarios abertos, contagens de
   feedback e o timestamp do evento mais recente. O helper e apoiado por
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   portanto o mesmo workflow pode rodar localmente ou em CI. Use o template de digest em
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   ao publicar o recap da onda.
2. Tague os dashboards de telemetria com o `DOCS_RELEASE_TAG` usado na onda para que
   picos possam ser correlacionados com as coortes de convite.
3. Rode `npm run probe:portal -- --expect-release=<tag>` apos o deploy para confirmar que
   o ambiente de preview anuncia a metadata correta de release.
4. Registre qualquer incidente no template do runbook e vincule a coorte.

## Feedback e fechamento

1. Agregue feedback em um doc compartilhado ou board de issues. Marque os itens com
   `docs-preview/<wave>` para que os owners do roadmap encontrem facilmente.
2. Use a saida summary do preview logger para preencher o relatorio da onda, depois
   resuma a coorte em `status.md` (participantes, principais achados, fixes planejados) e
   atualize `roadmap.md` se o marco DOCS-SORA mudou.
3. Siga os passos de offboarding de
   [`reviewer-onboarding`](./reviewer-onboarding.md): revogue acesso, arquive solicitacoes e
   agradeca os participantes.
4. Prepare a proxima onda atualizando artefatos, reexecutando os gates de checksum e
   atualizando o template de convite com novas datas.

Aplicar este playbook de forma consistente mantem o programa de preview auditable e
oferece ao Docs/DevRel um caminho repetivel para escalar convites a medida que o portal
se aproxima de GA.
