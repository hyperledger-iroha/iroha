---
lang: es
direction: ltr
source: docs/portal/docs/da/threat-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Refleja `docs/source/da/threat_model.md`. Mantenga ambas versiones en
:::

# Modelo de amenazas de Disponibilidad de Datos de Sora Nexus

_Última revisión: 2026-01-19 -- Próxima revisión programada: 2026-04-19_

Cadencia de mantenimiento: Grupo de Trabajo de Disponibilidad de Datos (<=90 días). Cada
revisión debe aparecer en `status.md` con enlaces a tickets de mitigación activos
y artefactos de simulación.

## Propósito y alcance

El programa de Disponibilidad de Datos (DA) mantiene transmisiones Taikai, blobs de
carril Nexus y artefactos de gobernanza recuperables ante fallas bizantinas,
de red y de operadores. Este modelo de amenazas ancla el trabajo de ingeniería.
para DA-1 (arquitectura y modelo de amenazas) y sirve como base para tareas
DA posteriores (DA-2 a DA-10).

Componentes dentro del alcance:
- Extension de ingesta DA en Torii y escritores de metadata Norito.
- Arboles de almacenamiento de blobs respaldados por SoraFS (tiers hot/cold) y
  políticas de replicación.
- Compromisos de bloques Nexus (formatos de cable, pruebas, API de cliente ligero).
- Ganchos de cumplimiento PDP/PoTR específicos para cargas útiles DA.
- Flujos de operadores (pinning, desalojo, slashing) y ductos de
  observabilidad.
- Aprobaciones de gobernanza que permitirán o expulsan operadores y contenido DA.Fuera de alcance para este documento:
- Modelado económico completo (capturado en el workstream DA-7).
- Protocolos base de SoraFS y cubiertos por el modelo de amenazas de SoraFS.
- Ergonomía de SDK de clientes más alla de consideraciones de superficie de
  amenaza.

## Panorama arquitectonico1. **Envio:** Los clientes envían blobs vía la API de ingesta DA de Torii. el
   nodo trocea blobs, manifiestos codificados Norito (tipo de blob, carril, época,
   flags de codec), y almacena fragmentos en el nivel caliente de SoraFS.
2. **Anuncio:** Intents de pin y tips de replicacion se propagan a proveedores
   de almacenamiento a través del registro (mercado SoraFS) con etiquetas de política
   que indican objetivos de retención frío/calor.
3. **Compromiso:** Los secuenciadores Nexus incluyen compromisos de blob (CID +
   raíces KZG opcionales) en el bloque canónico. Clientes ligeros dependen del
   hash de compromiso y la metadata anunciada para verificar disponibilidad.
4. **Replicación:** Nodos de almacenamiento descargan share/chunks asignados,
   satisfacen desafios PDP/PoTR y promueven datos entre niveles caliente y frío según
   política.
5. **Recuperación:** Los consumidores recuperan datos vía SoraFS o gateways DA-aware,
   verificando pruebas y levantando solicitudes de reparación cuando desaparezcan
   réplicas.
6. **Gobernanza:** Parlamento y el comité de supervisión DA aprueban operadores,
   horarios de alquiler y escaladas de cumplimiento. Artefactos de gobernanza se
   almacenan por la misma ruta DA para garantizar la transparencia del proceso.

## Activos y responsablesEscala de impacto: **Critico** rompe seguridad/vivacidad del ledger; **Alto**
bloques backfill DA o clientes; **Moderado** degrada calidad pero es
recuperable; **Bajo** efecto limitado.

