---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-ڈیفالٹ-لین کوک اسٹارٹ
عنوان: کوئیک اسٹارٹ ڈیفالٹ لین (NX-5)
سائڈبار_لیبل: فوری طور پر ڈیفالٹ لین
تفصیل: Nexus میں فال بیک بیک ڈیفالٹ لین کی تشکیل اور چیک کریں تاکہ Torii اور SDK عوامی لینوں میں LANE_ID کو چھوڑ دے۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/quickstart/default_lane.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں مطابقت پذیری میں رکھیں جب تک کہ لوکلائزیشن رن پورٹل سے ٹکرا نہ جائے۔
:::

# کوئیک اسٹارٹ ڈیفالٹ لین (NX-5)

> ** روڈ میپ سیاق و سباق: ** NX -5 - پہلے سے طے شدہ عوامی لین کا انضمام۔ رن ٹائم اب فال بیک بیک `nexus.routing_policy.default_lane` فراہم کرتا ہے تاکہ آرام/GRPC اختتامی نکات Torii اور ہر SDK جب ٹریفک کا تعلق کینونیکل پبلک لین سے تعلق رکھتا ہو تو `lane_id` کو محفوظ طریقے سے چھوڑ سکتا ہے۔ یہ گائیڈ آپریٹرز کو ڈائریکٹری سیٹ اپ ، `/status` میں فال بیک ٹیسٹنگ ، اور کلائنٹ کے طرز عمل کی جانچ شروع سے ختم ہونے تک چلتا ہے۔

## شرائط

- `irohad` (`irohad --sora --config ...` چل رہا ہے) کے لئے Sora/Nexus بنائیں۔
- سیکشن `nexus.*` میں ترمیم کرنے کے لئے کنفیگریشن ریپوزٹری تک رسائی۔
- `iroha_cli` ہدف کلسٹر پر تشکیل شدہ۔
- `curl`/`jq` (یا مساوی) پے لوڈ `/status` کو Torii دیکھنے کے لئے۔

## 1. لین اور ڈیٹا اسپیس ڈائرکٹری کی وضاحت کریں

لین اور ڈیٹا اسپیس کی وضاحت کریں جو نیٹ ورک پر موجود ہیں۔ نیچے کا ٹکڑا (`defaults/nexus/config.toml` سے کٹ) تین عوامی لین اور ڈیٹا اسپیس کے لئے اسی طرح کے عرف رجسٹرڈ کرتا ہے:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

ہر `index` منفرد اور مستقل ہونا چاہئے۔ ID ڈیٹا اسپیس 64 بٹ اقدار ہیں۔ مذکورہ بالا مثالوں میں وہی عددی اقدار کا استعمال کیا گیا ہے جیسے واضح طور پر لین کے اشارے۔

## 2۔ روٹنگ ڈیفالٹس اور اختیاری اوور رائڈس سیٹ کریں

