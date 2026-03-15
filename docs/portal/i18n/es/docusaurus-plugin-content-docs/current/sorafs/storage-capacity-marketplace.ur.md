---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado-capacidad-de-almacenamiento
título: SoraFS اسٹوریج کیپیسٹی مارکیٹ پلیس
sidebar_label: کیپیسٹی مارکیٹ پلیس
descripción: کیپیسٹی مارکیٹ پلیس، órdenes de replicación, ٹیلی میٹری اور گورننس ہکس کے لیے SF-2c پلان۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/storage_capacity_marketplace.md` کی عکاسی کرتا ہے۔ جب تک پرانا ڈاکیومنٹیشن فعال ہے دونوں لوکیشنز کو ہم آہنگ رکھیں۔
:::

# SoraFS اسٹوریج کیپیسٹی مارکیٹ پلیس (SF-2c ڈرافٹ)

روڈ میپ آئٹم SF-2c ایک گورنڈ مارکیٹ پلیس متعارف کراتا ہے جہاں اسٹوریج proveedores capacidad comprometida ڈیکلئر کرتے ہیں، órdenes de replicación وصول کرتے ہیں، اور فراہم کردہ disponibilidad کے متناسب tarifas کماتے ہیں۔ یہ دستاویز پہلی ریلیز کے لیے درکار entregables کا دائرہ کار طے کرتی ہے اور انہیں pistas procesables میں تقسیم کرتی ہے۔

## مقاصد

- proveedores compromisos de capacidad (bytes, carriles, límites, caducidad) استعمال کر سکیں۔
- capacidad declarada, participación, restricciones políticas, pines, proveedores, comportamiento determinista, comportamiento determinista.
- entrega de almacenamiento (éxito de la replicación, tiempo de actividad, pruebas de integridad) کو ناپنا اور distribución de tarifas کے لیے exportación de telemetría کرنا۔
- revocación اور disputa کے عمل فراہم کرنا تاکہ proveedores deshonestos کو penalizar یا eliminar کیا جا سکے۔

## ڈومین کانسیپٹس| Concepto | Descripción | Entregable inicial |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | Carga útil Norito, ID de proveedor, soporte de perfil de fragmentación, GiB comprometido, límites específicos de carril, sugerencias de precios, compromiso de apuesta y vencimiento de کرتا ہے۔ | `sorafs_manifest::capacity` Esquema + validador۔ |
| `ReplicationOrder` | gobernanza کی جانب سے جاری instrucción جو manifiesto CID کو ایک یا زائد proveedores کو asignar کرتی ہے، جس میں nivel de redundancia اور SLA métricas شامل ہیں۔ | Torii کے ساتھ مشترک Esquema Norito + API de contrato inteligente۔ |
| `CapacityLedger` | registro dentro/fuera de la cadena, declaraciones de capacidad activa, órdenes de replicación, métricas de rendimiento y acumulación de tarifas | módulo de contrato inteligente یا trozo de servicio fuera de la cadena مع instantánea determinista۔ |
| `MarketplacePolicy` | política de gobernanza, participación mínima, requisitos de auditoría, curvas de penalización متعین کرتی ہے۔ | `sorafs_manifest` Estructura de configuración + documento de gobierno ۔ |

### Esquemas implementados (estado)

## ورک بریک ڈاؤن

### 1. Esquema y capa de registro| Tarea | Propietario(s) | Notas |
|------|----------|-------|
| `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1` کی تعریف۔ | Equipo de Almacenamiento / Gobernanza | Norito استعمال کریں؛ versiones semánticas اور referencias de capacidad شامل کریں۔ |
| `sorafs_manifest` Módulos analizador + validador میں لاگو کریں۔ | Equipo de almacenamiento | ID monótonos, límites de capacidad, requisitos de participación نافذ کریں۔ |
| Metadatos del registro fragmentador میں ہر perfil کے لیے `min_capacity_gib` شامل کریں۔ | Grupo de Trabajo sobre Herramientas | clientes کو perfil کے حساب سے requisitos mínimos de hardware نافذ کرنے میں مدد۔ |
| `MarketplacePolicy` ڈاکیومنٹ ڈرافٹ کریں جو barreras de admisión اور calendario de sanciones بیان کرے۔ | Consejo de Gobierno | docs میں publicar کریں اور valores predeterminados de políticas کے ساتھ رکھیں۔ |

#### Definiciones de esquema (implementadas)- `CapacityDeclarationV1` proveedor de compromisos de capacidad firmados, captura de datos, identificadores de fragmentación canónicos, referencias de capacidad, límites de carril opcionales, sugerencias de precios, ventanas de validez y metadatos. validación یہ یقینی بناتا ہے کہ participación غیر صفر ہو، maneja canónicos ہوں، alias deduplicados ہوں، límites de carril اعلان کردہ total کے اندر ہوں اور GiB contable monótono ہو۔【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` manifiesta asignaciones emitidas por la gobernanza, objetivos de redundancia, umbrales de SLA y garantías por asignación. validadores identificadores de fragmentos canónicos, proveedores únicos y restricciones de fecha límite نافذ کرتے ہیں اس سے پہلے کہ Torii یا ingesta de orden de registro کریں۔【crates/sorafs_manifest/src/capacity.rs:301】
- Instantáneas de época `CapacityTelemetryV1` (GiB declarado versus utilizado, contadores de replicación, porcentajes de tiempo de actividad/PoR) ظاہر کرتا ہے جو distribución de tarifas کو feed کرتے ہیں۔ controles de límites استعمال کو declaraciones کے اندر اور porcentajes کو 0-100% میں رکھتے ہیں۔【crates/sorafs_manifest/src/capacity.rs:476】
- Ayudantes compartidos (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores de carril/asignación/SLA) validación de clave determinista e informes de errores فراہم کرتے ہیں جنہیں CI اور reutilización de herramientas posteriores ہیں۔【crates/sorafs_manifest/src/capacity.rs:230】- `PinProviderRegistry` اب instantánea en cadena کو `/v2/sorafs/capacity/state` کے ذریعے exponen کرتا ہے، declaraciones del proveedor اور entradas del libro mayor de tarifas کو کو determinista Norito JSON کے پیچھے جوڑ کر۔【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Cobertura de validación, aplicación de control canónico, detección de duplicados, límites por carril, guardias de asignación de replicación, comprobaciones de rango de telemetría, ejercicio, regresiones, CI میں ظاہر ہوں۔【crates/sorafs_manifest/src/capacity.rs:792】
- Herramientas del operador: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` especificaciones legibles por humanos, cargas útiles canónicas Norito, blobs base64 y resúmenes JSON, múltiples operadores `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry` اور accesorios de orden de replicación کو validación local کے ساتھ etapa کر سکیں۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Accesorios de referencia `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) میں ہیں اور `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` سے generar ہوتی ہیں۔

