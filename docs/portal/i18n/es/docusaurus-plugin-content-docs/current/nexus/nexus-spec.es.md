---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: especificación-nexus
título: Especificación técnica de Sora Nexus
descripción: Espejo completo de `docs/source/nexus.md`, que cubre la arquitectura y las restricciones de diseño para el libro mayor Iroha 3 (Sora Nexus).
---

:::nota Fuente canónica
Esta página refleja `docs/source/nexus.md`. Mantenga ambas copias alineadas hasta que el backlog de traducción llegue al portal.
:::

#! Iroha 3 - Sora Nexus Ledger: Especificación técnica de diseño

Este documento propone la arquitectura del Sora Nexus Ledger para Iroha 3, evolucionando Iroha 2 hacia un ledger global único y lógicamente unificado organizado alrededor de Data Spaces (DS). Los Espacios de Datos proveen dominios fuertes de privacidad ("espacios de datos privados") y participación abierta ("espacios de datos públicos"). El diseño preserva la composabilidad a través del libro mayor global mientras asegura aislamiento estricto y confidencialidad para los datos de private-DS, e introduce un escalado de disponibilidad de datos vía codificación de borrado en Kura (block Storage) y WSV (World State View).El mismo repositorio compila tanto Iroha 2 (redes autoalojadas) como Iroha 3 (SORA Nexus). La ejecucion esta impulsada por la Iroha Virtual Machine (IVM) compartida y la toolchain de Kotodama, por lo que los contratos y artefactos de bytecode permanecen portátiles entre despliegues autoalojados y el ledger global de Nexus.

Objetivos
- Un libro mayor lógico global compuesto por muchos validadores cooperantes y Data Spaces.
- Data Spaces privados para operación con permisos (p. ej., CBDC), con datos que nunca salen del DS privado.
- Data Spaces públicos con participación abierta, acceso permiso sins estilo Ethereum.
- Contratos inteligentes componibles entre Data Spaces, sujetos a permisos explícitos para acceso a activos de private-DS.
- Aislamiento de rendimiento para que la actividad pública no degrade las transacciones internas de private-DS.
- Disponibilidad de datos a escala: Kura y WSV con codificación de borrado para soportar datos efectivamente ilimitadas manteniendo los datos de private-DS privados.

Sin objetivos (fase inicial)
- Definir economía de token o incentivos de validadores; Las políticas de programación y apuesta son enchufables.
- Introducir una nueva versión de ABI; los cambios apuntan a ABI v1 con extensiones explícitas de syscalls y pointer-ABI según la política de IVM.Terminología
- Nexus Ledger: El libro mayor lógico global formado al componer bloques de Data Space (DS) en una historia ordenada y un compromiso de estado.
- Data Space (DS): Dominio acotado de ejecución y almacenamiento con sus propios validadores, gobernanza, clase de privacidad, política de DA, cuotas y política de tarifas. Existen dos clases: DS público y DS privado.
- Espacio Privado de Datos: Validadores con permisos y control de acceso; los datos de transacción y estado nunca salen del DS. Solo se anclan compromisos/metadatos globalmente.
- Espacio Público de Datos: Participación sin permisos; los datos completos y el estado son publicos.
- Data Space Manifest (DS Manifest): Manifiesto codificado con Norito que declara parámetros DS (validadores/llaves QC, clase de privacidad, política ISI, parámetros DA, retencion, cuotas, política ZK, tarifas). El hash del manifest se ancla en la cadena nexus. Salvo que se anule, los certificados de quórum DS usan ML-DSA-87 (clase Dilithium5) como esquema de firma post-cuantico por defecto.
- Space Directory: Contrato de directorio global on-chain que rastrea manifests DS, versiones y eventos de gobernanza/rotacion para resolvibilidad y auditorías.
- DSID: Identificador global único para un Data Space. Se utiliza para espacios de nombres de todos los objetos y referencias.- Anchor: Compromiso criptográfico de un bloque/header DS incluido en la cadena nexus para vincular la historia DS al ledger global.
- Kura: Almacenamiento de bloques de Iroha. Se extiende aquí con almacenamiento de blobs codificados con borrado y compromisos.
- WSV: Iroha Vista del estado mundial. Se extiende aquí con segmentos de estado versionados, con instantáneas y codificados con borrado.
- IVM: Iroha Máquina virtual para ejecución de contratos inteligentes (bytecode Kotodama `.to`).
  - AIRE: Representación Algebraica Intermedia. Vista algebraica del computo para pruebas estilo STARK, describiendo la ejecución como trazas basadas en campos con restricciones de transición y frontera.Modelo de Espacios de Datos
