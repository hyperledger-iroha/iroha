---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: تأليف ملف تعريف مقسم
العنوان: Guia de autoria de perfis de Chunker da SoraFS
Sidebar_label: دليل أداة القطع
الوصف: قائمة التحقق الخاصة بتركيبات القطع المثالية الجديدة من SoraFS.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sorafs/chunker_profile_authoring.md`. Mantenha ambas as copias sincronzadas.
:::

# دليل دليل أداة القطع من SoraFS

تم شرح هذا الدليل خصيصًا ونشر تحسينات القطع الجديدة لـ SoraFS.
هذا هو تكملة RFC للهندسة المعمارية (SF-1) ومرجع السجل (SF-2a)
مع المتطلبات الملموسة للسلطة وخطوات التحقق ونماذج الاقتراح.
كمثال قانوني، فيجا
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
e o سجل التشغيل الجاف المرتبط به
`docs/source/sorafs/reports/sf1_determinism.md`.

## فيساو جيرال

كل ملف شخصي لا يجب عليك تسجيله:

- الإعلان عن معلمات مراكز مكافحة الأمراض والوقاية منها وتكوينات المتطابقات المتعددة المتطابقة بين
  الهندسة المعمارية.
- تركيبات محدثة (JSON Rust/Go/TS + corpora fuzz + testemunhas PoR) que
  أدوات تطوير البرمجيات لنظام التشغيل (OS) ذات المصب للتحقق من أدوات SEM متوسطة الحجم؛
- تضمين metadados prontos para حاكم (مساحة الاسم، الاسم، العلامة) جنبًا إلى جنب مع orientacao
  عمليات الطرح والتشغيل؛ ه
- تمرير مجموعة من الفروق المحددة قبل مراجعة النصيحة.

قم بإلقاء نظرة على القائمة المرجعية حتى تتمكن من إعداد اقتراح لمتابعة هذه التغييرات.

## ملخص مذكرة التسجيلقبل إعادة كتابة عرض ما، تأكد من حضورك لبطاقة التسجيل المطبقة من قبل
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- معرفات الملف الشخصي هي عبارة عن إيجابيات داخلية تزيد من الشكل الرتيب بدون ثغرات.
- يظهر المقبض canonico (`namespace.name@semver`) في قائمة الأسماء المستعارة
  **deve** هو المدخل الأول. الأسماء المستعارة البديلة (على سبيل المثال، `sorafs.sf1@1.0.0`) موجودة منذ ذلك الحين.
- يمكن استخدام الاسم المستعار Nenhum في التعامل مع Canonico أو ظهوره مرة أخرى.
- الأسماء المستعارة يجب أن تكون nao vazios e aparados de espacos em branco.

مساعدو CLI:

```bash
# Listagem JSON de todos os descritores registrados (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Emitir metadados para um perfil default candidato (handle canonico + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Esses comandos mantem as propostas alinhadas com a carta do registro e fornecem os
Metadados Canonicos اللازمة لمناقشات الحوكمة.

## المتطلبات الوصفية| كامبو | وصف | مثال (`sorafs.sf1@1.0.0`) |
|-------|---------------------------|----------|
| `namespace` | تجميع المنطق للأداء المرتبط. | `sorafs` |
| `name` | Rotulo Legali Para Humanos. | `sf1` |
| `semver` | مجموعة من العبارات الدلالية لمجموعة من المعلمات. | `1.0.0` |
| `profile_id` | يتم تخصيص المعرّف الرقمي الرتيب عند إدخال الملف الشخصي. احتفظ بالمعرف التالي ولكن لا يمكنك إعادة استخدام الأرقام الموجودة. | `1` |
| `profile_aliases` | يعالج الإضافات الاختيارية (الأسماء البديلة والمختصرة) التي توضح للعملاء أثناء التفاوض. يشتمل دائمًا على مقبض Canonico كالمدخل الأول. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | قم بالاشتراك في الحد الأدنى من قطع البايتات. | `65536` |
| `profile.target_size` | قم بالاشتراك في قطع البايتات. | `262144` |
| `profile.max_size` | قم بالاشتراك في الحد الأقصى من قطع البايتات. | `524288` |
| `profile.break_mask` | ماسكارا أدابتاتيفا أوسادا بيلو رولنج هاش (ست عشرية). | `0x0000ffff` |
| `profile.polynomial` | كونستانتي دو بولينوميو جير (عرافة). | `0x3da3358b4dc173` |
| `gear_seed` | يتم استخدام البذور للحصول على ترس جدولي يبلغ 64 كيلو بايت. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Codigo multihash لهضم القطعة. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | ملخص حزمة تركيبات Canonico. | `13fa...c482` || `fixtures_root` | المديرية النسبية تتنافس على المباريات المتجددة. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed para amostragem PoR حتمية (`splitmix64`). | `0xfeedbeefcafebabe` (مثال) |

قد تظهر التوصيفات بشكل كبير بدون مستند اقتراح بقدر ما يوجد في التركيبتين الجيدتين
لكي يؤكد السجل أو أدوات CLI أو الإدارة التلقائية القيم على حد سواء
Cruzamentos manuais. في حالة حدوث ذلك، قم بتنفيذ CLIs الخاصة بـ Chunk-Store والبيان com
`--json-out=-` لإرسال البيانات الوصفية المحسوبة لملاحظات المراجعة.

### نقاط الاتصال والتسجيل في CLI

- `sorafs_manifest_chunk_store --profile=<handle>` - إعادة تنفيذ البيانات التعريفية للقطعة،
  ملخص القيام بالبيان والتحقق من PoR مع المعلمات المقترحة.
