---
lang: ur
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
آئینہ `docs/source/sns/bulk_onboarding_toolkit.md` تاکہ بیرونی آپریٹرز
ذخیرہ کو کلون کیے بغیر ایک ہی SN-3B واقفیت دیکھیں۔
:::

# SNS بلک آن بورڈنگ ٹول کٹ (SN-3B)

** روڈ میپ حوالہ: ** SN-3B "بلک آن بورڈنگ ٹولنگ"  
** نمونے: ** `scripts/sns_bulk_onboard.py` ، `scripts/tests/test_sns_bulk_onboard.py` ،
`docs/portal/scripts/sns_bulk_release.sh`

بڑے رجسٹرار اکثر سیکڑوں `.sora` یا تیار کرتے ہیں
اسی گورننس کی منظوری اور تصفیہ ریلوں کے ساتھ `.nexus`۔ جمع
JSON پے لوڈ دستی طور پر یا CLI کو دوبارہ جاری کرنا پیمانہ نہیں کرتا ہے ، لہذا SN-3B فراہم کرتا ہے
Norito کے لئے تیار کردہ ڈھانچے کی تیاری کے لئے ڈٹرمینسٹک CSV بلڈر
`RegisterNameRequestV1` to Torii یا CLI کو۔ مددگار ہر لائن کی توثیق کرتا ہے
پیشگی ، ایک مجموعی منشور اور JSON دونوں کو خارج کرتا ہے جس کی طرف سے
اختیاری لائن ریپنگ ، اور جب پے لوڈز خود بخود بھیج سکتی ہے
آڈٹ کے لئے ریکارڈ ساختی رسیدیں۔

## 1. CSV اسکیما

پارسر کو درج ذیل ہیڈر لائن کی ضرورت ہوتی ہے (آرڈر لچکدار ہے):

