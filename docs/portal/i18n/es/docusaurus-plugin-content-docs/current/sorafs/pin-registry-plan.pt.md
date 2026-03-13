---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-plan-registro
título: Plano de implementación del Registro de PIN del SoraFS
sidebar_label: Registro Plano do Pin
descripción: Plano de implementación SF-4 cobrindo una máquina de estados de registro, una fachada Torii, herramientas y observabilidade.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/pin_registry_plan.md`. Mantenha ambas as copias sincronizadas mientras la documentacao herdada permanezca activa.
:::

# Plano de implementación del Registro de PIN de SoraFS (SF-4)

La entrega del SF-4 o el contrato del Registro Pin y los servicios de apoyo que armazenam
compromisos de manifiesto, impoem politicas de pin y expoem APIs para Torii,
pasarelas y orquestadores. Este documento amplio o plano de validación com
tarefas de implementacao concretas, cobrindo a logica on-chain, os servicos do
host, los accesorios y los requisitos operativos.

##escopo1. **Maquina de estados de registro**: registros definidos por Norito para manifiestos,
   alias, cadeias sucessoras, epocas de retencao e metadados degobernanza.
2. **Implementación del contrato**: operaciones CRUD determinísticas para el ciclo de vida
   dos pines (`ReplicationOrder`, `Precommit`, `Completion`, desalojo).
3. **Fachada de servicio**: endpoints gRPC/REST sustentados por el registro que Torii
   Además de los SDK, se incluyen páginas y pruebas.
4. **Herramientas y accesorios**: ayudantes de CLI, vectores de prueba y documentación para mantener
   manifiestos, alias y sobres de gobierno sincronizados.
5. **Telemetría y operaciones**: métricas, alertas y runbooks para el registro.

## Modelo de dados

### Registros centrales (Norito)| Estructura | Descripción | Campos |
|--------|-----------|--------|
| `PinRecordV1` | Entrada canónica de manifiesto. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Alias ​​de Mapeia -> CID de manifiesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrucciones para que los proveedores fijen el manifiesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Confirmacao do proveedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Instantánea de política de gobierno. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referencia de implementación: veja `crates/sorafs_manifest/src/pin_registry.rs` para el sistema
esquemas Norito en Rust y los ayudantes de validación que sustentan estos registros. un
validacao espelha o tooling de manifest (búsqueda en el registro fragmentado, activación de políticas de pines)
para que o contrato, como fachadas Torii e a CLI compartilhem invariantes identicas.

Tarefas:
- Finalizar los esquemas Norito en `crates/sorafs_manifest/src/pin_registry.rs`.
- Generar código (Rust + otros SDK) usando macros Norito.
- Actualizar la documentación (`sorafs_architecture_rfc.md`) cuando los esquemas se activan pronto.

## Implementación del contrato| Tarefa | Responsavel(es) | Notas |
|--------|-----------------|-------|
| Implementar armamento de registro (sled/sqlite/off-chain) o módulo de contrato inteligente. | Equipo central de infraestructura/contrato inteligente | Fornecer hash determinístico, evitar ponto flutuante. |
| Puntos de entrada: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infraestructura básica | Ampliar `ManifestValidator` do plano de validación. O vinculante de alias agora flui via `RegisterPinManifest` (DTO do Torii) mientras `bind_alias` dedicado segue planeado para actualizaciones sucesivas. |
| Transicoes de estado: impor sucessao (manifiesto A -> B), epocas de retencao, unicidade de alias. | Consejo de Gobernanza / Core Infraestructura | Unicidade de alias, limites de retencao e checks de aprovacao/retirada de predecesores vivem em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a deteccao de sucessao multi-hop y o bookkeeping de replicacao permanecem em aberto. |
| Parámetros gobernados: cargar `ManifestPolicyV1` de config/estado de gobierno; permitir actualizaciones a través de eventos de gobierno. | Consejo de Gobierno | Fornecer CLI para actualización política. |
| Emissão de eventos: emite eventos Norito para telemetría (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilidad | Definir esquema de eventos + logging. |Testículos:
- Testes unitarios para cada punto de entrada (positivo + rejeicao).
- Testes de propriedades para a cadeia de sucessao (sem ciclos, epocas monotonicamente crescentes).
- Fuzz de validacao gerando manifests aleatorios (limitados).

## Fachada de servicio (Integracao Torii/SDK)

| Componente | Tarefa | Responsavel(es) |
|------------|--------|-----------------|
| Servico Torii | Export `/v2/sorafs/pin` (enviar), `/v2/sorafs/pin/{cid}` (búsqueda), `/v2/sorafs/aliases` (listar/vincular), `/v2/sorafs/replication` (pedidos/recibos). Fornecer paginacao + filtragem. | Redes TL / Core Infraestructura |
| Atestacao | Incluir altura/hash do registro nas respuestas; Agregar estructura de atestacao Norito consumida pelos SDK. | Infraestructura básica |
| CLI | Coloque `sorafs_manifest_stub` o una nueva CLI `sorafs_pin` con `pin submit`, `alias bind`, `order issue`, `registry export`. | Grupo de Trabajo sobre Herramientas |
| SDK | Generar fijaciones de cliente (Rust/Go/TS) a partir del esquema Norito; Adicionar testes de integracao. | Equipos SDK |

Óperas:
- Agregar camada de caché/ETag para puntos finales GET.
- Fornecer rate limiting / auth consistentes com as politicas do Torii.

## Calendario y CI- Directorio de accesorios: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` armazena snapshots assinados de manifest/alias/order regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa de CI: `ci/check_sorafs_fixtures.sh` regenera o snapshot y falha se houver diffs, manteniendo los accesorios de CI alinhados.
- Testes de integracao (`crates/iroha_core/tests/pin_registry.rs`) exercitam o fluxo feliz mais a rejeicao de alias duplicado, guards de aprovacao/retencao de alias, handles de chunker incompativeis, validacao de contagem de replicas e falhas de guardas de sucessao (ponteiros desconhecidos/preaprovados/retirados/autorreferencias); veja casos `register_manifest_rejects_*` para detalles de cobertura.
- Testes unitarios agora cobrem validacao de alias, guards de retencao e checks de sucessor em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a deteccao de sucessao multi-hop quando a maquina de estados chegar.
- JSON golden para eventos usados ​​pelos pipelines de observabilidade.

