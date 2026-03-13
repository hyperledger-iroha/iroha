---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: سجل المقطع
العنوان: تسجيل الملفات الشخصية Chunker SoraFS
Sidebar_label: سجل المقطع
الوصف: معرفات الملف الشخصي والإعدادات وخطة التداول لتسجيل القطعة SoraFS.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/chunker_registry.md`. قم بمزامنة النسختين حتى تكتمل مجموعة أبو الهول الموروثة.
:::

## تسجيل الملفات الشخصية Chunker SoraFS (SF-2a)

تعمل المكدس SoraFS على إجراء عملية التقطيع عبر مساحة اسم سجل صغير.
تم تعيين كل ملف تعريف لمعلمات CDC المحددة، والبيانات الموجزة على مدار الساعة، والملخص/الترميز المتعدد الذي يتم استخدامه في البيانات وأرشيفات CAR.

يجب استشارة مؤلفي الملفات الشخصية
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
من أجل المتطلبات الموضحة، وقائمة التحقق من الصحة، ونموذج الاقتراح المسبق
de soumettre de nouvelles Entrées. Une fois qu'une التعديل تمت الموافقة عليه من قبل la
الحوكمة، suivez la
[قائمة التحقق من بدء التسجيل](./chunker-registry-rollout-checklist.md) وآخرون
[دليل التشغيل للبيان المرحلي](./staging-manifest-playbook) للترويج
التركيبات مقابل التدريج والإنتاج.

### الملفات الشخصية| مساحة الاسم | الاسم | سيمفير | معرف الملف الشخصي | دقيقة (الثمانيات) | سيبل (الثمانيات) | ماكس (الثمانيات) | قناع التمزق | مولتيهاش | الاسم المستعار | ملاحظات |
|-----------|-----|--------|-------------|----------------|--------------|------------------|-----------|-----|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | يتم استخدام الملف التعريفي الكنسي في التركيبات SF-1 |

قم بالتسجيل في الكود التالي `sorafs_manifest::chunker_registry` (مسجل بالاسم [`chunker_registry_charter.md`](./chunker-registry-charter.md)). مدخل تشاك
تم اختباره كـ `ChunkerProfileDescriptor` مع :

* `namespace` - منطق إعادة تجميع ملفات التعريف (على سبيل المثال، `sorafs`).
* `name` – التشهير (`sf1`، `sf1-fast`، …).
* `semver` – سلسلة الإصدار الدلالي للعبة الإعدادات.
* `profile` – `ChunkProfile` حقيقي (الحد الأدنى/الهدف/الحد الأقصى/القناع).
* `multihash_code` – يتم استخدام التجزئة المتعددة عند إنتاج خلاصات القطع (`0x1f`
  من أجل الافتراضي SoraFS).يقوم البيان بتسلسل الملفات الشخصية عبر `ChunkingProfileV1`. يقوم الهيكل بتسجيل الميتادونات
du registre (namespace, name, semver) aux côtés des paramètres CDC bruts and de la liste d'alias ci-dessus.
يتعين على العملاء إجراء بحث في التسجيل باستخدام `profile_id` والرجوع مرة أخرى
المعلمات المضمنة عندما تظهر المعرفات غير المعروفة ; تضمن القائمة الاسمية لعملاء HTTP
يجب عليك التسجيل حتى يتم إدخال Canonique (`namespace.name@semver`) لأول مرة
`profile_aliases`، تابع للاسم المستعار الموروث.

لفحص التسجيل من خلال الأدوات، قم بتنفيذ مساعد CLI:

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

جميع أعلام CLI التي تكتب JSON (`--json-out`، `--por-json-out`، `--por-proof-out`،
`--por-sample-out`) مقبول `-` كطريق، ما الذي يبث الحمولة الصافية مقابل stdout بدلاً من ذلك
أنشئ ملفًا. هذا يسهل توصيل البيانات بجميع الأدوات مع الحفاظ عليها
السلوك من خلال défaut d'imprimer le Rapport Pride.

### مصفوفة الطرح وخطة النشر


تلتقط اللوحة هذه حالة الدعم الحالية لـ `sorafs.sf1@1.0.0` في les
مبدأ المكونات. تم تصميم "Bridge" لرحلة CARv1 + SHA-256
تحتاج إلى عميل تفاوض صريح (`Accept-Chunker` + `Accept-Digest`).| مركب | النظام الأساسي | ملاحظات |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅دعم | قم بالتحقق من المقبض canonique + alias، وقم ببث التقارير عبر `--json-out=-` وقم بتطبيق مخطط التسجيل عبر `ensure_charter_compliance()`. |
| `sorafs_fetch` (منسق المطور) | ✅دعم | Lit `chunk_fetch_specs`، يستوعب حمولات السعة `range` ويجمع طلعة CARv2. |
| تركيبات SDK (الصدأ/الذهاب/TS) | ✅دعم | Régénérées عبر `export_vectors` ; يظهر المقبض Canonique في كل قائمة مستعارة ويتم توقيعه من خلال مغلفات المجلس. |
| التفاوض على ملفات التعريف الخاصة بالبوابة Torii | ✅دعم | قم بتطبيق جميع القواعد `Accept-Chunker`، بما في ذلك الرؤوس `Content-Chunker` ولا تعرض الجسر CARv1 الذي تظهر فيه طلبات الرجوع إلى إصدار أقدم بوضوح. |

نشر جهاز القياس عن بعد :- **جلب القطع عن بعد** — يتضمن CLI Iroha `sorafs toolkit pack` خلاصات القطع وأجزاء CAR المتحولة وأصول PoR للتناول في لوحات المعلومات.
- **إعلانات الموفر** — تشتمل حمولات الإعلانات على قدرات متجددة وأسماء مستعارة؛ قم بالتحقق من الغطاء عبر `/v2/sorafs/providers` (على سبيل المثال، وجود السعة `range`).
- **بوابة المراقبة** — يجب على المشغلين تحديد القارنات `Content-Chunker`/`Content-Digest` لاكتشاف التخفيضات غير المقصودة؛ يتم تقليل استخدام الجسر إلى الصفر قبل التخفيض.

سياسة التخفيض: مرة واحدة يتم فيها التصديق على الملف الشخصي الناجح، قم بتخطيط نافذة للنشر المزدوج
(موثق في الاقتراح) قبل العلامة `sorafs.sf1@1.0.0` كسعر مخفض في التسجيل وسحبه
جسر CARv1 للبوابات في الإنتاج.

من أجل فحص نوع معين من PoR، قم بتوفير مؤشرات القطعة/القطعة/القطعة وما إلى ذلك، اختياريًا،
استمر في الاستمرار على القرص :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

يمكنك تحديد ملف تعريف بواسطة معرف رقمي (`--profile-id=1`) أو بواسطة مقبض التسجيل
(`--profile=sorafs.sf1@1.0.0`) ؛ المقبض النموذجي مناسب للنصوص البرمجية
مساحة الاسم/الاسم/النقل المباشر بشكل مباشر من خلال الحوكمة المتغيرة.استخدم `--promote-profile=<handle>` لإنشاء كتلة JSON من métadonnées (تتضمن جميع الأسماء المستعارة)
المسجلين) الذين يمكن أن يكونوا مجمعين في `chunker_registry_data.rs` أثناء الترويج لملف تعريف جديد
افتراضيا :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

يتضمن التقرير الرئيسي (وملف الخيارات المسبقة) ملخصًا للأصل وثمانيات الرسومات المتحركة
(يتم ترميزه بالنظام السداسي العشري) والملخصات الأصغر من المقطع/القطعة حتى يتمكن المدققون من إعادة صياغتها
تبلغ سعة المقاعد 64 كيلو بايت/4 كيلو بايت القيمة `por_root_hex`.

للتحقق من وجود حمولة مسبقة، قم بتمرير المسار عبر
`--por-proof-verify` (تم إضافة CLI إلى `"por_proof_verified": true` عندما تموين
تتوافق مع حساب العرق):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

للتزيين بكميات كبيرة، استخدم `--por-sample=<count>` وقم بتزويده في نهاية المطاف بسلسلة من البذور/الفرز.
يضمن CLI أمرًا محددًا (مدرجًا مع `splitmix64`) ويتم تشغيله تلقائيًا عند
الطلب يتخطى الأوراق المتاحة :```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Le manifest stub reflète les mêmes données, ce qui est pratique pour scripter la sélection de
`--chunker-profile-id` dans les pipelines. Les deux CLIs de chunk store acceptent aussi la forme de handle canonique
(`--profile=sorafs.sf1@1.0.0`) afin que les scripts de build évitent de coder en dur des IDs numériques :

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

