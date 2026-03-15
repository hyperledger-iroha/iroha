---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-plan-registro
título: Plan de implementación del Registro de PIN de SoraFS
sidebar_label: Registro Plan du Pin
descripción: Plan de implementación SF-4 que cubre la máquina de estados del registro, la fachada Torii, las herramientas y la observabilidad.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/pin_registry_plan.md`. Guarde las dos copias sincronizadas mientras la documentación heredada permanece activa.
:::

# Plan de implementación del Registro de PIN de SoraFS (SF-4)

SF-4 libre de contrato Pin Registry y servicios de aplicación que almacenan les
compromisos de manifiesto, aplicar las políticas de fijación y exponer los
API a Torii, puertas de enlace auxiliares y orquestadores auxiliares. Este documento étend le plan de
validación con los pasos de implementación concreta, couvrant la lógica
on-chain, los servicios côté hôte, los accesorios y las exigencias operativas.

## Portée1. **Máquina de estados de registro**: registros Norito para manifiestos, alias,
   cadenas de sucesión, épocas de retención y metadonnées de gobierno.
2. **Implementación del contrato**: operaciones CRUD determinadas para el ciclo de vida
   des pines (`ReplicationOrder`, `Precommit`, `Completion`, desalojo).
3. **Fachada de servicio**: puntos finales gRPC/REST añadidos al registro consommés por Torii
   y los SDK, con paginación y atestación.
4. **Herramientas y accesorios**: CLI auxiliares, vectores de prueba y documentación para
   guardar manifiestos, alias y sobres de gobierno sincronizados.
5. **Télémétrie et ops**: métricas, alertas y runbooks para la salud del registro.

## Modelo de données

### Registros principales (Norito)| Estructura | Descripción | Campeones |
|--------|-------------|--------|
| `PinRecordV1` | Entrada canónica de manifiesto. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Alias ​​​​del mapa -> CID de manifiesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrucciones para que los proveedores pinent el manifiesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Acusado de recepción del proveedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Instantánea de la política de gobierno. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referencia de implementación: ver `crates/sorafs_manifest/src/pin_registry.rs` para les
esquemas Norito en Rust y los asistentes de validación que supervisan estos registros.
La validación refleja el manifiesto de herramientas (búsqueda del registro fragmentador, activación de políticas de pines)
Para que el contrato, las fachadas Torii y la CLI participen de invariantes idénticas.

Tâches :
- Finalice los esquemas Norito en `crates/sorafs_manifest/src/pin_registry.rs`.
- Generar código (Rust + otros SDK) a través de las macros Norito.
- Mettre à jour la documentación (`sorafs_architecture_rfc.md`) una vez los esquemas en su lugar.## Implementación del contrato| tache | Propietario(s) | Notas |
|-------|----------|-------|
| Implemente el almacenamiento del registro (sled/sqlite/off-chain) o un módulo de contrato inteligente. | Equipo central de infraestructura/contrato inteligente | Si realiza un hash determinado, evitará el flotador. |
| Puntos de entrada: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infraestructura básica | S'appuyer sur `ManifestValidator` du plan de validación. El enlace de alias passe maintenant par `RegisterPinManifest` (DTO Torii expuesto) y el `bind_alias` dejaron el resto anterior para las actualizaciones del día sucesivas. |
| Transiciones de estado: imponer la sucesión (manifiesto A -> B), les époques de rétention, l'unicité des alias. | Consejo de Gobernanza / Core Infraestructura | La unidad de alias, los límites de retención y los controles de aprobación/retiro de los prédécesseurs viven en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; la detección de sucesión de saltos múltiples y la contabilidad de réplica quedan abiertos. |
| Parámetros gubernamentales: cargador `ManifestPolicyV1` después de la configuración/estado de gobierno; permettre les mises à jour via événements de gouvernance. | Consejo de Gobierno | Fournir une CLI pour les mises à jour de politique. |
| Emisión de eventos: emisión de eventos Norito para la televisión (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilidad | Definir el esquema de eventos + registro. |Pruebas :
- Pruebas unitarias para cada punto de entrada (positif + rejet).
- Pruebas de propiedades para la cadena de sucesión (pas de ciclos, épocas monótonas).
- Fuzz de validación en generación de manifiestos aléatoires (bornés).

## Fachada de servicio (Intégración Torii/SDK)

| Componente | tache | Propietario(s) |
|-----------|------|----------|
| Servicio Torii | Expositor `/v2/sorafs/pin` (enviar), `/v2/sorafs/pin/{cid}` (búsqueda), `/v2/sorafs/aliases` (listar/vincular), `/v2/sorafs/replication` (pedidos/recibos). Fournir paginación + filtración. | Redes TL / Core Infraestructura |
| Atestación | Incluir la altura/hash del registro en las respuestas; Agregue una estructura de certificación Norito proporcionada por los SDK. | Infraestructura básica |
| CLI | Tenga en cuenta `sorafs_manifest_stub` o una nueva CLI `sorafs_pin` con `pin submit`, `alias bind`, `order issue`, `registry export`. | Grupo de Trabajo sobre Herramientas |
| SDK | Generar cliente de enlaces (Rust/Go/TS) después del esquema Norito; Agregar pruebas de integración. | Equipos SDK |

Operaciones:
- Agregar un sofá de caché/ETag para los puntos finales GET.
- Fournir rate limiting / auth cohérents avec les politiques Torii.

## Accesorios y CI- Dossier de accesorios: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` stocke des snapshots signés de manifest/alias/order régénérés via `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa CI: `ci/check_sorafs_fixtures.sh` regénère le snapshot et echoue en cas de diff, manteniendo los accesorios CI alineados.
- Pruebas de integración (`crates/iroha_core/tests/pin_registry.rs`) que cubren el camino feliz más el rechazo de alias dupliqué, los guardias de aprobación/rétención, las manijas de fragmentación no concordantes, la validación del recuento de réplicas y las pruebas de guardia de sucesión (pointeurs inconnus/pré-approuvés/retirés/auto-référencés); Consulte el caso `register_manifest_rejects_*` para los detalles de la cobertura.
- Las pruebas unitarias deben mantener la validación de alias, los guardias de retención y las comprobaciones de éxito en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; la detección de sucesión de múltiples saltos asiste a la máquina de estados.
- JSON golden para los eventos utilizados por las tuberías de observación.

## Telemetría y observabilidad

Métricas (Prometheus) :
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- El proveedor de televisión existente (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) permanece en el alcance de los paneles de control de un extremo a otro.

Registros:
- Flux d'événements Norito structures pour les audits de gouvernance (signés ?).Alertas:
- Órdenes de réplica en atención al paso del SLA.
- Expiration d'alias < seuil.
- Violaciones de retención (manifiesto non renouvelé avant expiración).

Paneles de control:
- El JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` se adapta a todos los ciclos de vida de los manifiestos, la cobertura de alias, la saturación del trabajo pendiente, la relación SLA, las superposiciones de latencia frente a slack y los numerosos pedidos pendientes para la revista de guardia.

