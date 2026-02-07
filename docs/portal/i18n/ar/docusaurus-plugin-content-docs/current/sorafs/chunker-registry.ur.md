---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: سجل المقطع
العنوان: تسجيل ملف تعريف مقسم SoraFS
Sidebar_label: تسجيل المقطع
الوصف: تسجيل مقطع SoraFS لمعرفات الملفات الشخصية والمعلمات وخطة التفاوض.
---

:::ملاحظة مستند ماخذ
:::

## سجل ملف تعريف مقسم SoraFS (SF-2a)

سلوك تقطيع المكدس SoraFS هو نوع من التسجيل بمساحة الاسم الذي يمكنك التفاوض بشأنه.
يتم استخدام معلمات CDC المحددة للملف الشخصي، وبيانات التعريف الكاملة، والملخص المتوقع/تعيين برامج الترميز المتعدد، وبيانات البيانات وأرشيفات CAR.

مؤلفو الملف الشخصي کو
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
نحن بحاجة إلى البيانات الوصفية وقائمة التحقق من الصحة ونموذج الاقتراح الذي تم تقديمه قبل إرسال الإدخالات الجديدة.
يجب الموافقة على الحكم تبدیلی کر دے تو
[قائمة التحقق من بدء تشغيل التسجيل](./chunker-registry-rollout-checklist.md) و
[دليل التشغيل الواضح للتدريج](./staging-manifest-playbook) الذي يتوافق مع التركيبات والتجهيزات والإنتاج يمكن أن يروج للكر.

### الملفات الشخصية

| مساحة الاسم | الاسم | سيمفير | معرف الملف الشخصي | دقيقة (بايت) | الهدف (بايت) | ماكس (بايت) | قناع الكسر | مولتيهاش | الأسماء المستعارة | ملاحظات |
|-----------|------|--------|-----------|----------------|------------|------------|---------|--------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | يتم استخدام تركيبات SF-1 والملف التعريفي الكنسي |رمز التسجيل `sorafs_manifest::chunker_registry` موجود بالفعل (يحكم [`chunker_registry_charter.md`](./chunker-registry-charter.md) الكرتا). قم بإدخال `ChunkerProfileDescriptor` كما هو موضح أدناه:

* `namespace` – ملفات تعريف الارتباط والتجميع المنطقي (مثلاً `sorafs`).
* `name` – انسان کے لیے تسمية الملف الشخصي القابلة للقراءة (`sf1`, `sf1-fast`, …)۔
* `semver` - مجموعة المعلمات لسلسلة الإصدار الدلالي.
* `profile` – اصل `ChunkProfile` (الحد الأدنى/الهدف/الحد الأقصى/القناع)۔
* `multihash_code` – هضم القطعة بناتے وقت الاستخدام ہونے والا multihash (`0x1f`
  SoraFS الافتراضي).

البيان `ChunkingProfileV1` يقوم بتسلسل الملفات الشخصية. هيكل بيانات تعريف التسجيل
(مساحة الاسم، الاسم، الفصل) عبارة عن معلمات مركز السيطرة على الأمراض (CDC) الأولية وقائمة الأسماء المستعارة الأولية التي تسجل كرتا.
سيتمكن المستهلكون من الحصول على `profile_id` من خلال البحث في السجل وما إذا كانت المعرفات غير المعروفة ستوفر لك المعلمات المضمنة للرجوع الاحتياطي؛

تقوم أدوات التسجيل بفحص الملف المساعد CLI:

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

CLI هي كل الأعلام التي تستخدم JSON (`--json-out`، `--por-json-out`، `--por-proof-out`،
`--por-sample-out`) المسار إلى `-` قبول کرتے ہیں، جس سے الحمولة stdout پر دفق ہوتا ہے بجائے فائل بنانے کے.
هذه الأدوات عبارة عن أنابيب بيانات سهلة الاستخدام وتقرير رئيسي عن النشأة وتصحيح السلوك الافتراضي.```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub یہی data mirror کرتا ہے، جو pipelines میں `--chunker-profile-id` selection کو script کرنے کے لیے convenient ہے۔ دونوں chunk store CLIs canonical handle form (`--profile=sorafs.sf1@1.0.0`) بھی accept کرتے ہیں تاکہ build scripts numeric IDs hard-code کرنے سے بچ سکیں:

```
```

`handle` field (`namespace.name@semver`) وہی ہے جو CLIs `--profile=…` کے ذریعے accept کرتے ہیں، اس لیے اسے automation میں براہ راست copy کرنا محفوظ ہے۔

### Negotiating chunkers

Gateways اور clients provider adverts کے ذریعے supported profiles advertise کرتے ہیں:

```
```

Multi-source chunk scheduling `range` capability کے ذریعے announce ہوتی ہے۔ CLI اسے `--capability=range[:streams]` کے ساتھ accept کرتا ہے، جہاں optional numeric suffix provider کی preferred range-fetch concurrency encode کرتا ہے (مثلاً `--capability=range:64` 64-stream budget advertise کرتا ہے)۔ جب یہ omit ہو تو consumers advert میں کہیں اور شائع شدہ general `max_streams` hint پر fallback کرتے ہیں۔

CAR data request کرتے وقت clients کو `Accept-Chunker` header بھیجنا چاہیے جو preference order میں `(namespace, name, semver)` tuples list کرے:

```

البوابات ملف تعريف مدعوم بشكل متبادل منتخب البطاقة (الافتراضي `sorafs.sf1@1.0.0`) ورأس الاستجابة `Content-Chunker` يعكس البطاقة. البيانات ملف التعريف المنتخب تضمين البطاقة العقد النهائية مفاوضات HTTP عدم الحظر حظر تخطيط القطعة التحقق من صحة کر سکیں.



* **المسار الأساسي** - ملخص الحمولة النافعة CARv2، BLAKE3 (`0x1f` multihash)،
  `MultihashIndexSorted`، وملف تعريف القطعة أولًا يتوافق مع السجل ہوتا ہے.


### المطابقة

* التركيبات العامة للملف الشخصي `sorafs.sf1@1.0.0` (`fixtures/sorafs_chunker`) و`fuzz/sorafs_chunker` تحت تطابق سجل الهيئات. التكافؤ الشامل الصدأ، الذهاب والعقدة يقومان بإجراء اختبارات كتمرين.
* `chunker_registry::lookup_by_profile` تأكيد التباين العرضي ومعلمات الواصف `ChunkProfile::DEFAULT` تطابق الاختلاف العرضي.
* يعرض `iroha app sorafs toolkit pack` و`sorafs_manifest_stub` بيانات تعريف التسجيل التي تتضمن معلومات إضافية.