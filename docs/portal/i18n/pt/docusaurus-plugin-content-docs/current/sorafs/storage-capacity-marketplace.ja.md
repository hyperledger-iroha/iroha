---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/storage-capacity-marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22b0a08b6d44781b62343eb507d080d728039f15bda2ad7037905e5bc1ba1bc8
source_last_modified: "2025-11-14T04:43:22.362265+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/storage_capacity_marketplace.md`. Mantenha ambos os locais alinhados enquanto a documentacao herdada permanecer ativa.
:::

# Marketplace de capacidade de armazenamento SoraFS (Rascunho SF-2c)

O item SF-2c do roadmap introduz um marketplace governado onde providers de
armazenamento declaram capacidade comprometida, recebem ordens de replicacao e
ganham fees proporcionais a disponibilidade entregue. Este documento delimita
os entregaveis exigidos para a primeira release e os divide em trilhas acionaveis.

## Objetivos

- Expressar compromissos de capacidade (bytes totais, limites por lane, expiracao)
  em formato verificavel consumivel por governanca, transporte SoraNet e Torii.
- Alocar pins entre providers de acordo com capacidade declarada, stake e
  restricoes de politica mantendo comportamento deterministico.
- Medir entrega de armazenamento (sucesso de replicacao, uptime, proofs de
  integridade) e exportar telemetria para distribuicao de fees.
- Prover processos de revogacao e disputa para que providers desonestos sejam
  penalizados ou removidos.

## Conceitos de dominio

| Conceito | Descricao | Entregavel inicial |
|---------|-----------|--------------------|
| `CapacityDeclarationV1` | Payload Norito descrevendo ID do provider, suporte de perfil de chunker, GiB comprometidos, limites por lane, hints de pricing, compromisso de staking e expiracao. | Esquema + validador em `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instrucao emitida pela governanca que atribui um CID de manifest a um ou mais providers, incluindo nivel de redundancia e metricas SLA. | Esquema Norito compartilhado com Torii + API de smart contract. |
| `CapacityLedger` | Registry on-chain/off-chain que rastreia declaracoes de capacidade ativas, ordens de replicacao, metricas de performance e accrual de fees. | Modulo de smart contract ou stub de servico off-chain com snapshot deterministico. |
| `MarketplacePolicy` | Politica de governanca definindo stake minimo, requisitos de auditoria e curvas de penalidade. | Struct de config em `sorafs_manifest` + documento de governanca. |

### Esquemas implementados (status)

## Desdobramento de trabalho

### 1. Camada de schema e registry

| Tarefa | Responsavel(is) | Notas |
|------|------------------|-------|
| Definir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Storage Team / Governance | Usar Norito; incluir versionamento semantico e referencias de capacidade. |
| Implementar modulos de parser + validador em `sorafs_manifest`. | Storage Team | Impor IDs monotonicos, limites de capacidade, requisitos de stake. |
| Estender metadata do chunker registry com `min_capacity_gib` por perfil. | Tooling WG | Ajuda clientes a impor requisitos minimos de hardware por perfil. |
| Redigir documento `MarketplacePolicy` capturando guardrails de admissao e calendario de penalidades. | Governance Council | Publicar em docs junto com defaults de politica. |

#### Definicoes de schema (implementadas)

- `CapacityDeclarationV1` captura compromissos de capacidade assinados por provider, incluindo handles canonicos do chunker, referencias de capacidade, caps opcionais por lane, hints de pricing, janelas de validade e metadata. A validacao garante stake nao zero, handles canonicos, aliases deduplicados, caps por lane dentro do total declarado e contabilidade de GiB monotonicamente crescente. [crates/sorafs_manifest/src/capacity.rs:28]
- `ReplicationOrderV1` vincula manifests a atribuicoes emitidas pela governanca com metas de redundancia, limiares de SLA e garantias por atribuicao; validadores impoem handles canonicos, providers unicos e restricoes de deadline antes de Torii ou o registry ingerirem a ordem. [crates/sorafs_manifest/src/capacity.rs:301]
- `CapacityTelemetryV1` expressa snapshots por epoca (GiB declarados vs utilizados, contadores de replicacao, percentuais de uptime/PoR) que alimentam a distribuicao de fees. Checagens de limites mantem utilizacao dentro das declaracoes e percentuais dentro de 0-100%. [crates/sorafs_manifest/src/capacity.rs:476]
- Helpers compartilhados (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores de lane/atribuicao/SLA) fornecem validacao deterministica de chaves e report de erro reutilizavel por CI e tooling downstream. [crates/sorafs_manifest/src/capacity.rs:230]
- `PinProviderRegistry` agora expoe o snapshot on-chain via `/v1/sorafs/capacity/state`, combinando declaracoes de providers e entradas do fee ledger por meio de Norito JSON deterministico. [crates/iroha_torii/src/sorafs/registry.rs:17] [crates/iroha_torii/src/sorafs/api.rs:64]
- A cobertura de validacao exercita enforcement de handles canonicos, deteccao de duplicados, limites por lane, guardas de atribuicao de replicacao e checks de range de telemetria para que regressoes aparecam imediatamente no CI. [crates/sorafs_manifest/src/capacity.rs:792]
- Tooling para operadores: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` converte specs legiveis em payloads Norito canonicos, blobs base64 e resumos JSON para que operadores preparem fixtures de `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` e ordens de replicacao com validacao local. [crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1] Fixtures de referencia vivem em `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) e sao gerados via `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Integracao do plano de controle

