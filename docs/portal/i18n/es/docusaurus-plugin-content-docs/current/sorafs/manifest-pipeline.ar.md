---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تجزئة SoraFS → مسار المانيفست

يمثل هذا المرفق لدليل البدء السريع مساراً من البداية للنهاية يحوّل البايتات الخام إلى
Utilice Norito para establecer el registro de PIN en SoraFS. المحتوى مقتبس من
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
راجِع ذلك المستند للمواصفة المعتمدة وسجل التغييرات.

## 1. تجزئة حتمية

يستخدم SoraFS ملف تعريف SF-1 (`sorafs.sf1@1.0.0`): hash متدحرج مستوحى من FastCDC مع حد أدنى
Cada fragmento tiene 64 KiB, 256 KiB y 512 KiB y `0x0000ffff`. الملف
Aquí está `sorafs_manifest::chunker_registry`.

### Óxido

- `sorafs_car::CarBuildPlan::single_file` – يُنتج إزاحات trozos y أطوالها y ملخصات BLAKE3 أثناء
  تجهيز بيانات CAR الوصفية.
- `sorafs_car::ChunkStore`: cargas útiles de transmisión por secuencias y fragmentos de datos
  La prueba de recuperación (PoR) es de 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Haga clic en el botón CLI.

### أدوات CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

Este JSON contiene archivos y fragmentos. احتفظ بالخطة عند بناء المانيفستات
أو مواصفات buscar الخاصة بالأوركسترايتور.

### شواهد PoR

Ajustes `ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` y `--por-sample=<count>`
يتمكن المدققون من طلب مجموعات شواهد حتمية. قرن هذه الأعلام مع `--por-proof-out` أو
`--por-sample-out` en JSON.

## 2. تغليف مانيفست

تجمع `ManifestBuilder` بيانات chunks الوصفية مع مرفقات الحوكمة:- CID الجذر (dag-cbor) وتعهدات CAR.
- إثباتات alias ومطالبات قدرات المزوّدين.
- توقيعات المجلس وبيانات وصفية اختيارية (compilación de مثل معرفات).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

مخرجات مهمة:

- `payload.manifest` – La configuración de la unidad se realiza mediante Norito.
- `payload.report.json` – Para obtener más información, consulte el documento `chunk_fetch_specs` y
  `payload_digest_hex` وملخصات CAR وبيانات alias الوصفية.
- `payload.manifest_signatures.json` – ظرف يحتوي على ملخص BLAKE3 للمانيفست، y SHA3 لخطة
  trozos, وتوقيعات Ed25519 مرتبة.

استخدم `--manifest-signatures-in` للتحقق من الأظرف القادمة من موقّعين خارجيين قبل إعادة
Las conexiones `--chunker-profile-id` y `--chunker-profile=<handle>` están conectadas.

## 3. النشر والتثبيت (pin)

1. **تقديم الحوكمة** – قدّم ملخص المانيفست وظرف التوقيعات إلى المجلس حتى يمكن قبول الـ pin.
   Hay varios fragmentos de SHA3 que se pueden utilizar.
2. **تثبيت cargas útiles** – ارفع أرشيف CAR (وفهرس CAR الاختياري) المشار إليه في المانيفست إلى
   Registro de pines. تأكد من أن المانيفست وCAR يشتركان في CID جذر واحد.
3. **تسجيل التليمترية** – Utilice JSON y PoR para buscar artefactos.
   تغذي هذه السجلات لوحات معلومات المشغلين وتساعد على إعادة إنتاج المشكلات دون تنزيل
   cargas útiles كبيرة.

## 4. محاكاة buscar متعددة المزوّدين`ejecución de carga -p sorafs_car --bin sorafs_fetch --plan=payload.report.json \
  --provider=alpha=proveedores/alpha.bin --provider=beta=proveedores/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` يزيد التوازي لكل مزوّد (`#4` أعلاه).
- `@<weight>` يضبط انحياز الجدولة؛ القيمة الافتراضية هي 1.
- `--max-peers=<n>` يحد عدد المزوّدين المجدولين للتشغيل عندما يعيد الاكتشاف مرشحين أكثر من المطلوب.
- `--expect-payload-digest` y `--expect-payload-len` están conectados a la red.
- `--provider-advert=name=advert.to` يتحقق من قدرات المزوّد قبل استخدامه في المحاكاة.
- `--retry-budget=<n>` يستبدل عدد المحاولات لكل chunk (الافتراضي: 3) حتى يتمكن CI من كشف
  التراجعات أسرع عند اختبار سيناريوهات الفشل.

يعرض `fetch_report.json` مقاييس مجمّعة (`chunk_retry_total` و`provider_failure_rate` وغيرها)
مناسبة لassertions الخاصة بـ CI وقابلية الملاحظة.

## 5. تحديثات السجل والحوكمة

عند اقتراح ملفات تعريف chunker جديدة:

1. أنشئ الوصف في `sorafs_manifest::chunker_registry_data`.
2. حدّث `docs/source/sorafs/chunker_registry.md` والمواثيق ذات الصلة.
3. أعد توليد accesorios (`export_vectors`) والتقط المانيفستات الموقّعة.
4. قدّم تقرير الامتثال للميثاق مع توقيعات الحوكمة.

Esta opción maneja el nombre de usuario (`namespace.name@semver`) y muestra los identificadores de identificación.
إلا عند الحاجة إلى التوافق الرجعي.