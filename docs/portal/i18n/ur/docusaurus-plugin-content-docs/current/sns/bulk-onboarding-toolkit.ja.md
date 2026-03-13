---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sns/bulk-onboarding-toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03be5aec8821d1ac08d32fc0bfb6e53fda568bd93fa715f82e232857710a1654
source_last_modified: "2026-01-22T15:55:18+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: bulk-onboarding-toolkit
lang: ur
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sns/bulk_onboarding_toolkit.md` کی عکاسی کرتا ہے تاکہ بیرونی
آپریٹرز ریپو کلون کئے بغیر وہی SN-3b رہنمائی دیکھ سکیں۔
:::

# SNS Bulk Onboarding Toolkit (SN-3b)

**روڈمیپ حوالہ:** SN-3b "Bulk onboarding tooling"  
**Artefacts:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

بڑے registrars اکثر `.sora` یا `.nexus` registrations کو ایک ہی governance
approvals اور settlement rails کے ساتھ سیکڑوں کی تعداد میں پہلے سے تیار کرتے
ہیں۔ دستی طور پر JSON payloads بنانا یا CLI دوبارہ چلانا scale نہیں کرتا، اس
لئے SN-3b ایک deterministic CSV-to-Norito builder فراہم کرتا ہے جو Torii یا CLI
کے لئے `RegisterNameRequestV1` structures تیار کرتا ہے۔ helper ہر row کو پہلے
validate کرتا ہے، aggregated manifest اور optional newline-delimited JSON خارج
کرتا ہے، اور audits کے لئے structured receipts ریکارڈ کرتے ہوئے payloads کو
خودکار طور پر submit کر سکتا ہے۔

## 1. CSV اسکیمہ

Parser کو درج ذیل header row درکار ہے (order flexible ہے):

| Column | Required | Description |
|--------|----------|-------------|
| `label` | Yes | Requested label (mixed case accepted; tool Norm v1 اور UTS-46 کے مطابق normalize کرتا ہے). |
| `suffix_id` | Yes | Numeric suffix identifier (decimal یا `0x` hex). |
| `owner` | Yes | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Yes | Integer `1..=255`. |
| `payment_asset_id` | Yes | Settlement asset (مثال `xor#sora`). |
| `payment_gross` / `payment_net` | Yes | Unsigned integers جو asset-native units کو represent کریں۔ |
| `settlement_tx` | Yes | JSON value یا literal string جو payment transaction یا hash بیان کرے۔ |
| `payment_payer` | Yes | AccountId جس نے payment authorize کی۔ |
| `payment_signature` | Yes | JSON یا literal string جس میں steward/treasury signature proof ہو۔ |
| `controllers` | Optional | Controller account addresses کی semicolon/comma-separated list۔ خالی ہونے پر `[owner]`. |
| `metadata` | Optional | Inline JSON یا `@path/to/file.json` جو resolver hints، TXT records وغیرہ دے۔ Default `{}`۔ |
| `governance` | Optional | Inline JSON یا `@path` جو `GovernanceHookV1` کی طرف ہو۔ `--require-governance` اس column کو لازمی کرتا ہے۔ |

کوئی بھی column سیل ویلیو میں `@` لگا کر external file کو refer کر سکتا ہے۔
Paths CSV file کے relative resolve ہوتے ہیں۔

## 2. Helper چلانا

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Key options:

- `--require-governance` governance hook کے بغیر rows reject کرتا ہے (premium
  auctions یا reserved assignments کے لئے مفید).
- `--default-controllers {owner,none}` طے کرتا ہے کہ خالی controller cells owner
  account پر واپس جائیں یا نہیں.
- `--controllers-column`, `--metadata-column`, اور `--governance-column` upstream
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
```

اگر `--ndjson` دیا جائے تو ہر `RegisterNameRequestV1` کو single-line JSON
document کے طور پر بھی لکھا جاتا ہے تاکہ automations requests کو براہ راست Torii
میں stream کر سکیں:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v2/sns/registrations
  done
```

## 3. Automated submissions

### 3.1 Torii REST mode

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

- Helper ہر request کے لئے `POST /v2/sns/registrations` بھیجتا ہے اور پہلی HTTP
  error پر abort کرتا ہے۔ Responses log path میں NDJSON records کے طور پر append
  ہوتے ہیں۔
