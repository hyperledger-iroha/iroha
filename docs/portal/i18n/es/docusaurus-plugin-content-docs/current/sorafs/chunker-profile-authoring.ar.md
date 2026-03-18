---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: creación de perfiles fragmentados
título: دليل تأليف ملفات fragmentador في SoraFS
sidebar_label: fragmentador principal
descripción: قائمة تحقق لاقتراح ملفات fragmentador جديدة y accesorios في SoraFS.
---

:::nota المصدر المعتمد
Utilice el botón `docs/source/sorafs/chunker_profile_authoring.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# دليل تأليف ملفات fragmentador con SoraFS

يشرح هذا الدليل كيفية اقتراح ونشر ملفات fragmentador جديدة لـ SoraFS.
y RFC المعمارية (SF-1) y مرجع السجل (SF-2a)
بمتطلبات تأليف واضحة وخطوات تحقق وقوالب مقترح.
للاطلاع على مثال معتمد، راجع
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
وسجل funcionamiento en seco المرافق في
`docs/source/sorafs/reports/sf1_determinism.md`.

## نظرة عامة

يجب أن يحقق كل ملف يدخل السجل ما يلي:

- الإعلان عن معلمات CDC حتمية y multihash متطابقة عبر المعماريات؛
- Dispositivos de instalación de dispositivos (JSON Rust/Go/TS + corpora fuzz + PoR) en SDK descendentes
  التحقق منها دون herramientas مخصص؛
- تضمين بيانات جاهزة للحوكمة (espacio de nombres, nombre, semver) مع إرشادات الهجرة ونوافذ التوافق؛ y
- اجتياز حزمة diff الحتمية قبل مراجعة المجلس.

اتبع قائمة التحقق أدناه لإعداد مقترح يستوفي هذه القواعد.

## ملخص ميثاق السجل

قبل صياغة المقترح، تأكد من مطابقته لميثاق السجل الذي تفرضه
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:- معرفات الملفات أعداد صحيحة موجبة تزيد بشكل رتيب دون فجوات.
- يجب أن يظهر المقبض المعتمد (`namespace.name@semver`) في قائمة البدائل
  وأن يكون **الأول**. تليه البدائل القديمة (véase `sorafs.sf1@1.0.0`).
- لا يجوز لأي alias أن يتعارض مع maneja معتمد آخر أو أن يتكرر.
- يجب أن تكون alias غير فارغة ومقصوصة من المسافات.

Enlaces CLI:

```bash
# قائمة JSON بكل descripors المسجلة (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# إخراج بيانات ملف افتراضي مرشح (handle معتمد + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

تحافظ هذه الأوامر على توافق المقترحات مع ميثاق السجل وتوفر البيانات المعتمدة
اللازمة لنقاشات الحوكمة.

## البيانات المطلوبة| الحقل | الوصف | Hombre (`sorafs.sf1@1.0.0`) |
|-------|-------|---------------------------|
| `namespace` | تجميع منطقي للملفات ذات الصلة. | `sorafs` |
| `name` | تسمية مقروءة للبشر. | `sf1` |
| `semver` | سلسلة نسخة دلالية لمجموعة المعلمات. | `1.0.0` |
| `profile_id` | معرف رقمي رتيب يُسند عند إدخال الملف. احجز المعرف التالي ولا تعِد استخدام الأرقام الحالية. | `1` |
| `profile_aliases` | مقابض إضافية اختيارية (أسماء قديمة، اختصارات) تُعرض للعملاء أثناء التفاوض. يجب تضمين المقبض المعتمد أولاً. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | طول trozo الأدنى بالبايت. | `65536` |
| `profile.target_size` | طول trozo المستهدف بالبايت. | `262144` |
| `profile.max_size` | طول trozo الأقصى بالبايت. | `524288` |
| `profile.break_mask` | قناع تكيفي يستخدمه hash rodante (hexadecimal). | `0x0000ffff` |
| `profile.polynomial` | ثابت polinomio de engranajes (hexadecimal). | `0x3da3358b4dc173` |
| `gear_seed` | Semilla لاشتقاق جدول engranaje بحجم 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Un multihash digiere un fragmento. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Resumen de accesorios de لحزمة المعتمدة. | `13fa...c482` |
| `fixtures_root` | مسار نسبي يحتوي على accesorios المعاد توليدها. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semilla لعيّنات PoR الحتمية (`splitmix64`). | `0xfeedbeefcafebabe` (مثال) |يجب أن تظهر البيانات الوصفية في وثيقة المقترح وداخل accesorios المولدة حتى يتمكن السجل
Y herramientas الـ CLI وأتمتة الحوكمة من تأكيد القيم دون مطابقة يدوية. عند الشك، شغّل
Las CLI se encuentran en el almacén de fragmentos y en el manifiesto de `--json-out=-`.
ملاحظات المراجعة.

### نقاط تماس CLI y والسجل

