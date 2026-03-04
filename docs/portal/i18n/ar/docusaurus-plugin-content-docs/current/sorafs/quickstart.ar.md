---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#السابق في SoraFS

يرشدك هذا الدليل العملي عبر ملف تعريف الـ Chunker الحتمي SF-1،
وتوقيع المانيفست، ومسار الجلب متعدد المتحكمين الذي يدعم خط الأنابيب للتخزين SoraFS.
وازنه مع [التعمّق في خط أنابيب المانيفيست](manifest-pipeline.md)
للحصول على أفكار التصميم ومرجع أعلام سطر.

##المتطلبات الأساسية

- أداة الصدأ (`rustup update`) مع نسخة لمساحة العمل المحلي.
- اختياري: [زوج مفاتيح Ed25519 متوافق مع OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  لتوقيع المانيفستات.
- اختياري: Node.js ≥ 18 إذا كنت تخطط لمعاينة بوابة Docusaurus.

اضبط `export RUST_LOG=info` أثناء إعجابك برسائل CLI الخاصة.

## 1. تحديث الـ Installations الحامية

البدء في ابتكارات قياسية التقسيم (التقطيع) لـ SF-1. كما يصدر الأمر مظاريف
مانيفست موقعة عند تزويد `--signing-key`؛ استخدم `--allow-unsigned` أثناء التطوير
محلي فقط.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

المخرجات:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (إذا تم التوقيع)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2.قسّم الحمولة وفحص البناء

استخدم `sorafs_chunker` لتقسيم الملف أو الأرشيف:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

الفيروسات الأساسية:

- `profile` / `break_mask` – معاملات `sorafs.sf1@1.0.0`.
- `chunks[]` – إزاحات وأطوال طويلة وبصمات الأصابع BLAKE3 للـ قطع.

لـ المباريات الكبرى، شغّل اختبار الانحدار المبني على شرط وتزامن التقسيم
بالتدفق وبالدفعات:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. ابنِ وتوقيع مانيفيستلفّ خطة الـ قطعان والكنى وتواقيع التورم في مانيفيست باستخدام
`sorafs-manifest-stub`. يوضح الأمر أدناه الحمولة لملف واحد؛ مرّر مسار دليل الحزمة
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

مراجعة `/tmp/docs.report.json` من أجل:

- `chunking.chunk_digest_sha3_256` – بصمة SHA3 للإزاحات/الأطول، تطابق التركيبات الخاصة
  بالـ قطع.
- `manifest.manifest_blake3` – بصمة BLAKE3 الموقعة ضمن ظرف المانيفست.
- `chunk_fetch_specs[]` – تعليمات مرتبة حسب جلب الأوركسترات.

عندما تكون جاهزًا لتقديم تواقيع حقيقية، أضف الأسين `--signing-key` و `--signer`.
ويجب الأمر من كل توقيع Ed25519 قبل كتابة الظرف.

## 4.حاكِ الاسترجاع متعدد المتحكمين

استخدم CLI في تشغيل الجلب التطويري لإعادة تخطيط القطع مقابل منظم واحد أو أكثر.
هذه محترفات دمجات في CI ولنموذج الاوركستراتور.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

التحقق:

- `payload_digest_hex` يجب أن يتوافق مع تقرير المانيفست.
- `provider_reports[]` ظهرت تسجيل النجاح/الفشل لكل المرشحين.
- القيمة `chunk_retry_total` غير صفرية تُبرز تعديل الضغط الخلفي.
- مرّر `--max-peers=<n>` ملتقي عدد المتحكمين المجدولين للتشغيل ومحاكاة CI مركز البقاء
  على الكاملين.
- `--retry-budget=<n>` التجاوز العدد الافتراضي لمحاولات إعادة المحاولة لكل قطعة (3)
  لتسريع استعادة بيانات دابات أوركستراتور عند حقن الأعطال.

أضف `--expect-payload-digest=<hex>` و `--expect-payload-len=<bytes>` للفشل بسرعة
عندما ينحرف حمولة المعادى الباخرة عن المانيفست.

##5.الخطوات التالية- **تكامل النتو** – مرّر بصمة المانيفيست و`manifest_signatures.json` إلى سير عمل المجلس
  لتتمكن من تحديد رقم التعريف الشخصي (PIN) الخاص بالتسجيل من إعلان التوافر.
- **التفاوض مع السجل** – راجع [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  قبل تسجيل ملفات تعريف جديدة. ينبغي للإستخدام تفضيلات المفضلة
  (`namespace.name@semver`) على المعرفات الرقمية.
- ** تصميم CI ** – أضف المزيد إلى خطوط النشر حتى تنشر المنشورات والـ تركيبات
  وآرتيفاكت مانيفستات حتمية جنبًا إلى جنب مع بيانات وصفية موقعة.