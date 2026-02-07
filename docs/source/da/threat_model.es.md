---
lang: es
direction: ltr
source: docs/source/da/threat_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0bff91e735291e82d0d50b5dad4dfbf2b57af68f2f7067760add5da81fc7f554
source_last_modified: "2026-01-19T07:28:06.298292+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus Modelo de amenaza de disponibilidad de datos

_Última revisión: 2026-01-19 — Próxima revisión programada: 2026-04-19_

Cadencia de mantenimiento: Grupo de Trabajo de Disponibilidad de Datos (<=90 días). Cada revisión debe
aparecen en `status.md` con enlaces a tickets de mitigación activos y artefactos de simulación.

## Propósito y alcance

El programa de disponibilidad de datos (DA) mantiene las transmisiones de Taikai, los blobs de carril Nexus y
artefactos de gobernanza recuperables bajo fallas bizantinas, de red y de operador.
Este modelo de amenazas sustenta el trabajo de ingeniería para DA-1 (modelo de arquitectura y amenazas)
y sirve como base para las tareas posteriores de DA (DA-2 a DA-10).

Componentes dentro del alcance:
- Extensión de ingesta DA Torii y escritores de metadatos Norito.
- Políticas de replicación y árboles de almacenamiento de blobs respaldados por SoraFS (niveles activos/inactivos).
- Compromisos de bloque Nexus (formatos de conexión, pruebas, API de cliente ligero).
- Ganchos de aplicación de PDP/PoTR específicos para cargas útiles de DA.
- Flujos de trabajo del operador (fijación, desalojo, corte) y canales de observabilidad.
- Aprobaciones de gobernanza que admiten o desalojan a operadores y contenidos de DA.

Fuera del alcance de este documento:
- Modelado económico completo (capturado en el flujo de trabajo DA-7).
- Protocolos base SoraFS ya cubiertos por el modelo de amenaza SoraFS.
- Ergonomía del SDK del cliente más allá de las consideraciones de superficie de amenaza.

## Descripción general arquitectónica

1. **Envío:** Los clientes envían blobs a través de la API de ingesta de DA Torii. el nodo
   fragmenta blobs, codifica manifiestos Norito (tipo de blob, carril, época, indicadores de códec),
   y almacena fragmentos en el nivel activo SoraFS.
2. **Anuncio:** Las intenciones de fijación y las sugerencias de replicación se propagan al almacenamiento
   proveedores a través del registro (mercado SoraFS) con etiquetas de política que
   Indique los objetivos de retención de calor/frío.
3. **Compromiso:** Los secuenciadores Nexus incluyen compromisos de blobs (CID + KZG opcional).
   raíces) en el bloque canónico. Los clientes ligeros confían en el hash de compromiso y
   metadatos anunciados para verificar la disponibilidad.
4. **Replicación:** Los nodos de almacenamiento extraen recursos compartidos/fragmentos asignados y satisfacen PDP/PoTR
   desafíos y promover datos entre niveles fríos y calientes por política.
5. **Obtener:** Los consumidores obtienen datos a través de SoraFS o puertas de enlace compatibles con DA, verificando
   pruebas y plantear solicitudes de reparación cuando las réplicas desaparecen.
6. **Gobernanza:** El Parlamento y el comité de supervisión de la DA aprueban operadores,
   cronogramas de alquiler y escaladas de cumplimiento. Los artefactos de gobernanza se almacenan
   a través de la misma ruta DA para garantizar la transparencia del proceso. Los parámetros de alquiler
   rastreados bajo DA-7 se registran en `docs/source/da/rent_policy.md`, por lo que las auditorías
   y las revisiones de cumplimiento pueden hacer referencia a las cantidades exactas de XOR aplicadas por blob.

## Activos y propietarios

