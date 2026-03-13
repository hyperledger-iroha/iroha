---
lang: ur
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sns/bulk_onboarding_toolkit.md` کی عکاسی کرتا ہے تاکہ
بیرونی آپریٹرز ذخیرے کو کلون کیے بغیر ایک ہی SN-3B رہنمائی دیکھتے ہیں۔
:::

# SNS بڑے پیمانے پر آن بورڈنگ ٹول کٹ (SN-3B)

** حوالہ روڈ میپ: ** SN-3B "بلک آن بورڈنگ ٹولنگ"  
** نمونے: ** `scripts/sns_bulk_onboard.py` ، `scripts/tests/test_sns_bulk_onboard.py` ،
`docs/portal/scripts/sns_bulk_release.sh`

بڑے رجسٹرار اکثر سیکڑوں رجسٹریشن `.sora` یا تیار کرتے ہیں
اسی گورننس کی منظوری اور تصفیہ ریلوں کے ساتھ `.nexus`۔
ہاتھ سے JSON پے لوڈ بنانا یا CLI کو دوبارہ شروع کرنا پیمانہ نہیں ہے ، لہذا SN-3B
Norito پر ایک تعصب CSV بلڈر فراہم کرتا ہے جو ڈھانچے کو تیار کرتا ہے
`RegisterNameRequestV1` Torii یا CLI کے لئے۔ مددگار ہر لائن کی توثیق کرتا ہے
اوپر کی طرف ، ایک مجموعی مینی فیسٹ اور JSON دونوں کو خارج کرتا ہے جو نئے کے ذریعہ حد بندی کرتا ہے
اختیاری لائنیں ، اور جب پے لوڈز خود بخود جمع کراسکتی ہیں
آڈٹ کے لئے ساختی رسیدیں ریکارڈ کرنا۔

## 1. CSV اسکیما

پارسر کو درج ذیل ہیڈر لائن کی ضرورت ہوتی ہے (آرڈر لچکدار ہے):

| کالم | مطلوب | تفصیل |
| -------- | -------- | ------------- |
| `label` | ہاں | بے ہودہ درخواستیں (مخلوط کیس قبول کرلیا گیا tool ٹول نورم V1 اور UTS-46 کے مطابق معیاری ہوجاتا ہے)۔ |
| `suffix_id` | ہاں | لاحقہ عددی شناخت کنندہ (اعشاریہ یا `0x` ہیکس)۔ |
| `owner` | ہاں | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | ہاں | انٹیجر `1..=255`۔ |
| `payment_asset_id` | ہاں | تصفیہ اثاثہ (جیسے `xor#sora`)۔ |
| `payment_gross` / `payment_net` | ہاں | اثاثہ کے مقامی اکائیوں کی نمائندگی کرنے والے دستخط شدہ عدد۔ |
| `settlement_tx` | ہاں | ادائیگی کے لین دین یا ہیش کو بیان کرنے والے JSON قدر یا لفظی تار۔ |
| `payment_payer` | ہاں | اکاؤنٹ آئی ڈی جس نے ادائیگی کو اختیار دیا۔ |
| `payment_signature` | ہاں | JSON یا لفظی تار جس میں اسٹیورڈ یا ٹریژری کے دستخط کا ثبوت ہے۔ |
| `controllers` | اختیاری | سیمیکولون یا کوما سے الگ الگ کنٹرولر اکاؤنٹ کے پتے کی فہرست۔ اگر چھوڑ دیا گیا ہو تو `[owner]` میں پہلے سے طے شدہ۔ |
| `metadata` | اختیاری | ان لائن JSON یا `@path/to/file.json` حل کرنے والے اشارے ، TXT ریکارڈز ، وغیرہ کو بطور ڈیفالٹ `{}` فراہم کرتا ہے۔ |
| `governance` | اختیاری | ان لائن JSON یا `@path` `GovernanceHookV1` کی طرف اشارہ کررہا ہے۔ `--require-governance` اس کالم کو مسلط کرتا ہے۔ |

کوئی بھی کالم سیل ویلیو سے پہلے سے کسی بیرونی فائل کا حوالہ دے سکتا ہے
`@` کے ذریعہ۔ CSV فائل کے نسبت راستے حل کیے جاتے ہیں۔

## 2. ہیلپر چلائیں

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

کلیدی اختیارات:

- `--require-governance` بغیر گورننس ہکس کے لائنوں کو مسترد کرتا ہے (اس کے لئے مفید ہے
  پریمیم نیلامی یا محفوظ اسائنمنٹس)۔
