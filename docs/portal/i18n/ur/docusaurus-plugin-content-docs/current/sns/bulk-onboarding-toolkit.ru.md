---
lang: ur
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sns/bulk_onboarding_toolkit.md` کی عکاسی کرتا ہے تاکہ بیرونی
آپریٹرز نے وہی SN-3B سفارشات دیکھے بغیر ذخیرہ کو کلون کیے بغیر۔
:::

# SNS بلک آن بورڈنگ ٹول کٹ (SN-3B)

** روڈ میپ لنک: ** SN-3B "بلک آن بورڈنگ ٹولنگ"  
** نمونے: ** `scripts/sns_bulk_onboard.py` ، `scripts/tests/test_sns_bulk_onboard.py` ،
`docs/portal/scripts/sns_bulk_release.sh`

بڑے رجسٹرار اکثر سیکڑوں رجسٹریشن `.sora` یا تیار کرتے ہیں
`.nexus` اسی انتظامیہ اور تصفیہ ریلوں کی منظوری کے ساتھ۔ دستی اسمبلی
JSON پے لوڈز یا CLI ریرنس پیمائش نہیں کرتے ہیں ، لہذا SN-3B سپلائی
تعصب CSV-to-Norito بلڈر جو ڈھانچے کو تیار کرتا ہے
`RegisterNameRequestV1` برائے Torii یا CLI۔ مددگار ہر لائن کو پہلے سے توثیق کرتا ہے ،
مجموعی مینی فیسٹ اور اختیاری لائن بائی لائن JSON تیار کرتا ہے ، اور بھیج سکتا ہے
آڈٹ کے لئے ساختی رسیدوں کی ریکارڈنگ ، خود بخود پے لوڈ۔

## 1. CSV اسکیما

پارسر کو درج ذیل ہیڈر لائن کی ضرورت ہوتی ہے (آرڈر لچکدار ہے):

| کالم | مطلوب | تفصیل |
| --------- | ------------ | ---------- |
| `label` | ہاں | مطلوبہ لیبل (مخلوط کیس کی اجازت ہے the ٹول نورم V1 اور UTS-46 کے مطابق معمول بناتا ہے)۔ |
| `suffix_id` | ہاں | عددی لاحقہ شناخت کنندہ (اعشاریہ یا `0x` ہیکس)۔ |
| `owner` | ہاں | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | ہاں | انٹیجر `1..=255`۔ |
| `payment_asset_id` | ہاں | تصفیہ اثاثہ (مثال کے طور پر `61CtjvNd9T3THAR65GsMVHr82Bjc`)۔ |
| `payment_gross` / `payment_net` | ہاں | اثاثہ کے اکائیوں کی نمائندگی کرنے والے دستخط شدہ عدد۔ |
| `settlement_tx` | ہاں | ادائیگی کے لین دین یا ہیش کو بیان کرتے ہوئے JSON قدر یا تار۔ |
| `payment_payer` | ہاں | اکاؤنٹ آئی ڈی جس نے ادائیگی کو اختیار دیا۔ |
| `payment_signature` | ہاں | JSON یا اسٹیورڈ یا ٹریژری دستخطی پروف سٹرنگ۔ |
| `controllers` | اختیاری | `;` یا `,` کے ذریعہ الگ کردہ کنٹرولر پتے کی فہرست۔ پہلے سے طے شدہ `[owner]` ہے۔ |
| `metadata` | اختیاری | ان لائن JSON یا `@path/to/file.json` کے ساتھ حل کرنے والے اشارے ، TXT ریکارڈز وغیرہ کے ساتھ ڈیفالٹ `{}` کے ذریعہ۔ |
| `governance` | اختیاری | ان لائن JSON یا `@path` سے `GovernanceHookV1`۔ `--require-governance` کالم کو لازمی بناتا ہے۔ |

کوئی بھی کالم ویلیو کے آغاز میں `@` شامل کرکے بیرونی فائل کا حوالہ دے سکتا ہے۔
CSV فائل کے نسبت راستے حل کیے جاتے ہیں۔

## 2. مددگار لانچ کریں

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

کلیدی اختیارات:

- `--require-governance` بغیر گورننس ہک کے لائنوں کو مسترد کرتا ہے (اس کے لئے مفید ہے
  پریمیم نیلامی یا محفوظ اسائنمنٹس)۔
- `--default-controllers {owner,none}` فیصلہ کرتا ہے کہ آیا کنٹرولر خلیات خالی ہوں گے یا نہیں
  مالک کے پاس واپس گریں۔
- `--controllers-column` ، `--metadata-column` ، اور `--governance-column` اجازت دیں
  اپ اسٹریم برآمدات کے ساتھ کام کرتے وقت اختیاری کالموں کا نام تبدیل کریں۔

اگر کامیاب ہو تو ، اسکرپٹ ایک مجموعی مینی فیسٹ لکھتا ہے:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<katakana-i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<katakana-i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<katakana-i105-account-id>",
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

اگر `--ndjson` کی وضاحت کی گئی ہے تو ، ہر `RegisterNameRequestV1` بھی لکھا گیا ہے
ون لائن JSON دستاویز تاکہ آٹومیشن براہ راست درخواستوں کو آگے بڑھا سکے
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```## 3. خودکار ترسیل