- `sorafs_manifest_chunk_store --profile=<handle>` — إعادة تشغيل بيانات fragmento y resumen
  للـ manifiesto وفحوص PoR مع المعلمات المقترحة.
- `sorafs_manifest_chunk_store --json-out=-` — Se utiliza el almacén de fragmentos y la salida estándar
  للمقارنات الآلية.
- `sorafs_manifest_stub --chunker-profile=<handle>` — تأكيد أن manifests وخطط CAR
  تتضمن المقبض المعتمد والبدائل.
- `sorafs_manifest_stub --plan=-` — إعادة تغذية `chunk_fetch_specs` السابق للتحقق من
  compensaciones/resúmenes بعد التغيير.

سجّل مخرجات الأوامر (resúmenes, جذور PoR, hashes للـ manifiesto) في المقترح كي يستطيع
المراجعون إعادة إنتاجها حرفياً.

## قائمة تحقق الحتمية والتحقق1. ** إعادة توليد accesorios **
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **تشغيل مجموعة التكافؤ** — يجب أن تكون `cargo test -p sorafs_chunker` y arnés diferencial
   عبر اللغات (`crates/sorafs_chunker/tests/vectors.rs`) باللون الأخضر مع accesorios الجديدة.
3. **إعادة تشغيل corpora fuzz/contrapresión** — نفّذ `cargo fuzz list` y arnés البث
   (`fuzz/sorafs_chunker`) على الأصول المُعاد توليدها.
4. **التحقق من شهود Prueba de recuperabilidad** — شغّل
   `sorafs_manifest_chunk_store --por-sample=<n>` باستخدام الملف المقترح وأكد تطابق الجذور
   مع manifiesto الخاص بالـ accesorios.
5. **Funcionamiento en seco del CI** — Haga clic en `ci/check_sorafs_fixtures.sh` يجب أن ينجح
   مع accesorios الجديدة و `manifest_signatures.json` الحالي.
6. **تأكيد cross-runtime** — تأكد من أن ربط Go/TS يستهلك JSON المُعاد توليده ويُخرج
   حدود trozos y resúmenes متطابقة.

وثّق الأوامر والـ resúmenes الناتجة في المقترح كي يستطيع Tooling WG إعادة تشغيلها دون تخمين.

### تأكيد Manifiesto / PoR

بعد إعادة توليد accesorios, شغّل مسار manifiesto بالكامل لضمان بقاء بيانات CAR y PoR متسقة:

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

استبدل ملف الإدخال بأي corpus ممثل مستخدم في accesorios الخاصة بك
(Transmisión de مثلاً حتمي بحجم 1 GiB) وأرفق resúmenes الناتجة في المقترح.

## قالب المقترح

يتم تقديم المقترحات كسجلات Norito من نوع `ChunkerProfileProposalV1` محفوظة ضمن
`docs/source/sorafs/proposals/`. يوضح قالب JSON أدناه الشكل المتوقع
(استبدل القيم حسب الحاجة):


قدّم تقرير Markdown مطابقاً (`determinism_report`) يسجل مخرجات الأوامر y resúmenes de fragmentos y
انحرافات تمت ملاحظتها أثناء التحقق.

## سير عمل الحوكمة1. **تقديم PR مع المقترح + accesorios.** ضمّن الأصول المولدة, مقترح Norito, وتحديثات
   `chunker_registry_data.rs`.
2. **مراجعة Tooling WG.** يعيد المراجعون تشغيل قائمة التحقق ويتأكدون من أن المقترح
   يتوافق مع قواعد السجل (لا إعادة لاستخدام المعرفات، الحتمية متحققة).
3. **ظرف المجلس.** بعد الموافقة، يوقع أعضاء المجلس resumen المقترح
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) ويضيفون توقيعاتهم إلى
   ظرف الملف المخزن مع accesorios.
4. **نشر السجل.** يؤدي الدمج إلى تحديث السجل والوثائق والـ accesorios. يظل CLI الافتراضي
   على الملف السابق حتى تعلن الحوكمة أن الهجرة جاهزة.
   وأبلغ المشغلين عبر libro de migración.

## نصائح التأليف

- فضّل حدوداً من قوى اثنين زوجية لتقليل سلوك fragmentación في الحالات الطرفية.
- تجنب تغيير كود multihash دون تنسيق مع مستهلكي manifest y gateway؛ وأضف ملاحظة توافق عند ذلك.
- اجعل semillas لجدول engranaje قابلة للقراءة لكنها فريدة عالمياً لتسهيل التدقيق.
- خزّن أي artefactos قياس أداء (rendimiento de مثل مقارنات) ضمن
  `docs/source/sorafs/reports/` للرجوع لاحقاً.

لتوقعات التشغيل أثناء implementación del libro mayor de migración
(`docs/source/sorafs/migration_ledger.md`). لقواعد المطابقة وقت التشغيل راجع
`docs/source/sorafs/chunker_conformance.md`.