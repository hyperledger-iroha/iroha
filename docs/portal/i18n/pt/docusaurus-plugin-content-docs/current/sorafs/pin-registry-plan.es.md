---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de registro de pinos
título: Plano de implementação do Pin Registry de SoraFS
sidebar_label: Registro do Plano del Pin
descrição: Plano de implementação SF-4 que cobre a maquina de estados do registro, a fachada Torii, ferramental e observabilidade.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/pin_registry_plan.md`. Mantenha ambas as cópias sincronizadas enquanto a documentação herdada segue ativa.
:::

# Plano de implementação do Pin Registry de SoraFS (SF-4)

SF-4 entrega o contrato do Pin Registry e os serviços de suporte que armazena
compromissos de manifesto, cumplir políticas de pin e expor APIs a Torii, gateways
e orquestradores. Este documento amplia o plano de validação com tarefas de
implementação concreta, cobrindo a lógica on-chain, os serviços do host, os
fixtures e os requisitos operacionais.

## Alcance

1. **Máquina de estados do registro**: registros definidos por Norito para manifestos,
   aliases, cadenas sucessoras, épocas de retenção e metadados de governança.
2. **Implementação do contrato**: operações CRUD deterministas para o ciclo de vida
   de pinos (`ReplicationOrder`, `Precommit`, `Completion`, despejo).
3. **Fachada de serviço**: endpoints gRPC/REST respaldados pelo registro que consome
   Torii e os SDKs, incluindo paginação e atestado.
4. **Ferramentas e acessórios**: auxiliares de CLI, vetores de teste e documentação para manutenção
   manifestos, aliases e envelopes de governança sincronizados.
5. **Telemetria e operações**: métricas, alertas e runbooks para a saúde do registro.

## Modelo de dados

### Registros centrais (Norito)

| Estrutura | Descrição | Campos |
|------------|-------------|--------|
| `PinRecordV1` | Entrada canônica do manifesto. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Alias ​​do Mapea -> CID do manifesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruções para que os provedores fixem o manifesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Acusação de recibo do provedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Instantâneo da política de governo. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referência de implementação: versão `crates/sorafs_manifest/src/pin_registry.rs` para los
esquemas Norito en Rust e os auxiliares de validação que respaldam esses registros. La
validação reflete o conjunto de ferramentas do manifesto (pesquisa do registro do chunker, controle de política de pin)
para o contrato, as fachadas Torii e o CLI compartilham invariantes idênticas.Taras:
- Finalize os esquemas Norito em `crates/sorafs_manifest/src/pin_registry.rs`.
- Gerar código (Rust + outros SDKs) usando macros Norito.
- Atualize a documentação (`sorafs_architecture_rfc.md`) uma vez que os esquemas estejam listados.

## Implementação do contrato

| Tara | Responsáveis(es) | Notas |
|------|----------------|------|
| Implementar armazenamento de registro (sled/sqlite/off-chain) ou módulo de contrato inteligente. | Equipe Core de Infra/Contrato Inteligente | Prove o hashing determinista, evite punto flutuante. |
| Pontos de entrada: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infra principal | Aprovechar `ManifestValidator` do plano de validação. A ligação do alias agora flui via `RegisterPinManifest` (DTO de Torii) enquanto `bind_alias` dedicado segue planejado para atualizações sucessivas. |
| Transições de estado: sucessão imponente (manifesto A -> B), épocas de retenção, unicidade de alias. | Conselho de Governança / Core Infra | Unicidade de alias, limites de retenção e verificações de aprovação/retirada de antecessores vividos em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a detecção de sucessão multi-hop e a contabilidad de replicação são abertas. |
| Parâmetros governamentais: carregar `ManifestPolicyV1` desde config/estado de governo; permitir atualizações via eventos de governo. | Conselho de Governança | Proveer CLI para atualizações políticas. |
| Emissão de eventos: emitir eventos Norito para telemetria (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilidade | Definir esquema de eventos + registro. |

Pruebas:
- Testes unitários para cada ponto de entrada (positivo + rechazo).
- Testes de propriedades para a cadeia de sucessão (sem ciclos, épocas monotonicamente crecientes).
- Fuzz de validacion gerando manifestos aleatórios (acotados).

## Fachada de serviço (Integração Torii/SDK)

| Componente | Tara | Responsáveis(es) |
|-----------|-------|----------------|
| Serviço Torii | Exponer `/v1/sorafs/pin` (enviar), `/v1/sorafs/pin/{cid}` (pesquisa), `/v1/sorafs/aliases` (listar/vincular), `/v1/sorafs/replication` (pedidos/recebimentos). Provar paginação + filtrado. | Rede TL / Core Infra |
| Atestação | Incluir altura/hash do registro nas respostas; adicionar estrutura de atestado Norito consumida pelos SDKs. | Infra principal |
| CLI | Extensor `sorafs_manifest_stub` ou um novo CLI `sorafs_pin` com `pin submit`, `alias bind`, `order issue`, `registry export`. | GT Ferramentaria |
| SDK | Gerar ligações de cliente (Rust/Go/TS) a partir do esquema Norito; agregar testes de integração. | Equipes SDK |

Operações:
- Agregar capacidade de cache/ETag para endpoints GET.
- Provar limitação de taxa / autenticação consistente com as políticas de Torii.

## Luminárias e CI- Diretório de fixtures: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` guarda snapshots firmados de manifest/alias/order regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Paso de CI: `ci/check_sorafs_fixtures.sh` regenera o snapshot e falha se houver diferenças, mantendo os fixtures de CI alineados.
- Testes de integração (`crates/iroha_core/tests/pin_registry.rs`) ejercitam o fluxo feliz, mas o rechazo de alias duplicado, guardas de aprovação/retenção de alias, alças de chunker desalineados, validação de conteúdo de réplicas e falhas de guardas de sucessão (punteros desconhecidos/pré-aprovados/retirados/autorreferencias); ver casos `register_manifest_rejects_*` para detalhes de cobertura.
- Testes unitários agora cobrem validação de alias, guardas de retenção e verificações de sucessor em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a detecção de sucessão multi-hop quando a máquina de estados é acionada.
- JSON golden para eventos usados ​​por pipelines de observabilidade.

