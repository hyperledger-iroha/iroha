---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: especificación-nexus
título: Técnica de especificación de Sora Nexus
descripción: Espejo completo de `docs/source/nexus.md`, couvrant l'architecture et les contraintes de conception pour le ledger Iroha 3 (Sora Nexus).
---

:::nota Fuente canónica
Esta página informa `docs/source/nexus.md`. Gardez les deux copys alineados justqu'a ce que le backlog de traduction llegan sur le portail.
:::

#! Iroha 3 - Sora Nexus Ledger: Especificación técnica de concepción

Este documento propone la arquitectura de Sora Nexus Ledger para Iroha 3, y permite evolucionar Iroha 2 para un libro mayor global único y logístico unificado para organizar los espacios de datos (DS). Les Data Spaces fournissent des domaines de confidencialite forts ("espacios de datos privados") et une participation ouverte ("espacios de datos públicos"). El diseño preserva la composición a través del libro mayor global garantizando un aislamiento estricto y la confidencialidad de las personas en DS privado, e introduce la escala de disponibilidad de las personas a través del código de borrado en Kura (almacenamiento en bloque) y WSV (World State View).El depósito de memes construyó Iroha 2 (reseaux auto-heberges) y Iroha 3 (SORA Nexus). La ejecución está asegurada por la máquina virtual Iroha (IVM) y la cadena de herramientas Kotodama, además de los contratos y artefactos de código de bytes portátiles entre las implementaciones de auto-heberges y el libro mayor global Nexus.

Objetivos
- Un libro de contabilidad global compuesto por nombres de validadores cooperantes y espacios de datos.
- Los espacios de datos privados para un permiso de operación (p. ej., CBDC), con las personas que no salen nunca del DS privado.
- Los espacios públicos de datos con participación abierta, acceso sin permiso tipo Ethereum.
- Los contratos inteligentes componibles entre Data Spaces, así como los permisos explícitos para el acceso a actividades privadas de DS.
- Aislamiento del rendimiento porque la actividad pública no degrada las transacciones internas privadas-DS.
- Disponibilite des donnees a grande echelle: Kura y WSV con código de borrado para apoyar a las donnees efectivas ilimitadas para garantizar las donnees private-DS privees.No objetivos (fase inicial)
- Definir la economía del token o las incitaciones de los validadores; Las políticas de programación y apuesta son conectables.
- Introducción de una nueva versión de ABI; Los cambios son compatibles con ABI v1 con extensiones explícitas de llamadas al sistema y punteros ABI según la política IVM.Terminología
- Nexus Libro mayor: El libro mayor logístico global está compuesto por bloques Data Space (DS) y tiene una historia ordenada única y un compromiso de estado.
- Espacio de datos (DS): dominio de ejecución y almacenamiento soportado con validadores propios, gobernanza, clase de confidencialidad, política DA, cuotas y política de frais. Existen dos clases: DS pública y DS privada.
- Espacio de datos privado: validadores de permisos y control de acceso; les donnees de transaction et d'etat ne quittent jamais le DS. Seuls des engagements/metadoonnees sont ancres globalement.
- Espacio Público de Datos: Participación sin permiso; les donnees completes et l'etat sont publics.
- Manifiesto de espacio de datos (Manifiesto DS): código de manifiesto en Norito que declara los parámetros DS (validadores/cles QC, clase de confidencialidad, política ISI, parámetros DA, retención, cuotas, política ZK, frais). El hash del manifiesto está anclado en la cadena del nexo. Sin anular, los certificados de quórum DS utilizan ML-DSA-87 (clase Dilithium5) como el esquema de firma post-cuántico por defecto.
- Directorio espacial: contrato de directorio global en cadena que rastrea los manifiestos DS, versiones y eventos de gobierno/rotación para la resolución y las auditorías.
- DSID: Identificador global único para un espacio de datos. Utilice para el espacio de nombres de todos los objetos y referencias.- Ancla: El compromiso criptográfico de un bloque/encabezado DS incluye en la cadena el nexo para la historia de DS en el libro mayor global.
- Kura: Stockage de bloques Iroha. Etendu aquí con almacenamiento de blobs, código de borrado y compromisos.
- WSV: Iroha Vista del estado mundial. Etendu aquí con versiones de segmentos de estado, instantáneas y códigos de borrado.
- IVM: Máquina virtual Iroha para la ejecución de contratos inteligentes (código de bytes Kotodama `.to`).
  - AIRE: Representación Algebraica Intermedia. Vue algebrique du calcul pour des preuves type STARK, decrivant l'execution comme des traces basees sur des champs avec contraintes de transición et de frontiere.Modelo de espacios de datos
