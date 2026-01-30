---
lang: es
direction: ltr
source: docs/source/nexus.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8da33b0abb8a6d46dbaaed657c8338a9d723a97f6f28ff29a62caf84c0dbfd6
source_last_modified: "2025-12-27T07:56:34.355655+00:00"
translation_last_reviewed: 2026-01-01
---

#! Iroha 3 - Sora Nexus Ledger: Especificacion tecnica de diseno

Este documento propone la arquitectura del Sora Nexus Ledger para Iroha 3, evolucionando Iroha 2 hacia un ledger global unico, logicamente unificado, organizado alrededor de Data Spaces (DS). Los Data Spaces proveen dominios de privacidad fuertes ("private data spaces") y participacion abierta ("public data spaces"). El diseno preserva la componibilidad a traves del ledger global mientras garantiza aislamiento estricto y confidencialidad para los datos de DS privados, e introduce escalado de disponibilidad de datos via erasure coding en Kura (almacen de bloques) y WSV (World State View).

El mismo repositorio construye tanto Iroha 2 (redes autoalojadas) como Iroha 3 (SORA Nexus). La ejecucion esta impulsada por la Maquina Virtual de Iroha (IVM) y la cadena de herramientas Kotodama compartidas, por lo que contratos y artefactos de bytecode siguen siendo portables entre despliegues autoalojados y el ledger global Nexus.

Objetivos
- Un ledger logico global compuesto por muchos validadores cooperativos y Data Spaces.
- Data Spaces privados para operacion con permisos (ej., CBDCs), con datos que nunca salen del DS privado.
- Data Spaces publicos con participacion abierta, acceso sin permisos estilo Ethereum.
- Contratos inteligentes componibles entre Data Spaces, sujetos a permisos explicitos para acceso a activos de DS privados.
- Aislamiento de rendimiento para que la actividad publica no degrade transacciones internas de DS privados.
- Disponibilidad de datos a escala: Kura y WSV con erasure coding para soportar datos efectivamente ilimitados manteniendo privados los datos de DS privados.

No-Objetivos (Fase inicial)
- Definir economia de tokens o incentivos de validadores; la planificacion y el staking son enchufables.
- Introducir una nueva version de ABI o ampliar superficies de syscalls/pointer-ABI; ABI v1 es fijo y las runtime upgrades no cambian el ABI del host.

Terminologia
- Nexus Ledger: El ledger logico global formado al componer bloques de Data Space (DS) en un historial unico y ordenado y un compromiso de estado.
- Data Space (DS): Un dominio de ejecucion y almacenamiento acotado con sus propios validadores, gobernanza, clase de privacidad, politica de DA, cuotas y politica de fees. Existen dos clases: DS publico y DS privado.
- Private Data Space: Validadores con permisos y control de acceso; los datos de transaccion y estado nunca salen del DS. Solo se anclan compromisos/metadata globalmente.
- Public Data Space: Participacion sin permisos; datos completos y estado disponibles publicamente.
- Data Space Manifest (DS Manifest): Un manifiesto codificado en Norito que declara parametros de DS (validadores/llaves QC, clase de privacidad, politica ISI, parametros de DA, retencion, cuotas, politica ZK, fees). El hash del manifiesto se ancla en la cadena nexus. Salvo que se indique lo contrario, los quorum certificates de DS usan ML-DSA-87 (clase Dilithium5) como esquema de firma post-quantum por defecto.
- Space Directory: Un contrato directorio global en cadena que rastrea manifiestos de DS, versiones y eventos de gobernanza/rotacion para resolubilidad y auditorias.
- DSID: Un identificador unico global para un Data Space. Se usa para namespacing de todos los objetos y referencias.
- Anchor: Un compromiso criptografico de un bloque/header de DS incluido en la cadena nexus para enlazar la historia del DS con el ledger global.
- Kura: Almacen de bloques de Iroha. Aqui se extiende con almacenamiento blob con erasure coding y compromisos.
- WSV: World State View de Iroha. Aqui se extiende con segmentos de estado versionados, con snapshots y erasure coding.
- IVM: Iroha Virtual Machine para ejecucion de contratos inteligentes (bytecode Kotodama `.to`).
 - AIR: Algebraic Intermediate Representation. Una vista algebraica del computo para pruebas estilo STARK, describiendo la ejecucion como trazas basadas en campos con restricciones de transicion y borde.

Modelo de Data Spaces
- Identidad: `DataSpaceId (DSID)` identifica un DS y nombra todo. DS puede instanciarse en dos granularidades:
  - Domain-DS: `ds::domain::<domain_name>` - ejecucion y estado limitados a un dominio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - ejecucion y estado limitados a una sola definicion de asset.
  Ambas formas coexisten; las transacciones pueden tocar multiples DSID atomicamente.
- Ciclo de vida del manifiesto: creacion de DS, actualizaciones (rotacion de llaves, cambios de politica) y retiro se registran en el Space Directory. Cada artefacto por slot referencia el hash del manifiesto mas reciente.
- Clases: DS publico (participacion abierta, DA publica) y DS privado (con permisos, DA confidencial). Politicas hibridas son posibles via flags del manifiesto.
- Politicas por DS: permisos ISI, parametros DA `(k,m)`, cifrado, retencion, cuotas (participacion min/max de tx por bloque), politica de pruebas ZK/optimistas, fees.
- Gobernanza: membresia DS y rotacion de validadores definidas por la seccion de gobernanza del manifiesto (propuestas en cadena, multisig, o gobernanza externa anclada por transacciones y attestations nexus).