Escala de impacto: **Crítico** rompe la seguridad y vitalidad del libro mayor; **Alto** bloquea DA
reabastecimiento o clientes; **Moderado** degrada la calidad pero sigue siendo recuperable;
**Bajo** efecto limitado.| Activo | Descripción | Integridad | Disponibilidad | Confidencialidad | Propietario |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (fragmentos + manifiestos) | Taikai, carril, blobs de gobernanza almacenados en SoraFS | Crítico | Crítico | Moderado | DA WG / Equipo de almacenamiento |
| Norito Manifiestos DA | Metadatos escritos que describen blobs | Crítico | Alto | Moderado | Grupo de Trabajo sobre el Protocolo Básico |
| Compromisos en bloque | CID + raíces KZG dentro de bloques Nexus | Crítico | Alto | Bajo | Grupo de Trabajo sobre el Protocolo Básico |
| Horarios del PDP/PoTR | Cadencia de aplicación de réplicas de DA | Alto | Alto | Bajo | Equipo de almacenamiento |
| Registro de operadores | Políticas y proveedores de almacenamiento aprobados | Alto | Alto | Bajo | Consejo de Gobierno |
| Registros de alquileres e incentivos | Asientos contables para alquileres y sanciones del DA | Alto | Moderado | Bajo | Grupo de Trabajo de Tesorería |
| Paneles de observabilidad | DA SLO, profundidad de replicación, alertas | Moderado | Alto | Bajo | SRE / Observabilidad |
| Intentos de reparación | Solicitudes para rehidratar los trozos faltantes | Moderado | Moderado | Bajo | Equipo de almacenamiento |

## Adversarios y Capacidades

| Actor | Capacidades | Motivaciones | Notas |
| --- | --- | --- | --- |
| Cliente malicioso | Envíe blobs con formato incorrecto, reproduzca manifiestos obsoletos, intente DoS durante la ingesta. | Interrumpe las transmisiones de Taikai, inyecta datos no válidos. | Sin claves privilegiadas. |
| Nodo de almacenamiento bizantino | Eliminar réplicas asignadas, falsificar pruebas PDP/PoTR, confabularse con otros. | Reduzca la retención de DA, evite el alquiler, mantenga los datos como rehenes. | Posee credenciales de operador válidas. |
| Secuenciador comprometido | Omitir compromisos, equivocarse en bloques, reordenar los metadatos de blobs. | Ocultar envíos de DA, crear inconsistencia. | Limitado por mayoría de consenso. |
| Operador interno | Abusar del acceso a la gobernanza, alterar las políticas de retención, filtrar credenciales. | Ganancia económica, sabotaje. | Acceso a infraestructura de nivel frío/caliente. |
| Adversario de la red | Particionar nodos, retrasar la replicación, inyectar tráfico MITM. | Reducir la disponibilidad, degradar los SLO. | No se puede romper TLS pero puede soltar o ralentizar enlaces. |
| Atacante de observabilidad | Manipular paneles/alertas, suprimir incidentes. | Ocultar interrupciones de DA. | Requiere acceso a la canalización de telemetría. |

## Límites de confianza

- **Límite de ingreso:** Cliente a la extensión DA Torii. Requiere autenticación a nivel de solicitud,
  limitación de velocidad y validación de carga útil.
- **Límite de replicación:** Nodos de almacenamiento que intercambian fragmentos y pruebas. Los nodos son
  mutuamente autenticados pero pueden comportarse como bizantinos.
- **Límite del libro mayor:** Datos de bloque comprometidos versus almacenamiento fuera de la cadena. Guardias del consenso
  integridad, pero la disponibilidad requiere aplicación fuera de la cadena.
- **Límite de gobernanza:** Decisiones del Consejo/Parlamento que aprueban operadores,
  presupuestos y recortes. Las interrupciones aquí impactan directamente en la implementación de DA.
- **Límite de observabilidad:** Métricas/recopilación de registros exportados a paneles/alertas
  herramientas. La manipulación oculta interrupciones o ataques.

## Escenarios de amenazas y controles

### Ataques de ruta de ingesta**Escenario:** El cliente malintencionado envía cargas útiles Norito con formato incorrecto o de gran tamaño
blobs para agotar recursos o contrabandear metadatos no válidos.

