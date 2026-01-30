---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c4b6ab7ac4b05bd17a17ff4331e4c0e1e9086dad3a325b71a0339684c7475dd
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# البدء السريع في SoraFS

يرشدك هذا الدليل العملي عبر ملف تعريف الـ chunker الحتمي SF-1،
وتوقيع المانيفست، ومسار الجلب متعدد المزوّدين الذي يدعم خط أنابيب تخزين SoraFS.
وازنه مع [التعمّق في خط أنابيب المانيفست](manifest-pipeline.md)
للحصول على ملاحظات التصميم ومرجع أعلام سطر الأوامر.

## المتطلبات الأساسية

- أداة Rust (`rustup update`) مع نسخ مساحة العمل محليًا.
- اختياري: [زوج مفاتيح Ed25519 متوافق مع OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  لتوقيع المانيفستات.
- اختياري: Node.js ≥ 18 إذا كنت تخطط لمعاينة بوابة Docusaurus.

اضبط `export RUST_LOG=info` أثناء التجربة لإظهار رسائل CLI المفيدة.

## 1. تحديث الـ fixtures الحتمية

أعد توليد متجهات التقسيم (chunking) القياسية لـ SF-1. كما يُصدر الأمر مظاريف
مانيفست موقعة عند تزويد `--signing-key`؛ استخدم `--allow-unsigned` أثناء التطوير
المحلي فقط.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

المخرجات:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (إذا تم التوقيع)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. قسّم payload وافحص الخطة

استخدم `sorafs_chunker` لتقسيم ملف أو أرشيف عشوائي:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

الحقول الأساسية:

- `profile` / `break_mask` – يؤكد معاملات `sorafs.sf1@1.0.0`.
- `chunks[]` – إزاحات وأطوال مرتبة وبصمات BLAKE3 للـ chunks.

لـ fixtures الأكبر، شغّل اختبار الانحدار المبني على proptest لضمان تزامن التقسيم
بالتدفق وبالدفعات:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. ابنِ ووقّع مانيفست

لفّ خطة الـ chunks والكنى وتواقيع الحوكمة في مانيفست باستخدام
`sorafs-manifest-stub`. يوضح الأمر أدناه payload لملف واحد؛ مرّر مسار دليل لحزم
شجرة (تسير CLI ترتيبًا معجميًا).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

راجع `/tmp/docs.report.json` من أجل:

- `chunking.chunk_digest_sha3_256` – بصمة SHA3 للإزاحات/الأطوال، تطابق fixtures الخاصة
  بالـ chunker.
- `manifest.manifest_blake3` – بصمة BLAKE3 الموقعة ضمن ظرف المانيفست.
- `chunk_fetch_specs[]` – تعليمات جلب مرتبة للأوركستراتورات.

عندما تكون جاهزًا لتقديم تواقيع حقيقية، أضف الوسيطين `--signing-key` و `--signer`.
يتحقق الأمر من كل توقيع Ed25519 قبل كتابة الظرف.

## 4. حاكِ الاسترجاع متعدد المزوّدين

استخدم CLI الجلب التطويري لإعادة تشغيل خطة الـ chunks مقابل مزوّد واحد أو أكثر.
هذا مثالي لاختبارات الدخان في CI ولنمذجة الأوركستراتور.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

التحققات:

- `payload_digest_hex` يجب أن يطابق تقرير المانيفست.
- `provider_reports[]` تعرض أعداد النجاح/الفشل لكل مزود.
- قيمة `chunk_retry_total` غير الصفرية تُبرز تعديلات back-pressure.
- مرّر `--max-peers=<n>` لتقييد عدد المزوّدين المجدولين للتشغيل وإبقاء محاكاة CI مركّزة
  على المرشحين الأساسيين.
- `--retry-budget=<n>` يتجاوز العدد الافتراضي لمحاولات إعادة المحاولة لكل chunk (3)
  لتسريع كشف تراجعات الأوركستراتور عند حقن الأعطال.

أضف `--expect-payload-digest=<hex>` و `--expect-payload-len=<bytes>` للفشل بسرعة
عندما ينحرف payload المعاد بناؤه عن المانيفست.

## 5. الخطوات التالية

- **تكامل الحوكمة** – مرّر بصمة المانيفست و`manifest_signatures.json` إلى سير عمل المجلس
  لكي يتمكن Pin Registry من إعلان التوافر.
- **التفاوض مع السجل** – راجع [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  قبل تسجيل ملفات تعريف جديدة. ينبغي للأتمتة تفضيل المعالجات القياسية
  (`namespace.name@semver`) على المعرفات الرقمية.
- **أتمتة CI** – أضف الأوامر أعلاه إلى خطوط إصدار النشر حتى تنشر المستندات والـ fixtures
  والآرتيفاكت مانيفستات حتمية جنبًا إلى جنب مع بيانات وصفية موقعة.
