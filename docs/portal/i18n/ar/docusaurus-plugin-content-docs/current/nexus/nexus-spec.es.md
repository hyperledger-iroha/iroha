---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/nexus-spec.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-spec
title: Especificacion tecnica de Sora Nexus
description: Espejo completo de `docs/source/nexus.md`, que cubre la arquitectura y las restricciones de diseno para el ledger Iroha 3 (Sora Nexus).
---

:::note Fuente canonica
Esta pagina refleja `docs/source/nexus.md`. Manten ambas copias alineadas hasta que el backlog de traduccion llegue al portal.
:::

#! Iroha 3 - Sora Nexus Ledger: Especificacion tecnica de diseno

Este documento propone la arquitectura del Sora Nexus Ledger para Iroha 3, evolucionando Iroha 2 hacia un ledger global unico y logicamente unificado organizado alrededor de Data Spaces (DS). Data Spaces proveen dominios fuertes de privacidad ("private data spaces") y participacion abierta ("public data spaces"). El diseno preserva la composabilidad a traves del ledger global mientras asegura aislamiento estricto y confidencialidad para los datos de private-DS, e introduce escalado de disponibilidad de datos via codificacion de borrado en Kura (block storage) y WSV (World State View).

El mismo repositorio compila tanto Iroha 2 (redes autoalojadas) como Iroha 3 (SORA Nexus). La ejecucion esta impulsada por la Iroha Virtual Machine (IVM) compartida y la toolchain de Kotodama, por lo que los contratos y artefactos de bytecode permanecen portables entre despliegues autoalojados y el ledger global de Nexus.

Objetivos
- Un ledger logico global compuesto por muchos validadores cooperantes y Data Spaces.
- Data Spaces privados para operacion con permisos (p. ej., CBDC), con datos que nunca salen del DS privado.
- Data Spaces publicos con participacion abierta, acceso sin permisos estilo Ethereum.
- Contratos inteligentes composables entre Data Spaces, sujetos a permisos explicitos para acceso a activos de private-DS.
- Aislamiento de rendimiento para que la actividad publica no degrade las transacciones internas de private-DS.
- Disponibilidad de datos a escala: Kura y WSV con codificacion de borrado para soportar datos efectivamente ilimitados manteniendo los datos de private-DS privados.

No objetivos (fase inicial)
- Definir economia de token o incentivos de validadores; las politicas de scheduling y staking son enchufables.
- Introducir una nueva version de ABI; los cambios apuntan a ABI v1 con extensiones explicitas de syscalls y pointer-ABI segun la politica de IVM.

Terminologia
- Nexus Ledger: El ledger logico global formado al componer bloques de Data Space (DS) en una historia ordenada y un compromiso de estado.
- Data Space (DS): Dominio acotado de ejecucion y almacenamiento con sus propios validadores, gobernanza, clase de privacidad, politica de DA, cuotas y politica de tarifas. Existen dos clases: public DS y private DS.
- Private Data Space: Validadores con permisos y control de acceso; los datos de transaccion y estado nunca salen del DS. Solo se anclan compromisos/metadatos globalmente.
- Public Data Space: Participacion sin permisos; los datos completos y el estado son publicos.
- Data Space Manifest (DS Manifest): Manifest codificado con Norito que declara parametros DS (validadores/llaves QC, clase de privacidad, politica ISI, parametros DA, retencion, cuotas, politica ZK, tarifas). El hash del manifest se ancla en la cadena nexus. Salvo que se anule, los certificados de quorum DS usan ML-DSA-87 (clase Dilithium5) como esquema de firma post-cuantico por defecto.
- Space Directory: Contrato de directorio global on-chain que rastrea manifests DS, versiones y eventos de gobernanza/rotacion para resolvibilidad y auditorias.
- DSID: Identificador globalmente unico para un Data Space. Se usa para namespacing de todos los objetos y referencias.
- Anchor: Compromiso criptografico de un bloque/header DS incluido en la cadena nexus para vincular la historia DS al ledger global.
- Kura: Almacenamiento de bloques de Iroha. Se extiende aqui con almacenamiento de blobs codificados con borrado y compromisos.
- WSV: Iroha World State View. Se extiende aqui con segmentos de estado versionados, con snapshots, y codificados con borrado.
- IVM: Iroha Virtual Machine para ejecucion de contratos inteligentes (bytecode Kotodama `.to`).
  - AIR: Algebraic Intermediate Representation. Vista algebraica del computo para pruebas estilo STARK, describiendo la ejecucion como trazas basadas en campos con restricciones de transicion y frontera.

