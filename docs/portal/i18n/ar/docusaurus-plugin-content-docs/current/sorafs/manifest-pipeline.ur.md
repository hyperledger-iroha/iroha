---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/manifest-pipeline.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS تقطيع → خط أنابيب البيان

بداية سريعة وهي عبارة عن خط أنابيب مكتمل ومتكامل ومصدر خام Norito
تظهر البيانات الموجودة في SoraFS Pin Registry لموزو. مادة
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md)؛
سے ما خوذة ہے؛ فيلم وثائقي وضاح وتبديل لاغ لـ دستاويز سے رجوع كریں۔

## 1. حطی التقطيع

SoraFS SF-1 (`sorafs.sf1@1.0.0`) يستخدم كرتا: FastCDC راوتر رولينغز
الجزء العلوي 64 كيلو بايت، أكثر من 256 كيلو بايت، أكثر من 512 كيلو بايت وكتلة ضخمة `0x0000ffff`
ہے۔ تم تسجيله بالفعل `sorafs_manifest::chunker_registry`.

### الصدأ مددگار

- `sorafs_car::CarBuildPlan::single_file` – إزاحة قطع قطع غيار السيارات،
  هضم وBLAKE3 جارى كرتا.
- `sorafs_car::ChunkStore` – الحمولات الصافية والأجزاء التي تحتوي على قطع صغيرة و
  64 كيلو بايت / 4 كيلو بايت هو دليل إثبات الاسترجاع (PoR) الذي تم أخذه على محمل الجد.
- `sorafs_chunker::chunk_bytes_with_digests` – دون تحديد CLIs للمساعد الجديد.

### CLI ٹولنگ

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON عبارة عن إزاحات حربية وملخصات وملخصات كبيرة. جلب البيان أو الجلب
لقد حان الوقت لخطة رائعة للفتيات.

### PoR غواهاياں

`ChunkStore` `--por-proof=<chunk>:<segment>:<leaf>` و `--por-sample=<count>`
قم بحماية اللاعبين من موقع سكويك. علامات `--por-proof-out` أو `--por-sample-out`
لقد تم استخدام أداة تسجيل JSON في وقت لاحق.

## 2. بيان کو لپیتناقطع `ManifestBuilder` التي تحتوي على المزيد من المعلومات التي تم إجراؤها:

- روٹ CID (dag-cbor) والتزامات CAR ۔
- الاسم المستعار ثبوت وفراہم کنندہ صلاحیت کے دعوے۔
- المستشار والجهاز والمتخصصون (مثل بناء المعرفات).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

آما آخر:

- `payload.manifest` – Norito بيان البيان .
- `payload.report.json` – انسان/آٹومیشن كيلئے قابل فہم خلاصہ، جس میں `chunk_fetch_specs`,
  `payload_digest_hex`، تحتوي ملخصات CAR والاسم المستعار على المزيد.
- `payload.manifest_signatures.json` – لتكملة بيان BLAKE3، قطعة الخطة
  ملخص SHA3 وتصنيف Ed25519 هو برنامج شامل.

`--manifest-signatures-in` استخدام جهاز الكمبيوتر المحمول لإعادة التدوير
تم التحقق من الإصدار السابق و`--chunker-profile-id` أو `--chunker-profile=<handle>`
قم باختيار التسجيل.

## 3.اشاعت و پننگ1. **الجمع بين الصور** – الملخص الواضح وخط المساعدة للمجلس الذي ينشئ دبوسًا
   خطاب . العناصر البيرونية هي جزء من خطة SHA3 التي تحتوي على ملخص واضح وهو ما يجعل الأمر محفوظًا.
2. **الحمولات الصافية** – بيان حوالة دي جي آركايو (وأختيار رسوميات السيارات)
   يمكن أيضًا تنزيل Pin Registry. هذه الإرشادات تظهر وCAR هي بطاقة CID الرئيسية.
3. **السجلات المعدنية** – تقرير JSON وPoR غوايلا وما إلى ذلك يجلبون البيانات إلى الريليز
   المقالة القادمةمهم للغاية. إنها تقوم بتسجيل بورصة ذات سعة كبيرة وحمولات صافية
   يمكنك تنزيل المزيد من القضايا من جديد.

## 4. ملفات متعددة لجلب المحاكاة

`تشغيل البضائع -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` تم إجراء التوازي على التوازي (`#4` أول).
- `@<weight>` تحيز التحيز؛ ڈيفالٹ 1 .
- `--max-peers=<n>` تم إحراز تقدم كبير في هذا المجال للمرشحين المنتخبين الذين يبلغ عددهم نطاقًا محدودًا.
- `--expect-payload-digest` و `--expect-payload-len` خاموش كرپشن سے بچاتے ہیں.
- `--provider-advert=name=advert.to` تم تطوير المزيد من التحسين المستمر لصلاحيتو.
- `--retry-budget=<n>` عدد القطع الكبير من العدادات العالمية (عدد: 3) بدلتا وسعة CI
  منظر طبيعي ظاهري.`fetch_report.json` مجموع المقاييس (`chunk_retry_total`, `provider_failure_rate` وغير ذلك)
تأكيدات CI ومصادر المعلومات موجودة.

## 5. تسجيل المواقع والمواقع

أحدث ابتكارات Chunker في الوقت الحالي:

1. `sorafs_manifest::chunker_registry_data` هو واصف.
2.`docs/source/sorafs/chunker_registry.md` والمواثيق ذات الصلة بالسياسة النقدية.
3. التركيبات (`export_vectors`) معتمدة من قبل السجل التجاري والبيانات الموقعة.
4. متابعة إعداد تقرير الامتثال للميثاق الذي تم جمعه.

آلية المقابض الأساسية (`namespace.name@semver`) التي يتم ترجيحها وصرفها
هناك حاجة إلى مبدأ أساسي لعدد من معرفات الاتصال بالإنترنت.