## Telemetría y observabilidad

Métricas (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- La telemetría existente de proveedores (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) permanece en el alcance de los paneles de control de un extremo a otro.

Registros:
- Stream de eventos Norito estruturados para auditorias degobernanza (assinados?).

Alertas:
- Órdenes de replicación pendientes excedendo o SLA.
- Expiracao de alias abaixo do limiar.
- Violacoes de retencao (manifiesto nao renovado antes de expirar).Paneles de control:
- El JSON de Grafana `docs/source/grafana_sorafs_pin_registry.json` rastrea todo el ciclo de vida de dos manifiestos, cobertura de alias, saturación del trabajo pendiente, raza de SLA, superposiciones de latencia vs slack y taxas de órdenes perdidas para revisión de guardia.

## Runbooks y documentación

- Actualizar `docs/source/sorafs/migration_ledger.md` para incluir actualizaciones de estado del registro.
- Guía del operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ja publicado) cobrindo métricas, alertas, despliegue, respaldo y flujos de recuperación.
- Guía de gobernanza: descripción de parámetros de política, flujo de trabajo de aprobación, tratamento de disputas.
- Páginas de referencia de API para cada endpoint (docs Docusaurus).

## Dependencias y secuenciación

1. Completar tarefas del plano de validación (integración del ManifestValidator).
2. Finalizar esquema Norito + defaults de politica.
3. Implementar contrato + servicio, conectar telemetria.
4. Regenerar accesorios, rodar suites de integracao.
5. Actualizar documentos/runbooks y marcar elementos de la hoja de ruta como completos.

Cada lista de verificación del SF-4 debe hacer referencia a este plano cuando usted progresa.
A fachada REST ahora entrega endpoints de listagem com atestacao:- `GET /v2/sorafs/pin` e `GET /v2/sorafs/pin/{digest}` retorno de manifiestos com
  enlaces de alias, órdenes de replicación y un objeto de atestacao derivado del
  hash do ultimo bloco.
- `GET /v2/sorafs/aliases` e `GET /v2/sorafs/replication` exposición o catálogo de
  alias ativo e o backlog de órdenes de replicacao com paginacao consistente e
  filtros de estado.

Una CLI encapsula esas chamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que los operadores puedan automatizar auditorias
registro sin tocar API de bajo nivel.