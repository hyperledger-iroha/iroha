---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# البدء السريع لـ SoraFS

هذا هو الدليل العملي لملف تحديد القطع SF-1،
حشد البيانات وتدفق البحث عن مقدمين متعددين يدعمهم
خط أنابيب التخزين SoraFS. الجمع بين س كوم س
[دمج عميق بدون بيانات خطية](manifest-pipeline.md)
لملاحظات التصميم والإشارة إلى علامات CLI.

## المتطلبات المسبقة

- سلسلة أدوات الصدأ (`rustup update`)، نسخة محلية من مساحة العمل.
- اختياري: [وفقًا للشروط Ed25519 المتوافقة مع OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  لقتل البيانات.
- اختياري: Node.js ≥ 18 إذا كنت تريد التصور المسبق للبوابة Docusaurus.

قم بتعريف `export RUST_LOG=info` أثناء الخصيتين لتصدير الرسائل المفيدة من CLI.

## 1. تحديث محددات التركيبات

تم استخدام أسلحة التقطيع الكنسي الجديدة SF-1. يا كوماندوز ستنطلق أيضًا
مغلفات البيان التي تم اغتيالها عندما `--signing-key` é fornecido؛ استخدام
`--allow-unsigned` لا يتم تطويره محليًا.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

سعيداس:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (تم اغتياله)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. تجزئة الحمولة والفحص أو التخطيط

استخدم `sorafs_chunker` لتجزئة ملف أو ملف مضغوط بشكل عشوائي:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

كامبوس تشافي:

- `profile` / `break_mask` – تأكيد معلمات `sorafs.sf1@1.0.0`.
- `chunks[]` – إزاحة الترتيبات والملحقات وهضم قطع BLAKE3.بالنسبة للمباريات الأكبر، قم بتنفيذ التراجع بهدف ضمان ذلك
تقطيع البث ومزامنته بشكل دائم:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. بناء وتنفيذ بيان

تضمين خطة القطع والأسماء المستعارة وأعضاء الإدارة في بيان
أوساندو `sorafs-manifest-stub`. الأمر التالي يظهر حمولة ملف واحد؛ مرور
طريق الدليل لتعبئة شيء ما (CLI لترتيب المعجم).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

مراجعة `/tmp/docs.report.json` للفقرة:

- `chunking.chunk_digest_sha3_256` – ملخص SHA3 للإزاحات/التعليقات، المراسلات أيضًا
  تركيبات تفعل مقسم.
- `manifest.manifest_blake3` – ملخص BLAKE3 الذي تم اغتياله بدون مظروف في البيان.
- `chunk_fetch_specs[]` – تعليمات البحث عن الأوركسترا.

Quando estiver pronto para fornecer assinaturas حقيقية، إضافة إلى الحجج
`--signing-key` و`--signer`. O أمر التحقق من كل assinatura Ed25519 قبل الحفر
يا المغلف.

## 4. محاكاة الاسترداد متعدد المزودين

استخدم CLI لجلب التطوير لإعادة إنتاج أو تخطيط القطع ضدك
المزيد من الأدلة. إنه مثالي لاختبارات دخان CI ونموذج الأوركيستراد.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

التحقق:- `payload_digest_hex` يجب أن يتوافق مع بيان البيان.
- `provider_reports[]` يعرض عدوى النجاح/النجاح من خلال إثباته.
- `chunk_retry_total` مختلف عن الصفر لضبط الضغط الخلفي.
- قم بتمرير `--max-peers=<n>` لتحديد عدد المعالجين المبرمجين للتنفيذ
  ونستخدم محاكاة CI لمرشحينا الرئيسيين.
- `--retry-budget=<n>` استبدال لوحة التجارب للقطعة (3) للتصدير
  تراجعات الأوركسترا أسرع من خلال إدخال الأخطاء.

Adicione `--expect-payload-digest=<hex>` و`--expect-payload-len=<bytes>` للفالهار
بسرعة عندما يتم إعادة بناء الحمولة بشكل مختلف عن البيان.

## 5. المرور التالي

- **تكامل الحكم** – أرسل ملخص البيان و`manifest_signatures.json`
  من أجل تدفق النصيحة حتى يتمكن Pin Registry من الإعلان عن التوفر.
- **مفاوضات التسجيل** – راجع [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  قبل التسجيل الجديد. يجب أن تفضل الأتمتة المعرفات التقليدية
  (`namespace.name@semver`) من خلال المعرفات الرقمية.
- **Automação de CI** – إضافة الأوامر الموجودة على خطوط الأنابيب لإصدارها
  الوثائق والتركيبات والتحف والبيانات العامة الحتمية جنبًا إلى جنب مع
  metadados assinados.