| Activo | Descripción | Integridad | Disponibilidad | Confidencialidad | Responsable |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (fragmentos + manifiestos) | Blobs Taikai, carril y gobernanza almacenados en SoraFS | Crítico | Crítico | Moderado | DA WG / Equipo de almacenamiento |
| Manifiestos Norito DA | Metadatos tipados que describen blobs | Crítico | Alto | Moderado | Grupo de Trabajo sobre el Protocolo Básico |
| Compromisos de bloque | CIDs + root KZG dentro de bloques Nexus | Crítico | Alto | Bajo | Grupo de Trabajo sobre el Protocolo Básico |
| Horarios PDP/PoTR | Cadencia de cumplimiento para réplicas DA | Alto | Alto | Bajo | Equipo de almacenamiento |
| Registro de operadores | Proveedores de almacenamiento aprobados y políticos | Alto | Alto | Bajo | Consejo de Gobierno |
| Registros de renta e incentivos | Libro mayor de entradas para rentas y penalidades DA | Alto | Moderado | Bajo | Grupo de Trabajo de Tesorería |
| Paneles de observabilidad | SLOs DA, profundidad de replicación, alertas | Moderado | Alto | Bajo | SRE / Observabilidad |
| Intentos de reparación | Solicitudes para rehidratar trozos faltantes | Moderado | Moderado | Bajo | Equipo de almacenamiento |

## Adversarios y capacidades| Actor | Capacidades | Motivaciones | Notas |
| --- | --- | --- | --- |
| Cliente malicioso | Enviar blobs malformados, repeticiones de manifiestos obsoletos, intentar DoS en ingesta. | Interrumpir transmite Taikai, inyectar datos invalidos. | Sin claves privilegiadas. |
| Nodo de almacenamiento bizantino | Soltar réplicas asignadas, forjar pruebas PDP/PoTR, coludir con otros. | Reducir la retención DA, evitar renta, retener datos como rehens. | Posee credenciales validas de operador. |
| Secuenciador comprometido | Omitir compromisos, equivocar bloques, reordenar metadatos de blobs. | Ocultar envios DA, crear inconsistencia. | Limitado por la mayoría de consenso. |
| Operador interno | Abusar acceso de gobernanza, manipular políticas de retención, filtrar credenciales. | Ganancia económica, sabotaje. | Acceso a infraestructura frío/calor. |
| Adversario de rojo | Particionar nodos, demorar replicacion, inyectar trafico MITM. | Reducir la disponibilidad, degradar los SLO. | No puede romper TLS pero puede soltar/ralentizar enlaces. |
| Atacante de observabilidad | Manipular cuadros de mando/alertas, suprimir incidentes. | Ocultar caidas DA. | Requiere acceso al tubo de telemetría. |

## Fronteras de confianza- **Frontera de ingreso:** Cliente a extensión DA de Torii. Requiere autenticación por
  request, rate limiting y validación de payload.
- **Frontera de replicación:** Nodos de almacenamiento intercambian trozos y
  pruebas. Los nudos se autentican mutuamente pero pueden comportarse de forma.
  bizantina.
- **Frontera del libro mayor:** Datos de bloque comprometidos vs almacenamiento
  fuera de la cadena. El consenso protege la integridad, pero requiere disponibilidad.
  aplicación de la ley fuera de la cadena.
- **Frontera de gobernanza:** Decisiones de Council/Parliament que aprueban
  operadores, presupuestos y recortes. Fallas aquí impactan directamente el
  despliegue DA.
- **Frontera de observabilidad:** Recoleccion de metrics/logs exportada a
  paneles/herramientas de alerta. La manipulación oculta cortes o ataques.

## Escenarios de amenazas y controles

### Ataques en la ruta de ingesta

**Escenario:** Cliente malicioso envió cargas útiles Norito malformados o blobs
sobredimensionados para agotar recursos o contrabandear metadatos invalidos.**Controles**
- Validación de esquema Norito con negociación estricta de versiones; rechazar
  banderas desconocidas.
- Limitación de tasa y autenticación en el punto final de ingesta Torii.
- Limites de tamaño de fragmento y codificación determinista forzados por el fragmentador SoraFS.
- El pipeline de admision solo persiste manifiesta tras coincidir el checksum de
  integridad.
