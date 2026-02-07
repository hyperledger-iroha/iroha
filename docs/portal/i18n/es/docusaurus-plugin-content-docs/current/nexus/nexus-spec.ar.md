---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identificación: especificación-nexus
título: المواصفات التقنية لسورا نيكسس
descripción: مرآة كاملة لـ `docs/source/nexus.md` تغطي معمارية وقيود التصميم لدفتر Iroha 3 (Sora Nexus).
---

:::nota المصدر الرسمي
Utilice el botón `docs/source/nexus.md`. حافظ على النسختين متطابقتين حتى يصل تراكم الترجمات إلى البوابة.
:::

#! Iroha 3 - Sora Nexus Ledger: المواصفات التقنية للتصميم

Para obtener más información, consulte Sora Nexus Ledger o Iroha 3 o Iroha 2. منطقيا ومنظم حول Espacios de datos (DS). توفر Espacios de datos نطاقات خصوصية قوية ("espacios de datos privados") ومشاركة مفتوحة ("espacios de datos públicos"). Componibilidad de composición المحو في Kura (almacenamiento en bloque) y WSV (World State View).

Este es el caso de Iroha 2 (Iroha 3 (SORA Nexus). يعتمد التنفيذ على Iroha Virtual Machine (IVM) المشتركة وسلسلة ادوات Kotodama, لذا تبقى العقود وقطع الـ bytecode قابلة للنقل بين النشر الذاتي والدفتر العالمي لنكسس.الاهداف
- دفتر منطقي عالمي واحد مكون من العديد من المدققين المتعاونين y Data Spaces.
- Espacios de datos خاصة للتشغيل المقيّد (مثل CBDC) مع بقاء البيانات داخل DS الخاص.
- Espacios de datos عامة بمشاركة مفتوحة ووصول دون اذن على غرار Ethereum.
- عقود ذكية قابلة للتركيب عبر Data Spaces, مع صلاحيات صريحة للوصول إلى اصول private-DS.
- عزل الاداء بحيث لا تؤثر الحركة العامة على معاملات private-DS الداخلية.
- توفر بيانات على نطاق واسع: Kura y WSV بترميز محو لدعم بيانات شبه غير محدودة مع بقاء بيانات private-DS خاصة.

غير الاهداف (المرحلة الاولى)
- تعريف اقتصاديات التوكن او حوافز المدققين؛ سياسات programación y apuesta قابلة للتوصيل.
- إدخال اصدار ABI جديد؛ Las aplicaciones ABI v1 incluyen llamadas al sistema y puntero-ABI y el nombre IVM.مصطلحات
- Nexus Libro mayor: الدفتر المنطقي العالمي المتشكل من تركيب كتل Data Space (DS) في تاريخ مرتب واحد والتزام حالة.
- Espacio de datos (DS): نطاق تنفيذ وتخزين محدود بمدققين خاصين وحوكمة وفئة خصوصية وسياسة DA وحصص وسياسة رسوم. Tipos: DS público y DS privado.
- Espacio de datos privado: مدققون باذونات وتحكم وصول؛ بيانات المعاملة y لالحالة لا تغادر DS. يتم تثبيت الالتزامات/البيانات الوصفية فقط عالميا.
- Espacio público de datos: مشاركة بدون اذن؛ البيانات الكاملة والحالة متاحة للجميع.
- Manifiesto de espacio de datos (Manifiesto DS): manifiesto مشفّر عبر Norito يعلن معلمات DS (validadores/claves de control de calidad, فئة الخصوصية, سياسة ISI، معلمات DA، الاحتفاظ، الحصص، سياسة ZK، الرسوم). Hay un hash en el manifiesto del nexo. Hay un quórum de DS de ML-DSA-87 (Dilithium5) post-cuántico.
- Directorio de espacio: عقد دليل عالمي على السلسلة يتتبع manifiesta DS والنسخ واحداث الحوكمة/الدوران لاغراض الاستدلال والتدقيق.
- DSID: معرف فريد عالميا لـ Espacio de datos. يستخدم لعمل espacio de nombres لكل الكائنات والمراجع.
- Ancla: التزام تشفيري من كتلة/راس DS يضم الى سلسلة nexus لربط تاريخ DS بالدفتر العالمي.
- Kura: تخزين كتل Iroha. ممتد هنا بتخزين blobs بترميز محو والتزامات.
- WSV: Iroha Vista del estado mundial. ممتد هنا بمقاطع حالة ذات نسخ ولقطات ومرمزة بالمحو.
- IVM: Máquina virtual Iroha (código de bytes Kotodama `.to`).- AIRE: Representación Algebraica Intermedia. تمثيل جبري للحساب من اجل ادلة نمط STARK يصف التنفيذ كسلاسل تعتمد على الحقول مع قيود انتقال وحدود.

Más espacios de datos
- Texto: `DataSpaceId (DSID)` يعرّف DS y el espacio de nombres لكل شيء. يمكن انشاء DS على درجتين:
  - Dominio-DS: `ds::domain::<domain_name>` - تنفيذ وحالة ضمن نطاق dominio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - تنفيذ وحالة ضمن تعريف اصل واحد.
  الشكلان يتعايشان؛ ويمكن للمعاملات لمس عدة DSID بشكل ذري.
- Manifiesto de دورة حياة: تسجيل إنشاء DS والتحديثات (تدوير المفاتيح، تغييرات السياسة) y الإنهاء في Space Directory. La ranura DS de la ranura requiere un hash en el manifiesto.
- الفئات: DS público (مشاركة مفتوحة، DA عامة) y DS privado (مقيد، DA سرية). سياسات هجينة ممكنة عبر banderas في manifiesto.
- Dispositivos de DS: ISI, DA `(k,m)`, dispositivos de conexión (tx الدنيا/العليا لكل كتلة), سياسة اثبات ZK/تفاؤلي، رسوم.
- الحوكمة: العضوية والدوران في DS يحددان في قسم الحوكمة بالـ manifest (مقترحات على السلسلة, multisig, او حوكمة خارجية مثبتة بمعاملات nexo y atestaciones).Manifiestos القدرات y UAID
- الحسابات العالمية: يحصل كل مشارك على UAID حتمي (`UniversalAccountId` في `crates/iroha_data_model/src/nexus/manifest.rs`) يغطي جميع espacios de datos. Manifiestos de تربط القدرات (`AssetPermissionManifest`) الـ UAID بـ dataspace محدد وفترات تفعيل/انتهاء وقائمة مرتبة من قواعد permitir/denegar `ManifestEntry` التي Utilice `dataspace`, `program_id`, `method`, `asset` y AMX. قواعد negar تفوز دائما؛ ويصدر المقيّم `ManifestVerdict::Denied` مع سبب تدقيق او منح `Allowed` مع بيانات Allowance المطابقة.
- Asignaciones: يحمل كل permitir cubo حتمي `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) مع `max_amount` اختياري. تستعمل hosts y SDK نفس حمولة Norito, لذا يكون التنفيذ متطابقا عبر العتاد y SDK.
- Telemetría تدقيق: يبث Space Directory حدث `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) عند تغير حالة manifiesto. السطح الجديد `SpaceDirectoryEventFilter` يمكّن مشتركين Torii/data-event من مراقبة تحديثات UAID manifest y UAID y deny-wins اعداد مخصص.

