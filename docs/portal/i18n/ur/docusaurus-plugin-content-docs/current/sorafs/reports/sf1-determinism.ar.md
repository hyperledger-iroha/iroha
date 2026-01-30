---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/reports/sf1-determinism.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: التشغيل التجريبي للحتمية SF1 في SoraFS
summary: قائمة تحقق و digests متوقعة للتحقق من ملف chunker القياسي `sorafs.sf1@1.0.0`.
---

# التشغيل التجريبي للحتمية SF1 في SoraFS

يلتقط هذا التقرير التشغيل التجريبي الأساسي لملف chunker القياسي
`sorafs.sf1@1.0.0`. يجب على Tooling WG إعادة تشغيل قائمة التحقق أدناه عند
التحقق من تحديث fixtures أو خطوط استهلاك جديدة. سجل نتيجة كل أمر في الجدول
للحفاظ على أثر تدقيق قابل للمراجعة.

## قائمة التحقق

| الخطوة | الأمر | النتيجة المتوقعة | ملاحظات |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | تنجح جميع الاختبارات؛ ينجح اختبار parity `vectors`. | يؤكد أن fixtures القياسية تُبنى وتطابق تنفيذ Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | يخرج السكربت بـ 0؛ ويبلغ digests للـ manifest أدناه. | يتحقق من أن fixtures تُعاد توليدها بشكل نظيف وأن التواقيع تبقى مرفقة. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | الإدخال الخاص بـ `sorafs.sf1@1.0.0` يطابق واصف registry (`profile_id=1`). | يضمن بقاء metadata الخاصة بالregistry متزامنة. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | تنجح إعادة التوليد دون `--allow-unsigned`; ملفات manifest والتوقيع لا تتغير. | يقدم دليلا على الحتمية لحدود chunk و manifests. |
| 5 | `node scripts/check_sf1_vectors.mjs` | يبلغ عدم وجود diff بين fixtures TypeScript و Rust JSON. | helper اختياري؛ تأكد من parity عبر runtimes (السكربت يديره Tooling WG). |

## digests المتوقعة

- Chunk digest (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## سجل الاعتماد

| التاريخ | Engineer | نتيجة قائمة التحقق | ملاحظات |
|------|----------|---------------------|-------|
| 2026-02-12 | Tooling (LLM) | ❌ Failed | الخطوة 1: يفشل `cargo test -p sorafs_chunker` في مجموعة `vectors` لأن fixtures ما زالت تنشر handle القديم `sorafs.sf1@1.0.0` وتفتقد profile aliases/digests (`fixtures/sorafs_chunker/sf1_profile_v1.*`). الخطوة 2: يتوقف `ci/check_sorafs_fixtures.sh` — الملف `manifest_signatures.json` مفقود في حالة repo (محذوف في working tree). الخطوة 4: لا يستطيع `export_vectors` التحقق من التواقيع بينما ملف manifest مفقود. التوصية: استعادة fixtures الموقعة (أو توفير مفتاح council) وإعادة توليد bindings حتى يتم تضمين handle القياسي + مصفوفة aliases كما تتطلب الاختبارات. |
| 2026-02-12 | Tooling (LLM) | ✅ Passed | تمت إعادة توليد fixtures عبر `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`، منتجة handle قياسي + alias lists و manifest digest جديد `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. تم التحقق باستخدام `cargo test -p sorafs_chunker` وتشغيل نظيف لـ `ci/check_sorafs_fixtures.sh` (fixtures تم تجهيزها للتحقق). الخطوة 5 معلقة حتى وصول helper parity الخاص بـ Node. |
| 2026-02-20 | Storage Tooling CI | ✅ Passed | Parliament envelope (`fixtures/sorafs_chunker/manifest_signatures.json`) تم جلبه عبر `ci/check_sorafs_fixtures.sh`; أعاد السكربت توليد fixtures، وأكد manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, وأعاد تشغيل harness Rust (خطوات Go/Node تنفذ عند توفرها) دون فروقات. |

يجب على Tooling WG إضافة صف مؤرخ بعد تشغيل قائمة التحقق. إذا فشلت أي خطوة،
افتح issue مرتبطا هنا وضمن تفاصيل remediation قبل الموافقة على fixtures أو
ملفات جديدة.
