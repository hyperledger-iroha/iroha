---
lang: ur
direction: rtl
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: گٹھ جوڑ-ڈیفالٹ-لین کوک اسٹارٹ
عنوان: ڈیفالٹ لین کوئیک اسٹارٹ (NX-5)
سائڈبار_لیبل: پہلے سے طے شدہ لین کا فوری آغاز
تفصیل: Nexus میں ڈیفالٹ لین کے لئے فال بیک کو سیٹ اور تصدیق کریں تاکہ Torii اور SDKs عوامی لینوں میں LANE_ID کو حذف کرسکیں۔
---

::: سرکاری ماخذ کو نوٹ کریں
یہ صفحہ `docs/source/quickstart/default_lane.md` کی عکاسی کرتا ہے۔ دونوں کاپیاں ایک جیسی رکھیں جب تک کہ لوکلائزیشن اسکین گیٹ وے تک نہ پہنچے۔
:::

# ڈیفالٹ لین کا فوری آغاز (NX-5)

> ** روڈ میپ سیاق و سباق: ** NX -5 - پہلے سے طے شدہ عوامی لین کا انضمام۔ ماحول اب فال بیک بیک `nexus.routing_policy.default_lane` کو بے نقاب کرتا ہے تاکہ Torii میں آرام/GRPC اختتامی نکات اور تمام SDKs جب عوامی کیننیکل لین سے تعلق رکھتے ہیں تو `lane_id` کو محفوظ طریقے سے حذف کرسکتے ہیں۔ یہ گائیڈ آپریٹرز کو کیٹلاگ قائم کرنے ، `/status` پر فال بیک کو چیک کرنے ، اور آخر سے آخر تک کلائنٹ کے طرز عمل کی جانچ کرنے کے لئے رہنمائی کرتا ہے۔

## شرائط

- `irohad` (`irohad --sora --config ...` چلائیں) کا SORA/Nexus ورژن۔
- ترتیبات کے ذخیرے تک رسائی حاصل کریں تاکہ آپ `nexus.*` پارٹیشنز میں ترمیم کرسکیں۔
- `iroha_cli` کو ہدف کلسٹر سے بات کرنے کے لئے تشکیل دیا گیا ہے۔
- `curl`/`jq` (یا مساوی) `/status` کے پے لوڈ کو Torii میں چیک کرنے کے لئے۔

## 1. لین اور ڈیٹا اسپیس کیٹلاگ کی تفصیل

نیٹ ورک پر موجود لینوں اور ڈیٹا اسپیسوں کا اعلان کریں۔ نیچے کا ٹکڑا (`defaults/nexus/config.toml` سے اسنیپٹ) میں تین عوامی لینوں کے علاوہ متعلقہ ڈیٹاسپیس کے عرفی ناموں کو ریکارڈ کیا گیا ہے:

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

ہر `index` منفرد اور مستقل ہونا چاہئے۔ ڈیٹا اسپیس شناخت کار 64 بٹ اقدار ہیں۔ مذکورہ بالا مثالوں میں وہی عددی اقدار کا استعمال کیا گیا ہے جیسے واضح طور پر لین انڈیکس۔

## 2۔ روٹنگ ڈیفالٹس اور اختیاری اوور رائڈس سیٹ کریں

سیکشن `nexus.routing_policy` فال بیک لین کو کنٹرول کرتا ہے اور روٹنگ کو مخصوص ہدایات یا کمپیوٹیشن کے سابقہ ​​کے لئے نظرانداز کرنے کی اجازت دیتا ہے۔ اگر کوئی قاعدہ مماثل نہیں ہے تو ، شیڈولر ٹرانزیکشن کو مخصوص `default_lane` اور `default_dataspace` پر بھیجتا ہے۔ راؤٹر منطق `crates/iroha_core/src/queue/router.rs` میں ہے اور Torii REST/GRPC انٹرفیس پر شفاف طور پر پالیسی کا اطلاق کرتا ہے۔

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

جب بعد میں نئی ​​لینیں شامل کریں تو پہلے کیٹلاگ کو اپ ڈیٹ کریں اور پھر روٹنگ کے قواعد کو بڑھا دیں۔ فال بیک لین کو اب بھی عوامی لین کی طرف اشارہ کرنا چاہئے جو لیگیسی ایس ڈی کے کے لئے مطابقت پذیر رہنے کے ل user صارف ٹریفک کی اکثریت لے کر جاتا ہے۔

## 3. پالیسی کے ساتھ نوڈ بوٹ کرنا

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

