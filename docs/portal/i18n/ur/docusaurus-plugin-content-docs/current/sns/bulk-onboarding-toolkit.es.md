---
lang: ur
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
بیرونی آپریٹرز کو دیکھنے کے لئے آئینہ `docs/source/sns/bulk_onboarding_toolkit.md`
ذخیرہ کو کلون کیے بغیر وہی SN-3B گائیڈ۔
:::

# SNS بڑے پیمانے پر آن بورڈنگ ٹول کٹ (SN-3B)

** روڈ میپ حوالہ: ** SN-3B "بلک آن بورڈنگ ٹولنگ"  
** نمونے: ** `scripts/sns_bulk_onboard.py` ، `scripts/tests/test_sns_bulk_onboard.py` ،
`docs/portal/scripts/sns_bulk_release.sh`

بڑے رجسٹرار اکثر سینکڑوں `.sora` یا `.nexus` ریکارڈز سے پہلے سے پیش پیش کرتے ہیں
اسی گورننس کی منظوری اور تصفیہ ریلوں کے ساتھ۔ JSON پے لوڈ کو جمع کریں
ہاتھ سے یا دوبارہ جاری کرنے سے CLI پیمانہ نہیں ہوتا ہے ، لہذا SN-3B ایک بلڈر فراہم کرتا ہے
Norito سے تعی .ن CSV جو `RegisterNameRequestV1` ڈھانچے کے لئے تیار کرتا ہے
Torii یا CLI۔ مددگار ہر صف سے پہلے ہی توثیق کرتا ہے ، دونوں کو ظاہر کرتا ہے
ایگریگاڈو کومو جیسن ڈیمیٹاڈو پور سالٹوس ڈی لائنیا اوپیسیونل ، وائی پیوڈین انویئر لاس
آڈٹ کے لئے ساختی رسیدوں کو ریکارڈ کرتے وقت خود بخود پے لوڈز۔

## 1. CSV اسکیم

پارسر کو درج ذیل ہیڈر قطار کی ضرورت ہوتی ہے (آرڈر لچکدار ہے):

| کالم | مطلوب | تفصیل |
| --------- | ----------- | ------------- |
| `label` | ہاں | Requested label (upper/minus is accepted; the tool normalizes according to Norm v1 and UTS-46). |
| `suffix_id` | ہاں | لاحقہ عددی شناخت کنندہ (اعشاریہ یا `0x` ہیکس)۔ |
| `owner` | ہاں | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | ہاں | انٹیجر `1..=255`۔ |
| `payment_asset_id` | ہاں | تصفیہ اثاثہ (مثال کے طور پر `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `payment_gross` / `payment_net` | ہاں | دستخط شدہ عدد جو اثاثہ کے مقامی اکائیوں کی نمائندگی کرتے ہیں۔ |
| `settlement_tx` | ہاں | JSON قدر یا لفظی تار جو ہیش یا ادائیگی کے لین دین کو بیان کرتا ہے۔ |
| `payment_payer` | ہاں | اکاؤنٹ آئی ڈی جس نے ادائیگی کو اختیار دیا۔ |
| `payment_signature` | ہاں | JSON یا لفظی تار اسٹیورڈ یا ٹریژری دستخط کے ثبوت کے ساتھ۔ |
| `controllers` | اختیاری | سیمیکولون یا کوما سے الگ الگ کنٹرولر اکاؤنٹ کے پتے کی فہرست۔ جب چھوڑ دیا جاتا ہے تو `[owner]` میں پہلے سے طے شدہ ہوتا ہے۔ |
| `metadata` | اختیاری | ان لائن JSON یا `@path/to/file.json` جو حل کرنے والے اشارے ، TXT ریکارڈز وغیرہ کو بطور ڈیفالٹ `{}` فراہم کرتا ہے۔ |
| `governance` | اختیاری | JSON ان لائن یا `@path` `GovernanceHookV1` کی طرف اشارہ کررہا ہے۔ `--require-governance` کو اس کالم کی ضرورت ہے۔ |

کوئی بھی کالم `@` کے ساتھ سیل ویلیو کو پہلے سے تیار کرکے بیرونی فائل کا حوالہ دے سکتا ہے۔
CSV فائل کے نسبت راستے حل کیے جاتے ہیں۔

## 2. مددگار چلائیں

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

کلیدی اختیارات:

- `--require-governance` بغیر کسی گورننس ہک کے قطاروں کو مسترد کرتا ہے (اس کے لئے مفید ہے
  پریمیم نیلامی یا محفوظ مختص رقم)۔
- `--default-controllers {owner,none}` فیصلہ کرتا ہے کہ آیا کنٹرولرز کے خالی خلیات
  وہ اکاؤنٹ کے مالک کو لوٹتے ہیں۔
- `--controllers-column` ، Torii ، اور `--governance-column` اجازت دیں
  اپ اسٹریم برآمدات کے ساتھ کام کرتے وقت اختیاری کالموں کا نام تبدیل کریں۔

کامیابی پر اسکرپٹ ایک اضافی مینی فیسٹ لکھتا ہے:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "soraカタカナ...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"soraカタカナ...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"soraカタカナ...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```اگر `--ndjson` فراہم کیا گیا ہے تو ، ہر `RegisterNameRequestV1` بھی بطور لکھا جاتا ہے
سنگل لائن JSON دستاویز تاکہ آٹومیشنز منتقل ہوسکیں
براہ راست Torii کی درخواستیں:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. خودکار ترسیل

### 3.1 Torii REST موڈ

`--submit-torii-url` پلس `--submit-token` یا `--submit-token-file` کے لئے بتائیں
ہر مینی فیسٹ اندراج کو براہ راست Torii پر دبائیں:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- مددگار ایک `POST /v1/sns/names` فی درخواست جاری کرتا ہے اور اس سے پہلے اسقاط حمل کرتا ہے
  پہلی HTTP غلطی۔ جوابات کو بطور ریکارڈ لاگ ان راستے میں شامل کیا جاتا ہے
  ndjson.
- `--poll-status` `/v1/sns/names/{namespace}/{literal}` کے بعد ایک بار پھر مشورہ کرتا ہے
  ہر جمع کرانے (`--poll-attempts` ، پہلے سے طے شدہ 5) اس بات کی تصدیق کرنے کے لئے کہ ریکارڈ
  یہ دکھائی دیتا ہے۔ `--suffix-map` (`suffix_id` سے "لاحقہ" اقدار تک فراہم کریں) فراہم کریں)
  تاکہ ٹول پولنگ کے وقت لفظی `{label}.{suffix}` اخذ کرے۔
