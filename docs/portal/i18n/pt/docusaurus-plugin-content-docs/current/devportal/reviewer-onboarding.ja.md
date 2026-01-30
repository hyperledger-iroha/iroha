---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/devportal/reviewer-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5f3428d34cb92b6e5e13752497f919cf4baf628f26d669b4ce399b86f37cc20a
source_last_modified: "2025-11-14T04:43:20.136105+00:00"
translation_last_reviewed: 2026-01-30
---

# Onboarding de revisores do preview

## Visao geral

DOCS-SORA acompanha um lancamento em fases do portal de desenvolvedores. Builds com gate de checksum
(`npm run serve`) e fluxos Try it reforcados destravam o proximo marco:
onboarding de revisores validados antes de o preview publico se abrir amplamente. Este guia
descreve como coletar solicitacoes, verificar elegibilidade, provisionar acesso e fazer offboarding
de participantes com seguranca. Consulte o
[preview invite flow](./preview-invite-flow.md) para o planejamento de coortes, a
cadencia de convites e exports de telemetria; os passos abaixo focam nas acoes
a tomar quando um revisor ja foi selecionado.

- **Escopo:** revisores que precisam de acesso ao preview de docs (`docs-preview.sora`,
  builds do GitHub Pages ou bundles de SoraFS) antes de GA.
- **Fora do escopo:** operadores de Torii ou SoraFS (cobertos por seus proprios kits de onboarding)
  e implantacoes do portal em producao (ver
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Papeis e prerequisitos

| Papel | Objetivos tipicos | Artefatos requeridos | Notas |
| --- | --- | --- | --- |
| Core maintainer | Verificar novos guias, executar smoke tests. | GitHub handle, contato Matrix, CLA assinada em arquivo. | Geralmente ja esta no time GitHub `docs-preview`; ainda assim registre uma solicitacao para que o acesso seja auditavel. |
| Partner reviewer | Validar snippets de SDK ou conteudo de governanca antes do release publico. | Email corporativo, POC legal, termos de preview assinados. | Deve reconhecer requisitos de telemetria + tratamento de dados. |
| Community volunteer | Fornecer feedback de usabilidade sobre guias. | GitHub handle, contato preferido, fuso horario, aceitacao do CoC. | Mantenha coortes pequenas; priorize revisores que assinaram o acordo de contribuicao. |

Todos os tipos de revisores devem:

1. Reconhecer a politica de uso aceitavel para artefatos de preview.
2. Ler os apendices de seguranca/observabilidade
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Concordar em executar `docs/portal/scripts/preview_verify.sh` antes de servir qualquer
   snapshot localmente.

## Fluxo de intake

1. Pedir ao solicitante que preencha o
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulario (ou copiar/colar em uma issue). Capturar ao menos: identidade, metodo de contato,
   GitHub handle, datas previstas de revisao e confirmacao de que os docs de seguranca foram lidos.
2. Registrar a solicitacao no tracker `docs-preview` (issue GitHub ou ticket de governanca)
   e atribuir um aprovador.
3. Validar prerequisitos:
   - CLA / acordo de contribuicao em arquivo (ou referencia de contrato partner).
   - Reconhecimento de uso aceitavel armazenado na solicitacao.
   - Avaliacao de risco completa (por exemplo, revisores partner aprovados pelo Legal).
4. O aprovador faz o sign-off na solicitacao e vincula a issue de tracking a qualquer entrada de
   change-management (exemplo: `DOCS-SORA-Preview-####`).

## Provisionamento e ferramentas

1. **Compartilhar artefatos** - Fornecer o descriptor + arquivo de preview mais recente do workflow
   de CI ou do pin SoraFS (artefato `docs-portal-preview`). Lembrar os revisores de executar:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir com enforcement de checksum** - Apontar os revisores para o comando com gate de checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Isso reutiliza `scripts/serve-verified-preview.mjs` para que nenhum build nao verificado
   seja iniciado por acidente.

3. **Conceder acesso GitHub (opcional)** - Se revisores precisarem de branches nao publicadas,
   adiciona-los ao time GitHub `docs-preview` durante a revisao e registrar a mudanca de membership
   na solicitacao.

4. **Comunicar canais de suporte** - Compartilhar o contato on-call (Matrix/Slack) e o procedimento
   de incidentes de [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetria + feedback** - Lembrar os revisores que analytics anonimizada e coletada
   (ver [`observability`](./observability.md)). Fornecer o formulario de feedback ou template de issue
   citado no convite e registrar o evento com o helper
   [`preview-feedback-log`](./preview-feedback-log) para manter o resumo da onda atualizado.

## Checklist do revisor

Antes de acessar o preview, revisores devem completar:

1. Verificar os artefatos baixados (`preview_verify.sh`).
2. Iniciar o portal via `npm run serve` (ou `serve:verified`) para garantir que o guard de checksum esta ativo.
3. Ler as notas de seguranca e observabilidade vinculadas acima.
4. Testar a console OAuth/Try it usando device-code login (se aplicavel) e evitar reutilizar tokens de producao.
5. Registrar achados no tracker acordado (issue, doc compartilhado ou formulario) e taguea-los com
   o tag de release do preview.

## Responsabilidades de maintainers e offboarding

| Fase | Acoes |
| --- | --- |
| Kickoff | Confirmar que a checklist de intake esta anexada a solicitacao, compartilhar artefatos + instrucoes, adicionar uma entrada `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), e agendar um sync de meio periodo se a revisao durar mais de uma semana. |
| Monitoring | Monitorar telemetria de preview (procure trafego Try it incomum, falhas de probe) e seguir o runbook de incidentes se algo suspeito ocorrer. Registrar eventos `feedback-submitted`/`issue-opened` conforme os achados chegarem para manter as metricas da onda precisas. |
| Offboarding | Revogar acesso temporario a GitHub ou SoraFS, registrar `access-revoked`, arquivar a solicitacao (incluir resumo de feedback + acoes pendentes), e atualizar o registro de revisores. Solicitar ao revisor que remova builds locais e anexar o digest gerado a partir de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Use o mesmo processo ao rotacionar revisores entre ondas. Manter o rastro no repo (issue + templates)
ajuda o DOCS-SORA a permanecer auditavel e permite que a governanca confirme que o acesso de preview
seguiu os controles documentados.

## Templates de convite e tracking

- Inicie todo outreach com o arquivo
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  Ele captura o minimo de linguagem legal, instrucoes de checksum de preview e a expectativa de que
  revisores reconhecam a politica de uso aceitavel.
- Ao editar o template, substitua os placeholders de `<preview_tag>`, `<request_ticket>` e canais de contato.
  Guarde uma copia da mensagem final no ticket de intake para que revisores, aprovadores e auditores possam
  referenciar o texto exato enviado.
- Depois de enviar o convite, atualize a planilha de tracking ou issue com o timestamp `invite_sent_at` e a data
  de encerramento esperada para que o relatorio
  [preview invite flow](./preview-invite-flow.md) possa identificar a coorte automaticamente.
