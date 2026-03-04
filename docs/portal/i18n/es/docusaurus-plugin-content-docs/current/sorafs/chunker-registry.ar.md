---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: registro fragmentador
título: سجل ملفات fragmentador في SoraFS
sidebar_label: fragmentador
descripción: معرفات الملفات والمعلمات وخطة التفاوض لسجل fragmenter في SoraFS.
---

:::nota المصدر المعتمد
Utilice el código `docs/source/sorafs/chunker_registry.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

## Bloqueador de piezas para SoraFS (SF-2a)

Utilice SoraFS para evitar la fragmentación del sistema.
Hay archivos CDC, semver, digest/multicodec, archivos de manifiestos y CAR.

على مؤلفي الملفات الرجوع إلى
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
للاطلاع على البيانات المطلوبة وقائمة التحقق وقالب المقترح قبل إرسال أي إدخال جديد.
وبعد أن تعتمد الحوكمة أي تغيير، اتبع
[قائمة تحقق إطلاق السجل](./chunker-registry-rollout-checklist.md) y
[دليل manifiesto في puesta en escena](./staging-manifest-playbook) لترقية
الـ accesorios إلى puesta en escena والإنتاج.

### الملفات

| Espacio de nombres | الاسم | SemVer | معرف الملف | الحد الأدنى (بايت) | الهدف (بايت) | الحد الأقصى (بايت) | قناع القطع | Multihash | البدائل | ملاحظات |
|-----------|-------|--------|------------|--------------------|--------------|--------------------|-----------|-----------|--------|---------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Accesorios para el hogar Accesorios para accesorios SF-1 |La configuración de la unidad es `sorafs_manifest::chunker_registry` (y [`chunker_registry_charter.md`](./chunker-registry-charter.md)). ويُعبَّر عن كل إدخال كـ `ChunkerProfileDescriptor` يضم:

* `namespace` – تجميع منطقي للملفات ذات الصلة (مثل `sorafs`).
* `name` – تسمية مقروءة للبشر (`sf1`, `sf1-fast`,…).
* `semver` – سلسلة نسخة دلالية لمجموعة المعلمات.
* `profile` – `ChunkProfile` Pantalla (min/objetivo/max/máscara).
* `multihash_code` – El multihash de la computadora digiere el fragmento (`0x1f`
  للإعداد الافتراضي في SoraFS).

Este manifiesto es `ChunkingProfileV1`. تسجل البنية بيانات السجل (espacio de nombres, nombre, semver) إلى جانب
معلمات CDC الخام وقائمة البدائل أعلاه. ينبغي على المستهلكين أولاً محاولة البحث في السجل عبر `profile_id`
والرجوع إلى المعلمات المضمّنة عند ظهور معرفات غير معروفة؛ تضمن قائمة البدائل استمرار العملاء HTTP في إرسال
La fuente de alimentación es `Accept-Chunker`. وتفرض قواعد ميثاق السجل أن يكون المقبض المعتمد
(`namespace.name@semver`) أول إدخال في `profile_aliases`, تليه أي بدائل قديمة.

Para obtener más información, consulte la CLI:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

Utilice la CLI para crear JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) المسار `-`, ما يبث الحمولة إلى stdout بدل إنشاء ملف. يسهل ذلك تمرير
البيانات إلى الأدوات مع الحفاظ على السلوك الافتراضي لطباعة التقرير الرئيسي.

### مصفوفة التوافق وخطة الإطلاقيلخص الجدول التالي حالة الدعم الحالية لـ `sorafs.sf1@1.0.0` عبر المكونات الأساسية. يشير "Puente"
Esta es la versión CARv1 + SHA-256 del sistema operativo (`Accept-Chunker` + `Accept-Digest`).

| المكون | الحالة | ملاحظات |
|--------|--------|---------|
| `sorafs_manifest_chunk_store` | ✅ مدعوم | Conecte el cable + el bloque y el bloque `--json-out=-` y el bloque `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ قديم | مُنشئ manifiesto قديم؛ استخدم `iroha app sorafs toolkit pack` لتغليف CAR/manifest وأبقِ `--plan=-` لإعادة التحقق الحتمية. |
| `sorafs_provider_advert_stub` | ⚠️ قديم | مساعد تحقق sin conexión فقط؛ يجب إنتاج anuncios de proveedores عبر خط أنابيب النشر والتحقق منها عبر `/v1/sorafs/providers`. |
| `sorafs_fetch` (organizador de desarrollo) | ✅ مدعوم | يقرأ `chunk_fetch_specs` y حمولة قدرة `range` y يجمع إخراج CARv2. |
| Accesorios para SDK (Rust/Go/TS) | ✅ مدعوم | يُعاد توليدها عبر `export_vectors`؛ المقبض المعتمد يظهر أولاً في كل قائمة بدائل ويُوقَّع بواسطة أظرف المجلس. |
| Puerta de enlace Torii | ✅ مدعوم | يطبق كامل قواعد `Accept-Chunker`, y ويتضمن ترويسات `Content-Chunker`, y ويعرض Bridge CARv1 para degradar la versión. |

طرح التليمترية:- **تليمترية جلب الـ chunks** — يصدر CLI الخاص بـ Iroha `sorafs toolkit pack` digests للـ fragment, وبيانات CAR, y جذور PoR لإدخالها في لوحات المتابعة.
- **Anuncios de proveedores** — تتضمن حمولة الإعلانات بيانات القدرات والبدائل؛ Utilice el botón `/v1/sorafs/providers` (y el botón `range`).
- **مراقبة الـ gateway** — على المشغلين الإبلاغ عن أزواج `Content-Chunker`/`Content-Digest` لاكتشاف أي خفض غير متوقع؛ Y من المتوقع أن ينخفض ​​استخدام الـ bridge إلى الصفر قبل الإيقاف.

