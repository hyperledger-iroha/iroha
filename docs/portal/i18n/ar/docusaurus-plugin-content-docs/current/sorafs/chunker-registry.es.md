---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: سجل المقطع
العنوان: سجل ملفات التعريف SoraFS
Sidebar_label: سجل القطع
الوصف: معرفات الملف الشخصي والمعلمات وخطة التداول لسجل القطع SoraFS.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/chunker_registry.md`. حافظ على النسخ المتزامنة حتى يتم سحب مجموعة وثائق Sphinx المتوارثة.
:::

## سجل ملفات تعريف القطع SoraFS (SF-2a)

تتعامل حزمة SoraFS مع طريقة التقطيع عبر سجل صغير مع مساحات من الأسماء.
يتم تعيين كل ملف شخصي لمعلمات محددات CDC والبيانات الوصفية والملخص/الترميز المتعدد المستخدم في البيانات وأرشيفات CAR.

يحتاج أصحاب الملفات الشخصية إلى استشارة
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
بالنسبة للبيانات التعريفية المطلوبة، وقائمة التحقق من الصحة، وزرع الزرع قبل إرسال المدخلات الجديدة.
بمجرد أن تبدأ الحكومة في إجراء تغيير، بعد ذلك
[قائمة التحقق من بدء تشغيل السجل](./chunker-registry-rollout-checklist.md) والاختيار
[دليل التشغيل للبيان المرحلي](./staging-manifest-playbook) للترويج
يتم عرض التركيبات وإنتاجها.

### الملفات الشخصية| مساحة الاسم | الاسم | سيمفير | معرف الملف الشخصي | دقيقة (بايت) | الهدف (بايت) | ماكس (بايت) | ماسكارا دي كورتي | مولتيهاش | الاسم المستعار | نوتاس |
|-----------|--------|--------|------------|----------------|-------------|-----------------|------------|-----|-|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | الملف الشخصي المستخدم في التركيبات SF-1 |

السجل حي في الكود مثل `sorafs_manifest::chunker_registry` (يخضع لـ [`chunker_registry_charter.md`](./chunker-registry-charter.md)). كل مدخل
يتم التعبير عنه كـ `ChunkerProfileDescriptor` مع:

* `namespace` – مجموعة منطقية من الملفات الشخصية المرتبطة (ص. على سبيل المثال، `sorafs`).
* `name` – آداب مقروءة للإنسان (`sf1`، `sf1-fast`، …).
* `semver` – سلسلة الإصدار الدلالي لمجموعة المعلمات.
* `profile` – el `ChunkProfile` الحقيقي (الحد الأدنى/الهدف/الحد الأقصى/القناع).
* `multihash_code` – التجزئة المتعددة المستخدمة لإنتاج خلاصات القطع (`0x1f`
  للإعداد الافتراضي SoraFS).الملفات الشخصية التسلسلية الواضحة متوسطة `ChunkingProfileV1`. سجل البنية التحتية
بيانات تعريف السجل (مساحة الاسم، الاسم، الفصل) جنبًا إلى جنب مع معلمات CDC
في البداية وقائمة الأسماء المستعارة تظهر. سيحاول المستهلكون شراء واحدة
ابحث عن السجل عبر `profile_id` وكرر المعلمات المضمنة عندما
معرفات aparezcan desconocidos؛ تضمن قائمة الأسماء المستعارة أن عملاء HTTP يمكنهم الوصول إليها
يتبع ذلك التعامل مع الموروثين في `Accept-Chunker` بدون أي ضرر. لاس ريغلاس دي لا
تتطلب بطاقة السجل أن يكون المقبض الكنسي (`namespace.name@semver`) موجودًا
تم الإدخال الأول في `profile_aliases`، يتبعه أي اسم مستعار موجود.

لفحص السجل من الأدوات، قم بتشغيل مساعد CLI:

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

جميع علامات CLI التي تكتب JSON (`--json-out`، `--por-json-out`، `--por-proof-out`،
`--por-sample-out`) يقبل `-` كطريق، حيث يتم نقل الحمولة إلى مكان قياسي
إنشاء ملف. يسهل هذا إمكانية تحويل البيانات إلى الأدوات أثناء الحفاظ عليها
سلوك العيب في طباعة التقرير الرئيسي.

### ساحة الطرح وخطة النشر


تلتقط اللوحة التالية الحالة الفعلية للدعم لـ `sorafs.sf1@1.0.0` en
المكونات الأساسية. يتم إعادة توجيه "الجسر" إلى Carril CARv1 + SHA-256
الذي يتطلب مفاوضات صريحة مع العميل (`Accept-Chunker` + `Accept-Digest`).| مكون | حالة | نوتاس |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ سوبورتادو | التحقق من صحة المقبض القانوني + الاسم المستعار، وإرسال التقارير عبر `--json-out=-`، وتطبيق بطاقة التسجيل باستخدام `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ اعتزال | مُنشئ البيان القوي للرياضة؛ استخدم `iroha app sorafs toolkit pack` لتغليف السيارة/البيان والحفاظ على `--plan=-` لإعادة التحقق المحدد. |
| `sorafs_provider_advert_stub` | ⚠️ اعتزال | مساعد التحقق من الصحة دون اتصال بالإنترنت حصريًا؛ يجب أن يتم إنتاج إعلانات الموفر من خلال خط أنابيب النشر والتحقق من صحتها عبر `/v2/sorafs/providers`. |
| `sorafs_fetch` (منسق المطور) | ✅ سوبورتادو | Lee `chunk_fetch_specs`، يستوعب حمولات السعة `range` ويخرج CARv2. |
| تركيبات SDK (الصدأ/الذهاب/TS) | ✅ سوبورتادو | التجديد عبر `export_vectors`; يظهر المقبض الكنسي لأول مرة في كل قائمة من الأسماء المستعارة وقد تم تثبيته من خلال المشورة. |
| تداول الملفات الشخصية عبر البوابة Torii | ✅ سوبورتادو | قم بتنفيذ القواعد الكاملة لـ `Accept-Chunker`، بما في ذلك الرؤوس `Content-Chunker` وشرح الجسر CARv1 فقط من خلال طلبات التخفيض الواضحة. |

