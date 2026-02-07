---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: تأليف ملف تعريف مقسم
العنوان: دليل صانع ملفات التعريف SoraFS
Sidebar_label: دليل أداة القطع
الوصف: قائمة مرجعية لدعم الملفات الشخصية الجديدة وتركيبات القطعة SoraFS.
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sorafs/chunker_profile_authoring.md`. حافظ على النسخ المتزامنة حتى يتم سحب مجموعة وثائق Sphinx المتوارثة.
:::

# دليل إنشاء ملفات التعريف الخاصة بـ SoraFS

يشرح هذا الدليل كيفية عرض ونشر ملفات تعريف جديدة لـ SoraFS.
استكمال RFC للهندسة المعمارية (SF-1) ومرجع السجل (SF-2a)
مع متطلبات السلطة الملموسة وخطوات التحقق ونباتات الترويج.
للحصول على مثال قانوني، راجع
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
وتسجيل التشغيل الجاف المرتبط باللغة الإنجليزية
`docs/source/sorafs/reports/sf1_determinism.md`.

## السيرة الذاتية

يجب على كل ملف يدخل إلى السجل:

- الإعلان عن معلمات CDC المحددة وضبط المتطابقات المتعددة بين
  الهندسة المعمارية.
- تركيبات قابلة للتكرار (JSON Rust/Go/TS + corpora fuzz + testigos PoR) que
  يمكن لمجموعات تطوير البرامج (SDKs) في اتجاه المصب التحقق من استخدام الأدوات عبر الإنترنت؛
- تضمين قوائم التعريف للإدارة (مساحة الاسم، الاسم، الفصل) جنبًا إلى جنب مع دليل الطرح
  ونوافذ التشغيل. ذ
- تجاوز مجموعة الاختلافات المحددة قبل مراجعة النصيحة.

قم بإكمال قائمة المراجعة لإعداد اقتراح يكمل هذه القواعد.## استئناف مذكرة التسجيل

قبل تنقيح عرض ما، تأكد من إكمال بطاقة السجل المطبقة
من أجل `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- تعد معرفات الملفات الشخصية إيجابية مما يزيد من رتابة الشكل دون أي عدد.
- المقبض الكنسي (`namespace.name@semver`) سيظهر في قائمة الأسماء المستعارة
  و **يجب** أن تكون أول دخول. Siguen los aliasheredados (ص. على سبيل المثال، `sorafs.sf1@1.0.0`).
- يمكن اصطدام الاسم المستعار بمقبض آخر ولا يظهر أكثر من مرة.
- لا ينبغي أن يكون الاسم المستعار خاليًا ومسجلاً من المساحات على بياض.

مساعدة من CLI:

```bash
# Listado JSON de todos los descriptores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadatos para un perfil por defecto candidato (handle canónico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

تقوم هذه الأوامر بصيانة العروض التفصيلية باستخدام بطاقة السجل وتخصيصها
التعريفات الأساسية اللازمة في مناقشات الإدارة.

## البيانات التعريفية المطلوبة| كامبو | الوصف | مثال (`sorafs.sf1@1.0.0`) |
|-------|------------|-----------------------------|
| `namespace` | تجميع منطقي للملفات الشخصية ذات الصلة. | `sorafs` |
| `name` | آداب مقروءة للإنسان. | `sf1` |
| `semver` | سلسلة الإصدار الدلالي لمجموعة المعلمات. | `1.0.0` |
| `profile_id` | تم تعيين المعرف الرقمي المفرد مرة واحدة عندما يدخل الملف الشخصي. احتفظ بالمعرف التالي ولكن دون الحاجة إلى إعادة استخدام الأرقام الموجودة. | `1` |
| `profile_aliases` | يعالج الخيارات الإضافية (الأسماء المتوارثة والمختصرة) التي يقدمها للعملاء أثناء التفاوض. تتضمن دائمًا المقبض القانوني كالمدخل الأول. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | الحد الأدنى لطول القطعة بالبايت. | `65536` |
| `profile.target_size` | طول قطعة القطعة والبايت. | `262144` |
| `profile.max_size` | أقصى طول للقطعة بالبايت. | `524288` |
| `profile.break_mask` | ماسكارا قابلة للتكيف تستخدم من خلال التجزئة المتداول (ست عشرية). | `0x0000ffff` |
| `profile.polynomial` | ترس كونستانتي ديل بولينوميو (سداسي). | `0x3da3358b4dc173` |
| `gear_seed` | تم استخدام البذور للحصول على ترس لوحة يبلغ 64 كيلو بايت. | `sorafs-v1-gear` |
| `chunk_multihash.code` | كود multihash لهضم القطعة. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | ملخص الحزمة القانونية للتركيبات. | `13fa...c482` || `fixtures_root` | الدليل النسبي الذي يحتوي على التركيبات المجددة. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | البذور اللازمة لتحديد قوة التحمل (`splitmix64`). | `0xfeedbeefcafebabe` (مثال) |

يجب أن تظهر البيانات الوصفية في مستند العرض كما هو الحال داخل التركيبات
تم تجديده حتى يتمكن السجل وأدوات CLI وأتمتة الإدارة
تأكيد القيم بدون أدلة. إذا كان لديك أصدقاء، قم بتشغيل CLIs من متجر القطع و
بيان مع `--json-out=-` لإرسال البيانات الوصفية المحسوبة إلى ملاحظات المراجعة.

### نقاط اتصال CLI والتسجيل

- `sorafs_manifest_chunk_store --profile=<handle>` - قم بتحريك البيانات الوصفية الخاصة بالتنفيذ،
  ملخص البيان والتحقق من PoR باستخدام المعلمات المقترحة.
- `sorafs_manifest_chunk_store --json-out=-` — يرسل تقرير مخزن القطع أ
  stdout للمقارنات التلقائية.
- `sorafs_manifest_stub --chunker-profile=<handle>` - لتأكيد ظهور طائرات CAR
  قم بتضمين المقبض الكنسي الأكثر شهرة.
- `sorafs_manifest_stub --plan=-` — قم بتحريك `chunk_fetch_specs` إلى الإصدار السابق
  التحقق من الإزاحة/الخلاصات بعد التغيير.

قم بتسجيل خروج الأوامر (الخلاصات، والأرز، وتجزئة البيان) في العرض الخاص بك
يمكن إعادة إنتاج المراجعات نصيًا.

## قائمة التحقق من التحديد والتحقق1. **التركيبات المتجددة**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **قم بتشغيل مجموعة الجدار** — `cargo test -p sorafs_chunker` والمفتاح المختلف
   بين اللغات (`crates/sorafs_chunker/tests/vectors.rs`) يجب أن تكون خضراء مع لوس
   تركيبات جديدة في مكانك.
3. **إعادة إنتاج الزغب/الضغط الخلفي** — قم بتشغيل `cargo fuzz list` والفتحة
   البث (`fuzz/sorafs_chunker`) ضد الأنشطة المتجددة.
4. **التحقق من الخصية وإثبات الاسترجاع** — القذف
   `sorafs_manifest_chunk_store --por-sample=<n>` استخدام الملف الشخصي والتأكيد عليه
   تتزامن الجذور مع بيان التركيبات.
5. **التشغيل الجاف لـ CI** — استدعاء `ci/check_sorafs_fixtures.sh` محليًا؛ البرنامج النصي
   يجب أن تكون ناجحًا مع التركيبات الجديدة و`manifest_signatures.json` الموجودة.
6. **التأكيد عبر وقت التشغيل** — تأكد من أن روابط Go/TS تستهلك JSON المُعاد إنشاؤه
   وتطلق حدودًا وهضمًا متطابقًا.

قم بتوثيق الأوامر والخلاصات الناتجة في العرض حتى يتمكن Tooling WG من العمل
repetirlos sin conjeturas.

### تأكيد البيان / PoR

بعد تجديد التركيبات، قم بتشغيل خط الأنابيب الكامل للبيان للتأكد من أنهم
بيانات تعريف CAR وتجربة PoR sigan siendo متسقة:

```bash
# Validar metadata de chunk + PoR con el nuevo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Generar manifest + CAR y capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reejecutar usando el plan de fetch guardado (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

