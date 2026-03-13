---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-plan-registro
título: Plan de realización Registro de PIN SoraFS
sidebar_label: Registro de PIN del plan
descripción: Plan de realización SF-4, registro de máquina de escribir, modelo Torii, herramientas y datos.
---

:::nota Канонический источник
Esta página está escrita `docs/source/sorafs/pin_registry_plan.md`. Deje copias sincronizadas de todos los documentos que se encuentren activos.
:::

# План реализации Registro de PIN SoraFS (SF-4)

SF-4 contrato de registro de pines y servicios de soporte, códigos de registro
manifiesto de configuración, configuración de pin y API previa para Torii,
шлюзов и оркестраторов. Este documento requiere un plan de validación de contratos
задачами реализации, охватывая on-chain логику, сервисы на стороне хоста,
accesorios y operaciones.

## Область1. **Registro de registro**: записи Norito para manifiestos, alias,
   цепочек преемственности, эпох хранения и метаданных управления.
2. **Realisis del contrato**: operaciones CRUD determinadas para el bienestar
   pin de ciclo (`ReplicationOrder`, `Precommit`, `Completion`, desalojo).
3. **Faso principal**: puntos finales gRPC/REST, operaciones de registro y
   Utilice Torii y SDK, páginas actualizadas y certificados.
4. **Herramientas y accesorios**: ayudantes de CLI, vectores de prueba y documentación para el usuario
   manifiestos de sincronización, alias y sobres de gobernanza.
5. **Metricidad y operaciones**: métricas, alertas y runbooks para el registro de usuarios.

## Модель данных

### Основные записи (Norito)| Estructura | Descripción | Polonia |
|----------|----------|------|
| `PinRecordV1` | Каноническая запись manifiesto. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Alias ​​de Сопоставляет -> Manifiesto CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Las instrucciones para los proveedores eliminan el manifiesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Подтверждение провайдера. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Снимок политики управления. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Ссылка на реализацию: см. `crates/sorafs_manifest/src/pin_registry.rs` para el Norito
на Rust y helpers проверки, которые лежат в основе этих записей. Validación
повторяет herramientas de manifiesto (búsqueda de registro fragmentado, activación de política de pin), чтобы
El contrato, las formas Torii y CLI crean variantes idénticas.

Задачи:
- Cambie las combinaciones Norito en `crates/sorafs_manifest/src/pin_registry.rs`.
- Código de programación (Rust + SDK de software) con las computadoras Norito.
- Retire el documento (`sorafs_architecture_rfc.md`) después de la venta.

