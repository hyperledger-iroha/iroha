---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado de capacidade de armazenamento
título: Mercado de capacidade de armazenamento SoraFS
sidebar_label: Mercado de capacidade
descrição: Planeje SF-2c para o mercado de capacidade, as ordens de replicação, a telefonia e os ganchos de governo.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/storage_capacity_marketplace.md`. Certifique-se de que as duas posições estejam alinhadas para que a documentação herdada esteja ativa.
:::

# Mercado de capacidade de armazenamento SoraFS (Brouillon SF-2c)

O item SF-2c do roteiro apresenta um mercado controlado pelos provedores de
estoque declara uma capacidade engajada, recupera ordens de replicação e
perçoivent des taxas proporcionais à disponibilidade de entrega. Quadro de documentos
os livrables necessários para o lançamento de estreia e o declínio nas pistas de ação.

## Objetivos

- Exprime les engagements de capacité (bytes totais, limites por pista, expiração)
  sob uma forma de consumo verificável pelo governo, o transporte SoraNet e Torii.
- Coloque os pinos entre os provedores dependendo da capacidade declarada, da aposta e dos
  contraintes de politique tout en maintenant un comportement déterminista.
- Medir a entrega de estoque (sucesso de replicação, tempo de atividade, testes de integridade)
  e exporta a telefonia para a distribuição de taxas.
- Fornecimento de processos de revogação e disputa contra provedores mal-intencionados
  soientes penalizados ou aposentados.

## Conceitos de domínio

| Conceito | Descrição | Inicial livrável |
|--------|-------------|-----------------|
| `CapacityDeclarationV1` | Payload Norito descreve o ID do provedor, o suporte do chunker de perfil, os GiB envolvidos, os limites por via, as dicas de preços, o engajamento de staking e a expiração. | Esquema + validador em `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instrução emitida pelo governo atribuindo um CID de manifesto a um ou mais fornecedores, incluindo o nível de redundância e as métricas SLA. | Esquema Norito compartilhado com Torii + API de contrato inteligente. |
| `CapacityLedger` | Registro on-chain/off-chain que atende às declarações de capacidade ativa, às ordens de replicação, às métricas de desempenho e ao acúmulo de taxas. | Módulo de contrato inteligente ou stub de serviço off-chain com snapshot determinado. |
| `MarketplacePolicy` | A política de governo define a aposta mínima, as exigências de auditoria e as courbes de penalização. | Estrutura de configuração em `sorafs_manifest` + documento de governo. |

### Esquemas implementados (estatuto)

## Découpage du travail

### 1. Esquema de sofá e registro| Tache | Proprietário(s) | Notas |
|------|----------|-------|
| Defina `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Equipe de Armazenamento / Governança | Utilizador Norito ; incluem versão semântica e referências de capacidade. |
| Implemente o analisador de módulos + validador em `sorafs_manifest`. | Equipe de armazenamento | O impostor identifica monótonos, limites de capacidade, exigências de jogo. |
| Crie os metadados do registro do bloco com `min_capacity_gib` por perfil. | GT Ferramentaria | Ajuda os clientes a impor exigências mínimas de hardware por perfil. |
| Grave o documento `MarketplacePolicy` capturando as barreiras de admissão e o calendário de penalidades. | Conselho de Governança | Publicado em documentos com padrões políticos. |

#### Definições de esquema (implementadas)- `CapacityDeclarationV1` captura compromissos de capacidade assinados pelo provedor, incluindo identificadores de blocos canônicos, referências de capacidade, limites opcionais por faixa, dicas de preços, janelas de validade e metadados. A validação garante uma aposta não nula, identificadores canônicos, aliases duplicados, tampas por faixa no total declarado e comptabilidade GiB monótona.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` associação de manifestos às cessões émis pela governança com objetivos de redundância, seus próprios SLA e garantias por cessão; os validadores impõem identificadores canônicos, provedores exclusivos e restrições de prazo antes da ingestão por Torii ou registro.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` exprime instantâneos da época (GiB declarados vs utilizados, computadores de replicação, porcentagens de tempo de atividade/PoR) que alimentam a distribuição de taxas. Os custos mantêm a utilização nas declarações e as porcentagens em 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Os parceiros auxiliares (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores via/atribuição/SLA) fornecem uma validação determinada de chaves e um relatório de erro reutilizável por CI e ferramentas downstream.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` expõe a distribuição do snapshot on-chain via `/v1/sorafs/capacity/state`, combinando declarações de provedores e entradas do razão de taxas atrás de um Norito JSON determinista.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- A cobertura de validação exerce a aplicação de alças canônicas, a detecção de dobrões, os bornes por pista, as guardas de atribuição de replicação e as verificações de plage de télémétrie para que as regressões apareçam imediatamente em CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Operador de ferramentas: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` converte especificações lisíveis em cargas úteis Norito canônicos, blobs base64 e currículos JSON para que os operadores possam preparar os equipamentos `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` e as ordens de replicação com validação locale.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Os equipamentos de referência vivem em `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) e são gerados via `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Integração do plano de controle| Tache | Proprietário(s) | Notas |
|------|----------|-------|
| Adiciona os manipuladores Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` com cargas úteis Norito JSON. | Equipe Torii | Réplica a lógica de validação; reutilize os auxiliares Norito JSON. |
| Propaga os snapshots `CapacityDeclarationV1` para os metadados do placar do orquestrador e os planos do fetch gateway. | GT Ferramentaria / Equipe Orquestrador | Use `provider_metadata` com referências de capacidade para que a pontuação multifonte respeite os limites por faixa. |
| Alimente as ordens de replicação nos clientes orquestrador/gateway para pilotar as atribuições e dicas de failover. | Equipe de TL/Gateway de rede | O construtor de placar consome as ordens assinadas pelo governo. |
| CLI de ferramentas: `sorafs_cli` com `capacity declare`, `capacity telemetry`, `capacity orders import`. | GT Ferramentaria | Fournir JSON déterministe + placar de surtidas. |

