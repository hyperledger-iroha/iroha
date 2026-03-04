---
lang: pt
direction: ltr
source: docs/source/governance_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9201c0027f05b1ab2c83fa6b3e1a1e6dad3ff9660a8ed23bac7667408d421ada
source_last_modified: "2026-01-22T15:38:30.665014+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Manual de Governança

Este manual captura os rituais diários que mantêm a Rede Sora
conselho de governação alinhado. Ele agrega as referências oficiais do
repositório para que cerimônias individuais possam permanecer concisas, enquanto os operadores sempre
ter um único ponto de entrada para o processo mais amplo.

## Cerimônias do Conselho

- **Governança do dispositivo** – Consulte [Aprovação do dispositivo do Parlamento Sora](sorafs/signing_ceremony.md)
  para o fluxo de aprovação na cadeia que o Painel de Infraestrutura do Parlamento agora
  segue ao revisar as atualizações do chunker SoraFS.
- **Publicação da contagem de votos** – Consulte
  [Contagem de votos de governança](governance_vote_tally.md) para a CLI passo a passo
  fluxo de trabalho e modelo de relatório.

## Runbooks operacionais

- **Integrações de API** – [referência da API de governança](governance_api.md) lista os
  Superfícies REST/gRPC expostas pelos serviços do conselho, incluindo autenticação
  requisitos e regras de paginação.
- **Painéis de telemetria** – As definições JSON Grafana em
  `docs/source/grafana_*` definem as “Restrições de Governança” e “Agendador
  TEU”. Exporte o JSON para Grafana após cada versão para ficar alinhado
  com o layout canônico.

## Supervisão da disponibilidade de dados

### Aulas de retenção

Os painéis do Parlamento que aprovam os manifestos da DA devem fazer referência à retenção forçada
política antes de votar. A tabela abaixo reflete os padrões aplicados via
`torii.da_ingest.replication_policy` para que os revisores possam detectar incompatibilidades sem
procurando a fonte TOML.【docs/source/da/replication_policy.md:1】

| Etiqueta de governança | Classe de blob | Retenção quente | Retenção de frio | Réplicas necessárias | Classe de armazenamento |
|----------------|-----------|---------------|----------------|-------------------|---------------|
| `da.taikai.live` | `taikai_segment` | 24h | 14d | 5 | `hot` |
| `da.sidecar` | `nexus_lane_sidecar` | 6h | 7d | 4 | `warm` |
| `da.governance` | `governance_artifact` | 12h | 180d | 3 | `cold` |
| `da.default` | _todas as outras classes_ | 6h | 30d | 3 | `warm` |

O Painel de Infraestrutura deverá anexar o modelo preenchido do
`docs/examples/da_manifest_review_template.md` para todas as cédulas, então o manifesto
resumo, etiqueta de retenção e artefatos Norito permanecem vinculados na governança
registro.

### Trilha de auditoria do manifesto assinado

Antes de uma votação chegar à ordem do dia, os funcionários do conselho devem provar que o manifesto
os bytes em análise correspondem ao envelope do Parlamento e ao artefato SoraFS. Usar
as ferramentas existentes para coletar essas evidências:1. Obtenha o pacote de manifesto de Torii (`iroha app da get-blob --storage-ticket <hex>`
   ou o auxiliar SDK equivalente) para que todos façam hash dos mesmos bytes que alcançaram
   os portais.
2. Execute o verificador de stub de manifesto com o envelope assinado:
   ```
   cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json \
     --manifest-signatures-in=fixtures/sorafs_chunker/manifest_signatures.json \
     --json-out=/tmp/manifest_report.json
   ```
   Isso recalcula o resumo do manifesto BLAKE3, valida o
   `chunk_digest_sha3_256` e verifica cada assinatura Ed25519 incorporada em
   `manifest_signatures.json`. Consulte `docs/source/sorafs/manifest_pipeline.md`
   para opções CLI adicionais.
3. Copie o resumo, `chunk_digest_sha3_256`, identificador de perfil e lista de assinantes para
   o modelo de revisão. NOTA: se o verificador relatar “incompatibilidade de perfil” ou um
   falta de assinatura, suspender a votação e solicitar um envelope corrigido.
4. Armazene a saída do verificador (ou artefato CI do
   `ci/check_sorafs_fixtures.sh`) junto com a carga útil Norito `.to` para que os auditores
   pode reproduzir as evidências sem acessar gateways internos.

O pacote de auditoria resultante deverá permitir ao Parlamento recriar cada hash e assinatura
verifique mesmo depois que o manifesto for retirado do armazenamento quente.

### Revise a lista de verificação

1. Retire o envelope do manifesto aprovado pelo Parlamento (ver
   `docs/source/sorafs/signing_ceremony.md`) e registre o resumo BLAKE3.
2. Verifique se o bloco `RetentionPolicy` do manifesto corresponde à tag na tabela
   acima; Torii rejeitará incompatibilidades, mas o conselho deve capturar o
   evidências para auditores.【docs/source/da/replication_policy.md:32】
3. Confirme se a carga Norito enviada faz referência à mesma tag de retenção
   e classe blob que aparece no ticket de entrada.
4. Anexe o comprovante da verificação da política (saída CLI, `torii.da_ingest.replication_policy`
   dump ou artefato CI) ao pacote de revisão para que o SRE possa repetir a decisão.
5. Registre as torneiras de subsídio ou ajustes de aluguel planejados quando a proposta depender de
   `docs/source/sorafs_reserve_rent_plan.md`.

### Matriz de escalonamento

