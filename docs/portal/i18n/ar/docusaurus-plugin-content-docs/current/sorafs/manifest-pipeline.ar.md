---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تجزئة SoraFS → مسار المانيفيست

يمثل هذا المرفق لدليل المبتدئين المسار السريع من البداية للنهاية يحوّل البايتات الخام إلى
مانيفستات Norito مناسبة لـ Pin Registry في SoraFS. المادة مقتبسة من
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md)؛
راجع هذا المستند لتغيير الوجه.

## 1. تجزئة حتمية

يستخدم SoraFS ملف تعريف SF-1 (`sorafs.sf1@1.0.0`): هاش مموج مستوحى من FastCDC مع حد أدنى
لحجم قطعة مخصص 64 كيلو بايت، والهدف 256 كيلو بايت، أقصى حد 512 كيلو بايت، وقناع كسر `0x0000ffff`. الملف
مسجل في `sorafs_manifest::chunker_registry`.

### مساعدات الصدأ

- `sorafs_car::CarBuildPlan::single_file` – يتم إنتاج إزاحات قطع وأطوالها وملخصات BLAKE3 خلال
  تجهيزات بيانات CAR الوصفية.
- `sorafs_car::ChunkStore` – يمرر الحمولات بشكل متدفق، ويحفظ بيانات القطع الوصفية، ويشتق
  أخذت شجرة عينة من إثبات قابلية الاسترجاع (PoR) بحجم 64 كيلو بايت / 4 كيلو بايت.
- `sorafs_chunker::chunk_bytes_with_digests` – مساعد مكتبة الاتصال خلف كلا الـ CLI.

### أدوات CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

يحتوي على JSON على الإزاحات، وعلى طول وملخصات القطع. تستخدم بالخطة عند بناء المانيفستات
أو مواصفات الجلب الخاصة بالأوركستراتور.

### شواهد بور

تُتيح `ChunkStore` الخيارين `--por-proof=<chunk>:<segment>:<leaf>` و`--por-sample=<count>` حتى
يشترط المون من الطلب مجموعات شواهد حتمية. قرن هذه الأعلام مع `--por-proof-out` أو
`--por-sample-out` لتسجيل JSON.

## 2. تعبئة المانيفيست

ميجا `ManifestBuilder` قطع البيانات الوصفية مع مرفقات ال تور:- CID RGB (dag-cbor) و تعهدات CAR.
- إثباتات الاسم المستعار ومطالبات القدرات المتحكمين.
- توقيعات مجلس وبيانات وصفية اختيارية (مثل معرفات البناء).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

مخرجات مهمة:

- `payload.manifest` – بايتات مانيفيست مشفرة بـ Norito.
- `payload.report.json` – ملخص قابل للقراءة للبشر/الطباعة يشمل `chunk_fetch_specs` و
  `payload_digest_hex` وملخصات CAR وبيانات مستعارة الوصفية.
- `payload.manifest_signatures.json` – ظرف يحتوي على ملخص BLAKE3 للمانيفست، وملخص SHA3 لخطة
  قطع، وتوقيعات Ed25519 مرتبة.

استخدم `--manifest-signatures-in` من الأظرف القادمة من إعادة التوقيع النهائي من قبل
كتابها، لتحديد `--chunker-profile-id` أو `--chunker-profile=<handle>` لحجز السجل.

## 3. النشر والتثبيت (pin)

1. **تقديم التورم** – مجمع المانيفيست ووظرف التوقيعات إلى المجلس حتى يمكن قبول الـ pin.
   يجب على المتسلسلين الخارجيين الحفاظ على ملخص SHA3 لخطة قطع استمرار ملخص المانيفست.
2. **تثبيت الحمولات** – رفع أرشيف CAR (وفرس CAR الاختياري) المشار إليه في المانيفيست إلى
   سجل الدبوس. تأكد من أن المانيفيست وCAR يشتركان في CID جذر واحد.
3. **تسجيل التليميترية** – استخدم بتقرير JSON وشواهد PoR واختيار مقاييس الجلب ضمن نسخة القطع الأثرية.
   تغذي هذه السجلات لوحات المعلومات للبدء في إعادة إنتاج المشكلات دون تنزيل
   الحمولات كبيرة.

##4.محاكاة الجلب المتعددة المتحكمين`تشغيل البضائع -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` يزيد التوازي لكل متوازي (`#4` فيما بعد).
- `@<weight>` يضبط انحياز الجدولة؛ القيمة القيمة هي 1.
- `--max-peers=<n>` ويستقبل عدد المتحكمين المجدولين للتشغيل عندما لا يقبل مرشحين أكثر من المطلوب.
- `--expect-payload-digest` و`--expect-payload-len` الحكمان من غير الصامت.
- `--provider-advert=name=advert.to` ويتحقق من قدرات المتحكمين قبل استخدامه في المحاكاة.
- `--retry-budget=<n>` يستبدل عدد المحاولات لكل قطعة (الافتراضي: 3) حتى متقدمة CI من كشف
  الكاراتيات الأسرع عند الاختبار سيناريوهات لذلك.

المعروضة `fetch_report.json` معايير مجمّعة (`chunk_retry_total` و`provider_failure_rate` وغيرها)
التأكيدات المناسبة الخاصة بـ CI وإمكانية الربح.

## 5. تحديثات السجل والتكامل

عند تكوين ملفات تعريف مقسم جديد:

1. أنشئ الوصف في `sorafs_manifest::chunker_registry_data`.
2. تحديث `docs/source/sorafs/chunker_registry.md` والمواثيق ذات العلاقة.
3. إعادة توليد التركيبات (`export_vectors`) واختار المانيفستات الموقّعة.
4. ولم يؤكد للمحادثة مع توقيعات الـ.

ينبغي أن يستخدم المطبوع مقابض (`namespace.name@semver`) ولا تعود إلى المعرفات الرقمية
إلا عند الحاجة إلى التوافق الرجوعي.