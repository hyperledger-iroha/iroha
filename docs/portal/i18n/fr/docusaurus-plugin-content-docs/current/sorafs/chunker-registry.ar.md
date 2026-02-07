---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry
title: سجل ملفات chunker في SoraFS
sidebar_label : ajouter un chunker
Description : Les liens vers les chunker sont disponibles dans SoraFS.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/chunker_registry.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة توثيق Sphinx القديمة.
:::

## Utilisez le chunker pour SoraFS (SF-2a)

Utilisez SoraFS pour le chunking en utilisant la fonction chunking.
Il y a plusieurs logiciels CDC qui semver et digest/multicodec pour les manifestes CAR.

على مؤلفي الملفات الرجوع إلى
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
للاطلاع على البيانات المطلوبة وقائمة التحقق وقالب المقترح قبل إرسال أي إدخال جديد.
وبعد أن تعتمد الحوكمة أي تغيير، اتبع
[قائمة تحقق إطلاق السجل](./chunker-registry-rollout-checklist.md) et
[manifest دليل في staging](./staging-manifest-playbook) لترقية
الـ luminaires إلى mise en scène والإنتاج.

### الملفات

| Espace de noms | الاسم | SemVer | معرف الملف | الحد الأدنى (بايت) | الهدف (بايت) | الحد الأقصى (بايت) | قناع القطع | Multihash | البدائل | ملاحظات |
|-----------|-------|--------|------------|------------------------|--------------|--------------------|-----------|---------------|--------|---------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Détails sur les luminaires SF-1 |يعيش السجل في الشيفرة ضمن `sorafs_manifest::chunker_registry` (et [`chunker_registry_charter.md`](./chunker-registry-charter.md)). ويُعبَّر عن كل إدخال كـ `ChunkerProfileDescriptor` يضم:

* `namespace` – تجميع منطقي للملفات ذات الصلة (مثل `sorafs`).
* `name` – تسمية مقروءة للبشر (`sf1`, `sf1-fast`, …).
* `semver` – سلسلة نسخة دلالية لمجموعة المعلمات.
* `profile` – `ChunkProfile` الفعلي (min/cible/max/masque).
* `multihash_code` – La fonction multihash digère le morceau (`0x1f`
  (Lire la suite sur SoraFS).

يقوم manifeste بتسلسل الملفات عبر `ChunkingProfileV1`. تسجل البنية بيانات السجل (espace de noms, nom, semver) إلى جانب
Le CDC est également présent. ينبغي على المستهلكين أولاً محاولة البحث في السجل عبر `profile_id`
والرجوع إلى المعلمات المضمّنة عند ظهور معرفات غير معروفة؛ Utiliser le protocole HTTP pour créer un lien HTTP
Le numéro de téléphone est `Accept-Chunker` دون تخمين. وتفرض قواعد ميثاق السجل أن يكون المقبض المعتمد
(`namespace.name@semver`) Il s'agit d'un `profile_aliases`, qui est également disponible.

لفحص السجل من الأدوات، شغّل CLI المساعد :

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

Utilisez la CLI pour utiliser JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) `-`, vous devez utiliser la sortie standard pour la sortie standard. يسهل ذلك تمرير
البيانات إلى الأدوات مع الحفاظ على السلوك الافتراضي لطباعة التقرير الرئيسي.

### مصفوفة التوافق وخطة الإطلاقيلخص الجدول التالي حالة الدعم الحالية لـ `sorafs.sf1@1.0.0` عبر المكونات الأساسية. يشير "Pont"
Utilisez CARv1 + SHA-256 pour créer un lien vers la version (`Accept-Chunker` + `Accept-Digest`).

