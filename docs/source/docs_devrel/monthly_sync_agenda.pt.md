---
lang: pt
direction: ltr
source: docs/source/docs_devrel/monthly_sync_agenda.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2f89131efc0c79ddf63d71a25c04029014ba58393fb6336e676181322bc5066
source_last_modified: "2026-01-03T18:08:00.500077+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Docs/DevRel Agenda de sincronização mensal

Esta agenda formaliza a sincronização mensal do Docs/DevRel que é referenciada em
`roadmap.md` (consulte “Adicionar revisão da equipe de localização ao Docs/DevRel mensal
sincronização”) e o plano Android AND5 i18n. Use-o como lista de verificação canônica e
atualize-o sempre que as entregas do roteiro adicionarem ou retirarem itens da agenda.

## Cadência e Logística

- **Frequência:** mensalmente (normalmente na segunda quinta-feira, 16:00UTC)
- **Duração:** 45 minutos + retorno opcional de 15 minutos para mergulhos profundos
- **Localização:** Zoom (`https://meet.sora.dev/docs-devrel-sync`) com compartilhamento
  notas em HackMD ou `docs/source/docs_devrel/minutes/<yyyy-mm>.md`
- **Público:** Gerente do Docs/DevRel (presidente), engenheiros do Docs, localização
  gerenciador de programa, SDK DX TLs (Android, Swift, JS), documentação do produto, versão
  Delegado de engenharia, observadores de suporte/QA
- **Facilitador:** Gerenciador de Docs/DevRel; nomear um escriba rotativo que
  comprometer os minutos no repositório dentro de 24 horas

## Lista de verificação pré-trabalho

| Proprietário | Tarefa | Artefato |
|-------|------|----------|
| Escriba | Crie o arquivo de notas do mês (`docs/source/docs_devrel/minutes/<yyyy-mm>.md`) usando o modelo abaixo. | Arquivo de notas |
| Localização PM | Atualize `docs/source/sdk/android/i18n_plan.md#translation-status` e o log de pessoal; preencher previamente as decisões propostas. | plano i18n |
| TLs DX | Execute `ci/check_android_docs_i18n.sh` ou `scripts/sync_docs_i18n.py --dry-run` e anexe resumos para discussão. | Artefatos de CI |
| Ferramentas do Documentos | Exporte resumos `docs/i18n/manifest.json` + lista de tickets pendentes de `docs/source/sdk/android/i18n_requests/`. | Resumo do manifesto e do ticket |
| Suporte/Lançamento | Reúna todos os encaminhamentos que exijam ação do Docs/DevRel (por exemplo, convites de visualização pendentes, bloqueio de feedback do revisor). | Status.md ou documento de escalonamento |

## Blocos de agenda1. **Chamada e objetivos (5min)**
   - Confirme quórum, escriba e logística.
   - Destaque quaisquer incidentes urgentes (interrupção de visualização de documentos, bloqueio de localização).
2. **Revisão da equipe de localização (15min)**
   - Revise o login de decisão de pessoal
     `docs/source/sdk/android/i18n_plan.md#staffing-decision-log`.
   - Confirme o status dos pedidos de compra abertos (`DOCS-L10N-*`) e cobertura provisória.
   - Compare a saída de atualização do CI com a tabela de status da tradução; chame qualquer
     documento cujo SLA local (>5 dias úteis) será violado antes do próximo
     sincronizar.
   - Decidir se o escalonamento é necessário (operações de produto, finanças, contratante
     gestão). Registre a decisão no registro de pessoal e no relatório mensal
     minutos, incluindo proprietário + data de vencimento.
   - Se a equipe estiver saudável, documente a confirmação para que a ação do roteiro possa
     volte para 🈺/🈴 com evidências.
3. **Atualizações de documentos/roteiro (10min)**
   - Status do trabalho do portal DOCS-SORA, proxy Try-It e publicação SoraFS
     prontidão.
   - Destaque a dívida documental ou os revisores necessários para os trens de lançamento atuais.
