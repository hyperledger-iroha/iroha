---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-لچکدار لین
عنوان: لچکدار لین کی فراہمی (NX-7)
سائڈبار_لیبل: لچکدار لین کی فراہمی
تفصیل: لین Nexus ظاہر کرنے کے لئے بوٹسٹریپ فلو ، کیٹلاگ اندراجات اور رول آؤٹ ثبوت۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_elastic_lane.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں سیدھے رکھیں جب تک کہ مقام کا سویپ پورٹل تک نہ پہنچے۔
:::

# لچکدار لین کی فراہمی کٹ (NX-7)

> ** روڈ میپ عنصر: ** NX -7 - لچکدار لین کی فراہمی کا ٹولنگ  
> ** حیثیت: ** ٹولنگ مکمل - منشور پیدا کرتا ہے ، کیٹلاگ کے ٹکڑوں ، Norito پے لوڈ ، دھواں ٹیسٹ ،
> اور بوجھ ٹیسٹ کے بنڈل مددگار اب لیٹینسی گیٹنگ کو یکجا کرتا ہے۔
> کسٹم اسکرپٹنگ کے بغیر شائع ہوتا ہے۔

یہ گائیڈ آپریٹرز کو نئے `scripts/nexus_lane_bootstrap.sh` مددگار کے ذریعے لے جاتا ہے جو لین کے منشور ، لین/ڈیٹا اسپیس کیٹلاگ کے ٹکڑوں ، اور رول آؤٹ شواہد کی نسل کو خود کار کرتا ہے۔ مقصد یہ ہے کہ ہاتھ سے متعدد فائلوں میں ترمیم کیے بغیر یا ہاتھ سے کیٹلاگ کی جیومیٹری کو دوبارہ ڈیریٹ کیے بغیر نئی Nexus لین (عوامی یا نجی) کی رجسٹریشن کی سہولت فراہم کی جائے۔

## 1. شرائط

1. لین عرف ، ڈیٹاسپیس ، ویلڈیٹر سیٹ ، فالٹ رواداری (`f`) اور تصفیہ پالیسی کے لئے گورننس کی منظوری۔
2. جائزوں کی ایک حتمی فہرست (اکاؤنٹ IDs) اور محفوظ نام کی جگہوں کی ایک فہرست۔
3. نوڈ کنفیگریشن ریپوزٹری تک رسائی پیدا شدہ ٹکڑوں کو جوڑنے کے قابل ہو۔
4. لین مینی فیسٹ رجسٹریشن کے راستے (`nexus.registry.manifest_directory` اور `cache_directory` دیکھیں)۔
5. لین کے لئے پیجریڈی ٹیلی میٹری رابطے/ہینڈلز ، تاکہ لین آن لائن ہونے کے ساتھ ہی انتباہات منسلک ہوجائیں۔

## 2. لین نمونے تیار کریں

ذخیرہ کی جڑ سے مددگار چلائیں:

```bash
scripts/nexus_lane_bootstrap.sh \
  --lane-alias "Payments Lane" \
  --lane-id 3 \
  --dataspace-alias payments \
  --governance-module parliament \
  --settlement-handle xor_global \
  --validator i105... \
  --validator i105... \
  --validator i105... \
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
- `--dataspace-alias` اور `--dataspace-id/hash` ڈیٹا اسپیس کیٹلاگ انٹری کو کنٹرول کریں (جب ڈیفالٹ کو چھوڑ دیا جاتا ہے تو لین ID استعمال کرتا ہے)۔
- `--validator` کو `--validators-file` سے دہرایا جاسکتا ہے یا پڑھا جاسکتا ہے۔
- `--route-instruction` / `--route-account` آؤٹ پٹ روٹنگ روٹنگ کے قواعد پیسٹ کرنے کے لئے تیار ہیں۔
- `--metadata key=value` (یا `--telemetry-contact/channel/runbook`) رن بک سے رابطوں پر قبضہ کریں تاکہ ڈیش بورڈز صحیح مالکان دکھائیں۔
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` جب لین میں توسیعی آپریٹر کنٹرول کی ضرورت ہوتی ہے تو رن ٹائم اپ گریڈ ہک کو ظاہر میں شامل کریں۔
- `--encode-space-directory` `cargo xtask space-directory encode` خود بخود طلب کرتا ہے۔ جب آپ انکوڈڈ `.to` فائل کو پہلے سے طے شدہ کے علاوہ کہیں اور جانے کے لئے `--space-directory-out` کے ساتھ جوڑیں۔

