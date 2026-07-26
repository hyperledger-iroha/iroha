---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-ڈیفالٹ-لین کوک اسٹارٹ
عنوان: پہلے سے طے شدہ لین کے لئے فوری رہنما (NX-5)
سائڈبار_لیبل: پہلے سے طے شدہ لین کے لئے فوری رہنما
تفصیل: Nexus کے پہلے سے طے شدہ لین فال بیک کو تشکیل اور تصدیق کریں تاکہ Torii اور SDKs عوامی لینوں میں LANE_ID کو نظرانداز کرسکیں۔
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/quickstart/default_lane.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں سیدھے رکھیں جب تک کہ مقام کا سویپ پورٹل تک نہ پہنچے۔
:::

# ڈیفالٹ لین کے لئے فوری رہنما (NX-5)

> ** روڈ میپ سیاق و سباق: ** NX -5 - پہلے سے طے شدہ عوامی لین کا انضمام۔ رن ٹائم اب ایک `nexus.routing_policy.default_lane` فال بیک کو بے نقاب کرتا ہے تاکہ Torii REST/GRPC اختتامی نکات اور ہر SDK جب ٹریفک کا تعلق کیننیکل پبلک لین سے تعلق رکھتا ہو تو `lane_id` کو بحفاظت چھوڑ سکتا ہے۔ یہ گائیڈ آپریٹرز کو کیٹلاگ کی تشکیل ، `/status` پر فال بیک کی تصدیق کرنے اور آخر سے آخر تک کلائنٹ کے طرز عمل کو استعمال کرنے میں لیتا ہے۔

## شرائط

- `irohad` (`irohad --sora --config ...` چلتا ہے) کی ایک SORA/Nexus بلڈ۔
- سیکشن `nexus.*` میں ترمیم کرنے کے لئے کنفیگریشن ریپوزٹری تک رسائی۔
- `iroha_cli` ہدف کلسٹر سے بات کرنے کے لئے تشکیل دیا گیا ہے۔
- `curl`/`jq` (یا مساوی) Torii کے پے لوڈ `/status` کا معائنہ کرنے کے لئے۔

## 1. گلیوں اور ڈیٹا اسپیس کے کیٹلاگ کی وضاحت کریں

لینوں اور ڈیٹا اسپیسوں کا اعلان کریں جو نیٹ ورک پر موجود ہیں۔ مندرجہ ذیل ٹکڑے (`defaults/nexus/config.toml` سے تیار شدہ) میں تین عوامی لینوں کے علاوہ متعلقہ ڈیٹا اسپیس عرفی ناموں کو ریکارڈ کیا گیا ہے:

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

ہر `index` منفرد اور متضاد ہونا چاہئے۔ ڈیٹا اسپیس آئی ڈی 64 بٹ اقدار ہیں۔ مذکورہ بالا مثالیں وہی عددی اقدار کا استعمال کرتی ہیں جیسے واضح طور پر لین کے اشارے۔

## 2۔ روٹنگ ڈیفالٹس اور اختیاری اوور رائڈس کی تشکیل کریں

`nexus.routing_policy` سیکشن فال بیک لین کو کنٹرول کرتا ہے اور آپ کو مخصوص ہدایات یا اکاؤنٹ کے سابقہ ​​کے لئے روٹنگ کو اوور رائڈ کرنے کی اجازت دیتا ہے۔ اگر کوئی قواعد مماثل نہیں ہیں تو ، شیڈولر ٹرانزیکشن کو تشکیل شدہ `default_lane` اور `default_dataspace` پر لے جاتا ہے۔ راؤٹر منطق `crates/iroha_core/src/queue/router.rs` پر رہتا ہے اور Torii کے باقی/GRPC سطحوں پر شفاف طور پر پالیسی کا اطلاق کرتا ہے۔

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

جب آپ بعد میں نئی ​​لینیں شامل کریں تو پہلے کیٹلاگ کو اپ ڈیٹ کریں اور پھر روٹنگ کے قواعد کو بڑھا دیں۔ فال بیک لین کو عوامی لین کی طرف اشارہ کرنا جاری رکھنا چاہئے جو زیادہ تر صارف ٹریفک کو مرکوز کرتا ہے تاکہ لیگیسی ایس ڈی کے کام جاری رکھیں۔

## 3. لاگو پالیسی کے ساتھ ایک نوڈ شروع کریں

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

نوڈ اسٹارٹ اپ کے دوران اخذ کردہ روٹنگ پالیسی کو ریکارڈ کرتا ہے۔ کسی بھی توثیق کی غلطیاں (گمشدہ اشاریہ جات ، ڈپلیکیٹ عرفی نام ، غلط ڈیٹا اسپیس آئی ڈی) گپ شپ شروع ہونے سے پہلے ہی دکھائے جاتے ہیں۔

