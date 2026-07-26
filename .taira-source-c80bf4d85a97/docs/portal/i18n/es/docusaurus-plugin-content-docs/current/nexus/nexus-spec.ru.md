---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: especificación-nexus
título: Техническая спецификация Sora Nexus
descripción: Полное отражение `docs/source/nexus.md`, охватывающее архитектуру and ограничения проектирования для Iroha 3 (Sora Nexus).
---

:::nota Канонический источник
Esta página está escrita `docs/source/nexus.md`. Deje copias sincronizadas de todos los bloques que no estén disponibles en el portal.
:::

#! Iroha 3 - Sora Nexus Libro mayor: Especificaciones técnicas

Este documento del arquitecto Sora Nexus Ledger para Iroha 3, editado Iroha 2 en edición Libro mayor logístico global y espacio de datos organizado (DS). Espacios de datos предоставляют сильные домены приватности ("espacios de datos privados") y открытую участие ("espacios de datos públicos"). El libro mayor global de componibilidad de la red social está diseñado para aislar y confiar en la privacidad de DS privado y la descarga de archivos adjuntos. Aquí hay varias configuraciones en Kura (almacenamiento en bloque) y WSV (World State View).

Aquí y todo el repositorio combina Iroha 2 (conjuntos autohospedados) e Iroha 3 (SORA Nexus). Información adicional sobre la máquina virtual Iroha (IVM) y la cadena de herramientas Kotodama, contratos de contactos y artefactos байткода остаются переносимыми между self-hospedado развертываниями y global ledger Nexus.Цели
- Un libro de contabilidad global de Odín, un conjunto de validadores cooperativos y espacios de datos.
- Espacios de datos privados para operadores autorizados (por ejemplo, CBDC), que no contienen DS privados.
- Espacios de datos públicos с открытым участием, доступом в стиле Ethereum.
- Los contratos inteligentes Composable permiten utilizar espacios de datos para crear archivos de activación de DS privado.
- Изоляция производительности, чтобы public активность не ухудшала private-DS внутренние транзакции.
- Descarga de datos en la computadora: Kura y WSV con código de borrado para aplicaciones prácticas de datos privados de DS.

Не цели (начальная фаза)
- Определение токеномики или стимулов валидаторов; programación de políticas y apuestas должны оставаться подключаемыми.
- Введение новой версии ABI; Las aplicaciones de ABI v1 están diseñadas para realizar llamadas al sistema y punteros ABI según la política IVM.términos
- Nexus Libro mayor: Libro mayor lógico global, bloques compuestos integrados de Data Space (DS) en una historia y compromiso actualizados состояния.
- Data Space (DS): Ограниченный домен исполнения и хранения со своими валидаторами, gobernancia, классом приватности, DA политикой, квотами и политикой комиссий. Существуют два класса: DS público y DS privado.
- Espacio de datos privado: validadores y controladores de descarga autorizados; данные транзакций и состояния никогда не выходят за DS. Глобально якорятся только compromisos/metadatos.
- Espacio de datos públicos: acceso sin permiso; полные данные и состояние публичны.
- Manifiesto de espacio de datos (Manifiesto DS): manifiesto codificado con Norito, parámetros de declaración DS (validadores/claves de control de calidad, privacidad de clase, política ISI, parámetros DA, retención, cuotas, política ZK, comissias). Anclaje del manifiesto hash en la cadena de nexo. Si no es así, los certificados de quórum DS utilizan ML-DSA-87 (clase Dilithium5) como un complemento de diseño moderno.
- Directorio espacial: contrato de directorio global en cadena, manifiestos de DS actualizados, versiones y gobernanza/rotación de datos para registros y auditorías.
- DSID: Espacio de datos de identificador único global. Используется для namespace всех объектов и ссылок.
- Ancla: compromiso criptográfico del bloque/bloqueo de DS, incluido en la cadena de nexo, que sigue la historia de DS en el libro mayor global.- Kura: Хранилище блоков Iroha. Здесь расширяется compromisos y almacenamiento de blobs con código de borrado.
- WSV: Iroha Vista del estado mundial. Здесь расширяется версионированными, segmentos con capacidad de captura de instantáneas y con codificación de borrado.
- IVM: Máquina virtual Iroha para dispositivos inteligentes (código de bytes Kotodama `.to`).
  - AIRE: Representación Algebraica Intermedia. Алгебраическое представление вычислений для подобных доказательств, описывающее исполнение как трассы с переходными и граничными ограничениями.Модель Espacios de datos
