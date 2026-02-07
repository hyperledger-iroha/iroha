---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تجزئة SoraFS → مسار المانيفست

يمثل هذا المرفق لدليل البدء السريع مساراً من البداية للنهاية يحوّل البايتات الخام إلى
Você pode usar Norito para Pin Registry em SoraFS. المحتوى مقتبس من
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
Verifique o valor do produto e do produto.

## 1. تجزئة حتمية

يستخدم SoraFS ملف تعريف SF-1 (`sorafs.sf1@1.0.0`): hash متدحرج مستوحى من FastCDC مع حد أدنى
Cada pedaço tem 64 KiB, e 256 KiB, e 512 KiB, e é `0x0000ffff`. الملف
É usado em `sorafs_manifest::chunker_registry`.

### مساعدات Ferrugem

- `sorafs_car::CarBuildPlan::single_file` – يُنتج إزاحات pedaços e وأطوالها وملخصات BLAKE3 أثناء
  تجهيز بيانات CAR الوصفية.
- `sorafs_car::ChunkStore` – يمرر payloads بشكل streaming, ويحفظ بيانات chunks الوصفية, ويشتق
  A prova de recuperação (PoR) é de 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – permite que você use o CLI.

### CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

O JSON é usado para armazenar pedaços e pedaços. احتفظ بالخطة عند بناء المانيفستات
أو مواصفات fetch الخاصة بالأوركسترايتور.

### PoR

Nome `ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` e `--por-sample=<count>`
Você pode fazer isso sem problemas. Este é o modelo `--por-proof-out` ou
`--por-sample-out` para JSON.

## 2. تغليف مانيفست

تجمع `ManifestBuilder` بيانات chunks الوصفية مع مرفقات الحوكمة:

- CID الجذر (dag-cbor) e CAR.
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

- `payload.manifest` – um dispositivo de armazenamento de dados para Norito.
- `payload.report.json` – Você pode usar o `chunk_fetch_specs` ou
  `payload_digest_hex` وملخصات CAR وبيانات alias الوصفية.
- `payload.manifest_signatures.json` – ظرف يحتوي على ملخص BLAKE3 para للمانيفست, وملخص SHA3 لخطة
  pedaços, وتوقيعات Ed25519 مرتبة.

Use `--manifest-signatures-in` para obter mais informações sobre o produto
Use `--chunker-profile-id` ou `--chunker-profile=<handle>` para obter mais informações.

## 3. النشر والتثبيت (alfinete)

1. **تقديم الحوكمة** – قدّم ملخص المانيفست وظرف التوقيعات إلى المجلس حتى يمكن قبول الـ pin.
   يجب على المدققين الخارجيين حفظ ملخص SHA3 لخطة pedaços بجانب ملخص المانيفست.
2. **cargas úteis** – ارفع أرشيف CAR (وفهرس CAR الاختياري) المشار إليه في المانيفست إلى
   Registro de pinos. تأكد من أن المانيفست وCAR يشتركان do CID جذر واحد.
3. **تسجيل التليمترية** – احتفظ بتقرير JSON e PoR e مقاييس fetch ضمن artefatos الإصدار.
   Você pode fazer isso com o dinheiro e o dinheiro que deseja.
   cargas úteis.

## 4. محاكاة fetch متعددة المزوّدين

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=provedores/alpha.bin --provider=beta=provedores/beta.bin#4@3 \
  --output = carga útil.bin --json-out = fetch_report.json`- `#<concurrency>` não é compatível com `#4`.
- `@<weight>` يضبط انحياز الجدولة؛ A resposta é 1.
- `--max-peers=<n>` يحد عدد المزوّدين المجدولين للتشغيل عندما يعيد الاكتشاف مرشحين أكثر من المطلوب.
- `--expect-payload-digest` e `--expect-payload-len` não estão disponíveis.
- `--provider-advert=name=advert.to` não pode ser instalado no computador.
- `--retry-budget=<n>` يستبدل عدد المحاولات لكل chunk (الافتراضي: 3) حتى يتمكن CI من كشف
  Você pode fazer isso sem problemas.

يعرض `fetch_report.json` مقاييس مجمّعة (`chunk_retry_total` e `provider_failure_rate` وغيرها)
As afirmações do CI são válidas.

## 5. تحديثات السجل والحوكمة

Para obter mais informações sobre o chunker:

1. Verifique o código em `sorafs_manifest::chunker_registry_data`.
2. Verifique `docs/source/sorafs/chunker_registry.md` e remova o cabo.
3. Fixe os acessórios de fixação (`export_vectors`) e os acessórios.
4. Certifique-se de que o dispositivo está funcionando corretamente.

Não há nenhum identificador de identificador (`namespace.name@semver`) e identificador de IDs
Não há nada que você possa fazer.