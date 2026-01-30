---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8459fea08685e5608927f9bc85e69b83c3ebeba6deba2cc715ed10d5785bd4ca
source_last_modified: "2025-11-14T04:43:21.768177+00:00"
translation_last_reviewed: 2026-01-30
---

# تجزئة SoraFS → مسار المانيفست

يمثل هذا المرفق لدليل البدء السريع مساراً من البداية للنهاية يحوّل البايتات الخام إلى
مانيفستات Norito مناسبة لـ Pin Registry في SoraFS. المحتوى مقتبس من
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
راجِع ذلك المستند للمواصفة المعتمدة وسجل التغييرات.

## 1. تجزئة حتمية

يستخدم SoraFS ملف تعريف SF-1 (`sorafs.sf1@1.0.0`): hash متدحرج مستوحى من FastCDC مع حد أدنى
لحجم chunk يبلغ 64 KiB، وهدف 256 KiB، وحد أقصى 512 KiB، وقناع كسر `0x0000ffff`. الملف
مسجل في `sorafs_manifest::chunker_registry`.

### مساعدات Rust

- `sorafs_car::CarBuildPlan::single_file` – يُنتج إزاحات chunks وأطوالها وملخصات BLAKE3 أثناء
  تجهيز بيانات CAR الوصفية.
- `sorafs_car::ChunkStore` – يمرر payloads بشكل streaming، ويحفظ بيانات chunks الوصفية، ويشتق
  شجرة أخذ عينات Proof-of-Retrievability (PoR) بحجم 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – مساعد مكتبة يقف خلف كلا الـ CLI.

### أدوات CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

يحتوي JSON على الإزاحات المرتبة والأطوال وملخصات chunks. احتفظ بالخطة عند بناء المانيفستات
أو مواصفات fetch الخاصة بالأوركسترايتور.

### شواهد PoR

تُتيح `ChunkStore` الخيارين `--por-proof=<chunk>:<segment>:<leaf>` و`--por-sample=<count>` حتى
يتمكن المدققون من طلب مجموعات شواهد حتمية. قرن هذه الأعلام مع `--por-proof-out` أو
`--por-sample-out` لتسجيل JSON.

## 2. تغليف مانيفست

تجمع `ManifestBuilder` بيانات chunks الوصفية مع مرفقات الحوكمة:

- CID الجذر (dag-cbor) وتعهدات CAR.
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

- `payload.manifest` – بايتات مانيفست مشفرة بـ Norito.
- `payload.report.json` – ملخص قابل للقراءة للبشر/الأتمتة يتضمن `chunk_fetch_specs` و
  `payload_digest_hex` وملخصات CAR وبيانات alias الوصفية.
- `payload.manifest_signatures.json` – ظرف يحتوي على ملخص BLAKE3 للمانيفست، وملخص SHA3 لخطة
  chunks، وتوقيعات Ed25519 مرتبة.

استخدم `--manifest-signatures-in` للتحقق من الأظرف القادمة من موقّعين خارجيين قبل إعادة
كتابتها، واستخدم `--chunker-profile-id` أو `--chunker-profile=<handle>` لتثبيت اختيار السجل.

## 3. النشر والتثبيت (pin)

1. **تقديم الحوكمة** – قدّم ملخص المانيفست وظرف التوقيعات إلى المجلس حتى يمكن قبول الـ pin.
   يجب على المدققين الخارجيين حفظ ملخص SHA3 لخطة chunks بجانب ملخص المانيفست.
2. **تثبيت payloads** – ارفع أرشيف CAR (وفهرس CAR الاختياري) المشار إليه في المانيفست إلى
   Pin Registry. تأكد من أن المانيفست وCAR يشتركان في CID جذر واحد.
3. **تسجيل التليمترية** – احتفظ بتقرير JSON وشواهد PoR وأي مقاييس fetch ضمن artefacts الإصدار.
   تغذي هذه السجلات لوحات معلومات المشغلين وتساعد على إعادة إنتاج المشكلات دون تنزيل
   payloads كبيرة.

## 4. محاكاة fetch متعددة المزوّدين

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` يزيد التوازي لكل مزوّد (`#4` أعلاه).
- `@<weight>` يضبط انحياز الجدولة؛ القيمة الافتراضية هي 1.
- `--max-peers=<n>` يحد عدد المزوّدين المجدولين للتشغيل عندما يعيد الاكتشاف مرشحين أكثر من المطلوب.
- `--expect-payload-digest` و`--expect-payload-len` يحميان من الفساد الصامت.
- `--provider-advert=name=advert.to` يتحقق من قدرات المزوّد قبل استخدامه في المحاكاة.
- `--retry-budget=<n>` يستبدل عدد المحاولات لكل chunk (الافتراضي: 3) حتى يتمكن CI من كشف
  التراجعات أسرع عند اختبار سيناريوهات الفشل.

يعرض `fetch_report.json` مقاييس مجمّعة (`chunk_retry_total` و`provider_failure_rate` وغيرها)
مناسبة لassertions الخاصة بـ CI وقابلية الملاحظة.

## 5. تحديثات السجل والحوكمة

عند اقتراح ملفات تعريف chunker جديدة:

1. أنشئ الوصف في `sorafs_manifest::chunker_registry_data`.
2. حدّث `docs/source/sorafs/chunker_registry.md` والمواثيق ذات الصلة.
3. أعد توليد fixtures (`export_vectors`) والتقط المانيفستات الموقّعة.
4. قدّم تقرير الامتثال للميثاق مع توقيعات الحوكمة.

ينبغي أن تفضّل الأتمتة handles القياسية (`namespace.name@semver`) وألا تعود إلى IDs رقمية
إلا عند الحاجة إلى التوافق الرجعي.