- ترتیبات: `--submit-timeout` ، `--poll-attempts` ، اور `--poll-interval`۔

### 3.2 Iroha Cli وضع

سی ایل آئی کے ذریعے ہر ظاہر ہونے والے اندراج کو روٹ کرنے کے لئے ، بائنری کو راستہ فراہم کریں:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- کنٹرولرز لازمی طور پر `Account` (`controller_type.kind = "Account"`) ہونا چاہئے
  کیونکہ CLI فی الحال صرف اکاؤنٹ پر مبنی کنٹرولرز کو بے نقاب کرتا ہے۔
- میٹا ڈیٹا اور گورننس بلبس عارضی فائلوں پر لکھے گئے ہیں
  درخواست اور `iroha sns register --metadata-json ... --governance-json ...` پر منتقل کردی گئی ہے۔
- سی ایل آئی کے اسٹڈ آؤٹ اور اسٹڈر کے علاوہ خارجی کوڈ ریکارڈ کیے گئے ہیں۔ کوڈز
  غیر صفر پھانسی کو ختم کردیں۔

دونوں شپنگ طریقوں کو ایک ساتھ چلایا جاسکتا ہے (Torii اور CLI) تصدیق کرنے کے لئے
فال بیکس کو ریکارڈنگ یا ریہرسل کرنے کی تعیناتی۔

### 3.3 شپنگ رسیدیں

جب `--submission-log <path>` فراہم کیا جاتا ہے تو ، اسکرپٹ NDJSON اندراجات میں شامل ہوتا ہے
وہ گرفتاری:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Torii پر کامیاب ردعمل میں ساختہ فیلڈز شامل ہیں جن سے نکالا گیا ہے
`NameRecordV1` یا `RegisterNameResponseV1` (مثال کے طور پر `record_status` ،
`record_pricing_class` ، `record_owner` ، `record_expires_at_ms` ،
`registry_event_version` ، `suffix_id` ، `label`) تاکہ ڈیش بورڈز اور رپورٹس
گورننس مفت متن کا معائنہ کیے بغیر لاگ ان کی تجزیہ کرسکتی ہے۔ اس لاگ کو منسلک کریں
تولیدی شواہد کے لئے ظاہر کے ساتھ رجسٹرار کے ٹکٹ۔

## 4. پورٹل ریلیز آٹومیشن

سی آئی اور پورٹل ملازمتوں کو `docs/portal/scripts/sns_bulk_release.sh` پر کال کریں ،
جو مددگار کو لپیٹتا ہے اور `artifacts/sns/releases/<timestamp>/` کے تحت نمونے کو بچاتا ہے:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

اسکرپٹ:1. `registrations.manifest.json` ، `registrations.ndjson` بنائیں ، اور کاپی کریں
   ریلیز ڈائرکٹری میں اصل CSV۔
2. Torii اور/یا CLI (جب تشکیل شدہ) ، ٹائپنگ کا استعمال کرتے ہوئے مینی فیسٹ جمع کروائیں
   `submissions.log` اوپر ساختہ رسیدوں کے ساتھ۔