Para obtener más información sobre el SDK y los manifiestos, consulte la Guía de cuentas universal. (`docs/source/universal_accounts_guide.md`). ابق الوثيقتين متطابقتين عند تغير سياسة UAID او الادوات.المعمارية عالية المستوى
1) طبقة التركيب العالمية (Cadena Nexus)
- Utilice el software Nexus y 1 software de Data Spaces (DS). كل معاملة ملتزمة تحدث estado mundial الموحد (متجه raíces لكل DS).
- تحتوي بيانات وصفية دنيا مع pruebas/QC مجمعة لضمان componsability والنهائية وكشف الاحتيال (DSIDs المتأثرة, rootes لكل DS قبل/بعد، Las pruebas de DA, las pruebas de DS y el quórum de DS (ML-DSA-87). لا تضمن بيانات خاصة.
- الاجماع: لجنة BFT عالمية بPipeline من 22 عقدة (3f+1 مع f=7) مختارة عبر VRF/stake لكل حقبة من مجموعة حتى ~200k مدقق محتمل. لجنة nexus ترتب المعاملات y تؤكد الكتلة خلال 1s.

2) Espacio de datos externo (público/privado)
- تنفذ مقاطع المعاملات لكل DS، وتحدث WSV المحلي، وتنتج artefactos صلاحية لكل كتلة (pruebas مجمعة لكل DS والتزامات DA) تتجمع Aquí está Nexus durante 1s.
- DS privado تشفر البيانات في السكون والحركة بين المدققين المصرح لهم؛ ولا يغادر DS سوى الالتزامات و pruebas صلاحية PQ.
- Public DS تصدر اجسام البيانات الكاملة (a través de DA) y pruebas صلاحية PQ.3) Espacios de datos (AMX)
- النموذج: كل معاملة مستخدم قد تمس عدة DS (مثل dominio DS y activo DS). تلتزم ذرّيا في كتلة Nexus واحدة او تلغى؛ لا آثار جزئية.
- Prepare-Commit 1s: لكل معاملة مرشحة, تنفذ DS المعنية بالتوازي على نفس snapshot (roots DS ببداية slot) y pruebas صلاحية PQ لكل DS (FASTPQ-ISI) y DA. يلتزم comité nexus فقط اذا تحققت كل pruebas المطلوبة ووصلت شهادات DA في الوقت (هدف <=300 ms)؛ والا تعاد جدولة المعاملة للـ slot التالي.
- الاتساق: مجموعات القراءة/الكتابة مصرح بها؛ يتم كشف التعارض عند الالتزام مقابل root بداية slot. التنفيذ المتفائل بدون اقفال لكل DS يتجنب التعطيل العالمي؛ الذرية تفرض بقاعدة التزام nexus (الكل او لا شيء عبر DS).
- الخصوصية: تصدر DS privado فقط pruebas/compromisos مرتبطة بـ raíces قبل/بعد. لا تخرج بيانات خاصة خام.