Modelo de Data Spaces
- Identidad: `DataSpaceId (DSID)` identifica un DS y da namespacing a todo. Los DS pueden instanciarse en dos granularidades:
  - Domain-DS: `ds::domain::<domain_name>` - ejecucion y estado acotados a un dominio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - ejecucion y estado acotados a una definicion de activo unica.
  Ambas formas coexisten; las transacciones pueden tocar multiples DSID de forma atomica.
- Ciclo de vida del manifest: la creacion de DS, actualizaciones (rotacion de llaves, cambios de politica) y retiro se registran en Space Directory. Cada artefacto DS por slot referencia el hash del manifest mas reciente.
- Clases: Public DS (participacion abierta, DA publica) y Private DS (permisionado, DA confidencial). Politicas hibridas son posibles via flags del manifest.
- Politicas por DS: permisos ISI, parametros DA `(k,m)`, cifrado, retencion, cuotas (min/max participacion de tx por bloque), politica de pruebas ZK/optimistas, tarifas.
- Gobernanza: membresia DS y rotacion de validadores definidas por la seccion de gobernanza del manifest (propuestas on-chain, multisig o gobernanza externa anclada por transacciones nexus y atestaciones).

Manifests de capacidades y UAID
- Cuentas universales: cada participante recibe un UAID deterministico (`UniversalAccountId` en `crates/iroha_data_model/src/nexus/manifest.rs`) que abarca todos los dataspaces. Los manifests de capacidades (`AssetPermissionManifest`) vinculan un UAID a un dataspace especifico, epocas de activacion/expiracion y una lista ordenada de reglas allow/deny `ManifestEntry` que acotan `dataspace`, `program_id`, `method`, `asset` y roles AMX opcionales. Las reglas deny siempre ganan; el evaluador emite `ManifestVerdict::Denied` con una razon de auditoria o un grant `Allowed` con la metadata de allowance coincidente.
- Allowances: cada entrada allow lleva buckets deterministas `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) mas un `max_amount` opcional. Hosts y SDK consumen el mismo payload Norito, asi que la aplicacion permanece identica entre hardware y SDK.
- Telemetria de auditoria: Space Directory emite `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) cuando un manifest cambia de estado. La nueva superficie `SpaceDirectoryEventFilter` permite a suscriptores de Torii/data-event monitorear actualizaciones de manifest UAID, revocaciones y decisiones deny-wins sin plumbing personalizado.

Para evidencia operativa end-to-end, notas de migracion de SDK y checklists de publicacion de manifests, espejea esta seccion con la Universal Account Guide (`docs/source/universal_accounts_guide.md`). Manten ambos documentos alineados cuando cambien la politica o las herramientas UAID.

Arquitectura de alto nivel
1) Capa de composicion global (Nexus Chain)
- Mantiene un orden canonico unico de bloques Nexus de 1 segundo que finalizan transacciones atomicas que abarcan uno o mas Data Spaces (DS). Cada transaccion confirmada actualiza el world state global unificado (vector de roots por DS).
- Contiene metadatos minimos mas pruebas/QC agregados para garantizar composabilidad, finalizacion y deteccion de fraude (DSIDs tocados, roots de estado por DS antes/despues, compromisos DA, pruebas de validez por DS, y el certificado de quorum DS usando ML-DSA-87). No se incluye data privada.
- Consenso: comite BFT global con pipeline de tamano 22 (3f+1 con f=7), seleccionado de un pool de hasta ~200k validadores potenciales via un mecanismo VRF/stake por epocas. El comite nexus ordena transacciones y finaliza el bloque en 1s.