| Tarefa | Responsavel(is) | Notas |
|------|------------------|-------|
| Adicionar handlers Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` com payloads Norito JSON. | Torii Team | Espelhar logica do validador; reutilizar helpers Norito JSON. |
| Propagar snapshots `CapacityDeclarationV1` para metadata do scoreboard do orchestrator e planos de fetch do gateway. | Tooling WG / Orchestrator team | Estender `provider_metadata` com referencias de capacidade para que o scoring multi-source respeite limites por lane. |
| Alimentar ordens de replicacao em clientes de orchestrator/gateway para orientar atribuicoes e hints de failover. | Networking TL / Gateway team | O builder do scoreboard consome ordens assinadas pela governanca. |
| Tooling CLI: estender `sorafs_cli` com `capacity declare`, `capacity telemetry`, `capacity orders import`. | Tooling WG | Fornecer JSON deterministico + saidas de scoreboard. |

### 3. Politica de marketplace e governanca

| Tarefa | Responsavel(is) | Notas |
|------|------------------|-------|
| Ratificar `MarketplacePolicy` (stake minimo, multiplicadores de penalidade, cadencia de auditoria). | Governance Council | Publicar em docs, capturar historico de revisoes. |
| Adicionar hooks de governanca para que o Parlamento possa aprovar, renovar e revogar declaracoes. | Governance Council / Smart Contract team | Usar eventos Norito + ingestao de manifests. |
| Implementar calendario de penalidades (reducao de fees, slashing de bond) ligado a violacoes de SLA telegrafadas. | Governance Council / Treasury | Alinhar com outputs de settlement do `DealEngine`. |
| Documentar processo de disputa e matriz de escalonamento. | Docs / Governance | Linkar ao runbook de disputa + helpers de CLI. |

### 4. Metering e distribuicao de fees

| Tarefa | Responsavel(is) | Notas |
|------|------------------|-------|
| Expandir ingesta de metering do Torii para aceitar `CapacityTelemetryV1`. | Torii Team | Validar GiB-hour, sucesso PoR, uptime. |
| Atualizar pipeline de metering do `sorafs_node` para reportar utilizacao por ordem + estatisticas SLA. | Storage Team | Alinhar com ordens de replicacao e handles de chunker. |
| Pipeline de settlement: converter telemetria + dados de replicacao em pagamentos denominados em XOR, produzir resumos prontos para governanca e registrar o estado do ledger. | Treasury / Storage Team | Conectar ao Deal Engine / Treasury exports. |
| Exportar dashboards/alertas para saude do metering (backlog de ingesta, telemetria stale). | Observability | Estender pack de Grafana referenciado por SF-6/SF-7. |

- Torii agora expoe `/v1/sorafs/capacity/telemetry` e `/v1/sorafs/capacity/state` (JSON + Norito) para que operadores enviem snapshots de telemetria por epoca e inspetores recuperem o ledger canonico para auditoria ou empacotamento de evidencia. [crates/iroha_torii/src/sorafs/api.rs:268] [crates/iroha_torii/src/sorafs/api.rs:816]
- A integracao `PinProviderRegistry` garante que ordens de replicacao sejam acessiveis pelo mesmo endpoint; helpers de CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) agora validam e publicam telemetria a partir de runs automatizados com hashing deterministico e resolucao de aliases.
- Snapshots de metering produzem entradas `CapacityTelemetrySnapshot` fixadas ao snapshot `metering`, e exports Prometheus alimentam o board Grafana pronto para importacao em `docs/source/grafana_sorafs_metering.json` para que equipes de faturamento monitorem acumulacao de GiB-hour, fees nano-SORA projetados e compliance de SLA em tempo real. [crates/iroha_torii/src/routing.rs:5143] [docs/source/grafana_sorafs_metering.json:1]
- Quando o smoothing de metering esta habilitado, o snapshot inclui `smoothed_gib_hours` e `smoothed_por_success_bps` para que operadores comparem valores com EMA contra contadores brutos usados pela governanca para payouts. [crates/sorafs_node/src/metering.rs:401]

### 5. Tratamento de disputa e revogacao

| Tarefa | Responsavel(is) | Notas |
|------|------------------|-------|
| Definir payload `CapacityDisputeV1` (reclamante, evidencia, provider alvo). | Governance Council | Schema Norito + validador. |
| Suporte de CLI para abrir disputas e responder (com anexos de evidencia). | Tooling WG | Garantir hashing deterministico do bundle de evidencia. |
| Adicionar checks automatizados para violacoes SLA repetidas (auto-escalada para disputa). | Observability | Limiares de alerta e hooks de governanca. |
| Documentar playbook de revogacao (periodo de graca, evacuacao de dados pinados). | Docs / Storage Team | Linkar ao doc de politica e runbook de operador. |

## Requisitos de testes e CI

- Testes unitarios para todos os novos validadores de schema (`sorafs_manifest`).
- Testes de integracao que simulam: declaracao -> ordem de replicacao -> metering -> payout.
- Workflow de CI para regenerar declaracoes/telemetria de capacidade e garantir que as assinaturas permanecam sincronizadas (estender `ci/check_sorafs_fixtures.sh`).
- Testes de carga para a API do registry (simular 10k providers, 100k ordens).

## Telemetria e dashboards

- Paines de dashboard:
  - Capacidade declarada vs utilizada por provider.
  - Backlog de ordens de replicacao e atraso medio de atribuicao.
  - Compliance de SLA (uptime %, taxa de sucesso PoR).
  - Accrual de fees e penalidades por epoca.
- Alertas:
  - Provider abaixo da capacidade minima comprometida.
  - Ordem de replicacao travada > SLA.
  - Falhas no pipeline de metering.

## Entregaveis de documentacao

- Guia de operador para declarar capacidade, renovar compromissos e monitorar utilizacao.
- Guia de governanca para aprovar declaracoes, emitir ordens e lidar com disputas.
- Referencia de API para endpoints de capacidade e formato de ordem de replicacao.
- FAQ do marketplace para developers.

## Checklist de readiness GA

O item de roadmap **SF-2c** bloqueia o rollout em producao com base em evidencia concreta
sobre contabilidade, tratamento de disputa e onboarding. Use os artefatos abaixo para
manter os criterios de aceitacao em sync com a implementacao.

### Contabilidade noturna e reconciliacao XOR
- Exporte o snapshot de estado de capacidade e o export do ledger XOR para a mesma janela,
  depois execute:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  O helper sai com codigo nao zero em settlements ou penalidades ausentes/excessivas e emite
  um resumo Prometheus em formato textfile.
- O alerta `SoraFSCapacityReconciliationMismatch` (em `dashboards/alerts/sorafs_capacity_rules.yml`)
  dispara quando metricas de reconciliacao reportam gaps; dashboards ficam em
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Arquive o resumo JSON e hashes em `docs/examples/sorafs_capacity_marketplace_validation/`
  junto com pacotes de governanca.

### Evidencia de disputa e slashing
- Arquive disputas via `sorafs_manifest_stub capacity dispute` (tests:
  `cargo test -p sorafs_car --test capacity_cli`) para manter payloads canonicos.
- Execute `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` e as suites
  de penalidade (`record_capacity_telemetry_penalises_persistent_under_delivery`) para provar
  que disputas e slashes fazem replay deterministico.
- Siga `docs/source/sorafs/dispute_revocation_runbook.md` para captura de evidencia e escalonamento;
  linke aprovacoes de strike no relatorio de validacao.

### Onboarding de providers e exit smoke tests
- Regenere artefatos de declaracao/telemetria com `sorafs_manifest_stub capacity ...` e rode os
  tests de CLI antes da submissao (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Submeta via Torii (`/v1/sorafs/capacity/declare`) e capture `/v1/sorafs/capacity/state` mais
  screenshots do Grafana. Siga o fluxo de saida em `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Arquive artefatos assinados e outputs de reconciliacao dentro de
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dependencias e sequenciamento

1. Finalizar SF-2b (politica de admissao) - o marketplace depende de providers aprovados.
2. Implementar camada de schema + registry (este doc) antes da integracao Torii.
3. Completar pipeline de metering antes de habilitar payouts.
4. Etapa final: habilitar distribuicao de fees controlada por governanca quando dados de
   metering forem verificados em staging.

O progresso deve ser rastreado no roadmap com referencias a este documento. Atualize o
roadmap assim que cada secao principal (schema, plano de controle, integracao, metering,
tratamento de disputa) atingir status feature complete.