- `--poll-status` ہر submission کے بعد `/v2/sns/registrations/{selector}` کو
  دوبارہ query کرتا ہے (زیادہ سے زیادہ `--poll-attempts`, default 5) تاکہ record
  visible ہونے کی تصدیق ہو۔ `--suffix-map` (JSON جو `suffix_id` کو "suffix" values
  سے map کرے) فراہم کریں تاکہ tool `{label}.{suffix}` literals derive کر سکے۔
- Tunables: `--submit-timeout`, `--poll-attempts`, `--poll-interval`.

### 3.2 iroha CLI mode

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
  کئے جاتے ہیں۔
- CLI stdout/stderr اور exit codes log ہوتے ہیں؛ non-zero codes run کو abort کرتے ہیں۔

دونوں submission modes ایک ساتھ چل سکتے ہیں (Torii اور CLI) تاکہ registrar
deployments cross-check ہوں یا fallbacks rehearse کئے جائیں۔

### 3.3 Submission receipts

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

## 4. Docs portal release automation

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

Script:

1. `registrations.manifest.json`, `registrations.ndjson` بناتا ہے اور original
   CSV کو release directory میں کاپی کرتا ہے۔
2. Torii اور/یا CLI کے ذریعے manifest submit کرتا ہے (جب configure ہو)، اور
   `submissions.log` میں اوپر والے structured receipts لکھتا ہے۔
3. `summary.json` emit کرتا ہے جو release کو describe کرتا ہے (paths, Torii URL,
   CLI path, timestamp) تاکہ portal automation bundle کو artefact storage پر
   upload کر سکے۔
4. `metrics.prom` بناتا ہے (`--metrics` کے ذریعے override)، جس میں Prometheus-
   format counters ہوتے ہیں: total requests, suffix distribution, asset totals,
   اور submission outcomes۔ summary JSON اس file کی طرف link کرتا ہے۔

Workflows صرف release directory کو ایک artefact کے طور پر archive کرتے ہیں، جس میں
اب وہ سب کچھ ہے جو governance کو audit کے لئے درکار ہے۔

## 5. Telemetry اور dashboards

`sns_bulk_release.sh` کے ذریعہ بننے والی metrics file درج ذیل series expose کرتی ہے:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom` کو اپنے Prometheus sidecar میں feed کریں (مثال کے طور پر Promtail
یا batch importer کے ذریعے) تاکہ registrars, stewards اور governance peers bulk
progress پر aligned رہیں۔ Grafana board
`dashboards/grafana/sns_bulk_release.json` وہی data panels میں دکھاتا ہے: per-suffix
counts، payment volume، اور submission success/failure ratios۔ Board `release`
filter کرتا ہے تاکہ auditors ایک CSV run پر drill کر سکیں۔

## 6. Validation اور failure modes

- **Label canonicalisation:** inputs Python IDNA کے ساتھ lowercase اور Norm v1
  character filters سے normalise ہوتے ہیں۔ invalid labels network calls سے پہلے
  fail fast ہوتے ہیں۔
- **Numeric guardrails:** suffix ids, term years اور pricing hints `u16` اور `u8`
  bounds کے اندر ہونے چاہئیں۔ Payment fields decimal یا hex integers `i64::MAX`
  تک accept کرتے ہیں۔
- **Metadata یا governance parsing:** inline JSON براہ راست parse ہوتا ہے؛ file
  references CSV location کے relative resolve ہوتی ہیں۔ Non-object metadata
  validation error دیتا ہے۔
- **Controllers:** خالی cells `--default-controllers` کو honour کرتے ہیں۔ non-owner
  actors کو delegate کرتے وقت explicit controller lists دیں (مثال `i105...;i105...`)۔

Failures contextual row numbers کے ساتھ report ہوتے ہیں (مثال
`error: row 12 term_years must be between 1 and 255`). Script validation errors پر
`1` اور CSV path missing ہونے پر `2` کے ساتھ exit کرتا ہے۔

## 7. Testing اور provenance

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` CSV parsing، NDJSON
  emission، governance enforcement، اور CLI/Torii submission paths کو cover کرتا ہے۔
- Helper pure Python ہے (کوئی اضافی dependencies نہیں) اور جہاں `python3` دستیاب
  ہو وہاں چلتا ہے۔ Commit history CLI کے ساتھ main repository میں track ہوتی ہے
  تاکہ reproducibility ہو۔

Production runs کے لئے، generated manifest اور NDJSON bundle کو registrar ticket کے
ساتھ attach کریں تاکہ stewards Torii کو submit ہونے والے exact payloads replay
کر سکیں۔
