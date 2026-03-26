---
lang: he
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sns/bulk_onboarding_toolkit.md` کی عکاسی کرتا ہے تاکہ بیرونی
آپریٹرز ریپو کلون کئے بغیر وہی SN-3b رہنمائی دیکھ سکیں۔
:::

# ערכת הכלים לכניסה לכמות גדולה של SNS (SN-3b)

**روڈمیپ حوالہ:** SN-3b "Bulk onboarding tooling"  
**חפצי אמנות:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

רשמים של `.sora` רישומים `.nexus` או ממשל
approvals اور settlement rails کے ساتھ سیکڑوں کی تعداد میں پہلے سے تیار کرتے
ہیں۔ دستی طور پر JSON payloads بنانا یا CLI دوبارہ چلانا scale نہیں کرتا، اس
لئے SN-3b ایک deterministic CSV-to-Norito builder فراہم کرتا ہے جو Torii یا CLI
کے لئے `RegisterNameRequestV1` structures تیار کرتا ہے۔ helper ہر row کو پہلے
validate کرتا ہے، aggregated manifest اور optional newline-delimited JSON خارج
کرتا ہے، اور audits کے لئے structured receipts ریکارڈ کرتے ہوئے payloads کو
خودکار طور پر submit کر سکتا ہے۔

## 1. CSV اسکیمہ

Parser کو درج ذیل header row درکار ہے (order flexible ہے):

| עמודה | חובה | תיאור |
|--------|--------|----------------|
| `label` | כן | Requested label (mixed case accepted; tool Norm v1 اور UTS-46 کے مطابق normalize کرتا ہے). |
| `suffix_id` | כן | Numeric suffix identifier (decimal یا `0x` hex). |
| `owner` | כן | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | כן | מספר שלם `1..=255`. |
| `payment_asset_id` | כן | Settlement asset (مثال `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | כן | Unsigned integers جو asset-native units کو represent کریں۔ |
| `settlement_tx` | כן | JSON value یا literal string جو payment transaction یا hash بیان کرے۔ |
| `payment_payer` | כן | AccountId جس نے payment authorize کی۔ |
| `payment_signature` | כן | JSON یا literal string جس میں steward/treasury signature proof ہو۔ |
| `controllers` | אופציונלי | Controller account addresses کی semicolon/comma-separated list۔ خالی ہونے پر `[owner]`. |
| `metadata` | אופציונלי | Inline JSON یا `@path/to/file.json` جو resolver hints، TXT records وغیرہ دے۔ ברירת מחדל `{}`. |
| `governance` | אופציונלי | Inline JSON یا `@path` جو `GovernanceHookV1` کی طرف ہو۔ `--require-governance` اس column کو لازمی کرتا ہے۔ |

کوئی بھی column سیل ویلیو میں `@` لگا کر external file کو refer کر سکتا ہے۔
Paths CSV file کے relative resolve ہوتے ہیں۔

## 2. עוזר

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

אפשרויות עיקריות:

- `--require-governance` governance hook کے بغیر rows reject کرتا ہے (premium
  auctions یا reserved assignments کے لئے مفید).
- `--default-controllers {owner,none}` طے کرتا ہے کہ خالی controller cells owner
  account پر واپس جائیں یا نہیں.
- `--controllers-column`, `--metadata-column`, אוור `--governance-column` במעלה הזרם
  exports کے ساتھ کام کرتے ہوئے optional columns کے نام بدلنے دیتے ہیں.

کامیابی پر script aggregated manifest لکھتا ہے:

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

