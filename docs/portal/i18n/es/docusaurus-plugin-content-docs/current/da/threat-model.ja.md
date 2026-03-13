---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/da/threat-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: de0ee9cba67f114ff2601b6dbeed2a73d7477cc5872996198b05a63086280346
source_last_modified: "2026-01-19T07:28:06+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: es
direction: ltr
source: docs/portal/docs/da/threat-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canonica
Refleja `docs/source/da/threat_model.md`. Mantenga ambas versiones en
:::

# Modelo de amenazas de Data Availability de Sora Nexus

_Ultima revision: 2026-01-19 -- Proxima revision programada: 2026-04-19_

Cadencia de mantenimiento: Data Availability Working Group (<=90 dias). Cada
revision debe aparecer en `status.md` con links a tickets de mitigacion activos
y artefactos de simulacion.

## Proposito y alcance

El programa de Data Availability (DA) mantiene transmisiones Taikai, blobs de
lane Nexus y artefactos de gobernanza recuperables ante fallas bizantinas,
de red y de operadores. Este modelo de amenazas ancla el trabajo de ingenieria
para DA-1 (arquitectura y modelo de amenazas) y sirve como baseline para tareas
DA posteriores (DA-2 a DA-10).

Componentes dentro de alcance:
- Extension de ingesta DA en Torii y escritores de metadata Norito.
- Arboles de almacenamiento de blobs respaldados por SoraFS (tiers hot/cold) y
  politicas de replicacion.
- Compromisos de bloques Nexus (wire formats, proofs, APIs de cliente ligero).
- Hooks de enforcement PDP/PoTR especificos para payloads DA.
- Flujos de operadores (pinning, eviction, slashing) y pipelines de
  observabilidad.
- Aprobaciones de gobernanza que admiten o expulsan operadores y contenido DA.

Fuera de alcance para este documento:
- Modelado economico completo (capturado en el workstream DA-7).
- Protocolos base de SoraFS ya cubiertos por el modelo de amenazas de SoraFS.
- Ergonomia de SDK de clientes mas alla de consideraciones de superficie de
  amenaza.

## Panorama arquitectonico

1. **Envio:** Los clientes envian blobs via la API de ingesta DA de Torii. El
   nodo trocea blobs, codifica manifests Norito (tipo de blob, lane, epoch,
   flags de codec), y almacena chunks en el tier hot de SoraFS.
2. **Anuncio:** Intents de pin y hints de replicacion se propagan a proveedores
   de almacenamiento via el registry (SoraFS marketplace) con tags de politica
   que indican objetivos de retencion hot/cold.
3. **Compromiso:** Los secuenciadores Nexus incluyen compromisos de blob (CID +
   roots KZG opcionales) en el bloque canonico. Clientes ligeros dependen del
   hash de compromiso y la metadata anunciada para verificar availability.
4. **Replicacion:** Nodos de almacenamiento descargan shares/chunks asignados,
   satisfacen desafios PDP/PoTR y promueven datos entre tiers hot y cold segun
   politica.
5. **Recuperacion:** Consumidores recuperan datos via SoraFS o gateways DA-aware,
   verificando proofs y levantando solicitudes de reparacion cuando desaparecen
   replicas.
6. **Gobernanza:** Parlamento y el comite de supervision DA aprueban operadores,
   schedules de renta y escalaciones de enforcement. Artefactos de gobernanza se
   almacenan por la misma ruta DA para garantizar transparencia del proceso.

## Activos y responsables

Escala de impacto: **Critico** rompe seguridad/vivacidad del ledger; **Alto**
bloquea backfill DA o clientes; **Moderado** degrada calidad pero es
recuperable; **Bajo** efecto limitado.

| Activo | Descripcion | Integridad | Disponibilidad | Confidencialidad | Responsable |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (chunks + manifests) | Blobs Taikai, lane y gobernanza almacenados en SoraFS | Critico | Critico | Moderado | DA WG / Storage Team |
| Manifests Norito DA | Metadata tipada que describe blobs | Critico | Alto | Moderado | Core Protocol WG |
| Compromisos de bloque | CIDs + roots KZG dentro de bloques Nexus | Critico | Alto | Bajo | Core Protocol WG |
| Schedules PDP/PoTR | Cadencia de enforcement para replicas DA | Alto | Alto | Bajo | Storage Team |
| Registry de operadores | Proveedores de almacenamiento aprobados y politicas | Alto | Alto | Bajo | Governance Council |
| Registros de renta e incentivos | Entradas ledger para rentas y penalidades DA | Alto | Moderado | Bajo | Treasury WG |
| Dashboards de observabilidad | SLOs DA, profundidad de replicacion, alertas | Moderado | Alto | Bajo | SRE / Observability |
| Intents de reparacion | Solicitudes para rehidratar chunks faltantes | Moderado | Moderado | Bajo | Storage Team |

