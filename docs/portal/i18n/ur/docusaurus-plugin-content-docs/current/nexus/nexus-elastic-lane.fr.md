---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-لچکدار لین
عنوان: لچکدار لین کی فراہمی (NX-7)
سائڈبار_لیبل: لچکدار لین کی فراہمی
تفصیل: بوٹسٹریپ ورک فلو Nexus لین منشور ، کیٹلاگ اندراجات اور رول آؤٹ ثبوت تخلیق کرنے کے لئے۔
---

::: نوٹ کینونیکل ماخذ
اس صفحے میں `docs/source/nexus_elastic_lane.md` شامل ہے۔ دونوں کاپیاں منسلک رکھیں جب تک کہ ترجمے کی لہر پورٹل سے ٹکرا نہ جائے۔
:::

# لچکدار لین کی فراہمی کٹ (NX-7)

> ** روڈ میپ کا عنصر: ** NX -7 - لچکدار لین کی فراہمی کا ٹولنگ  
> ** حیثیت: ** مکمل ٹولنگ - ظاہر ہوتا ہے ، کیٹلاگ کے ٹکڑوں ، Norito پے لوڈ ، دھواں ٹیسٹ ،
> اور بوجھ ٹیسٹ کے بنڈل مددگار اب فی سلاٹ لیٹینسی گیٹنگ + پروف کو جمع کرتا ہے تاکہ بوجھ کی توثیق کرنا چلتی ہے
> کسٹم اسکرپٹنگ کے بغیر شائع کیا جاسکتا ہے۔

یہ گائیڈ نئے مددگار `scripts/nexus_lane_bootstrap.sh` کے ذریعہ آپریٹرز کی حمایت کرتا ہے جو لین کے منشور ، لین/ڈیٹاسپیس کیٹلاگ کے ٹکڑوں اور رول آؤٹ ثبوتوں کی نسل کو خود کار کرتا ہے۔ اس کا مقصد یہ ہے کہ کئی فائلوں میں دستی طور پر ترمیم کیے بغیر یا کیٹلاگ کی جیومیٹری کو دستی طور پر دوبارہ حاصل کیے بغیر نئی Nexus لین (عوامی یا نجی) کی تخلیق میں آسانی پیدا کی جائے۔

## 1. شرائط

1. لین عرف ، ڈیٹاسپیس ، ویلڈیٹر سیٹ ، فالٹ رواداری (`f`) اور تصفیہ پالیسی کے لئے گورننس کی منظوری۔
2. جائزوں کی ایک حتمی فہرست (اکاؤنٹ IDs) اور محفوظ نام کی جگہوں کی ایک فہرست۔
3. نوڈ کنفیگریشن ریپوزٹری تک رسائی حاصل کرنے کے لئے تیار کردہ ٹکڑوں کو شامل کرنے کے ل .۔
4. لین منشور رجسٹری کے لئے راستے (`nexus.registry.manifest_directory` اور `cache_directory` دیکھیں)۔
5. ٹیلی میٹری رابطے/پیجریڈی لین کے ل hands ہینڈلز تاکہ لین آن لائن آنے کے ساتھ ہی انتباہات منسلک ہوجائیں۔

## 2. لین نمونے تیار کریں

ذخیرہ کی جڑ سے مددگار چلائیں:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --validator <katakana-i105-account-id> \
  --protected-namespace payments \
  --description "High-throughput interbank payments lane" \
  --dataspace-description "Payments dataspace" \
  --route-instruction finance::pacs008 \
  --encode-space-directory \
  --space-directory-out artifacts/nexus/payments_lane/payments.manifest.to \
  --telemetry-contact payments-ops@sora.org \
  --output-dir artifacts/nexus/payments_lane
```

کلیدی جھنڈے:

- `--lane-id` کو `nexus.lane_catalog` میں نئی ​​اندراج کے انڈیکس سے ملنا چاہئے۔
- `--dataspace-alias` اور `--dataspace-id/hash` ڈیٹا اسپیس کیٹلاگ انٹری کو کنٹرول کریں (جب کسی کو چھوڑ دیا جاتا ہے تو لین ID سے پہلے سے طے ہوتا ہے)۔
- `--validator` کو `--validators-file` سے دہرایا جاسکتا ہے یا پڑھا جاسکتا ہے۔
- `--route-instruction` / `--route-account` روٹنگ کے قواعد کو پیسٹ کرنے کے لئے تیار ہے۔
- `--metadata key=value` (یا `--telemetry-contact/channel/runbook`) رن بک رابطوں کو اپنی گرفت میں لے لیتا ہے تاکہ ڈیش بورڈز صحیح مالکان کو ظاہر کریں۔
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` جب لین میں توسیعی آپریٹر کنٹرول کی ضرورت ہوتی ہے تو رن ٹائم اپ گریڈ ہک کو ظاہر میں شامل کریں۔
- `--encode-space-directory` خود بخود `cargo xtask space-directory encode` کی درخواست کرتا ہے۔ جب آپ انکوڈڈ `.to` فائل کو پہلے سے طے شدہ راستے کے علاوہ کہیں اور جانے کے لئے `--space-directory-out` کے ساتھ جوڑیں۔