## 4. لین کی حکمرانی کی حیثیت کی تصدیق کریںایک بار نوڈ آن لائن ہونے کے بعد ، یہ تصدیق کرنے کے لئے سی ایل آئی ہیلپر کا استعمال کریں کہ پہلے سے طے شدہ لین سیل (مینی فیسٹ بھری ہوئی) اور ٹریفک کے لئے تیار ہے۔ خلاصہ نظارہ فی لین میں ایک قطار پرنٹ کرتا ہے:

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

اگر پہلے سے طے شدہ لین `sealed` کو ظاہر کرتی ہے تو ، بیرونی ٹریفک کی اجازت دینے سے پہلے لین گورننس رن بک پر عمل کریں۔ `--fail-on-sealed` پرچم CI کے لئے مفید ہے۔

## 5. Torii کے اسٹیٹس پے لوڈ کا معائنہ کریں

`/status` ردعمل روٹنگ پالیسی اور شیڈیولر سنیپ شاٹ دونوں لین کو بے نقاب کرتا ہے۔ تشکیل شدہ ڈیفالٹ اقدار کی تصدیق کے لئے `curl`/`jq` استعمال کریں اور اس بات کی تصدیق کریں کہ فال بیک لین ٹیلی میٹری تیار کررہی ہے:

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

لین `0` کے لئے شیڈیولر کے براہ راست کاؤنٹرز کا معائنہ کرنے کے لئے:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

اس سے تصدیق ہوتی ہے کہ ٹی ای یو اسنیپ شاٹ ، عرف میٹا ڈیٹا ، اور منشور کے جھنڈے ترتیب کے ساتھ ہم آہنگ ہیں۔ وہی پے لوڈ وہی ہے جو لین-ایجسٹ ڈیش بورڈ کے لئے Grafana پینلز کے ذریعہ استعمال ہوتا ہے۔

## 6. ورزش کلائنٹ کے پہلے سے طے شدہ

- ** زنگ / سی ایل آئی۔ قطار میں قطار لگانے والا روٹر `default_lane` پر واپس آتا ہے۔ واضح جھنڈے `--lane-id`/`--dataspace-id` صرف غیر ڈیفالٹ لین کی طرف اشارہ کرتے وقت استعمال کریں۔
- ** جے ایس/سوئفٹ/اینڈروئیڈ۔ اسٹیجنگ اور پروڈکشن کے مابین روٹنگ پالیسی کو ہم آہنگ رکھیں تاکہ موبائل ایپس کو ہنگامی تشکیل نو کی ضرورت نہ ہو۔
- ** پائپ لائن/ایس ایس ای ٹیسٹ۔ اس فلٹر کے ساتھ `/v1/pipeline/events/transactions` کو سبسکرائب کریں تاکہ یہ ظاہر کیا جاسکے کہ بغیر کسی واضح لین کے فال بیک لین ID کے تحت بھیجے گئے لکھتے ہیں۔

## 7. مشاہدہ اور گورننس ہکس

- `/status` `nexus_lane_governance_sealed_total` اور `nexus_lane_governance_sealed_aliases` بھی شائع کرتا ہے تاکہ جب کوئی لین اپنا مظہر کھو دیتی ہے تو الرٹ مینجر مطلع کرسکتا ہے۔ ان انتباہات کو بھی ڈیونیٹس پر فعال رکھیں۔
- شیڈیولر ٹیلی میٹری کا نقشہ اور لین گورننس ڈیش بورڈ (`dashboards/grafana/nexus_lanes.json`) کیٹلاگ سے عرف/سلگ فیلڈز کی توقع کرتا ہے۔ اگر آپ کسی عرف کا نام تبدیل کرتے ہیں تو ، اس سے متعلقہ کورا ڈائریکٹریوں کو دوبارہ شامل کریں تاکہ آڈیٹرز عصبی راستے برقرار رکھیں (NX-1 کے تحت پیروی کریں)۔
- پہلے سے طے شدہ لینوں کے لئے پارلیمانی منظوریوں میں رول بیک پلان شامل ہونا ضروری ہے۔ اپنے آپریٹر رن بک میں اس کوئیک اسٹارٹ کے ساتھ منشور ہیش اور گورننس شواہد کو ریکارڈ کریں تاکہ آئندہ کی گردشوں کو مطلوبہ حالت کا اندازہ لگانے کی ضرورت نہ ہو۔ایک بار جب یہ چیک گزر جاتے ہیں تو آپ `nexus.routing_policy.default_lane` کا علاج SDK کنفیگریشن کے لئے سچائی کے ذریعہ کے طور پر کرسکتے ہیں اور نیٹ ورک پر لیگیسی سنگل لین کوڈ کے راستوں کو غیر فعال کرنا شروع کر سکتے ہیں۔