- Identidad: `DataSpaceId (DSID)` identifica un DS y da namespace a todo. Los DS pueden instanciarse en dos granularidades:
  - Domain-DS: `ds::domain::<domain_name>` - ejecucion y estado acotados a un dominio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - ejecucion y estado acotados a una definicion de activo unica.
  Ambas formas coexisten; las transacciones pueden tocar múltiples DSID de forma atómica.
- Ciclo de vida del manifiesto: la creación de DS, actualizaciones (rotación de llaves, cambios de política) y retiro se registran en Space Directory. Cada artefacto DS por slot referencia el hash del manifest más reciente.
- Clases: Public DS (participación abierta, DA publica) y Private DS (permisionado, DA confidencial). Politicas hibridas son posibles via flags del manifest.
- Politicas por DS: permisos ISI, parametros DA `(k,m)`, cifrado, retencion, cuotas (min/max participacion de tx por bloque), politica de pruebas ZK/optimistas, tarifas.
- Gobernanza: membresia DS y rotación de validadores definidas por la sección de gobernanza del manifiesto (propuestas on-chain, multisig o gobernanza externa anclada por transacciones nexus y atestaciones).Manifiestos de capacidades y UAID
- Cuentas universales: cada participante recibe una UAID determinística (`UniversalAccountId` en `crates/iroha_data_model/src/nexus/manifest.rs`) que abarca todos los espacios de datos. Los manifiestos de capacidades (`AssetPermissionManifest`) vinculan un UAID a un espacio de datos específico, épocas de activación/expiración y una lista ordenada de reglas enable/deny `ManifestEntry` que acotan `dataspace`, `program_id`, `method`, `asset` y roles AMX opcionales. Las reglas niegan siempre ganan; el evaluador emite `ManifestVerdict::Denied` con una razón de auditoría o una subvención `Allowed` con la metadatos de asignación coincidente.
- Bonificaciones: cada entrada permite llevar cubetas deterministas `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) más un `max_amount` opcional. Los hosts y SDK consumen la misma carga útil Norito, por lo que la aplicación permanece idéntica entre el hardware y el SDK.
- Telemetria de auditoria: Space Directory emite `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) cuando un manifiesto cambia de estado. La nueva superficie `SpaceDirectoryEventFilter` permite a suscriptores de Torii/data-event monitorear actualizaciones de manifiesto UAID, revocaciones y decisiones deny-wins sin plomería personalizada.Para evidencia operativa de extremo a extremo, notas de migración de SDK y listas de verificación de publicación de manifiestos, espejea esta sección con la Universal Account Guide (`docs/source/universal_accounts_guide.md`). Mantenga ambos documentos alineados cuando cambien la política o las herramientas UAID.