Gossip consciente de dataspace
- Los lotes de gossip de transacciones ahora llevan una etiqueta de plano (publico vs restringido) derivada del catalogo de lanes; los lotes restringidos se envian por unicast a los peers en linea de la topologia de commit actual (respetando `transaction_gossip_restricted_target_cap`) mientras que los lotes publicos usan `transaction_gossip_public_target_cap` (configurar `null` para broadcast). La seleccion de objetivos se remezcla con la cadencia por plano definida por `transaction_gossip_public_target_reshuffle_ms` y `transaction_gossip_restricted_target_reshuffle_ms` (por defecto: `transaction_gossip_period_ms`). Cuando no hay peers en linea de la topologia de commit, los operadores pueden elegir rechazar o reenviar payloads restringidos al overlay publico via `transaction_gossip_restricted_public_payload` (por defecto `refuse`); la telemetria expone intentos de fallback, conteos de reenvio/descarta, y la politica configurada junto con selecciones de objetivos por dataspace.
- Dataspaces desconocidos se re-encolan cuando `transaction_gossip_drop_unknown_dataspace` esta habilitado; de lo contrario caen a targeting restringido para evitar filtraciones.
- La validacion del lado receptor descarta entradas cuyas lanes/dataspaces no concuerdan con el catalogo local, cuya etiqueta de plano no coincide con la visibilidad del dataspace derivada, o cuya ruta anunciada no coincide con la decision de enrutamiento re-derivada localmente.

Manifiestos de capacidades y UAID
- Cuentas universales: Cada participante recibe un UAID determinista (`UniversalAccountId` en `crates/iroha_data_model/src/nexus/manifest.rs`) que abarca todos los dataspaces. Los manifiestos de capacidades (`AssetPermissionManifest`) vinculan un UAID a un dataspace especifico, epochs de activacion/expiracion y una lista ordenada de reglas `ManifestEntry` allow/deny que delimitan `dataspace`, `program_id`, `method`, `asset` y roles AMX opcionales. Las reglas deny siempre ganan; el evaluador emite `ManifestVerdict::Denied` con un motivo de auditoria o un grant `Allowed` con la metadata de la asignacion que coincide.
- Los snapshots de portafolio UAID se exponen ahora via `GET /v1/accounts/{uaid}/portfolio` (ver `docs/source/torii/portfolio_api.md`), respaldados por el agregador determinista en `iroha_core::nexus::portfolio`.
- Allowances: Cada entrada allow lleva buckets `AllowanceWindow` deterministas (`PerSlot`, `PerMinute`, `PerDay`) mas un `max_amount` opcional. Hosts y SDKs consumen el mismo payload Norito, por lo que la aplicacion se mantiene identica entre hardware y SDKs.
- Telemetria de auditoria: El Space Directory emite `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) cada vez que un manifiesto cambia de estado. La nueva superficie `SpaceDirectoryEventFilter` permite a suscriptores Torii/data-event monitorear actualizaciones, revocaciones y decisiones de deny-wins sin plomeria personalizada.

### Operaciones de manifiestos UAID

Las operaciones del Space Directory se entregan en dos formas para que los operadores elijan el
CLI interno (para despliegues con scripts) o envios directos a Torii (para
CI/CD automatizado). Ambos caminos aplican el permiso `CanPublishSpaceDirectoryManifest{dataspace}`
 dentro del ejecutor (`crates/iroha_core/src/smartcontracts/isi/space_directory.rs`)
y registran eventos del ciclo de vida en el world state (`iroha_core::state::space_directory_manifests`).

#### Flujo de trabajo CLI (`iroha app space-directory manifest ...`)

1. **Codificar JSON del manifiesto** - convertir borradores de politica a bytes Norito y emitir un
   hash reproducible antes de revision:

   ```bash
   iroha app space-directory manifest encode \
     --json dataspace/capability.json \
     --out artifacts/capability.manifest.to \
     --hash-out artifacts/capability.manifest.hash
   ```

   El helper acepta `--json` (manifiesto JSON en crudo) o `--manifest` (payload `.to` existente)
   y refleja la logica en
   `crates/iroha_cli/src/space_directory.rs::ManifestEncodeArgs`.

2. **Publicar/reemplazar manifiestos** - encolar instrucciones `PublishSpaceDirectoryManifest`
   desde fuentes Norito o JSON:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/capability.manifest.to \
     --reason "Retail wave 4 on-boarding"
   ```

   `--reason` completa `entries[*].notes` para registros que omitieron notas del operador.

3. **Expirar** manifiestos que llegaron al fin de vida programado o **revocar**
   UAIDs bajo demanda. Ambos comandos aceptan `--uaid uaid:<hex>` o un digest hex
   de 64 caracteres (LSB=1) y el id numerico de dataspace:

   ```bash
   iroha app space-directory manifest expire \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --expired-epoch 4600

   iroha app space-directory manifest revoke \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --revoked-epoch 9216 \
     --reason "Fraud investigation NX-16-R05"
   ```