- Identificador: `DataSpaceId (DSID)` identifica DS y espacio de nombres. DS puede configurar instalaciones en dos grandes grupos:
  - Dominio-DS: `ds::domain::<domain_name>` - исполнение и состояние, ограниченные доменом.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - исполнение и состояние, ограниченные одной дефиницией актива.
  Обе формы сосуществуют; транзакции могут атомарно касаться нескольких DSID.
- Manifiesto del ciclo actual: создание DS, обновления (ротация ключей, изменения политики) y вывод из эксплуатации записываются в Space Directorio. El artefacto DS por ranura se crea mediante un hash de manifiesto posterior.
- Clases: DS público (открытое участие, DA público) y DS privado (DA autorizado, confidencial). Гибридные политики возможны через флаги manifest.
- Políticas en DS: permisos ISI, parámetros DA `(k,m)`, шифрование, retención, квоты (min/max доля tx на блок), ZK/política de prueba optimista, комиссии.
- Gobernanza: членство DS y ротация валидаторов определяются секцией manifiesto de gobernanza (on-chain предложения, multisig или внешняя Governance, заякоренная транзакциями nexus и atestados).Manifiestos de capacidad y UAID
- Cuentas universales: каждый участник получает детерминированный UAID (`UniversalAccountId` в `crates/iroha_data_model/src/nexus/manifest.rs`), который охватывает все espacios de datos. Manifiestos de capacidad (`AssetPermissionManifest`) связывают UAID с конкретным dataspace, epoch активации/истечения and упорядоченным списком enable/deny `ManifestEntry` правил, Están configurados `dataspace`, `program_id`, `method`, `asset` y funciones AMX opcionales. Negar правила всегда побеждают; El evaluador utiliza `ManifestVerdict::Denied` con una auditoría específica o una concesión `Allowed` con metadatos de asignación de datos.
- Asignaciones: каждая permitir запись несет детерминированные `AllowanceWindow` cubos (`PerSlot`, `PerMinute`, `PerDay`) y opcionales `max_amount`. Los hosts y el SDK se pueden usar con la carga útil Norito, lo que permite implementar aplicaciones en el tiempo y el SDK.
- Telemetría de auditoría: Directorio espacial traducido `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) para el manifiesto de configuración. La nueva opción `SpaceDirectoryEventFilter` incluye Torii/data-event que permite eliminar manifiestos, revocaciones y denegaciones de victorias de UAID решения без кастомного fontanería.

Para la documentación operativa de un extremo a otro, las notas de migración del SDK y la lista de verificación del manifiesto de publicación se sincronizan con esta guía de cuenta universal (`docs/source/universal_accounts_guide.md`). Aquí encontrará documentos sobre políticas e instrumentos de la UAID.Arquitecto верхнего уровня
1) Cadena de composición global (cadena Nexus)
- Поддерживает единый canonical порядок 1-секундных Nexus Bloques, которые финализируют атомарные транзакции, затрагивающие один или более Espacios de datos (DS). Каждая comprometido транзакция обновляет единый глобальный estado mundial (vector por raíces DS).
- Содержит метаданные метаданные плюс агрегированные pruebas/QC para la componibilidad, finalidad y detección de fraude (desbloqueo DSID, raíces estatales por DS до/после, compromisos DA, pruebas de validez por DS y certificado de quórum DS) (véase ML-DSA-87). Приватные данные не включаются.
- Consenso: el comité BFT global en desarrollo tiene una sección 22 (3f+1 с f=7), la época en la que VRF/stake es de aproximadamente 200.000 candidatos. Nexus comité упорядочивает транзакции и финализирует блок за 1s.

2) Espacio de datos Слой (público/privado)
- Исполняет por-DS фрагменты глобальных транзакций, обновляет локальный DS WSV and proisevodit artefactos de validez por bloque (pruebas agregadas por-DS y compromisos DA), которые сворачиваются в 1-секундный Nexus Block.
- DS privados en reposo y en vuelo mediante validadores autorizados; наружу выходят только compromisos y pruebas de validez de PQ.
- Cuerpos de datos públicos de DS (a través de DA) y pruebas de validez de PQ.3) Transacciones entre el espacio de datos y el átomo (AMX)
- Modelo: каждая пользовательская транзакция может касаться нескольких DS (por ejemplo, dominio DS y odin o несколько activo DS). Она коммитится атомарно в одном Nexus Block или откатывается; частичных эффектов нет.
- Prepare-Commit para 1s: para las transmisiones de datos individuales de DS implementadas de forma paralela en una instantánea diferente (raíces de DS en la ranura) y descargas por DS Pruebas de validez de PQ (FASTPQ-ISI) y compromisos DA. Nexus comité коммитит транзакцию только если все требуемые DS pruebas проверяются и DA certificados приходят вовремя (цель <=300 ms); иначе транзакция переносится на следующий слот.
- Consistencia: lectura-escritura наборы объявляются; Se detecta un conflicto con la confirmación de las raíces de inicio de ranura. La parada optimista sin bloqueo del DS es global; atomicidad обеспечивается правилом nexus commit (todo o nada en DS).
- Privacidad: DS privado exporta pruebas/compromisos, привязанные к pre/post raíces DS. Никакие сырые privado данные не покидают DS.4) Disponibilidad de datos (DA) с codificación de borrado
- Kura crea cuerpos de bloques e instantáneas WSV como blobs codificados para borrar. Публичные blobs широко shard-ятся; Los blobs privados crean tanto validadores de DS privados como fragmentos de datos.
- Los compromisos de DA se incluyen en los artefactos de DS y en los bloques Nexus, y se garantizan muestras y recuperación mediante un registro de seguridad privado.

Bloqueo estructural y compromiso
- Artefacto de prueba de espacio de datos (en 1s, en DS)
  - Polo: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Private-DS exporta artefactos desde cuerpos de datos; public DS позволяют получение body через DA.

- Bloque Nexus (cadencia 1s)
  - Polo: block_number, parent_hash, slot_time, tx_list (transmisiones entre DS con DSID), ds_artifacts[], nexus_qc.
  - Funciones: finalización de transacciones atómicas, como resultado de artefactos DS; обновляет глобальный vector DS raíces одним шагом.Acuerdo y programación
- Consenso de cadena Nexus: единый глобальный BFT canalizado (clase Sumeragi) con 22 comités (3f+1, f=7) para bloques de 1s y finalidad de 1s. El comité está dividido en épocas en las que VRF/stake tiene ~200.000 candidatos; La rotación puede ser descentralizada y tensa.
- Consenso de espacio de datos: каждый DS запускает свой BFT для artefactos por ranura (pruebas, compromisos de DA, DS QC). Comités de retransmisión de carril размером `3f+1` используют `fault_tolerance` dataspace and детерминированно сэмплируются по epoch из пула валидаторов dataspace с VRF seed, привязанным к `(dataspace_id, lane_id)`. DS privado: autorizado; DS público: vida abierta con política anti-Sybil. Comité de nexo global неизменен.
- Programación de transacciones: permite realizar transacciones automáticas con DSID y funciones de lectura y escritura. DS está instalado paralelamente a la ranura; El comité de nexo envía transmisiones en bloques de 1 segundo, además de artefactos DS disponibles y certificados DA disponibles (<=300 ms).
- Aislamiento de rendimiento: en el catálogo de DS se utilizan mempools e implementaciones. Las cuotas por DS se organizan en bloques de datos de DS, se bloquea el encabezado de línea anterior y se reduce la latencia del DS privado.Modelo de datos y espacio de nombres
- ID calificados por DS: все сущности (dominios, cuentas, activos, roles) квалифицированы `dsid`. Ejemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias globales: código mundial: corte `(dsid, object_id, version_hint)` y se pueden combinar on-chain en la capa de nexo o en descriptores AMX para cross-DS.
- Serialización Norito: все cross-DS сообщения (descriptores AMX, pruebas) используют códecs Norito. Serde в продакшене не применяется.Contratos inteligentes y aplicaciones IVM
- Contexto de ejecución: agregue `dsid` al concurso IVM. Kotodama los contratos están implementados en el espacio de datos consolidado.
- Primitivas atómicas Cross-DS:
  - `amx_begin()` / `amx_commit()` configura la transmisión atómica multi-DS al host IVM.
  - `amx_touch(dsid, key)` elimina la intención de lectura/escritura para la detección de conflictos en la ranura de raíces de instantáneas.
  - `verify_space_proof(dsid, proof, statement)` -> booleano
  - `use_asset_handle(handle, op, amount)` -> resultado (операция разрешена только при политическом разрешении и валидном handle)
- Manejo de activos y tarifas:
  - Операции активов авторизуются politicamy ISI/rol DS; комиссии оплачиваются в gas tokene DS. Tokens de capacidad opcionales y políticas más ricas (aprobadores múltiples, límites de tasas, geocercas) pueden incluirse en modelos atómicos de personalización.
- Determinismo: все новые syscalls чистые и детерминированные при данных входах и заявленных AMX read/write наборах. No hay efectos negativos o negativos.Pruebas de validez poscuánticas (ISI generalizadas)
- FASTPQ-ISI (PQ, sin configuración confiable): argumento basado en hash, prueba de transferencia de datos a través de ISI, prueba de transferencia de calor por lotes de 20k en GPU железа.
  - Perfil operativo:
    - Los nodos de producción строят prover через `fastpq_prover::Prover::canonical`, который теперь всегда инициализирует backend de producción; детерминированный simulacro de удален. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) e `irohad --fastpq-execution-mode` permiten al operador determinar la configuración de CPU/GPU, una configuración de enlace de observador solicitada/resuelta/backend тройки для auditorías de flotas. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetización:
  - KV-Update AIR: tratado WSV con un mapa clave-valor tipográfico, actualizado con Poseidon2-SMT. Каждый ISI разворачивается в небольшой набор lectura-verificación-escritura строк по ключам (cuentas, activos, roles, dominios, metadatos, suministro).
  - Restricciones controladas por código de operación: las tablas de AIR con columnas selectoras aplican-ит правила по ISI (conservación, contadores monótonos, permisos, comprobaciones de rango, actualizaciones de metadatos limitados).
  - Argumentos de búsqueda: tablas de permisos/roles confirmadas por hash, precisiones de activos y parámetros de políticas que están configurados bit a bit.- Compromisos y actualizaciones estatales:
  - Prueba SMT agregada: все затронутые ключи (pre/post) доказываются против `old_root`/`new_root` с компрессированным frontier и deduped siblings.
  - Invariantes: variables globales (por ejemplo, suministro total por activo) que imponen la igualdad de conjuntos múltiples mediante filas de efectos y contadores rastreados.
- Sistema de prueba:
  - Compromisos polinómicos estilo FRI (DEEP-FRI) con высокой arity (8/16) y explosión 8-16; hashes Poseidon2; Transcripción de Fiat-Shamir con SHA-2/3.
  - Recursividad opcional: agregación recursiva DS-local para crear microlotes en una prueba única de espacios reducidos.
- Alcance y primeros pasos:
  - Activos: transferir, acuñar, quemar, registrar/anular el registro de definiciones de activos, establecer precisión (limitada), establecer metadatos.
  - Cuentas/Dominios: crear/eliminar, establecer clave/umbral, agregar/eliminar firmantes (solo para el estado; проверки подписи аттестуются validadores DS, не доказываются в AIR).
  - Roles/Permisos (ISI): otorgar/revocar roles y permisos; tablas de búsqueda obligatorias y controles de políticas monótonos.
  - Contratos/AMX: marcadores de inicio/compromiso de AMX, capacidad de acuñar/revocar при включении; доказываются как transiciones estatales y contadores de políticas.
- Comprobaciones fuera del aire para la latencia del sistema:- Datos y criptografía (por ejemplo, firmas de usuario ML-DSA) que verifican los validadores DS y las certificaciones en DS QC; prueba de validez покрывает только consistencia estatal y cumplimiento de políticas. Это оставляет pruebas PQ и быстрыми.
- Objetivos de rendimiento (иллюстративно, CPU de 32 núcleos + GPU integrada):
  - 20.000 ISI mixtos con pulsación de tecla no deseada (<=8 teclas/ISI): ~0,4-0,9 s de prueba, ~150-450 KB de prueba, ~5-15 ms de verificación.
  - Более тяжелые ISI: micro-batch (por primera vez, 10x2k) + recursividad, чтобы держать <1 s на слот.
- Configuración del manifiesto DS:
  -`zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (probador de DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (по умолчанию; альтернативы должны быть явно объявлены)
- Opciones alternativas:
  - Сложные/кастомные ISI puede utilizar STARK (`zk.policy = "stark_fri_general"`) con prueba diferida y finalidad de 1 через QC аттестацию + corte en nunca доказательствах.
  - Las variantes que no son PQ (por ejemplo, Plonk con KZG) incluyen una configuración confiable y no se pueden utilizar en una computadora portátil.Cebador AIR (para Nexus)
- Seguimiento de ejecución: матрица с шириной (registrar columnas) и длиной (pasos). Каждая строка - логический шаг ISI; столбцы держат pre/post значения, selector и flags.
- Restricciones:
  - Restricciones de transición: aplicar отношения между строками (por ejemplo, post_balance = pre_balance - importe de las cuentas de débito según `sel_transfer = 1`).
  - Restricciones de límites: привязывают public I/O (old_root/new_root, contadores) к первой/последней строкам.
  - Búsquedas/permutaciones: garantiza la membresía y la igualdad de conjuntos múltiples y protege las tablas comprometidas (permisos, parámetros de activos) entre sí.
- Compromiso y verificación:
  - Prover combina varios codificadores basados en hash y polinomios de bajo grado, válidos para aplicaciones originales.
  - Verificador proporciona un grado bajo de FRI (basado en hash, post-cuántico) con nuevas aplicaciones de Merkle; стоимость логарифмична по pasos.
- Ejemplo (Transferencia): registros de pre_balance, importe, post_balance, nonce y selectores. Las organizaciones imponen nonce/disipación, sostenimiento y monotonía, y una prueba múltiple SMT agregada que combina hojas pre/post con raíces viejas/nuevas.Evolución de ABI y syscalls (ABI v1)
- Syscalls к добавлению (иллюстративные имена):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Puntero-ABI tipos de configuración:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Требуемые обновления:
  - Добавить в `ivm::syscalls::abi_syscall_list()` (сохранять порядок), gate по политике.
  - Introduzca el número nuevo en `VMError::UnknownSyscall` en los hosts.
  - Pruebas destacadas: lista de llamadas al sistema golden, hash ABI, ID de tipo de puntero goldens y pruebas de políticas.
  - Documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Модель приватности
- Contención de datos privados: transmisiones de datos, configuraciones diferentes y instantáneas WSV de archivos DS privados que no contienen subconjuntos de validadores privados.
- Exposición pública: encabezados exportados, compromisos DA y pruebas de validez PQ.
- Pruebas ZK opcionales: pruebas ZK privadas de DS (например, достаточный баланс, соблюдение политики), позволяя cross-DS действия без раскрытия внутреннего состояния.
- Control de acceso: авторизация политиками ISI/rol внутри DS. Los tokens de capacidad son opcionales y se pueden utilizar como opción.Configuración de aislamiento y QoS
- Consenso antiguo, mempools y almacenamiento en DS.
- Nexus cuotas de programación por DS que organizan anclajes de enlace y bloqueo de cabecera de línea previo.
- Los presupuestos de recursos de contacto por DS (cómputo/memoria/IO) aplicaron el host-ом IVM. La contención de DS público no puede bloquear archivos de DS privados.
- Los usuarios de DS cruzados sincronizados están conectados a dispositivos DS privados.

Disponibilidad de datos y diseño de almacenamiento.
1) Codificación de borrado
- Utilización sistemática de Reed-Solomon (por ejemplo, GF(2^16)) para la codificación de borrado a nivel de blob de bloques Kura y instantáneas WSV: parámetros `(k, m)` y fragmentos `n = k + m`.
- Parámetros completos (DS público): `k=32, m=16` (n=48), integrados en 16 fragmentos portátiles con una expansión de ~1,5x. Для privado DS: `k=16, m=8` (n=24) внутри набора autorizado. Оба конфигурируются según DS Manifest.
- Blobs públicos: fragmentos relacionados con muchos nodos/validadores DA y comprobaciones de disponibilidad basadas en muestreo. Los compromisos de DA en los encabezados позволяют clientes ligeros проверять.
- Blobs privados: fragmentos de validadores DS privados (o custodios privados). La clasificación global no requiere compromisos DA (sin ubicaciones de fragmentos o claves).2) Compromisos y muestreo
- Para un blob: agregue la raíz de Merkle a los fragmentos y agregue a `*_da_commitment`. Сохранять PQ, избегая compromisos de curva elíptica.
- DA Attesters: attestadores regionales muestreados por VRF (por ejemplo, 64 por región) cuentan con la certificación ML-DSA-87 de fragmentos de muestreo habituales. Целевая Latencia de atestación DA <=300 ms. Nexus comité de validación de fragmentos.