Arquitectura de alto nivel
1) Capa de composición global (Cadena Nexus)
- Mantiene un orden canónico único de bloques Nexus de 1 segundo que finalizan transacciones atómicas que abarcan uno o más Data Spaces (DS). Cada transacción confirmada actualiza el estado mundial unificado (vector de raíces por DS).
- Contiene metadatos minimos mas pruebas/QC agregados para garantizar composabilidad, finalización y detección de fraude (DSIDs tocados, root de estado por DS antes/despues, compromisos DA, pruebas de validez por DS, y el certificado de quorum DS usando ML-DSA-87). No se incluyen datos privados.
- Consenso: comité BFT global con pipeline de tamano 22 (3f+1 con f=7), seleccionado de un pool de hasta ~200k validadores potenciales vía un mecanismo VRF/stake por epocas. El comité nexus ordena transacciones y finaliza el bloque en 1s.2) Capa de Espacio de Datos (Público/Privado)
- Ejecuta fragmentos por DS de transacciones globales, actualiza el WSV local del DS y produce artefactos de validez por bloque (pruebas agregadas por DS y compromisos DA) que se agregan en el bloque Nexus de 1 segundo.
- DS privado cifran datos en reposo y en tránsito entre validadores autorizados; solo compromisos y pruebas de validez PQ salen del DS.
- Public DS exportan cuerpos completos de datos (vía DA) y pruebas de validez PQ.3) Transacciones atómicas cross-Data-Space (AMX)
- Modelo: cada transacción de usuario puede tocar múltiples DS (p. ej., dominio DS y uno o más activo DS). Se confirma de forma atómica en un solo bloque Nexus o se aborta; no hay efectos parciales.
- Prepare-Commit dentro de 1s: para cada transacción candidata, los DS tocados se ejecutan en paralelo contra el mismo snapshot (roots DS al inicio del slot) y producen pruebas de validez PQ por DS (FASTPQ-ISI) y compromisos DA. El comité nexus confirma la transacción solo si todas las pruebas DS requeridas verifican y los certificados DA llegan a tiempo (objetivo <=300 ms); De lo contrario, la transacción se reprogramará para la siguiente ranura.
- Consistencia: los conjuntos de lectura/escritura se declaran; la detección de conflictos ocurre al commit contra las raíces de inicio de slot. La ejecucion optimista sin locks por DS evita bloqueos globales; la atomicidad se impone por la regla de commit nexus (todo o nada entre DS).
- Privacidad: DS privado exportan solo pruebas/compromisos ligados a Roots DS pre/post. No hay datos de venta privada cruda del DS.4) Disponibilidad de datos (DA) con codificación de borrado
- Kura almacena cuerpos de bloques y instantáneas WSV como blobs codificados con borrado. Los blobs publicos se fragmentan ampliamente; los blobs privados se almacenan solo dentro de validadores private-DS, con chunks cifrados.
- Los compromisos DA se registran tanto en artefactos DS como en bloques Nexus, habilitando muestreos y garantías de recuperación sin revelar contenido privado.

Estructura de bloques y compromiso
- Artefacto de prueba de Data Space (por slot de 1s, por DS)
  - Campos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Private-DS exporta artefactos sin cuerpos de datos; public DS permite la recuperación de cuerpos vía DA.

- Bloque Nexus (cadencia de 1s)
  - Campos: block_number, parent_hash, slot_time, tx_list (transacciones atómicas cross-DS con DSIDs tocados), ds_artifacts[], nexus_qc.
  - Función: finalizar todas las transacciones atómicas cuyos artefactos DS requeridos verifican; actualiza el vector de raíces DS del estado mundial global en un solo paso.Consenso y programación
- Consenso de Nexus Chain: BFT global con pipeline (clase Sumeragi) con comité de 22 nodos (3f+1 con f=7) apuntando a bloques de 1s y finalizacion de 1s. Los miembros del comité se seleccionan por épocas vía VRF/stake entre ~200k candidatos; la rotación mantiene la descentralización y la resistencia a la censura.
- Consenso de Data Space: cada DS ejecuta su propio BFT entre sus validadores para producir artefactos por slot (pruebas, compromisos DA, DS QC). Los comités lane-relay se dimensionan en `3f+1` usando la configuración `fault_tolerance` del dataspace y se muestrean de forma determinista por epoca desde el pool de validadores del dataspace usando el seed de epoca VRF ligado a `(dataspace_id, lane_id)`. Privado DS son permisionados; public DS permiten liveness abierta sujeta a politicas anti-Sybil. El comité global nexus no cambia.
- Scheduling de transacciones: los usuarios envian transacciones atómicas declarando DSIDs tocados y conjuntos de lectura/escritura. Los DS se ejecutan en paralelo dentro de la ranura; el comité nexus incluye la transacción en el bloque de 1s si todos los artefactos DS verifican y los certificados DA son puntuales (<=300 ms).- Aislamiento de rendimiento: cada DS tiene mempools y ejecución independiente. Las cuotas por DS limitan cuantas transacciones que tocan un DS se pueden confirmar por bloque para evitar el bloqueo de cabecera y proteger la latencia de DS privado.