- `--default-controllers {owner,none}` فیصلہ کرتا ہے کہ آیا کنٹرولر خلیات خالی ہیں یا نہیں
  مالک کے اکاؤنٹ پر گریں۔
- `--controllers-column` ، Torii ، اور `--governance-column` اجازت دیں
  اپ اسٹریم برآمدات کے دوران اختیاری کالموں کا نام تبدیل کرنا۔

اگر کامیاب ہو تو ، اسکرپٹ ایک مجموعی مینی فیسٹ لکھتا ہے:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
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
JSON ایک لائن پر دستاویز کریں تاکہ آٹومیشنز کو اسٹریم کرسکیں
براہ راست Torii کی درخواستیں:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v2/sns/registrations
  done
```

## 3. خودکار گذارشات

### 3.1 وضع Torii REST

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

- مددگار `POST /v2/sns/registrations` فی درخواست کا اخراج کرتا ہے اور پہلے میں رک جاتا ہے
  HTTP غلطی. جوابات کو لاگ میں این ڈی جےسن ریکارڈ کے طور پر شامل کیا جاتا ہے۔
- `--poll-status` ہر ایک کے بعد `/v2/sns/registrations/{selector}` کو دوبارہ داخل کرتا ہے
  اس کی تصدیق کرنے کے لئے جمع کرانا (`--poll-attempts` ، پہلے سے طے شدہ 5)
  ریکارڈنگ نظر آتی ہے۔ `--suffix-map` (`suffix_id` کا JSON فراہم کریں
  "لاحقہ" اقدار کو) تاکہ آلے کے لغویوں کو اخذ کیا جائے
  پولنگ کے لئے `{label}.{suffix}`۔
- ایڈجسٹ: `--submit-timeout` ، `--poll-attempts` ، اور `--poll-interval`۔

### 3.2 Iroha Cli وضع

سی ایل آئی کے ذریعے ہر ظاہر ہونے والے اندراج کو منتقل کرنے کے ل the ، مینی فیسٹ کو راستہ فراہم کریں
بائنری:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- کنٹرولرز لازمی طور پر `Account` (`controller_type.kind = "Account"`) ہونا چاہئے
  کیونکہ CLI صرف اکاؤنٹ پر مبنی کنٹرولرز کو بے نقاب کرتا ہے۔
- میٹا ڈیٹا اور گورننس بلبس عارضی فائلوں پر لکھے گئے ہیں
  درخواست اور `iroha sns register --metadata-json ... --governance-json ...` میں منتقل کیا گیا۔
- CLI STDOUT اور STDERR کے ساتھ ساتھ ایگزٹ کوڈ لاگ ان ہیں۔
  غیر صفر کوڈز عملدرآمد میں خلل ڈالتے ہیں۔

دونوں جمع کرانے کے طریقے مل کر کام کرسکتے ہیں (Torii اور CLI)
کراس رجسٹرار کی تعیناتی یا فال بیکس کو دہرائیں۔

### 3.3 جمع کرانے کی رسید

جب `--submission-log <path>` فراہم کیا جاتا ہے تو ، اسکرپٹ NDJSON اندراجات میں اضافہ کرتا ہے
گرفتاری:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

کامیاب Torii جوابات میں ڈھانچے کے فیلڈز شامل ہیں جن سے نکالا گیا ہے
`NameRecordV1` یا `RegisterNameResponseV1` (مثال کے طور پر `record_status` ،
`record_pricing_class` ، `record_owner` ، `record_expires_at_ms` ،
`registry_event_version` ، `suffix_id` ، `label`) تاکہ ڈیش بورڈز اور
گورننس رپورٹس مفت متن کا معائنہ کیے بغیر لاگ ان کی تجزیہ کرسکتی ہیں۔
اس لاگ کو رجسٹرار کے ٹکٹوں پر منسلک کریں ثبوت کے لئے مینی فیسٹ کے ساتھ
تولیدی

## 4. پورٹل ریلیز آٹومیشن

CI اور پورٹل ملازمتیں `docs/portal/scripts/sns_bulk_release.sh` پر کال کرتی ہیں ، جو
مددگار کو لپیٹتا ہے اور نمونے کے تحت اسٹور کرتا ہے
`artifacts/sns/releases/<timestamp>/`:

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

اسکرپٹ:1. `registrations.manifest.json` ، `registrations.ndjson` تعمیر کریں ، اور اس کی کاپی کریں
   ریلیز ڈائرکٹری میں اصل CSV۔
2. Torii اور/یا CLI (جب تشکیل شدہ) کے ذریعے منشور پیش کرتا ہے ،
   `submissions.log` اوپر کی رسیدوں کے ڈھانچے کے ساتھ۔
3. `summary.json` کو ریلیز (راستے ، URL Torii ، CLI PATH ،
   ٹائم اسٹیمپ) تاکہ پورٹل آٹومیشن بنڈل اپ لوڈ کرسکے
   نوادرات کا اسٹوریج۔
4. پروڈکٹ `metrics.prom` (`--metrics` کے ذریعے اوور رائڈ) کاؤنٹرز پر مشتمل ہے
   Prometheus فارمیٹ میں درخواستوں کی کل تعداد ، لاحقہ کی تقسیم ،
   اثاثہ مجموعی اور جمع کرانے کے نتائج۔ JSON دوبارہ شروع کرنے کی طرف اشارہ کرتا ہے
   یہ فائل

ورک فلوز صرف ایک ہی نمونے کے طور پر ریلیز ڈائرکٹری کو محفوظ کرتے ہیں ،
جس میں اب ہر چیز پر مشتمل ہے جو گورننس کو آڈٹ کے لئے درکار ہے۔

## 5. ٹیلی میٹری اور ڈیش بورڈز

`sns_bulk_release.sh` کے ذریعہ تیار کردہ میٹرکس فائل سیریز کو بے نقاب کرتی ہے
دوبارہ:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom` کو اپنے سائڈیکار Prometheus میں انجیکشن لگائیں (جیسے پرومٹیل کے ذریعے یا
ایک بیچ کی درآمد) رجسٹرار ، اسٹیورڈز اور گورننس کے ساتھیوں کو سیدھ میں لانے کے لئے
بڑے پیمانے پر ترقی ٹیبل Grafana
`dashboards/grafana/sns_bulk_release.json` ایک ہی ڈیٹا کے ساتھ تصور کرتا ہے
لاحقہ ، ادائیگی کے حجم اور تناسب کے ذریعہ اکاؤنٹس کے لئے پینل
جمع کرانے کی کامیابی/ناکامی۔ ٹیبل `release` کے ذریعہ فلٹر کرتا ہے
سامعین ایک ہی CSV عملدرآمد پر توجہ دے سکتے ہیں۔