- Cache de replay determinista (`ReplayCache`) rastrea ventanas `(carril, época,
  secuencia)`, persiste marcas altas en disco, y rechaza duplicados/repeticiones
  obsoletos; arneses de propiedad y fuzz cubren huellas dactilares divergentes y
  envios fuera de orden. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Brechas residuales**
- Torii ingest debe incluir el caché de reproducción en la admisión y persistir cursores
  de secuencia a través de reinicios.
- Los esquemas Norito DA ahora tienen un arnés fuzz dedicado
  (`fuzz/da_ingest_schema.rs`) para estresar invariantes de codificación/decodificación; los
  Los paneles de cobertura deben alertar si el objetivo regresa.

### Retención por retención de replicación

**Escenario:** Operadores de almacenamiento bizantinos aceptan asignaciones de
pin pero sueltan trozos, pasando desafios PDP/PoTR vía respuestas forjadas o
colusión.**Controles**
- El calendario de desafíos PDP/PoTR se extiende a payloads DA con cobertura por
  época.
- Replicación multifuente con umbrales de quorum; el fetch Orchestrator detecta
  shards faltantes y dispara reparacion.
- Slashing de gobernanza vinculado a pruebas fallidas y réplicas faltantes.
- Trabajo de reconciliación automatizado (`cargo xtask da-commitment-reconcile`)
  compara recibos de ingesta con compromisos DA (SignedBlockWire, `.norito` o
  JSON), emite un paquete JSON de evidencia para gobernanza, y falla ante tickets
  faltantes o no coincidencias para que Alertmanager pagine por omisión/manipulación.

**Brechas residuales**
- El arnés de simulación en `integration_tests/src/da/pdp_potr.rs` (cubierto
  por `integration_tests/tests/da/pdp_potr_simulation.rs`) ahora ejercita
  escenarios de colusión y participación, validando que el calendario PDP/PoTR detecta
  Comportamiento bizantino de forma determinista. Siga extendiendolo junto a
  DA-5 para cubrir nuevas superficies de prueba.
- La política de desalojo del nivel frío requiere una pista de auditoría firmada para
  prevenir gotas encubiertos.

### Manipulación de compromisos

**Escenario:** Secuenciador comprometido publica bloques omitiendo o alterando
compromisos DA, provocando fallas de fetch o inconsistencias en clientes ligeros.**Controles**
- El consenso cruza propuestas de bloques con colas de envío DA; sus compañeros rechazan
  propuestas sin compromisos requeridos.
- Clientes ligeros verifican pruebas de inclusión antes de exponer handles de fetch.
- Pista de auditoría comparando recibos de envío con compromisos de bloque.
- Trabajo de conciliación automatizado (`cargo xtask da-commitment-reconcile`)
  compara recibos de ingesta con compromisos DA (SignedBlockWire, `.norito` o
  JSON), emite un paquete JSON de evidencia para gobernanza, y falla ante tickets
  faltantes o no coincidencias para que Alertmanager pagine por omisión/manipulación.

**Brechas residuales**
- Cubierto por el trabajo de reconciliacion + gancho de Alertmanager; los paquetes de
  Gobernanza ahora ingieren el paquete JSON de evidencia por defecto.

### Partición de red y censura

**Escenario:** Adversario particiona la red de replicacion, impidiendo que nodos
obtuvieron fragmentos asignados o respondieron a desafíos PDP/PoTR.

**Controles**
- Los requisitos de proveedores multirregionales garantizan rutas de red diversas.
- Ventanas de desafío incluyen jitter y fallback a canales de reparación fuera
  de banda.
- Dashboards de observabilidad monitorean profundidad de replicacion, exito de
  desafios y latencia de fetch con umbrales de alerta.**Brechas residuales**
- Faltan simulaciones de participación para eventos en vivo de Taikai; se requiere
  pruebas de remojo.
- La politica de reserva de ancho de banda de reparacion aun no esta codificada.

### Abuso interno