2) Capa de Data Space (Public/Private)
- Ejecuta fragmentos por DS de transacciones globales, actualiza el WSV local del DS y produce artefactos de validez por bloque (pruebas agregadas por DS y compromisos DA) que se agregan en el bloque Nexus de 1 segundo.
- Private DS cifran datos en reposo y en transito entre validadores autorizados; solo compromisos y pruebas de validez PQ salen del DS.
- Public DS exportan cuerpos completos de datos (via DA) y pruebas de validez PQ.

3) Transacciones atomicas cross-Data-Space (AMX)
- Modelo: cada transaccion de usuario puede tocar multiples DS (p. ej., domain DS y uno o mas asset DS). Se confirma de forma atomica en un solo bloque Nexus o se aborta; no hay efectos parciales.
- Prepare-Commit dentro de 1s: para cada transaccion candidata, los DS tocados ejecutan en paralelo contra el mismo snapshot (roots DS al inicio del slot) y producen pruebas de validez PQ por DS (FASTPQ-ISI) y compromisos DA. El comite nexus confirma la transaccion solo si todas las pruebas DS requeridas verifican y los certificados DA llegan a tiempo (objetivo <=300 ms); de lo contrario, la transaccion se reprograma para el siguiente slot.
- Consistencia: los conjuntos de lectura/escritura se declaran; la deteccion de conflictos ocurre al commit contra los roots de inicio de slot. La ejecucion optimista sin locks por DS evita bloqueos globales; la atomicidad se impone por la regla de commit nexus (todo o nada entre DS).
- Privacidad: private DS exportan solo pruebas/compromisos ligados a roots DS pre/post. No sale data privada cruda del DS.

4) Disponibilidad de datos (DA) con codificacion de borrado
- Kura almacena cuerpos de bloques y snapshots WSV como blobs codificados con borrado. Los blobs publicos se fragmentan ampliamente; los blobs privados se almacenan solo dentro de validadores private-DS, con chunks cifrados.
- Los compromisos DA se registran tanto en artefactos DS como en bloques Nexus, habilitando muestreo y garantias de recuperacion sin revelar contenido privado.

Estructura de bloques y commit
- Artefacto de prueba de Data Space (por slot de 1s, por DS)
  - Campos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Private-DS exporta artefactos sin cuerpos de datos; public DS permite recuperacion de cuerpos via DA.

- Bloque Nexus (cadencia de 1s)
  - Campos: block_number, parent_hash, slot_time, tx_list (transacciones atomicas cross-DS con DSIDs tocados), ds_artifacts[], nexus_qc.
  - Funcion: finaliza todas las transacciones atomicas cuyos artefactos DS requeridos verifican; actualiza el vector de roots DS del world state global en un solo paso.

Consenso y scheduling
- Consenso de Nexus Chain: BFT global con pipeline (clase Sumeragi) con comite de 22 nodos (3f+1 con f=7) apuntando a bloques de 1s y finalizacion de 1s. Los miembros del comite se seleccionan por epocas via VRF/stake entre ~200k candidatos; la rotacion mantiene descentralizacion y resistencia a censura.
- Consenso de Data Space: cada DS ejecuta su propio BFT entre sus validadores para producir artefactos por slot (pruebas, compromisos DA, DS QC). Los comites lane-relay se dimensionan en `3f+1` usando la configuracion `fault_tolerance` del dataspace y se muestrean de forma determinista por epoca desde el pool de validadores del dataspace usando el seed de epoca VRF ligado a `(dataspace_id, lane_id)`. Private DS son permisionados; public DS permiten liveness abierta sujeta a politicas anti-Sybil. El comite global nexus no cambia.
- Scheduling de transacciones: los usuarios envian transacciones atomicas declarando DSIDs tocados y conjuntos de lectura/escritura. Los DS ejecutan en paralelo dentro del slot; el comite nexus incluye la transaccion en el bloque de 1s si todos los artefactos DS verifican y los certificados DA son puntuales (<=300 ms).
- Aislamiento de rendimiento: cada DS tiene mempools y ejecucion independientes. Las cuotas por DS limitan cuantas transacciones que tocan un DS se pueden confirmar por bloque para evitar head-of-line blocking y proteger la latencia de private DS.

