---
lang: ur
direction: rtl
source: docs/source/runbooks/address_manifest_ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb5d84c6939c186ebb4cd1b622e5ab66872349f5c177191c940a9e9fd63d1a17
source_last_modified: "2025-12-14T09:53:36.233782+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# ایڈریس مینی فیسٹ آپریشنز رن بُک (ADDR-7c)

یہ رن بُک روڈمیپ آئٹم **ADDR-7c** کو آپریشنل بناتی ہے اور Sora Nexus کے
اکاؤنٹ/عرفی نام مینی فیسٹ میں اندراجات کی توثیق، اشاعت اور ریٹائرمنٹ کے
طریقہ کار بیان کرتی ہے۔ یہ تکنیکی معاہدے کی تکمیل کرتی ہے جو
[`docs/account_structure.md`](../../account_structure.md) §4 میں ہے اور
`dashboards/grafana/address_ingest.json` میں درج ٹیلی میٹری توقعات سے ہم آہنگ ہے۔

## 1. دائرہ کار اور ان پٹس

| ان پٹ | ماخذ | نوٹس |
|-------|--------|-------|
| سائن شدہ مینی فیسٹ بنڈل (`manifest.json`, `manifest.sigstore`, `checksums.sha256`, `notes.md`) | SoraFS پن (`sorafs://address-manifests/<CID>/`) اور HTTPS مرر | بنڈل ریلیز آٹومیشن سے نکلتے ہیں؛ مرر کرتے وقت ڈائریکٹری اسٹرکچر برقرار رکھیں۔ |
| پچھلے مینی فیسٹ کا digest + sequence | سابقہ بنڈل (وہی پاتھ پیٹرن) | monotonicity/immutability ثابت کرنے کے لیے ضروری۔ |
| ٹیلی میٹری رسائی | Grafana `address_ingest` ڈیش بورڈ + Alertmanager | Local‑8 ریٹائرمنٹ اور invalid address spikes کی نگرانی کے لیے ضروری۔ |
| ٹولنگ | `cosign`, `shasum`, `b3sum` (یا `python3 -m blake3`), `jq`, `iroha` CLI, `scripts/account_fixture_helper.py` | چیک لسٹ چلانے سے پہلے انسٹال کریں۔ |

## 2. آرٹیفیکٹ لے آؤٹ

ہر بنڈل نیچے دیے گئے لے آؤٹ پر چلتا ہے؛ ماحول کے درمیان کاپی کرتے وقت فائلوں کے
نام نہ بدلیں۔

```
address-manifest-<REVISION>/
├── manifest.json              # canonical JSON (UTF-8, newline-terminated)
├── manifest.sigstore          # Sigstore bundle from `cosign sign-blob`
├── checksums.sha256           # one-line SHA-256 sum for each artifact
└── notes.md                   # change log (reason codes, tickets, owners)
```

`manifest.json` کے ہیڈر فیلڈز:

| فیلڈ | وضاحت |
|-------|-------------|
| `version` | اسکیمہ ورژن (فی الحال `1`)۔ |
| `sequence` | monotonic ریویژن نمبر؛ لازماً ایک سے بڑھے۔ |
| `generated_ms` | اشاعت کا UTC ٹائم اسٹیمپ (epoch سے ملی سیکنڈ)۔ |
| `ttl_hours` | زیادہ سے زیادہ کیش لائف جسے Torii/SDKs احترام کر سکتے ہیں (ڈیفالٹ 24)۔ |
| `previous_digest` | پچھلے مینی فیسٹ باڈی کا BLAKE3 (hex)۔ |
| `entries` | ریکارڈز کی ترتیب وار فہرست (`global_domain`, `local_alias`, `tombstone`)۔ |

## 3. ویریفیکیشن طریقہ کار

1. **بنڈل ڈاؤن لوڈ کریں۔**

   ```bash
   export REV=2025-04-12
   sorafs_cli fetch --id sorafs://address-manifests/${REV} --out artifacts/address_manifest_${REV}
   cd artifacts/address_manifest_${REV}
   ```

