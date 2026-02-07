---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado de capacidade de armazenamento
título: Mercado de capacidade de armazenamento de SoraFS
sidebar_label: Mercado de capacidade
descrição: Plano SF-2c para o mercado de capacidade, ordens de replicação, telemetria e ganchos de governo.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/storage_capacity_marketplace.md`. Mantenha ambos os lugares alinhados enquanto a documentação herdada segue ativa.
:::

# Mercado de capacidade de armazenamento de SoraFS (Borrador SF-2c)

O item do roteiro SF-2c apresenta um mercado governado onde os fornecedores de
armazenamento declarado de capacidade comprometida, recebendo ordens de replicação e
Ganan taxas proporcionais à disponibilidade de entrega. Este documento delimita
as entregas exigidas para o primeiro lançamento e a divisão em faixas acionáveis.

## Objetivos

- Expresar compromissos de capacidade (bytes totais, limites por pista, expiração)
  em uma forma verificável consumível por governo, transporte SoraNet e Torii.
- Atribuir pins entre provedores conforme capacidade declarada, participação e restrições de
  A política enquanto se mantém um comportamento determinista.
- Medir a entrega de armazenamento (saída de replicação, tempo de atividade, provas de integridade) e
  exportar telemetria para distribuição de taxas.
- Proveer processos de revogação e disputa para que provedores desonestos sean
  penalizados ou removidos.

## Conceitos de domínio

| Conceito | Descrição | Entrega inicial |
|--------|-------------|---------------------|
| `CapacityDeclarationV1` | Payload Norito que descreve ID de provedor, suporte de perfil de chunker, GiB comprometido, limites por pista, faixas de preços, compromisso de staking e expiração. | Esquema + validador em `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instrução emitida pelo governo para atribuir um CID de manifesto a um ou mais provedores, incluindo nível de redundância e métricas de SLA. | Esquema Norito compartilhado com Torii + API de contrato inteligente. |
| `CapacityLedger` | Registro on-chain/off-chain que rastreia declarações de capacidade ativa, ordens de replicação, métricas de rendimento e acúmulo de taxas. | Módulo de contrato inteligente ou esboço de serviço off-chain com snapshot determinista. |
| `MarketplacePolicy` | Política de governança que define o mínimo de participação, requisitos de auditoria e curvas de penalização. | Estrutura de configuração em `sorafs_manifest` + documento de governança. |

### Esquemas implementados (estado)

## Desglose de trabalho

### 1. Capa de esquema e registro| Tara | Responsáveis(es) | Notas |
|------|----------------|------|
| Definir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Equipamento de Armazenamento / Governo | Usar Norito; incluir versão semântica e referenciais de capacidades. |
| Implementar módulos de analisador + validador em `sorafs_manifest`. | Equipamento de Armazenamento | Imponha IDs monotônicos, limites de capacidade, requisitos de participação. |
| Extensor de metadados do registro chunker com `min_capacity_gib` por perfil. | GT Ferramentaria | Ajude os clientes a impor requisitos mínimos de hardware por perfil. |
| Editar o documento `MarketplacePolicy` com barreiras de admissão e calendário de penalizações. | Conselho de Governança | Publicar documentos junto com padrões políticos. |

