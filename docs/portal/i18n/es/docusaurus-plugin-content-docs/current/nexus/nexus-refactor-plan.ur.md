---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-refactor-nexus
título: Sora Nexus لیجر ری فیکٹر پلان
descripción: `docs/source/nexus_refactor_plan.md` کا آئینہ، جو Iroha 3 کوڈ بیس کی مرحلہ وار صفائی کے کام کی تفصیل دیتا ہے۔
---

:::nota کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_refactor_plan.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ایڈیشنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus Plan de refactorización del libro mayor

یہ دستاویز Sora Nexus Ledger ("Iroha 3") کے ری فیکٹر کے لئے فوری roadmap کو محفوظ کرتی ہے۔ Diseño de diseño, contabilidad génesis/WSV, consenso Sumeragi, activadores de contrato inteligente, consultas de instantáneas, enlaces de host ABI de puntero y códecs Norito. گئی regresiones کی عکاسی کرتی ہے۔ مقصد یہ ہے کہ تمام corrige el parche monolítico کو ایک بڑے میں اتارنے کے بجائے ایک مربوط، قابل ٹیسٹ arquitectura تک پہنچا جائے۔## 0. رہنما اصول
- مختلف ہارڈویئر پر determinista رویہ برقرار رکھیں; aceleración صرف indicadores de funciones de inclusión voluntaria کے ذریعے اور ایک جیسے fallbacks کے ساتھ استعمال کریں۔
- Capa de serialización Norito ہے۔ کسی بھی estado/esquema تبدیلی میں Norito codificar/decodificar pruebas de ida y vuelta اور actualizaciones de dispositivos شامل ہونے چاہئیں۔
- configuración `iroha_config` (usuario -> real -> valores predeterminados) کے ذریعے گزرتی ہے۔ پروڈکشن rutas سے entorno ad-hoc alterna ہٹا دیں۔
- Política ABI V1 پر قائم اور غیر قابل گفت و شنید ہے۔ hosts کو نامعلوم tipos de puntero/llamadas al sistema کو determinista انداز میں رد کرنا ہوگا۔
- `cargo test --workspace` اور pruebas de oro (`ivm`, `norito`, `integration_tests`) ہر hito کے لئے بنیادی puerta رہیں گے۔## 1. ریپوزٹری ٹاپولوجی اسنیپ شاٹ
- `crates/iroha_core`: actores Sumeragi, WSV, cargador de génesis, canalizaciones (consulta, superposición, carriles zk), pegamento de host de contrato inteligente.
- `crates/iroha_data_model`: datos en cadena y consultas sobre esquemas autorizados
- `crates/iroha`: API de cliente, CLI, pruebas, SDK میں استعمال ہوتا ہے۔
- `crates/iroha_cli`: Instalar CLI, usar `iroha`, configurar API y duplicar archivos.
- `crates/ivm`: VM de código de bytes Kotodama, puntos de entrada de integración de host puntero-ABI ۔
- `crates/norito`: códec de serialización, adaptadores JSON y backends AoS/NCB.
- `integration_tests`: afirmaciones entre componentes, génesis/bootstrap, Sumeragi, activadores, paginación y کو کور کرتے ہیں۔
- Docs پہلے ہی Sora Nexus Ledger اہداف (`nexus.md`, `new_pipeline.md`, `ivm.md`) بیان کرتے ہیں، مگر implementación ٹکڑوں میں ہے اور کوڈ کے مقابلے میں جزوی طور پر پرانی ہے۔

## 2. ری فیکٹر ستون اور hitos### Fase A - Fundamentos y observabilidad
1. **Telemetría WSV + Instantáneas**
   - `state` میں API de instantáneas canónicas (rasgo `WorldStateSnapshot`) قائم کریں جسے consultas, Sumeragi اور CLI استعمال کریں۔
   - `scripts/iroha_state_dump.sh` استعمال کریں تاکہ `iroha state dump --format norito` کے ذریعے instantáneas deterministas بنیں۔
2. **Génesis/Determinismo Bootstrap**
   - ingestión de génesis کو اس طرح ری فیکٹر کریں کہ یہ ایک واحد Tubería alimentada por Norito (`iroha_core::genesis`) سے گزرے۔
   - cobertura de integración/regresión شامل کریں جو genesis اور پہلے بلاک کو replay کرے اور arm64/x86_64 کے درمیان یکساں raíces WSV ثابت کرے (ٹریک: `integration_tests/tests/genesis_replay_determinism.rs`)۔
