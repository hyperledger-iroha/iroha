---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: سجل المقطع
العنوان: Registro de perfis de Chunker da SoraFS
Sidebar_label: سجل القطع
الوصف: معرفات الملف الشخصي والمعلمات وخطة التداول لسجل القطع في SoraFS.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تحمل عنوان `docs/source/sorafs/chunker_registry.md`. Mantenha ambas as copias sincronzadas.
:::

## سجل أداء القطع من SoraFS (SF-2a)

مكدس SoraFS يتفاوض حول طريقة التقطيع عبر سجل صغير بمساحة الاسم.
يتميز كل ملف شخصي بمعلمات محددات مراكز مكافحة الأمراض والوقاية منها (CDC)، والتوصيفات المستمرة، والملخص/الرموز المتعددة المستخدمة في البيانات وملفات CAR.

يجب أن يكون خبراء الأداء مستشارين
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
بالنسبة إلى الاستبيانات المطلوبة، وقائمة التحقق من الصحة، ونموذج الاقتراح المسبق للمدخلات الجديدة.
عندما توافق الحكومة على مودانكا، سيجا أو
[قائمة التحقق من بدء التسجيل](./chunker-registry-rollout-checklist.md) e o
[دليل التشغيل للبيان المرحلي](./staging-manifest-playbook) للترويج
تركيبات نظام التشغيل والتدريج والإنتاج.

### بيرفيس| مساحة الاسم | الاسم | سيمفير | معرف الملف الشخصي | دقيقة (بايت) | الهدف (بايت) | ماكس (بايت) | ماسكارا دي كوبرا | مولتيهاش | الأسماء المستعارة | نوتاس |
|-----------|------|--------|------------|----------------|-------------|-----------------|-----------|-------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | ملف كانونيكو يستخدم في التركيبات SF-1 |

لا يوجد سجل حي كرمز `sorafs_manifest::chunker_registry` (يتم التحكم فيه بواسطة [`chunker_registry_charter.md`](./chunker-registry-charter.md)). كل مدخل
e Expressa como um `ChunkerProfileDescriptor` com:

* `namespace` - مجموعة منطق الأداء ذات الصلة (على سبيل المثال، `sorafs`).
* `name` - حقوق قانونية للبشر (`sf1`، `sf1-fast`، ...).
* `semver` - سلسلة الدلالات العكسية لمجموعة المعلمات.
* `profile` - o `ChunkProfile` حقيقي (الحد الأدنى/الهدف/الحد الأقصى/القناع).
* `multihash_code` - استخدام تجزئة متعددة في خلاصات المنتج (`0x1f`
  الفقرة o الافتراضي دا SoraFS).قم بالتسلسل الكامل للبيان عبر `ChunkingProfileV1`. سجل estrutura os metadados
 قم بالتسجيل (مساحة الاسم، الاسم، الفصل) جنبًا إلى جنب مع معلمات CDC الأولية
قائمة الأسماء المستعارة المعروضة هنا. يجب على المستهلكين أن يستأجروا أول مرة
لا يتم البحث عن تسجيل لـ `profile_id` وتصحيح المعلمات المضمنة عند
معرفات desconhecidos aparecerem؛ قائمة الأسماء المستعارة التي تضمن حصول العملاء على HTTP
استمر في إرسال مقابض البدائل في `Accept-Chunker` دون إضافة أي شيء. كما تفعل ريجراس
يتطلب ميثاق السجل التعامل مع Canonico (`namespace.name@semver`)
تم الإدخال الأول في `profile_aliases`، يتبعه أي أسماء مستعارة بديلة.

للتحقق من تسجيل الأدوات، قم بتنفيذ مساعد CLI:

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

جميع العلامات تفعل CLI التي تخفي JSON (`--json-out`، `--por-json-out`، `--por-proof-out`،
`--por-sample-out`) هذا هو `-` كطريق، أو الذي ينقل الحمولة إلى الوضع القياسي في وقت ما
اكتب ملفًا. من السهل جدًا تجميع البيانات للحفاظ على الأدوات
طريقة السلوك للطباعة أو العلاقة الأساسية.

