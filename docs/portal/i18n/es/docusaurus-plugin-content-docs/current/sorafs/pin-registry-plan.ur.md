---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-plan-registro
título: SoraFS Registro de PIN نفاذی منصوبہ
sidebar_label: Registro de PIN منصوبہ
descripción: SF-4 نفاذی منصوبہ جو registro کی máquina de estado, Torii fachada, herramientas اور observabilidad کو کور کرتا ہے۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہیں دونوں نقول ہم آہنگ رکھیں۔
:::

# SoraFS Registro de PIN نفاذی منصوبہ (SF-4)

Registro de pines SF-4
políticas de pin نافذ کرتی ہیں، اور Torii, gateways اور Orchestrators کے لیے API ظاہر کرتی ہیں۔
یہ دستاویز plan de validación کو ٹھوس tareas de implementación سے بڑھاتی ہے، جس میں lógica en cadena،
servicios del lado del host, accesorios, اور عملیاتی تقاضے شامل ہیں۔

## دائرہ کار1. **máquina de estado del registro**: registros definidos por Norito, manifiestos, alias, cadenas sucesoras,
   épocas de retención, y metadatos de gobernanza.
2. **کنٹریکٹ نفاذ**: ciclo de vida del pin کے لیے operaciones CRUD deterministas (`ReplicationOrder`,
   `Precommit`, `Completion`, desalojo).
3. **Fachada**: puntos finales gRPC/REST, registro, software de instalación de Torii y SDKs de software.
   جن میں paginación اور atestación شامل ہے۔
4. **herramientas y accesorios**: ayudantes de CLI, vectores de prueba, documentación y manifiestos, alias y
   sobres de gobernanza ہم آہنگ رہیں۔
5. **operaciones y operaciones de telemetría**: registros, métricas, alertas y runbooks.

## ڈیٹا ماڈل

### بنیادی ریکارڈز (Norito)| Estructura | وضاحت | فیلڈز |
|--------|-------|-------|
| `PinRecordV1` | entrada manifiesta canónica. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | alias -> mapeo CID manifiesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | proveedores کو pin de manifiesto کرنے کی ہدایت. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | reconocimiento del proveedor. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Panorama de la política de gobernanza. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referencia de implementación: `crates/sorafs_manifest/src/pin_registry.rs` Esquemas de Rust Norito
اور ayudantes de validación موجود ہیں۔ Herramientas de manifiesto de validación (búsqueda de registro de fragmentos, activación de políticas de pin)
کو آئینہ کرتی ہے تاکہ کنٹریکٹ، Torii fachadas, اور CLI ایک جیسے invariantes شیئر کریں۔

Tareas:
- `crates/sorafs_manifest/src/pin_registry.rs` میں Norito esquemas مکمل کریں۔
- Las macros Norito generan código کریں (Rust + دیگر SDK) ۔
- esquemas آ جانے کے بعد docs (`sorafs_architecture_rfc.md`) اپڈیٹ کریں۔