## Realización del contrato| Задача | Ответственные | Примечания |
|--------|---------------|-----------|
| Realice el registro de programas (sled/sqlite/off-chain) o un módulo de contacto inteligente. | Equipo central de infraestructura/contrato inteligente | Tenga en cuenta el hash determinado y utilice el punto flotante. |
| Puntos de entrada: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infraestructura básica | Utilice `ManifestValidator` para un plan de validez. El alias de enlace del producto `RegisterPinManifest` (DTO Torii), junto con el `bind_alias` instalado en el plano del día последующих обновлений. |
| Переходы состояния: обеспечивать преемственность (manifiesto A -> B), эпохи хранения, уникальность alias. | Consejo de Gobernanza / Core Infraestructura | Уникальность alias, лимиты хранения и проверки одобрения/вывода предшественников живут в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение многошаговой преемственности у учет репликации остаются открытыми. |
| Parámetros actualizados: descargue `ManifestPolicyV1` en config/состояния управления; разрешить обновления через события управления. | Consejo de Gobierno | Haga clic en CLI para novedades políticas. |
| Emisión de datos: envíe el mensaje Norito para televisores (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilidad | Определить схему событий + registro. |Testimonio:
- Юнит-тесты для каждой punto de entrada (позитивные + отказные сценарии).
- Pruebas de propiedad para las épocas anteriores (sin ciclos, épocas monotonales).
- Fuzz валидации с генерацией случайных manifests (с ограничениями).

## Сервисный фасад (Интеграция Torii/SDK)

| Componente | Задача | Ответственные |
|-----------|--------|---------------|
| Servicios Torii | Экспонировать `/v2/sorafs/pin` (enviar), `/v2/sorafs/pin/{cid}` (búsqueda), `/v2/sorafs/aliases` (lista/enlace), `/v2/sorafs/replication` (pedidos/recibos). Обеспечить пагинацию + фильтрацию. | Redes TL / Core Infraestructura |
| Аттестация | Включать высоту/хэш registro в ответы; Agregue los certificados de estructura Norito, actualmente SDK. | Infraestructura básica |
| CLI | Conecte `sorafs_manifest_stub` o la nueva CLI `sorafs_pin` con `pin submit`, `alias bind`, `order issue`, `registry export`. | Grupo de Trabajo sobre Herramientas |
| SDK | Сгенерировать клиентские enlaces (Rust/Go/TS) из схемы Norito; Realice pruebas de integración. | Equipos SDK |

Operaciones:
- Agregar caché/ETag para obtener puntos finales.
- Previa limitación de velocidad/autenticación en la política Torii.

## Calendario y CI- Accesorios del catálogo: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` хранит подписанные manifiesto/alias/orden de instantáneas, por ejemplo `cargo run -p iroha_core --example gen_pin_snapshot`.
- Muestra CI: `ci/check_sorafs_fixtures.sh` Después de la instantánea y de la diferencia, se sincronizan los accesorios CI.
- Pruebas integrales (`crates/iroha_core/tests/pin_registry.rs`) que permiten acceder a Happy Path además de incluir alias de duplicación, alias de guardias o alias de хранения, identificadores no disponibles chunker, проверку числа реплик и отказы guards преемственности (неизвестные/предодобренные/выведенные/самоссылки); см. кейсы `register_manifest_rejects_*` для деталей покрытия.
- Юнит-тесты теперь покрывают валидацию alias, guards хранения и проверки преемника в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение многошаговой преемственности появится, когда заработает машина состояний.
- Golden JSON para dispositivos instalados en archivos de pago.

## Telemetría y наблюдаемость

Métricas (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Существующая proveedor-telemetría (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) instalado en la conexión de un extremo a otro.

Logotipos:
- Структурированный поток событий Norito для аудиторов управления (¿подписанные?).

Alertas:
- Заказы репликации в ожидании, превышающие SLA.
- Истечение срока alias ниже порога.
- Нарушения хранения (manifiesto не продлен до истечения).Дашборды:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` elimina los totales de los manifiestos del ciclo de vida, alias de descarga, acumulación de pedidos, relación de SLA, latencia de superposiciones frente a holgura y mucho más. пропущенных заказов для de guardia ревью.

## Runbooks y documentación

- Обновить `docs/source/sorafs/migration_ledger.md`, чтобы включить обновления статуса registro.
- Usuario del operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (útil) con métricas, alertas, actualizaciones, copias de seguridad y mantenimiento.
- Руководство по управлению: описать параметры политики, flujo de trabajo одобрения, обработку споров.
- API de aplicación para el punto final (documentos Docusaurus).

## Зависимости и последовательность

1. Завершить задачи плана валидации (integración ManifestValidator).
2. Finalice el conjunto Norito + políticas predeterminadas.
3. Realice el contrato + servicio, coloque el televisor.
4. Перегенерировать los accesorios, запустить интеграционные suite.
5. Actualizar documentos/runbooks y eliminar puntos de la hoja de ruta que se pueden guardar.

El punto de control SF-4 debe estar conectado a este plan de progreso de la tecnología.
La forma REST es una forma de configurar puntos finales certificados:- `GET /v2/sorafs/pin` y `GET /v2/sorafs/pin/{digest}` manifiestan manifiestos с
  enlaces de alias, pedidos de réplicas y certificados de objetos, производным от
  хэша последнего блока.
- `GET /v2/sorafs/aliases` y `GET /v2/sorafs/replication` activos públicos
  alias de catálogo y trabajo pendiente заказов репликации с консистентной пагинацией и
  estado del filtro.

CLI muestra estos datos (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`), los operadores pueden automatizar el registro de auditoría
без обращения к низкоуровневым API.