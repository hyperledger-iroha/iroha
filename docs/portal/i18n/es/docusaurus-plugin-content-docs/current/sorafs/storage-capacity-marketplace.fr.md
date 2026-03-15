---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado-capacidad-de-almacenamiento
título: Mercado de capacidad de almacenamiento SoraFS
sidebar_label: Mercado de capacidades
descripción: Plan SF-2c para el mercado de capacidad, las órdenes de réplica, la télémétrie y los ganchos de gobierno.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/storage_capacity_marketplace.md`. Guarde los dos emplazamientos alineados hasta que la documentación heredada permanezca activa.
:::

# Marketplace de capacidad de almacenamiento SoraFS (Brouillon SF-2c)

El elemento SF-2c de la hoja de ruta introduce un mercado gobernado por los proveedores de
almacenamiento declara una capacidad comprometida, recibe órdenes de réplica y
Perçoivent des fee proporcionalnels à la disponibilité livrée. Cuadro de documentos ce
les livrables requis pour la première release et les decline en pistes actionnables.

## Objetivos- Exprimer les engagements de capacité (bytes totales, límites por carril, vencimiento)
  bajo una forma verificable y consumible por el gobierno, el transporte SoraNet y Torii.
- Coloque los pines entre los proveedores según la capacidad declarada, la apuesta y los
  Contraintes de politique tout en maintenant un comportement deterministe.
- Medir la vida útil del almacenamiento (éxito de réplica, tiempo de actividad, ventajas de integridad)
  et exporter la télémétrie pour la Distribution des fee.
- Fournir des Processus de révocation et de disputa afin que les proveedores malhonnêtes
  soient pénalisés ou jubilés.

## Conceptos de dominio| Concepto | Descripción | Inicial habitable |
|---------|-------------|-----------------|
| `CapacityDeclarationV1` | La carga útil Norito indica el ID del proveedor, el soporte del fragmentador de perfiles, los GiB activados, los límites del carril, las sugerencias de precios, el compromiso de participación y el vencimiento. | Esquema + validador en `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instrucción emitida por la Gouvernance Assignant un CID de manifiesto a uno o más proveedores, incluyendo el nivel de redondeo y las métricas SLA. | Esquema Norito compartido con Torii + API de contrato inteligente. |
| `CapacityLedger` | Registro dentro y fuera de la cadena que se adapta a las declaraciones de capacidad activa, las órdenes de réplica, las métricas de desempeño y la acumulación de tarifas. | Módulo de contrato inteligente o trozo de servicio fuera de cadena con instantánea determinada. |
| `MarketplacePolicy` | La política de gobierno determina las apuestas mínimas, las exigencias de auditoría y las corrientes de penalización. | Estructura de configuración en `sorafs_manifest` + documento de gobierno. |

### Esquemas implementados (estatuto)

## Découpage del trabajo

### 1. Couche schéma y registro| tache | Propietario(s) | Notas |
|------|----------|-------|
| Definir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Equipo de Almacenamiento / Gobernanza | Utilizador Norito; incluya versiones sémánticas y referencias de capacidades. |
| Implemente el analizador y validador de módulos en `sorafs_manifest`. | Equipo de almacenamiento | El impostor identifica monótonos, límites de capacidad, exigencias de juego. |
| Étendre los metadatos del registro fragmentador con `min_capacity_gib` por perfil. | Grupo de Trabajo sobre Herramientas | Ayude a los clientes a imponer exigencias mínimas de hardware por perfil. |
| Rédiger le document `MarketplacePolicy` capturant les guardrails d'admission et le calendrier de pénalités. | Consejo de Gobierno | Publier dans les docs avec les defaults de politique. |