## کنٹریکٹ نفاذ| کام | مالک/مالکان | نوٹس |
|-----|-------------|------|
| almacenamiento de registro (sled/sqlite/off-chain) یا módulo de contrato inteligente نافذ کریں۔ | Equipo central de infraestructura/contrato inteligente | hash determinista فراہم کریں، punto flotante سے بچیں۔ |
| Puntos de entrada: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infraestructura básica | plan de validación سے `ManifestValidator` استعمال کریں۔ enlace de alias اب `RegisterPinManifest` (Torii DTO) سے گزرتی ہے جبکہ مخصوص `bind_alias` آئندہ اپڈیٹس کے لیے منصوبہ بند ہے۔ |
| Transiciones de estado: sucesión (manifiesto A -> B), épocas de retención, alias unicidad نافذ کریں۔ | Consejo de Gobernanza / Core Infraestructura | unicidad de alias, límites de retención, verificaciones de aprobación/retiro del predecesor en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں ہیں؛ detección de sucesión de múltiples saltos y contabilidad de replicación ابھی باقی ہیں۔ |
| Parámetros gobernados: `ManifestPolicyV1` کو config/governance state سے لوڈ کریں؛ eventos de gobernanza کے ذریعے اپڈیٹس کی اجازت دیں۔ | Consejo de Gobierno | actualizaciones de políticas کے لیے CLI فراہم کریں۔ |
| Emisión de eventos: telemetría کے لیے Eventos Norito (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`) جاری کریں۔ | Observabilidad | esquema de evento + registro متعین کریں۔ |Pruebas:
- ہر punto de entrada کے لیے pruebas unitarias (positivas + rechazo).
- cadena de sucesión کے لیے pruebas de propiedad (sin ciclos, épocas monótonas).
- manifiestos aleatorios (limitados) y validación difusa.

## fachada exterior (Torii/SDK انضمام)

| جزو | کام | مالک/مالکان |
|------|-----|-------------|
| Torii Servicio | `/v1/sorafs/pin` (enviar), `/v1/sorafs/pin/{cid}` (búsqueda), `/v1/sorafs/aliases` (listar/enlazar), `/v1/sorafs/replication` (pedidos/recibos) فراہم کریں۔ paginación + filtrado مہیا کریں۔ | Redes TL / Core Infraestructura |
| Atestación | respuestas میں altura del registro/hash شامل کریں؛ Norito estructura de atestación شامل کریں جسے Los SDK consumen کریں۔ | Infraestructura básica |
| CLI | `sorafs_manifest_stub` Tarjeta gráfica de usuario `sorafs_pin` Tarjeta gráfica CLI `pin submit`, `alias bind`, `order issue`, `registry export` ہو۔ | Grupo de Trabajo sobre Herramientas |
| SDK | El esquema Norito y los enlaces de cliente (Rust/Go/TS) generan کریں؛ pruebas de integración شامل کریں۔ | Equipos SDK |

Operaciones:
- OBTENER puntos finales کے لیے caché/capa ETag شامل کریں۔
- Políticas Torii کے مطابق limitación de velocidad / autenticación فراہم کریں۔

## Calendario de CI- Directorio de accesorios: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` میں manifiesto firmado/alias/instantáneas de orden محفوظ ہوتے ہیں جو `cargo run -p iroha_core --example gen_pin_snapshot` سے regenerar ہوتے ہیں۔
- Paso CI: regeneración de instantánea `ci/check_sorafs_fixtures.sh` کرتا ہے اور diff ہونے پر fail کرتا ہے تاکہ Accesorios CI alineados رہیں۔
- Pruebas de integración (`crates/iroha_core/tests/pin_registry.rs`) camino feliz کے ساتھ rechazo de alias duplicados, protección de aprobación/retención de alias, identificadores de fragmentación no coincidentes, validación de recuento de réplicas, fallas de protección de sucesión (desconocido/preaprobado/retirado/autopunteros) کور کرتے ہیں؛ تفصیل کے لیے `register_manifest_rejects_*` casos دیکھیں۔
- Pruebas unitarias اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں validación de alias, guardias de retención, اور comprobaciones de sucesor کور کرتے ہیں؛ detección de sucesión de saltos múltiples تب آئے گا جب máquina de estado دستیاب ہوگی۔
- Canalizaciones de observabilidad کے لیے eventos JSON dorados ۔

## Telemetría y observabilidad

Métricas (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Telemetría del proveedor de موجودہ (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) paneles de control de extremo a extremo کے لیے alcance میں رہے گی۔

Registros:
- auditorías de gobernanza کے لیے flujo de eventos Norito estructurado (¿firmado?).

Alertas:
- SLA سے زیادہ órdenes de replicación pendientes.
- umbral de caducidad del alias سے کم.
- violaciones de retención (renovación manifiesta وقت سے پہلے نہ ہو).Paneles de control:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` Totales del ciclo de vida del manifiesto, cobertura de alias, saturación de trabajos pendientes, relación SLA, latencia frente a superposiciones de holgura, tasas de pedidos perdidos y revisión de guardia.

## Runbooks y documentación

- `docs/source/sorafs/migration_ledger.md` کو actualizaciones de estado del registro شامل کرنے کے لیے اپڈیٹ کریں۔
- Guía del operador: `docs/source/sorafs/runbooks/pin_registry_ops.md` (اب شائع شدہ) métricas, alertas, implementación, copia de seguridad, flujos de recuperación کور کرتا ہے۔
- Guía de gobernanza: parámetros de políticas, flujo de trabajo de aprobación, manejo de disputas.
- ہر endpoint کے لیے Páginas de referencia de API (documentos Docusaurus).

## Dependencias y Secuenciación

1. Tareas del plan de validación مکمل کریں (integración de ManifestValidator).
2. Esquema Norito + valores predeterminados de política کو حتمی بنائیں۔
3. contrato + servicio نافذ کریں اور cable de telemetría کریں۔
4. Los dispositivos se regeneran کریں اور suites de integración چلائیں۔
5. documentos/runbooks اپڈیٹ کریں اور elementos de la hoja de ruta کو مکمل مارک کریں۔

SF-4 کے ہر lista de verificación آئٹم میں پیش رفت پر اس منصوبے کا حوالہ ہونا چاہیے۔
Fachada REST اب listado de puntos finales certificado کے ساتھ آتی ہے:

- `GET /v1/sorafs/pin` اور `GET /v1/sorafs/pin/{digest}` manifiesta واپس کرتے ہیں جن میں
  enlaces de alias, órdenes de replicación, اور تازہ ترین bloque hash سے ماخوذ objeto de atestación شامل ہے۔
- `GET /v1/sorafs/aliases` اور `GET /v1/sorafs/replication` فعال alias catálogo اور
  acumulación de pedidos de replicación کو paginación consistente اور filtros de estado کے ساتھ ظاہر کرتے ہیں۔CLI y llamadas کو wrap کرتی ہے (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) تاکہ operadores کم سطحی API کو چھوئے بغیر auditorías de registro خودکار بنا سکیں۔