### 2. Integración del plano de control| Tarea | Propietario(s) | Notas |
|------|----------|-------|
| Controladores `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, `/v2/sorafs/capacity/orders` Torii y cargas útiles JSON Norito en varios colores | Torii Equipo | lógica del validador کو espejo کریں؛ Norito Los ayudantes JSON reutilizan کریں۔ |
| `CapacityDeclarationV1` instantáneas, metadatos del marcador del orquestador, planes de recuperación de la puerta de enlace, propagación | Equipo de trabajo de herramientas / orquestador | `provider_metadata` میں referencias de capacidad شامل کریں تاکہ límites de carril de puntuación de múltiples fuentes کا خیال رکھے۔ |
| órdenes de replicación کو clientes de orquestador/gateway میں feed کریں تاکہ asignaciones اور sugerencias de conmutación por error ڈرائیو ہوں۔ | Equipo de Networking TL / Gateway | Órdenes de replicación firmadas por la gobernanza del generador de marcadores استعمال کرتا ہے۔ |
| Herramientas CLI: `sorafs_cli` میں `capacity declare`, `capacity telemetry`, `capacity orders import` شامل کریں۔ | Grupo de Trabajo sobre Herramientas | salidas deterministas de marcador JSON + فراہم کریں۔ |

### 3. Política y gobernanza del mercado| Tarea | Propietario(s) | Notas |
|------|----------|-------|
| `MarketplacePolicy` (apuesta mínima, multiplicadores de penalización, cadencia de auditoría) کی توثیق۔ | Consejo de Gobierno | docs میں publicar کریں، captura del historial de revisiones کریں۔ |
| ganchos de gobernanza شامل کریں تاکہ Declaraciones del Parlamento کو aprobar، renovar اور revocar کر سکے۔ | Consejo de Gobernanza / Equipo de Contratos Inteligentes | Eventos Norito + ingestión de manifiesto استعمال کریں۔ |
| calendario de sanciones (reducción de tarifas, reducción de fianzas) نافذ کریں جو violaciones de SLA telemétricas سے منسلک ہو۔ | Consejo de Gobierno / Tesorería | Salidas de liquidación `DealEngine` کے ساتھ align کریں۔ |
| proceso de disputa اور matriz de escalada کی دستاویز بنائیں۔ | Documentos / Gobernanza | runbook de disputas + ayudantes de CLI لنک کریں۔ |

### 4. Medición y distribución de tarifas| Tarea | Propietario(s) | Notas |
|------|----------|-------|
| Torii ingesta de medición کو `CapacityTelemetryV1` قبول کرنے کے لیے بڑھائیں۔ | Torii Equipo | GiB-hora, éxito de PoR, tiempo de actividad validar کریں۔ |
| Tubería de medición `sorafs_node` کو utilización por pedido + estadísticas de SLA رپورٹ کرنے کے لیے اپڈیٹ کریں۔ | Equipo de almacenamiento | órdenes de replicación اور identificadores de fragmentación کے ساتھ align کریں۔ |
| Canal de liquidación: telemetría + datos de replicación کو Pagos denominados en XOR میں تبدیل کریں، resúmenes listos para la gobernanza بنائیں، اور estado del libro mayor ریکارڈ کریں۔ | Equipo de Tesorería / Almacenamiento | Deal Engine / Exportaciones del Tesoro میں wire کریں۔ |
| Estado de la medición (acumulación de ingesta, telemetría obsoleta) Exportación de paneles/alertas کریں۔ | Observabilidad | SF-6/SF-7 میں ریفرنس کیے گئے Grafana pack کو extender کریں۔ |- Torii اب `/v2/sorafs/capacity/telemetry` اور `/v2/sorafs/capacity/state` (JSON + Norito) exponen instantáneas de telemetría de época de operadores de کرتا ہے تاکہ جمع کر سکیں اور inspectores auditoría یا embalaje de evidencia کے لیے libro mayor canónico حاصل کر سکیں۔【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- Integración `PinProviderRegistry` یقینی بناتی ہے کہ órdenes de replicación اسی endpoint کے ذریعے قابلِ رسائی ہوں؛ Ayudantes de CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) Hashing determinista اور resolución de alias کے ساتھ ejecuciones de automatización سے telemetría validar/publicar کرتے ہیں۔
- Medición de instantáneas `CapacityTelemetrySnapshot` entradas پیدا کرتے ہیں جو `metering` instantánea سے fijada ہوتے ہیں، اور Prometheus exportaciones `docs/source/grafana_sorafs_metering.json` Placa Grafana lista para importar, alimentación, equipos de facturación, acumulación de horas GiB, tarifas nano-SORA proyectadas, cumplimiento de SLA, tiempo real سکیں۔【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Suavizado de medición habilitado ہو تو instantánea میں `smoothed_gib_hours` اور `smoothed_por_success_bps` شامل ہوتے ہیں تاکہ operadores Valores de tendencia EMA کو contadores sin procesar کے مقابلے میں دیکھ سکیں جنہیں pagos de gobernanza کے لیے استعمال کرتی ہے۔【crates/sorafs_node/src/metering.rs:401】

### 5. Manejo de disputas y revocación| Tarea | Propietario(s) | Notas |
|------|----------|-------|
| Carga útil `CapacityDisputeV1` (denunciante, evidencia, proveedor de destino) کی تعریف۔ | Consejo de Gobierno | Esquema Norito + validador۔ |
| disputas فائل کرنے اور جواب دینے کے لیے CLI support (adjuntos de evidencia کے ساتھ)۔ | Grupo de Trabajo sobre Herramientas | paquete de evidencia کی hash determinista یقینی بنائیں۔ |
| incumplimientos repetidos del SLA کے لیے verificaciones automatizadas شامل کریں (escalamiento automático a disputa)۔ | Observabilidad | Umbrales de alerta y ganchos de gobernanza. |
| manual de revocación (período de gracia, datos fijados کی evacuación) دستاویزی بنائیں۔ | Equipo de Documentos/Almacenamiento | documento de políticas y runbook del operador سے لنک کریں۔ |

## Requisitos de pruebas y CI

- تمام نئے validadores de esquemas کے لیے pruebas unitarias (`sorafs_manifest`).
- pruebas de integración y simulaciones: declaración → orden de replicación → medición → pago ۔
- Flujo de trabajo de CI, declaraciones de capacidad de muestra/regeneración de telemetría, sincronización de firmas (`ci/check_sorafs_fixtures.sh`, extensión de datos)
- API de registro کے لیے pruebas de carga (10k proveedores, 100k pedidos simulan کریں)۔

## Telemetría y paneles- Paneles de instrumentos:
  - el proveedor کے حساب سے declaró بمقابلہ capacidad utilizada۔
  - acumulación de pedidos de replicación اور retraso promedio en la asignación۔
  - Cumplimiento de SLA (% de tiempo de actividad, tasa de éxito de PoR) ۔
  - فی acumulación de tarifas de época y sanciones ۔
- Alertas:
  - proveedor کم از کم capacidad comprometida سے نیچے۔
  - orden de replicación SLA سے زیادہ atascada۔
  - fallas en las tuberías de medición ۔

## Entregables de documentación

- capacidad declarar کرنے، compromisos renovar کرنے، اور utilización مانیٹر کرنے کے لیے guía del operador۔
- declaraciones aprueban کرنے، órdenes جاری کرنے، disputas manejan کرنے کے لیے guía de gobernanza۔
- puntos finales de capacidad y formato de orden de replicación کے لیے referencia API۔
- Preguntas frecuentes sobre el mercado de desarrolladores کے لیے ۔

## Lista de verificación de preparación para GA

روڈ میپ آئٹم **SF-2c** contabilidad, manejo de disputas اور onboarding میں ٹھوس شواہد کی بنیاد پر lanzamiento de producción کو گیٹ کرتا ہے۔ نیچے دیے گئے artefactos استعمال کریں تاکہ implementación de criterios de aceptación کے ساتھ sincronización رہے۔### Contabilidad nocturna y conciliación XOR
- instantánea del estado de capacidad اور XOR exportación del libro mayor ایک ہی ventana کے لیے نکالیں، پھر چلائیں:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  یہ liquidaciones faltantes/pagadas en exceso del ayudante یا sanciones پر salida distinta de cero کرتا ہے اور Prometheus emisión de resumen del archivo de texto کرتا ہے۔
- Alerta `SoraFSCapacityReconciliationMismatch` (`dashboards/alerts/sorafs_capacity_rules.yml` میں)
  métricas de conciliación کے brechas رپورٹ کرنے پر fuego ہوتا ہے؛ cuadros de mando `dashboards/grafana/sorafs_capacity_penalties.json` میں ہیں۔
- Resumen JSON اور hashes کو `docs/examples/sorafs_capacity_marketplace_validation/` کے تحت archivo کریں
  paquetes de gobernanza کے ساتھ۔

### Disputa y evidencia de reducción
- disputas `sorafs_manifest_stub capacity dispute` کے ذریعے فائل کریں (pruebas:
  `cargo test -p sorafs_car --test capacity_cli`) Cargas útiles canónicas رہیں۔
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` اور suites de penalización
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) چلائیں تاکہ disputas اور corta la repetición determinista ثابت ہوں۔
- captura de evidencia اور escalada کے لیے `docs/source/sorafs/dispute_revocation_runbook.md` فالو کریں؛ aprobaciones de huelgas informe de validación میں واپس لنک کریں۔