## Adversarios y capacidades

| Actor | Capacidades | Motivaciones | Notas |
| --- | --- | --- | --- |
| Cliente malicioso | Enviar blobs malformados, replays de manifests obsoletos, intentar DoS en ingesta. | Interrumpir broadcasts Taikai, inyectar datos invalidos. | Sin claves privilegiadas. |
| Nodo de almacenamiento bizantino | Soltar replicas asignadas, forjar proofs PDP/PoTR, coludir con otros. | Reducir retencion DA, evitar renta, retener datos como rehens. | Posee credenciales validas de operador. |
| Secuenciador comprometido | Omitir compromisos, equivocar bloques, reordenar metadata de blobs. | Ocultar envios DA, crear inconsistencia. | Limitado por la mayoria de consenso. |
| Operador interno | Abusar acceso de gobernanza, manipular politicas de retencion, filtrar credenciales. | Ganancia economica, sabotaje. | Acceso a infraestructura hot/cold. |
| Adversario de red | Particionar nodos, demorar replicacion, inyectar trafico MITM. | Reducir availability, degradar SLOs. | No puede romper TLS pero puede soltar/ralentizar links. |
| Atacante de observabilidad | Manipular dashboards/alertas, suprimir incidentes. | Ocultar caidas DA. | Requiere acceso al pipeline de telemetria. |

## Fronteras de confianza

- **Frontera de ingreso:** Cliente a extension DA de Torii. Requiere auth por
  request, rate limiting y validacion de payload.
- **Frontera de replicacion:** Nodos de almacenamiento intercambian chunks y
  proofs. Los nodos se autentican mutuamente pero pueden comportarse de forma
  bizantina.
- **Frontera del ledger:** Datos de bloque comprometidos vs almacenamiento
  off-chain. El consenso protege integridad, pero availability requiere
  enforcement off-chain.
- **Frontera de gobernanza:** Decisiones de Council/Parliament que aprueban
  operadores, presupuestos y slashing. Fallas aqui impactan directamente el
  despliegue DA.
- **Frontera de observabilidad:** Recoleccion de metrics/logs exportada a
  dashboards/alert tooling. La manipulacion oculta outages o ataques.

## Escenarios de amenazas y controles

### Ataques en la ruta de ingesta

**Escenario:** Cliente malicioso envia payloads Norito malformados o blobs
sobredimensionados para agotar recursos o contrabandear metadata invalida.

**Controles**
- Validacion de schema Norito con negociacion estricta de versiones; rechazar
  flags desconocidos.
- Rate limiting y autenticacion en el endpoint de ingesta Torii.
- Limites de chunk size y encoding determinista forzados por el chunker SoraFS.
- El pipeline de admision solo persiste manifests tras coincidir el checksum de
  integridad.
- Cache de replay determinista (`ReplayCache`) rastrea ventanas `(lane, epoch,
  sequence)`, persiste high-water marks en disco, y rechaza duplicados/replays
  obsoletos; harnesses de propiedad y fuzz cubren fingerprints divergentes y
  envios fuera de orden. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Brechas residuales**
- Torii ingest debe enhebrar el replay cache en la admision y persistir cursores
  de sequence a traves de reinicios.
- Los schemas Norito DA ahora tienen un fuzz harness dedicado
  (`fuzz/da_ingest_schema.rs`) para estresar invariantes de encode/decode; los
  dashboards de cobertura deben alertar si el target regresa.

### Retencion por withholding de replicacion

**Escenario:** Operadores de almacenamiento bizantinos aceptan asignaciones de
pin pero sueltan chunks, pasando desafios PDP/PoTR via respuestas forjadas o
colusion.

**Controles**
- El schedule de desafios PDP/PoTR se extiende a payloads DA con cobertura por
  epoch.
- Replicacion multi-source con umbrales de quorum; el fetch orchestrator detecta
  shards faltantes y dispara reparacion.
- Slashing de gobernanza vinculado a proofs fallidas y replicas faltantes.
- Job de reconciliacion automatizado (`cargo xtask da-commitment-reconcile`)
  compara receipts de ingesta con compromisos DA (SignedBlockWire, `.norito` o
  JSON), emite un bundle JSON de evidencia para gobernanza, y falla ante tickets
  faltantes o no coincidentes para que Alertmanager pagine por omission/tampering.

**Brechas residuales**
- El harness de simulacion en `integration_tests/src/da/pdp_potr.rs` (cubierto
  por `integration_tests/tests/da/pdp_potr_simulation.rs`) ahora ejercita
  escenarios de colusion y particion, validando que el schedule PDP/PoTR detecta
  comportamiento bizantino de forma determinista. Siga extendiendolo junto a
  DA-5 para cubrir nuevas superficies de proof.
