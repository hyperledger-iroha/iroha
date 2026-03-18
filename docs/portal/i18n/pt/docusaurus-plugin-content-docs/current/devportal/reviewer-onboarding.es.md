---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Onboarding de revisores de visualização

## Resumo

DOCS-SORA segue um lançamento escalonado do portal de desenvolvimento. Los builds com portão de checksum
(`npm run serve`) e os fluxos Try it reforzados desbloqueados no próximo hit:
onboarding de revisores validados antes de a visualização pública se abrir de forma ampliada. Este guia
descrever como solicitar solicitações de recuperação, verificar elegibilidade, fornecer acesso e dar de baixo
participantes de forma segura. Consulte o
[visualizar fluxo de convite](./preview-invite-flow.md) para o planejamento de coortes, la
cadência de convites e exportações de telemetria; os passos de baixo são enfocados nas ações
uma vez que um revisor foi selecionado.

- **Alcance:** revisores que precisam acessar a visualização de documentos (`docs-preview.sora`,
  builds de GitHub Pages ou pacotes de SoraFS) antes de GA.
- **Fora de alcance:** operadores de Torii ou SoraFS (cubos por seus próprios kits de integração)
  e despliegues del portal de produção (ver
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Funções e pré-requisitos

| Rolo | Objetivos típicos | Artefatos necessários | Notas |
| --- | --- | --- | --- |
| Mantenedor principal | Verifique novas orientações, execute testes de fumaça. | Identificador GitHub, matriz de contato, CLA firmado em arquivo. | Geralmente você está no equipamento GitHub `docs-preview`; Aun asi registra uma solicitação para que o acesso seja auditável. |
| Revisor parceiro | Valide trechos do SDK ou conteúdo de governança antes do lançamento público. | Email corporativo, POC legal, termos de visualização firmados. | Você deve reconhecer os requisitos de telemetria + gerenciamento de dados. |
| Voluntário comunitário | Aportar feedback de usabilidade sobre guias. | Identificador GitHub, contato preferido, horário de trabalho, aceitação do CoC. | Mantener coortes pequenas; priorizar revisores que firmaram o acordo de contribuição. |

Todos os tipos de revisores devem:

1. Reconheça a política de uso aceitável para artefatos de visualização.
2. Leia os apêndices de segurança/observabilidade
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Aceite a execução do `docs/portal/scripts/preview_verify.sh` antes de servir qualquer um
   instantâneo localmente.

## Fluxo de ingestão

1. Peça ao solicitante que complete o
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulário (ou copiar/pegar em um problema). Capturar al menos: identidade, método de contato,
   Identificador do GitHub, datas de revisão, previsões e confirmação de que os documentos de segurança foram lidos.
2. Registrar a solicitação no rastreador `docs-preview` (emitir GitHub ou ticket de governo)
   e atribua um aprovador.
3. Validar pré-requisitos:
   - CLA / acordo de contribuição em arquivo (a referência de contrato do parceiro).
   - Reconhecimento de uso aceitável armazenado na solicitação.
   - Avaliação completa do risco (por exemplo, revisores parceiros aprovados pelo Departamento Jurídico).
4. O aprovador firma a solicitação e envia a questão do rastreamento com qualquer um
   entrada de gerenciamento de mudanças (exemplo: `DOCS-SORA-Preview-####`).

## Aprovisionamento e ferramentas

1. **Compartir artefatos** - Proporcionar o descritor + arquivo de visualização mais recente desde
   o fluxo de trabalho de CI ou o pino de SoraFS (artefato `docs-portal-preview`). Gravar os revisores
   executar:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Serviço com aplicação de checksum** - Indica aos revisores o comando com gate de checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Isso é reutilizado `scripts/serve-verified-preview.mjs` para que você não lance uma construção sem verificar
   por acidente.

3. **Dar acesso ao GitHub (opcional)** - Se os revisores precisarem de ramas não publicadas, adicione-os
   no equipamento GitHub `docs-preview` durante a revisão e registrar a mudança de membro na solicitação.

4. **Comunicar canais de suporte** - Compartilhar o contato de plantão (Matrix/Slack) e o procedimento
   incidentes de [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetria + feedback** - Registrar os revisores que são recopilados anonimizadamente
   (ver [`observability`](./observability.md)). Proporcionar o formulário de feedback para a planta
   de emissão referenciada no convite e registrar o evento com o ajudante
   [`preview-feedback-log`](./preview-feedback-log) para que o currículo de ola seja mantido ao dia.

## Checklist do revisor

Antes de acessar a visualização, os revisores devem completar o seguinte:1. Verifique os artefatos baixados (`preview_verify.sh`).
2. Abra o portal via `npm run serve` (ou `serve:verified`) para garantir que a proteção de soma de verificação esteja ativa.
3. Leia as notas de segurança e observação enlazadas arriba.
4. Experimente o console OAuth/Try it usando login do código do dispositivo (se aplicável) e evite reutilizar tokens de produção.
5. Registrar hallazgos no tracker acordado (issue, doc compartido ou formulario) e etiquetarlos
   com a tag de lançamento de visualização.

## Responsabilidades de mantenedores e desligamentos

| Fase | Ações |
| --- | --- |
| Início | Confirme que a lista de verificação de ingestão está anexada à solicitação, compartilhe artefatos + instruções, adicione uma entrada `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), e agende uma sincronização de metade do período se a revisão durar mais de uma semana. |
| Monitoramento | Monitore a telemetria de visualização (pesquise tráfego, tente incomum, falhas de sonda) e siga o runbook de incidentes se ocorrer algo específico. Registrar eventos `feedback-submitted`/`issue-opened` conforme llegan hallazgos para que as métricas da ola sejam mantidas precisas. |
| Desativação | Revogar o acesso temporal do GitHub ou SoraFS, registrar `access-revoked`, arquivar a solicitação (incluindo currículo de feedback + ações pendentes) e atualizar o registro de revisores. Peça ao revisor que elimine locais de compilação e adicione o resumo gerado de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Use o mesmo processo para revisores rotativos entre olas. Manter o rastro no repositório (edição + plantillas) ajuda
a que DOCS-SORA siga siendo auditável e permita que o governo confirme que o acesso de visualização segue
os controles documentados.

## Plantas de convite e rastreamento

- Inicia toda divulgação com el
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  arquivo. Captura o idioma mínimo legal, as instruções de checksum de visualização e expectativa
  de que os revisores reconheçam a política de uso aceitável.
- Ao editar a planta, substitua os espaços reservados para `<preview_tag>`, `<request_ticket>` e canais
  de contato. Guarde uma cópia da mensagem final no ticket de entrada para que revisores, aprovadores
  e os auditores podem referenciar o texto exato que foi enviado.
- Após enviar o convite, atualizar a hora de rastreamento ou emitir com o carimbo de data e hora `invite_sent_at`
  e a data esperada de cierre para que o relatório de
  [visualizar fluxo de convite](./preview-invite-flow.md) pode detectar a coorte automaticamente.