4. **Producir bundles de auditoria** - `manifest audit-bundle` escribe el JSON del manifiesto,
   payload `.to`, hash, perfil de dataspace y metadata legible por maquina en un
   directorio de salida para que revisores de gobernanza descarguen un solo archivo:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest-json dataspace/capability.json \
     --profile dataspace/profiles/cbdc_profile.json \
     --out-dir artifacts/capability_bundle
   ```

   El bundle incrusta hooks `SpaceDirectoryEvent` desde el perfil para probar que el
   dataspace expone los webhooks de auditoria obligatorios; ver `docs/space-directory.md`
   para el layout de campos y requisitos de evidencia.

#### APIs Torii

Operadores y SDKs pueden ejecutar las mismas acciones via HTTPS. Torii aplica los
mismos chequeos de permiso y firma transacciones en nombre de la autoridad
suministrada (las llaves privadas viajan solo en memoria dentro del handler seguro de Torii):

- `GET /v1/space-directory/uaids/{uaid}` - resolver las vinculaciones actuales de dataspace
  para un UAID (direcciones normalizadas, ids de dataspace, bindings de programa). Agregar
  `address_format=compressed` para salida de Sora Name Service (IH58 es preferido; compressed (`sora`) es segunda mejor opcion Sora-only).
- `GET /v1/space-directory/uaids/{uaid}/portfolio` -
  agregador respaldado por Norito que refleja `ToriiClient.getUaidPortfolio` para que wallets
  muestren holdings universales sin raspar estado por dataspace.
- `GET /v1/space-directory/uaids/{uaid}/manifests?dataspace={id}` - obtener el JSON del manifiesto
  canonico, metadata de ciclo de vida y hash del manifiesto para auditorias.
- `POST /v1/space-directory/manifests` - enviar manifiestos nuevos o de reemplazo
  desde JSON (`authority`, `private_key`, `manifest`, `reason` opcional). Torii
  devuelve `202 Accepted` una vez que la transaccion esta en cola.
- `POST /v1/space-directory/manifests/revoke` - encolar revocaciones de emergencia
  con UAID, id de dataspace, epoch efectivo y motivo opcional (refleja el layout del
  CLI).

El SDK JS (`javascript/iroha_js/src/toriiClient.js`) ya envuelve estas superficies de lectura
via `ToriiClient.getUaidPortfolio`, `.getUaidBindings` y
`.getUaidManifests`; futuras versiones Swift/Python reutilizan los mismos payloads REST.
Referenciar `docs/source/torii/portfolio_api.md` para esquemas completos de request/response y
`docs/space-directory.md` para el playbook de operador end-to-end.

Actualizaciones recientes de SDK/AMX
- **NX-11 (verificacion de relay cross-lane):** helpers de SDK ahora validan los
  envelopes de relay de lane expuestos por `/v1/sumeragi/status`. El cliente Rust envia
  helpers `iroha::nexus` para construir/verificar pruebas de relay y rechazar tuplas
  duplicadas `(lane_id, dataspace_id, height)`, el binding Python expone
  `verify_lane_relay_envelope_bytes`/`lane_settlement_hash`, y el SDK JS expone
  `verifyLaneRelayEnvelope`/`laneRelayEnvelopeSample` para que operadores puedan validar
  pruebas de transferencia cross-lane con hashes consistentes antes de reenviarlas.
  (crates/iroha/src/nexus.rs:1, python/iroha_python/iroha_python_rs/src/lib.rs:666, crates/iroha_js_host/src/lib.rs:640, javascript/iroha_js/src/nexus.js:1)
- **NX-17 (guardrails de presupuesto AMX):** `ivm::analysis::enforce_amx_budget` estima
  el costo de ejecucion por dataspace y grupo usando el reporte de analisis estatico y
  aplica los presupuestos de 30 ms / 140 ms capturados aqui. El helper expone violaciones
  claras para presupuestos por DS y grupo y esta cubierto por pruebas unitarias,
  haciendo el presupuesto de slots AMX determinista para planificadores Nexus y tooling
  de SDK. (crates/ivm/src/analysis.rs:142, crates/ivm/src/analysis.rs:241)

Arquitectura de alto nivel
1) Capa de composicion global (Nexus Chain)
- Mantiene un orden canonico unico de bloques Nexus de 1 s que finalizan transacciones atomicas que abarcan uno o mas Data Spaces (DS). Cada transaccion comprometida actualiza el world state global unificado (vector de raices por DS).
- Contiene metadata minima mas pruebas/QCs agregadas para asegurar componibilidad, finalidad y deteccion de fraude (DSIDs tocados, raices de estado por DS antes/despues, compromisos DA, pruebas de validez por DS y el quorum certificate de DS usando ML-DSA-87). No se incluye data privada.
- Consenso: Comite BFT global, canalizado, de tamano 22 (3f+1 con f=7), seleccionado de un pool de hasta ~200k validadores potenciales por un mecanismo VRF/stake por epoch. El comite nexus secuencia transacciones y finaliza el bloque dentro de 1 s.

2) Capa Data Space (Publica/Privada)
- Ejecuta fragmentos por DS de transacciones globales, actualiza WSV local del DS y produce artefactos de validez por bloque (pruebas por DS agregadas y compromisos DA) que se agregan en el bloque Nexus de 1 s.
- DS privados cifran datos en reposo y en vuelo entre validadores autorizados; solo compromisos y pruebas PQ de validez salen del DS.
- DS publicos exportan cuerpos de datos completos (via DA) y pruebas PQ de validez.

3) Transacciones atomicas cross-Data-Space (AMX)
- Modelo: Cada transaccion de usuario puede tocar multiples DS (por ejemplo, dominio DS y uno o mas asset DS). Se compromete atomicamente en un bloque Nexus de 1 s o aborta; no hay efectos parciales.
- Preparar-commit en 1 s: Para cada transaccion candidata, los DS tocados ejecutan en paralelo contra el mismo snapshot (raices DS de inicio de slot) y producen pruebas PQ de validez por DS (FASTPQ-ISI) y compromisos DA. El comite nexus compromete la transaccion solo si todas las pruebas DS requeridas verifican y los certificados DA llegan (objetivo <=300 ms); de lo contrario la transaccion se reprograma para el siguiente slot.
- Consistencia: Se declaran conjuntos de lectura-escritura; la deteccion de conflictos ocurre al commit contra las raices de inicio de slot. La ejecucion optimista sin locks por DS evita bloqueos globales; la atomicidad se impone por la regla de commit nexus (todo-o-nada entre DS).
- Privacidad: DS privados exportan solo pruebas/compromisos ligados a raices DS pre/post. No sale data privada cruda del DS.

4) Disponibilidad de datos (DA) con erasure coding
- Kura almacena cuerpos de bloque y snapshots de WSV como blobs con erasure coding. Los blobs publicos se fragmentan ampliamente; los blobs privados se almacenan solo dentro de validadores DS privados, con chunks cifrados.
- Los compromisos DA se registran en artefactos DS y bloques Nexus, habilitando muestreo y recuperacion sin revelar contenidos privados.

Estructura de bloque y commit
- Artefacto de prueba de Data Space (por slot de 1 s, por DS)
  - Campos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - DS privados exportan artefactos sin cuerpos de datos; DS publicos permiten recuperar cuerpos via DA.

- Bloque Nexus (cadencia de 1 s)
  - Campos: block_number, parent_hash, slot_time, tx_list (transacciones atomicas cross-DS con DSIDs tocados), ds_artifacts[], nexus_qc.
  - Funcion: finaliza todas las transacciones atomicas cuyas artefactos DS requeridos verifican; actualiza el vector de world state global de raices DS en un paso.

Consenso y planificacion
- Consenso de la Nexus Chain: BFT global, canalizado (clase Sumeragi) con un comite de 22 nodos (3f+1 con f=7) apuntando a bloques de 1 s y finality de 1 s. Los miembros del comite se seleccionan por epoch via VRF/stake de ~200k candidatos; la rotacion mantiene descentralizacion y resistencia a censura.
- Consenso de Data Space: Cada DS ejecuta su propio BFT entre sus validadores para producir artefactos por slot (pruebas, compromisos DA, DS QC). Los comites lane-relay se dimensionan a `3f+1` usando el valor `fault_tolerance` del dataspace y se muestrean de forma determinista por epoch desde el pool de validadores del dataspace usando la semilla VRF del epoch vinculada con `(dataspace_id, lane_id)`. Los DS privados tienen permisos; los DS publicos permiten liveness abierta sujeta a politicas anti-Sybil. El comite global nexus permanece sin cambios.
- Planificacion de transacciones: Los usuarios envian transacciones atomicas declarando DSIDs tocados y conjuntos de lectura-escritura. Los DS ejecutan en paralelo dentro del slot; el comite nexus incluye la transaccion en el bloque de 1 s si todos los artefactos DS verifican y los certificados DA son oportunos (<=300 ms).
- Aislamiento de rendimiento: Cada DS tiene mempools y ejecucion independientes. Las cuotas por DS limitan cuantas transacciones que tocan un DS pueden confirmarse por bloque para evitar head-of-line blocking y proteger la latencia de DS privados.

Modelo de datos y namespacing
- IDs calificados por DS: Todas las entidades (dominios, cuentas, assets, roles) se califican por `dsid`. Ejemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias globales: Una referencia global es una tupla `(dsid, object_id, version_hint)` y puede colocarse en cadena en la capa nexus o en descriptores AMX para uso cross-DS.
- Serializacion Norito: Todos los mensajes cross-DS (descriptores AMX, pruebas) usan codecs Norito. No se usa serde en rutas de produccion.

Contratos inteligentes y extensiones IVM
- Contexto de ejecucion: Agregar `dsid` al contexto de ejecucion de IVM. Los contratos Kotodama siempre se ejecutan dentro de un Data Space especifico.
- Primitivas atomicas cross-DS:
  - `amx_begin()` / `amx_commit()` delimitan una transaccion atomica multi-DS en el host IVM.
  - `amx_touch(dsid, key)` declara intencion de lectura/escritura para deteccion de conflictos contra raices snapshot del slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (operacion permitida solo si la politica lo permite y el handle es valido)
- Handles de assets y fees:
  - Las operaciones de assets son autorizadas por politicas ISI/roles del DS; las fees se pagan en el token gas del DS. Tokens de capacidad opcionales y politicas mas ricas (multi-aprobador, rate-limits, geofencing) pueden agregarse despues sin cambiar el modelo atomico.
- Determinismo: Todas las syscalls son puras y deterministas dadas las entradas y los conjuntos de lectura/escritura AMX declarados. Sin efectos ocultos de tiempo o entorno.

Pruebas de validez post-quantum (ISIs generalizados)
- FASTPQ-ISI (PQ, sin trusted setup): Un argumento basado en hash y kernelizado que generaliza el diseno de transfer a todas las familias ISI mientras apunta a pruebas sub-segundo para lotes a escala 20k en hardware clase GPU.
  - Perfil operativo:
    - Los nodos de produccion construyen el prover a traves de `fastpq_prover::Prover::canonical`, que ahora siempre inicializa el backend de produccion; el mock determinista ha sido removido. (crates/fastpq_prover/src/proof.rs:126)
    - `zk.fastpq.execution_mode` (config) y `irohad --fastpq-execution-mode` permiten a los operadores fijar ejecucion CPU/GPU de forma determinista mientras el hook observador registra los triples requested/resolved/backend para auditorias de flota. (crates/iroha_config/src/parameters/user.rs:1357, crates/irohad/src/main.rs:270, crates/irohad/src/main.rs:2192, crates/iroha_telemetry/src/metrics.rs:8887)
- Aritmetizacion:
  - AIR de actualizacion KV: Trata WSV como un mapa clave-valor tipado comprometido via Poseidon2-SMT. Cada ISI se expande a un conjunto pequeno de filas read-check-write sobre claves (cuentas, assets, roles, dominios, metadata, supply).
  - Restricciones con puerta de opcode: Una unica tabla AIR con columnas selectoras impone reglas por ISI (conservacion, contadores monotonos, permisos, range checks, actualizaciones de metadata acotadas).
  - Argumentos de lookup: Tablas transparentes, con hash-commit, para permisos/roles, precisiones de asset y parametros de politica evitan constraints bit-heavy.
- Compromisos de estado y actualizaciones:
  - Prueba SMT agregada: Todas las claves tocadas (pre/post) se prueban contra `old_root`/`new_root` usando un frontier comprimido con siblings deduplicados.
  - Invariantes: Invariantes globales (ej., supply total por asset) se aplican via igualdad de multisets entre filas de efectos y contadores rastreados.
- Sistema de pruebas:
  - Compromisos polinomiales estilo FRI (DEEP-FRI) con alta aridad (8/16) y blow-up 8-16; hashes Poseidon2; transcript Fiat-Shamir con SHA-2/3.
  - Recursion opcional: agregacion recursiva local al DS para comprimir micro-batches a una prueba por slot si es necesario.
- Alcance y ejemplos cubiertos:
  - Assets: transfer, mint, burn, registrar/deregistrar definiciones de asset, set precision (acotado), set metadata.
  - Cuentas/Dominios: crear/eliminar, set key/threshold, agregar/eliminar signatories (solo estado; las verificaciones de firma son atestiguadas por validadores DS, no probadas dentro del AIR).
  - Roles/Permisos (ISI): otorgar/revocar roles y permisos; aplicados por tablas de lookup y checks de politica monotona.
  - Contratos/AMX: marcadores AMX begin/commit, mint/revoke de capacidades si se habilita; probados como transiciones de estado y contadores de politica.
- Checks fuera de AIR para preservar latencia:
  - Firmas y criptografia pesada (ej., firmas ML-DSA de usuario) son verificadas por validadores DS y atestiguadas en el DS QC; la prueba de validez cubre solo consistencia de estado y cumplimiento de politica. Esto mantiene las pruebas PQ y rapidas.
- Metas de rendimiento (ilustrativas, CPU 32-core + una GPU moderna):
  - 20k ISIs mixtos con poco toque de claves (<=8 claves/ISI): ~0.4-0.9 s de prueba, ~150-450 KB de prueba, ~5-15 ms de verificacion.
  - ISIs mas pesados (mas claves/constraints ricas): micro-batch (ej., 10x2k) + recursion para mantener por slot <1 s.
- Configuracion de DS Manifest:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (firmas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (por defecto; alternativas deben declararse explicitamente)
- Fallbacks:
  - ISIs complejos/personalizados pueden usar un STARK general (`zk.policy = "stark_fri_general"`) con prueba diferida y finality de 1 s via attestation QC + slashing en pruebas invalidas.
  - Opciones no-PQ (ej., Plonk con KZG) requieren trusted setup y ya no se soportan en el build por defecto.

Introduccion a AIR (para Nexus)
- Traza de ejecucion: Una matriz con ancho (columnas de registros) y largo (pasos). Cada fila es un paso logico del procesamiento ISI; las columnas contienen valores pre/post, selectores y flags.
- Constraints:
  - Constraints de transicion: fuerzan relaciones fila-a-fila (ej., post_balance = pre_balance - amount para una fila de debito cuando `sel_transfer = 1`).
  - Constraints de borde: vinculan I/O publico (old_root/new_root, contadores) a las primeras/ultimas filas.
  - Lookups/permutaciones: aseguran membresia e igualdades de multiconjuntos contra tablas comprometidas (permisos, parametros de asset) sin circuitos de bits pesados.
- Compromiso y verificacion:
  - El prover compromete trazas via codificaciones basadas en hash y construye polinomios de bajo grado que son validos si las constraints se cumplen.
  - El verificador checa bajo grado via FRI (basado en hash, post-quantum) con pocas aperturas Merkle; el costo es logaritmico en pasos.
- Ejemplo (Transfer): registros incluyen pre_balance, amount, post_balance, nonce y selectores. Las constraints imponen no-negatividad/rango, conservacion y monotonicidad de nonce, mientras una multi-prueba SMT agregada enlaza hojas pre/post a raices old/new.

Estabilidad de ABI (ABI v1)
- La superficie ABI v1 es fija; no se agregan nuevos syscalls ni tipos pointer-ABI en este release.
- Las runtime upgrades deben mantener `abi_version = 1` con `added_syscalls`/`added_pointer_types` vacios.
- Los goldens de ABI (lista de syscalls, hash ABI, IDs de pointer type) permanecen fijos y no deben cambiar.

Modelo de privacidad
- Contencion de datos privados: cuerpos de transaccion, diffs de estado y snapshots WSV para DS privados nunca salen del subconjunto de validadores privados.
- Exposicion publica: Solo headers, compromisos DA y pruebas PQ de validez se exportan.
- Pruebas ZK opcionales: DS privados pueden producir pruebas ZK (ej., balance suficiente, politica satisfecha) habilitando acciones cross-DS sin revelar estado interno.
- Control de acceso: La autorizacion se aplica por politicas ISI/roles dentro del DS. Tokens de capacidad son opcionales y pueden introducirse mas tarde si es necesario.

Aislamiento de rendimiento y QoS
- Consenso, mempools y almacenamiento separados por DS.
- Cuotas de planificacion Nexus por DS para acotar el tiempo de inclusion de anchors y evitar head-of-line blocking.
- Presupuestos de recursos de contrato por DS (compute/memory/IO), aplicados por el host IVM. La contencion de DS publicos no puede consumir presupuestos de DS privados.
- Llamadas cross-DS asincronas evitan esperas sincronas largas dentro de ejecucion DS privada.

Disponibilidad de datos y diseno de almacenamiento
1) Erasure Coding
- Usar Reed-Solomon sistematico (ej., GF(2^16)) para erasure coding a nivel blob de bloques Kura y snapshots WSV: parametros `(k, m)` con `n = k + m` shards.
- Parametros por defecto (propuestos, DS publicos): `k=32, m=16` (n=48), habilitando recuperacion de hasta 16 shards perdidos con ~1.5x expansion. Para DS privados: `k=16, m=8` (n=24) dentro del conjunto con permisos. Ambos son configurables por DS Manifest.
- Blobs publicos: Shards distribuidos entre muchos nodos DA/validadores con cheques de disponibilidad basados en muestreo. Los compromisos DA en headers permiten verificacion de light clients.
- Blobs privados: Shards cifrados y distribuidos solo entre validadores DS privados (o custodios designados). La cadena global solo lleva compromisos DA (sin ubicaciones de shards o llaves).

2) Compromisos y muestreo
- Para cada blob: calcular una raiz Merkle sobre shards e incluirla en `*_da_commitment`. Mantener PQ evitando compromisos de curva eliptica.
- Attesters DA: attesters regionales muestreados por VRF (ej., 64 por region) emiten un certificado ML-DSA-87 que atestigua muestreo exitoso de shards. Objetivo de latencia de attestation DA <=300 ms. El comite nexus valida certificados en lugar de extraer shards.

3) Integracion Kura
- Los bloques almacenan cuerpos de transaccion como blobs con erasure coding y compromisos Merkle.
- Los headers llevan compromisos de blobs; los cuerpos se recuperan via red DA para DS publicos y via canales privados para DS privados.

4) Integracion WSV
- Snapshotting WSV: Periodicamente se hacen checkpoints de estado DS en snapshots chunked con erasure coding y compromisos registrados en headers. Entre snapshots, se mantienen logs de cambios. Los snapshots publicos se fragmentan ampliamente; los snapshots privados permanecen dentro de validadores privados.
- Acceso con pruebas: Los contratos pueden proporcionar (o solicitar) pruebas de estado (Merkle/Verkle) ancladas por compromisos de snapshot. DS privados pueden suministrar attestations de conocimiento-cero en lugar de pruebas crudas.

5) Retencion y pruning
- Sin pruning para DS publicos: retener todos los cuerpos Kura y snapshots WSV via DA (escalado horizontal). DS privados pueden definir retencion interna, pero los compromisos exportados permanecen inmutables. La capa nexus retiene todos los bloques Nexus y compromisos de artefactos DS.

Redes y roles de nodos
- Validadores globales: Participan en consenso nexus, validan bloques Nexus y artefactos DS, ejecutan cheques DA para DS publicos.
- Validadores de Data Space: Ejecutan consenso DS, ejecutan contratos, manejan Kura/WSV local, manejan DA para su DS.
- Nodos DA (opcionales): Almacenan/publican blobs publicos, facilitan muestreo. Para DS privados, los nodos DA se co-ubican con validadores o custodios confiables.

Mejoras y consideraciones a nivel sistema
- Desacoplar secuenciamiento/mempool: Adoptar un mempool DAG (ej., estilo Narwhal) alimentando un BFT canalizado en la capa nexus para reducir latencia y mejorar throughput sin cambiar el modelo logico.
- Cuotas DS y fairness: Cuotas por DS por bloque y limites de peso para evitar head-of-line blocking y asegurar latencia predecible para DS privados.
- Attestation DS (PQ): Los quorum certificates DS por defecto usan ML-DSA-87 (clase Dilithium5). Esto es post-quantum y mas grande que firmas EC pero aceptable a un QC por slot. DS pueden optar explicitamente por ML-DSA-65/44 (mas pequeno) o firmas EC si se declara en el DS Manifest; se recomienda que DS publicos mantengan ML-DSA-87.
- Attesters DA: Para DS publicos, usar attesters regionales muestreados por VRF que emiten certificados DA. El comite nexus valida certificados en lugar de muestreo crudo de shards; DS privados mantienen attestations DA internas.
- Recursion y pruebas por epoch: Agregar micro-batches dentro de un DS en una prueba recursiva por slot/epoch para mantener tamano de prueba y tiempo de verificacion estable bajo alta carga.
- Escalado por lanes (si es necesario): Si un comite global unico se vuelve cuello de botella, introducir K lanes de secuenciamiento paralelos con merge determinista. Esto preserva un orden global unico mientras escala horizontalmente.
- Aceleracion determinista: Proveer kernels SIMD/CUDA con feature-gate para hashing/FFT con fallback CPU bit-exacto para preservar determinismo entre hardware.
- Umbrales de activacion de lanes (propuesta): Habilitar 2-4 lanes si (a) p95 finality excede 1.2 s por >3 minutos consecutivos, o (b) ocupacion por bloque excede 85% por >5 minutos, o (c) la tasa de tx entrante requeriria >1.2x capacidad de bloque a niveles sostenidos. Las lanes asignan buckets de transacciones deterministamente por hash de DSID y hacen merge en el bloque nexus.

Fees y economia (valores iniciales)
- Unidad de gas: token de gas por DS con compute/IO medido; las fees se pagan en el asset gas nativo del DS. La conversion entre DS es una preocupacion de aplicacion.
- Prioridad de inclusion: round-robin entre DS con cuotas por DS para preservar fairness y SLOs de 1 s; dentro de un DS, la puja de fees puede resolver empates.
- Futuro: un mercado global de fees o politicas que minimicen MEV pueden explorarse sin cambiar atomicidad o el diseno de pruebas PQ.

Flujo cross-Data-Space (Ejemplo)
1) Un usuario envia una transaccion AMX que toca DS publico P y DS privado S: mover el asset X desde S a beneficiario B cuya cuenta esta en P.
2) Dentro del slot, P y S ejecutan su fragmento contra el snapshot del slot. S verifica autorizacion y disponibilidad, actualiza su estado interno y produce una prueba PQ de validez y compromiso DA (sin fuga de datos privados). P prepara la actualizacion de estado correspondiente (ej., mint/burn/locking en P segun la politica) y su prueba.
3) El comite nexus verifica ambas pruebas DS y certificados DA; si ambas verifican dentro del slot, la transaccion se compromete atomicamente en el bloque Nexus de 1 s, actualizando ambas raices DS en el vector de world state global.
4) Si alguna prueba o certificado DA falta o es invalido, la transaccion aborta (sin efectos), y el cliente puede reenviar para el siguiente slot. Ningun dato privado sale de S en ningun paso.

- Consideraciones de seguridad
- Ejecucion determinista: Los syscalls de IVM siguen siendo deterministas; los resultados cross-DS se impulsan por el commit AMX y la finality, no por tiempo de reloj o timing de red.
- Control de acceso: Permisos ISI en DS privados restringen quien puede enviar transacciones y que operaciones se permiten. Tokens de capacidad codifican derechos finos para uso cross-DS.
- Confidencialidad: Cifrado de extremo a extremo para datos DS privados, shards con erasure coding almacenados solo entre miembros autorizados, pruebas ZK opcionales para attestations externas.
- Resistencia a DoS: El aislamiento en capas de mempool/consenso/almacenamiento evita que la congestion publica impacte el progreso de DS privados.

Cambios en componentes de Iroha
- iroha_data_model: Introducir `DataSpaceId`, identificadores calificados por DS, descriptores AMX (conjuntos de lectura/escritura), tipos de prueba/compromiso DA. Serializacion solo Norito.
- ivm: Mantener fija la superficie ABI v1 (sin nuevos syscalls ni tipos pointer-ABI); AMX/runtime upgrades usan primitivas v1 existentes; mantener goldens ABI.
- iroha_core: Implementar planificador nexus, Space Directory, ruteo/validacion AMX, verificacion de artefactos DS y enforcement de politicas para muestreo DA y cuotas.
- Space Directory y cargadores de manifiestos: Pasar metadata de endpoints FMS (y otros descriptores de servicios de bien comun) a traves del parseo de manifiestos DS para que los nodos auto-descubran endpoints locales al unirse a un Data Space.
- kura: Almacen de blobs con erasure coding, compromisos, APIs de recuperacion respetando politicas privadas/publicas.
- WSV: Snapshotting, chunking, compromisos; APIs de prueba; integracion con deteccion y verificacion de conflictos AMX.
- irohad: Roles de nodo, redes para DA, membresia/autenticacion DS privada, configuracion via `iroha_config` (sin toggles de entorno en rutas de produccion).

Configuracion y determinismo
- Todo el comportamiento de runtime se configura via `iroha_config` y se conecta via constructores/hosts. Sin toggles de entorno en produccion.
- Aceleracion de hardware (SIMD/NEON/METAL/CUDA) es opcional y esta protegida por feature-gates; fallbacks deterministas deben producir resultados identicos entre hardware.
- - Post-Quantum por defecto: Todos los DS deben usar pruebas de validez PQ (STARK/FRI) y ML-DSA-87 para QCs de DS por defecto. Alternativas requieren declaracion explicita en el DS Manifest y aprobacion de politica.

### Control de ciclo de vida de lanes en runtime

- **Endpoint admin:** `POST /v1/nexus/lifecycle` (Torii) acepta un cuerpo Norito/JSON con `additions` (objetos completos `LaneConfig`) y `retire` (ids de lane) para agregar o retirar lanes sin reinicio. Las solicitudes se protegen con `nexus.enabled=true` y reutilizan la misma vista de configuracion/estado Nexus que la cola.
- **Comportamiento:** En exito, el nodo aplica el plan de ciclo de vida a metadata WSV/Kura, reconstruye routing/limites/manifiestos de cola y responde con `{ ok: true, lane_count: <u32> }`. Los planes que fallan validacion (ids de retiro desconocidos, alias/ids duplicados, Nexus deshabilitado) devuelven `400 Bad Request` con un `lane_lifecycle_error`.
- **Seguridad:** El handler usa el lock de vista de estado compartido para evitar carreras con lectores mientras actualiza catalogos; los llamadores aun deben serializar actualizaciones de ciclo de vida externamente para evitar planes en conflicto.
- **Propagacion:** El routing/limites de la cola y los manifiestos de lane se reconstruyen con el catalogo actualizado, y los workers de consenso/DA/RBC leen el lane config refrescado via snapshots de estado, de modo que el scheduling y la seleccion de validadores cambian sin reinicio (el trabajo en vuelo termina con la configuracion previa).
- **Limpieza de almacenamiento:** Kura y la geometria de WSV por capas se reconcilian (crear/retirar/re-etiquetar), se sincronizan/persisten los mapeos de cursores de shards DA, y los lanes retirados se purgan de caches de lane relay y de los stores de commitments DA/confidential-compute/pin-intent.

Ruta de migracion (Iroha 2 -> Iroha 3)
1) Introducir IDs calificados por dataspace y composicion de bloque nexus/estado global en el data model; agregar feature flags para mantener modos heredados de Iroha 2 durante la transicion.
2) Implementar backends de erasure coding Kura/WSV tras feature flags, preservando backends actuales como default durante fases tempranas.
3) Mantener fija la superficie ABI v1; implementar AMX sin nuevos syscalls/tipos pointer y actualizar pruebas/docs sin cambiar ABI.
4) Entregar una cadena nexus minima con un solo DS publico y bloques de 1 s; luego agregar el primer piloto de DS privado exportando solo pruebas/compromisos.
5) Expandir a transacciones atomicas cross-DS completas (AMX) con pruebas FASTPQ-ISI locales al DS y attesters DA; habilitar QCs ML-DSA-87 en todos los DS.

Estrategia de pruebas
- Pruebas unitarias para tipos de data model, roundtrips Norito, comportamientos de syscalls AMX y codificacion/decodificacion de pruebas.
- Pruebas IVM para fijar goldens de ABI v1 (lista de syscalls, hash ABI, IDs de pointer type).
- Pruebas de integracion para transacciones atomicas cross-DS (positivas/negativas), objetivos de latencia de attesters DA (<=300 ms) y aislamiento de rendimiento bajo carga.
- Pruebas de seguridad para verificacion de DS QC (ML-DSA-87), deteccion de conflictos/semantica de abortos y prevencion de filtracion de shards confidenciales.

### Activos de telemetria y runbook NX-18

- **Panel Grafana:** `dashboards/grafana/nexus_lanes.json` ahora exporta el dashboard "Nexus Lane Finality & Oracles" solicitado por NX-18. Los paneles cubren `histogram_quantile()` sobre `iroha_slot_duration_ms`, `iroha_da_quorum_ratio`, advertencias de disponibilidad DA (`sumeragi_da_gate_block_total{reason="missing_local_data"}`), gauges de precio/estancamiento/TWAP/haircut de oraculos, y el panel vivo de buffer `iroha_settlement_buffer_xor` para que los operadores prueben el slot de 1 s, DA y SLOs de tesoreria sin queries bespoke.
- **CI gate:** `scripts/telemetry/check_slot_duration.py` parsea snapshots Prometheus, imprime la latencia p50/p95/p99 y aplica los umbrales NX-18 (p95 <= 1000 ms, p99 <= 1100 ms). El harness companero `scripts/telemetry/nx18_acceptance.py` verifica quorum DA, staleness/TWAP/haircuts de oraculos, buffers de settlement y cuantiles de slot en una pasada (`--json-out` persiste evidencia), y ambos corren dentro de `ci/check_nexus_lane_smoke.sh` para RCs.
- **Empaquetador de evidencia:** `scripts/telemetry/bundle_slot_artifacts.py` copia el snapshot de metricas + resumen JSON en `artifacts/nx18/` y emite `slot_bundle_manifest.json` con digests SHA-256, asegurando que cada RC sube exactamente los artefactos que dispararon el gate NX-18.
- **Automatizacion de release:** `scripts/run_release_pipeline.py` ahora invoca `ci/check_nexus_lane_smoke.sh` (omitir con `--skip-nexus-lane-smoke`) y copia `artifacts/nx18/` en la salida de release para que la evidencia NX-18 viaje junto con artefactos de bundle/imagen sin paso manual.
- **Runbook:** `docs/source/runbooks/nexus_lane_finality.md` documenta el flujo on-call (umbrales, pasos de incidente, captura de evidencia, drills de caos) que acompana el dashboard, cumpliendo el bullet de "publicar dashboards/runbooks de operador" de NX-18.
- **Helpers de telemetria:** reutilizar `scripts/telemetry/compare_dashboards.py` existente para diff de dashboards exportados (evitando drift entre staging/prod) y `scripts/telemetry/check_nexus_audit_outcome.py` durante ensayos de routed-trace o caos para que cada drill NX-18 archive el payload `nexus.audit.outcome` correspondiente.

Preguntas abiertas (requieren aclaracion)
1) Firmas de transacciones: Decision - los usuarios finales son libres de elegir cualquier algoritmo de firma que su DS objetivo anuncie (Ed25519, secp256k1, ML-DSA, etc.). Los hosts deben aplicar flags de capacidad multisig/curva en manifiestos, proveer fallbacks deterministas y documentar implicaciones de latencia al mezclar algoritmos. Pendiente: finalizar flujo de negociacion de capacidades en Torii/SDKs y actualizar pruebas de admision.
2) Economia de gas: Cada DS puede denominar gas en un token local, mientras las fees de settlement global se pagan en SORA XOR. Pendiente: definir la ruta estandar de conversion (DEX de lane publica vs otras fuentes de liquidez), hooks de contabilidad del ledger y salvaguardas para DS que subsidian o fijan precio cero.
3) Attesters DA: Numero objetivo por region y umbral (ej., 64 muestreados, 43-de-64 firmas ML-DSA-87) para cumplir <=300 ms manteniendo durabilidad. Alguna region que debamos incluir desde el dia uno?
4) Parametros DA por defecto: Propusimos DS publico `k=32, m=16` y DS privado `k=16, m=8`. Quieren un perfil de redundancia mas alto (ej., `k=30, m=20`) para ciertas clases DS?
5) Granularidad DS: Dominios y assets pueden ser DS. Debemos soportar DS jerarquico (domain DS como padre de asset DS) con herencia opcional de politicas, o mantenerlo plano para v1?
6) ISIs pesados: Para ISIs complejos que no puedan producir pruebas sub-segundo, debemos (a) rechazarlos, (b) dividirlos en pasos atomicos mas pequenos entre bloques, o (c) permitir inclusion diferida con flags explicitos?
7) Conflictos cross-DS: El conjunto de lectura/escritura declarado por el cliente es suficiente, o el host debe inferir y expandirlo automaticamente por seguridad (a costo de mas conflictos)?

Anexo: Cumplimiento con politicas del repositorio
- Norito se usa para todos los formatos on-wire y serializacion JSON via helpers Norito.
- ABI v1 solo; sin toggles de runtime para politicas ABI. Las superficies de syscalls y pointer-type estan fijas y fijadas por pruebas golden.
- Determinismo preservado entre hardware; aceleracion es opcional y con gating.
- Sin serde en rutas de produccion; sin configuracion basada en entorno en produccion.