Modelo de datos y namespacing
- IDs calificados por DS: todas las entidades (dominios, cuentas, activos, roles) se califican por `dsid`. Ejemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias globales: una referencia global es una tupla `(dsid, object_id, version_hint)` y puede colocarse on-chain en la capa nexus o en descriptores AMX para uso cross-DS.
- Serializacion Norito: todos los mensajes cross-DS (descriptores AMX, pruebas) usan codecs Norito. No se usa serde en paths de produccion.

Contratos inteligentes y extensiones de IVM
- Contexto de ejecucion: agregar `dsid` al contexto de ejecucion de IVM. Los contratos Kotodama siempre se ejecutan dentro de un Data Space especifico.
- Primitivas atomicas cross-DS:
  - `amx_begin()` / `amx_commit()` delimitan una transaccion atomica multi-DS en el host IVM.
  - `amx_touch(dsid, key)` declara intencion de lectura/escritura para deteccion de conflictos contra roots snapshot del slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (operacion permitida solo si la politica lo permite y el handle es valido)
- Handles de activos y tarifas:
  - Las operaciones de activos se autorizan por las politicas ISI/rol del DS; las tarifas se pagan en el token de gas del DS. Los tokens de capacidad opcionales y politicas mas ricas (multi-approver, rate-limits, geofencing) se pueden agregar mas adelante sin cambiar el modelo atomico.
- Determinismo: todas las nuevas syscalls son puras y deterministas dadas las entradas y los conjuntos de lectura/escritura AMX declarados. Sin efectos ocultos de tiempo o entorno.

Pruebas de validez post-cuanticas (ISI generalizados)
- FASTPQ-ISI (PQ, sin trusted setup): un argumento hash-based que generaliza el diseno de transfer a todas las familias ISI mientras apunta a prueba sub-segundo para lotes de escala 20k en hardware clase GPU.
  - Perfil operativo:
    - Los nodos de produccion construyen el prover via `fastpq_prover::Prover::canonical`, que ahora siempre inicializa el backend de produccion; el mock determinista fue removido. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) y `irohad --fastpq-execution-mode` permiten a los operadores fijar ejecucion CPU/GPU de forma determinista mientras el observer hook registra triples solicitados/resueltos/backend para auditorias de flota. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetizacion:
  - KV-Update AIR: trata WSV como un mapa key-value tipado comprometido via Poseidon2-SMT. Cada ISI se expande a un conjunto pequeno de filas read-check-write sobre claves (cuentas, activos, roles, dominios, metadata, supply).
  - Restricciones con puertas de opcode: una sola tabla AIR con columnas selectoras impone reglas por ISI (conservacion, contadores monotonic, permisos, range checks, actualizaciones de metadata acotadas).
  - Argumentos de lookup: tablas transparentes comprometidas por hash para permisos/roles, precisiones de activos y parametros de politica evitan constraints bitwise pesadas.
- Compromisos y actualizaciones de estado:
  - Prueba SMT agregada: todas las claves tocadas (pre/post) se prueban contra `old_root`/`new_root` usando un frontier comprimido con siblings deduplicados.
  - Invariantes: invariantes globales (p. ej., supply total por activo) se imponen via igualdad de multiconjuntos entre filas de efecto y contadores rastreados.
- Sistema de prueba:
  - Compromisos polinomiales estilo FRI (DEEP-FRI) con alta aridad (8/16) y blow-up 8-16; hashes Poseidon2; transcript Fiat-Shamir con SHA-2/3.
  - Recursion opcional: agregacion recursiva local a DS para comprimir micro-lotes a una prueba por slot si se necesita.
