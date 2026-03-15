---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de registro de pinos
título: Plano de implementação do Pin Registry de SoraFS
sidebar_label: Registro Plan du Pin
descrição: Plano de implementação SF-4 cobrindo a máquina de registro, a fachada Torii, as ferramentas e a observabilidade.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/pin_registry_plan.md`. Garanta as duas cópias sincronizadas para que a documentação herdada esteja ativa.
:::

# Plano de implementação do Pin Registry de SoraFS (SF-4)

SF-4 libera o contrato Pin Registry e os serviços de aplicativo que você armazena
compromissos de manifesto, aplicação de políticas de fixação e exposição de
API para Torii, gateways auxiliares e orquestradores auxiliares. Este documento estende o plano de
validação com técnicas de implementação concretas, cobrindo a lógica
on-chain, serviços de hospedagem, instalações e exigências operacionais.

## Portée

1. **Máquina de status de registro**: registros Norito para manifestos, aliases,
   cadeias de sucessão, épocas de retenção e metas de governo.
2. **Implementação do contrato**: operações CRUD determinadas para o ciclo de vida
   des pinos (`ReplicationOrder`, `Precommit`, `Completion`, despejo).
3. **Façade de serviço**: endpoints gRPC/REST adicionados ao registro consumido por Torii
   e os SDKs, com paginação e atestado.
4. **Ferramentas e acessórios**: CLI auxiliar, vetores de teste e documentação para
   Garder manifestos, pseudônimos e envelopes de governo sincronizados.
5. **Telemetria e operações**: métricas, alertas e runbooks para a saúde do registro.

## Modèle de données

### Registros principais (Norito)

| Estrutura | Descrição | Campeões |
|--------|-------------|--------|
| `PinRecordV1` | Entrada canônica de manifesto. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Alias ​​do mapa -> CID do manifesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrução para que os provedores pintem o manifesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Acusado de recepção do provedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Instantâneo da política de governo. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referência de implementação: veja `crates/sorafs_manifest/src/pin_registry.rs` para os
esquemas Norito em Rust e os auxiliares de validação que sustentam esses registros.
A validação reflete o manifesto de ferramentas (pesquisa de registro de chunker, controle de política de pin)
para o contrato, as fachadas Torii e a CLI compartilham invariantes idênticas.Taches:
- Finalize os esquemas Norito em `crates/sorafs_manifest/src/pin_registry.rs`.
- Gere o código (Rust + outros SDKs) por meio das macros Norito.
- Mantenha a documentação (`sorafs_architecture_rfc.md`) atualizada e os esquemas no local.

## Implementação do contrato

| Tache | Proprietário(s) | Notas |
|-------|----------|-------|
| Implemente o armazenamento de registro (sled/sqlite/off-chain) ou um módulo de contrato inteligente. | Equipe Core de Infra/Contrato Inteligente | Forneça um hash determinado e evite a flutuação. |
| Pontos de entrada: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infra principal | Toque em `ManifestValidator` do plano de validação. A ligação do alias foi mantida por `RegisterPinManifest` (DTO Torii exposta) e `bind_alias` foi restaurada antes das mises dos dias sucessivos. |
| Transições de Estado: impor a sucessão (manifesto A -> B), épocas de retenção, unicidade de apelidos. | Conselho de Governança / Core Infra | A unicidade de aliases, os limites de retenção e as verificações de aprovação/retirada dos prédécesseurs existentes em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a detecção de sucessão multi-hop e a contabilidade de replicação permanecem abertas. |
| Parâmetros governamentais: carregador `ManifestPolicyV1` a partir da configuração/estado de governo; permitir as misérias do dia através de eventos de governo. | Conselho de Governança | Fornecer uma CLI para as misérias do dia a dia político. |
| Emissão de eventos: emissão de eventos Norito para a televisão (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilidade | Defina o esquema de eventos + registro. |

Testes:
- Testes unitários para cada ponto de entrada (positivo + rejeição).
- Testes de propriedades para a cadeia de sucessão (pas de ciclos, épocas monótonas).
- Fuzz de validação em geração de manifestos aleatórios (nascidos).

## Fachada de serviço (Integração Torii/SDK)

| Composto | Tache | Proprietário(s) |
|-----------|------|----------|
| Serviço Torii | Expositor `/v1/sorafs/pin` (enviar), `/v1/sorafs/pin/{cid}` (pesquisa), `/v1/sorafs/aliases` (listar/vincular), `/v1/sorafs/replication` (pedidos/recebimentos). Paginação Fournir + filtração. | Rede TL / Core Infra |
| Atestado | Inclua a hauteur/hash do registro nas respostas; adicione uma estrutura de atestado Norito fornecida pelos SDKs. | Infra principal |
| CLI | Use `sorafs_manifest_stub` ou uma nova CLI `sorafs_pin` com `pin submit`, `alias bind`, `order issue`, `registry export`. | GT Ferramentaria |
| SDK | Gerar vinculações do cliente (Rust/Go/TS) a partir do esquema Norito ; adicionar testes de integração. | Equipes SDK |

Operações:
- Adiciona um cache/ETag para endpoints GET.
- Limitação de taxa / autenticação coerente com as políticas Torii.

## Luminárias e CI- Dossiê de fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` armazena snapshots assinados por manifesto/alias/pedido registrados via `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa CI: `ci/check_sorafs_fixtures.sh` gera o instantâneo e o eco em caso de diferença, mantendo os equipamentos CI alinhados.
- Testes de integração (`crates/iroha_core/tests/pin_registry.rs`) cobrem o caminho feliz mais a rejeição de alias duplicados, os guardas de aprovação/retenção, os identificadores de blocos não concordantes, a validação da conta de réplicas e as verificações de garde de sucessão (ponteiros inconnus/pré-approuvés/retirés/auto-référencés); veja o caso `register_manifest_rejects_*` para os detalhes da cobertura.
- Os testes unitários mantêm a validação do alias, as proteções de retenção e as verificações do sucessor em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; a detecção de sucessão multi-hop atende à máquina de estados.
- JSON dourado para eventos utilizados pelos pipelines de observabilidade.

## Telemetria e observabilidade

Métricas (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- O provedor de telefonia existente (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) permanece no escopo dos painéis de ponta a ponta.

Registros:
- Fluxos de eventos Norito estruturados para auditorias de governo (assinaturas?).

Alertas:
- Ordens de replicação atentas ao SLA.
- Expiração do alias < seuil.
- Violações de retenção (não renovações manifestadas antes do vencimento).

Painéis:
- Le JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` atende a todos os ciclos de vida dos manifestos, à cobertura de alias, à saturação do backlog, à relação SLA, às sobreposições de latência vs folga e às taxas de ordem perdidas para a revista de plantão.