**Controles**
- Validación del esquema Norito con negociación estricta de la versión; rechazar banderas desconocidas.
- Limitación de velocidad y autenticación en el punto final de ingesta Torii.
- Límites de tamaño de fragmento y codificación determinista aplicada por el fragmento SoraFS.
- La canalización de admisión solo conserva los manifiestos después de que coincida la suma de comprobación de integridad.
- La caché de reproducción determinista (`ReplayCache`) rastrea las ventanas `(lane, epoch, sequence)`, conserva marcas altas en el disco y rechaza repeticiones duplicadas o obsoletas; Los arneses de propiedad y fuzz cubren huellas dactilares divergentes y envíos fuera de orden.

**Brechas residuales**
- La ingesta de Torii debe enhebrar el caché de reproducción en la admisión y persistir los cursores de secuencia durante los reinicios.
- Los esquemas DA Norito ahora tienen un arnés fuzz dedicado (`fuzz/da_ingest_schema.rs`) para enfatizar las invariantes de codificación/decodificación; Los paneles de cobertura deben alertar si el objetivo retrocede.

### Retención de replicación

**Escenario:** Los operadores de almacenamiento bizantinos aceptan asignaciones de pines pero descartan fragmentos,
pasar los desafíos del PDP/PoTR a través de respuestas falsificadas o colusión.

**Controles**
- El cronograma de desafío PDP/PoTR se extiende a las cargas útiles de DA con cobertura por época.
- Replicación de múltiples fuentes con umbrales de quórum; buscar orquestador detecta
  Fragmentos faltantes y reparación de desencadenantes.
- Recorte de la gobernanza vinculado a pruebas fallidas y réplicas faltantes.

**Brechas residuales**
- Arnés de simulación en `integration_tests/src/da/pdp_potr.rs` (cubierto por
  `integration_tests/tests/da/pdp_potr_simulation.rs`) ahora ejerce colusión
  y escenarios de partición, validando que el cronograma PDP/PoTR detecte
  Comportamiento bizantino de manera determinista. Continúe ampliándolo junto a la DA-5 hasta
  Cubra nuevas superficies de prueba.
- La política de desalojo de nivel frío requiere un registro de auditoría firmado para evitar caídas encubiertas.

### Manipulación de compromiso

**Escenario:** El secuenciador comprometido publica bloques omitiendo o alterando DA
compromisos, provocando fallas en la recuperación o inconsistencias en el cliente ligero.

**Controles**
- El consenso verifica las propuestas de bloque con las colas de envío de DA; los compañeros rechazan
  a las propuestas les faltan los compromisos requeridos.
- Los clientes ligeros verifican las pruebas de inclusión de compromiso antes de salir a la superficie para buscar identificadores.
- Pista de auditoría que compara los recibos de envío con los compromisos en bloque.
- El trabajo de conciliación automatizado (`cargo xtask da-commitment-reconcile`) compara
  ingerir recibos con compromisos de DA (SignedBlockWire, `.norito` o JSON),
  emite un paquete de evidencia JSON para la gobernanza y falla si falta o
  tickets no coincidentes para que Alertmanager pueda localizar omisiones/manipulación.

**Brechas residuales**
- Cubierto por el trabajo de conciliación + gancho Alertmanager; paquetes de gobernanza ahora
  ingiera el paquete de pruebas JSON de forma predeterminada.

### Partición de red y censura**Escenario:** El adversario divide la red de replicación, evitando que los nodos
obtener fragmentos asignados o responder a desafíos de PDP/PoTR.

**Controles**
- Los requisitos de proveedores multirregionales garantizan diversas rutas de red.
- Las ventanas de desafío incluyen fluctuación y retroceso a canales de reparación fuera de banda.
- Los paneles de observabilidad monitorean la profundidad de la replicación, el éxito del desafío y
  recuperar latencia con umbrales de alerta.

**Brechas residuales**
- Aún faltan simulaciones de partición para eventos en vivo de Taikai; Necesita pruebas de remojo.
- Reparar la política de reserva de ancho de banda aún no codificada.

### Abuso interno

**Escenario:** El operador con acceso al registro manipula las políticas de retención,
incluye en la lista blanca proveedores maliciosos o suprime alertas.