4. **Destaques do SDK (10min)**
   - Preparação de documentos Android AND5/AND7, paridade Swift IOS5, progresso JS GA.
   - Capture equipamentos compartilhados ou diferenças de esquema que afetarão os documentos.
5. **Revisão da ação e estacionamento (5min)**
   - Revisitar itens em aberto da sincronização anterior; confirmar fechamentos.
   - Registre novas ações no arquivo de notas com proprietários e prazos explícitos.

## Modelo de revisão de equipe de localização

Incluir a seguinte tabela na ata de cada mês:

| Local | Capacidade (ETI) | Compromissos e POs | Riscos/Escalações | Decisão e Proprietário |
|--------|----------------|-------------------|---------------------|------------------|
| JP | por exemplo, 0,5 contratante + 0,1 backup de documentos | PO `DOCS-L10N-4901` (aguardando assinatura) | “Contrato não assinado até 04/03/2026” | “Escalar para operações de produto — @docs-devrel, previsto para 02/03/2026” |
| ELE | por exemplo, engenheiro do Documentos 0.1 | Rotação entra no PTO 2026-03-18 | “Precisa de revisor de backup” | “@docs-lead para identificar backup até 05/03/2026” |

Registre também uma breve narrativa cobrindo:

- **Perspectiva do SLA:** qualquer documento que deverá perder o SLA de cinco dias úteis e o
  mitigação (prioridade de troca, recrutamento de fornecedor de backup, etc.).
- **Saúde de tickets e ativos:** Entradas pendentes em
  `docs/source/sdk/android/i18n_requests/` e se as capturas de tela/recursos são
  pronto para tradutores.

### Registro de revisão da equipe de localização

- **Atas:** Copie a tabela de pessoal + narrativa em
  `docs/source/docs_devrel/minutes/<yyyy-mm>.md` (todas as localidades espelham o
  Minutos em inglês por meio de arquivos localizados no mesmo diretório). Vincule a entrada
  de volta à agenda (`docs/source/docs_devrel/monthly_sync_agenda.md`) então
  a governança pode rastrear evidências.
- **Plano i18n:** Atualizar o registro de decisões de equipe e a tabela de status de tradução
  em `docs/source/sdk/android/i18n_plan.md` imediatamente após a reunião.
- **Status:** quando as decisões de pessoal afetarem as portas do roteiro, adicione uma entrada curta em
  `status.md` (seção Docs/DevRel) referenciando o arquivo de minutos e o plano i18n
  atualizar.

## Modelo de Minutos

Copie este esqueleto em `docs/source/docs_devrel/minutes/<yyyy-mm>.md`:

```markdown
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Docs/DevRel Monthly Sync — 2026-03-12

## Attendees
- Chair: …
- Scribe: …
- Participants: …

## Agenda Notes
1. Roll call & objectives — …
2. Localization staffing review — include table + narrative.
3. Docs/roadmap updates — …
4. SDK highlights — …
5. Action review & parking lot — …

## Decisions & Actions
| Item | Owner | Due | Notes |
|------|-------|-----|-------|
| JP contractor PO follow-up | @docs-devrel-manager | 2026-03-02 | Example entry |
```Publique as notas via PR logo após a reunião e vincule-as em `status.md`
ao fazer referência a decisões de risco ou de pessoal.

## Expectativas de acompanhamento

1. **Minutos comprometidos:** dentro de 24 horas (`docs/source/docs_devrel/minutes/`).
2. **Plano i18n atualizado:** ajuste o registro de pessoal e a tabela de tradução para
   refletir novos compromissos ou escaladas.
3. **Entrada Status.md:** resuma quaisquer decisões de alto risco para manter o roteiro
   em sincronia.
4. **Encaminhamentos arquivados:** quando a revisão exigir escalonamento, crie/atualize
   o ticket relevante (por exemplo, operações de produto, aprovação financeira, integração do fornecedor)
   e referenciá-lo nas atas e no plano i18n.

Ao seguir esta agenda, o requisito do roteiro para incluir a localização
as revisões de equipe na sincronização mensal do Docs/DevRel permanecem auditáveis e downstream
as equipes sempre sabem onde encontrar as evidências.