سیکشن `nexus.routing_policy` فال بیک لین کو کنٹرول کرتا ہے اور روٹنگ کو مخصوص ہدایات یا اکاؤنٹ کے سابقہ ​​کے لئے اوورراڈ کرنے کی اجازت دیتا ہے۔ اگر کوئی قواعد مماثل نہیں ہیں تو ، شیڈولر ٹرانزیکشن کو `default_lane` اور `default_dataspace` پر لے جاتا ہے۔ روٹر منطق `crates/iroha_core/src/queue/router.rs` میں رہتا ہے اور شفاف طور پر پالیسی کو Torii REST/GRPC سطحوں پر لاگو کرتا ہے۔

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```


## 3. لاگو پالیسی کے ساتھ ایک نوڈ لانچ کریں

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

نوڈ اسٹارٹ اپ پر حساب کتاب روٹنگ پالیسی کو لاگ ان کرتا ہے۔ گپ شپ شروع ہونے سے پہلے کسی بھی توثیق کی غلطیاں (گمشدہ اشاریہ جات ، ڈپلیکیٹ عرف ، غلط IDS ڈیٹا اسپیس) پاپ اپ ہوجاتی ہیں۔

## 4. لین کے لئے گورننس اسٹیٹ کی تصدیق کریں

ایک بار نوڈ آن لائن ہونے کے بعد ، اس بات کو یقینی بنانے کے لئے سی ایل آئی ہیلپر کا استعمال کریں کہ پہلے سے طے شدہ لین پر مہر لگا دی جائے (مینی فیسٹ بھری ہوئی) اور ٹریفک کے لئے تیار ہو۔ خلاصہ نظارہ فی لین میں ایک لائن دکھاتا ہے:

```bash
iroha_cli app nexus lane-report --summary
```

مثال کے طور پر آؤٹ پٹ:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

اگر پہلے سے طے شدہ لین `sealed` کو ظاہر کرتی ہے تو ، بیرونی ٹریفک کی اجازت دینے سے پہلے لینوں کے لئے گورننس رن بک پر عمل کریں۔ `--fail-on-sealed` پرچم CI کے لئے آسان ہے۔

## 5. اسٹیٹس پے لوڈ Torii چیک کریںجواب `/status` لینوں کے روٹنگ پالیسی اور شیڈیولر سنیپ شاٹ دونوں کو ظاہر کرتا ہے۔ تشکیل شدہ ڈیفالٹس کی تصدیق کے ل I `curl`/`jq` استعمال کریں اور اس بات کی تصدیق کریں کہ فال بیک لین ٹیلی میٹری شائع کررہی ہے:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

نمونہ آؤٹ پٹ:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

لین `0` کے لئے براہ راست شیڈولر کاؤنٹر دیکھنے کے لئے:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

اس سے تصدیق ہوتی ہے کہ ٹی ای یو اسنیپ شاٹ ، عرف میٹا ڈیٹا ، اور منشور کے جھنڈے ترتیب سے ملتے ہیں۔ اسی پے لوڈ کا استعمال ڈیش بورڈ لین آنے کے لئے Grafana پینلز کے ذریعہ کیا جاتا ہے۔

## 6. کلائنٹ کے پہلے سے طے شدہ سلوک کو چیک کریں

- ** زنگ / سی ایل آئی۔ اس معاملے میں قطار روٹر `default_lane` پر گر کر تباہ ہوتا ہے۔ واضح جھنڈے `--lane-id`/`--dataspace-id` صرف غیر ڈیفالٹ لین کے ساتھ کام کرنے پر استعمال کریں۔
- ** جے ایس/سوئفٹ/اینڈروئیڈ۔ اسٹیجنگ اور پروڈکشن کے مابین روٹنگ پالیسیاں ہم آہنگ رکھیں تاکہ موبائل ایپس کو تباہی کی تشکیل نو کی ضرورت نہ ہو۔
- ** پائپ لائن/ایس ایس ای ٹیسٹ۔ اس فلٹر کے ساتھ `/v2/pipeline/events/transactions` کو سبسکرائب کریں تاکہ یہ ثابت کیا جاسکے کہ واضح لین کے بغیر بھیجے گئے ریکارڈ فال بیک لین ID کے تحت آتے ہیں۔

## 7. مشاہدہ اور گورننس ہکس

- `/status` `nexus_lane_governance_sealed_total` اور `nexus_lane_governance_sealed_aliases` بھی شائع کرتا ہے تاکہ انتباہی مینیجر جب لین کا مظہر ہار جاتا ہے تو انتباہ کرسکتا ہے۔ ان انتباہات کو بھی ڈیونیٹ پر فعال رکھیں۔
- ٹیلی میٹری میپ شیڈیولر اور ڈیش بورڈ گورننس برائے لین (`dashboards/grafana/nexus_lanes.json`) ڈائریکٹری سے عرف/سلگ فیلڈز کی توقع کریں۔ اگر آپ عرف کا نام تبدیل کرتے ہیں تو ، اس سے متعلقہ کورا ڈائریکٹریز کا نام تبدیل کریں تاکہ آڈیٹر عین مطابق راستوں (NX-1 کے ذریعہ ٹریک کردہ) کو برقرار رکھیں۔
- پہلے سے طے شدہ لینوں کے لئے پارلیمانی منظوریوں میں رول بیک پلان شامل ہونا ضروری ہے۔ اپنے آپریٹر رن بک میں اس کوئیک اسٹارٹ کے آگے ہیش منشور اور حکمرانی کا ثبوت ریکارڈ کریں تاکہ مستقبل کی گردشوں کا اندازہ مطلوبہ حالت میں نہ لگے۔

ایک بار جب یہ چیک منظور ہوجائیں تو ، آپ `nexus.routing_policy.default_lane` پر SDK ترتیب کے لئے سچائی کا ذریعہ پر غور کرسکتے ہیں اور نیٹ ورک پر لیگیسی سنگل لین کوڈ کے راستوں کو غیر فعال کرنا شروع کر سکتے ہیں۔