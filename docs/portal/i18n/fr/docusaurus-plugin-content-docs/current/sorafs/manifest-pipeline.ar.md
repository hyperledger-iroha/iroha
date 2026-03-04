---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تجزئة SoraFS → مسار المانيفست

يمثل هذا المرفق لدليل البدء السريع مساراً من البداية للنهاية يحوّل البايتات الخام إلى
Utilisez Norito pour le registre des broches dans SoraFS. المحتوى مقتبس من
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
راجِع ذلك المستند للمواصفة المعتمدة وسجل التغييرات.

## 1. تجزئة حتمية

Utiliser SoraFS pour SF-1 (`sorafs.sf1@1.0.0`) : hash est compatible avec FastCDC ou FastCDC.
Un morceau fait 64 KiB, 256 KiB et 512 KiB, pour `0x0000ffff`. الملف
Il s'agit de `sorafs_manifest::chunker_registry`.

### مساعدات Rust

- `sorafs_car::CarBuildPlan::single_file` – contient des morceaux et BLAKE3
  تجهيز بيانات CAR الوصفية.
- `sorafs_car::ChunkStore` – Charges utiles pour le streaming, ainsi que pour les morceaux de fichiers
  Il s'agit d'une preuve de récupérabilité (PoR) de 64 Ko / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – مساعد مكتبة يقف خلف كلا الـ CLI.

### أدوات CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON contient des morceaux de fichiers et des morceaux. احتفظ بالخطة عند بناء المانيفستات
أو مواصفات fetch الخاصة بالأوركسترايتور.

### شواهد PoR

تُتيح `ChunkStore` الخيارين `--por-proof=<chunk>:<segment>:<leaf>` و`--por-sample=<count>` حتى
يتمكن المدققون من طلب مجموعات شواهد حتمية. قرن هذه الأعلام مع `--por-proof-out` أو
`--por-sample-out` pour JSON.

## 2. تغليف مانيفست

Utilisez `ManifestBuilder` pour les chunks en utilisant les éléments suivants :- CID الجذر (dag-cbor) et CAR.
- إثباتات alias ومطالبات قدرات المزوّدين.
- توقيعات المجلس وبيانات وصفية اختيارية (مثل معرفات build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

مخرجات مهمة:

- `payload.manifest` – Mise en relation avec Norito.
- `payload.report.json` – ملخص قابل للقراءة للبشر/الأتمتة يتضمن `chunk_fetch_specs` et
  `payload_digest_hex` وملخصات CAR وبيانات alias الوصفية.
- `payload.manifest_signatures.json` – Utiliser BLAKE3 pour utiliser SHA3
  chunks, وتوقيعات Ed25519 مرتبة.

استخدم `--manifest-signatures-in` للتحقق من الأظرف القادمة من موقّعين خارجيين قبل إعادة
كتابتها، واستخدم `--chunker-profile-id` et `--chunker-profile=<handle>` اختيار السجل.

## 3. النشر والتثبيت (épingle)

1. **تقديم الحوكمة** – قدّم ملخص المانيفست وظرف التوقيعات إلى المجلس حتى يمكن قبول الـ pin.
   Vous pouvez utiliser des fragments SHA3 pour plus de détails.
2. **Charges utiles de chargement** – ارفع أرشيف CAR (وفهرس CAR الاختياري) المشار إليه في المانيفست إلى
   Registre des broches. Il s'agit d'un cas où CAR est en contact avec le CID.
3. **Récupération de données** – Utiliser JSON et PoR et récupérer vos artefacts.
   تغذي هذه السجلات لوحات معلومات المشغلين وتساعد على إعادة إنتاج المشكلات دون تنزيل
   charges utiles ici.

## 4. محاكاة fetch متعددة المزوّدين`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` est utilisé pour le téléchargement (`#4` est disponible).
- `@<weight>` يضبط انحياز الجدولة؛ القيمة الافتراضية هي 1.
- `--max-peers=<n>` يحد عدد المزوّدين المجدولين للتشغيل عندما يعيد الاكتشاف مرشحين أكثر من المطلوب.
- `--expect-payload-digest` et `--expect-payload-len` sont utilisés pour les tâches.
- `--provider-advert=name=advert.to` يتحقق من قدرات المزوّد قبل استخدامه في المحاكاة.
- `--retry-budget=<n>` يستبدل عدد المحاولات لكل chunk (الافتراضي: 3) حتى يتمكن CI من كشف
  التراجعات أسرع عند اختبار سيناريوهات الفشل.

يعرض `fetch_report.json` مقاييس مجمّعة (`chunk_retry_total` و`provider_failure_rate` وغيرها)
مناسبة لassertions الخاصة بـ CI وقابلية الملاحظة.

## 5. تحديثات السجل والحوكمة

عند اقتراح ملفات تعريف chunker جديدة:

1. أنشئ الوصف في `sorafs_manifest::chunker_registry_data`.
2. حدّث `docs/source/sorafs/chunker_registry.md` والمواثيق ذات الصلة.
3. Fixez les luminaires (`export_vectors`) et les luminaires.
4. قدّم تقرير الامتثال للميثاق مع توقيعات الحوكمة.

ينبغي أن تفضّل الأتمتة handles القياسية (`namespace.name@semver`) et ألا تعود إلى IDs رقمية
إلا عند الحاجة إلى التوافق الرجعي.