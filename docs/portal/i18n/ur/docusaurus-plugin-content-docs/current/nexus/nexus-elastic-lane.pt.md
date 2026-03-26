---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-elastic-lane.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-لچکدار لین
عنوان: لچکدار لین کی فراہمی (NX-7)
سائڈبار_لیبل: لچکدار لین کی فراہمی
تفصیل: Nexus لین منشور ، کیٹلاگ اندراجات اور رول آؤٹ ثبوت بنانے کے لئے بوٹسٹریپ فلو۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_elastic_lane.md` کا آئینہ دار ہے۔ دونوں کاپیاں سیدھے رکھیں جب تک کہ مقام کا سویپ پورٹل تک نہ پہنچے۔
:::

# لچکدار لین کی فراہمی کٹ (NX-7)

> ** روڈ میپ آئٹم: ** NX -7 - لچکدار لین کی فراہمی کا ٹولنگ  
> ** حیثیت: ** مکمل ٹولنگ - ظاہر ہوتا ہے ، کیٹلاگ کے ٹکڑوں ، Norito پے لوڈ ، دھواں ٹیسٹ ،
> اور بوجھ ٹیسٹ کے بنڈل مددگار اب لیٹینسی گیٹنگ میں سلاٹ + شواہد کو سلائی کرتا ہے تاکہ توثیق کنندہ لوڈ راؤنڈز
> کسٹم اسکرپٹنگ کے بغیر شائع کیا جاسکتا ہے۔

یہ گائیڈ آپریٹرز کو نئے `scripts/nexus_lane_bootstrap.sh` مددگار کے ذریعے چلتا ہے جو لین کے منشور ، لین/ڈیٹا اسپیس کاتلاگ کے ٹکڑوں اور رول آؤٹ شواہد کی نسل کو خود کار کرتا ہے۔ مقصد یہ ہے کہ دستی طور پر متعدد فائلوں میں ترمیم کیے بغیر یا دوبارہ ماخوذ کیٹلاگ دستی طور پر تیار کیے بغیر نئی Nexus لین (عوامی یا نجی) کی تشکیل میں آسانی ہو۔

## 1. شرائط

1. لین عرف ، ڈیٹاسپیس ، ویلڈیٹر سیٹ ، فالٹ رواداری (`f`) اور تصفیہ پالیسی کے لئے گورننس کی منظوری۔
2. جائزوں کی ایک حتمی فہرست (اکاؤنٹ IDs) اور محفوظ نام کی جگہوں کی ایک فہرست۔
3. نوڈ کنفیگریشن ریپوزٹری تک رسائی پیدا شدہ ٹکڑوں کو جوڑنے کے قابل ہو۔
4. لین منشور کو رجسٹر کرنے کے راستے (`nexus.registry.manifest_directory` اور `cache_directory` دیکھیں)۔
5. لین کے ل Pa پیجریڈی ٹیلی میٹری رابطے/ہینڈلز تاکہ لین آن لائن ہونے کے ساتھ ہی انتباہات منسلک ہوجائیں۔

## 2. لین نمونے تیار کریں

ذخیرہ جڑ سے مددگار چلائیں:

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

کلیدی جھنڈے:

- `--lane-id` کو `nexus.lane_catalog` میں نئی ​​اندراج کے انڈیکس سے ملنا چاہئے۔
- `--dataspace-alias` اور `--dataspace-id/hash` ڈیٹا اسپیس کیٹلاگ انٹری کو کنٹرول کریں (بطور ڈیفالٹ لین ID استعمال کرتا ہے)۔
- `--validator` کو `--validators-file` سے دہرایا جاسکتا ہے یا پڑھا جاسکتا ہے۔
-`--route-instruction` / `--route-account` آؤٹ پٹ تیار کرنے کے لئے تیار روٹنگ کے قواعد۔
- `--metadata key=value` (یا `--telemetry-contact/channel/runbook`) رن بک رابطوں کو اپنی گرفت میں لے لیتا ہے تاکہ ڈیش بورڈز صحیح مالکان کو دکھائیں۔
- `--allow-runtime-upgrades` + `--runtime-upgrade-*` جب لین میں توسیعی آپریٹر کنٹرول کی ضرورت ہوتی ہے تو ظاہر ہونے میں رن ٹائم اپ گریڈ ہک شامل کریں۔
- `--encode-space-directory` `cargo xtask space-directory encode` خود بخود کال کرتا ہے۔ جب آپ Nexus انکوڈڈ فائل کو پہلے سے طے شدہ کے علاوہ کہیں اور جانے کے لئے چاہتے ہیں تو `--space-directory-out` کے ساتھ یکجا کریں۔

اسکرپٹ `--output-dir` (موجودہ ڈائرکٹری کے مطابق) کے اندر تین نمونے تیار کرتا ہے ، اور جب انکوڈنگ کو فعال کیا جاتا ہے تو اختیاری چوتھا:1. `<slug>.manifest.json` - لین مینی فیسٹ جس میں رن ٹائم اپ گریڈ ہک سے تصدیق کنندگان ، محفوظ نام کی جگہوں اور اختیاری میٹا ڈیٹا پر مشتمل ہے۔
2. `<slug>.catalog.toml` - `[[nexus.lane_catalog]]` ، `[[nexus.dataspace_catalog]]` ، اور کسی بھی درخواست کردہ روٹنگ کے قواعد کے ساتھ ایک ٹومل اسنیپٹ۔ اس بات کو یقینی بنائیں کہ `fault_tolerance` لین ریلے کمیٹی (`3f+1`) کے سائز کے لئے ڈیٹا اسپیس انٹری میں بیان کیا گیا ہے۔
3. `<slug>.summary.json` - جیومیٹری (سلگ ، طبقات ، میٹا ڈیٹا) کے علاوہ مطلوبہ رول آؤٹ اقدامات اور `cargo xtask space-directory encode` (`space_directory_encode.command` میں) کی عین مطابق کمانڈ کی وضاحت کرنے والی آڈٹ سمری۔ اس JSON کو بطور ثبوت آن بورڈنگ ٹکٹ سے منسلک کریں۔
4. `<slug>.manifest.to` - جاری کیا گیا جب `--encode-space-directory` فعال ہو ؛ Torii سے `iroha app space-directory manifest publish` کو اسٹریم کرنے کے لئے تیار ہے۔

موجودہ نمونے کو اوور رائٹ کرنے کے لئے فائلیں لکھے بغیر JSON/ٹکڑوں کو دیکھنے کے لئے `--dry-run` کا استعمال کریں۔

## 3. تبدیلیوں کا اطلاق کریں

1. JSON منشور کو تشکیل شدہ `nexus.registry.manifest_directory` (اور اگر رجسٹری ریموٹ بنڈلوں کا آئینہ دار ہے تو کیش ڈائرکٹری میں) پر کاپی کریں۔ اگر آپ کے کنفیگریشن ریپوزٹری میں ظاہر ہوتا ہے تو فائل کا ارتکاب کریں۔
2. `config/config.toml` (یا مناسب `config.d/*.toml`) سے کیٹلاگ کے ٹکڑوں کو منسلک کریں۔ اس بات کو یقینی بنائیں کہ `nexus.lane_count` کم از کم `lane_id + 1` ہے اور کسی بھی `nexus.routing_policy.rules` کو اپ ڈیٹ کریں جو نئی لین کی طرف اشارہ کرے۔
3. انکوڈ (اگر آپ نے `--encode-space-directory` کو چھوڑ دیا ہے) اور خلاصہ (`space_directory_encode.command`) میں حاصل کردہ کمانڈ کا استعمال کرتے ہوئے خلائی ڈائرکٹری میں ظاہر کریں۔ اس سے `.manifest.to` پے لوڈ تیار ہوتا ہے جس کی Torii آڈیٹرز کے لئے توقع کرتا ہے اور ثبوت ریکارڈ کرتا ہے۔ `iroha app space-directory manifest publish` کے ذریعے بھیجیں۔
4. `irohad --sora --config path/to/config.toml --trace-config` چلائیں اور رول آؤٹ ٹکٹ میں ٹریس آؤٹ پٹ فائل کریں۔ اس سے یہ ثابت ہوتا ہے کہ نیا جیومیٹری تیار کردہ کورا سلگ/طبقات سے میل کھاتا ہے۔
5. جب منشور/کیٹلاگ میں تبدیلیوں کو تعینات کیا جاتا ہے تو لین کو تفویض کردہ توثیق کاروں کو دوبارہ شروع کریں۔ مستقبل کے آڈٹ کے لئے JSON کا خلاصہ ٹکٹ میں رکھیں۔

## 4. رجسٹری کی تقسیم کا بنڈل بنائیں

پیدا شدہ مینی فیسٹ اور اوورلے کو پیکج کریں تاکہ آپریٹرز ہر میزبان پر تشکیلات میں ترمیم کیے بغیر لین گورننس ڈیٹا تقسیم کرسکیں۔ بنڈلنگ مددگار کاپیاں کیننیکل لے آؤٹ پر ظاہر ہوتی ہیں ، `nexus.registry.cache_directory` کے لئے ایک اختیاری گورننس کیٹلاگ اوورلی تیار کرتی ہیں ، اور آف لائن ٹرانسفر کے لئے ٹربال کو آؤٹ پٹ کرسکتی ہیں:

```bash
scripts/nexus_lane_registry_bundle.sh \
  --manifest artifacts/nexus/payments_lane/payments.manifest.json \
  --output-dir artifacts/nexus/payments_lane/registry_bundle \
  --default-module parliament \
  --module name=parliament,module_type=parliament,param.quorum=2 \
  --bundle-out artifacts/nexus/payments_lane/registry_bundle.tar.gz
```

نتائج:

1. `manifests/<slug>.manifest.json` - ان فائلوں کو تشکیل شدہ `nexus.registry.manifest_directory` میں کاپی کریں۔
2. `cache/governance_catalog.json` - `nexus.registry.cache_directory` میں ڈالیں۔ `config.toml` میں ترمیم کرنے کے بجائے کیشے کے اوورلے کو اپ ڈیٹ کرتے وقت ہر `--module` اندراج ایک پلگ ایبل ماڈیول تعریف بن جاتا ہے ، جس سے گورننس ماڈیول (NX-2) تبادلہ ہوتا ہے۔
3. `summary.json` - ہیشس ، اوورلے میٹا ڈیٹا اور آپریٹرز کے لئے ہدایات شامل ہیں۔
4. اختیاری `registry_bundle.tar.*` - ایس سی پی ، ایس 3 یا نمونے والے ٹریکروں کے لئے تیار ہے۔

ہر ایک توثیق کنندہ کے لئے پوری ڈائریکٹری (یا فائل) کو مطابقت پذیر کریں ، ہوا سے چلنے والے میزبانوں کو نکالیں اور Torii کو دوبارہ شروع کرنے سے پہلے ان کی رجسٹری کے راستوں پر منشور + کیشے کے اوورلے کو کاپی کریں۔## 5. توثیق کرنے والوں کے تمباکو نوشی کے ٹیسٹ

Torii دوبارہ شروع ہونے کے بعد ، اس بات کی تصدیق کے لئے نیا تمباکو نوشی مددگار چلائیں کہ لین `manifest_ready=true` کی اطلاع دیتا ہے ، کہ میٹرکس متوقع لین کی گنتی کو بے نقاب کرتی ہے ، اور یہ کہ مہر بند گیج واضح ہے۔ لینوں کو جن کو ظاہر کرنے کی ضرورت ہوتی ہے ان کو غیر خالی `manifest_path` کو بے نقاب کرنا چاہئے۔ جب راستہ غائب ہوتا ہے تو مددگار فوری طور پر ناکام ہوجاتا ہے تاکہ ہر NX-7 تعیناتی میں دستخط شدہ مینی فیسٹ کے ثبوت شامل ہوں:

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

جب خود دستخط شدہ ماحول کی جانچ کرتے ہو تو `--insecure` شامل کریں۔ اسکرپٹ غیر صفر کوڈ کے ساتھ باہر نکلتا ہے اگر لین غیر حاضر ہے ، مہر بند ہے یا اگر میٹرکس/ٹیلی میٹری متوقع اقدار سے ہٹ جاتی ہے۔ `--min-block-height` ، `--max-finality-lag` ، `--max-settlement-backlog` اور `--max-headroom-events` KNOBS اپنے آپریشنل حدود میں فی لین ٹیلی میٹری (بلاک اونچائی/مقصد/بیکلاگ/ہیڈ روم) کو `--max-slot-p95`/I188NI کے ساتھ جوڑیں `--min-slot-samples`) NX-18 سلاٹ لائف ٹائم اہداف کو نافذ کرنے کے لئے مددگار کو چھوڑے بغیر۔

ہوا سے چلنے والے (یا CI) کی توثیق کے ل you آپ براہ راست اختتامی نقطہ تک رسائی کے بجائے پکڑے گئے Torii ردعمل کو دوبارہ چلا سکتے ہیں:

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

`fixtures/nexus/lanes/` پر لکھے گئے فکسچر بوٹسٹریپ ہیلپر کے ذریعہ تیار کردہ نمونے کی عکاسی کرتے ہیں تاکہ نئے منشور کو کسٹم اسکرپٹ کے بغیر تشکیل دیا جاسکے۔ CI اسی بہاؤ کو `ci/check_nexus_lane_smoke.sh` اور `ci/check_nexus_lane_registry_bundle.sh` (عرف: `make check-nexus-lanes`) کے ذریعے چلاتا ہے تاکہ یہ ثابت کیا جاسکے کہ NX-7 دھواں مددگار شائع شدہ تنخواہوں کی شکل کے ساتھ مطابقت رکھتا ہے اور اس بات کو یقینی بناتا ہے کہ بنڈل ڈائجسٹ/اوورلیز دوبارہ پیدا ہونے والے ہیں۔