3. **Pruebas de fijación entre cajas**
   - `integration_tests/tests/genesis_json.rs` کو بڑھائیں تاکہ WSV, اور invariantes ABI کو ایک arnés میں validar کیا جا سکے۔
   - Andamio `cargo xtask check-shape` متعارف کریں جو deriva del esquema پر pánico کرے (DevEx herramientas pendientes میں ٹریک; `scripts/xtask/README.md` کی elemento de acción دیکھیں)۔### Fase B: WSV y superficie de consulta
1. **Transacciones de almacenamiento estatal**
   - `state/storage_transactions.rs` کو ایک adaptador transaccional میں سمیٹیں جو confirmar orden اور detección de conflictos نافذ کرے۔
   - pruebas unitarias اب تصدیق کرتے ہیں کہ activo/mundo/disparadores کی تبدیلیاں ناکامی پر rollback ہوں۔
2. **Refactorización del modelo de consulta**
   - lógica de paginación/cursor کو `crates/iroha_core/src/query/` کے تحت componentes reutilizables میں منتقل کریں۔ `iroha_data_model` میں Norito representaciones کو align کریں۔
   - activadores, activos y roles, orden determinista, consultas de instantáneas, cobertura (cobertura `crates/iroha_core/tests/snapshot_iterable.rs`, ٹریک ہے) ۔
3. **Coherencia de las instantáneas**
   - یقینی بنائیں کہ `iroha ledger query` CLI y ruta de instantánea استعمال کرے جو Sumeragi/fetchers استعمال کرتے ہیں۔
   - Pruebas de regresión de instantáneas CLI `tests/cli/state_snapshot.rs` میں ہیں (ejecuciones lentas کے لئے controladas por funciones) ۔### Fase C - Tubería Sumeragi
1. **Topología y gestión de época**
   - `EpochRosterProvider` کو ایک rasgo میں نکالیں جس کی implementaciones Instantáneas de estaca WSV پر مبنی ہوں۔
   - Bancos/pruebas `WsvEpochRosterAdapter::from_peer_iter` کے لئے ایک سادہ constructor simulado فراہم کرتا ہے۔
2. **Simplificación del flujo de consenso**
   - `crates/iroha_core/src/sumeragi/*` Número de modelo: `pacemaker`, `aggregation`, `availability`, `witness` اور مشترکہ tipos کو `consensus` کے تحت رکھیں۔
   - paso de mensajes ad-hoc, sobres Norito escritos, pruebas de propiedad de cambio de vista, (acumulación de mensajes Sumeragi, میں ٹریک)۔
3. **Integración de carril/prueba**
   - pruebas de carril کو compromisos DA کے ساتھ alinear کریں اور یقینی بنائیں کہ RBC gating یکساں ہو۔
   - prueba de integración de extremo a extremo `integration_tests/tests/extra_functional/seven_peer_consistency.rs` اب ruta habilitada para RBC کو verificar کرتا ہے۔### Fase D: Contratos inteligentes y hosts Pointer-ABI
1. **Auditoría de límites del anfitrión**
   - comprobaciones de tipo puntero (`ivm::pointer_abi`) y adaptadores de host (`iroha_core::smartcontracts::ivm::host`) para consolidar datos
   - expectativas de la tabla de punteros, enlaces de manifiesto del host, `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs`, `ivm_host_mapping.rs`, enlaces de manifiesto del host, asignaciones de TLV dorado, ejercicio,
2. **Disparador de zona de pruebas de ejecución**
   - activadores کو اس طرح ری فیکٹر کریں کہ وہ مشترکہ `TriggerExecutor` کے ذریعے چلیں جو gas, validación de puntero, اور registro de eventos نافذ کرتا ہے۔
   - activadores de llamada/tiempo کے لئے pruebas de regresión شامل کریں جو rutas de fallo کو کور کرتے ہیں (ٹریک: `crates/iroha_core/tests/trigger_failure.rs`)۔
3. **CLI y alineación del cliente**
   - یقینی بنائیں کہ operaciones CLI (`audit`, `gov`, `sumeragi`, `ivm`) deriva سے بچنے کے لئے compartido Funciones del cliente `iroha` پر انحصار کریں۔
   - Pruebas de instantáneas CLI JSON `tests/cli/json_snapshot.rs` میں ہیں؛ انہیں اپ ٹو ڈیٹ رکھیں تاکہ salida del comando principal referencia JSON canónica سے coincidencia کرتا رہے۔### Fase E: endurecimiento del códec Norito
