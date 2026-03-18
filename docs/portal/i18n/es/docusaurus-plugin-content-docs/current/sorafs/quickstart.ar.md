---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# البدء السريع في SoraFS

يرشدك هذا الدليل العملي عبر ملف تعريف الـ chunker الحتمي SF-1,
وتوقيع المانيفست، ومسار الجلب متعدد المزوّدين الذي يدعم خط أنابيب تخزين SoraFS.
وازنه مع [التعمّق في خط أنابيب المانيفست](manifest-pipeline.md)
للحصول على ملاحظات التصميم ومرجع أعلام سطر الأوامر.

## المتطلبات الأساسية

- أداة Rust (`rustup update`) مع نسخ مساحة العمل محليًا.
- Texto: [زوج مفاتيح Ed25519 متوافق مع OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  لتوقيع المانيفستات.
- Nombre: Node.js ≥ 18 إذا كنت تخطط لمعاينة بوابة Docusaurus.

Utilice `export RUST_LOG=info` para acceder a la CLI المفيدة.

## 1. تحديث الـ accesorios الحتمية

أعد توليد متجهات التقسيم (fragmentación) القياسية لـ SF-1. كما يُصدر الأمر مظاريف
مانيفست موقعة عند تزويد `--signing-key`؛ Fuente de alimentación `--allow-unsigned`
المحلي فقط.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

المخرجات:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (إذا تم التوقيع)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Carga útil de قسّم وافحص الخطة

Aquí está el mensaje `sorafs_chunker` de la siguiente manera:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

الحقول الأساسية:

- `profile` / `break_mask` – يؤكد معاملات `sorafs.sf1@1.0.0`.
- `chunks[]` – إزاحات وأطوال مرتبة وبصمات BLAKE3 للـ trozos.

لـ accesorios الأكبر، شغّل اختبار الانحدار المبني على proptest لضمان تزامن التقسيم
بالتدفق y بالدفعات:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. ابنِ ووقّع مانيفستلفّ خطة الـ trozos والكنى وتواقيع الحوكمة في مانيفست باستخدام
`sorafs-manifest-stub`. يوضح الأمر أدناه carga útil لملف واحد؛ مرّر مسار دليل لحزم
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

Nombre `/tmp/docs.report.json` aquí:

- `chunking.chunk_digest_sha3_256` – بصمة SHA3 للإزاحات/الأطوال، تطابق accesorios الخاصة
  بالـ trozo.
- `manifest.manifest_blake3` – بصمة BLAKE3 الموقعة ضمن ظرف المانيفست.
- `chunk_fetch_specs[]` – تعليمات جلب مرتبة للأوركستراتورات.

Para obtener más información, consulte el documento "`--signing-key` e `--signer`".
يتحقق الأمر من كل توقيع Ed25519 قبل كتابة الظرف.

## 4. حاكِ الاسترجاع متعدد المزوّدين

Utilice la CLI para configurar los fragmentos que desee.
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
- قيمة `chunk_retry_total` غير الصفرية تُبرز تعديلات contrapresión.
- مرّر `--max-peers=<n>` لتقييد عدد المزوّدين المجدولين للتشغيل وإبقاء محاكاة CI مركّزة
  على المرشحين الأساسيين.
- `--retry-budget=<n>` يتجاوز العدد الافتراضي لمحاولات إعادة المحاولة لكل trozo (3)
  لتسريع كشف تراجعات الأوركستراتور عند حقن الأعطال.

Utilice `--expect-payload-digest=<hex>` e `--expect-payload-len=<bytes>` para evitar errores.
عندما ينحرف carga útil المعاد بناؤه عن المانيفست.

## 5. الخطوات التالية- **تكامل الحوكمة** – مرّر بصمة المانيفست و`manifest_signatures.json` إلى سير عمل المجلس
  لكي يتمكن Registro de PIN من إعلان التوافر.
- **التفاوض مع السجل** – راجع [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  قبل تسجيل ملفات تعريف جديدة. ينبغي للأتمتة تفضيل المعالجات القياسية
  (`namespace.name@semver`) على المعرفات الرقمية.
- **أتمتة CI** – أضف الأوامر أعلاه إلى خطوط إصدار النشر حتى تنشر المستندات Y ACCESORIOS
  والآرتيفاكت مانيفستات حتمية جنبًا إلى جنب مع بيانات وصفية موقعة.