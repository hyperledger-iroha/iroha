---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: تأليف ملف تعريف مقسم
العنوان: دليل تأليف ملفات Chunker في SoraFS
Sidebar_label: دليل التأليف Chunker
الوصف: قائمة الاختيار لاقتراح ملفات Chunker الجديدة والتركيبات في SoraFS.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/chunker_profile_authoring.md`. احرص على جميع النسختين متزامنتين إلى أن يتم إيقاف مجموعة Sphinx القديمة.
:::

# دليل تأليف ملفات Chunker في SoraFS

يشرح هذا الدليل كيفية تكوين و ملفات chunker جديدة لـ SoraFS.
وهو يكمل RFC Turner (SF-1) ومرجع السجل (SF-2a)
بمتطلبات التأليف تحديد وخطوات تحقق وقوالب الصنع.
مرجع على سبيل المثال، مرجع
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
بالاضافة الى التشغيل الجاف في
`docs/source/sorafs/reports/sf1_determinism.md`.

## نظرة عامة

يجب أن تنتج كل ملف الإدخالات ما يلي:

- الإعلان عن معلمات CDC حتمية وإعدادات متعددة متطابقة عبر المعماريات؛
- شحن تركيبات قابلة لإعادة التشغيل (JSON Rust/Go/TS + corpora fuzz + شهود PoR) يمكن لـ SDKs downstream
  التحقق منها دون الأدوات المخصصة؛
- تضمين بيانات الاتصال للموقع (مساحة الاسم، الاسم، الفصل) مع إرشادات الهجرة و نافذة التوافق؛ و
- أجتياز حزمة مختلفة الحامية قبل مراجعة المجلس.

اتبع القائمة أدناه للإعدادات واستوفي هذه التعليمات.

## ملخص سجل السجل

تم الاتفاق مسبقًا على ذلك، والتأكد من مطابقته لميثاق السجل الذي قام به
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:- سجلات البيانات المسجلة صحيحة مشخصا بشكل رتيب دون فجوات.
- يجب أن يظهر المقبض المؤهل (`namespace.name@semver`) في قائمة البدائل
  وأن يكون **الأول**. تليه البدائل القديمة (مثل `sorafs.sf1@1.0.0`).
- لا يجوز لأي شخص أن يعارض مع مقبض معتمد آخر أو يتكرر.
- يجب أن تكون اسم مستعار غير فارغ ومقرصوصة من المسافات.

مساعدات CLI رقم:

```bash
# قائمة JSON بكل descripors المسجلة (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# إخراج بيانات ملف افتراضي مرشح (handle معتمد + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

حافظ على هذه موافقتك على الموافقة على ميثاق تسجيل البيانات المعتمدة
ضرورية لنقاشات ال تور.

## البيانات المطلوبة| الحقل | الوصف | مثال (`sorafs.sf1@1.0.0`) |
|-------|-------|--------------------------|
| `namespace` | تجميع منطقي للملفات ذات الصلة. | `sorafs` |
| `name` | تسمية مسجلة للبشرة. | `sf1` |
| `semver` | سلسلة نسخة دلالية للمعلمات. | `1.0.0` |
| `profile_id` | معرف رقمي رتيب يُسند عند إرسال الملف. اطلب المعرف التالي ولا تعرف استخدام الأرقام الحالية. | `1` |
| `profile_aliases` | مقيدات إضافية اختيارية (أسماء طويلة، اختصارات) تُعرض إلا أثناء الاتصال. يجب أن تشمل المقيدات الجديدة. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | قطعة طويلة بالبايت. | `65536` |
| `profile.target_size` | قطعة طويلة تستهدف بالبايت. | `262144` |
| `profile.max_size` | طول القطعة الأقصى بالبايت. | `524288` |
| `profile.break_mask` | قناع التكيف يستخدمه المتداول التجزئة (ست عشري). | `0x0000ffff` |
| `profile.polynomial` | متعدد الحدود للعتاد الثابت (ست عشري). | `0x3da3358b4dc173` |
| `gear_seed` | بذرة لاشتقاق جدول جير بحجم 64 كيلو بايت. | `sorafs-v1-gear` |
| `chunk_multihash.code` | كود multihash لـ هضم لكل قطعة. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | ملخص لزمة التركيبات المعتمدة. | `13fa...c482` |
| `fixtures_root` | مسار نسبي يحتوي على تركيبات المعاد توليدها. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | بذور لعيّنات PoR الحتمية (`splitmix64`). | `0xfeedbeefcafebabe` (مثال) |يجب أن يتم حجز البيانات الوصفية في الاتفاقية المقترحة وداخل التركيبات المولدة حتى سجل السجل
والأدوات الـ CLI ورسوم المطبوعات من القيم دون الخبرة اليدوية. عند الشك، شغّل
CLIs الخاصة بـchunk-store والبيان مع `--json-out=-` لبث البيانات المحسوبة إلى
مراجعة المراجعة.

### نقاط تماس CLI والسجل