#### Definições de esquema (implementadas)- `CapacityDeclarationV1` captura compromissos de capacidade firmados pelo provedor, incluindo identificadores canônicos do chunker, referências de capacidades, limites opcionais por via, faixas de preços, janelas de validade e metadados. A validação garante a aposta sem zero, lida com canônicos, aliases desduplicados, limites por faixa dentro do total declarado e contabilidade de GiB monotônica.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` enlaza se manifesta com designações emitidas por governo que incluem objetivos de redundância, limites de SLA e garantias por designação; Os validadores impõem identificadores canônicos, provedores únicos e restrições de prazo antes de Torii ou o registro consome a ordem.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` instantâneos expressos por época (GiB declarados vs usados, contadores de replicação, porcentagens de tempo de atividade/PoR) que alimentam a distribuição de taxas. As validações mantêm o uso dentro da declaração e as porcentagens entre 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Helpers compartilhados (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores de pista/atribuição/SLA) provam validação determinista de chaves e relatam erros que CI e ferramentas downstream podem reutilizar.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` agora expõe o snapshot on-chain via `/v1/sorafs/capacity/state`, combinando declarações de provedor e entradas do livro de taxas com Norito JSON determinista.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- A cobertura de validação ejercita a aplicação de identificadores canônicos, detecção de duplicados, limites por via, guardas de atribuição de replicação e verificações de rango de telemetria para que as regressões apareçam no CI de imediato.【crates/sorafs_manifest/src/capacity.rs:792】
- Ferramentas para operadores: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` convertem especificações legíveis em cargas úteis Norito canônicos, blobs base64 e resumos JSON para que os operadores preparem fixtures de `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` e ordens de replicação com validação local.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Os fixtures de referência vivem em `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) e são gerados via `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Integração do plano de controle| Tara | Responsáveis(es) | Notas |
|------|----------------|------|
| Agregar manipuladores Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` com cargas úteis Norito JSON. | Equipe Torii | Refletir a lógica do validador; reutilizar ajudantes Norito JSON. |
| Propaga snapshots de `CapacityDeclarationV1` para metadados do placar do orquestrador e planos de busca do gateway. | GT Ferramentaria / Equipamento de Orquestrador | Extensor `provider_metadata` com referências de capacidade para que a pontuação multifonte respeite limites por pista. |
| Inicie ordens de replicação em clientes de orquestrador/gateway para orientar atribuições e dicas de failover. | Equipe de TL/Gateway de rede | O construtor do placar consome ordens firmadas por governo. |
| CLI de ferramentas: extensor `sorafs_cli` com `capacity declare`, `capacity telemetry`, `capacity orders import`. | GT Ferramentaria | Provar determinista JSON + saídas de placar. |

### 3. Política de mercado e governança

| Tara | Responsáveis(es) | Notas |
|------|----------------|------|
| Ratificar `MarketplacePolicy` (aposta mínima, multiplicadores de penalização, cadência de auditoria). | Conselho de Governança | Publicar em documentos, capturar histórico de revisões. |
| Agregar ganchos de governança para que o Parlamento possa aprovar, renovar e revogar declarações. | Conselho de Governança / Equipe de Contrato Inteligente | Usar eventos Norito + ingestão de manifestos. |
| Implementar o esquema de penalizações (redução de taxas, redução de títulos) vinculado a violações de SLA telegrafiadas. | Conselho de Governança / Tesouraria | Alinear com saídas de liquidação de `DealEngine`. |
| Documente o processo de disputa e a matriz de escalada. | Documentos / Governança | Vinculado ao runbook de disputa + ajudantes de CLI. |

### 4. Medição e distribuição de taxas

| Tara | Responsáveis(es) | Notas |
|------|----------------|------|
| Expanda a ingestão de medição em Torii para aceitar `CapacityTelemetryV1`. | Equipe Torii | Validar GiB-hora, saída PoR, tempo de atividade. |
| Atualizar o pipeline de medição de `sorafs_node` para reportar utilização por pedido + SLA estatístico. | Equipe de armazenamento | Alinear com ordens de replicação e manipuladores de chunker. |
| Pipeline de liquidação: converte telemetria + dados de replicação em pagamentos denominados em XOR, produz listas de currículos para governo e registra o estado do razão. | Equipe de Tesouraria/Armazenagem | Conecte-se com exportações de Deal Engine / Treasury. |
| Exportar painéis/alertas para saúde da medição (backlog de ingestão, telemetria obsoleta). | Observabilidade | Estende o pacote de Grafana referenciado por SF-6/SF-7. |- Torii agora exponha `/v1/sorafs/capacity/telemetry` e `/v1/sorafs/capacity/state` (JSON + Norito) para que os operadores enviem snapshots de telemetria por época e os inspetores recuperem o livro-razão canônico para auditórios ou empaquetado de evidência.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- A integração `PinProviderRegistry` garante que as ordens de replicação sejam acessíveis pelo mesmo endpoint; helpers de CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) agora validam e publicam telemetria de tarefas automatizadas com determinista de hash e resolução de aliases.
- Os snapshots de medição produzem entradas `CapacityTelemetrySnapshot` fijadas no snapshot `metering`, e as exportações Prometheus alimentam a tabela Grafana lista para importar em `docs/source/grafana_sorafs_metering.json` para que os equipamentos de faturação monitorem o acúmulo de GiB-hora, taxas nano-SORA projetadas e cumpridas de SLA em tempo real.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Quando a suavização de medição está habilitada, o snapshot inclui `smoothed_gib_hours` e `smoothed_por_success_bps` para que os operadores comparem valores com EMA frente a contadores brutos que a governança usa para pagamentos.【crates/sorafs_node/src/metering.rs:401】

### 5. Manejo de disputas e revogações