### مصفوفة الطرح والزرع


لوحة بعد التقاط الحالة الحالية للدعم لـ `sorafs.sf1@1.0.0` nos
المكونات الرئيسية. يشير "الجسر" إلى فاكس CARv1 + SHA-256
الذي يطلب التفاوض الصريح مع العميل (`Accept-Chunker` + `Accept-Digest`).| مكون | الحالة | نوتاس |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅مدعم | التحقق من صحة التعامل مع Canonico + الأسماء المستعارة، وتدفق الروابط عبر `--json-out=-` وتطبيق ميثاق التسجيل عبر `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ اعتزال | مُنشئ بيان لمنتدى الدعم؛ استخدم `iroha app sorafs toolkit pack` لتعبئة السيارة/البيان والحفاظ على `--plan=-` لإعادة التحقق من الحتمية. |
| `sorafs_provider_advert_stub` | ⚠️ اعتزال | مساعد التحقق من صحة البيانات دون اتصال بالإنترنت؛ إعلانات الموفر يجب أن يتم إنتاجها من خلال خط أنابيب النشر والمصادقة عبر `/v1/sorafs/providers`. |
| `sorafs_fetch` (منسق المطور) | ✅مدعم | يحتوي `chunk_fetch_specs` على حمولات ذات سعة `range` ويحمل CARv2. |
| تركيبات SDK (الصدأ/الذهاب/TS) | ✅مدعم | التجديد عبر `export_vectors`; o التعامل مع Canonico الذي يظهر لأول مرة في كل قائمة من الأسماء المستعارة ويتم حذفه من خلال المظاريف التي يتم اقتراحها. |
| Negociacao de perfil no gate Torii | ✅مدعم | قم بتنفيذ قواعد `Accept-Chunker` الكاملة، بما في ذلك الرؤوس `Content-Chunker` واعرض جسر CARv1 فقط من خلال طلبات الرجوع الصريحة إلى إصدار أقدم. |

طرح القياس عن بعد:- **القياس عن بعد لجلب القطع** - o CLI Iroha `sorafs toolkit pack` يصدر ملخصات القطع، metadados CAR ويزيد من PoR لاستيعاب لوحات المعلومات.
- **إعلانات الموفر** - تشتمل حمولات الإعلانات على بيانات وصفية للقدرة والأسماء المستعارة؛ صالحة cobertura عبر `/v1/sorafs/providers` (على سبيل المثال، presenca da capacidade `range`).
- **مراقبة البوابة** - يقوم المشغلون بالإبلاغ عن الإعدادات `Content-Chunker`/`Content-Digest` لاكتشاف التخفيضات غير المتوقعة؛ أتمنى أن يكون استخدام الجسر على وشك الصفر قبل الإهمال.

سياسة الإهمال: مرة واحدة يتم التصديق على الملف اللاحق، جدول نشر مزدوج
جسر CARv1 دوس بوابات م producao.

من أجل فحص شهادة أداء محددة، للحصول على مؤشرات القطعة/القطعة/الحجم واختياريًا
استمرار بروفا لا ديسكو:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

يمكن اختيار ملف شخصي بالمعرف الرقمي (`--profile-id=1`) أو من خلال مقبض التسجيل
(`--profile=sorafs.sf1@1.0.0`)؛ مقبض شكلي ومريح للنصوص البرمجية
Encadeiam namespace/name/semver مباشرة من metadados de Governmentanca.

استخدم `--promote-profile=<handle>` لإصدار كتلة JSON من metadados (بما في ذلك جميع الأسماء المستعارة)
المسجلين) يمكن أن يتم دمجها في `chunker_registry_data.rs` لتعزيز ملف تعريف جديد:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```تتضمن العلاقة الأساسية (ملف إثبات اختياري) إجمالي الزيادة ووحدات البايت من الأوراق المالية
(المشفرة في شكل سداسي عشري) والملخصات مقسمة إلى قطع/قطعة حتى يتمكن المدققون من السيطرة عليها
إعادة حساب التجزئة من 64 كيلو بايت/4 كيلو بايت مقابل القيمة `por_root_hex`.