Modelo de datos y espacio de nombres.
- IDs calificados por DS: todas las entidades (dominios, cuentas, activos, roles) se califican por `dsid`. Ejemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias globales: una referencia global es una tupla `(dsid, object_id, version_hint)` y puede colocarse on-chain en la capa nexus o en descriptores AMX para uso cross-DS.
- Serialización Norito: todos los mensajes cross-DS (descriptores AMX, pruebas) usan codecs Norito. No se usa serde en caminos de producción.Contratos inteligentes y extensiones de IVM
- Contexto de ejecución: agregar `dsid` al contexto de ejecución de IVM. Los contratos Kotodama siempre se ejecutan dentro de un Data Space específico.
- Primitivas atómicas cross-DS:
  - `amx_begin()` / `amx_commit()` delimitan una transacción atómica multi-DS en el host IVM.
  - `amx_touch(dsid, key)` declara intención de lectura/escritura para detección de conflictos contra root snapshot del slot.
  - `verify_space_proof(dsid, proof, statement)` -> booleano
  - `use_asset_handle(handle, op, amount)` -> resultado (operacion permitida solo si la politica lo permite y el handle es valido)
- Maneja de activos y tarifas:
  - Las operaciones de activos se autorizan por las políticas ISI/rol del DS; las tarifas se pagan en el token de gas del DS. Los tokens de capacidad opcionales y políticas más ricas (multi-approver, rate-limits, geofencing) se pueden agregar más sin cambiar el modelo atómico.
- Determinismo: todas las nuevas syscalls son puras y deterministas dadas las entradas y los conjuntos de lectura/escritura AMX declarados. Sin efectos ocultos de tiempo o entorno.Pruebas de validez post-cuanticas (ISI generalizados)
- FASTPQ-ISI (PQ, sin configuración confiable): un argumento basado en hash que generaliza el diseño de transferencia a todas las familias ISI mientras apunta a una prueba sub-segundo para muchos de escala 20k en hardware clase GPU.
  - Perfil operativo:
    - Los nodos de producción construyen el prover vía `fastpq_prover::Prover::canonical`, que ahora siempre inicializa el backend de producción; el simulacro determinista fue removido. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) y `irohad --fastpq-execution-mode` permiten a los operadores fijar la ejecución CPU/GPU de forma determinista mientras el observer hook registra triples solicitados/resueltos/backend para auditorías de flota. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetización:
  - KV-Update AIR: trata WSV como un mapa clave-valor tipado comprometido vía Poseidon2-SMT. Cada ISI se expande a un pequeño conjunto de filas lectura-verificación-escritura sobre claves (cuentas, activos, roles, dominios, metadatos, suministro).
  - Restricciones con puertas de opcode: una sola tabla AIR con columnas selectoras impone reglas por ISI (conservación, contadores monotonic, permisos, range checks, actualizaciones de metadata acotadas).- Argumentos de búsqueda: tablas transparentes comprometidas por hash para permisos/roles, precisiones de activos y parámetros de política evitan restricciones bit a bit pesadas.
- Compromisos y actualizaciones de estado:
  - Prueba SMT agregada: todas las claves tocadas (pre/post) se prueban contra `old_root`/`new_root` usando un frontier comprimido con siblings deduplicados.
  - Invariantes: invariantes globales (p. ej., suministro total por activo) se imponen vía igualdad de multiconjuntos entre filas de efecto y contadores rastreados.
- Sistema de prueba:
  - Compromisos polinomiales estilo FRI (DEEP-FRI) con alta aridad (8/16) y Blow-up 8-16; hashes Poseidon2; transcripción Fiat-Shamir con SHA-2/3.
  - Recursión opcional: agregación recursiva local a DS para comprimir microlotes a una prueba por slot si se necesita.