- `sorafs_manifest_chunk_store --profile=<handle>` — إعادة تشغيل بيانات القطعة والملخص
  للـ المانيفست و فحوصات PoR مع المعلمات.
- `sorafs_manifest_chunk_store --json-out=-` — تقرير استجابة لـchunk-store إلى stdout
  خطة المقارنة.
- `sorafs_manifest_stub --chunker-profile=<handle>` — بالتأكيد بيانات وخطط CAR
  تشمل المقبض والبدائل.
- `sorafs_manifest_stub --plan=-` — إعادة `chunk_fetch_specs` رد السابق من
  تعويضات/ملخصات بعد التغيير.

سجل مخرجات مميز (الملخصات، وضوح PoR، التجزئات للـ Manifest) في مقترح يمكنك اختياره
المراجع إعادة إنتاجها باحترافية.

## قائمة تحقق الحتمية والتحقق1. ** إعادة توليد التركيبات **
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **تشغيل مجموعة التكافؤ** — يجب أن تكون `cargo test -p sorafs_chunker` و Harness diff
   عبر اللغات (`crates/sorafs_chunker/tests/vectors.rs`) أخضر مع التركيبات الجديدة.
3. **إعادة تشغيل الزغب الجسدي/الضغط الخلفي** — نفّذ `cargo fuzz list` و Harness Television
   (`fuzz/sorafs_chunker`) على الأصول المُعاد توليدها.
4. **التحقق من شهود إثبات الاسترجاع** — شغّل
   `sorafs_manifest_chunk_store --por-sample=<n>` باستخدام الملف المقترح المعتمد ليتم الموافقة عليه
   مع البيان الخاص بالـ تركيبات.
5. **Dry run للـ CI** — شغّل `ci/check_sorafs_fixtures.sh` محلياً؛ يجب أن تنجح
   مع التركيبات الجديدة و `manifest_signatures.json` الحالية.
6. **تأكيد cross runtime** — تأكد من أن Go/TS يستهلك JSON المُعاد توليده ومن يخرج
   حدود القطعة و الهضم متطابقة.

وثيقة تفضل والـ Digests المقترح الذي يمكنك من خلاله Tooling WG إعادة تشغيلها دون اشتراك.

### تأكيد البيان / PoR

بعد إعادة إنشاء التركيبات، مسار شغّل واضح بشكل كامل وبقاء بيانات CAR وPoR متسقة:

```bash
# التحقق من بيانات chunk + PoR مع الملف الجديد
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# توليد manifest + CAR والتقاط chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# إعادة التشغيل باستخدام خطة fetch المحفوظة (تمنع offsets القديمة)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

استبدل ملف الإدخال بأي شكل من الأشكال ممثل مستخدم في التركيبات الخاصة بك
(مثلاً تيار حتمي بحجم 1 جيجا بايت) وأرفق الملخصات في المقترح.

##القالب المقترح

يتم تقديم المقترحات كسجلات Norito من النوع `ChunkerProfileProposalV1` المحفوظة ضمنا
`docs/source/sorafs/proposals/`. يوضح القالب JSON أدناه الشكل الرئيسي
(استبدل القيم حسب الحاجة):


تقرير تسجيل العلامات التجارية وفقا لذلك (`determinism_report`) يسجل مخرجات لاسلكية وهضم للـ Chunk وأيها
انحرافات تمت ملاحظتها أثناء التحقق.

## سير عمل ال تور1. ** تقديم العلاقات العامة مع المقترح + التركيبات . ** أجزاء الأصول المولدة، الآلة Norito، وتحديثات
   `chunker_registry_data.rs`.
2. **مراجعة Tooling WG.** إعادة المراجعون لإجراء قائمة التحقق والتأكدون من المقترح
   أهلاً بمتطلبات السجل (لا إعادة استخدام المعرفات، الحتمية متحققة).
3. **ظرف المجلس.** بعد الموافقة، يشترك أعضاء ملخص المجلس المقترح
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) ويضيفون توقيعاتهم إلى
   ظرف الملف المخزن مع التجهيزات.
4. **نشر السجل.** يؤدي إلى تحديث السجل والوثائق والـ المباريات. يختفي CLI افتراضيا
   على الملف السابق حتى تعلن عن هجرتها.
   وبلغم تشغيلين عبر دفتر حسابات الهجرة.

## نصائح التأليف

- ولا يقتصر الأمر على قوى زوجية رئيسية للتحكم في الحالات الطرفية.
- تجنب تغيير رمز multihash دون التنسيق مع مستهلكي البيان و البوابة؛ وأضف موافقًا عند ذلك.
- جعل البذور لجدول والعتاد قابلة للقراءة مختلفة مختلفة لتسهيل التمييز.
- خزين أي مصنوعات قياس الأداء (مثل مقارنات الإنتاجية) ضمنا
  `docs/source/sorafs/reports/` للرجوع لاحقاً.

لتوقعات التشغيل أثناء إعادة النظر في دفتر أستاذ ترحيل الطرح
(`docs/source/sorafs/migration_ledger.md`). لقواعد المطابقة لوقت العمل
`docs/source/sorafs/chunker_conformance.md`.