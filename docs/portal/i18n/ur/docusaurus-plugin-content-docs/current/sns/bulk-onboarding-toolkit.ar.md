---
lang: ur
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ معیاری ماخذ
`docs/source/sns/bulk_onboarding_toolkit.md` کی عکاسی کرتا ہے تاکہ بیرونی آپریٹر اسے دیکھ سکیں
ذخیرے کو کلون کیے بغیر SN-3B کی طرح ہدایات۔
:::

# SNS بلک کنفیگریشن کٹ (SN-3B)

** روڈ میپ حوالہ: ** SN-3B "بلک آن بورڈنگ ٹولنگ"  
** اثرات: ** `scripts/sns_bulk_onboard.py` ، `scripts/tests/test_sns_bulk_onboard.py` ،
`docs/portal/scripts/sns_bulk_release.sh`

بڑے رجسٹرار اکثر اسی کے ساتھ سیکڑوں `.sora` یا `.nexus` رجسٹریشن تیار کرتے ہیں
گورننس کی منظوری اور تصفیہ چینلز۔ فارمیٹ JSON پے لوڈ دستی طور پر یا دوبارہ شروع کریں
سی ایل آئی اسکیل نہیں کرتا ہے ، لہذا SN-3B بلڈر CSV سے Norito تیار کرنے والے ڈھانچے کو لازمی فراہم کرتا ہے۔
`RegisterNameRequestV1` Torii یا CLI کے لئے۔ اسسٹنٹ ہر صف سے پہلے چیک کرتا ہے ،
یہ ایک ریپر مینی فیسٹ اور اختیاری لائن سے الگ الگ JSON دونوں برآمد کرتا ہے ، جسے بھیجا جاسکتا ہے
آڈٹ کے مقاصد کے لئے تنظیم کے ساتھ پے لوڈ خود بخود رسیدیں ریکارڈ کرتے ہیں۔

## 1. CSV چارٹ

پارسر کو مندرجہ ذیل ایڈریس قطار کی ضرورت ہوتی ہے (آرڈر لچکدار ہے):

