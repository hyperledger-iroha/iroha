---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf1-determinism.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: SoraFS SF1 الحتمية التشغيل الجاف
ملخص: مجموعة مختارة وملخصات للتحقق من ملف تعريف القطاعة الكنسي `sorafs.sf1@1.0.0`.
---

# SoraFS SF1 الحتمية التشغيل الجاف

هذا هو التثبيت الأساسي للتشغيل الجاف للملف التعريفي القانوني للقطعة
`sorafs.sf1@1.0.0`. تحتاج مجموعة الأدوات إلى التحقق من الاختيار جيدًا عند التحقق
تركيبات بديلة أو خطوط أنابيب استهلاكية جديدة. قم بتسجيل النتيجة كل يوم
الأوامر الموجودة على الطاولة لحفظ المسار القابل للتدقيق.

## قائمة الاختيار| شاغ | القيادة | النتيجة النهائية | مساعدة |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | يتم تقديم جميع الاختبارات; تم اختبار التكافؤ `vectors` بنجاح. | تأكد من أن التركيبات الأساسية يتم تجميعها وإضافتها إلى تحقيق الصدأ. |
| 2 | `ci/check_sorafs_fixtures.sh` | انتهى النص 0; ملخص الخلاصة يظهر بشكل جيد. | تأكد من أن التركيبات يتم تجديدها تمامًا وأنها تحتوي على أعمدة. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | يتم التسجيل لـ `sorafs.sf1@1.0.0` باستخدام واصف التسجيل (`profile_id=1`). | يضمن أن يكون سجل بيانات التعريف متزامنًا. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | يتم التجديد بدون `--allow-unsigned`; لا يتم حفظ بيان الملفات والتوقيع. | هذا يدل على تحديد حدود القطع والبيانات. |
| 5 | `node scripts/check_sf1_vectors.mjs` | إليك الفرق بين تركيبات TypeScript وRust JSON. | مساعد اختياري تحقيق التوازن بين وقت التشغيل (يدعم البرنامج النصي Tooling WG). |

## خلاصات جيدة

- ملخص القطعة (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
-`manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
-`sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
-`sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## سجل الخروج| البيانات | مهندس | نتيجة الاختيار | مساعدة |
|------|----------|-------------------|------|
| 2026-02-12 | الأدوات (ماجستير في القانون) | ✅ نجح | تم تجديد التركيبات من خلال `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`، بمقبض قانوني + أسماء مستعارة مسجلة وملخص بياني جديد `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. تم التحقق من `cargo test -p sorafs_chunker` والبروغونوم الشفاف `ci/check_sorafs_fixtures.sh` (تركيبات قابلة للفحص). الجزء 5 يتعلق بمساعد تكافؤ العقدة. |
| 2026-02-20 | أدوات التخزين CI | ✅ نجح | مظروف البرلمان (`fixtures/sorafs_chunker/manifest_signatures.json`) تم الوصول إليه من خلال `ci/check_sorafs_fixtures.sh`; تم تجديد تركيبات النص البرمجي، وتم التحقق من ملخص البيان `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`، وحزام الصدأ الخلفي (يتم استخدام Go/Node عند الانتهاء) بدون فروق. |

يجب على Tooling WG إضافة البيانات بعد اختيار القائمة. اذا
مهما كان الأمر، قم بطرح المشكلة مرة أخرى وأضف التفاصيل
العلاج من خلال تركيبات جديدة أو ملفات تعريف.