4) توفر البيانات (DA) بترميز محو
- يخزن Kura اجسام الكتل ولقطات WSV كـ blobs بترميز محو. يتم توزيع blobs العامة على نطاق واسع؛ Hay blobs que contienen fragmentos de DS privados.
- يتم تسجيل التزامات DA في artefactos الخاصة بـ DS y كتل Nexus, مما يمكّن من ضمانات sampling y الاسترجاع دون كشف المحتوى الخاص.هيكل الكتل والالتزام
- Artefacto en espacio de datos (para ranura 1 y DS)
  - Contenido: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - تصدر artefactos DS privados بدون اجسام بيانات؛ وpublic DS تسمح باسترجاع الاجسام عبر DA.

- Bloque Nexus (aproximadamente 1 s)
  - Contenido: block_number, parent_hash, slot_time, tx_list (combinación cross-DS o DSID), ds_artifacts[], nexus_qc.
  - الوظيفة: تثبيت جميع المعاملات الذرية التي تتحقق artefactos المطلوبة لها؛ تحديث متجهroots العالمي لكل DS في خطوة واحدة.الاجماع y programación
- Cadena Nexus: BFT عالمي متسلسل (فئة Sumeragi) مع لجنة 22 عقدة (3f+1 مع f=7) تستهدف كتل 1s ونهائية 1s. يتم اختيار اللجنة عبر VRF/stake لكل حقبة من ~200k مرشح؛ الدوران يحافظ على اللامركزية ومقاومة الرقابة.
- Espacio de datos: يشغل كل DS BFT خاص به لانتاج artefactos لكل slot (pruebas, التزامات DA, DS QC). Relé de carril `3f+1` `fault_tolerance` espacio de datos y espacio de datos La semilla VRF se utiliza en `(dataspace_id, lane_id)`. DS privado مقيدة؛ Public DS تسمح بالحيوية المفتوحة مع سياسات anti-Sybil. اللجنة العالمية nexus ثابتة.
- Programación المعاملات: يرسل المستخدمون معاملات ذرية مع DSIDs ومجموعات القراءة/الكتابة. تنفذ DS بالتوازي داخل slot؛ وتضم لجنة nexus المعاملة في كتلة 1s اذا تحقق جميع artefactos ووصلت شهادات DA في الوقت (<=300 ms).
- عزل الاداء: لكل DS mempools وتنفيذ مستقل. تقيد cuotas de DS عدد المعاملات التي تلمس DS في كل كتلة لتجنب bloqueo de cabecera de línea y DS privado.Nombres de usuario y espacio de nombres
- ID calificados por DS: كل الكيانات (dominios, cuentas, activos, roles) مؤهلة بـ `dsid`. Nombre: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias globales: المرجع العالمي عبارة عن tupla `(dsid, object_id, version_hint)` ويمكن وضعه on-chain في طبقة nexus او في اوصاف AMX للاستخدام cross-DS.
- Serialización Norito: جميع رسائل cross-DS (اوصاف AMX، pruebas) تستخدم Norito codecs. لا استخدام لـ serde في مسارات الانتاج.