**Escenario:** Operador con acceso al registro manipula políticas de retención,
whitelistea proveedores maliciosos o suprime alertas.

**Controles**
- Acciones de gobernanza requieren firmas multipartidistas y registros Norito
  notarizados.
- Cambios de política emiten eventos a monitoreo y registros de archivo.
- El pipeline de observabilidad aplica registros Norito append-only con hash chaining.
- La automatizacion de revision de acceso trimestral
  (`cargo xtask da-privilege-audit`) recuperar directorios de manifest/replay
  (mas caminos provistos por operadores), marca entradas faltantes/no directorio/
  Se puede escribir en todo el mundo y emite un paquete JSON firmado para paneles de gobernanza.

**Brechas residuales**
- La evidencia de manipulación en tableros requiere instantáneas firmadas.

## Registro de riesgos residuales| Riesgo | Probabilidad | Impacto | Responsable | Plan de mitigación |
| --- | --- | --- | --- | --- |
| Reproducción de manifiestos DA antes de que aterrice el caché de secuencia DA-2 | Posible | Moderado | Grupo de Trabajo sobre el Protocolo Básico | Implementar cache de secuencia + validación de nonce en DA-2; Agregar pruebas de regresión. |
| Colusión PDP/PoTR cuando >f nodos se comprometen | Improbable | Alto | Equipo de almacenamiento | Derivar nuevo cronograma de desafíos con muestreo cross-proveedor; validar mediante arnés de simulación. |
| Brecha de auditoria en desalojo del tier cold | Posible | Alto | SRE / Equipo de Almacenamiento | Adjuntar registros firmados y recibos on-chain para desalojos; monitorear a través de tableros de control. |
| Latencia de detección de omisión de secuenciador | Posible | Alto | Grupo de Trabajo sobre el Protocolo Básico | `cargo xtask da-commitment-reconcile` nocturno compara recibos vs compromisos (SignedBlockWire/`.norito`/JSON) y pagina a gobernanza ante tickets faltantes o no coincidentes. |
| Resiliencia a participación para streams en vivo de Taikai | Posible | Crítico | Redes TL | Ejecutar taladros de partición; reservar ancho de banda de reparación; SOP documental de conmutación por error. |
| Deriva de privilegios de gobernanza | Improbable | Alto | Consejo de Gobierno | `cargo xtask da-privilege-audit` trimestral (dirs manifest/replay + paths extra) con JSON firmado + puerta de tablero; anclar artefactos de auditoria on-chain. |

## Seguimientos requeridos1. Publicar esquemas Norito de ingesta DA y vectores de ejemplo (se lleva a DA-2).
2. Enhebrar el caché de reproducción en la ingesta DA de Torii y persistir cursores de
   secuencia a través de reinicios de nodos.
3. **Completado (2026-02-05):** El arnés de simulación PDP/PoTR ahora ejercita
   escenarios de colusión + participación con modelado de backlog QoS; ver
   `integration_tests/src/da/pdp_potr.rs` (pruebas de prueba en
   `integration_tests/tests/da/pdp_potr_simulation.rs`) para la implementación y
   los resúmenes deterministas capturados abajo.
4. **Completado (2026-05-29):** `cargo xtask da-commitment-reconcile` compara
   recibos de ingesta contra compromisos DA (SignedBlockWire/`.norito`/JSON),
   emite `artifacts/da/commitment_reconciliation.json`, y esta conectado a
   Alertmanager/paquetes de gobernanza para alertas de omisión/manipulación
   (`xtask/src/da.rs`).
5. **Completado (2026-05-29):** `cargo xtask da-privilege-audit` recorre el spool
   de manifest/replay (mas caminos provistos por operadores), marca entradas
   faltantes/no directorio/world-writable, y produce un paquete JSON firmado para
   paneles/revisiones de gobernanza (`artifacts/da/privilege_audit.json`),
   cerrando la brecha de automatización de acceso.

