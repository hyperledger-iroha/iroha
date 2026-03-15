---
lang: es
direction: ltr
source: docs/portal/docs/da/threat-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota المصدر القياسي
Aquí `docs/source/da/threat_model.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# نموذج تهديدات توفر البيانات في Sora Nexus

_اخر مراجعة: 2026-01-19 -- المراجعة القادمة المجدولة: 2026-04-19_

وتيرة الصيانة: Grupo de trabajo sobre disponibilidad de datos (<=90 يوما). يجب ان تظهر كل
Asegúrese de que el `status.md` esté encendido y apagado.

## الهدف والنطاق

Disponibilidad de datos (DA) de Taikai y blobs Nexus y blobs
الحوكمة قابلة للاسترجاع تحت اخطاء بيزنطية, وشبكات، ومشغلين. هذا النموذج
يثبت عمل الهندسة لDA-1 (البنية ونموذج التهديدات) ويعد خط اساس لمهام DA اللاحقة
(DA-2 حتى DA-10).

المكونات ضمن النطاق:
- Ingesta de Torii y metadatos Norito.
- Establece blobs como SoraFS (caliente/frío) y otros.
- تعهدات كتل Nexus (formatos de conexión, pruebas, API de cliente ligero).
- aplicación de ganchos لPDP/PoTR الخاصة بحمولات DA.
- تدفقات المشغل (fijación, desalojo, corte) وخطوط الرصد.
- موافقات الحوكمة التي تقبل او تزيل مشغلي DA والمحتوى.

خارج النطاق لهذا المستند:
- نمذجة الاقتصاد الكاملة (ضمن مسار DA-7).
- بروتوكولات SoraFS الاساسية المغطاة في نموذج تهديدات SoraFS.
- ارغونوميا SDK للعميل خارج اعتبارات سطح التهديد.

## نظرة عامة معمارية1. **الارسال:** يرسل العملاء blobs عبر API ingest لDA في Torii. تقوم العقدة
   blobs, y manifiestos Norito (blob, carril, época, códec)
   Hay trozos en el interior SoraFS.
2. **الاعلان:** تنتشر نوايا pin وتلميحات التكرار الى مزودي التخزين عبر
   registro (mercado SoraFS) مع وسوم سياسة تحدد اهداف الاحتفاظ caliente/frío.
3. **الالتزام:** Los secuenciadores incluyen blobs Nexus (CID + KZG)
   في الكتلة القياسية. يعتمد clientes ligeros على hash الالتزام ymetadatos
   المعلنة للتحقق من disponibilidad.
4. **التكرار:** تسحب عقد التخزين الحصص/chunks المخصصة، وتلبي تحديات PDP/PoTR،
   وتقوم بترقية البيانات بين طبقات frío y caliente حسب السياسة.
5. **الجلب:** يسترجع المستهلكون البيانات عبر SoraFS او بوابات DA-aware مع
   التحقق من pruebas ورفع طلبات اصلاح عند اختفاء النسخ.
6. **الحوكمة:** يوافق البرلمان ولجنة الاشراف على DA على المشغلين وجداول alquiler
   Aplicación de la ley. تحفظ ادوات الحوكمة عبر نفس مسار DA لضمان شفافية
   العملية.

## الاصول والمالكون

مقياس الاثر: **حرج** يكسر سلامة/حيوية الدفتر؛ **عال** يحجب relleno DA او
العملاء؛ **متوسط** يخفض الجودة لكنه قابل للاسترجاع؛ **منخفض** اثر محدود.| الاصل | الوصف | السلامة | الاتاحة | السرية | المالك |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (fragmentos + manifiestos) | Blobs Taikai y lane وادوات الحوكمة المخزنة في SoraFS | حرج | حرج | متوسط ​​| DA WG / Equipo de almacenamiento |
| Manifiestos Norito DA | Metadatos مصنفة تصف blobs | حرج | عال | متوسط ​​| Grupo de Trabajo sobre el Protocolo Básico |
| تعهدات الكتل | CID + Tarjeta KZG desde Nexus | حرج | عال | منخفض | Grupo de Trabajo sobre el Protocolo Básico |
| جداول PDP/PoTR | وتيرة aplicación de la ley لنسخ DA | عال | عال | منخفض | Equipo de almacenamiento |
| سجل المشغلين | Artículos de tocador y artículos de tocador | عال | عال | منخفض | Consejo de Gobierno |
| Alquiler de muebles y muebles | قيود الدفتر لعائدات DA Y والعقوبات | عال | متوسط ​​| منخفض | Grupo de Trabajo de Tesorería |
| لوحات الرصد | SLO DA وعمق التكرار yالتنبيهات | متوسط ​​| عال | منخفض | SRE / Observabilidad |
| Nueva Zelanda | Piezas de repuesto para el hogar trozos | متوسط ​​| متوسط ​​| منخفض | Equipo de almacenamiento |

## الخصوم والقدرات| الفاعل | القدرات | الدوافع | الملاحظات |
| --- | --- | --- | --- |
| عميل خبيث | Los blobs que aparecen en los manifiestos de DoS o la ingesta de DoS. | تعطيل بث Taikai, حقن بيانات غير صحيحة. | لا مفاتيح مميزة. |
| عقدة تخزين بيزنطية | اسقاط نسخ مخصصة، تزوير pruebas PDP/PoTR، تواطؤ. | تقليص احتفاظ DA، تجنب rent, احتجاز البيانات. | يحمل بيانات اعتماد مشغل صحيحة. |
| secuenciador مخترق | حذف تعهدات، ازدواجية كتل، اعادة ترتيب metadatos. | اخفاء ارسال DA، خلق عدم اتساق. | محدود باغلبية الاجماع. |
| مشغل داخلي | اساءة استخدام الوصول للحوكمة، العبث بسياسات الاحتفاظ، تسريب بيانات اعتماد. | مكسب اقتصادي، تخريب. | وصول الى بنية frío/calor. |
| خصم شبكة | تقسيم العقد، تاخير التكرار، حقن حركة MITM. | خفض الاتاحة, تدهور SLO. | لا يكسر TLS لكنه يمكنه اسقاط/ابطاء الروابط. |
| مهاجم الرصد | العبث باللوحات/التنبيهات، اخفاء الحوادث. | اخفاء انقطاعات DA. | يتطلب وصولا لخط التليمترية. |

## حدود الثقة- **حد الدخول:** العميل الى امتداد DA في Torii. يتطلب auth على مستوى الطلب،
  تحديد المعدل، والتحقق من carga útil.
- **حد التكرار:** عقد التخزين تتبادل fragmentos y pruebas. العقد متصادقة لكنها قد
  تتصرف بشكل بيزنطي.
- **حد الدفتر:** بيانات الكتلة الملتزمة مقابل التخزين خارج السلسلة. الاجماع
  يحمي السلامة، لكن الاتاحة تتطلب aplicación de la ley خارج السلسلة.
- **حد الحوكمة:** قرارات Consejo/Parlamento لاعتماد المشغلين والميزانيات
  والعقوبات. الاختراق هنا يؤثر مباشرة على نشر DA.
- **حد الرصد:** جمع métricas/registros المصدرة الى لوحات/تنبيهات. العبث يخفي
  الانقطاعات او الهجمات.

## سيناريوهات التهديد والضوابط

### هجمات مسار ingerir

**السيناريو:** عميل خبيث يرسل payloads Norito تالفة او blobs كبيرة جدا لاستنزاف
الموارد او تمرير metadatos غير صحيحة.

**الضوابط**
- تحقق صارممن esquema Norito مع تفاوض الاصدار؛ رفض الاعلام غير المعروفة.
- Realice una ingesta de punto final mediante Torii.
- Tamaño del fragmento y tamaño del fragmento SoraFS.
- خط القبول لا يحفظ manifiesta الا بعد تطابق suma de comprobación السلامة.
- Reproducción de caché حتمي (`ReplayCache`) يتتبع نوافذ `(lane, epoch, sequence)`،
  يحفظ marcas de límite superior على القرص، ويرفض التكرار/اعادة التشغيل القديمة؛
  aprovecha la propiedad وfuzz تغطي huellas dactilares مختلفة وارسال خارج الترتيب.
  [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]**الثغرات المتبقية**
- يجب ان يمر Torii ingest replay cache في مسار القبول ويحفظ مؤشرات secuencia عبر
  اعادة التشغيل.
- Adaptador Norito DA para arnés de fuzz (`fuzz/da_ingest_schema.rs`)
  ثوابت codificar/decodificar؛ يجب ان تنبه لوحات التغطية عند التراجع.

### احتجاز التكرار

**السيناريو:** مشغلو التخزين البيزنطيون يقبلون مهام pin لكنهم يسقطون trozos,
ويمررون تحديات PDP/PoTR عبر ردود مزورة او تواطؤ.

**الضوابط**
- جدول تحديات PDP/PoTR يمتد لحمولات DA مع تغطية لكل época.
- تكرار متعدد المصادر مع عتبات quorum؛ orquestador يكتشف fragmentos المفقودة
  ويطلق اصلاحات.
- Recortar حوكمي مرتبط بفشل pruebas والنسخ المفقودة.
- مهمة مصالحة تلقائية (`cargo xtask da-commitment-reconcile`) تقارن recibos
  Compromisos DA (SignedBlockWire, `.norito`, y JSON) y paquete JSON
  للحوكمة، وتفشل عند فقدان/عدم تطابق entradas لكي يطلق Alertmanager تنبيهات.

**الثغرات المتبقية**
- arnés المحاكاة في `integration_tests/src/da/pdp_potr.rs` (مغطى عبر
  `integration_tests/tests/da/pdp_potr_simulation.rs`) يختبر سيناريوهات تواطؤ
  وتقسيم، ويتحقق ان جدول PDP/PoTR يكتشف السلوك البيزنطي بشكل حتمي. استمر في
  توسيعه مع DA-5 لتغطية اسطح prueba الجديدة.
- سياسة desalojo لطبقة frío تحتاج سجل تدقيق موقع لمنع الاسقاطات الخفية.

### العبث بالتعهدات

**السيناريو:** secuenciador مخترق ينشر كتل تحذف او تغير تعهدات DA، ما يسبب فشل
buscar او عدم اتساق لدى clientes ligeros.**الضوابط**
- الاجماع يطابق مقترحات الكتل مع قوائم ارسال DA؛ النظراء يرفضون المقترحات
  التي تفتقد تعهدات مطلوبة.
- clientes ligeros يتحققون من pruebas de inclusión قبل اظهار maneja للfetch.
- سجل تدقيق يقارن recibos الارسال بتعهدات الكتل.
- مهمة مصالحة تلقائية (`cargo xtask da-commitment-reconcile`) تقارن recibos
  Compromisos DA (SignedBlockWire, `.norito`, y JSON) y paquete JSON
  للحوكمة، وتفشل عند فقدان/عدم تطابق entradas لكي يطلق Alertmanager تنبيهات.

**الثغرات المتبقية**
- مغطاة بمهمة المصالحة + gancho Alertmanager؛ Paquete de paquetes JSON
  كافتراضي.

### تقسيم الشبكة والرقابة

**السيناريو:** خصم يقسم شبكة التكرار، مانعا العقد من الحصول على trozos
المخصصة او الرد على تحديات PDP/PoTR.

**الضوابط**
- متطلبات مزودين multirregión تضمن مسارات شبكة متنوعة.
- No hay fluctuaciones ni retrocesos en las aplicaciones.
- لوحات الرصد تراقب عمق التكرار ونجاح التحديات وزمن buscar مع عتبات تنبيه.

**الثغرات المتبقية**
- محاكاة التقسيم لاحداث Taikai المباشرة ما زالت مفقودة؛ نحتاج pruebas de remojo.
- سياسة حجز سعة اصلاح لم توثق بعد.

### اساءة داخلية

**السيناريو:** مشغل لديه وصول للregistry يتلاعب بسياسات الاحتفاظ، ويمنح
Lista blanca de لمزودين خبيثين، او يخفي التنبيهات.**الضوابط**
- اعمال الحوكمة تتطلب تواقيع متعددة الاطراف وسجلات Norito موثقة.
- تغييرات السياسة تصدر احداثا للرصد وسجلات ارشيف.
- Esta es la opción Norito de solo agregar para encadenamiento de hash.
- اتمتة مراجعة الوصول ربع السنوية (`cargo xtask da-privilege-audit`) تتفقد
  ادلة manifest/replay (مع مسارات يحددها المشغلون), وتعلم العناصر المفقودة/
  غير-دليل/قابلة للكتابة عالميا، وتنتج paquete JSON موقع للوحات الحوكمة.

**الثغرات المتبقية**
- ادلة العبث في اللوحات تحتاج instantáneas موقعة.

## سجل المخاطر المتبقية| الخطر | الاحتمال | الاثر | المالك | خطة التخفيف |
| --- | --- | --- | --- | --- |
| Repetir manifiestos DA y secuencia de caché en DA-2 | ممكن | متوسط ​​| Grupo de Trabajo sobre el Protocolo Básico | تنفيذ caché de secuencia + تحقق nonce في DA-2؛ اضافة اختبارات تراجع. |
| تواطؤ PDP/PoTR عند اختراق >f عقد | غير محتمل | عال | Equipo de almacenamiento | اشتقاق جدول تحديات جديد مع muestreo entre proveedores؛ التحقق عبر arnés المحاكاة. |
| فجوة تدقيق desalojo في طبقة frío | ممكن | عال | SRE / Equipo de Almacenamiento | ربط سجلات موقعة ووصولات on-chain لعمليات desalojo؛ الرصد عبر اللوحات. |
| زمن اكتشاف حذف secuenciador | ممكن | عال | Grupo de Trabajo sobre el Protocolo Básico | `cargo xtask da-commitment-reconcile` ليلي يقارن recibos y compromisos (SignedBlockWire/`.norito`/JSON) y غير متطابقة. |
| مقاومة التقسيم لبث Taikai المباشر | ممكن | حرج | Redes TL | تنفيذ تدريبات تقسيم؛ حجز سعة اصلاح؛ توثيق SOP للفشل. |
| Artículos relacionados | غير محتمل | عال | Consejo de Gobierno | `cargo xtask da-privilege-audit` ربع سنوي (dirs manifest/replay + مسارات اضافية) مع JSON موقع + gate لوحة؛ تثبيت artefactos التدقيق على السلسلة. |

## المتابعات المطلوبة1. Introduzca Norito para ingerir DA y مثلة (منقولة الى DA-2).
2. تمرير caché de reproducción عبر Torii DA ingesta y مؤشرات secuencia عبر اعادة التشغيل.
3. **مكتمل (2026-02-05):** arnés محاكاة PDP/PoTR يمارس تواطؤ + تقسيم مع نمذجة
   QoS pendiente راجع `integration_tests/src/da/pdp_potr.rs` (مع اختبارات في
   `integration_tests/tests/da/pdp_potr_simulation.rs`) للتنفيذ yالملخصات
   الحتمية الملتقطة ادناه.
4. **مكتمل (2026-05-29):** `cargo xtask da-commitment-reconcile` يقارن recibos
   Compromisos DA (SignedBlockWire/`.norito`/JSON)
   `artifacts/da/commitment_reconciliation.json`, y مربوط بAlertmanager/حزم
   الحوكمة لتنبيهات omisión/manipulación (`xtask/src/da.rs`).
5. **مكتمل (2026-05-29):** `cargo xtask da-privilege-audit` يتجول في carrete
   manifiesto/repetición (مع مسارات يحددها المشغلون), ويحدد العناصر المفقودة/غير
   دليل/قابلة للكتابة عالميا، وينتج paquete JSON موقع للحوكمة
   (`artifacts/da/privilege_audit.json`), مغلقا فجوة اتمتة مراجعة الوصول.

**اين تتابع:**- caché de reproducción واستمرار المؤشرات تم انجازهما في DA-2. راجع التنفيذ في
  `crates/iroha_core/src/da/replay_cache.rs` (caché externo) y Torii aquí
  `crates/iroha_torii/src/da/ingest.rs` الذي يمرر verifica la huella digital عبر `/v2/da/ingest`.
- محاكاة streaming PDP/PoTR تمارس عبر arnés prueba-stream في
  `crates/sorafs_car/tests/sorafs_cli.rs`, y تغطي تدفقات طلب PoR/PDP/PoTR y سيناريوهات
  الفشل المشار اليها في نموذج التهديدات.
- نتائج capacidad وreparación remojo موجودة في
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, بينما مصفوفة remojo Sumeragi
  الاوسع يتم تتبعها في `docs/source/sumeragi_soak_matrix.md` (مع نسخ محلية).
  هذه artefactos توثق التدريبات الطويلة المذكورة في سجل المخاطر.
- اتمتة المصالحة + privilegio-auditoría موجودة في `docs/automation/da/README.md`
  والاوامر الجديدة `cargo xtask da-commitment-reconcile` /
  `cargo xtask da-privilege-audit`; استخدم المخارج الافتراضية تحت
  `artifacts/da/` Asegúrese de que el dispositivo esté limpio.

## ادلة المحاكاة ونمذجة QoS (2026-02)

لاغلاق متابعة DA-1 #3, قمنا بتكويد arnés محاكاة PDP/PoTR حتمي تحت
`integration_tests/src/da/pdp_potr.rs` (مغطي بواسطة
`integration_tests/tests/da/pdp_potr_simulation.rs`). يقوم arnés بتوزيع العقد
على ثلاث مناطق، ويحقن التقسيم/التواطؤ حسب احتمالات roadmap, ويتتبع تاخر PoTR،
ويغذي نموذج atraso للاصلاح يعكس ميزانية اصلاح طبقة caliente. تشغيل السيناريو
الافتراضي (12 épocas, 18 تحديا PDP + نافذتان PoTR لكل época) انتج المقاييس
التالية:<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| المقياس | القيمة | الملاحظات |
| --- | --- | --- |
| Fallos de PDP detectados | 48 / 49 (98,0%) | ما زالت particiones تطلق الاكتشاف؛ فشل واحد غير مكتشف ناتج عن jitter صادق. |
| Latencia media de detección de PDP | 0,0 épocas | يتم اظهار الاعطال ضمن época الاصلي. |
| Fallos PoTR detectados | 28/77 (36,4%) | يتم اطلاق الاكتشاف عند فقدان العقدة >=2 نوافذ PoTR, مما يترك معظم الاحداث في سجل المخاطر المتبقية. |
| Latencia media de detección de PoTR | 2.0 épocas | يطابق عتبة تاخر مقدارها epochين مضمنة في تصعيد الارشفة. |
| Pico de cola de reparación | 38 manifiestos | يرتفع backlog عندما تتكدس particiones اسرع من اربع اصلاحات متاحة لكل epoch. |
| Latencia de respuesta p95 | 30.068 ms | La configuración es de 30 grados de fluctuación +/-75 ms para mejorar la calidad del servicio (QoS). |
<!-- END_DA_SIM_TABLE -->

هذه المخرجات تقود الان نماذج لوحات DA وتفي بمعايير قبول "arnés de simulación +
Modelado de QoS" المشار اليها في hoja de ruta.

الان توجد الاتمتة خلف
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
Más información sobre el arnés y el código Norito JSON
`artifacts/da/threat_model_report.json` افتراضيا. تستخدم الوظائف الليلية هذا
الملف لتحديث المصفوفات في هذا المستند والتنبيه عند انحراف معدلات الاكتشاف
او طوابير الاصلاح او عينات QoS.

لتحديث الجدول اعلاه للوثائق، شغل `make docs-da-threat-model`، والذي يستدعي
`cargo xtask da-threat-model-report` Más información
`docs/source/da/_generated/threat_model_report.json` ويعيد كتابة هذا القسم عبر
`scripts/docs/render_da_threat_model_tables.py`. يتم تحديث نسخة `docs/portal`
(`docs/portal/docs/da/threat-model.md`) في نفس التمرير لكي تبقى النسختان
متزامنتين.