**Controles**
- Las acciones de gobernanza requieren firmas de varias partes y registros notariados Norito.
- Los cambios de política emiten eventos a los registros de monitoreo y archivo.
- La canalización de observabilidad aplica registros Norito de solo agregar con encadenamiento hash.
- Paseos de automatización de revisión de acceso trimestral (`cargo xtask da-privilege-audit`)
  los directorios de manifiesto/reproducción de DA (más las rutas proporcionadas por el operador), banderas
  entradas faltantes/que no pertenecen al directorio/que se pueden escribir en todo el mundo y emite un paquete JSON firmado
  para paneles de control de gobernanza.

**Brechas residuales**
- La prueba de manipulación del panel requiere instantáneas firmadas.

## Registro de riesgos residuales

| Riesgo | Probabilidad | Impacto | Propietario | Plan de Mitigación |
| --- | --- | --- | --- | --- |
| Reproducción de manifiestos DA antes de que llegue el caché de secuencia DA-2 | Posible | Moderado | Grupo de Trabajo sobre el Protocolo Básico | Implementar caché de secuencia + validación nonce en DA-2; agregar pruebas de regresión. |
| Colusión PDP/PoTR cuando >f nodos se comprometen | Improbable | Alto | Equipo de almacenamiento | Derivar un nuevo cronograma de desafío con muestreo entre proveedores; validar mediante arnés de simulación. |
| Brecha en la auditoría de desalojos de nivel frío | Posible | Alto | SRE / Equipo de Almacenamiento | Adjunte registros de auditoría firmados y recibos en cadena para desalojos; monitorear a través de paneles de control. |
| Latencia de detección de omisiones del secuenciador | Posible | Alto | Grupo de Trabajo sobre el Protocolo Básico | Todas las noches, `cargo xtask da-commitment-reconcile` compara recibos con compromisos (SignedBlockWire/`.norito`/JSON) y controla las páginas sobre tickets faltantes o que no coinciden. |
| Resiliencia de partición para transmisiones en vivo de Taikai | Posible | Crítico | Redes TL | Ejecutar simulacros de partición; reservar ancho de banda de reparación; SOP de conmutación por error de documentos. |
| Deriva de los privilegios de gobernanza | Improbable | Alto | Consejo de Gobierno | Ejecución trimestral de `cargo xtask da-privilege-audit` (directorios de manifiesto/reproducción + rutas adicionales) con JSON firmado + puerta del panel; anclar artefactos de auditoría en la cadena. |

## Seguimientos requeridos1. Publicar esquemas de ingesta Norito de DA y vectores de ejemplo (incorporados a DA-2).
2. Pase el caché de reproducción a través de los cursores de secuencia de ingesta y persistencia de DA Torii durante los reinicios del nodo.
3. **Terminado (05/02/2026):** El arnés de simulación PDP/PoTR ahora ejercita escenarios de colusión + partición con modelado de acumulación de QoS; consulte [`integration_tests/src/da/pdp_potr.rs`](/integration_tests/src/da/pdp_potr.rs) (con pruebas en `integration_tests/tests/da/pdp_potr_simulation.rs`) para conocer la implementación y los resúmenes deterministas que se muestran a continuación.
4. **Completado (29 de mayo de 2026):** `cargo xtask da-commitment-reconcile` compara los recibos de ingesta con los compromisos de DA (SignedBlockWire/`.norito`/JSON), emite `artifacts/da/commitment_reconciliation.json` y está conectado a Alertmanager/paquetes de gobernanza para alertas de omisión/manipulación (`xtask/src/da.rs`).
5. **Completado (29 de mayo de 2026):** `cargo xtask da-privilege-audit` recorre el spool de manifiesto/reproducción (más las rutas proporcionadas por el operador), marca entradas faltantes/que no son de directorio/escribibles mundialmente y produce un paquete JSON firmado para paneles/revisiones de gobernanza (`artifacts/da/privilege_audit.json`), cerrando la brecha de automatización de acceso-revisión.

**Dónde buscar a continuación:**

