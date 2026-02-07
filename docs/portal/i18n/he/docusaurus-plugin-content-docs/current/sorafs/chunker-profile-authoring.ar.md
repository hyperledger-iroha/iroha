---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-profile-authoring
title: دليل تأليف ملفات chunker في SoraFS
sidebar_label: دليل تأليف chunker
description: قائمة تحقق لاقتراح ملفات chunker جديدة و fixtures في SoraFS.
---

:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/chunker_profile_authoring.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# دليل تأليف ملفات chunker في SoraFS

يشرح هذا الدليل كيفية اقتراح ونشر ملفات chunker جديدة لـ SoraFS.
وهو يكمل RFC المعمارية (SF-1) ومرجع السجل (SF-2a)
بمتطلبات تأليف واضحة وخطوات تحقق وقوالب مقترح.
للاطلاع على مثال معتمد، راجع
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
وسجل dry-run المرافق في
`docs/source/sorafs/reports/sf1_determinism.md`.

## نظرة عامة

يجب أن يحقق كل ملف يدخل السجل ما يلي:

- الإعلان عن معلمات CDC حتمية وإعدادات multihash متطابقة عبر المعماريات؛
- شحن fixtures قابلة لإعادة التشغيل (JSON Rust/Go/TS + corpora fuzz + شهود PoR) يمكن لـ SDKs downstream
  التحقق منها دون tooling مخصص؛
- تضمين بيانات جاهزة للحوكمة (namespace, name, semver) مع إرشادات الهجرة ونوافذ التوافق؛ ו
- اجتياز حزمة diff الحتمية قبل مراجعة المجلس.

اتبع قائمة التحقق أدناه لإعداد مقترح يستوفي هذه القواعد.

## ملخص ميثاق السجل

قبل صياغة المقترح، تأكد من مطابقته لميثاق السجل الذي تفرضه
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- معرفات الملفات أعداد صحيحة موجبة تزيد بشكل رتيب دون فجوات.
- يجب أن يظهر المقبض المعتمد (`namespace.name@semver`) في قائمة البدائل
  وأن يكون **الأول**. تليه البدائل القديمة (مثل `sorafs.sf1@1.0.0`).
- لا يجوز لأي alias أن يتعارض مع handle معتمد آخر أو أن يتكرر.
- يجب أن تكون alias غير فارغة ومقصوصة من المسافات.

مساعدات CLI المفيدة:

```bash
# قائمة JSON بكل descripors المسجلة (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# إخراج بيانات ملف افتراضي مرشح (handle معتمد + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

تحافظ هذه الأوامر على توافق المقترحات مع ميثاق السجل وتوفر البيانات المعتمدة
اللازمة لنقاشات الحوكمة.

## البيانات المطلوبة

| الحقل | אוטו | מאול (`sorafs.sf1@1.0.0`) |
|-------|-------|--------------------------------|
| `namespace` | تجميع منطقي للملفات ذات الصلة. | `sorafs` |
| `name` | تسمية مقروءة للبشر. | `sf1` |
| `semver` | سلسلة نسخة دلالية لمجموعة المعلمات. | `1.0.0` |
| `profile_id` | معرف رقمي رتيب يُسند عند إدخال الملف. احجز المعرف التالي ولا تعِد استخدام الأرقام الحالية. | `1` |
| `profile_aliases` | مقابض إضافية اختيارية (أسماء قديمة، اختصارات) تُعرض للعملاء أثناء التفاوض. يجب تضمين المقبض المعتمد أولاً. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | طول chunk الأدنى بالبايت. | `65536` |
| `profile.target_size` | طول chunk المستهدف بالبايت. | `262144` |
| `profile.max_size` | طول chunk الأقصى بالبايت. | `524288` |
| `profile.break_mask` | قناع تكيفي يستخدمه rolling hash (hex). | `0x0000ffff` |
| `profile.polynomial` | ثابت gear polynomial (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed لاشتقاق جدول gear بحجم 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | كود multihash لِـ digests لكل chunk. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest لحزمة fixtures المعتمدة. | `13fa...c482` |
| `fixtures_root` | مسار نسبي يحتوي على fixtures المعاد توليدها. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed لعيّنات PoR الحتمية (`splitmix64`). | `0xfeedbeefcafebabe` (מדי) |يجب أن تظهر البيانات الوصفية في وثيقة المقترح وداخل fixtures المولدة حتى يتمكن السجل
و tooling الـ CLI وأتمتة الحوكمة من تأكيد القيم دون مطابقة يدوية. عند الشك، شغّل
CLIs الخاصة بـ chunk-store و manifest مع `--json-out=-` لبث البيانات المحسوبة إلى
ملاحظات المراجعة.

### نقاط تماس CLI والسجل

- `sorafs_manifest_chunk_store --profile=<handle>` — إعادة تشغيل بيانات chunk و digest
  للـ manifest وفحوص PoR مع المعلمات المقترحة.
- `sorafs_manifest_chunk_store --json-out=-` — بث تقرير chunk-store إلى stdout
  للمقارنات الآلية.
- `sorafs_manifest_stub --chunker-profile=<handle>` — تأكيد أن manifests وخطط CAR
  تتضمن المقبض المعتمد والبدائل.
- `sorafs_manifest_stub --plan=-` — إعادة تغذية `chunk_fetch_specs` السابق للتحقق من
  offsets/digests بعد التغيير.

سجّل مخرجات الأوامر (digests، جذور PoR، hashes للـ manifest) في المقترح كي يستطيع
المراجعون إعادة إنتاجها حرفياً.

## قائمة تحقق الحتمية والتحقق

1. **إعادة توليد fixtures**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **تشغيل مجموعة التكافؤ** — يجب أن تكون `cargo test -p sorafs_chunker` و harness diff
   عبر اللغات (`crates/sorafs_chunker/tests/vectors.rs`) باللون الأخضر مع fixtures الجديدة.
3. **إعادة تشغيل corpora fuzz/back-pressure** — نفّذ `cargo fuzz list` و harness البث
   (`fuzz/sorafs_chunker`) على الأصول المُعاد توليدها.
4. **التحقق من شهود Proof-of-Retrievability** — شغّل
   `sorafs_manifest_chunk_store --por-sample=<n>` باستخدام الملف المقترح وأكد تطابق الجذور
   مع manifest الخاص بالـ fixtures.
5. **Dry run للـ CI** — شغّل `ci/check_sorafs_fixtures.sh` محلياً؛ يجب أن ينجح
   مع fixtures الجديدة و `manifest_signatures.json` الحالي.
6. **تأكيد cross-runtime** — تأكد من أن ربط Go/TS يستهلك JSON المُعاد توليده ويُخرج
   נתח ועיכול.

وثّق الأوامر والـ digests الناتجة في المقترح كي يستطيع Tooling WG إعادة تشغيلها دون تخمين.

### تأكيد Manifest / PoR

بعد إعادة توليد fixtures، شغّل مسار manifest بالكامل لضمان بقاء بيانات CAR و PoR متسقة:

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

استبدل ملف الإدخال بأي corpus ممثل مستخدم في fixtures الخاصة بك
(مثلاً stream حتمي بحجم 1 GiB) وأرفق digests الناتجة في المقترح.

## قالب المقترح

يتم تقديم المقترحات كسجلات Norito من نوع `ChunkerProfileProposalV1` محفوظة ضمن
`docs/source/sorafs/proposals/`. يوضح قالب JSON أدناه الشكل المتوقع
(استبدل القيم حسب الحاجة):


قدّم تقرير Markdown مطابقاً (`determinism_report`) يسجل مخرجات الأوامر و digests للـ chunk وأي
انحرافات تمت ملاحظتها أثناء التحقق.

## سير عمل الحوكمة

1. **تقديم PR مع المقترح + fixtures.** ضمّن الأصول المولدة، مقترح Norito، وتحديثات
   `chunker_registry_data.rs`.
2. **مراجعة Tooling WG.** يعيد المراجعون تشغيل قائمة التحقق ويتأكدون من أن المقترح
   يتوافق مع قواعد السجل (لا إعادة لاستخدام المعرفات، الحتمية متحققة).
3. **ظرف المجلس.** بعد الموافقة، يوقع أعضاء المجلس digest المقترح
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) ويضيفون توقيعاتهم إلى
   ظرف الملف المخزن مع fixtures.
4. **نشر السجل.** يؤدي الدمج إلى تحديث السجل والوثائق والـ fixtures. يظل CLI الافتراضي
   على الملف السابق حتى تعلن الحوكمة أن الهجرة جاهزة.
   ספר חשבונות העברה.

## نصائح التأليف- فضّل حدوداً من قوى اثنين زوجية لتقليل سلوك chunking في الحالات الطرفية.
- تجنب تغيير كود multihash دون تنسيق مع مستهلكي manifest و gateway؛ وأضف ملاحظة توافق عند ذلك.
- اجعل seeds لجدول gear قابلة للقراءة لكنها فريدة عالمياً لتسهيل التدقيق.
- خزّن أي artefacts قياس أداء (مثل مقارنات throughput) ضمن
  `docs/source/sorafs/reports/` للرجوع لاحقاً.

لتوقعات التشغيل أثناء rollout راجع migration ledger
(`docs/source/sorafs/migration_ledger.md`). لقواعد المطابقة وقت التشغيل راجع
`docs/source/sorafs/chunker_conformance.md`.