**Donde mirar después:**- El caché de reproducción y la persistencia de cursores aterrizaron en DA-2. Ver la
  implementación en `crates/iroha_core/src/da/replay_cache.rs` (lógica de caché)
  y la integracion Torii en `crates/iroha_torii/src/da/ingest.rs`, que enhebra las
  Comprobaciones de huellas dactilares a través de `/v2/da/ingest`.
- Las simulaciones de streaming PDP/PoTR se ejercitan a través del arnésproof-stream
  en `crates/sorafs_car/tests/sorafs_cli.rs`, cubriendo flujos de solicitud
  PoR/PDP/PoTR y escenarios de fallas animadas en el modelo de amenazas.
- Los resultados de capacidad y reparación se empapan viven en
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, mientras que la matriz de
  remojo Sumeragi mas amplia se rastrea en `docs/source/sumeragi_soak_matrix.md`
  (con variantes localizadas). Estos artefactos capturan los taladros de larga
  duración referenciada en el registro de riesgos residuales.
- La automatizacion de reconciliacion + privilegio-audit vive en
  `docs/automation/da/README.md` y los nuevos comandos
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; utilizar
  las salidas por defecto bajo `artifacts/da/` al adjuntar evidencia a paquetes
  de gobernanza.

## Evidencia de simulación y modelado QoS (2026-02)

Para cerrar el seguimiento DA-1 #3, codificamos un arnés de simulación PDP/PoTR

determinista bajo `integration_tests/src/da/pdp_potr.rs` (cubierto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). El arnés asigna nudosa tres regiones, inyecta particiones/colusion segun las probabilidades del
roadmap, rastrea tardanza PoTR, y alimenta un modelo de backlog de reparacion
que refleja el presupuesto de reparación del nivel caliente. Ejecutar el escenario
por defecto (12 épocas, 18 desafíos PDP + 2 ventanas PoTR por época) producido
las siguientes métricas:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | Valor | Notas |
| --- | --- | --- |
| Fallas PDP detectadas | 48 / 49 (98,0%) | Las particiones aun disparan deteccion; una sola falla no detectada viene de jitter honesto. |
| Latencia media de detección PDP | 0,0 épocas | Las fallas aparecen dentro de la época de origen. |
| Fallas PoTR detectadas | 28/77 (36,4%) | La detección se activa cuando un nodo pierde >=2 ventanas PoTR, dejando la mayoría de eventos en el registro de riesgos residuales. |
| Latencia media de detección PoTR | 2.0 épocas | Coincide con el umbral de tardanza de dos épocas incorporado en la escalada de archivo. |
| Pico de cola de reparación | 38 manifiestos | El backlog se dispara cuando las particiones se apilan más rápido que las cuatro reparaciones disponibles por época. |
| Latencia de respuesta p95 | 30.068 ms | Refleje la ventana de desafío de 30 s con jitter de +/-75 ms aplicado para muestreo QoS. |
<!-- END_DA_SIM_TABLE -->Estos resultados ahora alimentan los prototipos de tableros DA y satisfacen
los criterios de aceptación de "arnés de simulación + modelado QoS" referidos

en la hoja de ruta.

La automatizacion ahora vive detras de
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
que llama al arnés compartido y emite Norito JSON a
`artifacts/da/threat_model_report.json` por defecto. Trabajos nocturnos consumidos
este archivo para refrescar las matrices en este documento y alertar ante deriva

en tasas de detección, colas de reparación o muestras QoS.

Para refrescar la tabla de arriba para documentos, ejecute `make docs-da-threat-model`,
que invoca `cargo xtask da-threat-model-report`, regenera
`docs/source/da/_generated/threat_model_report.json`, y reescribe esta sección
vía `scripts/docs/render_da_threat_model_tables.py`. El espejo `docs/portal`
(`docs/portal/docs/da/threat-model.md`) se actualiza en el mismo paso para que

Ambas copias se mantienen sincronizadas.