شرح القياس عن بعد:- **القياس عن بعد لجلب القطع** — يصدر CLI لـ Iroha `sorafs toolkit pack` ملخصات للقطعة والبيانات الوصفية CAR ومصادر PoR لاستيعابها في لوحات المعلومات.
- **إعلانات الموفر** — تتضمن حمولات الإعلانات بيانات وصفية لقدرات واسم مستعار؛ التحقق من صحة التغطية عبر `/v2/sorafs/providers` (ص. على سبيل المثال، وجود السعة `range`).
- **مراقبة البوابة** — يجب على المشغلين الإبلاغ عن الأسماء `Content-Chunker`/`Content-Digest` لاكتشاف التخفيضات غير المتوقعة؛ كنت آمل أن يكون استخدام الجسر قد توقف تمامًا قبل التخفيض.

سياسة الإهمال: عندما تصادق على ملف شخصي لاحق، قم ببرمجة نافذة نشر مزدوجة
(موثق في العرض) قبل وضع علامة `sorafs.sf1@1.0.0` كما تم إهماله في السجل وإزالة
Bridge CARv1 de los gates en producción.

لفحص اختبار أداء محدد، وتقسيم مؤشرات القطعة/القطاع/اليوم واختياريًا
الاستمرار في تجربة الديسكو:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

يمكنك تحديد ملف شخصي بالمعرف الرقمي (`--profile-id=1`) أو من خلال مقبض التسجيل
(`--profile=sorafs.sf1@1.0.0`)؛ إن الشكل مع المقبض مناسب للنصوص البرمجية
قم بتمرير مساحة الاسم/الاسم/كل مرة مباشرة من بيانات تعريف الإدارة.