### 3.1 Torii REST موڈ

`--submit-torii-url` اور یا تو `--submit-token` یا `--submit-token-file` کی وضاحت کریں ،
ہر مینی فیسٹ انٹری کو براہ راست Torii پر بھیجنے کے لئے:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- مددگار ایک `POST /v1/sns/names` فی درخواست کرتا ہے اور رک جاتا ہے جب
  پہلی HTTP غلطی۔ جوابات کو NDJSON ریکارڈ کے بطور لاگ میں شامل کیا جاتا ہے۔
- `--poll-status` درخواست کرتا ہے `/v1/sns/names/{namespace}/{literal}` کے بعد ایک بار پھر
  تصدیق کرنے کے لئے ہر ایک بھیجیں (`--poll-attempts` ، پہلے سے طے شدہ 5)
  ریکارڈ کی نمائش۔ `--suffix-map` (JSON میپنگ `suffix_id` کو اقدار پر بتائیں
  "لاحقہ") تاکہ ٹول پولنگ کے لئے `{label}.{suffix}` آؤٹ پٹ کر سکے۔
- ترتیبات: `--submit-timeout` ، `--poll-attempts` ، اور `--poll-interval`۔

### 3.2 Iroha Cli وضع

سی ایل آئی کے ذریعے ہر ظاہر اندراج کو چلانے کے لئے ، بائنری کا راستہ طے کریں:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- کنٹرولرز `Account` اندراجات (`controller_type.kind = "Account"`) ہونا چاہئے ،
  کیونکہ CLI فی الحال صرف اکاؤنٹ پر مبنی کنٹرولرز کی حمایت کرتا ہے۔
- میٹا ڈیٹا اور گورننس بلبس ہر درخواست کے لئے عارضی فائلوں پر لکھے گئے ہیں اور
  `iroha sns register --metadata-json ... --governance-json ...` پر بھیجے جاتے ہیں۔
- stdout/stderr اور CLI سے باہر نکلنے والے کوڈ لاگ ان ہیں۔ غیر صفر کوڈ لانچ کو ختم کرتے ہیں۔

دونوں بھیجنے کے طریقوں کو کراس توثیق کے لئے ایک ساتھ چلایا جاسکتا ہے (Torii اور CLI)
رجسٹرار کی تعیناتی یا فال بیک راستوں کی مشقیں۔

### 3.3 شپنگ رسیدیں

`--submission-log <path>` پر اسکرپٹ میں NDJSON ریکارڈ شامل کیا گیا ہے:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

کامیاب Torii ردعمل میں `NameRecordV1` یا سے ساختی فیلڈز شامل ہیں
`RegisterNameResponseV1` (مثال کے طور پر `record_status` ، `record_pricing_class` ،
`record_owner` ، `record_expires_at_ms` ، `registry_event_version` ، `suffix_id` ،
`label`) ، تاکہ ڈیش بورڈز اور مینجمنٹ رپورٹس بغیر بغیر مفت تجزیہ کرسکیں
متن اس لاگ کو رجسٹرار کے ٹکٹوں کے ساتھ ساتھ منشور کے ساتھ منسلک کریں
تولیدی ثبوت

## 4. پورٹل ریلیز کا آٹومیشن

CI اور پورٹل ملازمتیں `docs/portal/scripts/sns_bulk_release.sh` پر کال کرتی ہیں ، جو
مددگار کو لپیٹتا ہے اور `artifacts/sns/releases/<timestamp>/` کے تحت نمونے اسٹور کرتا ہے:

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

1. `registrations.manifest.json` ، `registrations.ndjson` اور کاپیاں تخلیق کرتا ہے
   ریلیز ڈائرکٹری کا اصل CSV۔
2. Torii اور/یا CLI (جب تشکیل شدہ) کے ذریعے منشور بھیجتا ہے ، تحریر
   `submissions.log` اوپر ساختہ رسیدوں کے ساتھ۔