سياسة الإيقاف: بعد اعتماد ملف خلف، حدّد نافذة نشر مزدوجة (موثقة في المقترح) قبل وسم
`sorafs.sf1@1.0.0` بأنه متوقف ضمن السجل وإزالة Bridge CARv1 من بوابات الإنتاج.

Para obtener un PoR de un fragmento/segmento/hoja y de otro de los siguientes elementos:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

يمكنك اختيار ملف عبر معرف رقمي (`--profile-id=1`) أو عبر المقبض المسجل
(`--profile=sorafs.sf1@1.0.0`) صيغة المقبض مناسبة للسكريبتات التي تمرر espacio de nombres/nombre/semver
مباشرة من بيانات الحوكمة.

Formato `--promote-profile=<handle>` de código JSON para el sistema operativo (que no funciona correctamente)
المسجلة) يمكن لصقها في `chunker_registry_data.rs` عند ترقية ملف افتراضي جديد:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

يتضمن التقرير الرئيسي (وملف البرهان الاختياري) الـ digest الجذري، وbytes الأوراق المُعاينة
(مشفرة بالهيكس)، وdigests الأشقاء للـ segment/chunk بحيث يمكن للمدققين إعادة هاش طبقات
64 KiB/4 KiB Unidad de disco duro `por_root_hex`.للتحقق من برهان موجود مقابل حمولة، مرر المسار عبر
`--por-proof-verify` (تضيف CLI الحقل `"por_proof_verified": true` عندما يتطابق الشاهد مع الجذر المحسوب):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

لأخذ عينات على دفعات، استخدم `--por-sample=<count>` ويمكنك تمرير seed/مسار إخراج اختياري.
تضمن CLI ترتيبًا حتميًا (`splitmix64` sembrado) y بالاقتطاع تلقائيًا عندما تتجاوز
الطلبات الأوراق المتاحة:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

يعكس manifest stub البيانات نفسها، وهو مناسب عند برمجة اختيار `--chunker-profile-id` في
الـ pipelines. كما تقبل CLIs الخاصة بـ chunk store صيغة المقبض المعتمد
(`--profile=sorafs.sf1@1.0.0`) لتجنب ترميز معرفات رقمية صلبة في سكريبتات البناء:

```
$ ejecución de carga -p sorafs_manifest --bin sorafs_manifest_stub --list-chunker-profiles
[
  {
    "id_perfil": 1,
    "espacio de nombres": "sorafs",
    "nombre": "sf1",
    "semver": "1.0.0",
    "manejar": "sorafs.sf1@1.0.0",
    "tamaño_mínimo": 65536,
    "tamaño_objetivo": 262144,
    "tamaño_máximo": 524288,
    "break_mask": "0x0000ffff",
    "código_multihash": 31
  }
]
```

يتطابق الحقل `handle` (`namespace.name@semver`) مع ما تقبله CLIs عبر
`--profile=…`، مما يجعل نسخه مباشرة إلى الأتمتة آمنًا.

### التفاوض على chunkers

تعلن البوابات والعملاء الملفات المدعومة عبر provider adverts:

```
ProveedorAdvertBodyV1 {
    ...
    chunk_profile: perfil_id (ضمنيًا عبر السجل)
    capacidades: [...]
}
```

يُعلن جدولة chunk متعددة المصادر عبر القدرة `range`. يقبلها CLI عبر
`--capability=range[:streams]` حيث يشفّر اللاحق الرقمي الاختياري التوازي المفضل لدى المزود
لجلب النطاق (على سبيل المثال، `--capability=range:64` تعلن ميزانية 64 stream).
وعند حذفه، يعود المستهلكون إلى تلميح `max_streams` العام المنشور في مكان آخر من الإعلان.

عند طلب بيانات CAR، يجب أن يرسل العملاء ترويسة `Accept-Chunker` بقائمة tuples
`(namespace, name, semver)` بحسب ترتيب التفضيل:

```

تختار البوابات ملفًا مدعومًا من الطرفين (الافتراضي `sorafs.sf1@1.0.0`) وتعكس القرار عبر ترويسة
Nombre `Content-Chunker`. تضمن manifiesta الملف المختار كي تتمكن العقد اللاحقة من التحقق
Estos fragmentos están conectados a HTTP.

### COCHE

Utilice el manifiesto CIDv1 de `dag-cbor` (`0x71`). وللتوافق القديم نحتفظ
Programación CARv1+SHA-2:* **المسار الأساسي** – CARv2, resumen de BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`، والملف مسجل كما سبق.
  هذا الخيار عندما يهمل العميل `Accept-Chunker` أو يطلب `Accept-Digest: sha2-256`.

لكن يجب ألا تستبدل الـ digest المعتمد.

### المطابقة

* يطابق ملف `sorafs.sf1@1.0.0` الـ accesorios العامة في
  `fixtures/sorafs_chunker` Otras funciones
  `fuzz/sorafs_chunker`. Utilice Rust, Go y Node para crear aplicaciones.
* Utilice `chunker_registry::lookup_by_profile` para conectar los dispositivos `ChunkProfile::DEFAULT` a sus terminales.
* تتضمن manifiesta الناتجة عن `iroha app sorafs toolkit pack` y `sorafs_manifest_stub` بيانات السجل.