Le champ `handle` (`namespace.name@semver`) correspond à ce que les CLIs acceptent via
`--profile=…`, ce qui permet de le copier directement dans l'automatisation.

### Négocier les chunkers

Les gateways et les clients annoncent les profils supportés via des provider adverts :

```
موفرأدفرتبوديV1 {
    ...
    Chunk_profile: Profile_id (ضمني عبر التسجيل)
    القدرات: [...]
}
```

La planification multi-source des chunks est annoncée via la capacité `range`. Le CLI l'accepte avec
`--capability=range[:streams]`, où le suffixe numérique optionnel encode la concurrence de fetch par range préférée
par le provider (par exemple, `--capability=range:64` annonce un budget de 64 streams).
Lorsqu'il est omis, les consommateurs reviennent à l'indication générale `max_streams` publiée ailleurs dans l'advert.

Lorsqu'ils demandent des données CAR, les clients doivent envoyer un header `Accept-Chunker` listant des tuples
`(namespace, name, semver)` par ordre de préférence :

```

يتم تحديد البوابات كملف تعريف مدعوم بشكل متبادل (افتراضيًا `sorafs.sf1@1.0.0`)
ويعكس القرار من خلال رأس الاستجابة `Content-Chunker`. البيانات
قم بدمج ملف التعريف الذي تختاره حتى تتمكن البيانات النهائية من التحقق من تخطيط القطع
بدون الضغط على مفاوضات HTTP.

### دعم السيارة

نحن نحفظ طريق التصدير CARv1+SHA-2 :

* **مبدأ Chemin** - CARv2، ملخص الحمولة BLAKE3 (`0x1f` multihash)،
  `MultihashIndexSorted`، ملف تعريف القطعة المسجل باسم ci-dessus.
  PEUVENT يعرض هذا البديل عند العميل Omet `Accept-Chunker` أو الطلب
  `Accept-Digest: sha2-256`.

لا يلزم استبدال الملخص الكنسي الإضافي للانتقال أكثر.

### كونفورميتيه* الملف الشخصي `sorafs.sf1@1.0.0` يتوافق مع التركيبات العامة في
  `fixtures/sorafs_chunker` والشركات المسجلة أيضًا
  `fuzz/sorafs_chunker`. يتم ممارسة التكافؤ في النوبة في Rust و Go و Node
  عبر اختبارات les fournis.
* `chunker_registry::lookup_by_profile` يؤكد أن معلمات الواصف
  مراسل à `ChunkProfile::DEFAULT` لتفادي أي اختلاف عرضي.
* تتضمن بيانات المنتجات حسب `iroha app sorafs toolkit pack` و`sorafs_manifest_stub` بيانات التسجيل.