### Pruebas de humo de incorporación y salida de proveedores
- artefactos de declaración/telemetría کو `sorafs_manifest_stub capacity ...` سے regenerar کریں اور envío سے پہلے Repetición de pruebas CLI کریں (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)۔
- Torii (`/v2/sorafs/capacity/declare`) کے ذریعے enviar کریں پھر `/v2/sorafs/capacity/state` اور Grafana captura de pantalla کریں۔ `docs/source/sorafs/capacity_onboarding_runbook.md` Flujo de salida میں فالو کریں۔
- artefactos firmados اور salidas de reconciliación کو `docs/examples/sorafs_capacity_marketplace_validation/` میں archivo کریں۔

## Dependencias y secuenciación1. SF-2b (política de admisión) مکمل کریں: proveedores examinados por el mercado پر انحصار کرتا ہے۔
2. Integración Torii سے پہلے esquema + capa de registro (یہ ڈاک) مکمل کریں۔
3. los pagos permiten کرنے سے پہلے tubería de medición مکمل کریں۔
4. آخری قدم: puesta en escena میں datos de medición verificar ہونے کے بعد distribución de tarifas controlada por la gobernanza habilitar کریں۔

progreso کو hoja de ruta میں اس دستاویز کے حوالے کے ساتھ ٹریک کریں۔ جب ہر بڑا سیکشن (esquema, plano de control, integración, medición, manejo de disputas) función completa ہو جائے تو hoja de ruta اپڈیٹ کریں۔