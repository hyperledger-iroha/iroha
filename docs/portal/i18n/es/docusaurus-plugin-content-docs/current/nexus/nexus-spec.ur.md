---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: especificación-nexus
título: Sora Nexus کی تکنیکی اسپیسفیکیشن
descripción: `docs/source/nexus.md` کی مکمل عکاسی، جو Iroha 3 (Sora Nexus) لیجر کی معماری اور ڈیزائن پابندیوں کا احاطہ کرتی ہے۔
---

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/nexus.md` کی عکاسی کرتا ہے۔ ترجمے کا بیک لاگ پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

#! Iroha 3 - Sora Nexus Libro mayor: تکنیکی ڈیزائن اسپیسفیکیشن

یہ دستاویز Iroha 3 کے لئے Sora Nexus Ledger کی معماری تجویز کرتی ہے، جو Iroha 2 Espacios de datos (DS) Espacios de datos مضبوط پرائیویسی ڈومینز ("espacios de datos privados") اور کھلی شمولیت ("espacios de datos públicos") فراہم کرتے ہیں۔ یہ ڈیزائن عالمی لیجر میں componibilidad کو برقرار رکھتے ہوئے DS privado ڈیٹا کے لئے سخت aislamiento اور confidencialidad یقینی بناتا ہے، اور Kura (almacenamiento en bloque) اور WSV (World State View) میں codificación de borrado کے ذریعے disponibilidad de datos کو escala کرتا ہے۔

یہی ریپوزٹری Iroha 2 (redes autoalojadas) اور Iroha 3 (SORA Nexus) دونوں کو build کرتی ہے۔ Máquina virtual Iroha (IVM) y cadena de herramientas Kotodama, contratos y artefactos de código de bytes ڈپلائمنٹس اور Nexus عالمی لیجر کے درمیان portátil رہتے ہیں۔اہداف
- ایک عالمی منطقی لیجر جو بہت سے تعاون کرنے والے validadores اور Data Spaces پر مشتمل ہو۔
- آپریشن (مثلاً CBDC) con permiso کے لئے Espacios de datos privados, جہاں ڈیٹا کبھی DS privado سے باہر نہیں جاتا۔
- Espacios de datos públicos کھلی شمولیت کے ساتھ، Ethereum جیسی sin permiso رسائی۔
- Espacios de datos, contratos inteligentes componibles, activos de DS privados, etc.
- aislamiento de rendimiento تاکہ público سرگرمی privado-DS اندرونی ٹرانزیکشنز کو متاثر نہ کرے۔
- disponibilidad de datos a escala: Kura con código de borrado اور WSV تاکہ عملی طور پر لامحدود ڈیٹا سپورٹ ہو جبکہ private-DS ڈیٹا نجی رہے۔

غیر اہداف (ابتدائی مرحلہ)
- economía de tokens یا incentivos de validador کی تعریف؛ programación y apuesta پالیسیز enchufable ہیں۔
- نئی ABI ورژن متعارف کرانا؛ تبدیلیاں IVM پالیسی کے مطابق syscall explícito اور puntero-extensiones ABI کے ساتھ ABI v1 کو ہدف بناتی ہیں۔اصطلاحات
- Nexus Libro mayor: عالمی منطقی لیجر جو Data Space (DS) بلاکس کو ایک واحد، مرتب تاریخ اور state engagement میں compose کر کے بنتا ہے۔
- Espacio de datos (DS): ejecución de datos, almacenamiento, validadores, gobernanza, clase de privacidad, política de DA, cuotas, política de tarifas, etc. دو کلاسز ہیں: DS público o DS privado
- Espacio de datos privado: validadores autorizados y control de acceso. ٹرانزیکشن ڈیٹا اور estado کبھی DS سے باہر نہیں جاتے۔ صرف compromisos/metadatos عالمی طور پر Anchor ہوتے ہیں۔
- Espacio de datos públicos: sin permiso شمولیت؛ مکمل ڈیٹا اور estado عوامی طور پر دستیاب ہیں۔
- Manifiesto de espacio de datos (Manifiesto DS): manifiesto codificado con Norito y parámetros DS (validadores/claves de control de calidad, clase de privacidad, política ISI, parámetros DA, retención, cuotas, política ZK, tarifas) ظاہر کرتا ہے۔ cadena de nexo hash manifiesto پر ancla ہوتا ہے۔ جب تک anular los certificados de quórum DS ML-DSA-87 (clase Dilithium5) کو el esquema de firma poscuántica predeterminado کے طور پر استعمال کرتے ہیں۔
- Directorio espacial: contrato de directorio en cadena, manifiestos de DS, versiones, eventos de gobernanza/rotación, resolubilidad, auditorías, etc.
- DSID: Espacio de datos کے لئے عالمی طور پر منفرد شناخت۔ تمام objetos اور referencias کے لئے espacio de nombres میں استعمال ہوتا ہے۔
- Ancla: bloque/encabezado de DS کا compromiso criptográfico جو historial de DS کو عالمی لیجر میں باندھنے کے لئے nexus chain میں شامل ہوتا ہے۔- Kura: Iroha almacenamiento en bloque ۔ Almacenamiento de blobs con código de borrado y compromisos کے ساتھ توسیع دی گئی ہے۔
- WSV: Iroha Vista del estado mundial ۔ یہاں اسے versionado, con capacidad para tomar instantáneas, con borrado de segmentos de estado codificados کے ساتھ توسیع دی گئی ہے۔
- IVM: ejecución de contrato inteligente کے لئے Máquina virtual Iroha (código de bytes Kotodama `.to`) ۔
  - AIRE: Representación Algebraica Intermedia۔ STARK: pruebas, cálculo, vista algebraica, ejecución, trazas basadas en campos, transición, restricciones de límites, restricciones de límites,Espacios de datos ماڈل
- Identidad: `DataSpaceId (DSID)` DS کی شناخت کرتا ہے اور ہر چیز کو namespace کرتا ہے۔ DS tiene granularidades para crear instancias:
  - Dominio-DS: `ds::domain::<domain_name>` - ejecución اور estado ایک dominio تک محدود۔
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - ejecución اور estado ایک واحد definición de activo تک محدود۔
  دونوں شکلیں ساتھ موجود ہیں؛ ٹرانزیکشنز atómicamente متعدد DSID کو touch کر سکتی ہیں۔
- Ciclo de vida del manifiesto: creación de DS, actualizaciones (rotación de claves, cambios de políticas), retiro del Directorio espacial میں ریکارڈ ہوتے ہیں۔ ہر artefacto DS por ranura تازہ ترین hash manifiesto کو referencia کرتا ہے۔
- Clases: DS público (participación abierta, DA público) y DS privado (permitido, DA confidencial) ۔ banderas de manifiesto de políticas híbridas کے ذریعے ممکن ہیں۔
- Políticas por DS: permisos ISI, parámetros DA `(k,m)`, cifrado, retención, cuotas (por bloque tx share کی min/max), ZK/política de prueba optimista, tarifas۔
- Gobernanza: membresía de DS, manifiesto de rotación de validadores, gobernanza, transacciones de nexo, certificaciones, gobernanza externa anclada.La capacidad se manifiesta en la UAID
- Cuentas universales: ہر participante کو ایک UAID determinista (`UniversalAccountId` en `crates/iroha_data_model/src/nexus/manifest.rs`) ملتا ہے جو تمام espacios de datos پر محیط ہے۔ Manifiestos de capacidad (`AssetPermissionManifest`) UAID کو مخصوص dataspace, épocas de activación/caducidad, اور permitir/denegar `ManifestEntry` قواعد کی ترتیب شدہ فہرست سے Función de configuración: `dataspace`, `program_id`, `method`, `asset` y funciones AMX. ہے۔ Denegar قواعد ہمیشہ غالب رہتے ہیں؛ evaluador یا تو motivo de auditoría کے ساتھ `ManifestVerdict::Denied` جاری کرتا ہے یا metadatos de asignación coincidente کے ساتھ `Allowed` subvención دیتا ہے۔
- Asignaciones: ہر permitir la entrada میں depósitos deterministas `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) کے ساتھ ایک اختیاری `max_amount` ہوتا ہے۔ Hosts اور SDK ایک ہی Carga útil Norito استعمال کرتے ہیں، اس لئے hardware de aplicación اور Implementaciones de SDK میں یکساں رہتی ہے۔
- Telemetría de auditoría: Directorio espacial جب بھی کسی manifest کا state بدلتا ہے تو `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) نشر کرتا ہے۔ نئی `SpaceDirectoryEventFilter` superficie Torii/suscriptores de eventos de datos کو actualizaciones del manifiesto UAID, revocaciones, اور denegar ganancias فیصلوں کی نگرانی plomería personalizada کے بغیر کرنے دیتی ہے۔Evidencia de operador de extremo a extremo, notas de migración de SDK, listas de verificación de publicación de manifiestos, guía de cuenta universal (`docs/source/universal_accounts_guide.md`), espejo de espejo Política de la UAID یا herramientas میں تبدیلی ہو تو دونوں دستاویزات ہم آہنگ رکھیں۔

اعلی سطحی معماری
1) Capa de composición global (cadena Nexus)
- 1 سیکنڈ کے Nexus Bloques کی ایک واحد، کینونیکل ترتیب برقرار رکھتا ہے جو ایک یا زیادہ Datos Espacios (DS) پر پھیلی atómico ٹرانزیکشنز کو finalizar کرتا ہے۔ ہر comprometido ٹرانزیکشن متحد عالمی estado mundial کو اپ ڈیٹ کرتی ہے (por-DS raíces کا vector)۔
- کم سے کم metadatos کے ساتھ pruebas/QC agregados شامل ہوتے ہیں تاکہ componibilidad, finalidad, اور detección de fraude یقینی ہو (toque کیے گئے DSID, estado por DS raíces پہلے/بعد، compromisos DA, pruebas de validez por DS, اور ML-DSA-87 y certificado de quórum DS)۔ کوئی datos privados شامل نہیں ہوتا۔
- Consenso: comité BFT canalizado global 22 (3f+1 con f=7) 200.000 validadores de grupo 200.000 validadores VRF de época/mecanismo de participación 22 (3f+1 con f=7) منتخب ہوتا ہے۔ comité de nexo ٹرانزیکشنز کو secuencia کرتا ہے اور بلاک کو 1s میں finalizar کرتا ہے۔2) Capa de espacio de datos (público/privado)
- ejecución global de fragmentos por DS, actualización de WSV local de DS, artefactos de validez por bloque (pruebas agregadas por DS y compromisos de DA) de 1 segundo Nexus Bloquear میں roll up ہوتے ہیں۔
- Datos privados en reposo de DS, datos en tránsito, validadores autorizados y cifrado de datos صرف compromisos اور PQ pruebas de validez DS سے باہر جاتے ہیں۔
- Cuerpos de datos públicos DS مکمل (a través de DA) اور PQ exportación de pruebas de validez کرتے ہیں۔3) Transacciones atómicas entre datos y espacio (AMX)
- Modelo: ہر transacción de usuario متعدد DS کو touch کر سکتی ہے (مثلاً dominio DS اور ایک یا زیادہ activo DS)۔ یہ ایک ہی Nexus Bloquear میں confirmación atómica ہوتی ہے یا abortar؛ جزوی اثرات نہیں۔
- Preparar y confirmar en 1 s: ہر transacción candidata کے لئے DS tocado ایک ہی instantánea (ranura کے آغاز کے raíces DS) کے خلاف ejecución paralela کرتے ہیں اور pruebas de validez de PQ por DS (FASTPQ-ISI) اور Compromisos de la DA بناتے ہیں۔ comité de nexo ٹرانزیکشن کو تبھی commit کرتا ہے جب تمام مطلوبہ Pruebas DS verificar ہوں اور Certificados DA وقت پر پہنچیں (ہدف <=300 ms)؛ ورنہ ٹرانزیکشن اگلے slot کے لئے reprogramar ہوتی ہے۔
- Consistencia: los conjuntos de lectura y escritura declaran ہوتے ہیں؛ detección de conflictos raíces de inicio de ranura کے خلاف commit پر ہوتی ہے۔ ejecución optimista sin bloqueo por puestos globales de DS سے بچاتی ہے؛ regla de confirmación del nexo de atomicidad سے نافذ ہوتی ہے (DS کے درمیان سب یا کچھ نہیں)۔
- Privacidad: DS privado صرف raíces pre/post DS سے بندھے pruebas/compromisos exportación کرتے ہیں۔ کوئی datos privados sin procesar DS سے باہر نہیں جاتا۔4) Disponibilidad de datos (DA) con codificación de borrado
- Cuerpos de bloques Kura اور Instantáneas WSV کو blobs codificados por borrado کے طور پر ذخیرہ کرتا ہے۔ blobs públicos وسیع پیمانے پر shard ہوتے ہیں؛ blobs privados, validadores de DS privados, fragmentos cifrados, etc.
- Compromisos DA Artefactos DS اور Nexus Bloques دونوں میں ریکارڈ ہوتے ہیں، جس سے muestreo اور garantías de recuperación ممکن ہوتی ہیں بغیر contenidos privados ظاہر کیے۔

بلاک اور کمٹ ڈھانچہ
- Artefacto a prueba de espacio de datos (1 ranura, 1 DS)
  - Archivos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Cuerpos de datos de artefactos de Private-DS کے بغیر export کرتے ہیں؛ público DS DA کے ذریعے recuperación del cuerpo کی اجازت دیتے ہیں۔

- Bloque Nexus (cadencia 1s)
  - Números: block_number, parent_hash, slot_time, tx_list (los DSID tocaron کے ساتھ transacciones atómicas entre DS), ds_artifacts[], nexus_qc.
  - فنکشن: وہ تمام atomic ٹرانزیکشنز finalizar کرتا ہے جن کے مطلوبہ DS artefactos verificar ہوں؛ vector de estado mundial global کے DS raíces کو ایک ہی قدم میں actualización کرتا ہے۔Consenso sobre programación
- Consenso de cadena Nexus: BFT global único canalizado (clase Sumeragi) Comité de 22 nodos (3f+1 con f=7) Bloques de 1s y finalidad de 1s miembros del comité ~200.000 candidatos کے grupo سے VRF/estaca de época کے ذریعے منتخب ہوتے ہیں؛ rotación descentralización اور resistencia a la censura برقرار رکھتی ہے۔
- Consenso del espacio de datos: ہر DS اپنے validadores کے درمیان اپنا BFT چلاتا ہے تاکہ artefactos por ranura (pruebas, compromisos DA, DS QC) بنائے جا سکیں۔ Comités de retransmisión de carriles `3f+1` سائز میں espacio de datos `fault_tolerance` سیٹنگ استعمال کرتے ہیں اور ہر epoch میں grupo de validadores de espacio de datos سے muestra determinista ہوتے ہیں، VRF epoch seed کو `(dataspace_id, lane_id)` کے ساتھ bind کر کے۔ DS privado con permiso ہیں؛ público DS anti-Sybil پالیسیز کے تحت vida abierta دیتے ہیں۔ comité de nexo global تبدیل نہیں ہوتا۔
- Programación de transacciones: صارفین atómico ٹرانزیکشنز enviar کرتے ہیں جن میں tocó DSID اور conjuntos de lectura y escritura declarar ہوتے ہیں۔ Ranura DS کے اندر ejecución paralela کرتے ہیں؛ comité de nexo ٹرانزیکشن کو 1s بلاک میں تبھی شامل کرتا ہے جب تمام Los artefactos DS verifican ہوں اور certificados DA بروقت ہوں (<=300 ms)۔- Aislamiento de rendimiento: ہر DS کے پاس mempools independientes اور ejecución ہوتی ہے۔ Cuotas por DS تاکہ bloqueo de cabecera de línea سے بچا جا سکے اور latencia DS privada محفوظ رہے۔

ڈیٹا ماڈل اور Espacio de nombres
- ID calificados por DS: تمام entidades (dominios, cuentas, activos, roles) `dsid` کے ساتھ califican ہوتے ہیں۔ Nombre: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias globales: referencia global, tupla `(dsid, object_id, version_hint)`, capa de nexo, en cadena, DS cruzado, descriptores AMX, descriptores AMX. ہے۔
- Serialización Norito: mensajes cross-DS (descriptores AMX, pruebas) Códecs Norito استعمال کرتے ہیں۔ rutas de producción میں serde کا استعمال نہیں ہوتا۔Contratos inteligentes اور IVM توسیعات
- Contexto de ejecución: IVM contexto de ejecución میں `dsid` شامل کریں۔ Kotodama contratos ہمیشہ کسی مخصوص Data Space کے اندر ejecutar ہوتے ہیں۔
- Primitivas atómicas Cross-DS:
  - `amx_begin()` / `amx_commit()` IVM host میں atómico multi-DS ٹرانزیکشن کو delinear کرتے ہیں۔
  - `amx_touch(dsid, key)` detección de conflictos کے لئے raíces de instantáneas de ranura کے خلاف declaración de intención de lectura/escritura کرتا ہے۔
  - `verify_space_proof(dsid, proof, statement)` -> booleano
  - `use_asset_handle(handle, op, amount)` -> resultado (operación تبھی permitida ہے جب política اجازت دے اور identificador válido ہو)
- Manejo de activos y tarifas:
  - Operaciones de activos DS کے ISI/políticas de rol کے تحت autorizan ہوتے ہیں؛ tarifas DS کے token de gas میں ادا ہوتی ہیں۔ tokens de capacidad opcionales اور زیادہ políticas enriquecidas (aprobador múltiple, límites de velocidad, geocercas) بعد میں atomic ماڈل بدلے بغیر شامل کی جا سکتی ہیں۔
- Determinismo: entradas de llamadas al sistema, conjuntos de lectura/escritura AMX declarados, funciones puras o deterministas. وقت یا ماحول کے efectos ocultos نہیں ہوتے۔Pruebas de validez poscuánticas (ISI generalizadas)
- FASTPQ-ISI (PQ, sin configuración confiable): kernelizado, argumento basado en hash, transferencia, familias ISI, hardware de clase GPU, lotes a escala de 20k y pruebas en menos de un segundo. کو ہدف بناتا ہے۔
  - Perfil operativo:
    - Probador de nodos de producción کو `fastpq_prover::Prover::canonical` کے ذریعے build کرتے ہیں، جو اب ہمیشہ backend de producción inicializar کرتا ہے؛ simulacro determinista ہٹا دیا گیا ہے۔ [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) اور `irohad --fastpq-execution-mode` operadores کو ejecución de CPU/GPU کو pin determinista کرنے دیتے ہیں جبکہ auditorías de flota de gancho de observador کے لئے triples solicitados/resueltos/backend کو ریکارڈ کرتا ہے۔ [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetización:
  - KV-Update AIR: WSV کو Poseidon2-SMT کے ذریعے mapa clave-valor escrito comprometido کے طور پر tratar کرتا ہے۔ ہر claves ISI (cuentas, activos, roles, dominios, metadatos, suministro) پر filas de lectura-verificación-escritura کے چھوٹے سیٹ میں expandir ہوتا ہے۔
  - Restricciones controladas por código de operación: columnas selectoras کے ساتھ ایک ہی Tabla AIR por ISI قواعد نافذ کرتی ہے (conservación, contadores monótonos, permisos, comprobaciones de rango, actualizaciones de metadatos limitados) ۔- Argumentos de búsqueda: permisos/roles, precisiones de activos, parámetros de políticas, tablas transparentes con hash confirmado, fuertes restricciones bit a bit, etc.
- Compromisos y actualizaciones estatales:
  - Prueba SMT agregada: تمام teclas tocadas (pre/post) `old_root`/`new_root` کے خلاف ایک frontera comprimida کے ساتھ prueba ہوتے ہیں جس میں hermanos deduplicados ہوں۔
  - Invariantes: invariantes globales (مثلاً ہر activo کی oferta total) efecto filas اور contadores rastreados کے درمیان igualdad multiconjunto کے ذریعے نافذ ہوتے ہیں۔
- Sistema de prueba:
  - Compromisos polinómicos estilo FRI (DEEP-FRI) de alta aridad (16/8) اور explosión 8-16 کے ساتھ؛ Hashes Poseidon2؛ Transcripción Fiat-Shamir SHA-2/3 کے ساتھ۔
  - Recursión opcional: اگر ضرورت ہو تو micro-lotes کو ایک prueba فی ranura comprimir کرنے کے لئے DS-agregación recursiva local۔
- Alcance y ejemplos cubiertos:
  - Activos: transferir, acuñar, quemar, registrar/anular el registro de definiciones de activos, establecer precisión (limitada), establecer metadatos ۔
  - Cuentas/Dominios: crear/eliminar, establecer clave/umbral, agregar/eliminar firmantes (solo para el estado, comprobaciones de firma, los validadores de DS dan fe de کرتے ہیں، AIR کے اندر نہیں ہوتے)۔
  - Roles/Permisos (ISI): otorgar/revocar roles y permisos tablas de búsqueda, verificaciones de políticas monótonas y hacer cumplir ہوتے ہیں۔
  - Contratos/AMX: marcadores de inicio/compromiso de AMX, capacidad de acuñación/revocación habilitada ہو؛ transiciones estatales اور contadores de políticas کے طور پر probar ہوتے ہیں۔- Comprobaciones fuera del aire para preservar la latencia:
  - Firmas اور criptografía pesada (مثلاً ML-DSA firmas de usuario) Los validadores DS verifican کرتے ہیں اور DS QC میں atestiguan ہوتے ہیں؛ prueba de validez صرف consistencia del estado اور cumplimiento de la política کو cobertura کرتا ہے۔ یہ pruebas کو PQ اور تیز رکھتا ہے۔
- Objetivos de rendimiento (ilustrativos, CPU de 32 núcleos + GPU única y moderna):
  - 20.000 ISI mixtos con pulsación de tecla pequeña (<=8 teclas/ISI): ~0,4-0,9 s de prueba, ~150-450 KB de prueba, ~5-15 ms de verificación۔
  - ISI más pesados ​​(más claves/restricciones ricas): microlotes (más de 10x2k) + recursividad تاکہ por ranura <1 s رہے۔
- Configuración del manifiesto DS:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (firmas DS QC verificar کرتا ہے)
  - `attestation.qc_signature = "ml_dsa_87"` (predeterminado, alternativas y explícitas para declarar کرنا ہوگا)
- Opciones alternativas:
  - ISI complejos/personalizados عمومی STARK (`zk.policy = "stark_fri_general"`) استعمال کر سکتے ہیں جس میں prueba diferida اور 1 s finalidad Atestación de control de calidad + reducción کے ذریعے ہوتی ہے۔
  - Opciones que no son PQ (مثلاً Plonk con KZG) configuración confiable چاہتی ہیں اور compilación predeterminada میں اب compatible نہیں ہیں۔AIR تعارف (Nexus کے لئے)
- Seguimiento de ejecución: ایک matriz جس کی ancho (columnas de registro) اور longitud (pasos) ہوتی ہے۔ Procesamiento ISI de ہر fila کا منطقی قدم ہے؛ columnas con valores pre/post, selectores, banderas ہوتے ہیں۔
- Restricciones:
  - Restricciones de transición: relaciones fila a fila نافذ کرتی ہیں (مثلاً post_balance = pre_balance - importe جب `sel_transfer = 1` والی fila de débito ہو)۔
  - Restricciones de límites: E/S pública (raíz_antigua/raíz_nueva, contadores) کو پہلی/آخری filas سے enlazar کرتی ہیں۔
  - Búsquedas/permutaciones: tablas comprometidas (permisos, parámetros de activos), membresía, igualdades de conjuntos múltiples, circuitos de bits pesados,
- Compromiso y verificación:
  - Prover rastrea codificaciones basadas en hash, confirmaciones, polinomios de bajo grado, restricciones válidas y restricciones
  - Verificador FRI (basado en hash, post-cuántico) کے ذریعے verificación de bajo grado کرتا ہے، چند Aperturas de Merkle کے ساتھ؛ pasos de costo کے log پر منحصر ہے۔
- Ejemplo (Transferencia): registros میں pre_balance, cantidad, post_balance, nonce, اور selectores شامل ہیں۔ Restricciones no negatividad/rango, conservación, اور nonce monotonicidad نافذ کرتی ہیں، جبکہ hojas pre/post SMT agregadas de prueba múltiple کو raíces viejas/nuevas سے جوڑتی ہے۔ABI y Syscall (ABI v1)
- Syscalls para agregar (nombres ilustrativos):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos de puntero-ABI para agregar:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Actualizaciones requeridas:
  - `ivm::syscalls::abi_syscall_list()` میں شامل کریں (ordenando برقرار رکھیں), política کے ذریعے puerta کریں۔
  - números desconocidos کو hosts میں `VMError::UnknownSyscall` پر mapa کریں۔
  - Actualización de pruebas کریں: lista de llamadas al sistema golden, hash ABI, ID de tipo de puntero goldens, pruebas de políticas ۔
  - documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Privacidad ماڈل
- Contención de datos privados: DS privado, cuerpos de transacción, diferencias de estado, instantáneas de WSV, subconjunto de validador privado, etc.
- Exposición pública: encabezados, compromisos DA, exportación de pruebas de validez PQ
- Pruebas ZK opcionales: Pruebas privadas DS ZK (equilibrio کافی ہے، política پوری ہوئی) بنا سکتے ہیں تاکہ estado interno ظاہر کئے بغیر acciones entre DS ممکن ہوں۔
- Control de acceso: autorización DS کے اندر ISI/políticas de rol کے ذریعے نافذ ہوتی ہے۔ tokens de capacidad اختیاری ہیں اور بعد میں شامل کئے جا سکتے ہیں۔Aislamiento de rendimiento y QoS
- ہر DS کے لئے consenso, mempools, اور almacenamiento الگ۔
- Cuotas de programación Nexus por tiempo de inclusión del ancla DS کو محدود کرتی ہیں اور head-of-line blocking سے بچاتی ہیں۔
- Presupuestos de recursos de contrato por DS (cómputo/memoria/IO) IVM host کے ذریعے نافذ ہوتے ہیں۔ Contención de DS público-Presupuestos de DS privado استعمال نہیں کر سکتی۔
- Llamadas asincrónicas entre DS Ejecución de DS privado کے اندر لمبی esperas sincrónicas سے بچاتی ہیں۔

Disponibilidad de datos y diseño de almacenamiento
1) Codificación de borrado
- Bloques Kura اور Instantáneas WSV کے codificación de borrado a nivel de blob کے لئے Reed-Solomon sistemático (مثلاً GF(2^16)) استعمال کریں: parámetros `(k, m)` جہاں `n = k + m` fragmentos ہیں۔
- Parámetros predeterminados (DS público propuesto): `k=32, m=16` (n=48) ، جس سے ~1.5x expansión کے ساتھ 16 fragmentos تک recuperación ممکن ہے۔ DS privado کے لئے: `k=16, m=8` (n=24) conjunto autorizado کے اندر۔ دونوں DS Manifest کے ذریعے configurable ہیں۔
- Blobs públicos: fragmentos, nodos/validadores DA, distribución, comprobaciones de disponibilidad basadas en muestreo, etc. encabezados میں compromisos DA clientes ligeros کو verificar کرنے دیتے ہیں۔
- Blobs privados: los fragmentos cifran y distribuyen validadores DS privados (custodios designados) y distribuyen cadena global صرف compromisos DA رکھتی ہے (ubicaciones de fragmentos یا claves نہیں)۔2) Compromisos y muestreo
- ہر blob کے لئے shards پر Merkle root نکال کر `*_da_commitment` میں شامل کریں۔ compromisos de curva elíptica سے بچ کر PQ رہیں۔
- DA Attesters: atestados regionales muestreados por VRF (región مثلاً ہر میں 64) muestreo exitoso de fragmentos کی atestación کے لئے Certificado ML-DSA-87 جاری کرتے ہیں۔ ہدف Latencia de atestación DA <=300 ms ہے۔ fragmentos del comité nexo لینے کے بجائے certificados validar کرتی ہے۔

3) Integración de Kura
- Bloquea los cuerpos de transacciones کو Compromisos de Merkle کے ساتھ blobs codificados por borrado کے طور پر store کرتے ہیں۔
- Compromisos de blobs de encabezados رکھتے ہیں؛ organismos DS públicos کے لئے red DA اور DS privados کے لئے canales privados کے ذریعے قابلِ بازیافت ہیں۔

4) Integración WSV
- Instantáneas WSV: instantáneas periódicas del estado de DS, instantáneas fragmentadas y codificadas por borrado, puntos de control, encabezados de compromisos, etc. instantáneas کے درمیان registros de cambios برقرار رہتے ہیں۔ instantáneas públicas وسیع پیمانے پر shard ہوتے ہیں؛ instantáneas privadas validadores privados کے اندر رہتے ہیں۔
- Acceso con pruebas: contratos, pruebas estatales (Merkle/Verkle) فراہم یا طلب کر سکتے ہیں جو compromisos instantáneos سے anclados ہوں۔ Pruebas privadas de DS کے بجائے certificaciones de conocimiento cero دے سکتے ہیں۔5) Retención y Poda
- DS público کے لئے poda نہیں: تمام Cuerpos de Kura اور Instantáneas WSV DA کے ذریعے retener کریں (escalado horizontal) ۔ La retención interna de DS privado define los compromisos exportados inmutables. Capa Nexus Bloques Nexus Compromisos de artefactos DS برقرار رکھتی ہے۔

Redes y roles de nodo
- Validadores globales: consenso de nexo میں حصہ لیتے ہیں، Nexus Bloques اور DS artefactos validar کرتے ہیں، DS público کے لئے Verificaciones DA کرتے ہیں۔
- Validadores de espacio de datos: DS consenso چلاتے ہیں، contratos ejecutar کرتے ہیں، Kura/WSV local administrar کرتے ہیں، اپنے DS کے لئے DA manejar کرتے ہیں۔
- Nodos DA (opcional): almacenamiento/publicación de blobs públicos کرتے ہیں، muestreo میں مدد دیتے ہیں۔ DS privado کے لئے، Validadores de nodos DA یا custodios confiables کے ساتھ co-ubicar ہوتے ہیں۔Mejoras a nivel del sistema اور غور و فکر
- Secuenciación/desacoplamiento de mempool: DAG mempool (estilo Narwhal), capa de nexo, BFT canalizado, alimentación, latencia, rendimiento منطقی ماڈل بدلے۔
- Cuotas de DS اور equidad: cuotas por DS por bloque اور límites de peso bloqueo de cabecera سے بچانے اور DS privado کے لئے قابلِ پیش گوئی latencia یقینی بنانے کے لئے۔
- Atestación DS (PQ): certificados de quórum DS predeterminados ML-DSA-87 (clase Dilithium5) استعمال کرتے ہیں۔ یہ post-cuántico ہے اور firmas EC سے بڑا ہے مگر ایک QC فی ranura قابلِ قبول ہے۔ DS اگر DS Manifest میں declarar کریں تو ML-DSA-65/44 (چھوٹا) یا Firmas CE منتخب کر سکتے ہیں؛ public DS کے لئے ML-DSA-87 برقرار رکھنے کی سخت سفارش ہے۔
- Certificadores DA: DS públicos کے لئے Certificados DA regionales muestreados por VRF Certificados DA جاری کرتے ہیں۔ comité de nexo muestreo de fragmentos sin procesar کے بجائے certificados validar کرتی ہے؛ atestaciones privadas de DS اندرونی DA رکھتے ہیں۔
- Pruebas de recursividad y época: اختیاری طور پر ایک DS میں متعدد microlotes کو ایک prueba recursiva por ranura/época میں agregado کریں تاکہ tamaños de prueba اور verificar tiempo carga alta پر مستحکم رہیں۔
- Escalado de carriles (اگر ضرورت ہو): اگر ایک cuello de botella del comité global بن جائے تو K carriles de secuenciación paralela متعارف کریں جن کی fusión determinista ہو۔ اس سے orden global única برقرار رہتا ہے جبکہ escala horizontal ہو جاتی ہے۔- Aceleración determinista: hash/FFT, kernels activados por funciones SIMD/CUDA, respaldo de CPU con bits exactos, determinismo entre hardware, etc.
- Umbrales de activación de carril (propuesta): 2-4 carriles فعال کریں اگر (a) p95 finalidad 1,2 s سے زیادہ ہو >3 مسلسل منٹ، یا (b) ocupación por cuadra 85% سے زیادہ ہو >5 منٹ، یا (c) tasa de transmisión entrante sostenida سطحوں پر capacidad de bloque کی >1.2x ضرورت رکھتی ہو۔ carriles deterministas طور پر DSID hash کے ذریعے transacciones کو cubo کرتی ہیں اور nexus block میں fusionar ہوتی ہیں۔

Tarifas اور Economía (ابتدائی valores predeterminados)
- Unidad de gas: token de gas por DS جس میں cálculo/IO medido ہوتا ہے؛ tarifas DS کے activo de gas nativo میں ادا ہوتی ہیں۔ DS کے درمیان aplicación de conversión کی ذمہ داری ہے۔
- Prioridad de inclusión: round robin en DS کے ساتھ cuotas por DS تاکہ equidad اور 1s SLO برقرار رہیں؛ DS کے اندر tarifa de licitación desempate کر سکتی ہے۔
- Futuro: mercado de tarifas global opcional یا Políticas de minimización de MEV exploran کی جا سکتی ہیں بغیر atomicidad یا Diseño de prueba PQ بدلے۔Flujo de trabajo entre espacios de datos (مثال)
1) ایک صارف AMX ٹرانزیکشن enviar کرتا ہے جو público DS P اور privado DS S کو touch کرتی ہے: activo X کو S سے beneficiario B تک منتقل کریں جس کی cuenta P میں ہے۔
2) ranura کے اندر، P اور S ہر ایک اپنا fragmento ranura instantánea کے خلاف ejecutar کرتے ہیں۔ Autorización S اور verificación de disponibilidad کرتا ہے، اپنا actualización del estado interno کرتا ہے، اور prueba de validez de PQ اور compromiso DA بناتا ہے (کوئی fuga de datos privados نہیں ہوتا)۔ P متعلقہ actualización de estado تیار کرتا ہے (مثلاً política کے مطابق P میں mint/burn/locking) اور اپنی prueba۔
3) comité de nexo دونوں pruebas DS اور certificados DA verificar کرتی ہے؛ اگر دونوں ranura کے اندر verificar ہوں تو ٹرانزیکشن 1s Nexus Bloque میں confirmación atómica ہوتی ہے، vector de estado mundial global میں دونوں Actualización de raíces DS ہوتے ہیں۔
4) اگر کوئی prueba یا Certificado DA غائب/inválido ہو تو ٹرانزیکشن abortar ہو جاتی ہے (کوئی اثر نہیں), اور cliente اگلے ranura کے لئے دوبارہ بھیج سکتا ہے۔ کسی مرحلے پر S سے کوئی datos privados باہر نہیں جاتا۔- سکیورٹی پر غور و فکر
- Ejecución determinista: IVM llamadas al sistema deterministas رہتے ہیں؛ cross-DS نتائج AMX commit اور finalidad سے چلتے ہیں، reloj de pared یا sincronización de red سے نہیں۔
- Control de acceso: permisos privados de DS میں ISI محدود کرتی ہیں کہ کون ٹرانزیکشن enviar کر سکتا ہے اور کون سی operaciones permitidas ہیں۔ tokens de capacidad cross-DS استعمال کے لئے codificación detallada حقوق کرتے ہیں۔
- Confidencialidad: datos privados de DS, cifrado de extremo a extremo, fragmentos codificados con borrado, miembros autorizados, certificaciones, pruebas ZK opcionales
- Resistencia DoS: mempool/consenso/capas de almacenamiento میں aislamiento congestión pública کو progreso de DS privado پر اثر انداز ہونے سے روکتی ہے۔

Componentes Iroha میں تبدیلیاں
- iroha_data_model: `DataSpaceId`, identificadores calificados para DS, descriptores AMX (conjuntos de lectura/escritura), tipos de compromiso de prueba/DA serialización según Norito۔
- ivm: AMX (`amx_begin`, `amx_commit`, `amx_touch`) y pruebas DA, llamadas al sistema y tipos de puntero-ABI. Pruebas/documentos ABI کو v1 پالیسی کے مطابق اپ ڈیٹ کریں۔