العقود الذكية y امتدادات IVM
- Nombre del usuario: `dsid` y nombre del usuario IVM. Aquí Kotodama está conectado a Data Space.
- primitivo ذرية عبر DS:
  - `amx_begin()` / `amx_commit()` Establece un sistema multi-DS en el host IVM.
  - `amx_touch(dsid, key)` تعلن نية القراءة/الكتابة لكشف التعارض مقابل root snapshot للـ slot.
  - `verify_space_proof(dsid, proof, statement)` -> booleano
  - `use_asset_handle(handle, op, amount)` -> resultado (مسموح فقط اذا سمحت السياسة وكان handle صالحا)
- Manejadores de activos والرسوم:
  - عمليات الاصول مصرح بها عبر سياسات ISI/role للـ DS؛ Conecte el gas del DS. يمكن اضافة tokens de capacidad y اكثر ثراء (aprobación múltiple, límites de tasa, geocercado) لاحقا دون تغيير النموذج الذري.
- الحتمية: كل syscalls الجديدة نقية وحتمية عند المدخلات ومجموعات القراءة/الكتابة AMX المعلنة. لا تأثيرات خفية للوقت او البيئة.اثباتات الصلاحية post-cuántico (ISI معممة)
- FASTPQ-ISI (configuración confiable de PQ): transferencia de archivos basada en hash desde ISI hasta 20k de datos فئة GPU.
  - ملف التشغيل:
    - عقد الانتاج تنشئ prover عبر `fastpq_prover::Prover::canonical` الذي يهيئ دائما backend الانتاج؛ تمت ازالة simulacro de الحتمي. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) y `irohad --fastpq-execution-mode` يتيحان تثبيت تنفيذ CPU/GPU بشكل حتمي بينما يسجل observer hook ثلاثيات solicitado/resuelto/backend للتدقيقات. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetización:
  - KV-Update AIR: يعامل WSV كخريطة clave-valor نوعية ملتزمة عبر Poseidon2-SMT. كل ISI يتوسع الى مجموعة صغيرة من صفوف lectura-verificación-escritura على المفاتيح (cuentas, activos, roles, dominios, metadatos, suministro).
  - Código de operación del código de operación: AIR y selector del ISI (conservación, contadores monótonos, permisos, comprobaciones de rango, metadatos de conservación).
  - Argumentos de búsqueda: جداول شفافة ملتزمة بالهاش لصلاحيات/ادوار وprecicisions ومعلمات سياسة تتجنب قيود bit a bit الثقيلة.
- التزامات وتحديثات الحالة:
  - Prueba SMT agregada: جميع المفاتيح المتأثرة (pre/post) تثبت مقابل `old_root`/`new_root` باستخدام frontier مضغوط مع hermanos مكررة تمت ازالتها.- Invariantes: يتم فرض ثوابت عالمية (مثل اجمالي suministro لكل اصل) عبر مساواة multiset بين صفوف التأثير والعدادات المتعقبة.
- نظام الاثبات:
  - التزامات متعددة الحدود بأسلوب FRI (DEEP-FRI) مع arity عالية (8/16) y explosión 8-16؛ hashes Poseidon2؛ transcripción Fiat-Shamir مع SHA-2/3.
  - Recursividad: تجميع محلي داخل DS لضغط micro-lotes الى اثبات واحد لكل slot عند الحاجة.
- النطاق والامثلة:
  - Contenido: transferir, acuñar, grabar, registrar/anular el registro de definiciones de activos, establecer precisión (مقيد), establecer metadatos.
  - الحسابات/المجالات: crear/eliminar, establecer clave/umbral, agregar/eliminar firmantes (حالة فقط؛ فحص التواقيع يثبت من مدققي DS y AIR).
  - الادوار/الصلاحيات (ISI): otorgar/revocar roles y permisos؛ يتم فرضها عبر جداول búsquedas y comprobaciones سياسة monótonas.
  - العقود/AMX: علامات comenzar/confirmar للـ AMX, capacidad mint/revocar اذا كانت مفعلة؛ تثبت كتحولات حالة وعدادات سياسة.