| Tipo de solicitação | Painel proprietário | Provas a anexar | Prazos e telemetria | Referências |
|--------------|--------------|-----------------------|------------|------------|
| Subsídio/ajuste de renda | Infraestrutura + Tesouraria | Pacote DA preenchido, delta de aluguel de `reserve_rentd`, projeção de reserva atualizada CSV, ata de votação do conselho | Anote o impacto do aluguel antes de enviar a atualização do Tesouro; inclui telemetria de buffer 30d contínua para que o Departamento Financeiro possa reconciliar na próxima janela de liquidação | `docs/source/sorafs_reserve_rent_plan.md`, `docs/examples/da_manifest_review_template.md` |
| Remoção de moderação/ação de conformidade | Moderação + Conformidade | Ticket de conformidade (`ComplianceUpdateV1`), tokens de prova, resumo do manifesto assinado, status de apelação | Siga o SLA de conformidade do gateway (reconhecimento dentro de 24h, remoção completa ≤72h). Anexe o trecho `TransparencyReportV1` mostrando a ação. | `docs/source/sorafs_gateway_compliance_plan.md`, `docs/source/sorafs_moderation_panel_plan.md` |
| Congelamento/reversão de emergência | Painel de moderação do Parlamento | Pacote de aprovação prévia, nova ordem de congelamento, resumo do manifesto de reversão, registro de incidentes | Publicar aviso de congelamento imediatamente e agendar o referendo de reversão no próximo período de governança; incluem saturação de buffer + telemetria de replicação DA para justificar a emergência. | `docs/source/sorafs/signing_ceremony.md`, `docs/source/sorafs_moderation_panel_plan.md` |Use a tabela ao fazer a triagem dos tickets de admissão para que cada painel receba a informação exata
artefatos necessários para executar seu mandato.

### Relatórios de resultados

Cada decisão DA-10 deve ser enviada com os seguintes artefatos (anexe-os ao
Entrada do DAG de governança referenciada na votação):

- O pacote Markdown concluído de
  `docs/examples/da_manifest_review_template.md` (agora incluindo assinatura e
  seções de escalonamento).
- O manifesto Norito assinado (`.to`) mais o envelope `manifest_signatures.json`
  ou logs do verificador de CI que comprovam o resumo da busca.
- Quaisquer atualizações de transparência desencadeadas pela ação:
  - Delta `TransparencyReportV1` para remoções ou congelamentos orientados por conformidade.
  - Delta do razão de aluguel/reserva ou instantâneo `ReserveSummaryV1` para subsídios.
- Links para instantâneos de telemetria coletados durante a revisão (profundidade de replicação,
  margem de buffer, backlog de moderação) para que os observadores possam verificar as condições
  depois do fato.

## Moderação e escalonamento

Remoções de gateway, recuperação de subsídios ou congelamentos de DA seguem a conformidade
pipeline descrito em `docs/source/sorafs_gateway_compliance_plan.md` e o
ferramentas de recurso em `docs/source/sorafs_moderation_panel_plan.md`. Os painéis devem:

1. Registre o ticket de conformidade de origem (`ComplianceUpdateV1` ou
   `ModerationAppealV1`) e anexe os tokens de prova associados.【docs/source/sorafs_gateway_compliance_plan.md:20】
2. Confirme se a solicitação invoca o caminho de recurso de moderação (painel de cidadãos
   votação) ou um congelamento de emergência do Parlamento; ambos os fluxos devem citar o manifesto
   tag de resumo e retenção capturada no novo modelo.【docs/source/sorafs_moderation_panel_plan.md:1】
3. Enumerar prazos de escalonamento (janelas de confirmação/revelação de apelação, emergência
   duração do congelamento) e indicar qual conselho ou painel é responsável pelo acompanhamento.
4. Capture o instantâneo de telemetria (capacidade de buffer, backlog de moderação) usado para
   justificar a ação para que as auditorias posteriores possam combinar a decisão com o
   estado.

Os painéis de conformidade e moderação devem sincronizar seus relatórios semanais de transparência
com os operadores de roteadores de liquidação, de modo que remoções e subsídios afetem o mesmo
conjunto de manifestos.

## Modelos de relatórios

Todas as revisões do DA-10 agora exigem um pacote Markdown assinado. Copiar
`docs/examples/da_manifest_review_template.md`, preencha os metadados do manifesto,
tabela de verificação de retenção e resumo da votação do painel e, em seguida, fixe o formulário preenchido
documento (mais artefatos Norito/JSON referenciados) à entrada do Governance DAG.
Os painéis devem vincular o pacote nas atas de governança para que futuras remoções ou
renovações de subsídios podem citar o resumo do manifesto original sem executar novamente o
cerimônia inteira.

## Fluxo de trabalho de incidentes e revogações

As ações emergenciais agora acontecem on-chain. Quando um lançamento de fixture precisa ser
revertido, apresentar um bilhete de governança e abrir uma proposta de reversão do Parlamento
apontando para o resumo do manifesto previamente aprovado. O Painel de Infraestrutura
lida com a votação e, uma vez finalizado, o tempo de execução Nexus publica a reversão
evento que os clientes downstream consomem. Nenhum artefato JSON local é necessário.

## Mantendo o manual atualizado- Atualize esse arquivo sempre que um novo runbook voltado para governança chegar ao
  repositório.
- Faça links cruzados de novas cerimônias aqui para que o índice do conselho permaneça detectável.
- Se um documento referenciado for movido (por exemplo, um novo caminho do SDK), atualize o link
  como parte da mesma solicitação pull para evitar ponteiros obsoletos.