- Alcance y ejemplos cubiertos:
  - Activos: transfer, mint, burn, register/unregister asset definitions, set precision (acotado), set metadata.
  - Cuentas/Dominios: create/remove, set key/threshold, add/remove signatories (solo estado; las verificaciones de firma se atestan por validadores DS, no se prueban dentro del AIR).
  - Roles/Permisos (ISI): grant/revoke roles y permisos; impuestos por tablas de lookup y checks de politica monotonic.
  - Contratos/AMX: marcadores begin/commit AMX, capability mint/revoke si esta habilitado; se prueban como transiciones de estado y contadores de politica.
- Checks fuera del AIR para preservar latencia:
  - Firmas y criptografia pesada (p. ej., firmas ML-DSA de usuarios) se verifican por validadores DS y se atestan en el DS QC; la prueba de validez cubre solo consistencia de estado y cumplimiento de politicas. Esto mantiene pruebas PQ y rapidas.
- Objetivos de rendimiento (ilustrativos, CPU de 32 cores + una GPU moderna):
  - 20k ISI mixtas con key-touch pequeno (<=8 claves/ISI): ~0.4-0.9 s de prueba, ~150-450 KB de prueba, ~5-15 ms de verificacion.
  - ISI mas pesadas (mas claves/constraints ricas): micro-lotes (p. ej., 10x2k) + recursion para mantener por slot <1 s.
- Configuracion de DS Manifest:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (firmas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (por defecto; alternativas deben declararse explicitamente)
- Fallbacks:
  - ISI complejas/personalizadas pueden usar un STARK general (`zk.policy = "stark_fri_general"`) con prueba diferida y finalizacion de 1 s via atestacion QC + slashing en pruebas invalidas.
  - Opciones no PQ (p. ej., Plonk con KZG) requieren trusted setup y ya no se soportan en el build por defecto.

Introduccion a AIR (para Nexus)
- Traza de ejecucion: matriz con ancho (columnas de registros) y longitud (pasos). Cada fila es un paso logico del procesamiento ISI; las columnas contienen valores pre/post, selectores y flags.
- Restricciones:
  - Restricciones de transicion: imponen relaciones fila a fila (p. ej., post_balance = pre_balance - amount para una fila de debito cuando `sel_transfer = 1`).
  - Restricciones de frontera: vinculan E/S publica (old_root/new_root, contadores) a la primera/ultima fila.
  - Lookups/permutations: aseguran membresia e igualdades de multiconjuntos contra tablas comprometidas (permisos, parametros de activos) sin circuitos pesados de bits.
- Compromiso y verificacion:
  - El prover compromete trazas via codificaciones hash y construye polinomios de bajo grado que son validos si las restricciones se cumplen.
  - El verifier comprueba bajo grado via FRI (hash-based, post-cuantico) con pocas aperturas Merkle; el costo es logaritmico en los pasos.
- Ejemplo (Transfer): los registros incluyen pre_balance, amount, post_balance, nonce y selectores. Las restricciones imponen no negatividad/rango, conservacion y monotonicidad de nonce, mientras una multiprueba SMT agregada vincula hojas pre/post a los roots old/new.

Evolucion de ABI y syscalls (ABI v1)
- Syscalls a agregar (nombres ilustrativos):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos Pointer-ABI a agregar:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Actualizaciones requeridas:
  - Agregar a `ivm::syscalls::abi_syscall_list()` (mantener orden), gatear por politica.
  - Mapear numeros desconocidos a `VMError::UnknownSyscall` en hosts.
  - Actualizar tests: syscall list golden, ABI hash, pointer type ID goldens y policy tests.
  - Docs: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modelo de privacidad
- Contencion de datos privados: cuerpos de transaccion, diffs de estado y snapshots WSV de private DS nunca salen del subconjunto privado de validadores.
- Exposicion publica: solo headers, compromisos DA y pruebas de validez PQ se exportan.
- Pruebas ZK opcionales: private DS pueden producir pruebas ZK (p. ej., balance suficiente, politica cumplida) habilitando acciones cross-DS sin revelar estado interno.
- Control de acceso: la autorizacion se impone por politicas ISI/rol dentro del DS. Los tokens de capacidad son opcionales y pueden introducirse mas adelante.

Aislamiento de rendimiento y QoS
- Consenso, mempools y almacenamiento separados por DS.
- Cuotas de scheduling nexus por DS para limitar el tiempo de inclusion de anchors y evitar head-of-line blocking.
- Presupuestos de recursos de contrato por DS (compute/memory/IO), impuestos por el host IVM. La contencion en public DS no puede consumir presupuestos de private DS.
- Llamadas cross-DS asincronas evitan esperas sincronas largas dentro de ejecucion private-DS.

Disponibilidad de datos y diseno de almacenamiento
1) Codificacion de borrado
- Usar Reed-Solomon sistematico (p. ej., GF(2^16)) para codificacion de borrado a nivel de blob de bloques Kura y snapshots WSV: parametros `(k, m)` con `n = k + m` shards.
- Parametros por defecto (propuestos, public DS): `k=32, m=16` (n=48), permitiendo recuperacion de hasta 16 shards perdidos con ~1.5x expansion. Para private DS: `k=16, m=8` (n=24) dentro del conjunto permisionado. Ambos configurables por DS Manifest.
- Blobs publicos: shards distribuidos a traves de muchos nodos DA/validadores con checks de disponibilidad por muestreo. Los compromisos DA en headers permiten verificacion por light clients.
- Blobs privados: shards cifrados y distribuidos solo entre validadores private-DS (o custodios designados). La cadena global solo lleva compromisos DA (sin ubicaciones de shards ni llaves).