| کالم | لازمی | تفصیل |
| ----------- | ------------- | ----------- |
| `label` | ہاں | لیبل کی درخواست کی گئی (مخلوط کیس قبول ہو گیا tool ٹول نورم V1 اور UTS-46 کے مطابق معمول بناتا ہے)۔ |
| `suffix_id` | ہاں | عددی لاحقہ شناخت کنندہ (اعشاریہ یا `0x` ہیکس)۔ |
| `owner` | ہاں | AccountId string (domainless encoded literal; canonical i105 only; no `@<domain>` suffix). |
| `term_years` | ہاں | انٹیجر `1..=255`۔ |
| `payment_asset_id` | ہاں | تصفیہ اثاثہ (مثال کے طور پر `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `payment_gross` / `payment_net` | ہاں | اثاثہ کے مقامی اکائیوں کی نمائندگی کرنے والے دستخط شدہ عدد۔ |
| `settlement_tx` | ہاں | ادائیگی کے لین دین یا ہیش کو بیان کرنے والے JSON قدر یا لفظی تار۔ |
| `payment_payer` | ہاں | اکاؤنٹ آئی ڈی جس نے ادائیگی کو اختیار دیا۔ |
| `payment_signature` | ہاں | JSON OU STRING LITALL Contendo a prova de assinatora do steave Ou tesoraria. |
| `controllers` | اختیاری | سیمیکولون یا کوما سے الگ الگ کنٹرولر اکاؤنٹ کے پتے کی فہرست۔ ڈیفالٹ `[owner]` جب چھوڑ دیا جاتا ہے۔ |
| `metadata` | اختیاری | ان لائن JSON یا `@path/to/file.json` حل کے اشارے ، TXT ریکارڈز ، وغیرہ فراہم کرنا معیاری `{}`۔ |
| `governance` | اختیاری | ان لائن JSON یا `@path` `GovernanceHookV1` کی طرف اشارہ کررہا ہے۔ `--require-governance` کو اس کالم کی ضرورت ہے۔ |

کوئی بھی کالم `@` کے ساتھ سیل ویلیو کو پہلے سے تیار کرکے بیرونی فائل کا حوالہ دے سکتا ہے۔
CSV فائل کے نسبت راستے حل کیے جاتے ہیں۔

## 2. مددگار چلائیں

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

اہم اختیارات:

- `--require-governance` بغیر گورننس ہک کے لائنوں کو مسترد کرتا ہے (اس کے لئے مفید ہے
  پریمیم نیلامی یا محفوظ اسائنمنٹس)۔
- `--default-controllers {owner,none}` فیصلہ کرتا ہے کہ آیا خالی کنٹرولر خلیات
  مالک کے اکاؤنٹ میں واپس جائیں۔
- `--controllers-column` ، Torii ، اور `--governance-column` اجازت دیں
  اپ اسٹریم برآمدات کے ساتھ کام کرتے وقت اختیاری کالم کا نام تبدیل کرنا۔

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
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
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
```اگر `--ndjson` دیا گیا ہے تو ، ہر `RegisterNameRequestV1` بھی لکھا جاتا ہے
ایک واحد لائن JSON دستاویز تاکہ آٹومیشن درخواستوں کو منتقل کرسکیں
براہ راست Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. خودکار گذارشات

### 3.1 Torii REST موڈ

`--submit-torii-url` پلس `--submit-token` یا `--submit-token-file` کی وضاحت کریں
ہر مینی فیسٹ انٹری کو براہ راست Torii پر بھیجنے کے لئے:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- مددگار ایک `POST /v1/sns/names` فی درخواست جاری کرتا ہے اور پہلے اس کو ختم کرتا ہے
  HTTP غلطی. جوابات لاگ میں این ڈی جےسن ریکارڈ کے طور پر شامل کیے جاتے ہیں۔
- `--poll-status` ہر بھیجنے کے بعد `/v1/sns/names/{namespace}/{literal}` دوبارہ کدنتا ہے
  (`--poll-attempts` تک ، پہلے سے طے شدہ 5) اس بات کی تصدیق کرنے کے لئے کہ ریکارڈ نظر آتا ہے۔
  `--suffix-map` ("لاحقہ" اقدار کے لئے `suffix_id` کا JSON) فراہم کریں تاکہ
  پولنگ کے لئے ٹول اخذ `{label}.{suffix}` لغوی۔
- ترتیبات: `--submit-timeout` ، `--poll-attempts` ، اور `--poll-interval`۔

### 3.2 Iroha Cli وضع

سی ایل آئی کے ذریعے ہر ظاہر ہونے والے اندراج کو روٹ کرنے کے لئے ، بائنری راستہ فراہم کریں:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- کنٹرولرز کے پاس ان پٹ `Account` (`controller_type.kind = "Account"`) ہونا ضروری ہے
  کیونکہ CLI فی الحال صرف اکاؤنٹ پر مبنی کنٹرولرز کو بے نقاب کرتا ہے۔
- میٹا ڈیٹا اور گورننس بلبس فی درخواست عارضی فائلوں پر لکھے گئے ہیں
  اور `iroha sns register --metadata-json ... --governance-json ...` پر بھیج دیا گیا۔
- CLI stdout اور stderr ، نیز خارجی کوڈ ، رجسٹرڈ ہیں۔ کوڈ نمبر
  زیرو اسقاط حمل۔

دونوں جمع کرانے کے طریقوں کو ایک ساتھ چلایا جاسکتا ہے (Torii اور CLI) چیک کرنے کے لئے
تعیناتیوں کو رجسٹر کریں یا فال بیکس کی مشق کریں۔

### 3.3 جمع کرانے کی رسیدیں

جب `--submission-log <path>` فراہم کیا جاتا ہے تو ، اسکرپٹ NDJSON اندراجات میں شامل ہوتا ہے
گرفتاری:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

کامیاب Torii جوابات میں ساختہ فیلڈز شامل ہیں جن سے لیا گیا ہے
`NameRecordV1` یا `RegisterNameResponseV1` (مثال کے طور پر `record_status` ،
`record_pricing_class` ، `record_owner` ، `record_expires_at_ms` ،
`registry_event_version` ، `suffix_id` ، `label`) ڈیش بورڈز اور رپورٹس کے لئے
گورننس سسٹم مفت متن کا معائنہ کیے بغیر لاگ ان کی تجزیہ کرسکتے ہیں۔ اس لاگ کو منسلک کریں
تولیدی شواہد کے لئے مینی فیسٹ کے ساتھ ٹکٹوں کو ریکارڈ کرنا ضروری ہے۔

## 4. پورٹل ریلیز آٹومیشن

CI اور پورٹل ملازمتیں `docs/portal/scripts/sns_bulk_release.sh` پر کال کرتی ہیں ، جو
مددگار کو گھیراتا ہے اور اس کے تحت نمونے اسٹور کرتا ہے
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

اسکرپٹ:

1. `registrations.manifest.json` ، `registrations.ndjson` بنائیں ، اور CSV کاپی کریں
   ریلیز ڈائرکٹری سے اصل۔
2. Torii اور/یا CLI (جب تشکیل شدہ) کا استعمال کرتے ہوئے مینی فیسٹ جمع کروائیں ، بچت کریں
   `submissions.log` اوپر کی تشکیل شدہ رسیدوں کے ساتھ۔
3. جاری کریں `summary.json` ریلیز کی وضاحت (راستے ، Torii URL ، ریلیز کا راستہ
   سی ایل آئی ، ٹائم اسٹیمپ) تاکہ پورٹل آٹومیشن بنڈل کو بھیج سکے
   نوادرات کا اسٹوریج۔
4. `metrics.prom` تیار کرتا ہے (`--metrics` کے ذریعے اوور رائڈ) جس میں مطابقت پذیر کاؤنٹرز ہوتے ہیں
   کل درخواستوں ، لاحقہ تقسیم ، اثاثہ جات کے لئے Prometheus کے ساتھ
   اور پیش کرنے کے نتائج۔ خلاصہ JSON اس فائل کی طرف اشارہ کرتا ہے۔ورک فلوز صرف ایک ہی نمونے کے طور پر ریلیز ڈائرکٹری کو محفوظ کرتے ہیں ،
جس میں اب ہر چیز پر مشتمل ہے جو حکمرانی کو آڈیٹ کرنے کے لئے درکار ہے۔

## 5. ٹیلی میٹری اور ڈیش بورڈز

`sns_bulk_release.sh` کے ذریعہ تیار کردہ میٹرکس فائل مندرجہ ذیل سیریز کو بے نقاب کرتی ہے:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom` کو اپنے Prometheus sidecar (جیسے پرومٹیل کے ذریعے یا میں فیڈ کریں
ایک بیچ درآمد کنندہ) رجسٹرار ، اسٹیورڈز اور گورننس کے ساتھیوں کو برقرار رکھنے کے لئے
بڑے پیمانے پر پیشرفت کے بارے میں منسلک. فریم Grafana
`dashboards/grafana/sns_bulk_release.json` ڈیش بورڈز کے ساتھ ایک ہی ڈیٹا کا تصور کرتا ہے
لاحقہ ، ادائیگی کا حجم اور کامیابی/ناکامی کے تناسب کے حساب سے
گذارشات `release` کے ذریعہ ٹیبل فلٹرز تاکہ آڈیٹر ایک پر توجہ مرکوز کرسکیں
CSV کی سنگل پھانسی۔

## 6. توثیق اور ناکامی کے طریقوں

- ** لیبل کیننیکلائزیشن: ** اندراجات کو ازگر IDNA کے ساتھ معمول بنایا جاتا ہے
  لوئر کیس اور کریکٹر فلٹر نورم V1۔ غلط لیبل تیزی سے ناکام ہوجاتے ہیں
  کسی بھی نیٹ ورک کال سے پہلے۔
- ** عددی محافظ: ** لاحقہ آئی ڈی ، ٹرم سال اور قیمتوں کے اشارے باقی رہنا چاہئے
  `u16` اور `u8` کی حدود میں۔ ادائیگی کے شعبے اعشاریہ عدد کو قبول کرتے ہیں
  یا `i64::MAX` تک ہیکس۔
- ** میٹا ڈیٹا یا گورننس پارسنگ: ** JSON ان لائن اور براہ راست تجزیہ کیا گیا۔
  فائل کے حوالہ جات CSV مقام کے مطابق حل کیے جاتے ہیں۔ میٹا ڈیٹا
  کوئی شے توثیق کی غلطی پیدا نہیں کرتا ہے۔
- ** کنٹرولرز: ** خالی خلیات `--default-controllers` کا احترام کرتے ہیں۔ فراہمی
  غیر مالک اداکاروں کو تفویض کرتے وقت واضح فہرستیں (جیسے `i105...;i105...`)۔

کریشوں کو سیاق و سباق کے نمبروں کے ساتھ بتایا جاتا ہے (جیسے۔
`error: row 12 term_years must be between 1 and 255`)۔ اسکرپٹ کوڈ کے ساتھ سامنے آتا ہے
`1` توثیق کی غلطیوں اور `2` پر جب CSV راستہ غائب ہے۔

## 7. ٹیسٹ اور پروویژن

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` CSV تجزیہ کا احاطہ کرتا ہے ،
  این ڈی جےسن جاری کرنا ، گورننس انفورسمنٹ اور جمع کرنے والے راستے CLI یا Torii کے ذریعے۔
- مددگار خالص ازگر ہے (اضافی انحصار کے بغیر) اور کہیں بھی چلتا ہے
  جہاں `python3` دستیاب ہے۔ ارتکاب کی تاریخ کو ایک ساتھ ٹریک کیا جاتا ہے
  تولیدی صلاحیت کے لئے مرکزی ذخیرہ میں سی ایل آئی۔

پروڈکشن رنز کے لئے ، پیدا شدہ مینی فیسٹ اور این ڈی جےسن بنڈل کو رب سے جوڑیں
رجسٹر کریں تاکہ اسٹیورڈز عین مطابق پے لوڈ کو دوبارہ پیش کرسکیں جو تھے
Torii میں پیش کیا گیا۔