---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
כותרת: Plan de implementacion del Pin Registry de SoraFS
sidebar_label: Plan del Pin Registry
תיאור: Plan de implementacion SF-4 que cubre la maquina de estados del registry, la fachada Torii, tooling y observabilidad.
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/sorafs/pin_registry_plan.md`. Manten ambas copias sincronizadas mientras la documentacion heredada siga active.
:::

# תוכנית היישום של Pin Registry de SoraFS (SF-4)

SF-4 entrega el contrato del Pin Registry y los servicios de soporte que almacenan
compromisos de manifest, hasen cumplir politicas de pin y exponen APIs a Torii, gateways
y orquestadores. Este documento amplia el plan de validacion con tareas de
implementacion concretas, cubriendo la logica on-chain, los servicios del host, los
מתקנים y los requisitos operativos.

## אלקנס

1. **Maquina de estados del registry**: registros definidos por Norito para manifests,
   כינויים, קדנות המשך, אפוקאס דה שימור ו-metadatos de gobernanza.
2. **Implementacion del contrato**: operaciones CRUD deterministas para el ciclo de vida
   de pins (`ReplicationOrder`, `Precommit`, `Completion`, פינוי).
3. **Fachada de servicio**: נקודות קצה gRPC/REST respaldados por el registry que consumen
   Torii y los SDKs, inluyendo pagecion y atestacion.
4. **מכשירי כלי עבודה**: עוזרים של CLI, וקטורים של חפצים ומסמכים למטרות
   מניפסטים, כינויים y envelopes de gobernanza sincronizados.
5. **Telemetria y Ops**: metricas, alertas y runbooks para la salud del registry.

## מודל נתונים

### Registros centrales (Norito)

| אסטרוקטורה | תיאור | קמפוס |
|------------|----------------|--------|
| `PinRecordV1` | Entrada canonica de manifest. | I18NIS `governance_envelope_hash`. |
| `AliasBindingV1` | כינוי Mapea -> CID de manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruccio para que los providers pinneen el manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | הנחה של ספק. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | תמונת מצב של פוליטיקה דה גוברננסה. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

רפרנס ליישום: ver `crates/sorafs_manifest/src/pin_registry.rs` para los
esquemas Norito en Rust y los helpers de validacion que respaldan Estos registros. לה
validacion refleja el tooling de manifest (חיפוש של chunker registry, pin policy gating)
para que el contrato, las fachadas Torii y la CLI compartan invariantes identicas.טאראס:
- Finalizar los esquemas Norito en `crates/sorafs_manifest/src/pin_registry.rs`.
- קודיגו גנרי (Rust + otros SDKs) usando מאקרו Norito.
- Actualizar la Documentacion (`sorafs_architecture_rfc.md`) una vez que los esquemas esten listos.

## Implementacion del contrato

| טארא | אחראי(ים) | Notas |
|------|----------------|------|
| יישום יישום רישום (מזחלת/sqlite/מחוץ לשרשרת) או חוזה חכם. | Core Infra / צוות חוזה חכם | הוכח hashing determinista, evitar punto flotante. |
| נקודות כניסה: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | אינפרא ליבה | Aprovechar `ManifestValidator` del plan de validacion. El binding de alias ahora fluye via `RegisterPinManifest` (DTO de Torii) mientras `bind_alias` dedicado sugue planeado para actualizaciones sucesivas. |
| Transiciones de estado: ירושה של רוצח (התבטאויות A -> B), epocas de retencion, unicidad de alias. | מועצת ממשל / Infra Core | Unicidad de alias, limites de retencion y verificaciones de aprobacion/retiro de predecesores viven en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; la deteccion de sucesion multi-hop y la contabilidad de replicacion siguen abiertas. |
| Parametros gobernados: cargar `ManifestPolicyV1` desde config/estado de gobernanza; permitir actualizaciones via eventos de gobernanza. | מועצת ממשל | הוכח CLI למען הפוליטיקה בפועל. |
| פליטת אירועים: emitir eventos Norito עבור טלמטריה (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | צפייה | Definir esquema de eventos + רישום. |

פרובאס:
- בדיקות unitarios para cada נקודת כניסה (positivo + rechazo).
- Tests de propiedades para la cadena de sucesion (sin ciclos, epocas monotonicamente crecientes).
- Fuzz de validacion generando מניב aleatorios (acotados).

## Fachada de servicio (Integracion Torii/SDK)

| רכיב | טארא | אחראי(ים) |
|----------------|-------|----------------|
| Servicio Torii | Exponer `/v2/sorafs/pin` (שלח), `/v2/sorafs/pin/{cid}` (חיפוש), `/v2/sorafs/aliases` (רשימה/כריכה), `/v2/sorafs/replication` (הזמנות/קבלות). Proeer paginacion + פילטרדו. | Networking TL / Core Infra |
| Atestacion | כלול altura/hash del registry in respuestas; agregar estructura de atestacion Norito consumida por los SDKs. | אינפרא ליבה |
| CLI | Extender `sorafs_manifest_stub` או un nuevo CLI `sorafs_pin` עם `pin submit`, `alias bind`, `order issue`, `registry export`. | Tooling WG |
| SDK | כריכות כלליות של לקוחות (Rust/Go/TS) desde el esquema Norito; מבחני שילוב. | צוותי SDK |

פעולות:
- Agregar capa de cache/ETag עבור נקודות קצה GET.
- הגבלת שיעור הוכחה / אישור עקבי עם פוליטיקה של Torii.

## מתקנים y CI- מדריך מתקנים: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` שומר צילומי מצב של מניפסט/כינוי/הזמנה מחדש של `cargo run -p iroha_core --example gen_pin_snapshot`.
- Paso de CI: `ci/check_sorafs_fixtures.sh` regenera el snapshot y falla si hay diffs, manteniendo los fixtures de CI alineados.
- Tests de integracion (`crates/iroha_core/tests/pin_registry.rs`) ejercitan el flujo feliz mas el rechazo de alias duplicado, guards de aprobacion/retencion de alias, handles de chunker desalineados, validacion de conteo de replicas de fallos de guard desconocidos/preaprobados/retirados/autorreferencias); ver casos `register_manifest_rejects_*` לפרטי פרטים על קוברטורה.
- בדיקות unitarios ahora cubren validacion de alias, guards de retencion y checks de sucesor en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; la deteccion de sucesion multi-hop cuando aterrice la maquina de estados.
- JSON golden para eventos usados ​​por pipelines de observabilidad.

