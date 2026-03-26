---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-لچکدار لین
عنوان: لچکدار لین پروسیسنگ (NX-7)
سائڈبار_لیبل: لچکدار لین پروسیسنگ
تفصیل: Nexus لین منشور ، کیٹلاگ اندراجات ، اور رول آؤٹ ڈائریکٹریز بنانے کے لئے ایک بوٹسٹریپ ورک فلو۔
---

::: سرکاری ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/nexus_elastic_lane.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں ایک جیسی رکھیں جب تک کہ ترجمے کا اسکین پورٹل تک نہ پہنچے۔
:::

# لچکدار لین پروسیسنگ کٹ (NX-7)

> ** روڈ میپ آئٹم: ** NX -7 - لچکدار لین کی تیاری کے اوزار  
> ** حیثیت: ** ٹولز مکمل - ظاہر ہوتا ہے ، کیٹلاگ کے ٹکڑوں ، Norito پے لوڈ ، دھواں ٹیسٹ ،
> بنڈل لوڈ ٹیسٹ اسسٹنٹ اب تمام سلاٹ کے لئے رسپانس ٹائم گیٹ جمع کرتا ہے + آڈیٹرز کو لوڈ ٹیسٹ شائع کرنے کے لئے ثبوت ظاہر کرتا ہے
> کسٹم اسکرپٹ کے بغیر۔

یہ گائیڈ آپریٹرز کو نئے مددگار `scripts/nexus_lane_bootstrap.sh` کے ذریعے چلتا ہے جو لین مینی فیسٹ ، لین/ڈیٹا اسپیس کیٹلاگ کے ٹکڑوں ، اور رول آؤٹ ڈائریکٹریوں کی نسل کو خود کار کرتا ہے۔ مقصد یہ ہے کہ Nexus (عوامی یا نجی) میں نئی ​​لینوں کی تشکیل کو دستی طور پر متعدد فائلوں میں ترمیم کیے بغیر اور کیٹلاگ جیومیٹری کو دستی طور پر دوبارہ تیار کیے بغیر آسان بنانا ہے۔

## 1. شرائط

1۔ لین ، ڈیٹا اسپیس ، ویلیویٹر گروپ ، فالٹ رواداری (`f`) ، اور تصفیہ پالیسی کے لئے عرف کی حکمرانی کی منظوری۔
2. آڈیٹرز کی حتمی فہرست (اکاؤنٹ IDs) اور محفوظ نام کی جگہوں کی فہرست۔
3. معاہدے کی تشکیل کے ذخیرے تک رسائی حاصل کریں تاکہ آپ تیار کردہ ٹکڑوں کو شامل کرسکیں۔
4. لین کے منشور رجسٹر کے راستے (`nexus.registry.manifest_directory` اور `cache_directory` دیکھیں)۔
5. لین سے متعلق ٹیلی میٹری/پیجریڈی سوئچز لہذا لین کی خدمت میں داخل ہوتے ہی انتباہات سے وابستہ ہوسکتے ہیں۔

## 2. لین کے لئے نوادرات پیدا کریں

ذخیرہ کی جڑ سے مددگار چلائیں:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --validator soraカタカナ... \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

سب سے اہم میڈیا:

- `--lane-id` انڈیکس کو `nexus.lane_catalog` میں نئی ​​اندراج سے ملنا چاہئے۔
- `--dataspace-alias` اور `--dataspace-id/hash` ڈیٹا اسپیس کیٹلاگ انٹری کو کنٹرول کریں (بطور ڈیفالٹ لین ID حذف کرتے وقت استعمال ہوتا ہے)۔
- `--validator` کو `--validators-file` سے نقل یا پڑھا جاسکتا ہے۔
- `--route-instruction` / `--route-account` برآمدات پیسٹ کے لئے تیار روٹنگ کے قواعد۔
- `--metadata key=value` (یا `--telemetry-contact/channel/runbook`) رن بک رابطوں کو اپنی گرفت میں لے جاتا ہے تاکہ بورڈ اپنے صحیح مالکان کو ظاہر کریں۔
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` جب لین کو توسیعی رن ٹائم کنٹرولز کی ضرورت ہوتی ہے تو ظاہر کرنے میں ہک رن ٹائم اپ گریڈ شامل کرتا ہے۔
- `--encode-space-directory` `cargo xtask space-directory encode` خود بخود کال کرتا ہے۔ اگر آپ چاہتے ہیں کہ انکرپٹڈ `.to` فائل کو پہلے سے طے شدہ راستے میں رکھا جائے تو اگر آپ چاہتے ہیں تو اسے `--space-directory-out` کے ساتھ استعمال کریں۔

اسکرپٹ `--output-dir` (پہلے سے طے شدہ موجودہ ڈائرکٹری ہے) کے اندر تین نوادرات تیار کرتا ہے ، جب انکوڈنگ فعال ہوتی ہے تو چوتھا اختیاری ہوتا ہے:

1. `<slug>.manifest.json` - ہک رن ٹائم اپ گریڈ کے لئے کورم کی توثیق کرنے والے ، محفوظ نام کی جگہوں اور اختیاری ڈیٹا پر مشتمل لین کے لئے ظاہر ہے۔
2. `<slug>.catalog.toml` - toml اسنیپٹ جس میں `[[nexus.lane_catalog]]` ، `[[nexus.dataspace_catalog]]` اور روٹنگ کے کسی بھی مطلوبہ قواعد شامل ہیں۔ اس بات کو یقینی بنائیں کہ لین ریلے پینل (`3f+1`) کی وضاحت کرنے کے لئے `fault_tolerance` ڈیٹا اسپیس انٹری میں ترتیب دیا گیا ہے۔
3. `<slug>.summary.json` - آرکیٹیکچر (سلگ ، سیکٹرز ، ڈیٹا) ، مطلوبہ رول آؤٹ اقدامات ، اور عین مطابق کمانڈ `cargo xtask space-directory encode` (`space_directory_encode.command` کے اندر) کی وضاحت کرنے والا ایک آڈٹ سمری۔ اس JSON کو بطور ثبوت اپنے آن بورڈنگ ٹکٹ سے منسلک کریں۔
4. `<slug>.manifest.to` - جب `--encode-space-directory` کو چالو کیا جاتا ہے تو جاری کیا جاتا ہے۔ `iroha app space-directory manifest publish` کو Torii میں فلش کرنے کے لئے تیار ہے۔

فائلوں کو لکھے بغیر JSON/ٹکڑوں کا پیش نظارہ کرنے کے لئے `--dry-run` کا استعمال کریں ، اور موجودہ نمونے کو اوور رائٹ کرنے کے لئے `--force`۔## 3. تبدیلیوں کا اطلاق کریں

1. مینیفیسٹ JSON کو `nexus.registry.manifest_directory` فارمیٹ میں کاپی کریں (اور اگر رجسٹری ریموٹ پیکجوں سے مماثل ہے تو کیش ڈائرکٹری میں)۔ فائل کا ارتکاب کریں اگر کنفیگریشن ریپوزٹری میں کاپی کرکے منشور کا انتظام کیا جائے۔
2. `config/config.toml` (یا مناسب `config.d/*.toml`) پر دائیں کیٹلاگ کا اقتباس۔ اس بات کو یقینی بنائیں کہ `nexus.lane_count` کم از کم `lane_id + 1` کے برابر ہے اور اپ ڈیٹ ہے جس کو `nexus.routing_policy.rules` کو نئی لین کی طرف اشارہ کرنا چاہئے۔
3. انکرپٹ (اگر آپ `--encode-space-directory` سے تجاوز کرتے ہیں) اور خلاصہ (`space_directory_encode.command`) میں حاصل کردہ کمانڈ کا استعمال کرتے ہوئے خلائی ڈائرکٹری میں ظاہر کریں۔ اس سے پے لوڈ `.manifest.to` پیدا ہوتا ہے جس کی Torii آڈیٹرز کے ثبوت کی توقع اور ریکارڈ کرتا ہے۔ اسے `iroha app space-directory manifest publish` کے ذریعے بھیجیں۔
4. `irohad --sora --config path/to/config.toml --trace-config` چلائیں اور ٹریس آؤٹ پٹ کو رول آؤٹ ٹکٹ پر محفوظ کریں۔ اس سے یہ ثابت ہوتا ہے کہ نیا جیومیٹری سلگ/کورا سے پیدا ہونے والے شعبوں سے میل کھاتا ہے۔
5. منشور/کیٹلاگ کی تبدیلیوں کو تعینات کرنے کے بعد لین کے کسٹم تصدیق کنندگان کو دوبارہ چلائیں۔ مستقبل کے آڈٹ کیلئے سمری JSON فائل کو ٹکٹ میں رکھیں۔

## 4۔ لاگ تقسیم کرنے کے لئے ایک بنڈل بنائیں

پیدا شدہ مینی فیسٹ اور اوورلے مرتب کریں تاکہ آپریٹرز ہر میزبان پر تشکیلات میں ترمیم کیے بغیر لینز گورننس ڈیٹا تقسیم کرسکیں۔ بنڈلر ہیلپر کاپیاں قانونی ترتیب میں ظاہر ہوتی ہیں ، `nexus.registry.cache_directory` کے لئے گورننس کیٹلاگ کا اختیاری اوورلی تیار کرتی ہیں ، اور آف لائن ٹرانسفر کے لئے ٹربال کو آؤٹ پٹ کرسکتی ہیں۔

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

نتائج:

1. `manifests/<slug>.manifest.json` - اسے فارمیٹڈ `nexus.registry.manifest_directory` میں کاپی کریں۔
2. `cache/governance_catalog.json` - اسے `nexus.registry.cache_directory` میں رکھیں۔ ہر `--module` اندراج ایک تبادلہ ماڈیول کی تعریف بن جاتا ہے ، جس سے NX-2 گورننس ماڈیول کو `config.toml` میں ترمیم کرنے کے بجائے کیشے کے اوورلے کو اپ ڈیٹ کرکے تبدیل کیا جاسکتا ہے۔
3. `summary.json` - ہیش ، اوورلے ڈیٹا ، اور آپریٹرز کے لئے ہدایات شامل ہیں۔
4. اختیاری `registry_bundle.tar.*` - ایس سی پی ، ایس 3 یا آرٹ فیکٹ ٹریکرز کے لئے تیار ہے۔

ہر توثیق کنندہ کے لئے پورے فولڈر (یا محفوظ شدہ دستاویزات) کو ہم آہنگ کریں ، اسے الگ تھلگ میزبانوں پر کھولیں ، اور Torii کو دوبارہ شروع کرنے سے پہلے راستوں کو لاگ ان کرنے کے لئے مینی فیسٹ + اوورلی کیشے کاپی کریں۔

## 5. آڈیٹرز کے لئے دھواں ٹیسٹ

Torii کو دوبارہ چلانے کے بعد ، اس بات کی تصدیق کے لئے نیا تمباکو نوشی اسسٹنٹ چلائیں کہ لین `manifest_ready=true` کو اطلاع دے رہی ہے ، کہ گیجز لینوں کی متوقع تعداد دکھا رہے ہیں ، اور یہ کہ مہر بند گیج واضح ہے۔ لینوں کو جن کے لئے ظاہر کی ضرورت ہوتی ہے ان کو غیر خالی `manifest_path` قدر واپس کرنا ہوگا۔ جب راستہ غائب ہوتا ہے تو مددگار فوری طور پر ناکام ہوجاتا ہے تاکہ ہر NX-7 کی تعیناتی میں مقام مینی فیسٹ ڈائرکٹری شامل ہو۔

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v1/sumeragi/status \
  --metrics-url https://torii.example.com/metrics \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

جب خود دستخط شدہ ماحول کی جانچ کرتے ہو تو `--insecure` شامل کریں۔ اسکرپٹ غیر صفر کوڈ کے ساتھ باہر نکلتا ہے اگر لین غائب ہے یا مہر لگا ہوا ہے یا اگر پیرامیٹرز/پیمائش متوقع اقدار سے انحراف کرتی ہے۔ `--min-block-height` ، Nexus ، `--max-settlement-backlog` ، `--max-headroom-events` کو آپریٹنگ حدود میں فی لین پیمائش (بلاک/فائنل/بیکلاگ/ہیڈ روم اونچائی) کے ساتھ `--max-slot-p95`/I18NII کے ساتھ باہمی تعلق رکھنے کے لئے `--max-headroom-events` استعمال کریں۔ NX-18 میں اسسٹنٹ کو چھوڑنے کے بغیر سلاٹ مدت کے اہداف کو نافذ کریں۔

ایئر گیپڈ (یا سی آئی) چیکوں کے ل you آپ براہ راست اختتامی نقطہ تک رسائی کے بجائے پکڑے گئے Torii جواب کو دوبارہ چل سکتے ہیں:

```bash
scripts/nexus_lane_smoke.py \
  --status-file fixtures/nexus/lanes/status_ready.json \
  --metrics-file fixtures/nexus/lanes/metrics_ready.prom \
  --lane-alias core \
  --lane-alias payments \
  --expected-lane-count 3 \
  --min-da-quorum 0.95 \
  --max-oracle-staleness 75 \
  --expected-oracle-twap 60 \
  --oracle-twap-tolerance 5 \
  --max-oracle-haircut-bps 75 \
  --min-settlement-buffer 0.25 \
  --min-block-height 1000 \
  --max-finality-lag 4 \
  --max-settlement-backlog 0.5 \
  --max-headroom-events 0 \
  --max-slot-p95 1000 \
  --max-slot-p99 1100 \
  --min-slot-samples 10
```

`fixtures/nexus/lanes/` کے تحت رجسٹرڈ فکسچر بوٹسٹریپ ہیلپر کے ذریعہ تیار کردہ نوادرات کی عکاسی کرتے ہیں تاکہ نئے لنٹ کے منشور کسٹم اسکرپٹ کے بغیر لنٹ ہوسکتے ہیں۔ CI اسی بہاؤ کو `ci/check_nexus_lane_smoke.sh` اور `ci/check_nexus_lane_registry_bundle.sh` (عرف: `make check-nexus-lanes`) کے ذریعے چلاتا ہے تاکہ یہ ثابت کیا جاسکے کہ NX-7 کا دھواں پلگ ان شائع شدہ تنخواہوں کی شکل کے ساتھ مطابقت رکھتا ہے اور اس بات کو یقینی بنانے کے لئے کہ بنڈل کے ہضم/اوورلیز کو دوبارہ پیدا کیا جاسکتا ہے۔