2) Compromisos y muestreo
- Para cada blob: calcular un Merkle root sobre shards e incluirlo en `*_da_commitment`. Mantener PQ evitando compromisos de curva eliptica.
- DA Attesters: attesters regionales muestreados por VRF (p. ej., 64 por region) emiten un certificado ML-DSA-87 atestando muestreo exitoso de shards. Objetivo de latencia de atestacion DA <=300 ms. El comite nexus valida certificados en lugar de extraer shards.

3) Integracion con Kura
- Los bloques almacenan cuerpos de transaccion como blobs codificados con borrado con compromisos Merkle.
- Los headers llevan compromisos de blob; los cuerpos se recuperan via la red DA para public DS y via canales privados para private DS.

4) Integracion con WSV
- Snapshots WSV: periodicamente se hace checkpoint del estado DS en snapshots por chunks codificados con borrado con compromisos registrados en headers. Entre snapshots, se mantienen change logs. Los snapshots publicos se fragmentan ampliamente; los snapshots privados permanecen dentro de validadores privados.
- Acceso con pruebas: los contratos pueden proporcionar (o solicitar) pruebas de estado (Merkle/Verkle) ancladas por compromisos de snapshot. Private DS pueden suministrar atestaciones de conocimiento cero en lugar de pruebas crudas.

5) Retencion y pruning
- Sin pruning para public DS: retener todos los cuerpos Kura y snapshots WSV via DA (escalado horizontal). Private DS pueden definir retencion interna, pero los compromisos exportados permanecen inmutables. La capa nexus retiene todos los bloques Nexus y los compromisos de artefactos DS.

Red y roles de nodos
- Validadores globales: participan en el consenso nexus, validan bloques Nexus y artefactos DS, realizan checks DA para public DS.
- Validadores de Data Space: ejecutan consenso DS, ejecutan contratos, gestionan Kura/WSV local, manejan DA para su DS.
- Nodos DA (opcional): almacenan/publican blobs publicos, facilitan muestreo. Para private DS, los nodos DA se co-ubican con validadores o custodios confiables.

