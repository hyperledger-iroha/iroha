---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf1-determinism.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: SoraFS SF1 الحتمية التشغيل الجاف
ملخص: قائمة التحقق وملخصات الاختبارات للتحقق من ملف تعريف مقسم Canon `sorafs.sf1@1.0.0`.
---

# SoraFS SF1 الحتمية التشغيل الجاف

يلتقط هذا التقرير قاعدة التشغيل الجاف لملف تعريف Canonico
`sorafs.sf1@1.0.0`. يجب على Tooling WG إعادة تشغيل القائمة المرجعية عند الانتهاء
صالح تحديثات التركيبات أو خطوط الأنابيب الجديدة للمستهلكين. تسجيل إل
نتيجة كل أمر على الطاولة للحفاظ على مسار قابل للتدقيق.

## قائمة المراجعة| باسو | كوماندوز | النتيجة منتظرة | نوتاس |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | جميع الاختبارات باسان؛ تم إخراج اختبار paridad `vectors`. | تأكد من أن التركيبات الكنسي مجمعة ومتزامنة مع تطبيق Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | بيع النص يخدع 0; Reporta Los Digest de Manifest de Abajo. | تحقق من أن التركيبات تتجدد بشكل جيد وأن الشركات يمكنها تحمل الملحقات. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | يتزامن إدخال `sorafs.sf1@1.0.0` مع واصف التسجيل (`profile_id=1`). | تأكد من أن بيانات تعريف السجل ستتم مزامنتها. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | التجديد له خطيئة `--allow-unsigned`; ملفات البيان والشركة ليست متغيرة. | إثبات اختبار الحتمية لحدود القطعة والبيانات. |
| 5 | `node scripts/check_sf1_vectors.mjs` | أبلغ عن عدم وجود اختلاف بين تركيبات TypeScript وRust JSON. | مساعد اختياري. ضمان المساواة بين أوقات التشغيل (البرنامج النصي الذي يتم الاحتفاظ به بواسطة Tooling WG). |

## يلخص الإسبرادوس

- ملخص القطعة (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
-`manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
-`sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
-`sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## سجل الخروج| فيشا | مهندس | نتيجة قائمة التحقق | نوتاس |
|------|----------|------------------------|-------|
| 2026-02-12 | الأدوات (ماجستير في القانون) | موافق | يتم تجديد التركيبات عبر `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f`، ويتم إنتاج التعامل مع قوائم Canonico + الأسماء المستعارة ولوحة جدارية ملخصة واضحة `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. تم التحقق باستخدام `cargo test -p sorafs_chunker` وخط `ci/check_sorafs_fixtures.sh` (تركيبات معدة للتحقق). الخطوة 5 تشير إلى أن المساعد لشبكة Node. |
| 2026-02-20 | أدوات التخزين CI | موافق | مظروف البرلمان (`fixtures/sorafs_chunker/manifest_signatures.json`) تم الحصول عليه عبر `ci/check_sorafs_fixtures.sh`؛ تم تجديد تركيبات البرنامج النصي، وتأكيد ملخص البيان `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`، وإعادة تشغيل أداة Rust (يتم تنفيذ خطوة Go/Node عند توفرها) دون فرق. |

يجب على Tooling WG إضافة ملف تم إنشاؤه بعد تنفيذ قائمة التحقق. سي
قد تفشل خطوة أخرى، قبل أن يتم حل المشكلة هنا وتتضمن تفاصيل العلاج
قبل عرض التركيبات الجديدة أو الملفات الشخصية.