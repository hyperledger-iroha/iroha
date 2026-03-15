---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-لچکدار لین
عنوان: لچکدار ریلیز لین (NX-7)
سائڈبار_لیبل: لچکدار ہائی لائٹ لین
تفصیل: لین Nexus ظاہر ، ڈائرکٹری اندراجات اور رول آؤٹ ثبوت بنانے کے لئے بوٹسٹریپ عمل۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_elastic_lane.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں ہم آہنگ رکھیں جب تک کہ منتقلی کی لہر پورٹل سے ٹکرا نہ ہوجائے۔
:::

# لین لچکدار سلیکشن ٹول کٹ (NX-7)

> ** روڈ میپ آئٹم: ** NX -7 - لین لچکدار انتخاب کے اوزار  
> ** حیثیت: ** ٹولز مکمل - مینی فیسٹ ، ڈائریکٹری کے ٹکڑوں ، Norito پے لوڈ ، دھواں ٹیسٹ ، پیدا کرنا ،
> اور لوڈ ٹیسٹ کے بنڈل کے لئے مددگار اب سلاٹ لیٹینسی گیٹنگ + پروف کو لنک کرتا ہے۔
> کسٹم اسکرپٹ کے بغیر شائع کرنا ممکن تھا۔

یہ گائیڈ آپریٹرز کو نئے مددگار `scripts/nexus_lane_bootstrap.sh` کے ذریعے چلتا ہے ، جو لین کے منشور ، لین/ڈیٹا اسپیس ڈائرکٹری کے ٹکڑوں ، اور رول آؤٹ ثبوتوں کی نسل کو خود کار کرتا ہے۔ مقصد یہ ہے کہ دستی طور پر متعدد فائلوں میں ترمیم کیے بغیر اور ڈائریکٹری جیومیٹری کو دستی طور پر دوبارہ گنتی کیے بغیر آسانی سے نئی Nexus لین (عوامی یا نجی) کو آسانی سے بڑھانا ہے۔

## 1. شرائط