- Alcance y ejemplos cubiertos:
  - Activos: transferir, acuñar, quemar, registrar/anular el registro de definiciones de activos, establecer precisión (acotado), establecer metadatos.
  - Cuentas/Dominios: crear/eliminar, establecer clave/umbral, agregar/eliminar firmantes (solo estado; las verificaciones de firma se atestan por validadores DS, no se prueban dentro del AIR).
  - Roles/Permisos (ISI): otorgar/revocar roles y permisos; impuestos por tablas de búsqueda y cheques de política monotónica.- Contratos/AMX: marcadores comenzar/commitir AMX, capacidad mint/revoke si está habilitado; se prueban como transiciones de estado y contadores de política.
- Checks fuera del AIR para preservar la latencia:
  - Firmas y criptografia pesada (p. ej., firmas ML-DSA de usuarios) se verifican por validadores DS y se atestan en el DS QC; la prueba de validez cubre solo consistencia de estado y cumplimiento de políticas. Esto prueba mantiene PQ y rapidas.
- Objetivos de rendimiento (ilustrativos, CPU de 32 núcleos + una GPU moderna):
  - 20k ISI mixtas con key-touch pequeño (<=8 claves/ISI): ~0.4-0.9 s de prueba, ~150-450 KB de prueba, ~5-15 ms de verificación.
  - ISI mas pesadas (mas claves/constraints ricas): micro-lotes (p. ej., 10x2k) + recursión para mantener por slot <1 s.
- Configuración de manifiesto DS:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (firmas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (por defecto; las alternativas deben declararse explícitamente)
- Opciones alternativas:
  - ISI complejas/personalizadas pueden usar un STARK general (`zk.policy = "stark_fri_general"`) con prueba diferida y finalización de 1 s vía atestación QC + slashing en pruebas invalidas.
  - Las opciones no PQ (p. ej., Plonk con KZG) requieren configuración confiable y ya no se soportarán en la compilación por defecto.Introducción a AIR (para Nexus)
- Traza de ejecución: matriz con ancho (columnas de registros) y longitud (pasos). Cada fila es un paso lógico del procesamiento ISI; las columnas contienen valores pre/post, selectores y flags.
- Restricciones:
  - Restricciones de transición: imponen relaciones fila a fila (p. ej., post_balance = pre_balance - cantidad para una fila de débito cuando `sel_transfer = 1`).
  - Restricciones de frontera: vinculan E/S publica (old_root/new_root, contadores) a la primera/ultima fila.
  - Búsquedas/permutaciones: aseguran membresia e igualdades de multiconjuntos contra tablas comprometidas (permisos, parámetros de activos) sin circuitos pesados ​​de bits.
- Compromiso y verificación:
  - El prover compromete trazas vía codificaciones hash y construye polinomios de bajo grado que son válidos si las restricciones se cumplen.
  - El verificador comprueba bajo grado vía FRI (basado en hash, post-cuantico) con pocas aperturas Merkle; el costo es logarítmico en los pasos.
- Ejemplo (Transferencia): los registros incluyen pre_balance, importe, post_balance, nonce y selectores. Las restricciones imponen no negatividad/rango, conservación y monotonicidad de nonce, mientras una multiprueba SMT agregada vincula hojas pre/post a las raíces viejas/nuevas.Evolución de ABI y syscalls (ABI v1)
- Syscalls a agregar (nombres ilustrativos):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos de Pointer-ABI para agregar:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Actualizaciones requeridas:
  - Agregar a `ivm::syscalls::abi_syscall_list()` (mantener orden), gatear por política.
  - Mapear numeros desconocidos a `VMError::UnknownSyscall` en hosts.
  - Actualizar pruebas: syscall list golden, ABI hash, pointer type ID goldens y tests de políticas.
  - Documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modelo de privacidad
- Contención de datos privados: cuerpos de transacción, diffs de estado y snapshots WSV de private DS nunca salen del subconjunto privado de validadores.
- Exposición pública: solo headers, compromisos DA y pruebas de validez PQ se exportan.
- Pruebas ZK opcionales: private DS pueden producir pruebas ZK (p. ej., saldo suficiente, politica cumplida) habilitando acciones cross-DS sin revelar estado interno.
- Control de acceso: la autorización se impone por políticas ISI/rol dentro del DS. Los tokens de capacidad son opcionales y pueden introducirse más adelante.Aislamiento de rendimiento y QoS
- Consenso, mempools y almacenamiento separados por DS.
- Cuotas de scheduling nexus por DS para limitar el tiempo de inclusión de anclajes y evitar el bloqueo de cabecera de línea.
- Presupuestos de recursos de contrato por DS (compute/memory/IO), impuestos por el host IVM. La contención en DS públicos no puede consumir presupuestos de DS privados.
- Llamadas cross-DS asincronas evitan esperas sincronas largas dentro de ejecucion private-DS.

Disponibilidad de datos y diseño de almacenamiento
1) Codificación de borrado
- Usar Reed-Solomon sistematico (p. ej., GF(2^16)) para codificación de borrado a nivel de blob de bloques Kura y snapshots WSV: parámetros `(k, m)` con `n = k + m` shards.
- Parámetros por defecto (propuestos, public DS): `k=32, m=16` (n=48), permitiendo recuperación de hasta 16 fragmentos perdidos con ~1.5x expansión. Para DS privado: `k=16, m=8` (n=24) dentro del conjunto permitido. Ambos configurables por DS Manifest.
- Blobs publicos: shards distribuidos a través de muchos nodos DA/validadores con comprobaciones de disponibilidad por muestreo. Los compromisos DA en headers permiten verificación por clientes ligeros.
- Blobs privados: fragmentos cifrados y distribuidos solo entre validadores privados-DS (o custodios designados). La cadena global solo lleva compromisos DA (sin ubicaciones de shards ni llaves).2) Compromisos y muestreos
- Para cada blob: calcular una raíz de Merkle sobre fragmentos e incluirlo en `*_da_commitment`. Mantener PQ evitando compromisos de curva elíptica.
- DA Attesters: attesters regionales muestreados por VRF (p. ej., 64 por región) emiten un certificado ML-DSA-87 atestando exitoso muestreo de shards. Objetivo de latencia de atestación DA <=300 ms. El comité nexus valida certificados en lugar de extraer shards.

