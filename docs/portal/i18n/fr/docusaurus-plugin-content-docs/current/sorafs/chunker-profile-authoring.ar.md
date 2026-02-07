---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : création de profil chunker
title: دليل تأليف ملفات chunker في SoraFS
sidebar_label : ce chunker
description : Il s'agit d'un chunker et de luminaires pour SoraFS.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/chunker_profile_authoring.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

# دليل تأليف ملفات chunker pour SoraFS

Vous avez besoin d'un chunker pour SoraFS.
par RFC المعمارية (SF-1) et par RFC (SF-2a)
بمتطلبات تأليف واضحة وخطوات تحقق وقوالب مقترح.
للاطلاع على مثال معتمد، راجع
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
وسجل marche à sec المرافق في
`docs/source/sorafs/reports/sf1_determinism.md`.

## نظرة عامة

يجب أن يحقق كل ملف يدخل السجل ما يلي:

- Les applications CDC et les applications multihash avec multihash
- Les luminaires sont compatibles avec les SDK (JSON Rust/Go/TS + corpora fuzz + PoR) et les SDK en aval
  التحقق منها دون outillage مخصص؛
- تضمين بيانات جاهزة للحوكمة (espace de noms, nom, semver) مع إرشادات الهجرة ونوافذ التوافق؛ et
- اجتياز حزمة diff الحتمية قبل مراجعة المجلس.

اتبع قائمة التحقق أدناه لإعداد مقترح يستوفي هذه القواعد.

## ملخص ميثاق السجل