#### Definiciones de esquema (implementadas)- `CapacityDeclarationV1` captura los compromisos de capacidad firmados por el proveedor, incluidos manejadores canónicos fragmentados, referencias de capacidad, opciones de límites por carril, sugerencias de precios, ventanas de validación y metadatos. La validación asegura una participación no nula, des handles canonices, des alias dédupliqués, des caps par lane dans le total déclaré et une comptabilité GiB monotone.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` asociado de manifiestos de asignaciones emitidos por la gobernanza con objetivos de rondancia, seguros SLA y garantías por asignación; Los validadores imponen los identificadores canónicos, los proveedores únicos y las restricciones de fecha límite antes de la ingestión por Torii o el registro. 【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` exprime des snapshots por época (GiB declarado vs utilisés, compteurs de replication, pourcentages d'uptime/PoR) que alimenta la distribución de tarifas. Los pagos mantienen la utilización en las declaraciones y los porcentajes en 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Los asistentes participantes (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores lane/assignment/SLA) proporcionan una validación determinada de las claves y un informe de error reutilizable por CI y las herramientas posteriores.【crates/sorafs_manifest/src/capacity.rs:230】- `PinProviderRegistry` expone la instantánea en cadena a través de `/v1/sorafs/capacity/state`, y combina declaraciones de proveedores y entradas del libro mayor de tarifas posterior con un Norito JSON déterministe.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- La cobertura de validación ejerce la aplicación de manejadores canónicos, la detección de doblones, los bornes por carril, los guardias de asignación de réplica y las comprobaciones de placa de télémétrie para que las regresiones aparezcan inmediatamente en CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Operador de herramientas: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` convierte especificaciones flexibles en cargas útiles Norito canónicas, blobs base64 y currículums JSON para que los operadores puedan preparar accesorios `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` y pedidos. de réplica con validación local.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Los accesorios de referencia viven en `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) y son generados a través de `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Integración del plan de control| tache | Propietario(s) | Notas |
|------|----------|-------|
| Agregar los controladores Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` con cargas útiles Norito JSON. | Torii Equipo | Répliquer la lógica de validación; Reutilice los ayudantes Norito JSON. |
| Propager les snapshots `CapacityDeclarationV1` ver los metadatos del marcador del orquestador y los planes de fetch gateway. | Equipo de trabajo de herramientas / orquestador | Étendre `provider_metadata` con referencias de capacidad para que la puntuación de múltiples fuentes respete los límites del carril. |
| Alimente las órdenes de replicación en los clientes orquestador/puerta de enlace para controlar las asignaciones y sugerencias de conmutación por error. | Equipo de Networking TL / Gateway | Le builder de scoreboard consomme des ordres signés par la gouvernance. |
| CLI de herramientas: seleccione `sorafs_cli` con `capacity declare`, `capacity telemetry`, `capacity orders import`. | Grupo de Trabajo sobre Herramientas | Fournir JSON déterministe + marcador de salidas. |

### 3. Mercado político y gobernanza| tache | Propietario(s) | Notas |
|------|----------|-------|
| Ratificador `MarketplacePolicy` (participación mínima, multiplicadores de pena, cadencia de auditoría). | Consejo de Gobierno | Publier dans les docs, capture l'historique des révisions. |
| Ajouter des ganchos de gobierno para que el Parlamento apruebe, renouvelle y révoque les declaraciones. | Consejo de Gobernanza / Equipo de Contratos Inteligentes | Utilice los eventos Norito + ingestión de manifiestos. |
| Implementar el calendario de sanciones (reducción de tarifas, reducción de fianzas) en caso de violaciones del SLA télémétrées. | Consejo de Gobierno / Tesorería | Alinee con las salidas de liquidación del `DealEngine`. |
| Documenter le Processus de Disputa et la Matrice d'Escalade. | Documentos / Gobernanza | Lier au runbook de disputa + ayudantes CLI. |

### 4. Tarifas de medición y distribución| tache | Propietario(s) | Notas |
|------|----------|-------|
| Étendre l'ingestion de metering Torii pour aceptador `CapacityTelemetryV1`. | Torii Equipo | Validador de hora GiB, PoR exitoso, tiempo de actividad. |
| Mettre à jour le pipeline de metering `sorafs_node` pour reporter utilization par ordre + stats SLA. | Equipo de almacenamiento | Alineador con órdenes de réplica y mangos fragmentados. |
| Pipeline de liquidación: convertir télémétrie + replication en payouts dénommés en XOR, producir currículums vitae prêts pour gouvernance y registrar el estado del libro mayor. | Equipo de Tesorería / Almacenamiento | Conector à Deal Engine / Exportaciones de tesorería. |
| Paneles de control/alertas del exportador para la salud de la medición (acumulación de ingesta, télémétrie obsoleto). | Observabilidad | Tenga el paquete Grafana referenciado por SF-6/SF-7. |- Torii expone desormais `/v1/sorafs/capacity/telemetry` e `/v1/sorafs/capacity/state` (JSON + Norito) para que los operadores obtengan instantáneas de télémétrie por época y que los inspectores recuperen el libro mayor canónico para auditoría o embalaje. de preuves.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- La integración `PinProviderRegistry` garantiza que las órdenes de réplica son accesibles a través del mismo punto final; Los ayudantes CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) validan y publican la télémétrie después de las automatizaciones con hash determinado y resolución de alias.
- Las instantáneas de medición que producen los platos principales `CapacityTelemetrySnapshot` se fijan en la instantánea `metering`, y las exportaciones Prometheus alimentan la placa Grafana prêt à importer dans `docs/source/grafana_sorafs_metering.json` después de los equipos. de facturación posterior a la acumulación de GiB-hora, las tarifas nano-SORA proyectadas y la conformidad SLA en tiempo real.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Cuando el suavizado de medición está activado, la instantánea incluye `smoothed_gib_hours` e `smoothed_por_success_bps` para comparar los valores EMA con los computadores brutos utilizados por la gobernanza para los pagos.【crates/sorafs_node/src/metering.rs:401】

### 5. Gestión de disputas y revocaciones| tache | Propietario(s) | Notas |
|------|----------|-------|
| Definir la carga útil `CapacityDisputeV1` (plaignant, preuve, proveedor ciblé). | Consejo de Gobierno | Esquema Norito + validador. |
| Soporte CLI para presentar disputas y responder (con piezas anteriores). | Grupo de Trabajo sobre Herramientas | Asegúrese de que un hash determine el paquete de preuves. |
| Agregar controles automáticos para violaciones de SLA repetidas (escalada automática de disputas). | Observabilidad | Seuils d'alertes et hooks de gouvernance. |
| Documenter le playbook de révocation (période de grâce, évacuation des données pin). | Equipo de Documentos/Almacenamiento | Lier au doc ​​de politique et au runbook opérateur. |

## Exigencias de pruebas & CI

- Pruebas unitarias para todos los nuevos validadores de esquema (`sorafs_manifest`).
- Pruebas de integración simuladas: declaración → orden de réplica → medición → pago.
- Flujo de trabajo CI para regular las declaraciones/telemetría de capacidad y asegurar la sincronización de firmas (étendre `ci/check_sorafs_fixtures.sh`).
- Pruebas de carga para el registro API (simulador de 10k proveedores, 100k pedidos).

## Telemétrie y paneles de control- Paneles de salpicadero:
  - Capacidad declarada vs utilizada por el proveedor.
  - Backlog des ordres de replication et délai moyen d'assignation.
  - Conformidad SLA (uptime %, taux de succès PoR).
  - Acumulación de honorarios y penalidades por época.
- Alertas:
  - Proveedor con capacidad mínima comprometida.
  - Orden de réplica bloqué > SLA.
  - Échecs du pipeline de metering.

## Libros de documentación

- Guía del operador para declarar la capacidad, renovar los compromisos y monitorear la utilización.
- Guía de gobierno para aprobar las declaraciones, emitir órdenes y gestionar las disputas.
- Referencia API para los puntos finales de capacidad y el formato del orden de replicación.
- Mercado de preguntas frecuentes para desarrolladores.

## Lista de verificación de preparación GA

El elemento de la hoja de ruta **SF-2c** condiciona la implementación de la producción en los primeros concretos
sur la comptabilité, la gestion des disputas et l'onboarding. Utilizar los artefactos
ci-dessous pour garder les critères d'acceptation alignés avec l'implementation.### Comptabilité nocturno y reconciliación XOR
- Exportador de la instantánea del estado de capacidad y exportación del libro mayor XOR para la misma ventana, después de lanzar:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  Le helper sort avec un code non nul en caso de acuerdos/sanciones manquants o excesos y émet un currículum Prometheus en formato de archivo de texto.
- La alerta `SoraFSCapacityReconciliationMismatch` (en `dashboards/alerts/sorafs_capacity_rules.yml`) se desactiva cuando se activan las mediciones de reconciliación señaladas con los mensajes electrónicos; tableros de instrumentos en `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archivar el currículum JSON y los hashes en `docs/examples/sorafs_capacity_marketplace_validation/` con los paquetes de gobierno.