## Runbooks y documentación

- Mettre à jour de `docs/source/sorafs/migration_ledger.md` para incluir las actualizaciones del estado del registro.
- Guía del operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (déjà publié) couvrant métriques, alerting, déploiement, sauvegarde et flux de reprise.
- Guía de gobierno: describa los parámetros políticos, el flujo de trabajo de aprobación y la gestión de litigios.
- Páginas de referencia API para cada punto final (docs Docusaurus).

## Dependencias y secuenciación

1. Finalizar las tareas del plan de validación (integración ManifestValidator).
2. Finalizar el esquema Norito + defaults de politique.
3. Implementar el contrato + servicio, ramificar la télémétrie.
4. Regénérer les accesorios, ejecuter les suites d'intégration.
5. Mettre à jour docs/runbooks y marque los elementos de la hoja de ruta como completos.

Cada elemento de la lista de verificación SF-4 debe consultar este plan cuando el progreso esté registrado.
La fachada REST muestra los puntos finales del listado certificado:- `GET /v2/sorafs/pin` e `GET /v2/sorafs/pin/{digest}` remiten los manifiestos con
  vinculaciones de alias, órdenes de réplica y un objeto de certificación derivado del hash
  del último bloque.
- `GET /v2/sorafs/aliases` e `GET /v2/sorafs/replication` catálogo de archivos expuestos
  d'alias actif et le backlog des ordres de replication avec una paginación coherente
  et des filtres de statut.

La CLI encapsula estas aplicaciones (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para permitir a los operadores de automatización las auditorías del
registro sin contacto con API auxiliares bajo nivel.