## Telemetria e observabilidade

Métricas (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- A telemetria existente dos provedores (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) segue em alcance para painéis de controle ponta a ponta.

Registros:
- Stream de eventos Norito estruturados para auditórios de governo (firmados?).

Alertas:
- Ordens de replicação pendentes excedendo o SLA.
- Expiração de alias por baixo do umbral.
- Violações de retenção (manifestadas sem renovação antes de expirar).

Painéis:
- O JSON de Grafana `docs/source/grafana_sorafs_pin_registry.json` rastreia total de ciclo de vida de manifestos, cobertura de alias, saturação de backlog, proporção de SLA, sobreposições de latência vs folga e tarefas de ordens perdidas para revisão de plantão.

## Runbooks e documentação

- Atualizar `docs/source/sorafs/migration_ledger.md` para incluir atualizações de estado do registro.
- Guia de operadores: `docs/source/sorafs/runbooks/pin_registry_ops.md` (já publicado) cubriendo métricas, alertas, despliegue, backup e fluxos de recuperação.
- Guia de governança: descreve parâmetros políticos, fluxo de trabalho de aprovação, manejo de disputas.
- Páginas de referência de API para cada endpoint (documentos Docusaurus).

## Dependências e sequências

1. Completar tarefas do plano de validação (integração do ManifestValidator).
2. Finalizar esquema Norito + padrões de política.
3. Implementar contrato + serviço, conectar telemetria.
4. Regenerar fixtures, executar suítes de integração.
5. Atualize documentos/runbooks e marque itens do roadmap como completos.

Cada lista de verificação do SF-4 deve referenciar este plano à medida que progride.
La fachada REST agora entrega endpoints de listado com atestado:

- `GET /v1/sorafs/pin` e `GET /v1/sorafs/pin/{digest}` devemuelven manifesta-se con
  ligações de alias, ordens de replicação e um objeto de atestação derivado de
  hash do último bloco.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` expõem o catálogo de
  alias ativo e o backlog de ordens de replicação com paginação consistente e
  filtros de estado.O ambiente CLI é chamado (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que os operadores possam automatizar auditorias do
registro sem tocar APIs de baixo nível.