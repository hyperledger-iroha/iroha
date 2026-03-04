---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf1-determinism.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: SoraFS SF1 الحتمية التشغيل الجاف
ملخص: قائمة المراجعة والخلاصات اللازمة للتحقق من صحة ملف تعريف Canon `sorafs.sf1@1.0.0`.
---

# SoraFS SF1 الحتمية التشغيل الجاف

هذا هو الرابط الذي تم التقاطه أو قاعدة التشغيل الجاف لملف تعريف Canonico
`sorafs.sf1@1.0.0`. يجب على Tooling WG إعادة تنفيذ قائمة التحقق بعد التحقق من صحتها
تحديث التركيبات أو خطوط الأنابيب الاستهلاكية الجديدة. قم بتسجيل نتيجة كل يوم
قم بالقيادة على الطاولة لتتمكن من مراجعة المسار.

## قائمة المراجعة| باسو | كوماندوز | النتيجة منتظرة | نوتاس |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | جميع اختبارات نظام التشغيل passam؛ تم اختبار النجاح `vectors`. | تأكد من تجميع تركيبات Canon وتوافقها مع تطبيق Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | يا سكريبت ساي كوم 0; تقرير عن ملخصات البيان. | تحقق من أن التركيبات قد تم تجديدها وأن الإضافات تدوم طويلاً. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | يتوافق المدخل `sorafs.sf1@1.0.0` مع واصف التسجيل (`profile_id=1`). | تأكد من مزامنة بيانات التعريف بشكل دائم. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | تم تجديده Sem `--allow-unsigned`; محفوظات البيان والقتل غير المتعمد. | إثبات الحتمية لحدود القطع والبيانات. |
| 5 | `node scripts/check_sf1_vectors.mjs` | يتم الإبلاغ عن اختلافات بين التركيبات TypeScript وRust JSON. | مساعد اختياري. ضمان المساواة بين أوقات التشغيل (البرنامج النصي الذي يتم تنفيذه بواسطة Tooling WG). |

## يلخص الإسبرادوس

- ملخص القطعة (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
-`manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
-`sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
-`sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## سجل الخروج| البيانات | مهندس | النتيجة عمل قائمة مرجعية | نوتاس |
|------|----------|-----------------------|-------|
| 2026-02-12 | الأدوات (ماجستير في القانون) | موافق | تم تجديد التركيبات عبر `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f`، ويتم التعامل مع قوائم Canon + الأسماء المستعارة وملخص البيان الجديد `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. تم التحقق من `cargo test -p sorafs_chunker` و`ci/check_sorafs_fixtures.sh` limpo (التركيبات المعدة للتحقق). الخطوة 5 معلقة أو مساعد الجدار Node chegar. |
| 2026-02-20 | أدوات التخزين CI | موافق | مظروف البرلمان (`fixtures/sorafs_chunker/manifest_signatures.json`) obtido عبر `ci/check_sorafs_fixtures.sh`؛ o تجديد تركيبات البرنامج النصي، وتأكيد ملخص البيان `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`، وإعادة تنفيذ o أداة الصدأ (تمرير Go/Node الذي يتم تنفيذه عند توفرها) دون اختلاف. |

يجب على مجموعة عمل الأدوات إضافة بيانات واحدة عند تنفيذ قائمة التحقق. هذا هو الطحالب
الخطوة التالية هي التعامل مع المشكلة المرتبطة بها وتضمين تفاصيل العلاج المسبق
قم بالموافقة على التركيبات الجديدة أو المثالية.