---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: تأليف ملف تعريف مقسم
العنوان: دليل تأليف الملف التعريفي SoraFS
Sidebar_label: دليل تأليف Chunker
الوصف: SoraFS ملفات تعريف وتركيبات القطعة قم باختيار القائمة المرجعية.
---

:::ملاحظة مستند ماخذ
:::

# SoraFS دليل تأليف ملف تعريف القطعة

لقد تم توضيح هذه النقطة من خلال SoraFS لبحث الملفات الشخصية الجديدة ونشرها.
هناك بنية RFC (SF-1) ومرجع التسجيل (SF-2a).
متطلبات التأليف الملموسة، وخطوات التحقق من الصحة، وقوالب الاقتراحات كلها مكتملة.
مثال قانوني لمثال آخر
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
وما يتعلقہ سجل التشغيل الجاف
`docs/source/sorafs/reports/sf1_determinism.md`.

## نظرة عامة

يظهر ملفك الشخصي في التسجيل وهو موجود هنا:

- معلمات CDC الحتمية وإعدادات التجزئة المتعددة للهندسة المعمارية للإعلان عن RNA؛
- تركيبات قابلة لإعادة التشغيل (Rust/Go/TS JSON + Fuzz corpora + PoR شهود) يمكنك استخدام أدوات تطوير البرامج (SDK) النهائية والأدوات المخصصة للتحقق من صحة البيانات؛
- مراجعة المجلس سے پہلے مجموعة الفرق الحتمية پاس کرنا۔

لا يوجد أي قائمة مرجعية لعمل هذا الاقتراح وهو نوع من القواعد لمزيد من التفاصيل.

## لقطة ميثاق التسجيل

تتوافق مسودة الاقتراح مع ميثاق السجل أو ميثاق التسجيل مع ما هو موجود الآن
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` فرض الأمر:- معرفات الملف الشخصي، الأعداد الصحيحة الإيجابية، تحتوي على فجوات رتيبة بشكل متكرر.
- الاسم المستعار المتوافق مع المقبض الأساسي لن يتصادم أبدًا ولن يظهر أي شيء آخر.
- الأسماء المستعارة غير فارغة ہوں والمسافات البيضاء سے تقليم ہوں.

مساعدين CLI مفيدين:

```bash
# تمام registered descriptors کی JSON listing (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Candidate default profile کے لیے metadata emit کریں (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

إنها مقترحات أوامر في ميثاق التسجيل تتوافق مع مناقشات الحوكمة والحوكمة التي تتضمن إطار البيانات التعريفية الأساسية.

## البيانات الوصفية المطلوبة| المجال | الوصف | مثال (`sorafs.sf1@1.0.0`) |
|-------|------------|-----------------------------|
| `namespace` | ملفات التعريف المرتبطة بها هي التجميع المنطقي. | `sorafs` |
| `name` | تسمية يمكن قراءتها بواسطة الإنسان۔ | `sf1` |
| `semver` | مجموعة المعلمات کے لیے سلسلة الإصدار الدلالي۔ | `1.0.0` |
| `profile_id` | المعرف الرقمي الرتيب ملف تعريف جو کے أرض ہونے پر إسناد ہوتا ہے. لن يتم إعادة استخدام أي حجز معرف موجود. | `1` |
| `profile.min_size` | طول القطعة هو الحد الأدنى ولا يقل عن بايت. | `65536` |
| `profile.target_size` | طول القطعة هو الهدف لا يزيد عن بايت. | `262144` |
| `profile.max_size` | طول القطعة هو أقصى حد بايت. | `524288` |
| `profile.break_mask` | التجزئة المتداول کے لیے قناع التكيف (ست عشري)۔ | `0x0000ffff` |
| `profile.polynomial` | ثابت متعدد الحدود الترس (ست عشري)۔ | `0x3da3358b4dc173` |
| `gear_seed` | 64 كيلو بايت تستمد طاولة التروس البذور. | `sorafs-v1-gear` |
| `chunk_multihash.code` | ملخص لكل قطعة هو كود متعدد التجزئة. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | حزمة التركيبات الأساسية کا هضم۔ | `13fa...c482` |
| `fixtures_root` | التركيبات المجددة رکھنے والی الدليل النسبي۔ | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | أخذ عينات PoR الحتمية للبذور (`splitmix64`). | `0xfeedbeefcafebabe` (مثال) |البيانات التعريفية ووثيقة الاقتراح والتركيبات التي تم إنشاؤها لا تتضمن أي تسجيل وأدوات CLI وأتمتة الإدارة بالإضافة إلى الإسناد الترافقي اليدوي حيث تؤكد القيم صحة البيانات. إذا قمت بالتخزين المقطعي وبيانات CLIs الواضحة التي `--json-out=-` فستقوم ببث ملاحظات مراجعة البيانات الوصفية المحسوبة.

### واجهة سطر الأوامر (CLI) ونقاط اتصال التسجيل