قم بإعادة إنشاء ملف المدخل باستخدام أي مجموعة تمثيلية تستخدم من خلال تركيباتنا
(ص. على سبيل المثال، التدفق المحدد بـ 1 جيجا بايت) وإضافة الملخصات الناتجة إلى العرض.

##بلانتيللا دي بروبويستايتم إرسال العروض مثل السجلات Norito `ChunkerProfileProposalV1` المسجلة في
`docs/source/sorafs/proposals/`. توضح نبتة JSON الموضحة الشكل المتوقع
(تبديل قيمنا في البحر الضروري):


تقديم تقرير Markdown المقابل (`determinism_report`) الذي يلتقط
خروج الأوامر، وخلاصات القطع، وأي انحراف يتم اكتشافه أثناء التحقق من الصحة.

## تدفق حاكم

1. ** أرسل العلاقات العامة مع العرض + التركيبات. ** تشمل الأصول التي تم إنشاؤها، العرض
   Norito والتحديثات إلى `chunker_registry_data.rs`.
2. **مراجعة مجموعة عمل الأدوات.** تعيد المراجعات تشغيل قائمة التحقق من الصحة
   تأكد من أن العرض يتوافق مع قواعد التسجيل (بدون إعادة استخدام المعرف،
   الحتمية مرضية).
3. **عن النصيحة.** مرة واحدة مناسبة، أعضاء النصيحة يتأكدون من ملخصها
   تم عرض الملف (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) وقم بتركيبه
   شركة متخصصة في الحفاظ على المظهر جنبًا إلى جنب مع التركيبات.
4. **نشر السجل.** الدمج يقوم بتحديث السجل والمستندات والتركيبات. ش
   CLI بسبب الخلل الدائم في الملف السابق يسرع من إعلان الحكومة عن الهجرة
   lista.
5. **متابعة الإهمال.** بعد نافذة الهجرة، قم بتحديث السجل
   دفتر الهجرة.

## نصائح السلطة- اختر حدودًا لطاقة الزوجين لتقليل أداء التقطيع في الحالات الحدودية.
- تجنب تغيير الكود المتعدد بدون تنسيق مع مستهلكي البيان والبوابة؛ تشمل واحدة
  ملاحظة عملية عندما يحدث ذلك.
- الحفاظ على بذور الطبلة مقروءة للإنسان ولكن عالمية فريدة من نوعها لتبسيطها
  القاعات.
- حماية أي أداة قياس أداء (على سبيل المثال، مقارنات الإنتاجية) أدناه
  `docs/source/sorafs/reports/` للإشارة إلى المستقبل.

للتشغيل المتوقع أثناء الطرح راجع دفتر حسابات الهجرة
(`docs/source/sorafs/migration_ledger.md`). لضبط المطابقة في إصدار وقت التشغيل
`docs/source/sorafs/chunker_conformance.md`.