- فحوصات خارج AIR للحفاظ على الكمون:
  - التواقيع والتشفير الثقيل (مثل تواقيع ML-DSA للمستخدمين) يتحقق منها مدققو DS ويثبتونها في DS QC؛ اثبات الصلاحية يغطي فقط اتساق الحالة والامتثال للسياسات. هذا يبقي pruebas PQ وسريعة.
- اهداف الاداء (تقريبية, CPU 32 pulgadas + GPU حديثة):
  - 20k ISI (<=8 teclas/ISI): ~0,4-0,9 s de almacenamiento ~150-450 KB de almacenamiento ~5-15 ms de almacenamiento.
  - ISI اثقل: micro-batch (مثلا 10x2k) + recursividad للحفاظ على <1 s لكل slot.
- Actualización del Manifiesto DS:
  - `zk.policy = "fastpq_isi"`- `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (التواقيع تتحقق عبر DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (افتراضي؛ البدائل يجب اعلانها صراحة)
- Opciones alternativas:
  - ISI المعقدة/المخصصة يمكنها استخدام STARK عام (`zk.policy = "stark_fri_general"`) مع اثبات مؤجل ونهائية 1s عبر atestación QC + corte على ادلة غير صحيحة.
  - خيارات غير PQ (مثل Plonk مع KZG) تتطلب configuración confiable y تعد مدعومة في build الافتراضي.

Cebador AIR (por Nexus)
- Trace التنفيذ: مصفوفة بعرض (اعمدة سجلات) وطول (خطوات). كل صف هو خطوة منطقية في معالجة ISI؛ Haga clic en pre/post, selectores y banderas.
- القيود:
  - قيود الانتقال: تفرض علاقات صف-الى-صف (مثلا post_balance = pre_balance - importe لصف خصم عند `sel_transfer = 1`).
  - قيود الحدود: تربط E/S públicas (old_root/new_root, contadores) بالصف الاول/الاخير.
  - Búsquedas/permutaciones: تضمن العضوية والمساواة متعددة المجموعات مقابل جداول ملتزمة (permisos, parámetros de activos) دون دوائر poco pesado.
- الالتزام y التحقق:
  - يقوم prover بالالتزام بالتراسات عبر ترميزات hash ويبني متعددات حدود منخفضة الدرجة صالحة اذا تحققت القيود.
  - Verificador de يتحقق من الدرجة المنخفضة عبر FRI (basado en hash, post-cuántico) مع فتحات Merkle قليلة؛ التكلفة لوغاريتمية بعدد الخطوات.
- مثال (Transferencia): تتضمن السجلات pre_balance, cantidad, post_balance, nonce, selectores. تفرض القيود عدم السلبية/المدى، الحفظ، ورتابة nonce, بينما تربط multi-proof SMT مجمعة الاوراق pre/post بـ root old/new.Aplicación ABI y syscalls (ABI v1)
- Llamadas al sistema لاضافتها (اسماء توضيحية):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- انواع puntero-ABI لاضافتها:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- تحديثات مطلوبة:
  - Use `ivm::syscalls::abi_syscall_list()` (مع الحفاظ على الترتيب), مع gating حسب السياسة.
  - ربط الارقام غير المعروفة بـ `VMError::UnknownSyscall` في hosts.
  - Funciones: lista de llamadas al sistema golden, hash ABI, ID de tipo de puntero goldens, y funciones.
  - Documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

نموذج الخصوصية
- Servicios de acceso: WSV y DS privados para obtener acceso a Internet.
- التعرض العام: يتم تصدير encabezados والتزامات DA y pruebas صلاحية PQ فقط.
- ادلة ZK اختيارية: يمكن لـ DS privado انتاج ادلة ZK (مثل كفاية الرصيد او امتثال السياسة) لتمكين عمليات cross-DS دون كشف الحالة الداخلية.
- التحكم بالوصول: يتم فرض التخويل عبر سياسات ISI/role داخل DS. tokens de capacidad اختيارية ويمكن اضافتها لاحقا.

عزل الاداء y QoS
- اجماع ومجموعات انتظار وتخزين منفصلة لكل DS.
- cuotas, programación, DS, nexus, anclajes y bloqueo de cabecera de línea.
- Utilice el servidor DS (cómputo/memoria/IO) en el host IVM. Hay DS públicos y DS privados.
- استدعاءات cross-DS متزامنة تتجنب انتظار متزامن طويل داخل تنفيذ private-DS.توفر البيانات وتصميم التخزين
1) ترميز المحو
- استخدام Reed-Solomon منهجي (مثلا GF(2^16)) لترميز المحو على مستوى blob لكتل Kura ولقطات WSV: معلمات `(k, m)` مع Fragmentos `n = k + m`.
- Registros (DS públicos): `k=32, m=16` (n=48) Almacenamiento de 16 fragmentos con un aumento de ~1.5x. للـ DS privado: `k=16, m=8` (n=24) ضمن المجموعة المصرح بها. كلاهما قابل للتهيئة عبر DS Manifest.
- Blobs العامة: fragmentos موزعة عبر عقد DA/مدققين عديدة مع فحوص توفر بالعينات. التزامات DA في encabezados تسمح clientes ligeros بالتحقق.
- Blobs الخاصة: fragmentos de مشفرة وموزعة فقط بين مدققي private-DS (او حراس معينين). السلسلة العالمية تحمل فقط التزامات DA (دون مواقع fragmentos او المفاتيح).

2) Muestreo y muestreo
- Blob: raíz de Merkle y fragmentos de `*_da_commitment`. حافظ على PQ بتجنب الالتزامات البيضوية.
- DA Attesters: attesters اقليميون يتم اختيارهم عبر VRF (مثلا 64 لكل منطقة) يصدرون شهادة ML-DSA-87 تؤكد الناجح. Atestación de الهدف لزمن <=300 ms. El nexo está conectado a fragmentos de fragmentos.

3) تكامل Kura
- الكتل تخزن اجسام المعاملات كـ blobs بترميز محو مع التزامات Merkle.
- encabezados تحمل التزامات blobs؛ Puede utilizar DA en DS público y DS privado.4) تكامل WSV
- Instantáneas WSV: incluye puntos de control de DS y instantáneas y encabezados. بين اللقطات يتم الحفاظ على cambiar registros. لقطات public توزع على نطاق واسع، ولقطات تبقى داخل مدققين خاصين.
- Acceso con prueba: يمكن للعقود تقديم (او طلب) ادلة حالة (Merkle/Verkle) مثبتة عبر التزامات snapshot. يمكن لـ DS privado تقديم شهادات conocimiento cero بدلا من ادلة خام.

5) poda y poda
- Poda de بدون للـ público DS: الاحتفاظ بكل اجسام Kura ولقطات WSV عبر DA (توسع افقي). يمكن لـ DS privado تحديد احتفاظ داخلي، لكن الالتزامات المصدرة تظل غير قابلة للتغيير. طبقة nexus تحتفظ بكل Nexus Bloques y artefactos para DS.

الشبكات وادوار العقد
- Contenido del paquete: يشاركون في اجماع nexus, يتحققون من Nexus Blocks and DS artefactos, ويجرون فحوص DA لـ public DS.
- Espacio de datos: يشغلون اجماع DS, ينفذون العقود، يديرون Kura/WSV المحلي، ويتولون DA لـ DS الخاصة بهم.
- عقد DA (اختياري): تخزن/تنشر blobs عامة وتدعم muestreo. في DS privado, عقد DA تكون متجاورة مع المدققين او حراس موثوقين.تحسينات واعتبارات على مستوى النظام
- Secuenciación/mempool: mempool DAG (narval) y BFT para el nexo, rendimiento y rendimiento del nexus. المنطقي.
- حصص DS والعدالة: حصص لكل DS في كل كتلة وحدود وزن لتجنب bloqueo de cabecera de línea y ضمان كمون متوقع لـ privado DS.
- اقرار DS (PQ): Hay quórum para DS تستخدم ML-DSA-87 (فئة Dilithium5) افتراضيا. هذا post-quantum واكبر من تواقيع EC لكنه مقبول بمعدل QC واحد لكل slot. Según DS ML-DSA-65/44 y EC según DS Manifest Y el DS público se encuentra en ML-DSA-87.
- Certificadores de DA: للـ DS públicos, استخدم certificadores اقليميين مختارين بـ VRF يصدرون شهادات DA. لجنة nexus تتحقق من الشهادات بدلا من sampling الخام؛ DS privado تحتفظ بشهاداتها الداخلية.
- Recursos de recursividad: يمكن تجميع عدة micro-lotes داخل DS في اثبات واحد لكل slot/epoch للحفاظ على حجم الاثبات وزمن التحقق مستقرا تحت الحمل العالي.
- Escalado de carril (عند الحاجة): اذا اصبحت اللجنة العالمية عنق زجاجة، قدم K carriles تسلسل متوازية مع دمج حتمي. هذا يحافظ على ترتيب عالمي واحد مع توسع افقي.
- تسريع حتمي: توفير kernels SIMD/CUDA مع feature flags للـ hash/FFT مع fallback CPU مطابق للبت للحفاظ على الحتمية عبر العتاد.- عتبات تفعيل carriles (اقتراح): تفعيل 2-4 carriles اذا (a) تجاوزت نهائية p95 مدة 1,2 s لاكثر من 3 دقائق متتالية، او (b) تجاوز اشغال كتلة 85% لاكثر من 5 دقائق، او (c) تطلب معدل المعاملات الداخل >1.2x من سعة الكتلة بشكل مستدام. Los carriles deben usar hash para DSID y conectarse al nexo.

الرسوم والاقتصاد (افتراضات اولية)
- Nombre del token: token para DS para computación/IO؛ الرسوم تدفع بعملة الغاز الاصلية للـ DS. التحويل بين DS مسؤولية التطبيق.
- اولوية الادراج: round-robin عبر DS مع حصص لكل DS للحفاظ على العدالة y SLOs 1s؛ داخل DS يمكن للمزايدة على الرسوم ان تفك التعادل.
- Configuración: مستقبلا استكشاف سوق رسوم عالمي او سياسات تقلل MEV دون تغيير الذرية او تصميم ادلة PQ.

سير عمل espacio de datos cruzado (مثال)
1) يرسل مستخدم معاملة AMX تمس public DS P و private DS S: نقل الاصل X من S الى المستفيد B الذي حسابه في P.
2) داخل slot، ينفذ P و S مقطعهم على instantánea. تحقق S من التفويض والتوفر وتحدث حالتها الداخلية وتنتج اثبات صلاحية PQ والتزام DA (بدون تسريب بيانات خاصة). يحضر P تحديث الحالة المقابل (مثل mint/burning/locking في P حسب السياسة) واثباته.
3) تتحقق لجنة nexus من كلا اثباتي DS y DA؛ اذا تحقق كلاهما خلال slot يتم التزام المعاملة ذرياً في كتلة Nexus ذات 1s y تحديث raíces لكل DS في متجه estado mundial العالمي.
4) اذا كان اي اثبات او شهادة DA مفقودا او غير صالح، تلغى المعاملة (بدون اثر) ويمكن للعميل اعادة ارسالها للـ ranura التالي. لا تغادر بيانات S الخاصة في اي خطوة.- اعتبارات الامان
- Actualización: llamadas al sistema desde IVM Hay un compromiso DS y AMX y una confirmación de confirmación.
- التحكم بالوصول: صلاحيات ISI في DS privado تقيد من يمكنه ارسال المعاملات y ما العمليات المسموحة. tokens de capacidad تشفر حقوقا دقيقة لاستخدام cross-DS.
- السرية: تشفير من الطرف للطرف لبيانات private-DS, shards بترميز محو مخزنة فقط لدى الاعضاء المصرح لهم، وادلة ZK اختيارية لاثباتات خارجية.
- مقاومة DoS: العزل على مستويات mempool/consensus/storage يمنع ازدحام public من تعطيل تقدم private DS.

تغييرات على مكونات Iroha
- iroha_data_model: Nombre `DataSpaceId` e ID de descriptores DS y AMX (módulos/tarjetas) y pruebas/DA. Utilice Norito.
- ivm: llamadas al sistema y tipos de puntero-ABI en AMX (`amx_begin`, `amx_commit`, `amx_touch`) y pruebas DA؛ تحديث اختبارات/توثيق ABI y لسياسة v1.