## Telemetria y Observabilidad

מדדים (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- La telemetria existente de providers (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) היכנסו ללוחות מחוונים מקצה לקצה.

יומנים:
- Stream de eventos Norito estructurados para auditorias de gobernanza (firmados?).

התראות:
- Ordenes de replicacion pendientes excediendo el SLA.
- Expiracion de alias por debajo del umbral.
- Violaciones de retencion (מתברר ללא חידוש אנטות דה תפוגה).

לוחות מחוונים:
- El JSON de Grafana `docs/source/grafana_sorafs_pin_registry.json` rastrea totals de ciclo de vida de manifests, cobertura de alias, saturacion de backlog, ratio de SLA, overlays de latencia vs slack y tasas de ordenes perdidas para revision on-call.

## ספרי הפעלה ותיעוד

- אקטואליזר `docs/source/sorafs/migration_ledger.md` כולל רישום אקטואליזציה.
- מדריכים: `docs/source/sorafs/runbooks/pin_registry_ops.md` (יהיו פרסומים) מדדי מידע, התראות, דיפלוג, גיבוי ופעולות החלמה.
- Guia de gobernanza: תיאור פרמטרים פוליטיים, זרימת עבודה של אפרובאציון, מחלוקת.
- עיון ב-API לנקודת קצה (Docusaurus מסמכים).

## Dependencias y secuenciacion

1. השלם תאריכים של תוכנית אימות (integracion de ManifestValidator).
2. Finalizar esquema Norito + ברירות מחדל של פוליטיקה.
3. קונטרה מיושם + שירות, קונקטר טלמטריה.
4. מתקנים מחדש, correr suites de integracion.
5. אקטואליזר מסמכים/רונבוקים ופריטי מרקר של מפת דרכים כמו השלמות.

רשימת הבדיקה של SF-4 מפרטת את התוכנית הקודמת.
La fachada REST ahora entrega endpoints de listado con atestacion:

- `GET /v2/sorafs/pin` y `GET /v2/sorafs/pin/{digest}` devuelven manifests con
  bindings de alias, ordenes de replicacion y un objeto de atestacion derivado del
  hash del ultimo bloque.
- `GET /v2/sorafs/aliases` y `GET /v2/sorafs/replication` exponen el catalogo de
  כינוי activo y el backlog de ordenes de replicacion con paginacion consistente y
  filtros de estado.La CLI envuelve estas llamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que los operadores puedan automatizar auditorias del
registry sin tocar APIs de bajo nivel.