3) Integración de Kura
- Bloquea los cuerpos de transacciones y los blobs codificados por borrado de los compromisos de Merkle.
- Los encabezados no son compromisos de blobs; Los cuerpos están disponibles en canales privados para DS públicos y en canales privados para DS privados.

4) Integración WSV
- Instantáneas WSV: configuración periódica del punto de control DS en instantáneas fragmentadas y codificadas para borrar, en compromisos y encabezados. Algunas instantáneas pueden cambiar los registros. Instantáneas públicas широко shard-ятся; instantáneas privadas instaladas en validadores privados.
- Acceso con pruebas: контракты могут предоставлять (или запрашивать) pruebas estatales (Merkle/Verkle), compromisos de instantáneas anclados. DS privado puede incluir certificaciones de conocimiento cero en pruebas sin procesar.5) Retención y poda
- Poda de red para DS público: mejora los cuerpos de Kura y las instantáneas de WSV en DA (masticación masiva). El DS privado puede impedir la retención de datos, no hay compromisos de exportación necesarios. La capa Nexus analiza los bloques Nexus y los compromisos de artefactos DS.

Сеть и роли узлов
- Validadores globales: validan bloques Nexus y artefactos DS, validan comprobaciones DA para DS público.
- Validadores de espacio de datos: запускают DS consenso, обрабатывают DA для своего DS.
- Nodos DA (opcional): blobs públicos de expansión/descarga, muestreo de potencia. Hay nodos DS DA privados ubicados conjuntamente con validadores y custodios principales.Системные улучшения и соображения
- Secuenciación/desacoplamiento de mempool: integración de mempool DAG (por ejemplo, estilo Narwhal), BFT canalizado en la capa de nexo, optimización de la latencia y mejora del rendimiento sin lógica lógica modelos.
- Cuotas y equidad de DS: cuotas por bloque y límites de peso por DS para el bloqueo de cabecera de línea y latencia previa para el DS privado.
- Atestación DS (PQ): Certificados de quórum DS используют ML-DSA-87 (Dilithium5-класс) по умолчанию. Este post-kвантово y mucho, чеm EC подписи, no primero para 1 QC en la ranura. DS puede incluir ML-DSA-65/44 (menьше) o EC подписи, así como en DS Manifest; El DS público recomienda el uso de ML-DSA-87.
- Certificadores DA: para los DS públicos utilizan certificadores regionales muestreados con VRF, certificados DA seleccionados. Nexus comité de validación de muestras de fragmentos sin procesar; DS privado держат DA atestaciones внутренними.
- Recursión y pruebas de época: agrega opcionalmente microlotes no deseados en DS o prueba recursiva en ranura/epoca para una base estable pruebas y verificación completa нагрузке.
- Escalado de carriles (если нужно): если единый глобальный comité становится узким местом, ввести K параллельных carriles de secuenciación con детерминированным fusión. Este es un dispositivo globalizado y electrónico globalizado y masticable.- Aceleración determinista: prefiera los kernels SIMD/CUDA y los indicadores de características para hash/FFT con respaldo de CPU de bits exactos, lo que le permitirá determinar de forma más rápida el proceso.
- Umbrales de activación de carril (propuesta): включать 2-4 carriles, если (a) p95 finalidad превышает 1,2 s более 3 minут подряд, или (b) ocupación por cuadra превышает 85% более 5 minут, или (c) velocidad de transmisión máxima требует >1.2x capacidad de bloque устойчиво. Los carriles son una transferencia predeterminada de cubos mediante hash DSID y fusión en un bloque de nexo.