| المكون | الحالة | ملاحظات |
|--------|--------|---------|
| `sorafs_manifest_chunk_store` | ✅مدعوم | يتحقق من المقبض المعتمد + البدائل، ويبث التقارير عبر `--json-out=-`, ويفرض ميثاق السجل عبر `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ قديم | مُنشئ manifeste قديم؛ استخدم `iroha app sorafs toolkit pack` pour CAR/manifeste et `--plan=-` pour إعادة التحقق الحتمية. |
| `sorafs_provider_advert_stub` | ⚠️ قديم | مساعد تحقق hors ligne فقط؛ Les annonces des fournisseurs sont publiées par `/v1/sorafs/providers`. |
| `sorafs_fetch` (orchestrateur développeur) | ✅مدعوم | `chunk_fetch_specs` est compatible avec `range` et CARv2. |
| Luminaires SDK (Rust/Go/TS) | ✅مدعوم | يُعاد توليدها عبر `export_vectors`؛ المقبض المعتمد يظهر أولاً في كل قائمة بدائل ويُوقَّع بواسطة أظرف المجلس. |
| Passerelle pour passerelle Torii | ✅مدعوم | Vous devez utiliser `Accept-Chunker` et utiliser `Content-Chunker` pour Bridge CARv1 pour rétrograder la version. |

طرح التليمترية:- **تليمترية جلب الـ chunks** — يصدر CLI الخاص بـ Iroha `sorafs toolkit pack` digests للـ chunk, وبيانات CAR, وجذور PoR لإدخالها في لوحات المتابعة.
- **Annonces de fournisseurs** — تتضمن حمولة الإعلانات بيانات القدرات والبدائل؛ Utilisez le code `/v1/sorafs/providers` (ou `range`).
- **مراقبة الـ gateway** — على المشغلين الإبلاغ عن أزواج `Content-Chunker`/`Content-Digest` لاكتشاف أي خفض غير متوقع؛ ومن المتوقع ان ينخفض ​​استخدام الـ bridge إلى الصفر قبل الإيقاف.

سياسة الإيقاف: بعد اعتماد ملف خلف، حدّد نافذة نشر مزدوجة (موثقة في المقترح) قبل وسم
`sorafs.sf1@1.0.0` est utilisé pour le pont CARv1 avec le pont CARv1.

Pour PoR, vous pouvez utiliser un morceau/un segment/une feuille comme suit :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

يمكنك اختيار ملف عبر معرف رقمي (`--profile-id=1`) et عبر المقبض المسجل
(`--profile=sorafs.sf1@1.0.0`)؛ صيغة المقبض مناسبة للسكريبتات التي تمرر namespace/name/semver
مباشرة من بيانات الحوكمة.

Utilisez `--promote-profile=<handle>` pour utiliser JSON avec les paramètres d'interface (pour accéder à la version ultérieure
المسجلة) يمكن لصقها في `chunker_registry_data.rs` عند ترقية ملف افتراضي جديد:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

يتضمن التقرير الرئيسي (وملف البرهان الاختياري) الـ digest الجذري، وbytes الأوراق المُعاينة
(مشفرة بالهيكس)، وdigests الأشقاء للـ segment/chunk byحيث يمكن للمدققين إعادة هاش طبقات
64 KiB/4 KiB par `por_root_hex`.للتحقق من برهان موجود مقابل حمولة، مرر المسار عبر
`--por-proof-verify` (interface CLI `"por_proof_verified": true` pour la connexion avec la ligne de commande) :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

لأخذ عينات على دفعات، استخدم `--por-sample=<count>` ويمكنك تمرير seed/مسار إخراج اختياري.
تضمن CLI ترتيبًا حتميًا (`splitmix64` seeded) وتقوم بالاقتطاع تلقائيًا عندما تتجاوز
الطلبات الأوراق المتاحة:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

يعكس manifest stub البيانات نفسها، وهو مناسب عند برمجة اختيار `--chunker-profile-id` في
الـ pipelines. كما تقبل CLIs الخاصة بـ chunk store صيغة المقبض المعتمد
(`--profile=sorafs.sf1@1.0.0`) لتجنب ترميز معرفات رقمية صلبة في سكريبتات البناء:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "ID_profil": 1,
    "espace de noms": "sorafs",
    "nom": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "taille_cible": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

يتطابق الحقل `handle` (`namespace.name@semver`) مع ما تقبله CLIs عبر
`--profile=…`، مما يجعل نسخه مباشرة إلى الأتمتة آمنًا.

### التفاوض على chunkers

تعلن البوابات والعملاء الملفات المدعومة عبر provider adverts:

```
FournisseurAnnonceBodyV1 {
    ...
    chunk_profile : profile_id (ضمنيًا عبر السجل)
    capacités : [...]
}
```

يُعلن جدولة chunk متعددة المصادر عبر القدرة `range`. يقبلها CLI عبر
`--capability=range[:streams]` حيث يشفّر اللاحق الرقمي الاختياري التوازي المفضل لدى المزود
لجلب النطاق (على سبيل المثال، `--capability=range:64` تعلن ميزانية 64 stream).
وعند حذفه، يعود المستهلكون إلى تلميح `max_streams` العام المنشور في مكان آخر من الإعلان.

عند طلب بيانات CAR، يجب أن يرسل العملاء ترويسة `Accept-Chunker` بقائمة tuples
`(namespace, name, semver)` بحسب ترتيب التفضيل:

```

تختار البوابات ملفًا مدعومًا من الطرفين (الافتراضي `sorafs.sf1@1.0.0`) et تعكس القرار عبر ترويسة
Titre `Content-Chunker`. تضمن manifeste الملف المختار كي تتمكن العقد اللاحقة من التحقق
Il y a des morceaux de morceaux qui sont également HTTP.

### توافق CAR

Vous pouvez utiliser le manifeste CIDv1 comme `dag-cbor` (`0x71`). وللتوافق القديم نحتفظ
Pour utiliser CARv1+SHA-2 :* **المسار الأساسي** – CARv2, digest et BLAKE3 (`0x1f` multihash)
  `MultihashIndexSorted`, il s'agit d'une solution.
  Vous devez utiliser `Accept-Chunker` ou `Accept-Digest: sha2-256`.

لكن يجب ألا تستبدل الـ digest المعتمد.

### المطابقة

* يطابق ملف `sorafs.sf1@1.0.0` الـ luminaires العامة في
  `fixtures/sorafs_chunker` والمجموعات المسجلة تحت
  `fuzz/sorafs_chunker`. Il s'agit d'une application Rust, Go et Node.
* تؤكد `chunker_registry::lookup_by_profile` أن معلمات الوصف تطابق `ChunkProfile::DEFAULT` للحماية من الانحرافات العرضية.
* Les manifestes sont `iroha app sorafs toolkit pack` et `sorafs_manifest_stub` pour la fonction.