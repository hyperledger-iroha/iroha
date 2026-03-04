---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: التشغيل التجريبي للحتمية SF1 في SoraFS
resumen: قائمة تحقق و digests متوقعة للتحقق من ملف fragmentador القياسي `sorafs.sf1@1.0.0`.
---

# التشغيل التجريبي للحتمية SF1 في SoraFS

يلتقط هذا التقرير التشغيل التجريبي الأساسي لملف chunker القياسي
`sorafs.sf1@1.0.0`. يجب على Tooling WG إعادة تشغيل قائمة التحقق أدناه عند
التحقق من تحديث accesorios أو خطوط استهلاك جديدة. سجل نتيجة كل أمر في الجدول
للحفاظ على أثر تدقيق قابل للمراجعة.

## قائمة التحقق

| الخطوة | الأمر | النتيجة المتوقعة | ملاحظات |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | تنجح جميع الاختبارات؛ Esta es la paridad `vectors`. | يؤكد أن accesorios القياسية تُبنى وتطابق تنفيذ Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | يخرج السكربت بـ 0؛ ويبلغ digiere للـ manifiesto أدناه. | يتحقق من أن accesorios تُعاد توليدها بشكل نظيف وأن التواقيع تبقى مرفقة. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | Haga clic en el registro `sorafs.sf1@1.0.0` y (`profile_id=1`). | Hay metadatos en el registro de datos. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | تنجح إعادة التوليد دون `--allow-unsigned`; ملفات manifiesto والتوقيع لا تتغير. | يقدم دليلا على الحتمية لحدود fragmentos y manifiestos. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Hay diferencias entre dispositivos TypeScript y Rust JSON. | ayudante اختياري؛ تأكد من paridad de tiempos de ejecución (السكربت يديره Tooling WG). |

## resúmenes المتوقعة- Resumen de fragmentos (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## سجل الاعتماد| التاريخ | Ingeniero | نتيجة قائمة التحقق | ملاحظات |
|------|----------|---------------------|-------|
| 2026-02-12 | Herramientas (LLM) | ❌ Fallido | Paso 1: يفشل `cargo test -p sorafs_chunker` في مجموعة `vectors` لأن accesorios ما زالت تنشر handle القديم `sorafs.sf1@1.0.0` y alias/resúmenes de perfil (`fixtures/sorafs_chunker/sf1_profile_v1.*`). Paso 2: Paso `ci/check_sorafs_fixtures.sh` — Paso `manifest_signatures.json` en el repositorio (movimiento del árbol de trabajo). Capítulo 4: لا يستطيع `export_vectors` التحقق من التواقيع بينما ملف manifest مفقود. التوصية: استعادة accesorios الموقعة (أو توفير مفتاح consejo) وإعادة توليد enlaces حتى يتم تضمين mango القياسي + مصفوفة alias كما تطلب الاختبارات. |
| 2026-02-12 | Herramientas (LLM) | ✅ Aprobado | تمت إعادة توليد accesorios عبر `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, منتجة handle قياسي + listas de alias y resumen de manifiesto جديد `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. تم التحقق باستخدام `cargo test -p sorafs_chunker` y تشغيل نظيف لـ `ci/check_sorafs_fixtures.sh` (accesorios تم تجهيزها للتحقق). Haga 5 pasos para establecer la paridad del ayudante en Node. |
| 2026-02-20 | CI de herramientas de almacenamiento | ✅ Aprobado | Sobre del Parlamento (`fixtures/sorafs_chunker/manifest_signatures.json`) تم جلبه عبر `ci/check_sorafs_fixtures.sh`; Necesitamos accesorios, un resumen de manifiesto `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` y un arnés Rust (combinación de Go/Node) entre otros. |

يجب على Tooling WG إضافة صف مؤرخ بعد تشغيل قائمة التحقق. إذا فشلت أي خطوة،
افتح problema مرتبطا هنا وضمن تفاصيل remediación قبل الموافقة على accesorios أو
ملفات جديدة.