اسکرپٹ `--output-dir` (موجودہ ڈائرکٹری کے مطابق) کے اندر تین نمونے تیار کرتا ہے ، اور جب انکوڈنگ کو فعال کیا جاتا ہے تو اختیاری چوتھا:1. `<slug>.manifest.json` - لین مینی فیسٹ جس میں ویلیویٹر کورم ، محفوظ نام کی جگہیں اور اختیاری رن ٹائم اپ گریڈ ہک میٹا ڈیٹا پر مشتمل ہے۔
2. `<slug>.catalog.toml` - `[[nexus.lane_catalog]]` ، `[[nexus.dataspace_catalog]]` اور کسی بھی درخواست کردہ روٹنگ کے قواعد کے ساتھ ایک ٹومل اسنیپٹ۔ اس بات کو یقینی بنائیں کہ `fault_tolerance` لین ریلے کمیٹی (`3f+1`) کے سائز کے لئے ڈیٹاسپیس انٹری میں تشکیل دیا گیا ہے۔
3. `<slug>.summary.json` - جیومیٹری (سلگ ، طبقات ، میٹا ڈیٹا) کے علاوہ مطلوبہ رول آؤٹ اقدامات اور `cargo xtask space-directory encode` (`space_directory_encode.command` کے تحت) کی عین مطابق کمانڈ کی وضاحت کرنے والی آڈٹ سمری۔ اس JSON کو بطور ثبوت آن بورڈنگ ٹکٹ سے منسلک کریں۔
4. `<slug>.manifest.to` - جب `--encode-space-directory` کو چالو کیا جاتا ہے تو جاری کیا جاتا ہے۔ Torii سے بہاؤ `iroha app space-directory manifest publish` کے لئے تیار ہے۔

فائلوں کو لکھے بغیر JSON/ٹکڑوں کا پیش نظارہ کرنے کے لئے `--dry-run` کا استعمال کریں ، اور موجودہ نمونے کو اوور رائٹ کرنے کے لئے `--force`۔

## 3. تبدیلیوں کا اطلاق کریں

1. منشور JSON کو تشکیل شدہ `nexus.registry.manifest_directory` (اور اگر رجسٹری ریموٹ بنڈل کی عکاسی کرتا ہے تو کیش ڈائرکٹری میں) میں کاپی کریں۔ اگر آپ کے کنفیگریشن ریپو میں منشور کی شکل دی گئی ہے تو فائل کا ارتکاب کریں۔
2. `config/config.toml` (یا اسی طرح کے `config.d/*.toml`) میں کیٹلاگ کے ٹکڑوں کو شامل کریں۔ اس بات کو یقینی بناتا ہے کہ `nexus.lane_count` کم از کم `lane_id + 1` ہے اور کسی بھی `nexus.routing_policy.rules` کو اپ ڈیٹ کرتا ہے جو نئی لین کی طرف اشارہ کرنا چاہئے۔
3. انکوڈ (اگر آپ نے `--encode-space-directory` کو چھوڑ دیا ہے) اور خلاصہ (`space_directory_encode.command`) میں حاصل کردہ کمانڈ کا استعمال کرتے ہوئے خلائی ڈائرکٹری میں ظاہر کریں۔ اس سے پے لوڈ `.manifest.to` پیدا ہوتا ہے جس کا Torii آڈیٹرز کے لئے ثبوت کا انتظار کرتا ہے اور ریکارڈ کرتا ہے۔ `iroha app space-directory manifest publish` کے ساتھ بھیجیں۔
4. `irohad --sora --config path/to/config.toml --trace-config` چلائیں اور رول آؤٹ ٹکٹ میں ٹریس آؤٹ پٹ فائل کریں۔ اس سے یہ ثابت ہوتا ہے کہ نیا جیومیٹری تیار کردہ کورا سلگ/طبقات سے میل کھاتا ہے۔
5. جب منشور/کیٹلاگ میں تبدیلیوں کو تعینات کیا جاتا ہے تو لین کو تفویض کردہ توثیق کاروں کو دوبارہ شروع کریں۔ مستقبل کے آڈٹ کے لئے JSON کا خلاصہ ٹکٹ میں رکھیں۔

## 4. رجسٹری کی تقسیم کا بنڈل بنائیں

پیکیجز تیار کردہ مینی فیسٹ اور اوورلے تاکہ آپریٹرز ہر میزبان پر تشکیلات میں ترمیم کیے بغیر لین گورننس ڈیٹا تقسیم کرسکیں۔ بنڈلنگ مددگار کاپیاں کیننیکل لے آؤٹ میں ظاہر ہوتی ہیں ، `nexus.registry.cache_directory` کے لئے گورننس کیٹلاگ کا اختیاری اوورلی تیار کرتی ہیں ، اور آف لائن ٹرانسفر کے لئے ٹربال جاری کرسکتی ہیں:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