Usa `--promote-profile=<handle>` لإصدار كتلة بيانات تعريف JSON (بما في ذلك جميع الأسماء المستعارة)
المسجلين) يمكنك الاتصال بـ `chunker_registry_data.rs` للترويج لملف جديد بسبب الخلل:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```يتضمن التقرير الرئيسي (وملف الاختبار الاختياري) الملخص ووحدات البايت التي يتم قراءتها
(المشفرة بالست عشري) والملخصات مقسمة إلى أجزاء/قطعة حتى يتمكن المدققون من إعادة صياغتها
سعة 64 كيلو بايت/4 كيلو بايت مقابل القيمة `por_root_hex`.

للتحقق من وجود اختبار موجود مقابل حمولة، قم بتمرير المسار عبر
`--por-proof-verify` (CLI يضيف `"por_proof_verified": true` عند الاختبار
يتزامن مع حساب رايز):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

بالنسبة للمزارعين في الكثير، الولايات المتحدة الأمريكية `--por-sample=<count>` وتخصيصًا لطريقة البذور/اللعاب.
يضمن CLI أمرًا محددًا (مدرجًا مع `splitmix64`) ومقطعًا بشكل شفاف عندما
الطلب الكبير على الأدوات المتاحة:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

El manifest stub refleja los mismos datos, lo que es conveniente al automatizar la selección de
`--chunker-profile-id` en pipelines. Ambos CLIs de chunk store también aceptan la forma de handle canónico
(`--profile=sorafs.sf1@1.0.0`) para que los scripts de build puedan evitar hard-codear IDs numéricos:

```
تشغيل البضائع $ -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1،
    "مساحة الاسم": "سوراف"،
    "الاسم": "sf1"،
    "نصف": "1.0.0"،
    "المقبض": "sorafs.sf1@1.0.0"،
    "الحد الأدنى للحجم": 65536،
    "حجم_الهدف": 262144,
    "الحجم الأقصى": 524288،
    "break_mask": "0x0000ffff"،
    "كود_متعدد": 31
  }
]
```

El campo `handle` (`namespace.name@semver`) coincide con lo que aceptan los CLIs vía
`--profile=…`, por lo que es seguro copiarlo directamente a la automatización.

### Negociar chunkers

Gateways y clientes anuncian perfiles soportados vía provider adverts:

```
موفرأدفرتبوديV1 {
    ...
    Chunk_profile: Profile_id (ضمني عبر التسجيل)
    القدرات: [...]
}
```

La programación de chunks multi-source se anuncia vía la capacidad `range`. El CLI la acepta con
`--capability=range[:streams]`, donde el sufijo numérico opcional codifica la concurrencia preferida
de fetch por rango del proveedor (por ejemplo, `--capability=range:64` anuncia un presupuesto de 64 streams).
Cuando se omite, los consumidores vuelven al hint general `max_streams` publicado en otra parte del advert.

Al solicitar datos CAR, los clientes deben enviar un header `Accept-Chunker` que liste tuplas
`(namespace, name, semver)` en orden de preferencia:

```يتم تحديد البوابات لملف شخصي مدعوم بشكل متبادل (بسبب العيب `sorafs.sf1@1.0.0`)
وقم بإرجاع القرار عبر رأس الاستجابة `Content-Chunker`. يظهر لوس
تضمين ملف تعريف أنيق بحيث يمكن للعقد في اتجاه مجرى النهر التحقق من تخطيط القطع
لا يعتمد على تجارة HTTP.

### سوبورتي كار

نعيد النظر في طريق التصدير CARv1+SHA-2:

* **المرحلة الأولية** – CARv2، ملخص الحمولة BLAKE3 (`0x1f` multihash)،
  `MultihashIndexSorted`، ملف القطعة المسجل كما وصل.
  PUEDEN يشرح هذا البديل عند حذف العميل `Accept-Chunker` أو الطلب
  `Accept-Digest: sha2-256`.

إضافية للانتقال ولكن لا ينبغي استبدال الملخص القانوني.

### مطابق

* تم تعيين الملف `sorafs.sf1@1.0.0` إلى التركيبات العامة
  `fixtures/sorafs_chunker` والشركات المسجلة في
  `fuzz/sorafs_chunker`. يتم تنفيذ التسلسل من النهاية إلى النهاية بواسطة Rust و Go و Node
  وسط التوقعات.
* `chunker_registry::lookup_by_profile` يؤكد أن معلمات الواصف
  يتزامن مع `ChunkProfile::DEFAULT` لتجنب الاختلافات العرضية.
* تتضمن البيانات التي تم إنتاجها بواسطة `iroha app sorafs toolkit pack` و`sorafs_manifest_stub` بيانات تعريف السجل.