### 3. Mercado político e governança

| Tache | Proprietário(s) | Notas |
|------|----------|-------|
| Ratificador `MarketplacePolicy` (aposta mínima, multiplicadores de penalidade, cadência de auditoria). | Conselho de Governança | Publicado nos documentos, capturando a história das revisões. |
| Acrescenta ganchos de governo para que o Parlamento aprove, renove e revoque as declarações. | Conselho de Governança / Equipe de Contrato Inteligente | Use os eventos Norito + ingestão de manifestos. |
| Implemente o calendário de penalidades (redução de taxas, redução de títulos) devido a violações de SLA telemétricas. | Conselho de Governança / Tesouraria | Alinhe-o com as saídas de liquidação do `DealEngine`. |
| Documente o processo de disputa e a matriz de escalada. | Documentos / Governança | Lier no runbook de disputa + CLI de ajudantes. |

### 4. Medição e distribuição de taxas

| Tache | Proprietário(s) | Notas |
|------|----------|-------|
| Efetue a ingestão da medição Torii para aceitar `CapacityTelemetryV1`. | Equipe Torii | Validador GiB-hora, sucesso PoR, tempo de atividade. |
| Insira hoje o pipeline de medição `sorafs_node` para utilização do repórter por ordem + estatísticas SLA. | Equipe de armazenamento | Alinhar com as ordens de replicação e manipular chunker. |
| Pipeline de liquidação: conversão de telemetria + replicação de pagamentos denominados em XOR, produção de currículos prontos para governança e registro do estado do razão. | Equipe de Tesouraria/Armazenagem | Conectado às exportações do Deal Engine/Tesouraria. |
| Painéis/alertas do exportador para a saúde da medição (atraso de ingestão, transmissão obsoleta). | Observabilidade | Use o pacote Grafana referenciado por SF-6/SF-7. |- Torii expõe `/v1/sorafs/capacity/telemetry` e `/v1/sorafs/capacity/state` (JSON + Norito) desordenados para que os operadores recuperem o livro-razão canônico para auditoria ou embalagem de preuves.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- A integração `PinProviderRegistry` garante que as ordens de replicação sejam acessíveis através do mesmo endpoint ; os auxiliares CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) são válidos e públicos para desbloquear a transmissão de automatizações com determinação de hash e resolução de alias.
- Os snapshots de medição produzidos pelas entradas `CapacityTelemetrySnapshot` são fixados no snapshot `metering`, e as exportações Prometheus alimentam a placa Grafana pré-importados em `docs/source/grafana_sorafs_metering.json` para as equipes de faturação após acumulação de GiB-hora, taxas nano-SORA projetos e conformidade com SLA no tempo real.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Quando a suavização de medição estiver ativa, o snapshot inclui `smoothed_gib_hours` e `smoothed_por_success_bps` para comparar os valores EMA com os compteurs brutos utilizados pela governança para os pagamentos.【crates/sorafs_node/src/metering.rs:401】

### 5. Gestão de disputas e revogações