Tarifas y economía (начальные дефолты)
- Unidad de gas: token de gas por DS con cálculo/IO medido; tarifas оплачиваются в нативном gas activo DS. Conversión de DS: configuración no disponible.
- Prioridad de inclusión: round robin по DS с cuotas por DS для equidad и 1s SLO; внутри DS tarifa de licitación может разруливать.
- Futuro: mercado global de tarifas y políticas de minimización de MEV gracias a la atomicidad y la prueba PQ.Flujo de trabajo entre espacios de datos (пример)
1) Пользователь отправляет AMX транзакцию, затрагивающую public DS P и private DS S: переместить activo X из S к beneficiario B, чей аккаунт в P.
2) La ranura P y S introduce dos fragmentos en la instantánea. S проверяет autorización y disponibilidad, обновляет внутреннее состояние и формирует PQ validez prueba y compromiso DA (без утечки datos privados). P готовит соответствующее обновление состояния (por ejemplo, mint/burn/locking в P по политике) y своеproof.
3) Comité Nexus que verifica las pruebas DS y los certificados DA; Además de las válidas en la ranura, la transferencia de compromiso es atómica en 1s Nexus Block, incluida la raíz de DS en el vector de estado mundial global.
4) Si la prueba de descarga o el certificado DA son cancelados/no validados, la cancelación de la transferencia (sin efectos) y el cliente puede cancelar следующий слот. Никакие private данные S не покидают на любом шаге.- Consideraciones de seguridad
- Ejecución determinista: IVM syscalls остаются детерминированными; Los recursos de cross-DS incluyen compromiso y finalidad de AMX, un reloj de pared o varios archivos.
- Control de acceso: permisos ISI en DS privados que pueden controlar las transacciones y operaciones realizadas. Los tokens de capacidad son de grano fino para cross-DS.
- Confidencialidad: codificación de extremo a extremo para archivos DS privados, fragmentos codificados con borrado, herramientas para registros autorizados, pruebas ZK opcionales para archivos personales atestados.
- Resistencia DoS: la aislamiento de mempool/consensus/storage evita la congestión pública en el DS privado.

Configuración de componentes Iroha
- iroha_data_model: incluye `DataSpaceId`, ID calificados para DS, descriptores AMX (conjuntos de lectura/escritura), compromisos de prueba/DA. Número de serie Norito.
- ivm: incluye llamadas al sistema y tipos de puntero-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) y pruebas DA; обновить ABI tests/docs по политике v1.