3. جاری کریں `summary.json` ریلیز (راستے ، URL Torii ، CLI پاتھ ،
   ٹائم اسٹیمپ) تاکہ پورٹل آٹومیشن بنڈل کو لوڈ کرسکے
   نوادرات کا اسٹوریج۔
4. `metrics.prom` تیار کرتا ہے (`--metrics` کے ذریعے اوور رائڈ) کاؤنٹرز پر مشتمل ہے
   Prometheus فارمیٹ میں کل درخواستوں کے لئے ، لاحقہ کی تقسیم ،
   اثاثہ مجموعی اور شپنگ کے نتائج۔ خلاصہ JSON اس سے لنک کرتا ہے
   فائل

ورک فلوز صرف ایک ہی نمونے کے طور پر ریلیز ڈائرکٹری کو محفوظ کرتے ہیں ،
which now contains everything governance needs for auditing.

## 5. ٹیلی میٹری اور ڈیش بورڈز

`sns_bulk_release.sh` کے ذریعہ تیار کردہ میٹرکس فائل مندرجہ ذیل کو بے نقاب کرتی ہے
سیریز:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

پاور `metrics.prom` آپ کے Prometheus SIDECAR میں (جیسے پرومٹیل کے ذریعے یا
ایک بیچ درآمد کنندہ) رجسٹرار ، اسٹیورڈز اور گورننس کے ساتھیوں کو برقرار رکھنے کے لئے
بڑے پیمانے پر پیشرفت پر منسلک۔ بورڈ Grafana
`dashboards/grafana/sns_bulk_release.json` پینل کے ساتھ ایک ہی ڈیٹا کو دکھاتا ہے
لاحقہ گنتی ، ادائیگی کا حجم اور شپمنٹ کامیابی/ناکامی کے تناسب کے لئے۔
ڈیش بورڈ فلٹرز `release` کے ذریعہ تاکہ آڈیٹر کسی سنگل میں داخل ہوسکیں
CSV رن.

## 6. توثیق اور ناکامی کے طریقوں

- ** لیبل نارملائزیشن: ** اندراجات کو ازگر IDNA کے ساتھ معمول بنایا جاتا ہے
  لوئر کیس اور کریکٹر فلٹر نورم V1۔ اس سے پہلے غلط لیبل تیزی سے ناکام ہوجاتے ہیں
  کسی بھی نیٹ ورک کال کی۔
- ** عددی محافظ: ** لاحقہ IDs ، اصطلاحی سال ، اور قیمتوں کے اشارے ضرور گرنا ہوں گے
  حدود میں `u16` اور `u8`۔ ادائیگی کے شعبے اعشاریہ عدد کو قبول کرتے ہیں یا
  ہیکس سے `i64::MAX`۔
- ** میٹا ڈیٹا یا گورننس پارسنگ: ** ان لائن JSON براہ راست تجزیہ کیا گیا ہے۔
  فائل کے حوالہ جات CSV کے مقام کے مطابق حل کیے جاتے ہیں۔ میٹا ڈیٹا
  کہ یہ کوئی شے نہیں ہے توثیق کی غلطی پیدا کرتی ہے۔
- ** کنٹرولرز: ** خالی خلیات `--default-controllers` کا احترام کرتے ہیں۔ فراہم کریں
  جب تفویض کرتے ہو تو واضح کنٹرولر کی فہرستیں (مثال کے طور پر `i105...;i105...`)
  غیر مالک اداکار۔

کیڑے کو سیاق و سباق کی تعداد کے ساتھ اطلاع دی جاتی ہے (جیسے۔
`error: row 12 term_years must be between 1 and 255`)۔ اسکرپٹ کوڈ کے ساتھ باہر نکلتا ہے
`1` توثیق کی غلطیوں میں اور `2` جب CSV راستہ غائب ہے۔

## 7. جانچ اور پروویژن

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` CSV تجزیہ کا احاطہ کرتا ہے ،
  این ڈی جےسن کا اخراج ، گورننس انفورسمنٹ اور سی ایل آئی یا Torii کے ذریعہ راستے بھیجنا۔
- مددگار خالص ازگر ہے (کوئی اضافی انحصار نہیں) اور کسی پر چلتا ہے
  جگہ جہاں `python3` دستیاب ہے۔ ارتکاب کی تاریخ کو ایک ساتھ ٹریک کیا جاتا ہے
  تولیدی صلاحیت کے لئے مرکزی ذخیرہ میں سی ایل آئی کو۔پروڈکشن رنز کے لئے ، پیدا شدہ مینی فیسٹ اور این ڈی جےسن بنڈل کو رب سے جوڑیں
رجسٹرار ٹکٹ تو اسٹیورڈز عین مطابق پے لوڈ کو دوبارہ پیش کرسکتے ہیں
جو Torii پر بھیجا گیا تھا۔