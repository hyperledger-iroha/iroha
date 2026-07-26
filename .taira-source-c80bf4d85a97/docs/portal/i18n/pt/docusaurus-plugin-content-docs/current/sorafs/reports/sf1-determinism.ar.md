---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Nome do arquivo SF1 em SoraFS
resumo: قائمة تحقق و digests متوقعة للتحقق من ملف chunker القياسي `sorafs.sf1@1.0.0`.
---

# Você pode usar o SF1 em SoraFS

يلتقط هذا التقرير التشغيل التجريبي الأساسي لملف chunker القياسي
`sorafs.sf1@1.0.0`. O Tooling WG não está disponível para você
التحقق من تحديث fixtures أو خطوط استهلاك جديدة. سجل نتيجة كل أمر في الجدول
Não se preocupe, você precisará de uma chave de fenda.

## قائمة التحقق

| الخطوة | الأمر | النتيجة المتوقعة | Produtos |
|------|--------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | تنجح جميع الاختبارات؛ Você pode usar a paridade `vectors`. | يؤكد أن fixtures القياسية تُبنى وتطابق تنفيذ Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | يخرج السكربت بـ 0؛ Ele digeriu o manifesto. | يتحقق من أن fixtures تُعاد توليدها بشكل نظيف وأن التواقيع تبقى مرفقة. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | O registro do arquivo `sorafs.sf1@1.0.0` está associado ao registro (`profile_id=1`). | Você precisa de metadados para registro de dados. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | Você pode usar o `--allow-unsigned`; O manifesto é o mais importante. | Você pode usar pedaços e manifestos. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Você também pode usar o diff de fixtures TypeScript e Rust JSON. | ajudante Obtenha paridade para tempos de execução (consulte o Tooling WG). |

## resume المتوقعة

- Resumo de pedaços (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
-`manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
-`sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
-`sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
-`sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
-`sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## سجل الاعتماد

| التاريخ | Engenheiro | نتيجة قائمة التحقق | Produtos |
|------|----------|----------|-------|
| 12/02/2026 | Ferramentas (LLM) | ❌ Falhou | Passo 1: `cargo test -p sorafs_chunker` no modelo `vectors` para luminárias com alça de mão `sorafs.sf1@1.0.0` e perfil aliases/resumos (`fixtures/sorafs_chunker/sf1_profile_v1.*`). Passo 2: `ci/check_sorafs_fixtures.sh` — O `manifest_signatures.json` está no repositório (na árvore de trabalho). Passo 4: O código `export_vectors` está disponível no manifesto do arquivo. التوصية: استعادة fixtures الموقعة (أو توفير مفتاح Council) وإعادة توليد binds حتى يتم تضمين handle القياسي + مصفوفة aliases كما تتطلب الاختبارات. |
| 12/02/2026 | Ferramentas (LLM) | ✅ Aprovado | Você pode usar fixtures em `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, manipular قياسي + listas de alias e resumo de manifesto em `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Use o `cargo test -p sorafs_chunker` e o `ci/check_sorafs_fixtures.sh` (acessórios para não usar). O 5 método é usado e a paridade auxiliar é usada no Node. |
| 20/02/2026 | Ferramentas de armazenamento CI | ✅ Aprovado | Envelope do Parlamento (`fixtures/sorafs_chunker/manifest_signatures.json`) تم جلبه عبر `ci/check_sorafs_fixtures.sh`; أعاد السكربت توليد fixtures, وأكد manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, وأعاد تشغيل aproveitar Rust (خطوات Go/Node تنفذ عند توفرها) دون فروقات. |

O Tooling WG não está disponível para você. إذا فشلت أي خطوة,
افتح issue مرتبطا هنا وضمن تفاصيل remediação قبل الموافقة على fixtures أو
Não.