روانگی:

1. `manifests/<slug>.manifest.json` - ان فائلوں کو تشکیل شدہ `nexus.registry.manifest_directory` میں کاپی کریں۔
2. `cache/governance_catalog.json` - اسے `nexus.registry.cache_directory` پر چھوڑیں۔ ہر `--module` اندراج ایک پلگ ایبل ماڈیول تعریف بن جاتا ہے ، جس سے `config.toml` میں ترمیم کرنے کے بجائے کیشے کے اوورلی کو اپ ڈیٹ کرکے گورننس ماڈیول (NX-2) کے تبادلوں کو قابل بناتا ہے۔
3. `summary.json` - ہیشس ، اوورلے میٹا ڈیٹا اور آپریٹرز کے لئے ہدایات شامل ہیں۔
4. اختیاری `registry_bundle.tar.*` - ایس سی پی ، ایس 3 یا نمونے والے ٹریکروں کے لئے تیار ہے۔ہر ایک توثیق کنندہ کے ساتھ پوری ڈائریکٹری (یا فائل) کو ہم آہنگ کریں ، ہوا سے چلنے والے میزبانوں کو نکالیں ، اور Torii کو دوبارہ شروع کرنے سے پہلے ان کی رجسٹری کے راستوں پر منشور + کیشے کے اوورلے کو کاپی کریں۔

## 5. توثیق کرنے والوں کے لئے دھواں کی جانچ

Torii کو دوبارہ شروع کرنے کے بعد ، اس بات کی تصدیق کرنے کے لئے نیا تمباکو نوشی مددگار چلائیں کہ لین `manifest_ready=true` کی اطلاع دیتا ہے ، کہ میٹرکس متوقع لین کی گنتی کو ظاہر کرتا ہے ، اور یہ کہ مہر بند گیج واضح ہے۔ لینوں کو جن کو ظاہر کرنے کی ضرورت ہوتی ہے ان کو غیر خالی `manifest_path` کو بے نقاب کرنا چاہئے۔ جب مددگار مظہر کے ثبوت شامل کرنے کے لئے ہر NX-7 تعیناتی کے لئے راستہ غائب ہوتا ہے تو مددگار فوری طور پر ناکام ہوجاتا ہے۔

```bash
scripts/nexus_lane_smoke.py \
  --status-url https://torii.example.com/v2/sumeragi/status \
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

جب خود دستخط شدہ سرٹیفکیٹ کے ساتھ ماحول کی جانچ کرتے ہو تو `--insecure` شامل کریں۔ اسکرپٹ کا اختتام غیر صفر کوڈ کے ساتھ ہوتا ہے اگر لین غائب ہے ، مہر بند ہے یا میٹرکس/ٹیلی میٹری کو متوقع اقدار سے غلط استعمال کیا گیا ہے۔ لن ٹیلی میٹری (بلاک اونچائی/مقصد/بیکلاگ/ہیڈ روم) کو اپنی آپریٹنگ حدود میں رکھنے کے لئے KNOBS `--min-block-height` ، `--max-finality-lag` ، `--max-settlement-backlog` اور `--max-headroom-events` استعمال کریں ، اور ان کو `--max-slot-p95`/`--max-slot-p95`/`--max-slot-p95`/Torii استعمال کریں۔ `--min-slot-samples`) NX-18 سلاٹ مدت کے اہداف کو ہیلپر چھوڑنے کے بغیر نافذ کرنے کے لئے۔

ہوا سے چلنے والے (یا CI) کی توثیق کے ل you آپ براہ راست اختتامی نقطہ کو مارنے کے بجائے پکڑے گئے Torii جواب کو دوبارہ چل سکتے ہیں:

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

`fixtures/nexus/lanes/` کے تحت ریکارڈ کردہ فکسچر بوٹسٹریپ ہیلپر کے ذریعہ تیار کردہ نمونے کی عکاسی کرتے ہیں تاکہ نئے منشور کو کسٹم اسکرپٹ کے بغیر لنٹ پڑھا جاسکے۔ سی آئی اسی بہاؤ کو `ci/check_nexus_lane_smoke.sh` اور `ci/check_nexus_lane_registry_bundle.sh` (AKA: `make check-nexus-lanes`) کے ذریعے چلاتا ہے تاکہ یہ ظاہر کیا جاسکے کہ دھواں NX-7 مددگار شائع شدہ پے لوڈ فارمیٹ کے ساتھ منسلک رہتا ہے اور اس بات کو یقینی بنانے کے لئے کہ بنڈل ڈائجسٹ/اوورلیز دوبارہ پیدا ہونے والی ہے۔