## Runbooks e documentação

- Mettre a jour `docs/source/sorafs/migration_ledger.md` para incluir as mises no dia do status do registro.
- Guia do operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (déjà publié) cobrindo métricas, alertando, implementação, segurança e fluxo de reprise.
- Guia de governança: descreve os parâmetros políticos, o fluxo de trabalho de aprovação e o gerenciamento de litígios.
- API de páginas de referência para cada endpoint (docs Docusaurus).

## Dependências e sequência

1. Termine as etapas do plano de validação (integração ManifestValidator).
2. Finaliser le schéma Norito + defaults de politique.
3. Implemente o contrato + serviço, ramalize a telefonia.
4. Regenere os equipamentos e execute as suítes de integração.
5. Insira novos documentos/runbooks e marque os itens do roteiro como completos.

Cada item da lista de verificação SF-4 deve consultar este plano quando o progresso for registrado.
A fachada REST livre desordenada dos endpoints da listagem atesta:

- `GET /v1/sorafs/pin` e `GET /v1/sorafs/pin/{digest}` reenvio de manifestos com
  ligações de alias, ordens de replicação e um objeto de atestado derivado do hash
  du dernier bloco.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` expõem o catálogo
  o alias ativo e o backlog das ordens de replicação com uma paginação coerente
  e filtros de status.O encapsulamento CLI desses aplicativos (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para permitir que os operadores automatizem as auditorias
registro sem toucher aux APIs no nível básico.