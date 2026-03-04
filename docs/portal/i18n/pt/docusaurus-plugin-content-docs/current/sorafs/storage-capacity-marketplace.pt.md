---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado de capacidade de armazenamento
título: Mercado de capacidade de armazenamento SoraFS
sidebar_label: Marketplace de capacidade
descrição: Plano SF-2c para marketplace de capacidade, ordens de replicação, telemetria e ganchos de governança.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/storage_capacity_marketplace.md`. Mantenha ambos os locais alinhados enquanto a documentação herdada permanece ativa.
:::

# Marketplace de capacidade de armazenamento SoraFS (Rascunho SF-2c)

O item SF-2c do roadmap apresenta um marketplace governado onde provedores de
armazenamento declaram capacidade comprometida, recebem ordens de replicação e
ganhe taxas fornecidas a disponibilidade entregue. Este documento delimita
os entregaveis exigidos para o primeiro lançamento e os divididos em trilhas acionaveis.

## Objetivos

- Expressar compromissos de capacidade (bytes totais, limites por pista, expiração)
  em formato verificavel consumido por governança, transporte SoraNet e Torii.
- Alocar pinos entre provedores de acordo com capacidade declarada, participação e
  restrições de política mantendo comportamento determinístico.
- Medir entrega de armazenamento (sucesso de replicação, tempo de atividade, provas de
  integridade) e exportar telemetria para distribuição de taxas.
- Provar processos de revogação e disputa para que provedores desonestos sejam
  penalizados ou removidos.

## Conceitos de domínio

| Conceito | Descrição | Entrega inicial |
|--------|-----------|---------|
| `CapacityDeclarationV1` | Payload Norito descrevendo ID do provedor, suporte de perfil de chunker, GiB comprometidos, limites por lane, dicas de preços, compromisso de staking e expiração. | Esquema + validador em `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instrução emitida pela governança que atribui um CID de manifesto a um ou mais provedores, incluindo nível de redundância e métricas de SLA. | Esquema Norito compartilhado com Torii + API de contrato inteligente. |
| `CapacityLedger` | Registro on-chain/off-chain que rastreia declarações de capacidade ativa, ordens de replicação, métricas de desempenho e acúmulo de taxas. | Módulo de contrato inteligente ou stub de serviço off-chain com snapshot determinístico. |
| `MarketplacePolicy` | Política de governança definindo participação mínima, requisitos de auditoria e curvas de deliberação. | Estrutura de configuração em `sorafs_manifest` + documento de governança. |

### Esquemas implementados (status)

## Desdobramento de trabalho

### 1. Camada de esquema e registro| Tarefa | Responsavel(é) | Notas |
|------|------------------|-------|
| Definir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Equipe de Armazenamento / Governança | Usar Norito; incluir versionamento semântico e referenciais de capacidade. |
| Implementar módulos de analisador + validador em `sorafs_manifest`. | Equipe de armazenamento | Importar IDs monotônicos, limites de capacidade, requisitos de participação. |
| Estender metadata do chunker Registry com `min_capacity_gib` por perfil. | GT Ferramentaria | Ajuda os clientes a importar requisitos mínimos de hardware por perfil. |
| Redigir documento `MarketplacePolicy` capturando guardrails de admissão e calendário de deliberações. | Conselho de Governança | Publicar em documentos junto com padrões de política. |