- La politica de eviction del tier cold requiere un audit trail firmado para
  prevenir drops encubiertos.

### Manipulacion de compromisos

**Escenario:** Secuenciador comprometido publica bloques omitiendo o alterando
compromisos DA, causando fallas de fetch o inconsistencias en clientes ligeros.

**Controles**
- El consenso cruza propuestas de bloques con colas de envio DA; peers rechazan
  propuestas sin compromisos requeridos.
- Clientes ligeros verifican inclusion proofs antes de exponer handles de fetch.
- Audit trail comparando receipts de envio con compromisos de bloque.
- Job de reconciliacion automatizado (`cargo xtask da-commitment-reconcile`)
  compara receipts de ingesta con compromisos DA (SignedBlockWire, `.norito` o
  JSON), emite un bundle JSON de evidencia para gobernanza, y falla ante tickets
  faltantes o no coincidentes para que Alertmanager pagine por omission/tampering.

**Brechas residuales**
- Cubierto por el job de reconciliacion + hook de Alertmanager; los paquetes de
  gobernanza ahora ingieren el bundle JSON de evidencia por defecto.

### Particion de red y censura

**Escenario:** Adversario particiona la red de replicacion, evitando que nodos
obtengan chunks asignados o respondan a desafios PDP/PoTR.

**Controles**
- Requisitos de proveedores multi-region garantizan paths de red diversos.
- Ventanas de desafio incluyen jitter y fallback a canales de reparacion fuera
  de banda.
- Dashboards de observabilidad monitorean profundidad de replicacion, exito de
  desafios y latencia de fetch con umbrales de alerta.

**Brechas residuales**
- Faltan simulaciones de particion para eventos en vivo de Taikai; se requieren
  soak tests.
- La politica de reserva de ancho de banda de reparacion aun no esta codificada.

### Abuso interno

**Escenario:** Operador con acceso al registry manipula politicas de retencion,
whitelistea proveedores maliciosos o suprime alertas.

**Controles**
- Acciones de gobernanza requieren firmas multi-party y registros Norito
  notarizados.
- Cambios de politica emiten eventos a monitoreo y logs de archivo.
- El pipeline de observabilidad aplica logs Norito append-only con hash chaining.
- La automatizacion de revisiones de acceso trimestral
  (`cargo xtask da-privilege-audit`) recorre directorios de manifest/replay
  (mas paths provistos por operadores), marca entradas faltantes/no directorio/
  world-writable, y emite un bundle JSON firmado para dashboards de gobernanza.

**Brechas residuales**
- La evidencia de tamper en dashboards requiere snapshots firmados.

## Registro de riesgos residuales

| Riesgo | Probabilidad | Impacto | Responsable | Plan de mitigacion |
| --- | --- | --- | --- | --- |
| Replay de manifests DA antes de que aterrice el cache de sequence DA-2 | Posible | Moderado | Core Protocol WG | Implementar cache de sequence + validacion de nonce en DA-2; agregar tests de regresion. |
| Colusion PDP/PoTR cuando >f nodos se comprometen | Improbable | Alto | Storage Team | Derivar nuevo schedule de desafios con muestreo cross-provider; validar via harness de simulacion. |
| Brecha de auditoria en eviction del tier cold | Posible | Alto | SRE / Storage Team | Adjuntar logs firmados y receipts on-chain para evictions; monitorear via dashboards. |
| Latencia de deteccion de omision de secuenciador | Posible | Alto | Core Protocol WG | `cargo xtask da-commitment-reconcile` nocturno compara receipts vs compromisos (SignedBlockWire/`.norito`/JSON) y pagina a gobernanza ante tickets faltantes o no coincidentes. |
| Resiliencia a particion para streams en vivo de Taikai | Posible | Critico | Networking TL | Ejecutar drills de particion; reservar ancho de banda de reparacion; documentar SOP de failover. |
| Deriva de privilegios de gobernanza | Improbable | Alto | Governance Council | `cargo xtask da-privilege-audit` trimestral (dirs manifest/replay + paths extra) con JSON firmado + gate de dashboard; anclar artefactos de auditoria on-chain. |

## Follow-ups requeridos

1. Publicar schemas Norito de ingesta DA y vectores de ejemplo (se lleva a DA-2).
2. Enhebrar el replay cache en la ingesta DA de Torii y persistir cursores de
   sequence a traves de reinicios de nodos.
3. **Completado (2026-02-05):** El harness de simulacion PDP/PoTR ahora ejercita
   escenarios de colusion + particion con modelado de backlog QoS; ver
   `integration_tests/src/da/pdp_potr.rs` (con tests en
   `integration_tests/tests/da/pdp_potr_simulation.rs`) para la implementacion y
   los resumenes deterministas capturados abajo.