1. عرف لین ، ڈیٹا اسپیس ، جائزوں کا سیٹ ، غلطی رواداری (`f`) اور تصفیہ پالیسی کے لئے گورننس کی منظوری۔
2. جائز کاروں (اکاؤنٹ IDs) کی آخری فہرست اور محفوظ نام کی جگہوں کی ایک فہرست۔
3. پیدا شدہ ٹکڑوں کو شامل کرنے کے لئے نوڈ کنفیگریشن ریپوزٹری تک رسائی حاصل کریں۔
4. لین منشور رجسٹری کے لئے راستے (`nexus.registry.manifest_directory` اور `cache_directory` دیکھیں)۔
5. ٹیلی میٹری رابطے/پیجریڈی لن کے لئے ہینڈلز ، تاکہ انتباہات آن لائن کے فورا. بعد منسلک ہوجائیں۔

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
- `--dataspace-alias` اور `--dataspace-id/hash` ڈائریکٹری میں ڈیٹا اسپیس انٹری کو کنٹرول کریں (ID لین پہلے سے طے شدہ طور پر استعمال ہوتی ہے)۔
- `--validator` کو `--validators-file` سے دہرایا جاسکتا ہے یا پڑھا جاسکتا ہے۔
- `--route-instruction` / `--route-account` آؤٹ پٹ روٹنگ روٹنگ کے قواعد اندراج کے لئے تیار ہیں۔
- `--metadata key=value` (یا `--telemetry-contact/channel/runbook`) رن بک رابطے کیپچر کریں تاکہ ڈیش بورڈز صحیح مالکان کو دکھائیں۔
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` جب لین کو جدید آپریٹر کنٹرول کی ضرورت ہوتی ہے تو مینی فیسٹ میں رن ٹائم اپ گریڈ ہک شامل کریں۔
- `--encode-space-directory` خود بخود `cargo xtask space-directory encode` کال کرتا ہے۔ اگر آپ انکوڈڈ `.to` کو کسی مختلف جگہ پر رکھنا چاہتے ہیں تو `--space-directory-out` کے ساتھ استعمال کریں۔

اسکرپٹ `--output-dir` (موجودہ ڈائرکٹری کے مطابق موجودہ ڈائرکٹری) میں تین نمونے تیار کرتا ہے اور انکوڈنگ کے ساتھ اختیاری طور پر چوتھا:1. `<slug>.manifest.json` - لین ایک کورم کے ساتھ عکاسی کرنے والوں ، محفوظ نام کی جگہوں اور اختیاری رن ٹائم اپ گریڈ ہک میٹا ڈیٹا کے ساتھ ظاہر ہوتی ہے۔
2. `<slug>.catalog.toml` - `[[nexus.lane_catalog]]` ، `[[nexus.dataspace_catalog]]` اور درخواست کردہ روٹنگ کے قواعد کے ساتھ ٹومل ٹکڑا۔ اس بات کو یقینی بنائیں کہ `fault_tolerance` لین ریلے کمیٹی (`3f+1`) کے سائز کے لئے ڈیٹاسپیس انٹری میں متعین کیا گیا ہے۔
3. `<slug>.summary.json` - جیومیٹری (سلگ ، طبقات ، میٹا ڈیٹا) کے ساتھ آڈٹ کا خلاصہ ، مطلوبہ رول آؤٹ اقدامات اور عین مطابق کمانڈ `cargo xtask space-directory encode` (`space_directory_encode.command` کے تحت)۔ اس JSON کو بطور ثبوت آن بورڈنگ ٹکٹ سے منسلک کریں۔
4. `<slug>.manifest.to` - ظاہر ہوتا ہے جب `--encode-space-directory` ؛ تھریڈ Torii `iroha app space-directory manifest publish` کے لئے تیار ہے۔

فائلوں کو لکھے بغیر JSON/ٹکڑوں کو دیکھنے کے لئے `--dry-run` کا استعمال کریں ، اور موجودہ نمونے کو اوور رائٹ کرنے کے لئے `--force`۔

## 3. تبدیلیوں کا اطلاق کریں

1. منشور JSON کو تشکیل شدہ `nexus.registry.manifest_directory` (اور اگر رجسٹری ریموٹ بنڈلوں کا آئینہ دار ہے تو کیش ڈائرکٹری میں) میں کاپی کریں۔ فائل کا ارتکاب کریں اگر منشور کو کنفیگریشن ریپوزٹری میں تیار کیا جائے۔
2. `config/config.toml` (یا مناسب `config.d/*.toml`) میں ڈائریکٹری کے ٹکڑے کو شامل کریں۔ اس بات کو یقینی بنائیں کہ `nexus.lane_count` `lane_id + 1` سے کم نہیں ہے ، اور `nexus.routing_policy.rules` کو اپ ڈیٹ کریں ، جو نئی لین کی طرف اشارہ کرنا چاہئے۔
3. انکوڈ (اگر آپ کو `--encode-space-directory` یاد آرہا ہے) اور خلاصہ (`space_directory_encode.command`) سے کمانڈ کا استعمال کرتے ہوئے خلائی ڈائرکٹری میں ظاہر کریں۔ اس سے `.manifest.to` پیدا ہوتا ہے ، جو Torii کی توقع کرتا ہے ، اور آڈیٹرز کے لئے ثبوت حاصل کرتا ہے۔ `iroha app space-directory manifest publish` کے ذریعے بھیجیں۔
4. `irohad --sora --config path/to/config.toml --trace-config` چلائیں اور رول آؤٹ ٹکٹ میں ٹریس آؤٹ پٹ کو محفوظ کریں۔ اس سے تصدیق ہوتی ہے کہ نیا جیومیٹری تیار کردہ سلگ/کورا طبقات سے میل کھاتا ہے۔
5. منشور/ڈائریکٹری میں تبدیلیوں کی تعیناتی کے بعد لین کو تفویض کردہ توثیق کرنے والوں کو دوبارہ شروع کریں۔ مستقبل کے آڈٹ کے لئے ٹکٹ میں خلاصہ JSON کو محفوظ کریں۔

## 4. رجسٹری ڈسٹری بیوشن بنڈل بنائیں

پیدا شدہ مینی فیسٹ اور اوورلے کو پیکج کریں تاکہ آپریٹرز ہر میزبان پر تشکیلات میں ترمیم کیے بغیر لین میں گورننس ڈیٹا تقسیم کرسکیں۔ پیکیجنگ ہیلپر کاپیاں کیننیکل لے آؤٹ پر ظاہر ہوتی ہیں ، `nexus.registry.cache_directory` کے لئے اختیاری گورننس کیٹلاگ اوورلی کی تشکیل کرتی ہیں اور آف لائن ٹرانسفر کے لئے ٹربال کو جمع کرسکتی ہیں۔

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

نتائج:

1. `manifests/<slug>.manifest.json` - ان کو تشکیل شدہ `nexus.registry.manifest_directory` میں کاپی کریں۔
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` میں ڈالیں۔ ہر `--module` اندراج ایک پلگ ایبل ماڈیول تعریف بن جاتا ہے ، جس سے گورننس ماڈیول (NX-2) کو `config.toml` میں ترمیم کرنے کے بجائے کیشے کے اوورلے کو اپ ڈیٹ کرکے تبدیل کیا جاسکتا ہے۔
3. `summary.json` - ہیشس ، اوورلے میٹا ڈیٹا اور آپریٹرز کے لئے ہدایات پر مشتمل ہے۔
4. اختیاری `registry_bundle.tar.*` - ایس سی پی ، ایس 3 یا نمونے والے ٹریکروں کے لئے تیار ہے۔

مکمل ڈائریکٹری (یا محفوظ شدہ دستاویزات) کے مطابق ، ہوا سے چلنے والے میزبانوں پر نکالیں اور Torii کو دوبارہ شروع کرنے سے پہلے ان کی رجسٹری کے راستوں پر منشور + کیشے کی کاپی کریں۔

## 5. تمباکو نوشی کرنے والے سگریٹ نوشی کرنے والےTorii کو دوبارہ شروع کرنے کے بعد ، ایک نیا دھواں مددگار چلائیں تاکہ یہ معلوم کیا جاسکے کہ لین کی اطلاع ہے `manifest_ready=true` ، میٹرکس میں دکھایا گیا ہے کہ لینوں کی متوقع تعداد اور گیج پر مہر لگا ہوا ہے۔ لینوں کو جن کے ظاہر ہونے کی ضرورت ہے ان میں خالی `manifest_path` ہونا ضروری ہے۔ اگر کوئی راستہ نہیں ہے تو ہیلپر اب فوری طور پر گر کر تباہ ہوجاتا ہے ، تاکہ ہر NX-7 تعیناتی میں دستخط شدہ مینی فیسٹ کا ثبوت شامل ہو:

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

جب خود دستخط شدہ ماحول کی جانچ کرتے ہو تو `--insecure` شامل کریں۔ اسکرپٹ غیر صفر کوڈ کے ساتھ باہر نکلتا ہے اگر لین غائب ہے ، مہر لگا ہوا ہے ، مہر لگا ہوا ہے ، یا متوقع اقدار سے میٹرکس/ٹیلی میٹری ڈائیورج ہے۔ `--min-block-height` ، Nexus ، `--max-settlement-backlog` اور `--max-headroom-events` لین ٹیلی میٹری (بلاک اونچائی/فائنلٹی/بیکلاگ/ہیڈ روم) کو قابل قبول حدود میں رکھنے کے لئے استعمال کریں ، اور Torii/i1875x/i18nii) NX-18 سلاٹ مدت کے اہداف کو پورا کرنے کے لئے مددگار کو چھوڑ کر۔

ہوا سے چلنے والے چیکوں (یا CI) کے ل you ، آپ براہ راست اختتامی نقطہ پر کال کرنے کے بجائے محفوظ شدہ جواب Torii کھیل سکتے ہیں:

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

`fixtures/nexus/lanes/` میں ریکارڈ شدہ فکسچر بوٹسٹریپ ہیلپر کے ذریعہ تیار کردہ نمونے کی عکاسی کرتے ہیں تاکہ نئے منشور کو کسٹم اسکرپٹ کے بغیر لنٹ کیا جاسکے۔ CI اسی تھریڈ کو `ci/check_nexus_lane_smoke.sh` اور `ci/check_nexus_lane_registry_bundle.sh` (عرف: `make check-nexus-lanes`) کے ذریعے چلاتا ہے تاکہ یہ یقینی بنایا جاسکے کہ NX-7 دھواں مددگار شائع شدہ پے لوڈ فارمیٹ کے ساتھ مطابقت رکھتا ہے اور یہ کہ ہضم/اوورلیز بنڈل دوبارہ پیدا ہوتا ہے۔