3. ریلیز کی تفصیل کے ساتھ `summary.json` تیار کرتا ہے (راستے ، Torii URL ، CLI PATH ،
   ٹائم اسٹیمپ) تاکہ پورٹل آٹومیشن بنڈل کو اسٹوریج میں لاد سکے
   نمونے
4. `metrics.prom` پیدا کرتا ہے (`--metrics` کے ذریعے اوور رائڈ) Prometheus-مطابقت کے ساتھ
   درخواستوں کی کل تعداد ، لاحقہ کی تقسیم ، اثاثہ کے ذریعہ مقدار کے لئے کاؤنٹر
   اور نتائج بھیجنا۔ خلاصہ JSON اس فائل کا حوالہ دیتا ہے۔

ورک فلوز صرف ایک ہی نمونے کے طور پر ریلیز ڈائرکٹری کو محفوظ شدہ دستاویزات پر مشتمل ہے
آڈٹ کے لئے ضروری ہر چیز۔

## 5. ٹیلی میٹری اور ڈیش بورڈز

`sns_bulk_release.sh` کے ذریعہ تیار کردہ میٹرکس فائل میں مندرجہ ذیل سیریز موجود ہے:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
````metrics.prom` کو Prometheus SIDECAR پر پاس کریں (مثال کے طور پر پرومٹیل یا بیچ کے ذریعے
درآمد کنندہ) تاکہ رجسٹرار ، اسٹیورڈز اور گورننس کے ساتھی متفق ہوں
بلک عمل کی پیشرفت۔ ڈیش بورڈ Grafana
`dashboards/grafana/sns_bulk_release.json` ایک ہی ڈیٹا کو تصور کرتا ہے: مقدار
لاحقہ کے ذریعہ ، ادائیگیوں کا حجم اور کامیاب/ناکام ترسیل کا تناسب۔ ڈیش بورڈ
`release` کے ذریعہ فلٹر کیا گیا تاکہ آڈیٹر ایک ہی CSV رن کا معائنہ کرسکیں۔

## 6. توثیق اور ناکامی کے طریقوں

۔
  کریکٹر فلٹرز نورم V1۔ غلط ٹیگس تیزی سے نیٹ ورک کالز پر گرتے ہیں۔
- ** عددی محافظ: ** لاحقہ آئی ڈی ، ٹرم سال ، اور قیمتوں کے اشارے میں ہونا ضروری ہے
  `u16` اور `u8` کے اندر۔ ادائیگی کے شعبے اعشاریہ یا ہیکس نمبروں کو قبول کرتے ہیں
  `i64::MAX`۔
- ** میٹا ڈیٹا/گورننس پارسنگ: ** ان لائن JSON براہ راست تجزیہ کیا گیا ہے۔ فائلوں سے لنک
  CSV کے نسبت حل کیا جاتا ہے۔ میٹا ڈیٹا غیر آبجیکٹ کے نتیجے میں توثیق کی غلطی ہوتی ہے۔
- ** کنٹرولرز: ** خالی خلیات `--default-controllers` کا احترام کرتے ہیں۔ براہ کرم اشارہ کریں
  جب غیر مالک کو تفویض کرتے وقت کنٹرولرز کی واضح فہرستیں (مثال کے طور پر `<katakana-i105-account-id>;<katakana-i105-account-id>`)۔

غلطیوں کی اطلاع سیاق و سباق کے نمبروں کے ساتھ کی جاتی ہے (جیسے
`error: row 12 term_years must be between 1 and 255`)۔ اسکرپٹ کوڈ `1` کے ساتھ باہر ہے
توثیق کی غلطیوں اور `2` کے لئے اگر CSV راستہ غائب ہے۔

## 7. جانچ اور پروویژن

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` CSV تجزیہ کرنے کا احاطہ کرتا ہے ،
  این ڈی جےسن آؤٹ پٹ ، انفورسمنٹ گورننس اور CLI یا Torii کے ذریعے راستے بھیجنا۔
- مددگار خالص ازگر میں لکھا گیا ہے (اضافی انحصار کے بغیر) اور کام کرتا ہے
  جہاں بھی `python3` دستیاب ہے۔ کمٹ ہسٹری کو سی ایل آئی کے ساتھ ہی ٹریک کیا جاتا ہے
  تولیدی صلاحیت کے لئے بنیادی طور پر ذخیرے۔

پروڈکشن لانچوں کے لئے ، پیدا شدہ مینی فیسٹ اور این ڈی جےسن بنڈل کو منسلک کریں
رجسٹرار ٹکٹ تاکہ اسٹیورڈ بھیجے گئے عین مطابق پے لوڈ کو دوبارہ پیش کرسکیں
Torii میں۔