## 6. توثیق اور ناکامی کے طریقوں

- ** لیبل نارملائزیشن: ** اندراجات کو ازگر IDNA پلس کے ساتھ معمول بنایا جاتا ہے
  لوئر کیس اور کریکٹر فلٹر نورم V1۔ لیس لیبلز انیلائڈس ایکوینٹ وائٹ
  کسی بھی نیٹ ورک کال سے پہلے۔
- ** عددی حفاظت
  ٹرمینلز `u16` اور `u8` میں رہیں۔ ادائیگی کے فیلڈ قبول کریں
  `i64::MAX` تک اعشاریہ یا ہیکس عدد۔
- ** میٹا ڈیٹا یا گورننس کی تجزیہ کرنا: ** ان لائن JSON براہ راست تجزیہ کیا گیا ہے۔
  فائلوں کے حوالہ جات CSV کے مقام کے مطابق حل کیے جاتے ہیں۔
  غیر آبجیکٹ میٹا ڈیٹا توثیق کی غلطی پیدا کرتا ہے۔
- ** کنٹرولرز: ** خالی خلیات `--default-controllers` کا احترام کرتے ہیں۔ فراہم کریں
  جب آپ تفویض کرتے ہیں تو واضح فہرستیں (جیسے `i105...;i105...`)
  غیر مالک اداکار۔

متعلقہ لائن نمبر (جیسے۔
`error: row 12 term_years must be between 1 and 255`)۔ اسکرپٹ کے ساتھ باہر آتا ہے
کوڈ `1` توثیق کی غلطیوں اور `2` پر جب CSV راستہ غائب ہے۔

## 7. ٹیسٹ اور پروویژن

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` CSV تجزیہ کا احاطہ کرتا ہے ،
  این ڈی جےسن کا اخراج ، انفورسمنٹ گورننس ، اور سی ایل آئی جمع کرانے والے راستے یا Torii۔
- مددگار خالص ازگر (کوئی اضافی انحصار نہیں) ہے اور ہر جگہ چلتا ہے
  یا `python3` دستیاب ہے۔ کمٹ کی تاریخ کے ساتھ ساتھ ٹریک کیا جاتا ہے
  تولیدی صلاحیت کے لئے مرکزی ذخیرہ میں سی ایل آئی۔پروڈکشن رنز کے لئے ، پیدا شدہ مینی فیسٹ اور این ڈی جےسن بنڈل کو رب سے جوڑیں
رجسٹرار ٹکٹ تو اسٹیورڈز عین مطابق پے لوڈ کو دوبارہ چلا سکتے ہیں
Torii کے تابع۔