3) Integración con Kura
- Los bloques almacenan cuerpos de transacción como blobs codificados con borrado con compromisos Merkle.
- Los encabezados llevan compromisos de blob; los cuerpos se recuperan vía la red DA para public DS y vía canales privados para private DS.

4) Integración con WSV
- Instantáneas WSV: periódicamente se hace checkpoint del estado DS en instantáneas por fragmentos codificados con borrado con compromisos registrados en encabezados. Entre instantáneas, se mantienen registros de cambios. Las instantáneas públicas se fragmentan ampliamente; Las instantáneas privadas permanecen dentro de validadores privados.
- Acceso con pruebas: los contratos pueden proporcionar (o solicitar) pruebas de estado (Merkle/Verkle) ancladas por compromisos de snapshot. Private DS pueden proporcionar pruebas de conocimiento cero en lugar de pruebas crudas.5) Retención y poda
- Sin poda para public DS: retener todos los cuerpos Kura y snapshots WSV vía DA (escalado horizontal). Private DS pueden definir retención interna, pero los compromisos exportados permanecen inmutables. La capa nexus retiene todos los bloques Nexus y los compromisos de artefactos DS.

Red y roles de nudos
- Validadores globales: participante en el consenso nexus, validan bloques Nexus y artefactos DS, realiza checks DA para public DS.
- Validadores de Data Space: ejecutan consenso DS, ejecutan contratos, gestionan Kura/WSV local, manejan DA para su DS.
- Nodos DA (opcional): almacenan/publican blobs públicos, facilitan muestreos. Para Private DS, los nodos DA se co-ubican con validadores o custodios confiables.Mejoras y consideraciones a nivel de sistema
- Desacoplar secuenciacion/mempool: adoptar un mempool DAG (p. ej., estilo Narwhal) que alimenta un BFT con pipeline en la capa nexus para bajar latencia y mejorar el rendimiento sin cambiar el modelo lógico.
- Cuotas DS y fairness: cuotas por DS por bloque y caps de peso para evitar head-of-line blocking y asegurar latencia predecible para DS privado.
- Atestacion DS (PQ): los certificados de quorum DS usan ML-DSA-87 (clase Dilithium5) por defecto. Es post-cuantico y más grande que firmas EC pero aceptable con un QC por slot. DS pueden optar explícitamente por ML-DSA-65/44 (más pequeño) o firmas EC si se declara en el DS Manifest; Se recomienda mantener ML-DSA-87 para el DS público.
- DA attesters: para DS público, usar attesters regionales muestreados por VRF que emiten certificados DA. El comité nexus valida certificados en lugar de muestreo de shards crudos; DS privado mantiene testaciones DA internas.
- Recursion y pruebas por epoca: opcionalmente agrega varios microlotes dentro de un DS en una prueba recursiva por slot/epoca para mantener tamano de prueba y tiempo de verificación estables bajo alta carga.- Escalado de carriles (si se necesita): si un comité global único se vuelve un cuello de botella, introduzca K carriles de secuenciación paralelas con un merge determinista. Esto preserva un orden global único mientras escala horizontalmente.
- Aceleración determinista: proporciona kernels SIMD/CUDA con indicadores de función para hash/FFT con respaldo de CPU bit-exacto para preservar el determinismo entre hardware.
- Umbrales de activación de carriles (propuesta): habilitar 2-4 carriles si (a) p95 de finalización excede 1.2 s por >3 minutos consecutivos, o (b) la ocupación por bloque excede 85% por >5 minutos, o (c) la tasa entrante de tx requeriria >1.2x la capacidad de bloque en niveles sostenidos. Los carriles agrupan transacciones de forma determinista por hash de DSID y se fusionan en el bloque nexus.

