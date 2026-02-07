---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: تأليف ملف تعريف مقسم
العنوان: دليل إنشاء ملفات التعريف Chunker SoraFS
Sidebar_label: دليل إنشاء القطعة
الوصف: قائمة مرجعية لمقترح الملفات الشخصية الجديدة chunker SoraFS والتركيبات.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/chunker_profile_authoring.md`. قم بمزامنة النسختين حتى تكتمل مجموعة أبو الهول الموروثة.
:::

# دليل إنشاء ملفات التعريف Chunker SoraFS

يشرح هذا الدليل مقترح التعليقات وينشر الملفات الشخصية الجديدة لـ SoraFS.
أكمل بنية RFC (SF-1) ومرجع التسجيل (SF-2a)
مع متطلبات معالجة الخرسانة وخطوات التحقق ونماذج الاقتراح.
من أجل مثال قانوني، انظر
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
وسجل التشغيل الجاف مرتبط به
`docs/source/sorafs/reports/sf1_determinism.md`.

## عرض الفرقة

كل ملف تعريف يدخل في السجل يفعل:

- إعلان عن المعلمات التي يحددها مركز السيطرة على الأمراض (CDC) والضوابط المتعددة المتطابقة بين
  أبنية؛
- توفير التركيبات المتجددة (JSON Rust/Go/TS + corpora fuzz + témoins PoR) التي
  يمكن لأدوات تطوير البرامج (SDK) المتاحة التحقق من عدم وجود أدوات للقياس؛
- تضمين عروض métadonnées من أجل الإدارة (مساحة الاسم، الاسم، كل يوم) بالإضافة إلى ذلك
  نصائح التشغيل والنوافذ التشغيلية؛ وآخرون
- قم بتمرير مجموعة الفروق المحددة قبل مراجعة المجلس.قم بمتابعة القائمة المرجعية اللازمة لإعداد اقتراح يحترم هذه القواعد.

## انظر جدول التسجيل

قبل تعديل الاقتراح، تأكد من احترام لائحة التسجيل المطبقة
الاسمية `sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- معرف الملف الشخصي عبارة عن مجموعة من الإيجابيات التي تزيد من المظهر الرتيب بدون إزعاج.
- يظهر المقبض canonique (`namespace.name@semver`) في قائمة الأسماء المستعارة
- لا يمكن لأي اسم مستعار أن يدخل في تصادم مع مقبض آخر أو جهاز آخر.
- يجب أن يكون الاسم المستعار غير مرئي ومقطع للمسافات.

أدوات مساعدة CLI:

```bash
# Listing JSON de tous les descripteurs enregistrés (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Émettre des métadonnées pour un profil par défaut candidat (handle canonique + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

هذه الأوامر تحافظ على المقترحات المتوافقة مع لائحة التسجيل والمقدمة
البيانات الأساسية الضرورية لمناقشات الحوكمة.

## يتطلب Métadonnées| بطل | الوصف | مثال (`sorafs.sf1@1.0.0`) |
|-------|------------|-----------------------------|
| `namespace` | إعادة تجميع منطق الملفات الشخصية الكاذبة. | `sorafs` |
| `name` | Libellé مقبولة. | `sf1` |
| `semver` | سلسلة الإصدار الدلالي لمجموعة الإعدادات. | `1.0.0` |
| `profile_id` | يُنسب المعرف الرقمي الرتيب إلى ملف تعريف متكامل. احتفظ بالمعرف التالي ولا تستخدم الأرقام الموجودة. | `1` |
| `profile.min_size` | قم بإطالة الحد الأدنى من القطع والبايت. | `65536` |
| `profile.target_size` | إطالة حجم القطعة والبايت. | `262144` |
| `profile.max_size` | الحد الأقصى للقطعة والبايت هو الحد الأقصى. | `524288` |
| `profile.break_mask` | يتم استخدام قناع التكيف من خلال التجزئة المتداول (ست عشري). | `0x0000ffff` |
| `profile.polynomial` | ترس كونستانتي دو بولينوم (ست عشري). | `0x3da3358b4dc173` |
| `gear_seed` | يتم استخدام البذور للحصول على ترس جدولي يبلغ 64 كيلو بايت. | `sorafs-v1-gear` |
| `chunk_multihash.code` | رمز multihash من أجل الهضم على قدم المساواة. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | ملخص الحزمة الكنسي للتركيبات. | `13fa...c482` |
| `fixtures_root` | يحتوي المرجع على التركيبات المُعاد إنشاؤها. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | بذور من أجل échantillonnage PoR déterministe (`splitmix64`). | `0xfeedbeefcafebabe` (مثال) |تظهر العناصر المطلوبة مرة أخرى في مستند الاقتراح وفي داخل المنزل
تم إنشاء التركيبات لتمكين التسجيل وأدوات CLI وأتمتة الإدارة
يؤكد les valeurs sans recoupements manuels. في حالة القيام بذلك، قم بتنفيذ CLIs Chunk-store et
يظهر مع `--json-out=-` لبث البيانات المحسوبة في مذكرات المراجعة.

### نقاط الاتصال CLI والتسجيل

- `sorafs_manifest_chunk_store --profile=<handle>` — إعادة ربط القطع المعدنية،
  ملخص البيان والفحوصات مع الإعدادات المقترحة.
- `sorafs_manifest_chunk_store --json-out=-` — جهاز البث le Rapport Chunk-store vers
  stdout للمقارنات التلقائية.
- `sorafs_manifest_stub --chunker-profile=<handle>` - تأكيد البيانات والملفات
  خطط CAR تمنع التعامل مع Canonique et les aliases.
- `sorafs_manifest_stub --plan=-` — أعد إدخال `chunk_fetch_specs` السابقة من أجل
  التحقق من الإزاحات/الملخصات بعد التعديل.

قم بإرسال الأوامر (الملخصات، الجذور، تجزئات البيان) في الاقتراح الموجود
يمكن للمراجعين إعادة إنتاج الكلمة لكل كلمة.

## تحديد قائمة التحقق والتحقق من صحتها1. **تجديد التركيبات**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **تنفيذ مجموعة التكافؤ** — `cargo test -p sorafs_chunker` والحزام المختلف
   عبر اللغات (`crates/sorafs_chunker/tests/vectors.rs`) doivent être verts avec les
   تركيبات جديدة في مكانها.
3. **استمتع بالزغب/الضغط الخلفي** — قم بتنفيذ `cargo fuzz list` وحزام الأمان
   البث (`fuzz/sorafs_chunker`) للأصول المُعاد إنشاؤها.
4. **Vérifier les témoins Proof-of-Retrievability** — تنفيذ
   `sorafs_manifest_chunk_store --por-sample=<n>` مع الملف التعريفي المقترح والتأكيد عليه
   مراسل السباقات في بيان التركيبات.
5. **التشغيل الجاف CI** — موضع invoquez `ci/check_sorafs_fixtures.sh`؛ البرنامج النصي
   يجب إعادة العمل باستخدام التركيبات الجديدة و`manifest_signatures.json` الموجودة.
6. **التأكيد خلال وقت التشغيل** — تأكد من أن روابط Go/TS ستستهلك JSON
   متجدد ومخفف للحدود والهضم المتماثل.

قم بتوثيق الأوامر والخلاصات الناتجة في الاقتراح حتى يتمكن Tooling WG من العمل
les rejouer بلا تخمين.

### بيان التأكيد / PoR

بعد تجديد التركيبات، قم بتنفيذ بيان خط الأنابيب بشكل كامل لضمان ذلك
métadonnées CAR و les preuves PoR متماسكة :

```bash
# Valider les métadonnées chunk + PoR avec le nouveau profil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Générer manifest + CAR et capturer les chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Relancer avec le plan de fetch sauvegardé (évite les offsets obsolètes)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