- `sorafs_manifest_chunk_store --json-out=-` - جهاز الإرسال أو الارتباط الخاص بتخزين القطع
  stdout للمقارنات التلقائية.
- `sorafs_manifest_stub --chunker-profile=<handle>` - لتأكيد إظهار مخطط السيارة
  embutem أو التعامل مع canonico mais الأسماء المستعارة.
- `sorafs_manifest_stub --plan=-` - قم بالرجوع إلى `chunk_fetch_specs` الأمامي للفقرة
  التحقق من الإزاحة/الخلاصات في المستقبل.

قم بتسجيل كلمة dos comandos (الملخصات، ورفع PoR، وتجزئة البيان) على النحو المقترح
يمكن للمراجعة إعادة إنتاجها حرفيًا.

## قائمة التحقق من التحديد والتحقق1. **التركيبات المتجددة**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **تنفيذ مجموعة من الحواجز** - `cargo test -p sorafs_chunker` وفرق الحزام
   عبر اللغات (`crates/sorafs_chunker/tests/vectors.rs`) مع نظام التشغيل verdes ficar
   تركيبات نوفوس لا لوغار.
3. **إعادة تنفيذ الزغب/الضغط الخلفي** - تنفيذ `cargo fuzz list` وأداة التثبيت
   البث (`fuzz/sorafs_chunker`) مقابل تجديد الأصول.
4. **شهادة التحقق تحتوي على إثبات قابلية الاسترجاع** - نفذ
   `sorafs_manifest_chunk_store --por-sample=<n>` يتم استخدام الملف المقترح والتأكيد عليه
   كما يرفع المراسلات إلى بيان التركيبات.
5. **التشغيل الجاف لـ CI** - تنفيذ `ci/check_sorafs_fixtures.sh` محليًا؛ يا النصي
   يجب أن يكون هناك نجاح كبير مع التركيبات الجديدة ووجود `manifest_signatures.json`.
6. **تأكيد وقت التشغيل المشترك** - تأكد من أن الروابط Go/TS تستهلك JSON
   يتم تجديدها وإخراجها بحدود وهضم متطابق.

قم بتوثيق الأوامر والخلاصات الناتجة بما يقترحه Tooling WG
reexecuta-los sem adivinhacoes.

### تأكيد البيان / PoR

بعد تجديد التركيبات، قم بتنفيذ خط أنابيب كامل للبيان لضمان ذلك
metadados CAR e provas PoR يتسق باستمرار:

```bash
# Validar metadados de chunk + PoR com o novo perfil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Gerar manifest + CAR e capturar chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Reexecutar usando o plano de fetch salvo (evita offsets obsoletos)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

استبدال ملف الإدخال لأي مجموعة تمثيلية تستخدم تركيباتنا الخاصة
(على سبيل المثال، تيار محدد من 1 جيجا بايت) وملحق الملخصات الناتجة عن الاقتراح.

## نموذج الاقتراحكما هو مقترح مع الشروط الفرعية مثل السجلات Norito `ChunkerProfileProposalV1` المسجلين
`docs/source/sorafs/proposals/`. قم بإضافة قالب JSON إلى الصورة أو التنسيق المتوقع
(substitua seus valores conforme necessario):


للحصول على علاقة Markdown المقابلة (`determinism_report`) التي تلتقط
قال دوس كوماندوس، هضم قطعة و quaisquer desvios encontrados خلال validacao.

## تدفق الحكم

1. **مقياس فرعي للعلاقات العامة مع العرض والتركيبات.** يشمل العرض والأصول الجيدة
   Norito وتم تحديثه في `chunker_registry_data.rs`.
2. **مراجعة Tooling WG.** تتم إعادة تنفيذ المراجعة من خلال قائمة التحقق من الصحة والتأكيد
   يجب أن يكون الاقتراح مطابقًا لشروط التسجيل (بدون إعادة استخدام الهوية، تحديد ما يرضي).
3. **مغلف للاستشارة.** بمجرد الموافقة، يجب على أعضاء الاستشارة أن يستوعبوا الأمر
   الاقتراح (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) والإجابة على سؤالك
   Assinaturas ao مغلف لملف أرمازينادو جنبًا إلى جنب مع التركيبات.
4. ** عامة التسجيل. ** دمج تحديث السجل والمستندات والتركيبات. يا كلي
   لا يوجد ملف دائم افتراضي قبل أن تعلن الحكومة عن هجرة برونتا.
5. **إلغاء الإهمال.** بعد تسجيل الهجرة، قم بتحديث التسجيل

## ديكاس دي أوتوريا- اختر حدودًا مزدوجة من الطاقة لتقليل سلوك التقطيع في اللوحات.
- تجنب التعديل أو التشفير المتعدد من خلال تنسيق مستهلكي البيان والبوابة؛ تشمل أوما
  nota Operational quando fizer isso.
- الحفاظ عليها كبذور من الأدوات القانونية للإنسان، ولكن على مستوى العالم فريدة من نوعها لتبسيط المستمعين.
- أرمازين أرتيفاتوس دي قياس الأداء (على سبيل المثال، مقارنات الإنتاجية).
  `docs/source/sorafs/reports/` للإشارة إلى المستقبل.

بالنسبة للعمليات المتوقعة أثناء الطرح، راجع دفتر الأستاذ الخاص بالترحيل
(`docs/source/sorafs/migration_ledger.md`). من أجل الالتزام بالقواعد في وقت التشغيل، أصبح الأمر ممكنًا
`docs/source/sorafs/chunker_conformance.md`.