اسکرپٹ `--output-dir` (موجودہ ڈائرکٹری کے مطابق موجودہ ڈائرکٹری) میں تین نمونے تیار کرتا ہے ، نیز ایک اختیاری چوتھا جب انکوڈنگ فعال ہوتی ہے:1. `<slug>.manifest.json` - لین مینی فیسٹ جس میں ویلیویٹر کورم ، محفوظ نام کی جگہیں اور اختیاری رن ٹائم اپ گریڈ ہک میٹا ڈیٹا پر مشتمل ہے۔
2. `<slug>.catalog.toml` - `[[nexus.lane_catalog]]` ، `[[nexus.dataspace_catalog]]` اور کسی بھی درخواست کردہ روٹنگ کے قواعد کے ساتھ ایک ٹومل اسنیپٹ۔ یقینی بنائیں کہ `fault_tolerance` لین ریلے بورڈ (`3f+1`) کے سائز کے لئے ڈیٹا اسپیس انٹری پر سیٹ ہے۔
3. `<slug>.summary.json` - جیومیٹری (سلگ ، طبقات ، میٹا ڈیٹا) کے علاوہ مطلوبہ رول آؤٹ اقدامات اور عین مطابق کمانڈ `cargo xtask space-directory encode` (`space_directory_encode.command` کے تحت) کی وضاحت کرنے والی آڈٹ سمری۔ اس JSON کو بطور ثبوت آن بورڈنگ ٹکٹ سے منسلک کریں۔
4. `<slug>.manifest.to` - جاری کیا گیا جب `--encode-space-directory` فعال ہے۔ Torii سے اسٹریم `iroha app space-directory manifest publish` کے لئے تیار ہے۔

فائلوں کو لکھے بغیر JSON/ٹکڑوں کا پیش نظارہ کرنے کے لئے `--dry-run` کا استعمال کریں ، اور موجودہ نمونے کو اوور رائٹ کرنے کے لئے `--force`۔

## 3. تبدیلیوں کا اطلاق کریں

1. JSON منشور کو `nexus.registry.manifest_directory` تشکیل (اور اگر رجسٹری ریموٹ بنڈلوں کا آئینہ دار ہے تو کیش ڈائرکٹری میں) پر کاپی کریں۔ اگر آپ کے کنفیگریشن ریپو میں منشور کی شکل دی گئی ہے تو فائل کا ارتکاب کریں۔
2. `config/config.toml` (یا مناسب `config.d/*.toml`) میں کیٹلاگ کے اسنیپٹ کو شامل کریں۔ اس بات کو یقینی بنائیں کہ `nexus.lane_count` کم از کم `lane_id + 1` ہے ، اور کسی بھی قاعدہ `nexus.routing_policy.rules` کو اپ ڈیٹ کریں جس میں نئی ​​لین کی طرف اشارہ کرنا چاہئے۔
3. انکوڈ (اگر آپ نے `--encode-space-directory` کو چھوڑ دیا ہے) اور خلاصہ (`space_directory_encode.command`) میں حاصل کردہ کمانڈ کے ذریعہ خلائی ڈائرکٹری میں ظاہر کریں۔ اس سے پے لوڈ `.manifest.to` پیدا ہوتا ہے جس کی توقع Torii سے متوقع ہے اور آڈٹ کے لئے ثبوت محفوظ کرتا ہے۔ `iroha app space-directory manifest publish` کے ذریعے جمع کروائیں۔
4. `irohad --sora --config path/to/config.toml --trace-config` چلائیں اور رول آؤٹ ٹکٹ میں ٹریس آؤٹ پٹ کو محفوظ کریں۔ اس سے یہ ثابت ہوتا ہے کہ نیا جیومیٹری تیار کردہ سلگ کے کورا طبقات سے مساوی ہے۔
5. جب منشور/کیٹلاگ میں تبدیلیوں کو تعینات کیا جاتا ہے تو لین کو تفویض کردہ توثیق کاروں کو دوبارہ شروع کریں۔ مستقبل کے آڈٹ کے لئے JSON کا خلاصہ ٹکٹ میں رکھیں۔

## 4. رجسٹری کی تقسیم کا بنڈل بنائیں