نوڈ بوٹ اپ کے دوران اخذ کردہ روٹنگ پالیسی کو رجسٹر کرتا ہے۔ کسی بھی توثیق کی غلطیاں (گمشدہ اشاریہ جات ، ڈپلیکیٹ عرفی ، غلط ڈیٹا اسپیس شناخت کار) گپ شپ شروع ہونے سے پہلے ظاہر ہوتے ہیں۔

## 4. لین گورننس کی حیثیت کی تصدیق کریں

ایک بار نوڈ آن لائن ہونے کے بعد ، یہ تصدیق کرنے کے لئے سی ایل آئی ٹول کا استعمال کریں کہ ورچوئل لین پر مہر لگا دی گئی ہے (مینی فیسٹ بھری ہوئی) اور نقل و حرکت کے لئے تیار ہے۔ خلاصہ نظارہ ہر لین کی تفصیل دکھاتا ہے:

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

اگر پہلے سے طے شدہ لین `sealed` دکھاتا ہے تو ، بیرونی ٹریفک کی اجازت دینے سے پہلے لین گورننس رن بک پر عمل کریں۔ پرچم `--fail-on-sealed` CI کے لئے مفید ہے۔

## 5. Torii کیس لوڈ چیک کریں

جواب `/status` ہر لین کے لئے روٹنگ پالیسی اور شیڈیولر اسنیپ شاٹ دکھاتا ہے۔ عین مفروضوں کی تصدیق کے لئے `curl`/`jq` استعمال کریں اور اس بات کی تصدیق کریں کہ بیک اپ لین ٹیلی میٹری تیار کررہی ہے:

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

لین `0` کے لئے براہ راست شیڈولر کاؤنٹرز کی جانچ پڑتال کے لئے:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

اس سے تصدیق ہوتی ہے کہ ٹی ای یو اسنیپ شاٹ ، عرفی ڈیٹا ، اور منشور کے جھنڈے سیٹ اپ سے ملتے ہیں۔ اسی پے لوڈ کا استعمال LANE-ESTEST ظاہر کرنے کے لئے Grafana بورڈز کے ذریعہ کیا جاتا ہے۔## 6. کسٹمر کے مفروضوں کی جانچ کریں

- ** زنگ / سی ایل آئی۔ تو قطار روٹر `default_lane` لوٹاتا ہے۔ واضح جھنڈے `--lane-id`/`--dataspace-id` صرف غیر ڈیفالٹ لین کو نشانہ بناتے وقت استعمال کریں۔
- ** جے ایس/سوئفٹ/اینڈروئیڈ۔ اسٹیجنگ اور پروڈکشن کے مابین ہم آہنگی میں روٹنگ پالیسی جاری رکھیں تاکہ موبائل ایپس کو ہنگامی تشکیل نو کی ضرورت نہ ہو۔
- ** پائپ لائن/ایس ایس ای ٹیسٹ۔ اس شرط کے ساتھ `/v2/pipeline/events/transactions` کو سبسکرائب کریں تاکہ یہ ثابت کیا جاسکے کہ بغیر کسی واضح لین کے لکھنے والے لکھنے والے فال بیک لین ID کے تحت پہنچے۔

## 7. نگرانی اور گورننس لنکس

- `/status` `nexus_lane_governance_sealed_total` اور `nexus_lane_governance_sealed_aliases` بھی شائع کرتا ہے تاکہ جب لین مینی فیسٹ غائب ہو تو الرٹ مینجر انتباہ کرسکتا ہے۔ ان انتباہات کو بھی ڈیونیٹس میں فعال رکھیں۔
- شیڈولر میٹرک کا نقشہ اور لینز گورننس پینل (`dashboards/grafana/nexus_lanes.json`) کیٹلاگ سے عرف/سلگ فیلڈز کی توقع کرتا ہے۔ اگر آپ عرف کا نام تبدیل کرتے ہیں تو ، اس سے متعلقہ کورا ڈائریکٹریوں کا نام تبدیل کریں تاکہ آڈیٹر جینیاتی راستوں کو برقرار رکھیں (NX-1 کے تحت جاری رہیں)۔
ورچوئل لینوں کے لئے پارلیمانی منظوریوں میں رول بیک پلان شامل ہونا ضروری ہے۔ ٹرگر رن بک میں اس ڈائریکٹری کے ساتھ ہی ظاہر ہیش اور گورننس ڈائریکٹریوں کو ریکارڈ کریں تاکہ آئندہ کے چکروں کو اندازہ نہ لگائیں کہ ریاست کس ریاست کی ضرورت ہے۔