#### Definições de esquema (implementadas)- `CapacityDeclarationV1` captura compromissos de capacidade contratados por provedor, incluindo alças canônicas do chunker, referências de capacidade, limites máximos por via, dicas de preços, janelas de validade e metadados. A validação garante participação não zero, lida com canônicos, aliases deduplicados, caps por lane dentro do total declarado e contabilidade de GiB monotonicamente crescente. [crates/sorafs_manifest/src/capacity.rs:28]
- `ReplicationOrderV1` manifesta a atribuições emitidas pela governança com metas de redundância, limites de SLA e garantias por atribuição; validadores impoem tratam de canônicos, provedores únicos e restrições de prazo antes de Torii ou o registro ingerirem a ordem. [crates/sorafs_manifest/src/capacity.rs:301]
- `CapacityTelemetryV1` snapshots expressos por época (GiB declarados vs utilizados, contadores de replicação, percentuais de uptime/PoR) que alimentam a distribuição de taxas. Checagens de limites mantêm utilização dentro das declarações e percentuais dentro de 0-100%. [crates/sorafs_manifest/src/capacity.rs:476]
- Ajudantes compartilhados (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores de pista/atribuição/SLA) fornecem validação determinística de chaves e relatório de erro reutilizavel por CI e ferramentas downstream. [crates/sorafs_manifest/src/capacity.rs:230]
- `PinProviderRegistry` agora expõe o snapshot on-chain via `/v1/sorafs/capacity/state`, combinando declarações de provedores e entradas do livro de taxas por meio de Norito JSON determinístico. [crates/iroha_torii/src/sorafs/registry.rs:17] [crates/iroha_torii/src/sorafs/api.rs:64]
- A cobertura de validação, exercício de execução de cabos canônicos, detecção de duplicados, limites por via, guardas de atribuição de replicação e verificações de alcance de telemetria para que os regressos aparecam imediatamente no CI. [crates/sorafs_manifest/src/capacity.rs:792]
- Tooling para operadores: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` converte especificações legíveis em payloads Norito canônicos, blobs base64 e resumos JSON para que operadores preparem fixtures de `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` e ordens de replicação com validação local. [crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1] Fixtures de referência vivem em `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) e são gerados via `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Integração do plano de controle| Tarefa | Responsavel(é) | Notas |
|------|------------------|-------|
| Adicione os manipuladores Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` com payloads Norito JSON. | Equipe Torii | Espelhar lógica do validador; reutilizar ajudantes Norito JSON. |
| Propagar snapshots `CapacityDeclarationV1` para metadados do scoreboard do orquestrador e planos de busca do gateway. | GT Ferramentaria / Equipe Orquestrador | Estender `provider_metadata` com referências de capacidade para que o score multi-source respeite limites por pista. |
| Alimenta ordens de replicação em clientes de orquestrador/gateway para orientar atribuições e dicas de failover. | Equipe de TL/Gateway de rede | O construtor do placar consome ordens assinadas pela governança. |
| CLI de ferramentas: estender `sorafs_cli` com `capacity declare`, `capacity telemetry`, `capacity orders import`. | GT Ferramentaria | Fornecer JSON determinístico + saidas de scoreboard. |

### 3. Política de mercado e governança

| Tarefa | Responsavel(é) | Notas |
|------|------------------|-------|
| Ratificar `MarketplacePolicy` (stake minimo, multiplicadores de acaso, cadência de auditório). | Conselho de Governança | Publicar em documentos, capturar histórico de revisões. |
| Ganchos de governança para que o Parlamento possa aprovar, renovar e revogar declarações. | Conselho de Governança / Equipe de Contrato Inteligente | Usar eventos Norito + ingestão de manifestos. |
| Implementar calendário de negociação (redução de taxas, redução de títulos) ligado a violações de SLA telegrafadas. | Conselho de Governança / Tesouraria | Alinhar com saídas de liquidação do `DealEngine`. |
| Documentar processo de disputa e matriz de escalada. | Documentos / Governança | Linkar ao runbook de disputa + helpers da CLI. |

### 4. Medição e distribuição de taxas

| Tarefa | Responsavel(é) | Notas |
|------|------------------|-------|
| Expandir ingestão de medição do Torii para aceitar `CapacityTelemetryV1`. | Equipe Torii | Validar GiB-hora, sucesso PoR, tempo de atividade. |
| Atualizar pipeline de medição do `sorafs_node` para reportar utilização por pedido + estatísticas de SLA. | Equipe de armazenamento | Alinhar com ordens de replicação e alças de chunker. |
| Pipeline de liquidação: conversor telemetria + dados de replicação em pagamentos denominados em XOR, produzir resumos prontos para governança e registrar o estado do ledger. | Equipe de Tesouraria/Armazenagem | Conectar ao Deal Engine / Exportações do Tesouro. |
| Exportar dashboards/alertas para saúde do medidor (backlog de ingestão, telemetria obsoleta). | Observabilidade | Estender pack de Grafana referenciado por SF-6/SF-7. |- Torii agora expoe `/v1/sorafs/capacity/telemetry` e `/v1/sorafs/capacity/state` (JSON + Norito) para que os operadores enviem snapshots de telemetria por época e os inspetores recuperem o ledger canonico para auditorias ou empacotamento de evidência. [crates/iroha_torii/src/sorafs/api.rs:268] [crates/iroha_torii/src/sorafs/api.rs:816]
- A integração `PinProviderRegistry` garante que as ordens de replicação sejam acessíveis pelo mesmo endpoint; helpers de CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) agora validam e publicam telemetria a partir de execuções automatizadas com hashing determinístico e resolução de aliases.
- Snapshots de medição provenientes de entradas `CapacityTelemetrySnapshot` inseridos ao snapshot `metering`, e exporta Prometheus alimentam o board Grafana pronto para importação em `docs/source/grafana_sorafs_metering.json` para que equipes de entrega monitorem acumulação de GiB-hora, taxas nano-SORA projetado e cumprimento de SLA em tempo real. [crates/iroha_torii/src/routing.rs:5143] [docs/source/grafana_sorafs_metering.json:1]
- Quando a suavização de medição está habilitada, o snapshot inclui `smoothed_gib_hours` e `smoothed_por_success_bps` para que os operadores comparem valores com EMA contra contadores brutos usados pela governança para pagamentos. [crates/sorafs_node/src/metering.rs:401]

