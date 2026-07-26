---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/reports/sf1-determinism.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: التشغيل التجريبي اللتمية SF1 في SoraFS
ملخص: قائمة الاختيار والهضم المتوقعة من ملفchunker القياسي `sorafs.sf1@1.0.0`.
---

# التشغيل التجريبي للصورة SF1 في SoraFS

يلتقط هذا التقرير التشغيلي التجريبي الأساسي لملف Chunker القياسي
`sorafs.sf1@1.0.0`. يجب على Tooling WG إعادة قائمة التحقق أدناه عند
التحقق من تحديث التركيبات أو خطوط الاستهلاك الجديدة. نتيجة لأمر في الجدول
هجوم على جهد دقيق قابل للمراجعة.

## قائمة التحقق

| الخطوة | الأمر | النتيجة | تعليقات |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | إلى جميع التحديات؛ نجح في اختبار التكافؤ `vectors`. | وهي عبارة عن تركيبات قياسية تُبنى وتتطابق مع تنفيذ الصدأ. |
| 2 | `ci/check_sorafs_fixtures.sh` | يخرج السكربت بـ 0؛ حتى يهضم للـ المانيفست أدناه. | ومن أن التركيبات تُعاد توليدها بشكل نظيف وأن التواقيع تبقى مرفقة. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | الإدخال الخاص بـ `sorafs.sf1@1.0.0` يطابق سجل واصف (`profile_id=1`). | ضمان بيانات تعريف البقاء الخاصة بالسجل متزامنة. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | لإعادة إعادة التوليد دون `--allow-unsigned`; ملفات البيان والتوقيع لا. | يقدم دليلا على الحامية لحدود القطعة و البيانات. |
| 5 | `node scripts/check_sf1_vectors.mjs` | لا يوجد فرق بين تركيبات TypeScript وRust JSON. | مساعد اختياري؛ تأكد من التكافؤ عبر أوقات التشغيل (السكربت يديره Tooling WG). |

## الملخصات- ملخص القطعة (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
-`manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
-`sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
-`sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## سجل الاعتماد| التاريخ | مهندس | نتيجة "قائمة التحقق" | تعليقات |
|------|--------------------------|------|-----|
| 2026-02-12 | الأدوات (ماجستير في القانون) | ❌فشل | الخطوة 1: يفشل `cargo test -p sorafs_chunker` في مجموعة `vectors` لأن التركيبات ما تبحث عن مقبض `sorafs.sf1@1.0.0` القديم وتفتقد الأسماء المستعارة/الملخصات للملف الشخصي (`fixtures/sorafs_chunker/sf1_profile_v1.*`). الخطوة 2: يتوقف `ci/check_sorafs_fixtures.sh` — الملف `manifest_signatures.json` مفقود في حالة الريبو (محذوف في شجرة العمل). الخطوة 4: لا يستطيع `export_vectors` التحقق من التواقيع أثناء عدم وجود بيان مفقود. استعادة: استعادة موقع التركيبات (أو توفير مجلس المفتاح) جاهز لإضافة الارتباطات حتى يتم تضمين المقبض القياسي + مصفوفة الأسماء المستعارة كما تتطلب البحث. |
| 2026-02-12 | الأدوات (ماجستير في القانون) | ✅ نجح | بعد إعادة توليد التركيبات عبر `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`، منتجة مقبض قياسي + قوائم مستعارة وملخص واضح جديد `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. تم التحقق باستخدام `cargo test -p sorafs_chunker` وتشغيل نظيف لـ `ci/check_sorafs_fixtures.sh` (التركيبات التي تم تجهيزها لها). الخطوة 5 معلقة حتى الوصول help parity الخاص بـ Node. |
| 2026-02-20 | أدوات التخزين CI | ✅ نجح | ظرف البرلمان (`fixtures/sorafs_chunker/manifest_signatures.json`) تم جلبه عبر `ci/check_sorafs_fixtures.sh`; ثم بدأت السكربت توليد تركيبات، فهي تحتوي على خلاصات مانيفست `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`، وأعاد تشغيل تسخير الصدأ (خطوات Go/Node فقط عند وجودها) دون فروقات. |

يجب على Tooling WG إضافة متخصص بعد تشغيل قائمة التحقق. إذا تأكدت من أي خطوة،
تسجيل العدد مرتبط هنا وضمن تفاصيل المعالجة قبل الموافقة على المباريات أو
ملفات جديدة.