---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf1-determinism.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: SoraFS SF1 الحتمية التشغيل الجاف
ملخص: قائمة المراجعة والخلاصات متاحة للتحقق من الملف التعريفي الخاص بـ Canonique `sorafs.sf1@1.0.0`.
---

# SoraFS SF1 الحتمية التشغيل الجاف

يلتقط هذا التقرير التشغيل الأساسي للملف التعريفي الخاص بـ Canonique
`sorafs.sf1@1.0.0`. يجب على Tooling WG إعادة تعيين قائمة التحقق من خلال ذلك
التحقق من تحديثات التركيبات أو خطوط أنابيب المستهلكين الجدد.
قم بإرسال نتيجة كل أمر على اللوحة من أجل الحفاظ على واحدة
تتبع للتدقيق.

## قائمة المراجعة| إيتاب | أمر | نتائج الحضور | ملاحظات |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | تم اجتياز جميع الاختبارات ؛ اختبار التكافؤ `vectors` يتكرر. | تأكد من أن التركيبات الأساسية متوافقة ومتوافقة مع تطبيق Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | فرز البرنامج النصي en 0 ; قم بتقرير خلاصات البيان بوضوح. | تحقق من أن التركيبات تم تجديدها بشكل صحيح وأن التوقيعات لا تزال مرفقة. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | المدخل إلى `sorafs.sf1@1.0.0` يتوافق مع واصف التسجيل (`profile_id=1`). | تأكد من مزامنة بيانات تعريف التسجيل. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | يتم إعادة التجديد بدون `--allow-unsigned` ; تظل ملفات البيان والتوقيع قابلة للتغيير. | قم بتوفير إجراء تحديد لحدود القطع والبيانات. |
| 5 | `node scripts/check_sf1_vectors.mjs` | لا توجد علاقة بين تركيبات TypeScript وJSON Rust. | خيار مساعد؛ ضمان التكافؤ عبر وقت التشغيل (يتم الحفاظ على البرنامج النصي بواسطة Tooling WG). |

## خلاصات الحضور

- ملخص القطعة (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
-`manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
-`sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
-`sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## مجلة تسجيل الخروج| التاريخ | مهندس | نتيجة قائمة المراجعة | ملاحظات |
|------|----------|----------------------|-------|
| 2026-02-12 | الأدوات (ماجستير في القانون) | ✅ رويسي | تم تجديد التركيبات عبر `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`، مما أدى إلى إنتاج القائمة القياسية + الأسماء المستعارة وملخص واضح `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. تم التحقق باستخدام `cargo test -p sorafs_chunker` و`ci/check_sorafs_fixtures.sh` الخاص (التركيبات المرحلية للتحقق). Étape 5 en attente jusqu'à l'arrivée du helper de parité Node. |
| 2026-02-20 | أدوات التخزين CI | ✅ رويسي | مظروف البرلمان (`fixtures/sorafs_chunker/manifest_signatures.json`) تم استرداده عبر `ci/check_sorafs_fixtures.sh` ; قام البرنامج النصي بإعادة ضبط التركيبات، وتأكيد ملخص البيان `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`، وربط أداة Rust (يتم تنفيذ أشرطة Go/Node عندما تكون متاحة) بدون فرق. |

يجب على Tooling WG إضافة خط تاريخ بعد تنفيذ قائمة التحقق. سي اوني
étape échoue، افتح مشكلة Lié ici وأدرج تفاصيل العلاج
قبل الموافقة على التركيبات أو الملفات الشخصية الجديدة.