1. **Registro de esquema**
   - `crates/norito/src/schema/` کے تحت Registro de esquema Norito بنائیں تاکہ tipos de datos principales کے لئے codificaciones canónicas دستیاب ہوں۔
   - codificación de carga útil de muestra کو verificar کرنے والے pruebas de documentos شامل کریں (`norito::schema::SamplePayload`)۔
2. **Actualización de accesorios dorados**
   - `crates/norito/tests/*` کے accesorios dorados کو اپ ڈیٹ کریں تاکہ refactor کے بعد نئی Esquema WSV سے coincidencia ہوں۔
   - `scripts/norito_regen.sh` ayudante `norito_regen_goldens` کے ذریعے Norito JSON goldens کو determinista طور پر regenerar کرتا ہے۔
3. **Integración IVM/Norito**
   - Serialización del manifiesto Kotodama کو Norito کے ذریعے Validación de extremo a extremo کریں، تاکہ Metadatos ABI del puntero مستقل رہے۔
   - `crates/ivm/tests/manifest_roundtrip.rs` manifiesta کے لئے Norito paridad de codificación/decodificación برقرار رکھتا ہے۔## 3. Temas transversales
- **Estrategia de prueba**: ہر pruebas unitarias de fase -> pruebas de cajas -> pruebas de integración کو فروغ دیتا ہے۔ pruebas fallidas موجودہ regresiones کو پکڑتے ہیں؛ نئے pruebas انہیں واپس آنے سے روکتے ہیں۔
- **Documentación**: ہر fase کے اترنے کے بعد `status.md` اپ ڈیٹ کریں اور کھلے elementos کو `roadmap.md` میں منتقل کریں، جبکہ مکمل شدہ کام poda کریں۔
- **Parámetros de rendimiento**: `iroha_core`, `ivm` اور `norito` میں موجود bancos برقرار رکھیں؛ refactorizar کے بعد mediciones de referencia شامل کریں تاکہ regresiones نہ ہوں۔
- **Marcas de funciones**: alterna a nivel de caja, backends y backends, y cadenas de herramientas (`cuda`, `zk-verify-batch`) ۔ Rutas SIMD de CPU ہمیشہ build ہوتے اور runtime پر منتخب ہوتے ہیں؛ hardware no compatible کے لئے respaldos escalares deterministas فراہم کریں۔## 4. فوری اگلے اقدامات
- Andamiaje de la fase A (rasgo instantáneo + cableado de telemetría) - Actualizaciones de la hoja de ruta میں Tareas procesables دیکھیں۔
- `sumeragi`, `state`, اور `ivm` کی حالیہ auditoría de defectos نے درج ذیل aspectos más destacados سامنے لائے:
  - `sumeragi`: transmisiones a prueba de cambios de visualización de permisos de código inactivo, estado de reproducción VRF, exportación de telemetría EMA y protección de datos یہ Fase C کے simplificación del flujo de consenso اور entregables de integración de carril/prueba اترنے تک cerrado رہیں گے۔
  - `state`: Limpieza `Cell` Enrutamiento de telemetría Fase A کے Seguimiento de telemetría WSV پر منتقل ہوتے ہیں، جبکہ SoA/aplicación paralela نوٹس Trabajo pendiente de optimización de canalización de fase C میں شامل ہوتے ہیں۔
  - `ivm`: exposición de alternancia CUDA, validación de sobre, cobertura Halo2/Metal Fase D, límite de host, tema transversal de aceleración de GPU, más opciones kernels تیار ہونے تک acumulación de GPU dedicada پر رہتے ہیں۔
- cambios de código invasivos اترنے سے پہلے اس پلان کا خلاصہ دینے والا RFC entre equipos تیار کریں تاکہ aprobación de sesión مل سکے۔

## 5. کھلے سوالات
- کیا RBC کو P1 کے بعد بھی رہنا چاہیے، یا Nexus carriles de contabilidad کے لئے لازمی ہے؟ parte interesada فیصلہ درکار ہے۔
- کیا ہم P1 میں DS grupos de componibilidad نافذ کریں یا pruebas de carril کے maduro ہونے تک انہیں desactivar رکھیں؟
- Parámetros de ML-DSA-87 کی ubicación canónica کیا ہے؟ امیدوار: نیا caja `crates/fastpq_isi` (creación pendiente) ۔

---_آخری اپ ڈیٹ: 2025-09-12_