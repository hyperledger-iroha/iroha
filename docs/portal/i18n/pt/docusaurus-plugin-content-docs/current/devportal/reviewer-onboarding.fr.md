---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Onboarding dos reletores de visualização

## Vista do conjunto

DOCS-SORA é adequado para um lance por etapas do desenvolvimento do portal. As compilações com portão de checksum
(`npm run serve`) e o fluxo Try it durcis debloquent le prochain jalon:
onboarding de relecteurs verifica antes que a pré-visualização pública não seja grande. Ce guia
decrit comment coletor de demandas, verificador de elegibilidade, provisionador de acesso e offboarder
os participantes em segurança. Reporte-vous au
[visualizar fluxo de convite](./preview-invite-flow.md) para planejamento de coortes, cadência
convite e exportações de telemetria; As etapas ci-dessous se concentram nas ações
a prendre une fois qu'un relecteur a ete selectionne.

- **Perímetro:** leitores que precisam de acesso aos documentos de visualização (`docs-preview.sora`,
  constrói páginas do GitHub ou pacotes SoraFS) antes do GA.
- **Fora do perímetro:** operadores Torii ou SoraFS (couverts par seus próprios kits de integração)
  e implementações de portal em produção (veja
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Funções e pré-requisitos

| Função | Objectivos típicos | Requisito de artefatos | Notas |
| --- | --- | --- | --- |
| Mantenedor principal | Verifica os novos guias, executa testes de fumaça. | Lide com o GitHub, entre em contato com a Matrix, signatário do CLA no dossiê. | Souvent deja no equipamento GitHub `docs-preview`; deposer quando meme uma exigência para que o acesso seja auditável. |
| Revisor parceiro | Valide os snippets SDK ou o conteúdo de governança antes do lançamento público. | E-mail corporativo, POC legal, sinalização de visualização de termos. | Doit reconnaitre les exigences de telemetrie + traitement des donnees. |
| Voluntário comunitário | Fornecer feedback de utilização nos guias. | Lidar com GitHub, preferência de contato, fuso horário, aceitação do CoC. | Garder les cohortes petites; priorize os relecteurs antes de assinar o acordo de contribuição. |

Todos os tipos de revisores devem:

1. Reconheça a política de uso aceitável para os artefatos de visualização.
2. Leia os anexos seguros/observáveis
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Contrate um executor `docs/portal/scripts/preview_verify.sh` antes de servir um
   localização do instantâneo.

## Fluxo de trabalho de admissão

1. Demander au demandeur de remplir le
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulaire (ou le copier/coller dans une issue). Capturador mínimo: identidade, mínimo de contato,
   lidar com o GitHub, datas anteriores de revisão e confirmação de que os documentos de segurança estão lá.
2. Registre a demanda no rastreador `docs-preview` (emitir GitHub ou ticket de governo)
   e atribuir um aprovador.
3. Verifique os pré-requisitos:
   - CLA / acordo de contribuição au dossier (ou referência de parceiro contratante).
   - Acusar de usar ações aceitáveis ​​na demanda.
   - Avaliação de riscos terminada (exemplo: sócio revisor aprova par Jurídico).
4. O aprovador assina a demanda e coloca a questão de acompanhamento na entrada do gerenciamento de mudanças
   (exemplo: `DOCS-SORA-Preview-####`).

## Provisionamento e desativação

1. **Partager les artefacts** - Fornecer o descritor mais recente + arquivo de visualização a partir de então
   o fluxo de trabalho CI ou o pino SoraFS (artefato `docs-portal-preview`). Revisores auxiliares do Rappeler
   executor:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Serviço com aplicação de soma de verificação** - Oriente os revisores em relação ao comando gatee:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Cela reutiliza `scripts/serve-verified-preview.mjs` para que a compilação não seja verificada
   ne soit lance por acidente.

3. **Acesso ao GitHub (opcional)** - Se os revisores solicitarem ramificações não publicadas,
   adicione o equipamento GitHub `docs-preview` para a duração da revisão e consigne as alterações
   de adesão na demanda.

4. **Comunique os canais de suporte** - Compartilhe o contato de plantão (Matrix/Slack) e outros
   procedimento de incidente de [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetria + feedback** - Rappeler aux reviewers que des analytics anonimizados são coletados
   (veja [`observability`](./observability.md)). Fornecer o formulário de feedback ou o modelo
   o problema é mencionado no convite e o jornalista do evento com o ajudante
   [`preview-feedback-log`](./preview-feedback-log) para que o currículo vago reste um dia.

## Checklist do revisor

Antes de acessar a visualização, os revisores devem completar:1. Verificador de artefatos telecarregados (`preview_verify.sh`).
2. Lance o portal via `npm run serve` (ou `serve:verified`) para garantir que a soma de verificação da guarda esteja ativa.
3. Leia as notas seguras e observabilite referencias ci-dessus.
4. Teste o console OAuth/Try it via login de código do dispositivo (se aplicável) e evite reutilizar tokens de produção.
5. Deposite as informações no rastreador convencional (edição, documento ou formulário) e marque-o
   com a tag de visualização do lançamento.

## Responsabilidades dos mantenedores e do desligamento

| Fase | Ações |
| --- | --- |
| Início | Confirme que a lista de verificação de entrada está conjunta com a demanda, compartilhe os artefatos + instruções, adicione uma entrada `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), e planeje um ponto mi-parcours si a revista durante mais uma semana. |
| Monitoramento | Vigie a visualização da telemetria (tráfego Try it habituel, verifique a sonda) e siga o runbook do incidente se aquele escolhido for suspeito. Jornalize os eventos `feedback-submitted`/`issue-opened` na medida em que as estatísticas cheguem para que as métricas de vaga sejam exatas. |
| Desativação | Revogue o acesso temporário GitHub ou SoraFS, consignatário `access-revoked`, arquive a demanda (inclua currículo de feedback + ações em atenção) e envie a cada dia o registro dos revisores. Exija o revisor de limpar as compilações locais e juntar o resumo gerado a partir de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Utilize o processo de meme durante a rotação dos revisores entre vagas. Guarde o rastreamento no repositório
(edição + modelos) ajuda DOCS-SORA para restaurá-lo auditável e permitir que o governo confirme o acesso
visualizar e seguir os controles de documentos.

## Modelos de convite e convite

- Começar cada divulgação com ele
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  mais arquivo. Ele captura o idioma mínimo legal, as instruções de visualização da soma de verificação e atenção
  que os revisores reconhecem a política de uso aceitável.
- Ao editar o modelo, substitua os espaços reservados para `<preview_tag>`, `<request_ticket>`
  e os canais de contato. Armazene uma cópia da mensagem final no ticket de entrada para os revisores,
  aprovadores e auditores podem referenciar o texto exato enviado.
- Após o envio do convite, envie um dia a planilha de acompanhamento ou o problema com o carimbo de data e hora
  `invite_sent_at` e a data final do atendimento para que o relacionamento
  [visualizar fluxo de convite](./preview-invite-flow.md) pode capturar o grupo automaticamente.