### 5. Tratamento de disputa e revogação

| Tarefa | Responsavel(é) | Notas |
|------|------------------|-------|
| Definir payload `CapacityDisputeV1` (reclamante, evidência, provedor alvo). | Conselho de Governança | Esquema Norito + validador. |
| Suporte de CLI para abrir disputas e responder (com anexos de evidência). | GT Ferramentaria | Garantir hashing determinístico do pacote de evidências. |
| Adicionar verificações automatizadas para violações de SLA repetidas (auto-escalada para disputa). | Observabilidade | Limiares de alerta e ganchos de governança. |
| Documentar playbook de revogação (período de graça, evacuação de dados pinados). | Documentos / Equipe de armazenamento | Linkar ao documento de política e runbook do operador. |

## Requisitos de testes e CI

- Testes unitários para todos os novos validadores de esquema (`sorafs_manifest`).
- Testes de integração que simulam: declaração -> ordem de replicação -> medição -> pagamento.
- Workflow de CI para regenerar declarações/telemetria de capacidade e garantir que as assinaturas permaneçam sincronizadas (estender `ci/check_sorafs_fixtures.sh`).
- Testes de carga para a API do registro (10k provedores simulados, 100k pedidos).

## Telemetria e dashboards

- Painéis do painel:
  - Capacidade declarada vs utilizada pelo provedor.
  - Backlog de ordens de replicação e atraso médio de atribuição.
  - Cumprimento de SLA (% de uptime, taxa de sucesso PoR).
  - Acréscimo de taxas e deliberações por época.
- Alertas:
  - Provedor abaixo da capacidade mínima comprometida.
  - Ordem de replicação travada > SLA.
  - Falhas no pipeline de medição.

## Entregas de documentação- Guia de operador para declarar capacidade, renovar compromissos e monitorar utilização.
- Guia de governança para aprovar declarações, emitir ordens e lidar com disputas.
- Referência de API para endpoints de capacidade e formato de ordem de replicação.
- FAQ do marketplace para desenvolvedores.

## Checklist de prontidão GA

O item de roadmap **SF-2c** bloqueia o lançamento em produção com base em evidência concreta
sobre contabilidade, tratamento de disputa e onboarding. Use os artefatos abaixo para
manter os critérios de aceitação em sincronia com a implementação.

### Contabilidade noturna e reconciliação XOR
- Exportar o snapshot de estado de capacidade e o exportar do ledger XOR para a mesma janela,
  depois execute:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  O helper sai com código não zero em assentamentos ou deliberações ausentes/excessivas e emite
  um resumo Prometheus em formato textfile.
- O alerta `SoraFSCapacityReconciliationMismatch` (em `dashboards/alerts/sorafs_capacity_rules.yml`)
  dispara quando métricas de reconciliação reportam lacunas; dashboards ficam em
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Arquivar o resumo JSON e hashes em `docs/examples/sorafs_capacity_marketplace_validation/`
  junto com pacotes de governança.

### Evidência de disputa e corte
- Arquivar disputas via `sorafs_manifest_stub capacity dispute` (testes:
  `cargo test -p sorafs_car --test capacity_cli`) para manter cargas úteis canônicas.
- Execute `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` e como suítes
  de deliberação (`record_capacity_telemetry_penalises_persistent_under_delivery`) para provar
  que disputas e barras fazem replay determinístico.
- Siga `docs/source/sorafs/dispute_revocation_runbook.md` para captura de evidências e escalonamento;
  linke aprovações de greve no relatorio de validação.

### Onboarding de provedores e saída de testes de fumaça
- Regenere artistas de declaração/telemetria com `sorafs_manifest_stub capacity ...` e rode os
  testes de CLI antes da submissão (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Submeta via Torii (`/v1/sorafs/capacity/declare`) e captura `/v1/sorafs/capacity/state` mais
  capturas de tela fazem Grafana. Siga o fluxo de saida em `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Arquive artistas assinados e resultados de reconciliação dentro de
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dependências e sequenciamento

1. Finalizar SF-2b (política de admissão) - o mercado depende de fornecedores aprovados.
2. Implementar camada de esquema + registro (este documento) antes da integração Torii.
3. Conclua a medição do pipeline antes de ativar os pagamentos.
4. Etapa final: habilitar distribuição de taxas controlada por governança quando dados de
   medição foram selecionados na preparação.

O progresso deve ser rastreado no roteiro com referência a este documento. Atualizar o
roadmap assim que cada seção principal (esquema, plano de controle, integração, medição,
tratamento de disputa) atingir status feature complete.