| Tara | Responsáveis(es) | Notas |
|------|----------------|------|
| Defina a carga útil `CapacityDisputeV1` (demandante, evidência, objetivo do provedor). | Conselho de Governança | Esquema Norito + validador. |
| Suporte de CLI para iniciar disputas e responder (com complementos de evidência). | GT Ferramentaria | Garanta o hashing determinista do pacote de evidências. |
| Agregar verificações automatizadas para violações de SLA repetidas vezes (auto-escalado a disputa). | Observabilidade | Guarda-chuvas de alerta e ganchos de governo. |
| Documente o manual de revogação (período de graça, evacuação de dados pinheiros). | Documentos / Equipe de armazenamento | Vinculado a documento de política e runbook de operadores. |

## Requisitos de teste e CI

- Testes unitários para todos os validadores de esquemas novos (`sorafs_manifest`).
- Testes de integração que simulam: declaração -> ordem de replicação -> medição -> pagamento.
- Fluxo de trabalho de CI para regenerar declarações/telemetria de capacidade e garantir que as firmas sejam mantidas sincronizadas (extensor `ci/check_sorafs_fixtures.sh`).
- Testes de carga para API de registro (provedores simulados de 10k, pedidos de 100k).

## Telemetria e painéis

- Painéis do painel:
  - Capacidade declarada vs utilizada pelo provedor.
  - Backlog de ordens de replicação e atraso de atribuição.
  - Cumprimento de SLA (% de tempo de atividade, taxa de saída PoR).
  - Acumulação de taxas e penalizações por época.
- Alertas:
  - Provedor por baixo da capacidade mínima comprometida.
  - Ordem de replicação atascada > SLA.
  - Falha na medição do gasoduto.

## Entregas de documentação- Guia do operador para declarar capacidade, renovar compromissos e monitorar utilização.
- Guia de governo para aprovar declarações, emitir ordens e administrar disputas.
- Referência de API para endpoints de capacidade e formato de ordem de replicação.
- Perguntas frequentes do mercado para desenvolvedores.

## Checklist de prontidão para GA

O item do roteiro **SF-2c** bloqueou o lançamento na produção sobre evidências concretas
sobre contabilidade, gestão de disputas e integração. Use os artefatos seguintes
para manter os critérios de aceitação sincronizados com a implementação.

### Contabilidade noturna e reconciliação XOR
- Exporta o snapshot do estado de capacidade e exporta o ledger XOR para o misma
  ventana, luego ejecuta:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  El helper sale con codigo no cero si hay assentamentos o penalizaciones faltantes/excesivas y
  emita um resumo de Prometheus em formato textfile.
- O alerta `SoraFSCapacityReconciliationMismatch` (en `dashboards/alerts/sorafs_capacity_rules.yml`)
  dispara quando as métricas de reconciliação reportam lacunas; los dashboards viven pt
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Arquivar o currículo JSON e os hashes em `docs/examples/sorafs_capacity_marketplace_validation/`
  junto com os pacotes de governo.

### Evidência de disputa e corte
- Presenta disputas via `sorafs_manifest_stub capacity dispute` (testes:
  `cargo test -p sorafs_car --test capacity_cli`) para manter cargas úteis canônicas.
- Ejecuta `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` e as suítes de
  penalização (`record_capacity_telemetry_penalises_persistent_under_delivery`) para provar que
  disputas e barras são reproduzidas de maneira determinista.
- Siga `docs/source/sorafs/dispute_revocation_runbook.md` para captura de provas e escalada;
  coloque as aprovações de greves no relatório de validação.

### Onboarding de provedores e saída de testes de fumaça
- Regenera artefatos de declaração/telemetria com `sorafs_manifest_stub capacity ...` e
  reejecuta os testes da CLI antes do envio (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Enviados via Torii (`/v1/sorafs/capacity/declare`) e depois captura `/v1/sorafs/capacity/state` mas
  capturas de tela de Grafana. Siga o fluxo de saída em `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Arquivo de artefatos firmados e saídas de reconciliação dentro de
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dependências e sequências

1. Terminar SF-2b (política de admissão) — o mercado depende de provedores validados.
2. Implementar esquema + capacidade de registro (este documento) antes da integração de Torii.
3. Conclua o pipeline de medição antes de ativar os pagamentos.
4. Passo final: habilitar distribuição de taxas controlada pela governança uma vez que os dados
   a medição é verificada na preparação.

O progresso deve ser rastreado no roteiro com referências a este documento. Atualizar
roteiro uma vez que cada seção maior (esquema, plano de controle, integração, medição,
manejo de disputas) alcance estado recurso completo.