| کالم | مطلوب | تفصیل |
| -------- | ------- | ------- |
| `label` | ہاں | مطلوبہ لیبل (مخلوط کیس کو قبول کرتا ہے ؛ نورم V1 اور UTS-46 کے مطابق ٹول پرنٹس)۔ |
| `suffix_id` | ہاں | عددی لاحقہ شناخت کنندہ (اعشاریہ یا `0x` ہیکس)۔ |
| `owner` | ہاں | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | ہاں | انٹیجر `1..=255`۔ |
| `payment_asset_id` | ہاں | تصفیہ کی اصل (جیسے `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `payment_gross` / `payment_net` | ہاں | کارڈنلٹی کے اکائیوں کی نمائندگی کرنے والے دستخط شدہ عدد۔ |
| `settlement_tx` | ہاں | ادائیگی کے لین دین ، ​​یا ہیش کو بیان کرنے والے JSON قدر یا لفظی تار۔ |
| `payment_payer` | ہاں | اکاؤنٹڈ جس نے ادائیگی کا اختیار دیا۔ |
| `payment_signature` | ہاں | JSON یا لفظی تار جس میں اسٹیورڈ یا والٹ کے دستخط کے ثبوت موجود ہیں۔ |
| `controllers` | اختیاری | کنٹرولر اکاؤنٹ کے پتے کی کوما یا سیمیکولون سے الگ فہرست۔ جب حذف ہونے پر `[owner]` میں پہلے سے طے شدہ ہوتا ہے۔ |
| `metadata` | اختیاری | JSON ان لائن (`@path/to/file.json`) حل کرنے والے اشارے ، TXT ریکارڈز وغیرہ فراہم کرتا ہے۔ ڈیفالٹ `{}`۔ |
| `governance` | اختیاری | JSON ان لائن یا `@path` `GovernanceHookV1` سے مراد ہے۔ `--require-governance` اس کالم پر مجبور کرتا ہے۔ |

کوئی بھی کالم `@` کے ساتھ سیل ویلیو کو تیار کرکے بیرونی فائل کا حوالہ دے سکتا ہے۔
CSV فائل کے نسبت راستے حل کیے جاتے ہیں۔

## 2. پلگ ان چلائیں

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

اہم اختیارات:

- `--require-governance` بغیر ہک گورننس کے قطاروں کو مسترد کرتا ہے (پریمیم کے لئے مفید ہے یا
  محفوظ تقرریوں)۔
- `--default-controllers {owner,none}` فیصلہ کرتا ہے کہ خلیات کنٹرولر ہیں یا نہیں
  خالی مالک کے اکاؤنٹ میں واپس آجاتا ہے۔
- `--controllers-column` ، Torii ، اور `--governance-column` اجازت دیں
  بیرونی برآمدات کے ساتھ کام کرتے وقت اختیاری کالموں کا نام تبدیل کریں۔

کامیابی کے بعد ، اسکرپٹ ایک مرتب شدہ مینی فیسٹ لکھتا ہے:

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
```

اگر `--ndjson` گزر جاتا ہے تو ، ہر `RegisterNameRequestV1` بھی ایک JSON لائن کے طور پر لکھا جاتا ہے۔
تاکہ آٹومیشن براہ راست Torii پر درخواستوں کو نشر کرسکے:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. خودکار ٹرانسمیشن

### 3.1 Torii REST موڈ

id دد `--submit-torii-url` mae `--submit-token` اوس `--submit-token-file` لارسال ایل
براہ راست Torii پر منشور میں داخل ہوں:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- مددگار ہر درخواست کے لئے `POST /v1/sns/names` جاری کرتا ہے اور پہلی HTTP غلطی پر رک جاتا ہے۔
  جوابات کو لاگ پاتھ میں این ڈی جےسن ریکارڈ کے طور پر شامل کیا جاتا ہے۔
- `--poll-status` ہر ایک کے بعد `/v1/sns/names/{namespace}/{literal}`
  اس بات کی تصدیق کرنے کے لئے کہ ریکارڈ ظاہر ہوتا ہے (`--poll-attempts` ، پہلے سے طے شدہ 5) بھیجیں۔ بچت کریں
  `--suffix-map` (JSON `suffix_id` کو "لاحقہ" اقدار میں تبدیل کرتا ہے) تاکہ آلے کا ٹول ہو سکے
  پولنگ کے وقت `{label}.{suffix}` لاحقہ لاحقہ کا اخذ۔
- سایڈست کی ترتیبات: `--submit-timeout` ، `--poll-attempts` ، اور `--poll-interval`۔

### 3.2 Iroha Cli وضع

سی ایل آئی کے ذریعے ہر ظاہر ہونے والے اندراج کو منتقل کرنے کے لئے ، قابل عمل فائل کا راستہ فراہم کریں:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- کنٹرولرز ٹائپ `Account` (`controller_type.kind = "Account"`) کے ہونا چاہئے
  کیونکہ CLI فی الحال صرف اکاؤنٹ پر مبنی کنٹرولرز کی حمایت کرتا ہے۔
- میٹا ڈیٹا اور گورننس بلبس ہر درخواست کے لئے عارضی فائلوں پر لکھے گئے ہیں
  ستراریہا الى `iroha sns register --metadata-json ... --governance-json ...`۔
- ایس ٹی ڈی آؤٹ اور اسٹڈرر ایگزٹ کوڈ کے ساتھ رجسٹرڈ ہیں۔ غیر صفر کوڈز اسٹاپ آپریشن۔

دونوں ٹرانسمیشن طریقوں کو ایک ساتھ چلایا جاسکتا ہے (Torii اور CLI) کو کراس چیک لاگر کی تعیناتی کے لئے یا
فال بیک راستوں پر عمل کرنے کے لئے۔

### 3.3 پوسٹنگ رسیدیں

جب `--submission-log <path>` گزر جاتا ہے تو ، اسکرپٹ میں NDJSON ریکارڈ شامل ہوتا ہے جو گرفتاری کرتے ہیں:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

کامیاب Torii جوابات میں `NameRecordV1` سے نکالے گئے ساختہ فیلڈز شامل ہیں۔
`RegisterNameResponseV1` (جیسے `record_status` ، `record_pricing_class` ،
`record_owner` ، `record_expires_at_ms` ، `registry_event_version` ، `suffix_id` ،
`label`) لہذا ڈیش بورڈز اور گورننس رپورٹس بغیر معائنہ کے لاگ ان کا تجزیہ کرسکتی ہیں
مفت متن اس ریکارڈ کو رجسٹرار کے ٹکٹ سے جوڑیں اور اس کے ساتھ ساتھ تولیدی ثبوت کے لئے مینی فیسٹ کے ساتھ
پیداوار

## 4. دستاویزات کے پورٹل کے اجراء کو خودکار کرنا

CI اور گیٹ وے کے کام `docs/portal/scripts/sns_bulk_release.sh` پر کال کرتے ہیں جو لپیٹتے ہیں
پلگ ان `artifacts/sns/releases/<timestamp>/` کے تحت نشانات اسٹور کرتا ہے:

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

اسکرپٹ:

1. `registrations.manifest.json` اور `registrations.ndjson` تعمیر کرتا ہے اور اصل CSV کی کاپی کرتا ہے
   ریلیز فولڈر میں۔
2. Torii اور/یا CLI (جب تشکیل شدہ) کے ذریعے منشور بھیجتا ہے ، اور `submissions.log` کے ساتھ لکھتا ہے
   رسیدیں اوپر منظم ہیں۔
3. Issues `summary.json` which describes the version (paths, Torii address, CLI path,
   ٹائم اسٹیمپ) تاکہ گیٹ وے آٹومیشن پیکیج کو ریلک اسٹور پر اپ لوڈ کرسکے۔
4. `metrics.prom` تیار کرتا ہے (`--metrics` کے ذریعے اوور رائڈ) سمیت مطابقت پذیر کاؤنٹرز
   درخواستوں کی کل تعداد ، لاحقہ کی تقسیم ، اصل مجموعی اور ٹرانسمیشن کے نتائج کے لئے Prometheus۔
   JSON اس فائل کے ساتھ سمری کو جوڑتا ہے۔

ورکشاپ آرکائیوز ایک سے زیادہ ریلیز فولڈر ، جس میں اب آپ کی ہر چیز کی ضرورت ہے
آڈٹ کے لئے تخصیصات۔

## 5. پیمائش اور نگرانی کرنے والے پینل

`sns_bulk_release.sh` کے ذریعہ تیار کردہ میٹرکس فائل مندرجہ ذیل ڈور دکھاتی ہے:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom` کو اپنے Prometheus sidecar (جیسے پرومٹیل کے ذریعے یا کے ذریعے فیڈ کریں
ادائیگی امپورٹر) رجسٹرار ، اسٹیورڈز اور گورننس کے شراکت داروں کو پیشرفت پر منسلک رکھنے کے لئے
تھوک بورڈ Grafana `dashboards/grafana/sns_bulk_release.json` ایک ہی دکھاتا ہے
ہر لاحقہ ، ادائیگی کے سائز ، اور جمع کرانے کی کامیابی/ناکامی کی شرحوں کے لئے درخواستوں کی تعداد کے لئے پینلز والا ڈیٹا۔
بورڈ `release` کے ذریعے فلٹر کرتا ہے تاکہ آڈیٹر CSV رن میں گہری کھود سکیں
ایک

## 6. توثیق اور ناکامی۔
  نورم V1. کسی بھی نیٹ ورک کنکشن سے پہلے غلط لیبل تیزی سے ناکام ہوجاتے ہیں۔
- ** عددی رکاوٹیں: ** لاحقہ آئی ڈی ، ٹرم سال اور قیمتوں کے اشارے کو حدود میں آنا چاہئے
  `u16` v `u8`۔ ادائیگی کے فیلڈز `i64::MAX` تک اعشاریہ یا ہیکس نمبر قبول کرتے ہیں۔
- ** میٹا ڈیٹا یا گورننس تجزیہ: ** JSON ان لائن کو براہ راست تجزیہ کیا گیا ہے۔ اور یہ حل ہے
  CSV مقام سے متعلق فائل کے حوالہ جات۔ آبجیکٹ کے علاوہ میٹا ڈیٹا آبجیکٹ چیک کی خرابی پیدا کرتا ہے۔
- **Controllers:** Empty cells commit `--default-controllers`. فہرستیں بنائیں
  جب مالک کے علاوہ کسی اور فریقوں کو تفویض کرتے وقت واضح کنٹرولر (جیسے `i105...;i105...`)۔

غلطیوں کی اطلاع سیاق و سباق کی تعداد کے ساتھ کی جاتی ہے (جیسے
`error: row 12 term_years must be between 1 and 255`)۔ اسکرپٹ کوڈ `1` کے ساتھ باہر ہے
توثیق کی غلطیوں اور `2` پر جب CSV راستہ غائب ہے۔

## 7. جانچ اور وشوسنییتا

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` CSV تجزیہ کا احاطہ کرتا ہے ،
  این ڈی جےسن ورژن ، گورننس انفورسمنٹ ، اور سی ایل آئی جمع کرانے والے راستے یا Torii۔
- مددگار صرف ازگر میں لکھا گیا ہے (کوئی اضافی انحصار نہیں) اور کام کرتا ہے جہاں `python3` دستیاب ہے۔
  وشوسنییتا کے لئے مرکزی ذخیرے میں سی ایل آئی کے ساتھ وابستگی کی تاریخ کا سراغ لگایا جاتا ہے۔

پیداوار کے ل ، ، نتیجے میں ہونے والے مینی فیسٹ اور این ڈی جےسن پیکیج کو لاگر ٹکٹ سے جوڑیں تاکہ اسٹیورڈز
Torii پر بھیجے گئے عین مطابق پے لوڈ کو دوبارہ شروع کرنے سے۔