2. **چیک سم گارڈریل۔**

   ```bash
   shasum -a 256 -c checksums.sha256
   ```

   تمام فائلوں کا `OK` ہونا ضروری ہے؛ mismatch کو چھیڑ چھاڑ سمجھیں۔

3. **Sigstore ویریفیکیشن۔**

   ```bash
   cosign verify-blob \
     --bundle manifest.sigstore \
     --certificate-identity-regexp 'governance\.sora\.nexus/addr-manifest' \
     --certificate-oidc-issuer https://accounts.google.com \
     manifest.json
   ```

4. **Immutability ثبوت۔** `sequence` اور `previous_digest` کو archived مینی فیسٹ
   کے مقابل چیک کریں:

   ```bash
   jq '.sequence, .previous_digest' manifest.json
   b3sum -l 256 ../address-manifest_<prev>/manifest.json
   ```

   پرنٹ ہونے والا digest `previous_digest` سے میچ ہونا چاہیے۔ sequence میں
   خلاء قابل قبول نہیں؛ خلاف ورزی پر مینی فیسٹ دوبارہ جاری کریں۔

5. **TTL مطابقت۔** یقینی بنائیں کہ `generated_ms + ttl_hours` متوقع ڈپلائمنٹ
   ونڈوز کو کور کرے؛ ورنہ گورننس کو کیش ختم ہونے سے پہلے دوبارہ شائع کرنا ہوگا۔

6. **انٹری sanity۔**
   - `global_domain` انٹری لازماً `{ "domain": "example", "chain": "sora:nexus:global", "selector": "global" }` شامل کرے۔
   - `local_alias` انٹری لازماً Norm v1 سے بنے 12 بائٹ digest کو شامل کرے
     (تصدیق کے لیے `iroha tools address convert <address-or-account_id> --format json --expect-prefix 753` استعمال کریں؛ JSON خلاصہ `input_domain` کے ذریعے domain دکھاتا ہے اور `--append-domain` `<ih58>@<domain>` کے طور پر دوبارہ انکوڈ دکھاتا ہے)۔
   - `tombstone` انٹری لازماً درست selector کو ریفرنس کرے اور `reason_code`, `ticket`, `replaces_sequence` شامل کرے۔

7. **Fixture parity۔** canonical vectors دوبارہ بنائیں اور دیکھیں کہ Local digest
   ٹیبل غیر متوقع طور پر نہ بدلا ہو:

   ```bash
   cargo xtask address-vectors
   python3 scripts/account_fixture_helper.py check --quiet
   ```

8. **Automation guardrail۔** مینی فیسٹ ویریفائر چلائیں تاکہ بنڈل end-to-end
   دوبارہ چیک ہو (ہیڈر اسکیمہ، انٹری فارم، چیک سم اور previous‑digest wiring):

   ```bash
   cargo xtask address-manifest verify \
     --bundle artifacts/address-manifest_2025-05-12 \
     --previous artifacts/address-manifest_2025-04-30
   ```

   `--previous` فوراً پچھلے بنڈل کی طرف اشارہ کرتا ہے تاکہ ٹول `sequence`
   کی monotonicity اور `previous_digest` کی BLAKE3 proof دوبارہ حساب کر سکے۔
   چیک سم drift یا `tombstone` میں لازمی فیلڈ غائب ہونے پر کمانڈ فوراً ناکام
   ہوتی ہے، اس لیے دستخط سے پہلے آؤٹ پٹ کو تبدیلی ٹکٹ میں شامل کریں۔

## 4. Alias اور tombstone تبدیلی فلو

1. **تبدیلی تجویز کریں۔** گورننس ٹکٹ فائل کریں جس میں reason code
   (`LOCAL8_RETIREMENT`, `DOMAIN_REASSIGNED` وغیرہ) اور متاثرہ selectors درج ہوں۔
2. **Canonical payloads نکالیں۔** ہر alias کے لیے یہ چلائیں:

   ```bash
   iroha tools address convert snx1...@wonderland --expect-prefix 753 --format json > /tmp/alias.json
   jq '.canonical_hex, .input_domain' /tmp/alias.json
   ```

