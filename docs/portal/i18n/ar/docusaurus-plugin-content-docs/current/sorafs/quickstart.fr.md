---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# التنفيذ السريع SoraFS

سيعمل هذا الدليل على إعادة النظر في ملف تعريف القطع SF-1 المحدد,
التوقيع على البيانات وتدفق الاسترداد لموردين متعددين
تميل بشدة إلى خط أنابيب التخزين SoraFS. أكمل المستوى
l'[تحليل دقيق لخط البيانات](manifest-pipeline.md)
لملاحظات المفهوم ومرجع الأعلام CLI.

## المتطلبات الأساسية

- Toolchain Rust (`rustup update`)، نسخة محلية من مساحة العمل.
- الخيار: [زوج من المفاتيح Ed25519 المولدة بواسطة OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  صب التوقيع على البيانات.
- الخيار: Node.js ≥ 18 إذا كنت قد قمت بمعاينة البوابة Docusaurus.

قم بتعريف `export RUST_LOG=info` من خلال المقالات لعرض رسائل CLI المساعدة.

## 1. قم بتفكيك التركيبات المحددة

قم بإعادة إنشاء ناقلات الديكوباج SF-1 canoniques. لا أمر المنتج أيضا
مغلفات البيان الموقعة عندما يتم تسليم `--signing-key` ؛ يستخدم
`--allow-unsigned` فريد من نوعه في التطوير المحلي.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

الطلعات الجوية :

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (si Signé)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. اقطع الحمولة وافحص الخطة

استخدم `sorafs_chunker` لفك ملف أو أرشيف عشوائي:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

مفاتيح الأبطال :- `profile` / `break_mask` – تأكيد إعدادات `sorafs.sf1@1.0.0`.
- `chunks[]` – إزاحة القطع المنتظمة والطولية والإمبراطورية BLAKE3 للقطع.

من أجل التركيبات ذات الأحجام الكبيرة، قم بتنفيذ الانحدار بناءً على ما يناسبك
التأكد من مزامنة البث المباشر والباقي:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. إنشاء بيان وتوقيعه

قم بتغليف خطة القطع والأسماء المستعارة وتوقيعات الإدارة في الأمم المتحدة
البيان عبر `sorafs-manifest-stub`. يوضح الأمر ci-dessous الحمولة
ملف فريد ; قم بتمرير مسار ذخيرة لتغليف شجرة (la CLI le
parcourt en ordre lexicographique).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

فحص `/tmp/docs.report.json` ل:

- `chunking.chunk_digest_sha3_256` – تمكين SHA3 من الإزاحات/الأطوال، المقابلة للمساعد
  تركيبات دو تشانكر.
- `manifest.manifest_blake3` – تمكين BLAKE3 التوقيع في ظرف البيان.
- `chunk_fetch_specs[]` – تعليمات الاسترداد المخصصة للمنسقين.

عندما تكون جاهزًا لتقديم التوقيعات الحقيقية، قم بإضافة الوسائط
`--signing-key` و`--signer`. لا أمر التحقق من كل توقيع Ed25519 مقدما
كتابة المغلف.

## 4. محاكاة عملية الاسترداد المتعددة

استخدم CLI de fetch de développement لتجديد خطة القطع مقابل واحدة أو
بالإضافة إلى المستأجرين. إنه مثالي لاختبارات الدخان CI والنموذج الأولي
d'orchesstrateur.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

التحقق :- `payload_digest_hex` قم بمراسلة تقرير البيان.
- `provider_reports[]` يعرض حسابات النجاح/التحقق من قبل المورد.
- Un `chunk_retry_total` غير موجود كدليل على تعديلات الضغط الخلفي.
- قم بتمرير `--max-peers=<n>` لتحديد عدد الموردين المخططين من أجل واحد
  تنفيذ وحماية مراكز محاكاة CI للمرشحين الرئيسيين.
- `--retry-budget=<n>` يستبدل الرقم المؤقت للجزء الافتراضي (3) من أجل
  قياس الأدلة بالإضافة إلى تراجعات المنسق أثناء الحقن
  شيك.

أضف `--expect-payload-digest=<hex>` و`--expect-payload-len=<bytes>` للسماعة
يتم ذلك بسرعة عندما يتم إعادة بناء الحمولة ببطاقة البيان.

## 5. الأشرطة اللاحقة

- **الحوكمة التكاملية** – إدارة عملية البيان والإعلان
  `manifest_signatures.json` في تدفق المشورة حتى تتمكن من تشغيل Pin Registry
  إعلان التوفر.
- **مفاوضات التسجيل** – استشارة [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  قبل تسجيل الملفات الشخصية الجديدة. تعمل الأتمتة على منح المقابض امتيازًا
  الكنسي (`namespace.name@semver`) يحتوي على المعرف الرقمي.
- **أتمتة CI** – إضافة أوامر CI إلى خطوط الأنابيب التي يتم إطلاقها من أجلها
  الوثائق والتركيبات والمصنوعات التي تنشر البيانات المحددة
  مع التوقيع على métadonnées.