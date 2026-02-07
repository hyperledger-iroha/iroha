---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Onboarding de revisores de visualização

## Visão geral

DOCS-SORA acompanha um lançamento em fases do portal de desenvolvedores. Constrói com gate de checksum
(`npm run serve`) e fluxos Try it reforcados destravam o próximo marco:
onboarding de revisores validados antes de a visualização pública se abrir amplamente. Este guia
descreve como cobrar solicitações, verificar elegibilidade, provisionar acesso e fazer offboarding
de participantes com segurança. Consulte o
[visualizar fluxo de convite](./preview-invite-flow.md) para o planejamento de coortes, a
cadência de convites e exportações de telemetria; os passos abaixo focam nas ações
a tomar quando um revisor já foi selecionado.

- **Escopo:** revisores que precisam de acesso ao preview de documentos (`docs-preview.sora`,
  builds do GitHub Pages ou pacotes de SoraFS) antes de GA.
- **Fora do escopo:** operadoras de Torii ou SoraFS (cobertos por seus próprios kits de onboarding)
  e implantações do portal em produção (ver
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Papeis e pré-requisitos

| Papel | Objetivos típicos | Artefatos necessários | Notas |
| --- | --- | --- | --- |
| Mantenedor principal | Verifique novas orientações, execute testes de fumaça. | Identificador GitHub, contato Matrix, CLA assinado em arquivo. | Geralmente já está no momento GitHub `docs-preview`; ainda assim registre uma solicitação para que o acesso seja auditável. |
| Revisor parceiro | Valide trechos do SDK ou conteúdo de governança antes do lançamento público. | Email corporativo, POC legal, termos de pré-visualização assinados. | Deve considerar requisitos de telemetria + tratamento de dados. |
| Voluntário comunitário | Fornecer feedback de usabilidade sobre guias. | Identificador GitHub, contato preferido, fuso horário, aceitação do CoC. | Mantenha coortes pequenas; priorize revisores que concordaram com o acordo de contribuição. |

Todos os tipos de revisores devem:

1. Reconhecer a política de uso aceitavel para artistas de visualização.
2. Ler os apêndices de segurança/observabilidade
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Concordar em executar `docs/portal/scripts/preview_verify.sh` antes de servir qualquer
   instantâneo localmente.

## Fluxo de ingestão

1. Peça ao solicitante que preencha o
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulário (ou copiar/colar em uma edição). Capturar ao menos: identidade, método de contato,
   Identificador do GitHub, dados previstos de revisão e confirmação de que os documentos de segurança foram lidos.
2. Registrar uma solicitação no tracker `docs-preview` (emitir GitHub ou ticket de governança)
   e conceda um aprovador.
3. Validar pré-requisitos:
   - CLA / acordo de contribuição em arquivo (ou referência de contrato parceiro).
   - Reconhecimento de uso aceitavel armazenado na solicitação.
   - Avaliação de risco completa (por exemplo, revisores parceiros aprovados pelo Legal).
4. O aprovador faz a assinatura na solicitação e vincula a emissão de rastreamento a qualquer entrada de
   gerenciamento de mudanças (exemplo: `DOCS-SORA-Preview-####`).

## Provisionamento e ferramentas

1. **Compartilhar artefatos** - Fornecer o descritor + arquivo de visualização mais recente do fluxo de trabalho
   de CI ou do pino SoraFS (artefato `docs-portal-preview`). Lembrar os revisores de execução:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Serviço com aplicação de checksum** - Apontar os revisores para o comando com gate de checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Isso reutiliza `scripts/serve-verified-preview.mjs` para que nenhum build não seja selecionado
   seja iniciado por acidente.

3. **Conceda acesso GitHub (opcional)** - Se revisores precisamrem de ramos não publicados,
   adicione-os ao tempo GitHub `docs-preview` durante a revisão e registro para mudança de associação
   na solicitação.

4. **Comunicar canais de suporte** - Compartilhar o contato on-call (Matrix/Slack) e o procedimento
   incidentes de [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetria + feedback** - Lembrar os revisores que análises anonimizadas e coletadas
   (ver [`observability`](./observability.md)). Fornecer o formulário de feedback ou modelo de problema
   citado no convite e registrador do evento com o helper
   [`preview-feedback-log`](./preview-feedback-log) para manter o resumo da onda atualizada.

## Checklist do revisor

Antes de acessar a visualização, os revisores devem completar:1. Verifique os arquivos baixados (`preview_verify.sh`).
2. Inicie o portal via `npm run serve` (ou `serve:verified`) para garantir que o protetor de checksum esteja ativo.
3. Leia as notas de segurança e observabilidade vinculadas acima.
4. Teste um console OAuth/Try it usando login com código de dispositivo (se aplicável) e evite reutilizar tokens de produção.
5. Registrar achados no tracker acordado (issue, documento compartilhado ou formulário) e taguea-los com
   o tag de lançamento do preview.

## Responsabilidades de mantenedores e offboarding

| Fase | Aços |
| --- | --- |
| Início | Confirme que a lista de verificação de ingestão está anexada à solicitação, compartilhe artigos + instruções, adicione uma entrada `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), e agende uma sincronização de meio período se a revisão durar mais de uma semana. |
| Monitoramento | Monitore a telemetria de visualização (obtenha tráfego incomum, falhas de sonda) e siga o runbook de incidentes se algo suspeito ocorrer. Registrar eventos `feedback-submitted`/`issue-opened` conforme os achados encontrados para manter as métricas da onda precisas. |
| Desativação | Revogar o acesso temporário ao GitHub ou SoraFS, registrador `access-revoked`, arquivar a solicitação (incluir resumo de feedback + ações pendentes), e atualizar o registro de revisores. Solicite ao revisor que remova builds locais e fixação do resumo gerado a partir de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Use o mesmo processo de rotação revisores entre ondas. Manter o rastro no repo (issue + templates)
ajuda o DOCS-SORA a permanecer auditável e permite que a governança confirme que o acesso de visualização
melhore os controles documentados.

## Templates de convite e rastreamento

- Inicie todo outreach com o arquivo
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  Ele captura o mínimo de linguagem legal, instruções de checksum de visualização e a expectativa de que
  revisores reconhecam a política de uso aceitavel.
- Ao editar o template, Substitua os placeholders de `<preview_tag>`, `<request_ticket>` e canais de contato.
  Guarde uma cópia da mensagem final no ticket de admissão para que revisores, aprovadores e auditores possam
  referenciar o texto exato enviado.
- Depois de enviar o convite, atualize a planilha de rastreamento ou emita com o timestamp `invite_sent_at` e os dados
  de encerramento extraordinário para que o relatório
  [visualizar fluxo de convite](./preview-invite-flow.md) pode identificar um coorte automaticamente.