Mejoras y consideraciones a nivel de sistema
- Desacoplar secuenciacion/mempool: adoptar un mempool DAG (p. ej., estilo Narwhal) que alimente un BFT con pipeline en la capa nexus para bajar latencia y mejorar throughput sin cambiar el modelo logico.
- Cuotas DS y fairness: cuotas por DS por bloque y caps de peso para evitar head-of-line blocking y asegurar latencia predecible para private DS.
- Atestacion DS (PQ): los certificados de quorum DS usan ML-DSA-87 (clase Dilithium5) por defecto. Es post-cuantico y mas grande que firmas EC pero aceptable con un QC por slot. DS pueden optar explicitamente por ML-DSA-65/44 (mas pequeno) o firmas EC si se declara en el DS Manifest; se recomienda mantener ML-DSA-87 para public DS.
- DA attesters: para public DS, usar attesters regionales muestreados por VRF que emiten certificados DA. El comite nexus valida certificados en lugar de muestreo de shards crudos; private DS mantienen atestaciones DA internas.
- Recursion y pruebas por epoca: opcionalmente agregar varios micro-lotes dentro de un DS en una prueba recursiva por slot/epoca para mantener tamano de prueba y tiempo de verificacion estables bajo alta carga.
- Escalado de lanes (si se necesita): si un comite global unico se vuelve un cuello de botella, introducir K lanes de secuenciacion paralelas con un merge determinista. Esto preserva un orden global unico mientras escala horizontalmente.
- Aceleracion determinista: proveer kernels SIMD/CUDA con feature flags para hashing/FFT con fallback CPU bit-exacto para preservar determinismo cross-hardware.
- Umbrales de activacion de lanes (propuesta): habilitar 2-4 lanes si (a) p95 de finalizacion excede 1.2 s por >3 minutos consecutivos, o (b) la ocupacion por bloque excede 85% por >5 minutos, o (c) la tasa entrante de tx requeriria >1.2x la capacidad de bloque en niveles sostenidos. Las lanes agrupan transacciones de forma determinista por hash de DSID y se mergean en el bloque nexus.

Tarifas y economia (valores iniciales)
- Unidad de gas: token de gas por DS con compute/IO medido; las tarifas se pagan en el activo de gas nativo del DS. La conversion entre DS es responsabilidad de la aplicacion.
- Prioridad de inclusion: round-robin entre DS con cuotas por DS para preservar fairness y SLOs de 1s; dentro de un DS, el fee bidding puede desempatar.
- Futuro: se puede explorar un mercado global de tarifas o politicas que minimicen MEV sin cambiar la atomicidad ni el diseno de pruebas PQ.

Flujo cross-Data-Space (ejemplo)
1) Un usuario envia una transaccion AMX que toca public DS P y private DS S: mover activo X desde S a beneficiario B cuya cuenta esta en P.
2) Dentro del slot, P y S ejecutan su fragmento contra el snapshot del slot. S verifica autorizacion y disponibilidad, actualiza su estado interno y produce una prueba de validez PQ y compromiso DA (sin filtrar datos privados). P prepara la actualizacion de estado correspondiente (p. ej., mint/burn/locking en P segun politica) y su prueba.
3) El comite nexus verifica ambas pruebas DS y certificados DA; si ambas verifican dentro del slot, la transaccion se confirma atomicamente en el bloque Nexus de 1s, actualizando ambos roots DS en el vector de world state global.
4) Si alguna prueba o certificado DA falta o es invalido, la transaccion se aborta (sin efectos) y el cliente puede reenviar para el siguiente slot. Ningun dato privado sale de S en ningun paso.

- Consideraciones de seguridad
- Ejecucion determinista: las syscalls IVM permanecen deterministas; los resultados cross-DS los dictan AMX commit y finalizacion, no el reloj o el timing de red.
- Control de acceso: los permisos ISI en private DS restringen quien puede enviar transacciones y que operaciones se permiten. Los tokens de capacidad codifican derechos de grano fino para uso cross-DS.
- Confidencialidad: cifrado end-to-end para datos private-DS, shards codificados con borrado almacenados solo entre miembros autorizados, pruebas ZK opcionales para atestaciones externas.
- Resistencia a DoS: el aislamiento en mempool/consenso/almacenamiento evita que la congestion publica impacte el progreso de private DS.

Cambios en componentes de Iroha
- iroha_data_model: introducir `DataSpaceId`, IDs calificados por DS, descriptores AMX (conjuntos de lectura/escritura), tipos de prueba/compromisos DA. Serializacion solo Norito.
- ivm: agregar syscalls y tipos pointer-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) y pruebas DA; actualizar tests/docs de ABI segun la politica v1.