- La caché de reproducción de DA y la persistencia del cursor llegaron a DA-2. Ver el
  implementación en `crates/iroha_core/src/da/replay_cache.rs` (lógica de caché) y
  la integración Torii en `crates/iroha_torii/src/da/ingest.rs`, que enhebra el
  Verificaciones de huellas dactilares a través de `/v1/da/ingest`.
- Las simulaciones de transmisión PDP/PoTR se realizan a través del arnés de transmisión de prueba en
  `crates/sorafs_car/tests/sorafs_cli.rs`, que cubre los flujos de solicitud PoR/PDP/PoTR
  y escenarios de falla animados en el modelo de amenaza.
- Los resultados de capacidad y remoción de reparación están disponibles en
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, mientras que el más amplio
  La matriz de remojo Sumeragi se rastrea en `docs/source/sumeragi_soak_matrix.md`
  (variantes localizadas incluidas). Estos artefactos capturan los simulacros de larga duración.
  referenciados en el registro de riesgos residuales.
- La automatización de la conciliación y la auditoría de privilegios existe
  `docs/automation/da/README.md` y el nuevo `cargo xtask da-commitment-reconcile`
  / comandos `cargo xtask da-privilege-audit`; utilice las salidas predeterminadas en
  `artifacts/da/` al adjuntar evidencia a los paquetes de gobernanza.

## Evidencia de simulación y modelado de QoS (2026-02)

Para cerrar el seguimiento #3 de DA-1, codificamos una simulación determinista de PDP/PoTR
arnés bajo `integration_tests/src/da/pdp_potr.rs` (cubierto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). el arnés
Asigna nodos en tres regiones, inyecta particiones/colusión según
las probabilidades de la hoja de ruta, rastrea los retrasos de PoTR y alimenta una acumulación de reparaciones
modelo que refleja el presupuesto de reparación de nivel más alto. Ejecutando el escenario predeterminado
(12 épocas, 18 desafíos PDP + 2 ventanas PoTR por época) produjeron el
siguientes métricas:<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | Valor | Notas |
| --- | --- | --- |
| Fallos de PDP detectados | 48 / 49 (98,0%) | Las particiones aún activan la detección; un solo fallo no detectado proviene de un nerviosismo sincero. |
| Latencia media de detección de PDP | 0,0 épocas | Los fracasos surgen en la época de origen. |
| Fallos PoTR detectados | 28/77 (36,4%) | La detección se activa una vez que un nodo pierde ≥2 ventanas PoTR, lo que deja la mayoría de los eventos en el registro de riesgo residual. |
| Latencia media de detección de PoTR | 2.0 épocas | Coincide con el umbral de retraso de dos épocas incluido en la escalada de archivos. |
| Pico de cola de reparación | 38 manifiestos | El trabajo pendiente aumenta cuando las particiones se acumulan más rápido que las cuatro reparaciones disponibles por época. |
| Latencia de respuesta p95 | 30.068 ms | Refleja la ventana de desafío de 30 s con la fluctuación de ±75 ms aplicada para el muestreo de QoS. |
<!-- END_DA_SIM_TABLE -->

Estas salidas ahora impulsan los prototipos del tablero de DA y satisfacen la "simulación".
Arnés + modelado QoS” criterios de aceptación a los que se hace referencia en la hoja de ruta.

La automatización ahora reside detrás de `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`, que llama al arnés compartido y
emite Norito JSON a `artifacts/da/threat_model_report.json` de forma predeterminada. todas las noches
Los trabajos consumen este archivo para actualizar las matrices en este documento y alertar sobre
variación en las tasas de detección, colas de reparación o muestras de QoS.

Para actualizar la tabla anterior para documentos, ejecute `make docs-da-threat-model`, que
invoca `cargo xtask da-threat-model-report`, regenera
`docs/source/da/_generated/threat_model_report.json`, y reescribe esta sección
vía `scripts/docs/render_da_threat_model_tables.py`. El espejo `docs/portal`
(`docs/portal/docs/da/threat-model.md`) se actualiza en la misma pasada para que ambos
las copias permanecen sincronizadas.