### Preuve de disputa y corte
- Déposer des disputas vía `sorafs_manifest_stub capacity dispute` (pruebas:
  `cargo test -p sorafs_car --test capacity_cli`) para guardar las cargas útiles canónicas.
- Lancer `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` et les suites de pénalité (`record_capacity_telemetry_penalises_persistent_under_delivery`) para demostrar que las disputas y las barras diagonales rejouent de manera determinada.
- Suivre `docs/source/sorafs/dispute_revocation_runbook.md` pour la capture de preuves et l'escalade ; lier les approbations de strike au rapport de validation.

### Incorporación de proveedores y pruebas de humo de salida.
- Regénérer les artefactos de declaración/télémétrie con `sorafs_manifest_stub capacity ...` y rejouer les tests CLI avant soumission (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Soumettre vía Torii (`/v1/sorafs/capacity/declare`) luego de la captura `/v1/sorafs/capacity/state` y de las capturas Grafana. Siga el flujo de salida en `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archivar los artefactos firmados y las salidas de reconciliación en el país.
  `docs/examples/sorafs_capacity_marketplace_validation/`.## Dependencias y secuenciación

1. Terminer SF-2b (política de admisión): el mercado depende de los proveedores validados.
2. Implemente el esquema de sofá + registro (ce doc) antes de la integración Torii.
3. Finalizar la tubería de medición antes de activar los pagos.
4. Etapa final: activer la distribución de tarifas pilotada por gobierno una vez que se donan las donaciones de medición verificadas en la puesta en escena.

El progreso debe seguirse en la hoja de ruta con las referencias a este documento. Mettre à jour la hoja de ruta una vez que cada sección mayor (esquema, plan de control, integración, medición, gestión de disputas) atteint le statut característica completa.