اگر `--ndjson` دیا جائے تو ہر `RegisterNameRequestV1` کو single-line JSON
document کے طور پر بھی لکھا جاتا ہے تاکہ automations requests کو براہ راست Torii
זרם מוזיקה:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```## 3. הגשות אוטומטיות

### 3.1 Torii מצב REST

`--submit-torii-url` کے ساتھ `--submit-token` یا `--submit-token-file` دیں تاکہ
manifest کی ہر entry براہ راست Torii کو push ہو:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Helper ہر request کے لئے `POST /v1/sns/names` بھیجتا ہے اور پہلی HTTP
  error پر abort کرتا ہے۔ Responses log path میں NDJSON records کے طور پر append
  ہوتے ہیں۔
- `--poll-status` ہر submission کے بعد `/v1/sns/names/{namespace}/{literal}` کو
  دوبارہ query کرتا ہے (زیادہ سے زیادہ `--poll-attempts`, default 5) تاکہ record
  visible ہونے کی تصدیق ہو۔ `--suffix-map` (JSON או `suffix_id` ערכי "סיומת"
  سے map کرے) فراہم کریں تاکہ tool `{label}.{suffix}` literals derive کر سکے۔
- מתכוונים: `--submit-timeout`, `--poll-attempts`, `--poll-interval`.

### מצב 3.2 iroha CLI

ہر manifest entry کو CLI سے گزارنے کے لئے binary path دیں:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Controllers کو `Account` entries ہونا چاہئے (`controller_type.kind = "Account"`)
  کیونکہ CLI فی الحال صرف account-based controllers expose کرتا ہے۔
- Metadata اور governance blobs ہر request کے لئے temporary files میں لکھے جاتے
  ہیں اور `iroha sns register --metadata-json ... --governance-json ...` کو پاس
  ‏
- CLI stdout/stderr اور exit codes log ہوتے ہیں؛ non-zero codes run کو abort کرتے ہیں۔

دونوں submission modes ایک ساتھ چل سکتے ہیں (Torii اور CLI) تاکہ registrar
deployments cross-check ہوں یا fallbacks rehearse کئے جائیں۔

### 3.3 קבלות על הגשה

جب `--submission-log <path>` فراہم کیا جائے تو script NDJSON entries append کرتا ہے:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Successful Torii responses میں `NameRecordV1` یا `RegisterNameResponseV1` سے نکالے
گئے structured fields شامل ہوتے ہیں (مثال `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`) تاکہ dashboards اور governance reports log کو free-form text دیکھے بغیر
parse کر سکیں۔ اس log کو manifest کے ساتھ registrar tickets پر attach کریں تاکہ
reproducible evidence رہے۔

## 4. אוטומציה של פורטל Docs

CI اور portal jobs `docs/portal/scripts/sns_bulk_release.sh` کو call کرتے ہیں، جو
helper کو wrap کرتا ہے اور artefacts کو `artifacts/sns/releases/<timestamp>/` کے
تحت store کرتا ہے:

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

תסריט:

1. `registrations.manifest.json`, `registrations.ndjson` بناتا ہے اور original
   CSV کو release directory میں کاپی کرتا ہے۔
2. Torii اور/یا CLI کے ذریعے manifest submit کرتا ہے (جب configure ہو)، اور
   `submissions.log` میں اوپر والے structured receipts لکھتا ہے۔
3. `summary.json` emit کرتا ہے جو release کو describe کرتا ہے (paths, Torii URL,
   CLI path, timestamp) تاکہ portal automation bundle کو artefact storage پر
   upload کر سکے۔
4. `metrics.prom` بناتا ہے (`--metrics` کے ذریعے override)، جس میں Prometheus-
   פורמט מונים: סך כל הבקשות, הפצת סיומת, סכומי נכסים,
   תוצאות הגשה. summary JSON اس file کی طرف link کرتا ہے۔

Workflows صرف release directory کو ایک artefact کے طور پر archive کرتے ہیں، جس میں
اب وہ سب کچھ ہے جو governance کو audit کے لئے درکار ہے۔

## 5. לוחות מחוונים של טלמטריה אור

`sns_bulk_release.sh` کے ذریعہ بننے والی metrics file درج ذیل series expose کرتی ہے:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
````metrics.prom` کو اپنے Prometheus sidecar میں feed کریں (مثال کے طور پر Promtail
یا batch importer کے ذریعے) تاکہ registrars, stewards اور governance peers bulk
progress پر aligned رہیں۔ לוח Grafana
`dashboards/grafana/sns_bulk_release.json` לוחות נתונים ומיידיים: לכל סיומת
ספירות, נפח תשלום, או יחסי הצלחה/כישלון של הגשה. לוח `release`
filter کرتا ہے تاکہ auditors ایک CSV run پر drill کر سکیں۔

## 6. אימות או מצבי כשל

- **Label canonicalisation:** inputs Python IDNA کے ساتھ lowercase اور Norm v1
  character filters سے normalise ہوتے ہیں۔ invalid labels network calls سے پہلے
  fail fast ہوتے ہیں۔
- **מעקות בטיחות מספריים:** מזהי סיומת, שנות טווח או רמזים לתמחור `u16` או `u8`
  bounds کے اندر ہونے چاہئیں۔ Payment fields decimal یا hex integers `i64::MAX`
  تک accept کرتے ہیں۔
- **Metadata یا governance parsing:** inline JSON براہ راست parse ہوتا ہے؛ קובץ
  references CSV location کے relative resolve ہوتی ہیں۔ מטא נתונים שאינם אובייקטים
  validation error دیتا ہے۔
- **Controllers:** خالی cells `--default-controllers` کو honour کرتے ہیں۔ לא בעלים
  actors کو delegate کرتے وقت explicit controller lists دیں (مثال `<katakana-i105-account-id>;<katakana-i105-account-id>`)۔

Failures contextual row numbers کے ساتھ report ہوتے ہیں (مثال
`error: row 12 term_years must be between 1 and 255`). Script validation errors پر
`1` اور CSV path missing ہونے پر `2` کے ساتھ exit کرتا ہے۔

## 7. בדיקת מקור מקור

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` ניתוח CSV, NDJSON
  emission، governance enforcement، اور CLI/Torii submission paths کو cover کرتا ہے۔
- Helper pure Python ہے (کوئی اضافی dependencies نہیں) اور جہاں `python3` دستیاب
  ہو وہاں چلتا ہے۔ Commit history CLI کے ساتھ main repository میں track ہوتی ہے
  تاکہ reproducibility ہو۔

Production runs کے لئے، generated manifest اور NDJSON bundle کو registrar ticket کے
ساتھ attach کریں تاکہ stewards Torii کو submit ہونے والے exact payloads replay
کر سکیں۔