3. **مینی فیسٹ انٹری ڈرافٹ۔** JSON ریکارڈ اس طرح شامل کریں:

   ```json
   {
     "type": "tombstone",
     "selector": { "kind": "local", "digest_hex": "b18fe9c1abbac45b3e38fc5d" },
     "reason_code": "LOCAL8_RETIREMENT",
     "ticket": "ADDR-7c-2025-04-12",
     "replaces_sequence": 36
   }
   ```

   Local alias کو Global سے بدلتے وقت `tombstone` کے ساتھ بعد والی `global_domain`
   انٹری بھی شامل کریں جس میں Nexus discriminant ہو۔

4. **بنڈل کی توثیق۔** دستخط سے پہلے ڈرافٹ مینی فیسٹ پر اوپر والے ویریفیکیشن
   مراحل دوبارہ چلائیں۔
5. **اشاعت اور مانیٹرنگ۔** گورننس کے دستخط کے بعد §3 کے مطابق عمل کریں اور Torii
   پر رکھیں جب metrics بتائیں کہ Local‑8 استعمال صفر ہے۔ صرف dev/test میں، اضافی
   soak کے لیے `false` کریں۔

## 5. مانیٹرنگ اور رول بیک

- ڈیش بورڈز: `dashboards/grafana/address_ingest.json` (پینلز:
  `torii_address_invalid_total{endpoint,reason}`،
  `torii_address_local8_total{endpoint}`،
  `torii_address_collision_total{endpoint,kind="local12_digest"}`،
  `torii_address_collision_domain_total{endpoint,domain}`) کو 30 دن تک گرین رکھیں
  قبل ازیں Local‑8/Local‑12 ٹریفک مستقل طور پر گیٹ ہو۔
- گیٹ ثبوت: 30 دن کے لیے Prometheus رینج query برآمد کریں
  `torii_address_local8_total` اور `torii_address_collision_total` پر (مثلاً
  `promtool query range --output=json ...`) اور چلائیں
  `cargo xtask address-local8-gate --input <file> --json-out artifacts/address_gate.json`؛
  JSON + CLI آؤٹ پٹ rollout ٹکٹ میں شامل کریں تاکہ گورننس کوریج ونڈو دیکھ سکے۔
- Alerts (`dashboards/alerts/address_ingest_rules.yml`):
  - `AddressLocal8Resurgence` — Local‑8 میں کوئی بھی نیا اضافہ پیج کرتا ہے۔ اسے
    عارضی طور پر سیٹ کریں جب تک کلائنٹ ٹھیک نہ ہو۔ صفائی کے بعد `true` کریں۔
  - `AddressLocal12Collision` — دو Local‑12 لیبلز ایک ہی digest میں ٹکرا جائیں تو
    فوراً فائر۔ مینی فیسٹ promotion روکیں، `scripts/address_local_toolkit.sh` چلائیں،
    اور Nexus گورننس کے ساتھ کوآرڈینیٹ کر کے متاثرہ انٹری دوبارہ جاری کریں۔
  - `AddressInvalidRatioSlo` — invalid IH58/کمپریسڈ submissions (Local‑8/strict‑mode
    ریجیکشنز کے علاوہ) 0.1% SLO سے 10 منٹ تک بڑھیں تو وارننگ۔ `torii_address_invalid_total`
    کو context/reason کے حساب سے دیکھیں اور SDK ٹیم سے مل کر strict‑mode دوبارہ آن کریں۔
- لاگز: Torii `manifest_refresh` لاگز اور گورننس ٹکٹ نمبر `notes.md` میں محفوظ کریں۔
- رول بیک: پچھلا بنڈل دوبارہ شائع کریں (وہی فائلیں، rollback کا ٹکٹ)، اور
  جائے تو `true` پر واپس لائیں۔

## 6. حوالہ جات

- [`docs/account_structure.md`](../../account_structure.md) §§4–4.1 (کنٹریکٹ).
- [`scripts/account_fixture_helper.py`](../../../scripts/account_fixture_helper.py) (fixture sync).
- [`fixtures/account/address_vectors.json`](../../../fixtures/account/address_vectors.json) (canonical digests).
- [`dashboards/grafana/address_ingest.json`](../../../dashboards/grafana/address_ingest.json) (ٹیلی میٹری).

</div>
