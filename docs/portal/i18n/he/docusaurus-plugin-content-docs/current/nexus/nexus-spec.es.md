---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-spec.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-spec
כותרת: Especificacion tecnica de Sora Nexus
תיאור: Espejo completo de `docs/source/nexus.md`, que cubre la arquitectura y las restricciones de diseno para el ledger Iroha 3 (Sora Nexus).
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/nexus.md`. Manten ambas copias alineadas hasta que el backlog de traduccion llegue al portal.
:::

#! Iroha 3 - Sora Nexus ספר חשבונות: Especificacion tecnica de diseno

Este documento propone la arquitectura del Sora Nexus Ledger para Iroha 3, evolucionando Iroha 2 hacia un Ledger global unico y logicamente unificado organizado alrededor of Data Spaces (DS). Data Spaces proveen dominios fuertes de privacidad ("מרחבי נתונים פרטיים") y participacion abierta ("מרחבי נתונים ציבוריים"). El diseno preserva la composabilidad and traves del Book global mientras asegura aislamiento estricto y confidencialidad para los datas de private-DS, e הצג escalado de disponibilidad de data via codificacion de borrado en Kura (אחסון בלוק) y WSV (World State View).

El mismo repositorio compila tanto Iroha 2 (redes autoalojadas) como Iroha 3 (SORA Nexus). La ejecucion esta impulsada por la Iroha Machine Virtual (IVM) compartida y la toolchain de Kotodama, por lo que los contratos y artefactos de bytecode permanecen portables entre led autodosy global despliegue Nexus.

אובייקטיבוס
- חשבונאות עולמית של חשבונות חשבונות עולמיים של שיתוף פעולה עם מרחבי נתונים.
- Data Spaces privados para operation con permisos (p. ej., CBDC), con datas que nunca salen del DS privado.
- Data Spaces publicos con participacion abierta, acceso in permisos estilo Ethereum.
- חומרי חיבור אינטנסיביים למרחבי נתונים, ניתנים להרשאה מפורשת עבור גישה לפעילות פרטית-DS.
- Aislamiento de rendimiento para que la actividad publica no degrade las transacciones internas de private-DS.
- Disponibilidad de datas a escala: Kura y WSV con codificacion de borrado para soportar datas efectivamente ilimitados manteniendo los datas de private-DS privados.

ללא מטרות (שלב ראשוני)
- Definir economia de token o incentivos de validadores; las politicas de scheduling y staking בן enchufables.
- Introducir una nueva version de ABI; los cambios apuntan a ABI v1 with extensions explicitas de syscalls y pointer-ABI segun la politica de IVM.טרמינולוגיה
- Nexus ספר חשבונות: הפורמט ההגיוני הגלובלי של חשבונות חשבונות המחבר את חבילות הנתונים של מרחב הנתונים (DS) בהיסטוריה מסודרת ובלתי מתפשרת.
- מרחב נתונים (DS): Dominio acotado de ejecucion y almacenamiento con sus propios validadores, gobernanza, clase de privacidad, politica de DA, cuotas y politica de tarifas. קיימות דוס קלאסות: DS ציבורי y DS פרטי.
- מרחב נתונים פרטי: תוקף הרשאות ושליטה בגישה; los datas de transaccion y estado nunca salen del DS. סולו זה הסכם פשרות/metadatos globalmente.
- מרחב נתונים ציבוריים: Participacion sin permisos; los datos completos y el estado son publicos.
- Manifest Space (DS Manifest): Manifest codificado con Norito que declara parametros DS (validadores/llaves QC, clase de privacidad, politica ISI, parametros DA, retencion, cuotas, politica ZK, tarifas). El hash del manifest se ancla en la cadena nexus. Salvo que se anule, los certificados de quorum DS usan ML-DSA-87 (Case Dilithium5) como esquema de firma post-cuantico por defecto.
- ספריית חלל: התנגדות למדריך הגלובלי על-השרשרת להצגת DS, גרסאות ואירועי ניהול/רוטציה לפתרון אודיטוריות.
- DSID: זיהוי גלובלי יחיד עבור מרחב נתונים. ראה ארה"ב עבור ריווח שמות של נקודות עניין ונקודות.
- עוגן: Compromiso criptografico de un bloque/header DS כלול ב-Cadena nexus para vincular la historia DS al ledger global.
- Kura: Almacenamiento de bloques de Iroha. Se extiende aqui con almacenamiento de blobs codificados con borrado y compromisos.
- WSV: Iroha World State View. Se extiende aqui con segmentos de estado versionados, con snapshots, y codificados con borrado.
- IVM: Iroha מכונה וירטואלית עבור פליטת אינטליגנציה נגדית (קוד בייט Kotodama `.to`).
  - AIR: ייצוג ביניים אלגברי. ויסטה אלגברית מחשבים על STARK, תיאור של יציאתו כמו טרזאס באסדה ו-campos con restricciones de transicion y frontera.דגם מרחבי נתונים
- זיהוי: `DataSpaceId (DSID)` זיהוי ו-DS y da מרווח שמות של מטלה. Los DS pueden instanciarse en dos granularidades:
  - דומיין-DS: `ds::domain::<domain_name>` - הוצאת דומיין.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - eecucion y estado acotados a una definicion de activo unica.
  אמבס פורמאס מתקיים במקביל; las transacciones pueden tocar כפולות DSID de forma atomica.
- Ciclo de vida del manifest: la creacion de DS, actualizaciones (rotacion de llaves, cambios de politica) y retiro se registeran in Space Directory. Cada artefacto DS por slot referencia el hash del manifest mas reciente.
- שיעורים: Public DS (participacion abierta, DA publica) y Private DS (permisionado, DA confidencial). Politicas hibridas son posibles via flags del manifest.
- Politicas por DS: permisos ISI, parametros DA `(k,m)`, cifrado, retencion, cuotas (min/max participacion de tx por bloque), politica de pruebas ZK/optimistas, tarifas.
- Gobernanza: membresia DS y rotacion de validadores definidas por la seccion de gobernanza del manifest (propuestas on-chain, multisig o gobernanza externa anclada por transacciones nexus y atestaciones).

מניפסטים של היכולות של UAID
- Cuentas universales: קביעת UAID ו-UAID deterministico (`UniversalAccountId` en `crates/iroha_data_model/src/nexus/manifest.rs`) עבור מרחבי נתונים. Los manifestes de capacidades (`AssetPermissionManifest`) וינקולן ו-UAID וספציפיות של מרחב נתונים, אפוקסות הפעלה/תפוגה ורשימה רשמית לאפשר/הכחשה של `ManifestEntry` que acotan Sumeragi000050,Sumeragi0000500, Sumeragi000050, `method`, `asset` y תפקידים AMX opcionales. Las reglas להכחיש siempre ganan; el evaluador emite `ManifestVerdict::Denied` con una razon de auditoria o un grant `Allowed` con la metadata de קצבה coincidente.
- קצבאות: cada entrada allow lleva buckets deterministas `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) mas un `max_amount` אופציונלי. מארחים ו-SDK צורכים את מטען המשאבים של Norito, כמו גם יישום זהה לחומרה ו-SDK.
- Telemetria de Auditoria: מדריך החלל emit `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) cuando un manifest cambia de estado. La nueva superficie `SpaceDirectoryEventFilter` מתיר רשומות של Torii/data-event monitorear אקטואליזציה של UAID, ביטול והחלטות הכחיש-מנצח בהתאמה אישית של אינסטלציה.

עבור הוכחות לפעולה מקצה לקצה, הוראות המעבר ל-SDK ורשימות הביקורת לפרסום מניפסטים, espejea esta seccion con la Universal Account Guide (`docs/source/universal_accounts_guide.md`). Manten ambos documentos alineados cuando cambien la politica o las herramientas UAID.Arquitectura de alto nivel
1) Capa de composicion global (שרשרת Nexus)
- Mantiene un orden canonico unico de bloques Nexus de 1 סיגונדו que finalizan transacciones atomicas que abarcan uno o mas Data Spaces (DS). Cada transaccion confirmada actualiza el state global unificado (vector de roots por DS).
- Contiene metadatos minimos mas pruebas/QC agregados para garantizar composabilidad, finalizacion y deteccion de fraude (DSIDs tocados, roots de estado por DS antes/despues, compromisos DA, pruebas de validez por DS, y el certificado de quorum DSSA-7. אין זה כולל פרטי נתונים.
- קונצנזו: comite BFT global con pipeline de tamano 22 (3f+1 con f=7), seleccionado de un pool de hasta ~200k validadores potenciales via un mecanismo VRF/stake por epocas. El comite nexus ordena transacciones y finaliza el bloque en 1s.

2) Capa de Data Space (ציבורי/פרטי)
- Ejecuta fragmentos por DS de transacciones globales, actualiza el WSV local del DS y produce artefactos de validez por bloque (pruebas agregadas por DS y compromisos DA) que se agregan en el bloque Nexus de 1 segundo.
- פרטי DS cifran datos en reposo y en transito entre validadores autorizados; סולו compromisos y pruebas de validez PQ salen del DS.
- Public DS exportan cuerpos completos de datas (via DA) y pruebas de validez PQ.

3) Transacciones atomicas cross-data-space (AMX)
- דגם: cada transaccion de usuario puede tocar multiples DS (עמוד ej., domain DS y uno o mas asset DS). Se confirma de forma atomica en un solo bloque Nexus o se aborta; ללא שחת efectos parciales.
- Prepare-Commit dentro de 1s: para cada transaccion candidata, los DS tocados ejecutan en paralelo contra el mismo snapshot (roots DS al inicio del slot) y produce pruebas de validez PQ por DS (FASTPQ-ISI) y compromisos DA. El comite nexus confirma la transaccion solo si todas las pruebas DS requeridas verifican y los certificados DA llegan a tiempo (objetivo <=300 ms); de lo contrario, la transaccion se reprograma para el suuiente slot.
- Consistencia: los conjuntos de lectura/escritura se declaran; la deteccion de conflictos ocurre al commit contra los roots de inicio de slot. La ejecucion optimista sin locks por DS evita bloqueos globales; la atomicidad se impone por la regla de commit nexus (todo o nada entre DS).
- פרטיות: DS פרטי ייצוא סולו pruebas/compromisos ligados ושורשים DS לפני/פוסט. אין נתוני מכירה פרטית cruda del DS.

4) Disponibilidad de datas (DA) con codificacion de borrado
- Kura almacena cuerpos de bloques y snapshots WSV como blobs codificados con borrado. Los blobs publicos se fragmentan ampliamente; los blobs privados se almacenan solo dentro de validadores Private-DS, con chunks cifrados.
- Los compromisos DA registran tanto en artefactos DS como en bloques Nexus, habilitando muestreo y garantias de recuperacion sin revelar contenido privado.Estructura de bloques y commit
- Artefacto de prueba de Data Space (פור חריץ de 1s, por DS)
  - Campos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - פרטי-DS exporta artefactos sin cuerpos de datos; הציבור DS permite recuperacion de cuerpos via DA.

- Bloque Nexus (קדנציה דה 1s)
  - Campos: block_number, parent_hash, slot_time, tx_list (transacciones atomicas cross-DS con DSIDs tocados), ds_artifacts[], nexus_qc.
  - פונקציה: finaliza todas las transacciones atomicas cuyos artefactos DS requeridos verifican; אקטואליזציה של וקטור שורשים DS של מדינת העולם העולמית ב-un solo paso.

קונצנזו ותזמון
- קונסנסו של Nexus שרשרת: BFT Global Con pipeline (Case Sumeragi) עם 22 נקודות (3f+1 con f=7) אפונטנדו ל-bloques de 1s y finalizacion de 1s. Los miembros del comite seleccionan por epocas באמצעות VRF/stake entre ~200k מועמדים; la rotacion mantiene descentralizacion y resistencia a censura.
- קונסנסו של מרחב נתונים: DS ejecuta su propio BFT entre sus validadores para producir artefactos por slot (pruebas, compromises DA, DS QC). Los comites lane-relay se dimensionan en `3f+1` usando la configuracion `fault_tolerance` del dataspace y se muestrean de forma determinista por epoca desde el pool de validadores del dataspace usando el seed deliga000X I007NI epoca I007NI. פרטי DS בן permisionados; הציבור DS permiten liveness abierta sujeta a politicas anti-Sybil. El comite nexus global no cambia.
- תזמון פעולות טרנספר: los usuarios envian transacciones atomicas declarando DSIDs tocados y conjuntos de lectura/escritura. Los DS ejecutan en paralelo dentro del slot; el comite nexus incluye la transaccion en el bloque de 1s si todos los artefactos DS verifican y los certificados DA son puntuales (<=300 ms).
- Aislamiento de rendimiento: cada DS tiene mempools y ejecucion independientes. Las cuotas por DS limitan cuantas transacciones que tocan un DS se pueden confirmar por bloque para evitar head-of-line blocking y proteger la latencia de private DS.

מודלים של נתונים וריווח שמות
- IDs calificados por DS: todas las entidades (dominios, cuentas, activos, roles) se califican por `dsid`. דוגמה: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias Globals: una referencia global es una tupla `(dsid, object_id, version_hint)` y puede colocarse on-chain in la capa nexus o en descriptores AMX para uso cross-DS.
- Serializacion Norito: todos los mensajes cross-DS (תיאורי AMX, pruebas) ארה"ב Norito. No se usa serde en paths de produccion.Contratos Intelligentes y extensiones de IVM
- Contexto de ejecucion: agregar `dsid` al contexto de ejecucion de IVM. Los contratos Kotodama סימפרה ejecutan dentro de un Data Space especifico.
- Primitivas atomicas cross-DS:
  - `amx_begin()` / `amx_commit()` מגביל una transaccion atomica multi-DS en el host IVM.
  - `amx_touch(dsid, key)` הצהרת כוונה ללימודים/ספרות לגילוי קונפליקט נגד שורשי תמונה של חריץ.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> תוצאה (operacion permitida solo si la politica lo permite y el handle es valido)
- ידיות פעילים ותעריפים:
  - Las operaciones de activos se autorisan por las politicas ISI/rol del DS; las tarifas se pagan en el token de gas del DS. Los tokens de capacidad opcionales y politicas mas ricas (מרובה מאשרים, מגבלות תעריפים, גיאופנסינג) ראה את האגרה של ה-Adelante Sin Cambiar El Modelo Atomico.
- דטרמיניסמו: todas las nuevas syscalls son puras y deterministas dadas las entradas y los conjuntos de lectura/escritura AMX declarados. Sin efectos ocultos de tiempo o entorno.Pruebas de validez post-cuanticas (ISI generalizados)
- FASTPQ-ISI (PQ, הגדרה מהימנה): אין טיעונים מבוססי hash que generalize el diseno de transfer todas las familias.
  - פרפיל אופרטיבי:
    - Los nodos de produccion construyen el prover via `fastpq_prover::Prover::canonical`, que ahora siempre inicializa el backend de produccion; el mock determinista fue removido. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) y `irohad --fastpq-execution-mode` מאפשרים הפעלת מעבד/GPU של מעבד/GPU דה פורמה קבעה מיינטרס אל ה-observer hook registra triples solicitados/resueltos/backend para auditorias de flota. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/888.rs:8
- Aritmetizacion:
  - KV-Update AIR: WSV como un mapa key-value tipado comprometido via Poseidon2-SMT. Cada ISI זה להרחיב את חיבורי הקריאה, הבדיקה-כתוב הקודמים (קודים, פעילים, תפקידים, דומינוסים, מטא נתונים, אספקה).
  - הגבלות על קוד אופציה: יחידת טבלה של AIR עם עמודות בחירה להגדיר את ISI (שימור, תקינות מונוטוניות, הרשאות, בדיקות טווח, אקטואליזציה של מטא נתונים).
  - טיעוני חיפוש: טבלאות שקופות ל-hash para permisos/תפקידים, דיוק פעיל ופרמטרים של פוליטיקה evitan constraints bitwise pesadas.
- Compromisos y actualizaciones de estado:
  - Prueba SMT agregada: todas las claves tocadas (pre/post) se prueban contra `old_root`/`new_root` usando un frontier comprimido con brothers deduplicados.
  - Invariantes: invariantes globales (עמ' ej., supply total por activo) se imponen via igualdad de multiconjuntos entre filas de efecto y contadores rastreados.
- סיסטמה דה פרובה:
  - Compromisos polinomiales estilo FRI (DEEP-FRI) con alta aridad (8/16) y blow-up 8-16; hashes Poseidon2; תמלול פיאט-שמיר קון SHA-2/3.
  - Recursion אופציונלי: agregacion recursiva local a DS para comprimir micro-lotes a una prueba por slot si se necesita.
- Alcance y ejemplos cubiertos:
  - Activos: העברה, טביעה, צריבה, רישום/ביטול רישום הגדרות נכסים, הגדרת דיוק (acotado), הגדרת מטא נתונים.
  - Cuentas/Dominios: צור/הסר, הגדר מפתח/סף, הוספה/הסרה של חותמים (solo estado; las verificaciones de firma se atestan por validadores DS, no se prueban dentro del AIR).
  - תפקידים/הרשאות (ISI): הענק/ביטול תפקידים ואישורים; impuestos por tablas de lookup y checks de politica monotonic.
  - Contratos/AMX: מרקדורים מתחילים/מבצעים AMX, יכולת נחוש/ביטול סי esta habilitado; se prueban como transiciones de estado y contadores de politica.
- בודק את fuera del AIR לשמירה על זמן השהייה:- Firmas y criptografia pesada (עמ' ej., firmas ML-DSA de usuarios) se verifican por validadores DS y se atestan en el DS QC; la prueba de validez cubre solo consistencia de estado y cumplimiento de politicas. Esto mantiene pruebas PQ y rapidas.
- אובייקטיביות של חידושים (תמונות, מעבד עם 32 ליבות + יחידת GPU מודרנית):
  - 20k ISI mixtas with key-touch pequeno (<=8 claves/ISI): ~0.4-0.9 s de prueba, ~150-450 KB de prueba, ~5-15 ms de prueba.
  - ISI mas pesadas (mas claves/constraints ricas): מיקרו-לוטות (עמ' ej., 10x2k) + רקורסיה para mantener por slot <1 s.
- Configuracion de DS Manifest:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (מאומת של DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (por defecto; alternativas deben declararse explicitamente)
- נפילות:
  - ISI complejas/personalizadas pueden usar un STARK general (`zk.policy = "stark_fri_general"`) con prueba diferida y finalizacion de 1 s via atestacion QC + slashing and pruebas invalidas.
  - Opciones no PQ (p. ej., Plonk con KZG) דורש התקנה מהימנה y ya no se soportan en el build por defecto.

היכרות עם AIR (סעיף Nexus)
- Traza de ejecucion: matriz con ancho (columnas de registros) y longitud (pasos). Cada fila es un paso logico del processamiento ISI; las columnas contienen valores pre/post, selectores y flags.
- הגבלות:
  - Restricciones de transicion: imponen relaciones fila a fila (עמ' ej., post_balance = pre_balance - amount para una fila de debito cuando `sel_transfer = 1`).
  - Restricciones de frontera: vinculan E/S publica (ישן_שורש/חדש_שורש, contadores) a la primera/ultima fila.
  - חיפושים/תמורות: aseguran membresia e igualdades de multiconjuntos contra tablas comprometidas (permisos, parametros de activos) sin circuitos pesados ​​de bits.
- אימות פשרה:
  - El prover compromete trazas via codificaciones hash y construye polinomios de bajo grado que son validos si las restricciones se cumplen.
  - El verifier comprueba bajo grado via FRI (מבוסס חשיש, פוסט-קוואנטיקו) con pocas aperturas Merkle; el costo es logaritmico en los pasos.
- דוגמה (העברה): registros כוללות pre_balance, amount, post_balance, nonce y selectores. Las restricciones imponen no negativeidad/rango, conservacion y monotonicidad de nonce, mientras una multiprueba SMT agregada vincula hojas pre/post a los roots old/new.

Evolucion de ABI y syscalls (ABI v1)
- Syscalls agregar (nombres illustrativos):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- טיפוס פוינטר-ABI אגרגר:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- מבוקש בפועל:
  - Agregar a `ivm::syscalls::abi_syscall_list()` (mantener orden), gatear por politica.
  - Mapear numeros desconocidos a `VMError::UnknownSyscall` en hosts.
  - בדיקות אקטואליזר: רשימת syscall golden, ABI hash, זיהוי סוג מצביע goldens y מבחני מדיניות.
  - מסמכים: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.דגם פרטיות
- Contencion de datas privados: cuerpos de transaccion, diffs de estado y snapshots WSV de private DS nunca salen del subconjunto privado de validadores.
- Exposicion publica: כותרות סולו, פשרות DA y pruebas de validez PQ se exportan.
- Pruebas ZK opcionales: פרטי DS pueden producer pruebas ZK (p. ej., balance suficiente, politica cumplida) habilitando acciones cross-DS sin revelar estado interno.
- בקרת גישה: la autorisacion se impone por politicas ISI/rol dentro del DS. Los tokens de capacidad son opcionales y pueden introducirse mas adelante.

Aislamiento de rendimiento y QoS
- קונסנסו, mempools y almacenamiento separados por DS.
- קשרי תזמון ל-DS להגביל את זמן ההכללה של עוגנים וחסימת ראש קו.
- Presupuestos de recursos de contrato por DS (מחשב/זיכרון/IO), impuestos por el host IVM. La contencion en public DS no puede consumir presupuestos de private DS.
- Llamadas cross-DS asincronas evitan esperas sincronas largas dentro de ejecucion פרטי-DS.

Disponibilidad de datas y diseno de almacenamiento
1) Codificacion de borrado
- Usar Reed-Solomon sistematico (p. ej., GF(2^16)) para codificacion de borrado a nivel de blob de bloques Kura y snapshots WSV: parametros `(k, m)` con `n = k + m` shards.
- Parametros por defecto (propuestos, Public DS): `k=32, m=16` (n=48), permitiendo recuperacion de hasta 16 רסיסים perdidos עם ~1.5x הרחבה. עבור DS פרטי: `k=16, m=8` (n=24) dentro del conjunto permisionado. הגדרות אמבוס ל-DS Manifest.
- Blobs publicos: רסיסי חלוקת רסיסים a traves de muchos nodos DA/validadores con checks de disponibilidad por muestreo. Los compromisos DA וכותרות מותרות אימות ללקוחות קלים.
- Blobs privados: shards cifrados y distribuidos solo entre validadores private-DS (o custodios designados). La cadena global solo lleva compromisos DA (sin ubicaciones de shards ni llaves).

2) Compromisos y muestreo
- Para cada blob: calcular un Merkle root sobre shards e incluirlo en `*_da_commitment`. Mantener PQ evitando compromisos de curva eliptica.
- DA Attesters: attesters regionales muestredos por VRF (p. ej., 64 por region) emiten un certificado ML-DSA-87 atestando muestreo exitoso de shards. Objectivo de latencia de atestacion DA <=300 ms. El comite nexus valida certificados en lugar de extraer shards.

3) Integracion con Kura
- Los bloques almacenan cuerpos de transaccion como blobs codificados con borrado con compromisos Merkle.
- Los headers llevan compromisos de blob; los cuerpos se recuperan via la red DA para public DS y via canales privados para private DS.4) אינטגרציה עם WSV
- Snapshots WSV: periodicamente se hasce checkpoint del estado DS בתמונות מצב של נתחים קודים עם בורראדו עם פשרות רשומות בכותרות. הכנס תמונות מצב, ראה יומני שינוי. Los Photos Publicos se fragmentan ampliamente; לוס תמונות פרטיות קבועות דנטרו validadores privados.
- Acceso con pruebas: los contratos pueden proporcionar (o solicitar) pruebas de estado (Merkle/Verkle) ancladas por compromisos de snapshot. פרטי DS pueden suministrar atestaciones de conocimiento cero en lugar de pruebas crudas.

5) שמירה וגיזום
- חטא גיזום עבור DS ציבורי: retener todos los cuerpos Kura y snapshots WSV via DA (escalado horizontal). DS פרטי מגדיר שמירה פנימית, אבל לא ניתן לשנות את הפשרות. La capa nexus retiene todos los bloques Nexus y los compromisos de artefactos DS.

Red y roles de nodos
- Validadores globales: participan en el consenso nexus, validan bloques Nexus y artefactos DS, realizan checks DA para public DS.
- Validadores de Data Space: הוצאת קונצנזו DS, ביטול קונטרטוס, הדרכה של Kura/WSV מקומית, מנגינה DA עבור DS.
- Nodos DA (אופציונלי): almacenan/publican blobs publicos, facilitan muestreo. Para private DS, los nodos DA se co-ubican con validadores o custodios confiables.Mejoras y consideraciones a nivel de sistema
- Desacoplar secuenciacion/mempool: adoptar un mempool DAG (p. ej., estilo Narwhal) que alimente un BFT con pipeline en la capa nexus para bajar latencia y mejorar תפוקה ב-cambiar el modelo logico.
- Cuotas DS y הוגנות: cuotas por DS por bloque y caps de peso para evitar head-of-line blocking y asegurar latencia predecible para private DS.
- Atestacion DS (PQ): אישורי הקוורום הקוורום DS בשימוש ML-DSA-87 (Case Dilithium5) על ידי defecto. זה פוסט-cuantico y mas grande que firmas EC מקובל עם un QC por חריץ. DS pueden optar explicitamente por ML-DSA-65/44 (mas pequeno) o firmas EC si se declara en el DS Manifest; ראה מדריך ML-DSA-87 לציבור DS.
- אישורי DA: para public DS, usar attesters regionales muestreados por VRF que emiten certificados DA. El comite nexus valida certificados en lugar de muestreo de shards crudos; פרטי DS mantienen atestaciones DA internas.
- Recursion y pruebas por epoca: opcionalmente agregar varios micro-lotes dentro de un DS en una prueba recursiva por slot/epoca para mantener tamano de prueba y tiempo de verificacion estables bajo alta carga.
- Escalado de lanes (si se necesita): אין אחד עולמיים עולמיים ו-cuello de botella, היכרות עם K lanes de secuenciacion paralelas con un merge determinista. Esto preserva un orden global unico mientras escala horizontalmente.
- אצה דטרמיניסטית: מוכיח את ליבות SIMD/CUDA עם דגלים עבור hashing/FFT עם סיביות חילופין של מעבד חילופין בדיוק לשמירה על חומרה צולבת.
- Umbrales de activacion de lanes (propuesta): יש 2-4 נתיבים si (a) p95 de finalizacion עולים על 1.2 שניות ל->3 דקות עוקבות, o (ב) ל-ocupacion por bloque עולים על 85% ל->5 דקות, o (c) la tasa entranteria de la txa entrante de la txa rebloque. niveles sostenidos. Las lanes agrupan transacciones de forma determinista por hash de DSID y se mergean en el bloque nexus.

Tarifas y Economia (valores iniciales)
- Unidad de gas: token de gas por DS con compute/IO medido; las tarifas se pagan en el activo de gas nativo del DS. La conversion entre DS es responsabilidad de la aplicacion.
- Prioridad de inclusion: round-robin entre DS con cuotas por DS para preservar fairness y SLOs de 1s; dentro de un DS, el עמלה הצעות puede desempatar.
- Futuro: אתה יכול לחקור את המחירים העולמיים או הפוליטיקה למינימום של MEV.Flujo Cross-Data-Space (דוגמה)
1) Un usuario envia una transaccion AMX que toca public DS P y private DS S: mover activo X desde S a beneficiario B cuya cuenta esta en P.
2) Dentro del חריץ, P y S ejecutan su fragmento contra el snapshot del חריץ. S verifica autorizacion y disponibilidad, actualiza su estado interno y produce una prueba de validez PQ y compromiso DA (sin filtrar datas privados). P prepara la actualizacion de estado correspondiente (עמ' ej., מנטה/צריבה/נעילה en P segun politica) y su prueba.
3) El comite nexus verifica ambas pruebas DS y certificados DA; si ambas verifican dentro del slot, la transaccion se confirma atomicamente en el bloque Nexus de 1s, actualizando ambos roots DS en el vector de world state global.
4) Si alguna prueba o certificado DA falta o es invalido, la transaccion se aborta (sin efectos) y el cliente puede reenviar para el suuiente slot. Ningun dato privado sale de S en ningun paso.

- Consideraciones de seguridad
- Ejecucion determinista: las syscalls IVM permanecen deterministas; los resultados cross-DS los dictan AMX commit y finalizacion, no el reloj o el timing de red.
- Control de acceso: los permisos ISI en private DS restringen quien puede enviar transacciones y que operaciones se permiten. Los tokens de capacidad codifican derechos de grano fino para uso cross-DS.
- סודיות: cifrado מקצה לקצה para datas private-DS, shards codificados con borrado almacenados solo entre miembros autorizados, pruebas ZK opcionales para atestaciones externas.
- Resistencia a DoS: el aislamiento en mempool/consenso/almacenamiento evita que la congestion publica impacte el progreso de private DS.

Cambios en Componentes de Iroha
- iroha_data_model: introducir `DataSpaceId`, IDs calificados por DS, descriptores AMX (conjuntos de lectura/escritura), tipos de prueba/compromisos DA. Serializacion solo Norito.
- ivm: agregar syscalls y tipos pointer-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) y pruebas DA; מבחנים/מסמכים בפועל של ABI segun la politica v1.