| Tache | Proprietário(s) | Notas |
|------|----------|-------|
| Defina a carga útil `CapacityDisputeV1` (reclamante, preuve, provedor ciblé). | Conselho de Governança | Esquema Norito + validador. |
| Suporte CLI para resolver disputas e responder (com peças de teste). | GT Ferramentaria | Garanta um hash determinado pelo pacote de testes. |
| Adicionar verificações automáticas para violações do SLA recorrentes (auto-escalada em disputa). | Observabilidade | Seus alertas e ganchos de governo. |
| Documente o manual de revogação (período de graça, evacuação de données pin). | Documentos / Equipe de armazenamento | Lier au doc ​​de politique et au runbook opérateur. |

## Exigências de testes e CI

- Testes unitários para todos os novos validadores de esquema (`sorafs_manifest`).
- Simulador de testes de integração: declaração → ordem de replicação → medição → pagamento.
- Workflow CI para gerenciar declarações/telemetria de capacidade e garantir a sincronização de assinaturas (tendre `ci/check_sorafs_fixtures.sh`).
- Testes gratuitos para o registro API (simulando 10 mil provedores, 100 mil pedidos).

## Telemetria e painéis

- Painéis do painel:
  - Capacidade declarada vs utilizada pelo provedor.
  - Atraso nas ordens de replicação e atraso na atribuição.
  - Conformidade com SLA (% de tempo de atividade, taxa de sucesso PoR).
  - Acumulação de taxas e penalidades por época.
- Alertas:
  - Provedor com capacidade mínima engajada.
  - Ordem de replicação bloqueada > SLA.
  - Échecs du pipeline de medição.

## Livrables de documentação- Guia do operador para declarar a capacidade, renovar compromissos e monitorar a utilização.
- Guia de governo para aprovar declarações, emitir ordens e gerenciar disputas.
- API de referência para endpoints de capacidade e formato de ordem de replicação.
- Mercado de perguntas frequentes para desenvolvedores.

## Checklist de prontidão GA

O item do roteiro **SF-2c** condiciona o lançamento da produção às pré-concreções
sobre a comptabilidade, a gestão de disputas e a integração. Utilize os artefatos
ci-dessous pour garder les critères d'aceitation alinhados com a implementação.

### Comptabilidade noturna e reconciliação XOR
- Exporte o instantâneo do estado de capacidade e a exportação do razão XOR para a mesma janela, depois lance:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  O ajudante classificou com um código não nulo em casos de liquidações/penalidades manquants ou excessivas e emitiu um currículo Prometheus em formato de arquivo de texto.
- O alerta `SoraFSCapacityReconciliationMismatch` (em `dashboards/alerts/sorafs_capacity_rules.yml`) é desativado quando as métricas de reconciliação sinalizam os cartões; painéis em `dashboards/grafana/sorafs_capacity_penalties.json`.
- Arquive o currículo JSON e os hashes sob `docs/examples/sorafs_capacity_marketplace_validation/` com os pacotes de governo.

### Preuve de contest & slashing
- Deposer des disputas via `sorafs_manifest_stub capacity dispute` (testes:
  `cargo test -p sorafs_car --test capacity_cli`) para armazenar cargas úteis canônicas.
- Lancer `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` e as suítes de penalidade (`record_capacity_telemetry_penalises_persistent_under_delivery`) para provar que disputas e cortes rejuvenescem de forma determinada.
- Siga `docs/source/sorafs/dispute_revocation_runbook.md` para capturar as pressões e a escalada; lier les aprovações de greve no relatório de validação.

### Onboarding de provedores e testes de fumaça de surtida
- Gere os artefatos de declaração/télemetria com `sorafs_manifest_stub capacity ...` e repita os testes CLI antes de enviar (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Soumettre via Torii (`/v1/sorafs/capacity/declare`) depois do capturador `/v1/sorafs/capacity/state` e das capturas Grafana. Siga o fluxo de surtida em `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Arquivar os artefatos assinados e as saídas de reconciliação em
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dependências e sequência

1. Terminer SF-2b (política de admissão) — o mercado depende de fornecedores válidos.
2. Implemente o esquema de sofá + registro (este documento) antes da integração Torii.
3. Finalize o pipeline de medição antes de ativar os pagamentos.
4. Etapa final: ativa a distribuição de taxas piloto para o governo, uma vez que os dados de medição são verificados em teste.

O progresso deve ser seguido no roteiro com referências a este documento. Mettre a jour o roteiro uma vez que cada seção maior (esquema, plano de controle, integração, medição, gerenciamento de disputas) atteint le status feature complete.