Tarifas y economía (valores iniciales)
- Unidad de gas: token de gas por DS con computar/IO medido; las tarifas se pagan en el activo de gas nativo del DS. La conversión entre DS es responsabilidad de la aplicación.
- Prioridad de inclusión: round-robin entre DS con cuotas por DS para preservar la equidad y SLOs de 1s; Dentro de un DS, el fee bidding puede desempatar.
- Futuro: se puede explorar un mercado global de tarifas o políticas que minimicen MEV sin cambiar la atomicidad ni el diseño de pruebas PQ.Flujo cross-Data-Space (ejemplo)
1) Un usuario envia una transacción AMX que toca public DS P y private DS S: mover activo X desde S a beneficiario B cuya cuenta está en P.
2) Dentro del slot, P y S ejecutan su fragmento contra la instantánea del slot. S verificacion autorizacion y disponibilidad, actualiza su estado interno y produce una prueba de validez PQ y compromiso DA (sin filtrar datos privados). P prepara la actualización de estado correspondiente (p. ej., mint/burn/locking en P segun politica) y su prueba.
3) El comité nexus verifica ambas pruebas DS y certificados DA; si ambas verifican dentro del slot, la transacción se confirma atómicamente en el bloque Nexus de 1s, actualizando ambas raíces DS en el vector de estado mundial global.
4) Si alguna prueba o certificado DA falta o es invalido, la transacción se cancela (sin efectos) y el cliente puede reenviar para el siguiente slot. Ningun dato privado sale de S en ningun paso.- Consideraciones de seguridad
- Ejecucion determinista: las syscalls IVM permanecen deterministas; los resultados cross-DS los dictan AMX commit y finalizacion, no el reloj o el timing de red.
- Control de acceso: los permisos ISI en DS privado restringen a quien puede enviar transacciones y que operaciones se permiten. Los tokens de capacidad codifican derechos de grano fino para uso cross-DS.
- Confidencialidad: cifrado de extremo a extremo para datos private-DS, shards codificados con borrado almacenados solo entre miembros autorizados, pruebas ZK opcionales para atestaciones externas.
- Resistencia a DoS: el aislamiento en mempool/consenso/almacenamiento evita que la congestión pública impacte el progreso de DS privado.

Cambios en componentes de Iroha
- iroha_data_model: introducir `DataSpaceId`, IDs calificados por DS, descriptores AMX (conjuntos de lectura/escritura), tipos de prueba/compromisos DA. Serialización solo Norito.
- ivm: agregar syscalls y tipos pointer-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) y pruebas DA; pruebas/docs de ABI actualizar según la política v1.