استبدل ملف الإدخال بالمجموعة الممثلة المستخدمة من خلال تركيباتك
(على سبيل المثال، التدفق المحدد لـ 1 جيجا بايت) وقم بربط الملخصات الناتجة بالاقتراح.

## نموذج الاقتراحيتم تقديم الاقتراحات على شكل سجلات Norito `ChunkerProfileProposalV1` المودعة في
`docs/source/sorafs/proposals/`. يوضح قالب JSON ci-dessous شكل الحضور
(استبدل القيمة التي تحتاجها):


قم بإعداد تقرير مراسل Markdown (`determinism_report`) الذي يلتقط عملية الإزالة
يتم لقاء الأوامر وخلاصات القطعة وكل التباعد عند التحقق من صحتها.

## تدفق الحكم

1. **الحصول على علاقات عامة مع الاقتراح + التركيبات.** يشمل الأصول المولدة،
   الاقتراح Norito وآخر المستجدات في `chunker_registry_data.rs`.
2. **Revue Tooling WG.** يجدد المراجعون قائمة التحقق من الصحة والتأكيد
   أن الاقتراح يحترم قواعد التسجيل (pas de réutilisation d'id،
   تحديد الرضا).
3. **مغلف المجلس.** تمت الموافقة عليه مرة أخرى، ويوقع أعضاء المجلس على الملخص
   دي لا الاقتراح (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) وملحقاته
   توقيعاتهم على مغلف الملف الشخصي مخزنة مع التركيبات.
4. **نشر السجل.** يتم الدمج مع السجل والمستندات والتركيبات يوميًا.
   يبقى CLI بشكل افتراضي على الملف الشخصي السابق حتى تعلن الحكومة عن ذلك
   الهجرة مقدما.
5. **متابعة التخفيض.** بعد نافذة الترحيل، قم بالتسجيل حاليًا من أجل
   دي الهجرة.

##مبادئ الخلق- تفضيل قوى التحمل من زوجين لتقليل سلوك التقطيع على اللوح.
- تجنب تغيير رمز multihash بدون تنسيق بيان العملاء والبوابة؛
  قم بتضمين ملاحظة عملية عند القيام بذلك.
- قم بتزويد بذور الجدول بأدوات أكثر عالمية وفريدة من نوعها لتبسيط عمليات التدقيق.
- قم بتخزين جميع أدوات القياس (على سبيل المثال، مقارنات الديون).
  `docs/source/sorafs/reports/` للإشارة إلى المستقبل.

من أجل تنبيهات العمليات أثناء الطرح، راجع دفتر الأستاذ الخاص بالترحيل
(`docs/source/sorafs/migration_ledger.md`). من أجل قواعد المطابقة لوقت التشغيل، انظر
`docs/source/sorafs/chunker_conformance.md`.