قبل صياغة المقترح، تأكد من مطابقته لميثاق السجل الذي تفرضه
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` :- معرفات الملفات أعداد صحيحة موجبة تزيد بشكل رتيب دون فجوات.
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

## البيانات المطلوبة| الحقل | الوصف | مثال (`sorafs.sf1@1.0.0`) |
|-------|-------|--------------------------------|
| `namespace` | تجميع منطقي للملفات ذات الصلة. | `sorafs` |
| `name` | تسمية مقروءة للبشر. | `sf1` |
| `semver` | سلسلة نسخة دلالية لمجموعة المعلمات. | `1.0.0` |
| `profile_id` | معرف رقمي رتيب يُسند عند إدخال الملف. احجز المعرف التالي ولا تعِد استخدام الأرقام الحالية. | `1` |
| `profile_aliases` | مقابض إضافية اختيارية (أسماء قديمة، اختصارات) تُعرض للعملاء أثناء التفاوض. يجب تضمين المقبض المعتمد أولاً. | `["sorafs.sf1@1.0.0"]` |
| `profile.min_size` | طول chunk الأدنى بالبايت. | `65536` |
| `profile.target_size` | طول chunk المستهدف بالبايت. | `262144` |
| `profile.max_size` | طول chunk الأقصى بالبايت. | `524288` |
| `profile.break_mask` | C'est un hash roulant (hex). | `0x0000ffff` |
| `profile.polynomial` | Il s'agit d'un polynôme d'engrenage (hexadécimal). | `0x3da3358b4dc173` |
| `gear_seed` | Seed لاشتقاق جدول gear بحجم 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Il s'agit d'un multihash qui digère un morceau. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest لحزمة rencontres المعتمدة. | `13fa...c482` |
| `fixtures_root` | مسار نسبي يحتوي على rencontres المعاد توليدها. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Semence لعيّنات PoR الحتمية (`splitmix64`). | `0xfeedbeefcafebabe` (مثال) |يجب أن تظهر البيانات الوصفية في وثيقة المقترح وداخل luminaires المولدة حتى يتمكن السجل
Et les outils CLI et les outils de travail sont également disponibles. عند الشك، شغّل
CLIs pour le magasin de blocs et le manifeste avec `--json-out=-` pour les fonctions de magasin de blocs et de manifeste
ملاحظات المراجعة.

### نقاط تماس CLI والسجل

- `sorafs_manifest_chunk_store --profile=<handle>` — Un morceau et un résumé de fragments
  للـ manifest وفحوص PoR مع المعلمات المقترحة.
- `sorafs_manifest_chunk_store --json-out=-` — pour ajouter un chunk-store à la sortie standard
  للمقارنات الآلية.
- `sorafs_manifest_stub --chunker-profile=<handle>` — تأكيد أن manifeste et CAR
  تتضمن المقبض المعتمد والبدائل.
- `sorafs_manifest_stub --plan=-` — إعادة تغذية `chunk_fetch_specs` السابق للتحقق من
  offsets/digests بعد التغيير.

سجّل مخرجات الأوامر (digests, جذور PoR, hashs للـ manifest) في المقترح كي يستطيع
المراجعون إعادة إنتاجها حرفياً.

## قائمة تحقق الحتمية والتحقق1. **Les calendriers de إعادة توليد**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Utilisation du système d'exploitation** — Utilisez le système `cargo test -p sorafs_chunker` et le diff du faisceau
   عبر اللغات (`crates/sorafs_chunker/tests/vectors.rs`) باللون الأخضر مع luminaires الجديدة.
3. **Fuzz/contre-pression du corps** — Produit `cargo fuzz list` et harnais de sécurité
   (`fuzz/sorafs_chunker`) على الأصول المُعاد توليدها.
4. **التحقق من شهود Preuve de récupérabilité** — شغّل
   `sorafs_manifest_chunk_store --por-sample=<n>` باستخدام الملف المقترح وأكد تطابق الجذور
   مع manifeste الخاص بالـ luminaires.
5. **Fonctionnement à sec avec CI** — شغّل `ci/check_sorafs_fixtures.sh` محلياً؛ يجب أن ينجح
   مع luminaires الجديدة و `manifest_signatures.json` الحالي.
6. **Temps d'exécution croisé** — Fonctionne avec Go/TS en utilisant JSON et en utilisant Go/TS
   حدود chunk et digère متطابقة.

وثّق الأوامر والـ digests الناتجة في المقترح كي يستطيع Tooling WG إعادة تشغيلها دون تخمين.

### تأكيد Manifeste / PoR

Dans le cadre des rencontres, il y a le manifeste de la CAR et du PoR :

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

استبدل ملف الإدخال بأي corpus ممثل مستخدم في luminaires الخاصة بك
(le flux a une taille de 1 GiB) et digère les fichiers à partir des fichiers.

## قالب المقترح

يتم تقديم المقترحات كسجلات Norito pour `ChunkerProfileProposalV1` محفوظة ضمن
`docs/source/sorafs/proposals/`. يوضح قالب JSON أدناه الشكل المتوقع
(استبدل القيم حسب الحاجة):


Il s'agit de Markdown مطابقاً (`determinism_report`) qui contient des informations et des résumés de chunk et de
انحرافات تمت ملاحظتها أثناء التحقق.

## سير عمل الحوكمة1. **تقديم PR مع المقترح + luminaires.** ضمّن الأصول المولدة، مقترح Norito, وتحديثات
   `chunker_registry_data.rs`.
2. **مراجعة Tooling WG.** يعيد المراجعون تشغيل قائمة التحقق ويتأكدون من أن المقترح
   يتوافق مع قواعد السجل (لا إعادة لاستخدام المعرفات، الحتمية متحققة).
3. **ظرف المجلس.** بعد الموافقة، يوقع أعضاء المجلس digest المقترح
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) ويضيفون توقيعاتهم إلى
   ظرف الملف المخزن مع prochains matchs.
4. **نشر السجل.** يؤدي الدمج إلى تحديث السجل والوثائق والـ luminaires. يظل CLI الافتراضي
   على الملف السابق حتى تعلن الحوكمة أن الهجرة جاهزة.
   وأبلغ المشغلين عبر registre des migrations.

## نصائح التأليف

- Il s'agit d'une méthode de chunking pour les tâches ménagères.
- Vous pouvez utiliser le multihash pour créer un manifeste et une passerelle. وأضف ملاحظة توافق عند ذلك.
- اجعل graines لجدول gear قابلة للقراءة لكنها فريدة عالمياً لتسهيل التدقيق.
- خزّن أي artefacts قياس أداء (مثل مقارنات débit) ضمن
  `docs/source/sorafs/reports/` للرجوع لاحقاً.

لتوقعات التشغيل أثناء déploiement du grand livre de migration
(`docs/source/sorafs/migration_ledger.md`). لقواعد المطابقة وقت التشغيل راجع
`docs/source/sorafs/chunker_conformance.md`.