4. **Completado (2026-05-29):** `cargo xtask da-commitment-reconcile` compara
   receipts de ingesta contra compromisos DA (SignedBlockWire/`.norito`/JSON),
   emite `artifacts/da/commitment_reconciliation.json`, y esta conectado a
   Alertmanager/paquetes de gobernanza para alertas de omission/tampering
   (`xtask/src/da.rs`).
5. **Completado (2026-05-29):** `cargo xtask da-privilege-audit` recorre el spool
   de manifest/replay (mas paths provistos por operadores), marca entradas
   faltantes/no directorio/world-writable, y produce un bundle JSON firmado para
   dashboards/revisiones de gobernanza (`artifacts/da/privilege_audit.json`),
   cerrando la brecha de automatizacion de acceso.

**Donde mirar despues:**

- El replay cache y la persistencia de cursores aterrizaron en DA-2. Ver la
  implementacion en `crates/iroha_core/src/da/replay_cache.rs` (logica de cache)
  y la integracion Torii en `crates/iroha_torii/src/da/ingest.rs`, que enhebra las
  comprobaciones de fingerprint a traves de `/v2/da/ingest`.
- Las simulaciones de streaming PDP/PoTR se ejercitan via el harness proof-stream
  en `crates/sorafs_car/tests/sorafs_cli.rs`, cubriendo flujos de solicitud
  PoR/PDP/PoTR y escenarios de falla animados en el modelo de amenazas.
- Los resultados de capacidad y repair soak viven en
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, mientras que la matriz de
  soak Sumeragi mas amplia se rastrea en `docs/source/sumeragi_soak_matrix.md`
  (con variantes localizadas). Estos artefactos capturan los drills de larga
  duracion referenciados en el registro de riesgos residuales.
- La automatizacion de reconciliacion + privilege-audit vive en
  `docs/automation/da/README.md` y los nuevos comandos
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; use
  las salidas por defecto bajo `artifacts/da/` al adjuntar evidencia a paquetes
  de gobernanza.

## Evidencia de simulacion y modelado QoS (2026-02)

Para cerrar el follow-up DA-1 #3, codificamos un harness de simulacion PDP/PoTR

determinista bajo `integration_tests/src/da/pdp_potr.rs` (cubierto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). El harness asigna nodos

a tres regiones, inyecta particiones/colusion segun las probabilidades del
roadmap, rastrea tardanza PoTR, y alimenta un modelo de backlog de reparacion
que refleja el presupuesto de reparacion del tier hot. Ejecutar el escenario
por defecto (12 epochs, 18 desafios PDP + 2 ventanas PoTR por epoch) produjo
las siguientes metricas:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Metrica | Valor | Notas |
| --- | --- | --- |
| Fallas PDP detectadas | 48 / 49 (98.0%) | Las particiones aun disparan deteccion; una sola falla no detectada viene de jitter honesto. |
| Latencia media de deteccion PDP | 0.0 epochs | Las fallas aparecen dentro del epoch de origen. |
| Fallas PoTR detectadas | 28 / 77 (36.4%) | La deteccion se activa cuando un nodo pierde >=2 ventanas PoTR, dejando la mayoria de eventos en el registro de riesgos residuales. |
| Latencia media de deteccion PoTR | 2.0 epochs | Coincide con el umbral de tardanza de dos epochs incorporado en la escalacion de archivo. |
| Pico de cola de reparacion | 38 manifests | El backlog se dispara cuando las particiones se apilan mas rapido que las cuatro reparaciones disponibles por epoch. |
| Latencia de respuesta p95 | 30,068 ms | Refleja la ventana de desafio de 30 s con jitter de +/-75 ms aplicado para muestreo QoS. |
<!-- END_DA_SIM_TABLE -->

Estos resultados ahora alimentan los prototipos de dashboards DA y satisfacen
los criterios de aceptacion de "simulation harness + QoS modelling" referidos

en el roadmap.

La automatizacion ahora vive detras de
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
que llama al harness compartido y emite Norito JSON a
`artifacts/da/threat_model_report.json` por defecto. Jobs nocturnos consumen
este archivo para refrescar las matrices en este documento y alertar ante deriva

en tasas de deteccion, colas de reparacion o muestras QoS.

Para refrescar la tabla de arriba para docs, ejecute `make docs-da-threat-model`,
que invoca `cargo xtask da-threat-model-report`, regenera
`docs/source/da/_generated/threat_model_report.json`, y reescribe esta seccion
via `scripts/docs/render_da_threat_model_tables.py`. El espejo `docs/portal`
(`docs/portal/docs/da/threat-model.md`) se actualiza en el mismo paso para que

ambas copias se mantengan en sync.
