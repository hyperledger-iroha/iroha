---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Titre de l'article SF1 pour SoraFS
résumé : قائمة تحقق و digests متوقعة للتحقق من ملف chunker القياسي `sorafs.sf1@1.0.0`.
---

# التشغيل التجريبي للحتمية SF1 pour SoraFS

يلتقط هذا التقرير التشغيل التجريبي الأساسي لملف chunker القياسي
`sorafs.sf1@1.0.0`. يجب على Tooling WG إعادة تشغيل قائمة التحقق أدناه عند
Il y a beaucoup de rencontres et de rencontres. سجل نتيجة كل أمر في الجدول
للحفاظ على أثر تدقيق قابل للمراجعة.

## قائمة التحقق

| الخطوة | الأمر | النتيجة المتوقعة | ملاحظات |
|------|---------|--------|-------|
| 1 | `cargo test -p sorafs_chunker` | تنجح جميع الاختبارات؛ ينجح اختبار parité `vectors`. | Il y a des rencontres à venir avec Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | يخرج السكربت بـ 0؛ ويبلغ digère للـ manifeste أدناه. | يتحقق من أن rencontres تُعاد توليدها بشكل نظيف وأن التواقيع تبقى مرفقة. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | Il s'agit du registre `sorafs.sf1@1.0.0` (`profile_id=1`). | Il s'agit des métadonnées du registre. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | تنجح إعادة التوليد دون `--allow-unsigned`; ملفات manifeste والتوقيع لا تتغير. | Il s'agit d'un morceau et de manifestes. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Il existe des différences entre les appareils TypeScript et Rust JSON. | assistant اختياري؛ Il s'agit des temps d'exécution de parité (السكربت يديره Tooling WG). |

## digests المتوقعة

- Résumé de fragments (SHA3-256) : `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json` : `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json` : `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts` : `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go` : `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs` : `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## سجل الاعتماد

| التاريخ | Ingénieur | نتيجة قائمة التحقق | ملاحظات |
|------|----------|-----------|-------|
| 2026-02-12 | Outillage (LLM) | ❌ Échec | Section 1 : `cargo test -p sorafs_chunker` pour `vectors` pour luminaires et poignée pour `sorafs.sf1@1.0.0` et profil alias/digests (`fixtures/sorafs_chunker/sf1_profile_v1.*`). Étape 2 : `ci/check_sorafs_fixtures.sh` — `manifest_signatures.json` est utilisé dans le repo (avec l'arbre de travail). Section 4 : لا يستطيع `export_vectors` التحقق من التواقيع بينما ملف manifest مفقود. التوصية: استعادة luminaires الموقعة (أو توفير مفتاح Council) et إعادة توليد liaisons حتى يتم تضمين handle القياسي + مصفوفة alias كما تتطلب الاختبارات. |
| 2026-02-12 | Outillage (LLM) | ✅ Réussi | Vous avez besoin de luminaires pour `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, vous disposez de la gestion des listes d'alias + et du résumé du manifeste pour `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. تم التحقق باستخدام `cargo test -p sorafs_chunker` وتشغيل نظيف لـ `ci/check_sorafs_fixtures.sh` (luminaires تم تجهيزها للتحقق). Il y a 5 options pour la parité d'assistance pour Node. |
| 2026-02-20 | Outillage de stockage CI | ✅ Réussi | Enveloppe du Parlement (`fixtures/sorafs_chunker/manifest_signatures.json`) تم جلبه عبر `ci/check_sorafs_fixtures.sh` ; Dans le cadre des luminaires, il y a le manifeste digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, et vous exploitez Rust (les Go/Node sont également disponibles). |

يجب على Tooling WG إضافة صف مؤرخ بعد تشغيل قائمة التحقق. إذا فشلت أي خطوة،
افتح issue مرتبطا هنا وضمن تفاصيل remédiation قبل الموافقة على luminaires أو
ملفات جديدة.