پیدا شدہ مینی فیسٹ اور اوورلے کو پیکج کریں تاکہ آپریٹرز ہر میزبان پر تشکیلات میں ترمیم کیے بغیر لین گورننس ڈیٹا تقسیم کرسکیں۔ بنڈلنگ مددگار کاپیاں کیننیکل لے آؤٹ میں ظاہر ہوتی ہیں ، `nexus.registry.cache_directory` کے لئے اختیاری گورننس کیٹلاگ اوورلی تیار کرتی ہیں ، اور آف لائن ٹرانسفر کے لئے ٹربال جاری کرسکتی ہیں:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

نتائج:

1. `manifests/<slug>.manifest.json` - ان کو `nexus.registry.manifest_directory` تشکیل میں کاپی کریں۔
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` میں ڈراپ کریں۔ ہر `--module` اندراج ایک پلگ ایبل ماڈیول تعریف بن جاتا ہے ، جس سے `config.toml` میں ترمیم کرنے کے بجائے کیشے کے اوورلے کو اپ ڈیٹ کرکے گورننس ماڈیول (NX-2) تبادلہ کرنے کی اجازت دی جاتی ہے۔
3. `summary.json` - ہیش ، اوورلے میٹا ڈیٹا اور آپریٹر ہدایات شامل ہیں۔
4. اختیاری `registry_bundle.tar.*` - ایس سی پی ، ایس 3 یا نمونے والے ٹریکروں کے لئے تیار ہے۔ہر ایک توثیق کنندہ کے ساتھ پوری ڈائریکٹری (یا محفوظ شدہ دستاویزات) کو ہم آہنگ کریں ، ہوا سے چلنے والے میزبانوں کو نکالیں ، اور Torii کو دوبارہ شروع کرنے سے پہلے ان کی رجسٹری کے راستوں پر مینی فیسٹ + کیشے کے اوورلے کو کاپی کریں۔

## 5. توثیق کرنے والوں کی دھواں کی جانچ

Torii کو دوبارہ شروع کرنے کے بعد ، اس بات کی تصدیق کرنے کے لئے نیا سگریٹ نوشی مددگار چلائیں کہ لین `manifest_ready=true` کی اطلاع دے رہی ہے ، کہ میٹرکس لینوں کی متوقع تعداد کی اطلاع دے رہی ہے ، اور یہ کہ مہر بند گیج خالی ہے۔ لینوں کو جن کو ظاہر کرنے کی ضرورت ہوتی ہے ان کو غیر خالی `manifest_path` کو بے نقاب کرنا چاہئے۔ جیسے ہی راستہ غائب ہوتا ہے ، مددگار ناکام ہوجاتا ہے تاکہ ہر NX-7 تعیناتی میں دستخط شدہ مینی فیسٹ کا ثبوت شامل ہو:

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

جب خود دستخط شدہ ماحول کی جانچ کرتے ہو تو `--insecure` شامل کریں۔ اسکرپٹ غیر صفر کوڈ کے ساتھ باہر نکلتا ہے اگر لین غائب ہے ، مہر لگا دی گئی ہے ، یا اگر میٹرکس/ٹیلی میٹری متوقع اقدار سے اخذ کرتی ہے۔ نوبس `--min-block-height` ، `--max-finality-lag` ، `--max-settlement-backlog` اور `--max-headroom-events` کو اپنے آپریشنل لفافوں میں فی لین ٹیلی میٹری (بلاک اونچائی/فائنلٹی/بیکلاگ/ہیڈ روم) کو برقرار رکھنے کے لئے `--max-slot-p95`/`--max-headroom-events` استعمال کریں ، اور ان کو جوڑیں۔ `--min-slot-samples`) NX-18 سلاٹ مدت کے مقاصد کو مددگار نہیں چھوڑے۔

ایئر گیپڈ (یا سی آئی) کی توثیق کے ل you آپ براہ راست اختتامی نقطہ سے استفسار کرنے کے بجائے پکڑے گئے Torii جواب کو دوبارہ چل سکتے ہیں:

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

`fixtures/nexus/lanes/` کے تحت رجسٹرڈ فکسچر بوٹسٹریپ ہیلپر کے ذریعہ تیار کردہ نمونے کی عکاسی کرتے ہیں تاکہ نئے منشور کو کسٹم اسکرپٹ کے بغیر لنٹ کیا جاسکے۔ CI `ci/check_nexus_lane_smoke.sh` اور `ci/check_nexus_lane_registry_bundle.sh` (AKA: `make check-nexus-lanes`) کے ذریعے اسی بہاؤ کو انجام دیتا ہے تاکہ یہ ثابت کیا جاسکے کہ NX-7 دھواں مددگار شائع شدہ تنخواہوں کی شکل کے مطابق رہتا ہے اور اس بات کو یقینی بنانے کے لئے کہ بنڈل ہضم/اوورلیز دوبارہ قابل تولیدی ہے۔