- Identificar: `DataSpaceId (DSID)` identificar un DS y un espacio de nombres. Les DS puede haber instancias de dos granularidades:
  - Dominio-DS: `ds::domain::<domain_name>`: ejecución y alcance del dominio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>`: la ejecución y el alcance tienen una definición de acción única.
  Les dos formas coexistentes; Las transacciones pueden tocar más DSID de manera atómica.
- Cycle de vie du manifest: creación de DS, mises a jour (rotation de cles, changements de politique) y retrait registres dans Space Directory. Chaque artefacto DS por ranura hace referencia al hash du manifest le plus reciente.
- Clases: DS Públicas (participación abierta, DA pública) y DS Privadas (permiso, DA confidencial). Los híbridos políticos son posibles a través de las banderas de manifiesto.
- Politiques par DS: permisos ISI, parámetros DA `(k,m)`, chiffrement, retención, cuotas (parte min/max de tx par bloc), politique de preuve ZK/optimiste, frais.
- Gobernanza: la gobernanza de miembros DS y la rotación de validadores se definen por la sección de gobernanza del manifiesto (proposiciones en cadena, multifirma, o gobernanza externa ancree por transacciones nexus y atestados).Manifiestos de capacidades y UAID
- Cuentas universales: cada participante recupera un determinante de la UAID (`UniversalAccountId` en `crates/iroha_data_model/src/nexus/manifest.rs`) que cubre todos los espacios de datos. Les manifests de capacites (`AssetPermissionManifest`) lient un UAID a un dataspace specifique, des epochs d'activation/expiration and una liste ordonnee de regles enable/deny `ManifestEntry` qui bornent `dataspace`, `program_id`, `method`, `asset` y las funciones opcionales AMX. Les regles niegan los toujours gagnent; El evaluador emet `ManifestVerdict::Denied` con una razón de auditoría o una subvención `Allowed` con los metadatos de asignación correspondiente.
- Asignaciones: cada entrada permite el transporte de cubos determinados `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) más un opcional `max_amount`. Los hosts y el SDK contienen la carga útil Norito, donde la aplicación permanece idéntica entre el hardware y el SDK.
- Telemetría de auditoría: Directorio espacial difuso `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) cada vez que se manifiesta un cambio de estado. La nueva superficie `SpaceDirectoryEventFilter` permite aux abonnes Torii/data-event de monitorer les mises a jour de manifest UAID, les revocaciones y les decisiones deny-wins sans plumbing sur mesure.Para las opciones operativas de un extremo a otro, las notas de migración del SDK y las listas de verificación de publicación del manifiesto, consulte esta sección con la Guía universal de cuentas (`docs/source/universal_accounts_guide.md`). Gardez les dos documentos se alinean cuando la política o los instrumentos de la UAID cambian.

Arquitectura de alto nivel
1) Sofá de composición global (Cadena Nexus)
- Mantenga un orden canónica única de bloques Nexus de 1 segundo que finaliza las transacciones atómicas que cubren uno o más espacios de datos (DS). Cada compromiso de transacción se reunió con un día en el estado mundial unificado global (vector de raíces por DS).
- Contient des metadonnees minimales plus des preuves/QC agreges pour surer la composabilite, la finalite et la deteccion de fraude (DSID touches, root d'etat par DS avant/apres, engagements DA, preuves de validite par DS, et le certificado de quorum DS avec ML-DSA-87). Aucune donnee privee n'est incluse.
- Consenso: comité BFT global pipeline de taille 22 (3f+1 con f=7), selección de un pool de jusqu'a ~200k validadores potenciales a través de un mecanismo VRF/stake por épocas. El comité de nexo secuencia las transacciones y finaliza el bloque en 1 segundo.2) Espacio de datos Couche (público/privado)
- Ejecute los fragmentos por DS des transacciones globales, cumpla un día con el WSV local del DS y produzca artefactos de validación por bloque (preuves por DS agregados y compromisos DA) que se remonten en el bloque Nexus de 1 segundo.
- Les Private DS chiffrent les donnees au repos et en transit entre validadores autorizados; Seuls les engagements et preuves de validite PQ quittent le DS.
- Les Public DS exportan los cuerpos completos de donnees (a través de DA) y las preuves de validite PQ.3) Transacciones atómicas entre el espacio de datos (AMX)
- Modelo: cada usuario de transacción puede tocar más DS (p. ej., dominio DS y un o plusieurs active DS). Elle est commit atomiquement dans un bloque Nexus único o elle avorte; aucun efecto partiel.
- Prepare-Commit en 1s: para cada transacción candidata, los toques de DS se ejecutan en paralelo con la instantánea del meme (raíces de DS en el debut de la ranura) y producen las preuves de validite PQ par DS (FASTPQ-ISI) y des engagements DA. El comité de nexo confirma la transacción solo si todos los preuves DS requieren verificación y si los certificados DA llegan a tiempo (objetivo <= 300 ms); Sinon la transacción está replanificada para la ranura siguiente.
- Coherencia: les ensembles listening/ecriture sont declara; la detección de conflictos en lugar de comprometerse con las raíces de debut de slot. La ejecución optimista sin bloqueos de DS evita los bloqueos globales; La atomicidad está impuesta por la regla de compromiso de nexus (todo o nada entre DS).
- Confidencialidad: los DS privados exportan únicamente des preuves/engagements y se encuentran en las raíces DS pre/post. Aucune donnee privee brute ne quitte le DS.4) Disponibilite des donnees (DA) con código de eliminación
- Kura almacena los cuerpos de bloques y las instantáneas WSV como los blobs, un código de borrado. Las manchas públicas son mucho más duras; Los blobs privados son existencias exclusivas de los validadores de DS privados, con los trozos de chiffres.
- Los compromisos DA se registran a la vez en los artefactos DS y en los bloques Nexus, lo que permite garantías de muestreo y recuperación sin contenido privado.

Estructura de bloque y compromiso
- Artefacto de espacio de datos anterior (por ranura de 1, por DS)
  - Campeones: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Les Private-DS exportador de artefactos sin cuerpo de donantes; Les Public DS permiten la recuperación a través de DA.

- Bloque Nexus (cadencia 1s)
  - Campeones: block_number, parent_hash, slot_time, tx_list (transacciones atómicas cross-DS con toques DSID), ds_artifacts[], nexus_qc.
  - Función: finalizar todas las transacciones atómicas sin los artefactos DS requeridos verificables; Conocí un jour le vecteur deroots DS du world state global en une etapa.Consenso y programación
- Cadena de consenso Nexus: BFT global pipeline (classe Sumeragi) avec un comite de 22 noeuds (3f+1 avec f=7) visant des blocs de 1s et une finalite 1s. Los miembros del comité son seleccionados por épocas a través de VRF/participación parmi ~200k candidatos; la rotación mantiene la descentralización y la resistencia a la censura.
- Consensus Data Space: cada DS ejecuta su propio BFT entre sus validadores para producir artefactos por ranura (preuves, engagements DA, DS QC). Los comités lane-relay son dimensiones de `3f+1` al utilizar el parámetro `fault_tolerance` del espacio de datos y son cambiables de forma determinada por época después del grupo de validadores del espacio de datos en uso de la semilla VRF situada en `(dataspace_id, lane_id)`. Los Private DS no tienen permiso; les Public DS permettent la liveness ouverte sous politiques anti-Sybil. El comité del nexo global descansa en el cambio.
- Programación de transacciones: los usuarios que realizan transacciones atómicas declaran los toques DSID y los conjuntos de lectura/escritura. El DS se ejecuta en paralelo en la ranura; El comité nexus incluye la transacción en el bloque 1 si todos los artefactos DS verifican y si los certificados DA son a la hora (<= 300 ms).- Aislamiento de rendimiento: cada DS a des mempools y una ejecución independiente. Las cuotas de DS nacidas combinadas de transacciones que tocan un DS pueden comprometerse por bloque para evitar el bloqueo del encabezado de línea y proteger la latencia de Private DS.

Modelo de donnees y espacio de nombres
- Los ID califican por DS: todas las entidades (dominios, cuentas, activos, roles) son calificados por `dsid`. Ejemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias globales: una referencia global es una tupla `(dsid, object_id, version_hint)` y puede colocarse en la cadena en el sofá nexus o en los descriptores AMX para un uso cross-DS.
- Serialización Norito: todos los mensajes cross-DS (descriptores AMX, anteriores) utilizan los códecs Norito. Pas de serde en producción.Contratos inteligentes y extensiones IVM
- Contexto de ejecución: agregar `dsid` al contexto de ejecución IVM. Los contratos Kotodama se ejecutan constantemente en un espacio de datos específico.
- Primitivas atómicas cruzadas-DS:
  - `amx_begin()` / `amx_commit()` delimita una transacción atómica multi-DS en el host IVM.
  - `amx_touch(dsid, key)` declara una intención de lectura/escritura para la detección de conflictos con las raíces de la instantánea de la ranura.
  - `verify_space_proof(dsid, proof, statement)` -> booleano
  - `use_asset_handle(handle, op, amount)` -> resultado (permiso de operación solo si la política de autorización y si el manejo es válido)
- Asset handles et frais:
  - Las operaciones activas están autorizadas por los políticos ISI/roles du DS; les frais sont payes en token de gas du DS. Las opciones de tokens de capacidad y las políticas más riquezas (aprobadores múltiples, límites de tasas, geocercas) pueden aumentar más tarde sin cambiar el modelo atómico.
- Determinismo: todos los nouvelles syscalls sont pures et deterministes donnees les entrees et les ensembles lectura/escritura AMX declara. Pas d'effets caches de temps ou d'environnement.Preuves de validite post-quantiques (ISI generaliza)
- FASTPQ-ISI (PQ, sin configuración confiable): un argumento basado en hash que generaliza la transferencia de diseño a todas las familias ISI y mira un segundo previo para lotes de 20k en la clase de hardware GPU.
  - Perfil operativo:
    - Los nuevos componentes de producción se construyen mediante `fastpq_prover::Prover::canonical`, que inicializan el backend de producción; Le Mock determina a ete retirarse. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) e `irohad --fastpq-execution-mode` permiten a los operadores de figurar la ejecución CPU/GPU de manera determinante así como el gancho del observador registra las triples demandas/resolución/backend para las auditorías de flotte. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetización:
  - KV-Update AIR: actualiza WSV como un tipo de clave-valor de mapa, conectado a través de Poseidon2-SMT. Cada ISI se considera un pequeño conjunto de líneas de lectura, verificación y escritura en las celdas (cuentas, actividades, roles, dominios, metadatos, suministro).
  - Contraintes gatees par opcode: una sola tabla AIR con columnas selecteurs imponen des regles par ISI (conservación, ordenadores monotónicos, permisos, comprobaciones de rango, mises a jour de metadata bornees).- Argumentos de búsqueda: tablas transparentes involucradas por hash para permisos/roles, precisiones de acciones y parámetros políticos para evitar las restricciones bit a bit.
- Compromisos y mises del día de estado:
  - Preuve SMT agregee: toutes les cles touchees (pre/post) sont prouvees contre `old_root`/`new_root` utilizando una frontera comprimida con hermanos dedupes.
  - Invariantes: los invariantes globaux (p. ej., suministro total par actif) se imponen vía egalite de multiensembles entre lignes d'effet et compteurs suivis.
- Sistema de prevención:
  - Compromisos estilo polinómico FRI (DEEP-FRI) con forte arite (8/16) y explosión 8-16; hashes Poseidon2; transcripción Fiat-Shamir avec SHA-2/3.
  - Opción de recursión: agregación local recursiva DS para comprimir microlotes en una ranura previa si es necesario.
- Portée et exemples couverts:
  - Actifs: transferir, acuñar, quemar, registrar/anular el registro de definiciones de activos, establecer precisión (borne), establecer metadatos.
  - Cuentas/Dominios: crear/eliminar, establecer clave/umbral, agregar/eliminar firmantes (estado único; las comprobaciones de firmas son attestadas por los validadores DS, no se prueban en el AIR).
  - Roles/Permisos (ISI): otorgar/revocar roles y permisos; impone a través de tablas de búsqueda y controles de política monótona.- Contratos/AMX: las marcas comienzan/confirman AMX, capacidad de acuñar/revocar si está activa; Prouves comme transiciones de estado y compteurs de politique.
- Controles fuera del AIRE para preservar la latencia:
  - Las firmas y criptografías antiguas (p. ej., firmas utilizadas por ML-DSA) son verificadas por los validadores DS y atestados en el DS QC; la preuve de validite couvre seulement la coherencia de estado et la conformité de politique. Cela garde des preuves PQ et rapides.
- Datos de rendimiento (ilustrativo, CPU 32 núcleos + una GPU moderna):
  - Mezclas de 20k ISI con pulsación de tecla pequeña (<=8 claves/ISI): ~0,4-0,9 s de anticipación, ~150-450 KB de anticipación, ~5-15 ms de verificación.
  - ISI plus lourdes (plus de cles/contraintes riches): microlote (p. ej., 10x2k) + recursión para garder <1 s par slot.
- Configuración del manifiesto DS:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (firmas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (por defecto; las alternativas deben declararse explícitamente)
- Opciones alternativas:
  - Los complejos/personalizados de ISI pueden utilizar un STARK general (`zk.policy = "stark_fri_general"`) con un precio diferente y finalizado 1 s mediante atestación QC + corte sobre preuves invalides.
  - Las opciones que no son PQ (p. ej., Plonk avec KZG) exigen una configuración confiable y no son más compatibles con la compilación por defecto.Introducción AIRE (vierta Nexus)
- Seguimiento de ejecución: matrice avec largeur (colonnes de registres) et longueur (etapes). Chaque ligne est una etapa lógica del tratamiento ISI; les colonnes contiennent valeurs pre/post, selecteurs et flags.
- Contraindicaciones:
  - Contraintes de transición: imponentes des relaciones de línea a línea (p. ej., post_balance = pre_balance - importe para una línea de débito cuando `sel_transfer = 1`).
  - Contraintes de frontiere: cliente I/O público (old_root/new_root, compteurs) a la premiere/última línea.
  - Búsquedas/permutaciones: asegurar la unidad y la legalidad de múltiples conjuntos con tablas activas (permisos, parámetros activos) sin circuitos de bits.
- Compromiso y verificación:
  - Probar activar las trazas mediante codificaciones hash y construir polinomos de factible grado válidos si las restricciones actuales.
  - Le verifier verifie le faible degre via FRI (basado en hash, post-quantique) con quelques ouvertures Merkle; le cout est logarithmique en etapas.
- Ejemplo (Transferencia): los registros incluyen pre_balance, importe, post_balance, nonce y selectores. Les contraintes imponent non-negativité/range, conservation et monotonicite de nonce, tandis qu'une multi-preuve SMT agregee lie les feuilles pre/post aux root old/new.Evolución ABI y llamadas al sistema (ABI v1)
- Syscalls a ajouter (nombres ilustrativos):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos de puntero-ABI a ajouter:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Mises a jour requiere:
  - Ajouter a `ivm::syscalls::abi_syscall_list()` (garder l'ordre), puerta par politique.
  - Mapper les numeros inconnus a `VMError::UnknownSyscall` dans les hosts.
  - Incluye pruebas diarias: lista de llamadas al sistema golden, hash ABI, ID de tipo de puntero golden, pruebas de políticas.
  - Documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modelo de confidencialidad
- Contención de donantes privados: el cuerpo de transacción, las diferencias de estado y las instantáneas WSV de los DS privados no abandonan el conjunto de validadores privados.
- Exposición pública: seuls les headers, engagements DA et preuves de validite PQ sont exportes.
- Preuves ZK optionnelles: les Private DS pueden producir preuves ZK (p. ej., solde suffisant, politique satisfaite) permettant des action cross-DS sans juer l'etat interna.
- Control de acceso: la autorización está impuesta por las políticas ISI/roles dans le DS. Los tokens de capacidad son opcionales y pueden aumentar más tarde.Aislamiento de rendimiento y QoS
- Consenso, mempools y stockage separes par DS.
- Cuotas de programación de nexus par DS para soportar el tiempo de inclusión de anclajes y evitar el bloqueo de cabecera.
- Presupuestos de recursos de contratos por DS (cómputo/memoria/IO), impuestos por el host IVM. La contención del DS público no puede consumir los presupuestos del DS privado.
- Las llamadas asíncronas cruzadas de DS evitan las largas atentes sincronizadas en la ejecución privada de DS.

Disponibilite des donnees et design de stockage
1) Código de borrado
- Utilice el sistema Reed-Solomon (p. ej., GF(2^16)) para borrar el código en el nivel de bloques Kura y instantáneas WSV: parámetros `(k, m)` con fragmentos `n = k + m`.
- Parámetros por defecto (propuesto, DS público): `k=32, m=16` (n=48), permitiendo la recuperación solo con 16 fragmentos perdidos con ~1.5x de expansión. Para DS privado: `k=16, m=8` (n=24) con permiso del conjunto. Los dos son configurables por DS Manifest.
- Blobs públicos: los fragmentos se distribuyen a través de nombres de datos/validadores con comprobaciones de disponibilidad por muestreo. Les engagements DA dans les headers permiten la verificación por parte de clientes ligeros.
- Blobs privados: fragmentos chiffres y distribuidos exclusivamente entre validadores privados de DS (o custodios designados). La chaine globale ne porte que des engagements DA (sans emplacement de shard nicles).2) Compromisos y muestreo
- Para cada blob: calcule une racine Merkle sur les shards et l'inclure dans `*_da_commitment`. Rester PQ y eviten los compromisos en una curva elíptica.
- DA Attesters: attesters regionaux echantillonnes par VRF (p. ej., 64 par región) emiten un certificado ML-DSA-87 attestant un sampling reussi des shards. Objetivo de latencia de atestación DA <=300 ms. El comité nexus valida los certificados en lugar de tirar los fragmentos.

3) Integración Kura
- Les blocs stockent les corps de transaction comme blobs un código de borramiento con compromisos Merkle.
- Los encabezados presagian los compromisos de blob; Los cuerpos son recuperables vía le reseau DA pour public DS y vía des canaux prives pour private DS.

4) Integración WSV
- Instantáneas WSV: periódicamente, punto de control del estado DS en fragmentos de instantáneas y códigos eliminados con compromisos registrados en los encabezados. Entre las instantáneas y los registros de cambios se mantienen. Las instantáneas públicas son fragmentos grandes; Las instantáneas privadas se guardan en los validadores privados.
- Acceso porteador de preuves: les contrats peuvent fournir (ou demander) des preuves d'etat (Merkle/Verkle) ancrees par des engagements de snapshot. Les Private DS puede proporcionar certificaciones de conocimiento cero en lugar de previos brutos.5) Retención y poda
- Paso de poda para DS público: conserva todos los cuerpos Kura y instantáneas WSV a través de DA (escalabilidad horizontal). Los DS privados pueden definir una retención interna, pero los compromisos exportados permanecen inmutables. El sofá nexus conserva todos los bloques Nexus y los compromisos de artefactos DS.

Reseau et roles de noeuds
- Validadores globales: participantes en el nexo de consenso, válidos en los bloques Nexus y en los artefactos DS, efectivos en las comprobaciones DA para el DS público.
- Validadores de espacio de datos: ejecuta el consenso DS, ejecuta los contratos, gestiona Kura/WSV local, gestiona DA para el DS.
- Noeuds DA (opcional): stockent/publient des blobs publics, facilitando el muestreo. Para DS privado, los datos DA se ubican conjuntamente con los validadores o custodios de confianza.Sistema de mejoras y consideraciones.
- Secuenciación de desacoplamiento/mempool: adopta un mempool DAG (p. ej., estilo Narwhal) que alimenta un canal BFT a la sofá nexo para reducir la latencia y mejorar el rendimiento sin cambiar el modelo lógico.
- Cuotas DS y equidad: cuotas por DS por bloque y límites de peso para evitar el bloqueo del encabezado de línea y asegurar una latencia previsible para DS privado.
- Atestación DS (PQ): los certificados de quórum DS utilizan ML-DSA-87 (clase Dilithium5) por defecto. Es post-cuántico y más voluminoso que las firmas EC más aceptables para un control de calidad por ranura. Los DS pueden optar explícitamente por ML-DSA-65/44 (más pequeños) o las firmas CE si se declaran en el Manifiesto DS; les Public DS sont fortement anima a un jardinero ML-DSA-87.
- Certificadores DA: para DS públicos, utilizan los certificados regionales echantillonnes par VRF que emiten certificados DA. El comité nexus valida los certificados en lugar del muestreo brut des shards; les Private DS gardent les attestations DA internes.
- Recursion et preuves par epoch: opción para agregar más microlotes en un DS en une preuve recursive par slot/epoch para guardar la cola de preuve y los tiempos de verificación estables bajo carga elevee.- Escalado de carriles (si es necesario): si un comité global único desarrolla un goulot, introduce K carriles de secuenciación paralelos con una fusión determinista. Esto preserva un orden global único en escala horizontal.
- Determinación de la aceleración: establezca las puertas SIMD/CUDA del kernel para funciones de hash/FFT con una CPU de respaldo exacta en bits para preservar el determinismo entre hardware.
- Seuils d'activation des lanes (proposition): activer 2-4 lanes si (a) la finalite p95 depasse 1.2 s durante >3 minutos consecutivos, ou (b) l'ocupation par block depasse 85% durante >5 minutos, o (c) le debit entrantet de tx requerrait >1.2x la capacidad de bloque a des niveaux soutenus. Los carriles agrupan las transacciones de manera determinada por el hash de DSID y se fusionan en el bloque nexus.

Frais et economie (valores iniciales)
- Unite de gas: token de gas par DS avec compute/IO metes; les frais sont payes en actif de gas natif du DS. La conversión entre DS es una preocupación aplicable.
- Prioridad de inclusión: round-robin entre DS con cuotas par DS para preservar la equidad y los SLO 1s; dans un DS, la mise en avant par fee peut departager.
- Futur: una marcha global de tarifas o políticas minimizantes MEV puede explorar sin cambiar la atomicidad ni el diseño de preuves PQ.Flujo de trabajo entre espacios de datos (ejemplo)
1) Un usuario soumet una transacción AMX touchant un public DS P et un private DS S: reemplace el activo X de S vers le beneficiario B dont le compte est dans P.
2) Dans le slot, P et S ejecutan leur fragment contre le snapshot du slot. S verifie l'autorisation et la disponibilite, met a jour son etat interna, et produit une preuve de validite PQ et un engagement DA (aucune donnee privee ne fuite). P prepare la mise a jour d'etat correspondante (p. ej., mint/burn/locking dans P selon la politique) et sa preuve.
3) Le comité nexus verifie les deux preuves DS et les certificats DA; Si las dos personas se verifican en la ranura, la transacción se compromete atomicamente en el bloque Nexus de 1s, mettant a jour les dos raíces DS en el vector del estado mundial.
4) Si une preuve ou un certificado DA es manquant/invalide, la transacción se cancelará (aucun effet), y el cliente podrá recuperar el dinero para la ranura siguiente. Aucune donnee privee ne quitte S a aucun moment.- Consideraciones de seguridad
- Ejecución determinista: las llamadas al sistema IVM permanecen deterministas; Los resultados entre DS están dictados por el compromiso y la finalización de AMX, por el reloj o el resultado de sincronización.
- Control de acceso: los permisos ISI en DS privados restringen los que pueden realizar transacciones y operaciones autorizadas. Los tokens de capacidad codifican los derechos para su uso entre DS.
- Confidencialidad: intercambio de extremo a extremo para donnees private-DS, fragmentos de un código de borrado exclusivo de los miembros autorizados, opciones ZK previas para atestados externos.
- Resistencia DoS: el aislamiento del nivel de mempool/consenso/stockage empeche la congestión pública que afecta la progresión de los DS privados.

Cambios de componentes Iroha
- iroha_data_model: introducción `DataSpaceId`, ID calificados por DS, descriptores AMX (conjuntos de lectura/ecritura), tipos de preuve/engagement DA. Único de serialización Norito.
- ivm: agrega llamadas al sistema y tipos de puntero ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) y preuves DA; Mettre a jour les tests/docs ABI según la política v1.