- `sorafs_manifest_chunk_store --profile=<handle>` — المعلمات المقترحة عبارة عن بيانات تعريف القطعة وملخص البيان وفحوصات PoR مرة أخرى.
- `sorafs_manifest_chunk_store --json-out=-` — تقرير مخزن القطع الذي يوفر مقارنات آلية للبث المباشر.
- `sorafs_manifest_stub --chunker-profile=<handle>` - تأكيد بيانات البطاقة وخطط CAR، المقبض الأساسي والأسماء المستعارة التي تتضمن البطاقة.
- `sorafs_manifest_stub --plan=-` — تم التحقق من `chunk_fetch_specs` تغيير حجم التغذية بعد التحقق من الإزاحات/الملخصات.

مخرجات الأمر (الملخصات، جذور PoR، التجزئات الواضحة) التي يتم إعادة إنتاجها حرفيًا من قبل المراجعين.

## قائمة التحقق من الحتمية والتحقق من الصحة1. ** تركيبات تجديد کریں **
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **مجموعة التكافؤ** — `cargo test -p sorafs_chunker` ومجموعة أدوات الاختلاف عبر اللغات (`crates/sorafs_chunker/tests/vectors.rs`) تركيبات جديدة باللون الأخضر.
3. **إعادة التشغيل الجسدي للضغط الخلفي/الزغب** — `cargo fuzz list` وأداة البث (`fuzz/sorafs_chunker`) وهي الأصول التي تم تجديدها.
4. **شهود إثبات الاسترجاع يتحققون من صحة البطاقة** — `sorafs_manifest_chunk_store --por-sample=<n>` الملف الشخصي المقترح يتطابق مع الأصل وجذور بيان التركيبات.
5. **التشغيل الجاف لـ CI** — `ci/check_sorafs_fixtures.sh` لوكل چلايں؛ البرنامج النصي الجديد والتركيبات المتوفرة `manifest_signatures.json` سينجحان مرة أخرى.
6. **تأكيد وقت التشغيل المتبادل** — هذه هي الطريقة التي تستهلك بها روابط Go/TS المُعاد إنشاؤها JSON نصوصًا وتنبعث منها حدود القطع المتماثلة والهضمات.

تعمل الأوامر والملخصات الناتجة عن الاقتراح على تطوير أسلوب العمل Tooling WG وهو التخمين الذي سيتكرر مرة أخرى.

### تأكيد البيان / PoR

تعمل التركيبات على إعادة إنشاء الشبكة بعد استكمال خط الأنابيب الواضح الذي يتضمن بيانات تعريف CAR وإثباتات PoR المتسقة:

```bash
# نئے profile کے ساتھ chunk metadata + PoR validate کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# manifest + CAR generate کریں اور chunk fetch specs capture کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# محفوظ fetch plan کے ساتھ دوبارہ چلائیں (stale offsets سے بچاتا ہے)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

يمكن استخدام ملف الإدخال الذي يتم تثبيته على الأجهزة التمثيلية فقط
(على سبيل المثال، تيار حتمي 1 جيجا بايت) والملخصات الناتجة والمقترح الذي يتم إرفاقه لاحقًا.

## قالب الاقتراح

يتم تسجيل المقترحات التي `ChunkerProfileProposalV1` Norito على `docs/source/sorafs/proposals/` في هذا المكان. قالب JSON الجديد المتوقع هو شكل من أشكال الدعم (تستبدل قيم الامتداد القيمة):مطابقة تقرير تخفيض السعر (`determinism_report`) يتم تنفيذ الأمر من خلال إخراج الأمر وملخصات القطع والتحقق من صحة الانحرافات التي تتضمنها.

## سير عمل الحوكمة

1. **الاقتراح + التركيبات التي يتم إرسالها من خلال العلاقات العامة. ** الأصول التي تم إنشاؤها، اقتراح Norito، وتحديثات `chunker_registry_data.rs` تشمل الكريں.
2. **مراجعة مجموعة عمل الأدوات.** قم بإعادة قائمة مراجعة التحقق من صحة المراجعين وتأكد من أن قواعد تسجيل المقترح تتوافق مع (عدم إعادة استخدام المعرف، استيفاء الحتمية).
3. **مغلف المجلس.** الموافقة مرة واحدة بعد ملخص مقترحات أعضاء المجلس (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) قم بالتوقيع على البطاقة والتوقيعات على مغلف الملف الشخصي الذي يمكن إلحاقه بالتركيبات التي تم تركيبها.
4. **نشر السجل.** دمج السجل وتحديث المستندات والتركيبات. الملف الشخصي الافتراضي لـ CLI هو الحل الأمثل لترحيل الحوكمة، وهو أمر جاهز.

## نصائح التأليف

- قوة اثنين هي حدود متساوية ترجع إلى سلوك تقطيع حالة الحافة.
- بذور طاولة التروس التي يمكن قراءتها بواسطة الإنسان وفريدة من نوعها عالميًا ومسارات التدقيق سهلة الاستخدام.
- قياس النتائج الفنية (مثل مقارنات الإنتاجية) التي `docs/source/sorafs/reports/` هي بمثابة مرجع مستقبلي ومرجعي.

يتم الطرح وفقًا للتوقعات التشغيلية لدفتر أستاذ الهجرة
(`docs/source/sorafs/migration_ledger.md`)۔ قواعد التوافق في وقت التشغيل
`docs/source/sorafs/chunker_conformance.md` د.