للتحقق من وجود حمولة، قم بتمريرها أو طريقها عبرها
`--por-proof-verify` (أو إضافة CLI `"por_proof_verified": true` عند الاختبار
تتوافق مع حساب الرايز):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

للتمكن من ذلك، استخدم `--por-sample=<count>` واختياريًا للحصول على طريق البذور/البذور.
يضمن O CLI التنظيم الحتمي (المصنف مع `splitmix64`) ويتم التحويل تلقائيًا عند
هناك طلب إضافي كما هو متاح:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

O manifest stub espelha os mesmos dados, o que e conveniente ao automatizar a selecao de
`--chunker-profile-id` em pipelines. Ambos os CLIs de chunk store tambem aceitam a forma de handle canonico
(`--profile=sorafs.sf1@1.0.0`) para que scripts de build evitem hard-codear IDs numericos:

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

O campo `handle` (`namespace.name@semver`) corresponde ao que os CLIs aceitam via
`--profile=...`, tornando seguro copiar direto para automacao.

### Negociar chunkers

Gateways e clientes anunciam perfis suportados via provider adverts:

```
موفرأدفرتبوديV1 {
    ...
    Chunk_profile: Profile_id (ضمنيًا عبر التسجيل)
    القدرات: [...]
}
```

O agendamento de chunks multi-source e anunciado via a capacidade `range`. O CLI aceita isso com
`--capability=range[:streams]`, onde o sufixo numerico opcional codifica a concorrencia preferida
para fetch por range do provider (por exemplo, `--capability=range:64` anuncia um budget de 64 streams).
Quando omitido, consumidores recorrem ao hint geral `max_streams` publicado em outro ponto do advert.

Ao solicitar dados CAR, clientes devem enviar um header `Accept-Chunker` listando tuplas
`(namespace, name, semver)` em ordem de preferencia:

```يتم تحديد البوابات لملف دعم متبادل (الافتراضي `sorafs.sf1@1.0.0`)
قم بإعادة اتخاذ القرار عبر رأس الرد `Content-Chunker`. البيانات
قم بتضمين الملف الشخصي حتى نتمكن من التحقق من صحة تخطيط القطع في اتجاه المصب
لا يعتمد ذلك على تجارة HTTP.

### سوبورتي كار

الحفاظ على طريق التصدير CARv1+SHA-2:

* **الكاميرا الأولية** - CARv2، ملخص الحمولة BLAKE3 (`0x1f` multihash)،
  `MultihashIndexSorted`، ملف القطعة المسجل كما هو موضح أعلاه.
  يتم تصدير PODEM بشكل مختلف عند حذف العميل `Accept-Chunker` أو طلبه
  `Accept-Digest: sha2-256`.

إضافة للتحويل، لكن يجب عليك استبدال أو ملخص الكنسي.

### كونفورميداد

* ملف التعريف `sorafs.sf1@1.0.0` خريطة للتركيبات العامة em
  `fixtures/sorafs_chunker` والهيئات المسجلة فيها
  `fuzz/sorafs_chunker`. مساواة شاملة وممارسة في Rust وGo وNode
  عبر نظام التشغيل الخصيتين fornecidos.
* `chunker_registry::lookup_by_profile` يؤكد أن المعلمات تعمل بالواصف
  قم بالمراسلة مع `ChunkProfile::DEFAULT` لتجنب التباعد الحمضي.
* تتضمن بيانات المنتجات حسب `iroha app sorafs toolkit pack` و`sorafs_manifest_stub` بيانات التعريف المسجلة.