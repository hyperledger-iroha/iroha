---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/provider-admission-policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb2b7787e303d0abec1b52d30fcee137ae5cc0d4a357aba4bf97ce46ae8f0b52
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: provider-admission-policy
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/provider-admission-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


> [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) سے ماخوذ۔

# SoraFS فراہم کنندگان کی قبولیت اور شناخت کی پالیسی (SF-2b مسودہ)

یہ نوٹ **SF-2b** کے لیے قابلِ عمل ڈیلیوریبلز کو سمیٹتا ہے: SoraFS اسٹوریج فراہم کنندگان کے لیے قبولیت کے ورک فلو، شناختی تقاضے، اور توثیقی payloads کی تعریف اور نفاذ۔ یہ SoraFS Architecture RFC میں بیان کردہ اعلی سطح کے عمل کو وسعت دیتا ہے اور باقی کام کو قابلِ ٹریک انجینئرنگ ٹاسکس میں تقسیم کرتا ہے۔

## پالیسی کے اہداف

- یہ یقینی بنانا کہ صرف تصدیق شدہ آپریٹرز ہی `ProviderAdvertV1` ریکارڈز شائع کر سکیں جنہیں نیٹ ورک قبول کرے۔
- ہر اعلان کلید کو گورننس سے منظور شدہ شناختی دستاویز، توثیق شدہ endpoints، اور کم از کم stake شراکت کے ساتھ باندھنا۔
- Torii، gateways، اور `sorafs-node` ایک ہی چیک نافذ کریں اس کے لیے ڈیٹرمنسٹک ویریفیکیشن ٹولنگ فراہم کرنا۔
- ڈیٹرمنزم یا ٹولنگ ergonomics کو توڑے بغیر تجدید اور ہنگامی منسوخی کی حمایت کرنا۔

## شناخت اور stake تقاضے

| تقاضا | وضاحت | ڈیلیوریبل |
|-------|-------|-----------|
| اعلان کلید کا ماخذ | فراہم کنندگان کو Ed25519 keypair رجسٹر کرنا ہوگا جو ہر advert پر دستخط کرے۔ admission bundle گورننس دستخط کے ساتھ پبلک key محفوظ کرتا ہے۔ | `ProviderAdmissionProposalV1` اسکیمہ میں `advert_key` (32 bytes) شامل کریں اور اسے رجسٹری (`sorafs_manifest::provider_admission`) سے ریفرنس کریں۔ |
| Stake پوائنٹر | قبولیت کے لیے فعال staking pool کی طرف اشارہ کرنے والا غیر صفر `StakePointer` درکار ہے۔ | `sorafs_manifest::provider_advert::StakePointer::validate()` میں ویلیڈیشن شامل کریں اور CLI/tests میں errors ظاہر کریں۔ |
| Jurisdiction ٹیگز | فراہم کنندگان jurisdiction + قانونی رابطہ ظاہر کرتے ہیں۔ | پروپوزل اسکیمہ میں `jurisdiction_code` (ISO 3166-1 alpha-2) اور اختیاری `contact_uri` شامل کریں۔ |
| Endpoint توثیق | ہر اعلان شدہ endpoint کو mTLS یا QUIC سرٹیفکیٹ رپورٹ سے سپورٹ کرنا لازم ہے۔ | Norito payload `EndpointAttestationV1` کی تعریف کریں اور اسے admission bundle کے اندر ہر endpoint کے ساتھ اسٹور کریں۔ |

## قبولیت کا ورک فلو

1. **پروپوزل تیار کرنا**
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     شامل کریں جو `ProviderAdmissionProposalV1` + توثیقی bundle بنائے۔
   - ویلیڈیشن: ضروری فیلڈز، stake > 0، اور `profile_id` میں canonical chunker handle کو یقینی بنائیں۔
2. **گورننس کی منظوری**
   - کونسل موجودہ envelope tooling (ماڈیول `sorafs_manifest::governance`) استعمال کرتے ہوئے
     `blake3("sorafs-provider-admission-v1" || canonical_bytes)` پر دستخط کرتی ہے۔
   - Envelope کو `governance/providers/<provider_id>/admission.json` میں محفوظ کیا جاتا ہے۔
3. **رجسٹری میں اندراج**
   - ایک مشترکہ verifier (`sorafs_manifest::provider_admission::validate_envelope`) نافذ کریں جسے Torii/gateways/CLI دوبارہ استعمال کریں۔
   - Torii کے admission راستے کو اپڈیٹ کریں تاکہ وہ ایسے adverts کو مسترد کرے جن کا digest یا expiry envelope سے مختلف ہو۔
4. **تجدید اور منسوخی**
   - اختیاری endpoint/stake اپڈیٹس کے ساتھ `ProviderAdmissionRenewalV1` شامل کریں۔
   - ایک CLI راستہ `--revoke` فراہم کریں جو منسوخی کی وجہ ریکارڈ کرے اور گورننس ایونٹ بھیجے۔

## عمل درآمد کے کام

| علاقہ | کام | Owner(s) | حالت |
|-------|-----|----------|------|
| اسکیمہ | `crates/sorafs_manifest/src/provider_admission.rs` کے تحت `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) کی تعریف کریں۔ `sorafs_manifest::provider_admission` میں ویلیڈیشن helpers کے ساتھ نافذ کیا گیا ہے۔【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Storage / Governance | ✅ مکمل |
| CLI ٹولنگ | `sorafs_manifest_stub` کو سب کمانڈز کے ساتھ توسیع دیں: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Tooling WG | ✅ مکمل |

CLI فلو اب درمیانی سرٹیفکیٹ bundles (`--endpoint-attestation-intermediate`) قبول کرتا ہے،
canonical proposal/envelope bytes جاری کرتا ہے، اور `sign`/`verify` کے دوران کونسل signatures کو ویری فائی کرتا ہے۔ آپریٹرز advert bodies براہ راست فراہم کر سکتے ہیں یا signed adverts دوبارہ استعمال کر سکتے ہیں، اور signature files کو `--council-signature-public-key` کے ساتھ `--council-signature-file` جوڑ کر فراہم کیا جا سکتا ہے تاکہ automation آسان ہو۔

### CLI حوالہ

ہر کمانڈ کو `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...` کے ذریعے چلائیں۔

- `proposal`
  - درکار فلیگز: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, اور کم از کم ایک `--endpoint=<kind:host>`۔
  - ہر endpoint کے لیے توثیق میں `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, ایک سرٹیفکیٹ بذریعہ
    `--endpoint-attestation-leaf=<path>` (ہر چین عنصر کے لیے اختیاری `--endpoint-attestation-intermediate=<path>`) اور
    کوئی بھی مذاکرات شدہ ALPN IDs (`--endpoint-attestation-alpn=<token>`) شامل ہیں۔ QUIC endpoints نقل و حمل رپورٹس
    `--endpoint-attestation-report[-hex]=...` کے ذریعے فراہم کر سکتے ہیں۔
  - آؤٹ پٹ: canonical Norito proposal bytes (`--proposal-out`) اور ایک JSON خلاصہ
    (ڈیفالٹ stdout یا `--json-out`).
- `sign`
  - ان پٹس: ایک پروپوزل (`--proposal`)، ایک signed advert (`--advert`)، اختیاری advert body
    (`--advert-body`)، retention epoch، اور کم از کم ایک کونسل signature۔ signatures کو inline
    (`--council-signature=<signer_hex:signature_hex>`) یا فائلز کے ذریعے فراہم کیا جا سکتا ہے جب
    `--council-signature-public-key` کو `--council-signature-file=<path>` کے ساتھ ملایا جائے۔
  - ایک validated envelope (`--envelope-out`) اور JSON رپورٹ تیار کرتا ہے جو digest bindings، signer count، اور input paths دکھاتی ہے۔
- `verify`
  - موجودہ envelope (`--envelope`) کی تصدیق کرتا ہے، اور اختیاری طور پر matching proposal، advert، یا advert body چیک کرتا ہے۔ JSON رپورٹ digest values، signature verification status، اور وہ optional artefacts دکھاتی ہے جو match ہوئے۔
- `renewal`
  - نئے منظور شدہ envelope کو پہلے سے منظور شدہ digest کے ساتھ جوڑتا ہے۔ اس کے لیے
    `--previous-envelope=<path>` اور جانشین `--envelope=<path>` (دونوں Norito payloads) درکار ہیں۔
    CLI اس بات کی تصدیق کرتا ہے کہ profile aliases، capabilities، اور advert keys بغیر تبدیلی کے رہیں، جبکہ stake، endpoints، اور metadata اپڈیٹس کی اجازت دیتا ہے۔ canonical `ProviderAdmissionRenewalV1` bytes (`--renewal-out`) اور JSON خلاصہ آؤٹ پٹ ہوتا ہے۔
- `revoke`
  - ایسے provider کے لیے ہنگامی `ProviderAdmissionRevocationV1` bundle جاری کرتا ہے جس کا envelope واپس لینا ضروری ہو۔
    `--envelope=<path>`, `--reason=<text>`, کم از کم ایک `--council-signature` درکار ہے، اور
    `--revoked-at`/`--notes` اختیاری ہیں۔ CLI revocation digest کو sign/verify کرتا ہے، Norito payload
    `--revocation-out` کے ذریعے لکھتا ہے، اور digest اور signature count کے ساتھ JSON رپورٹ پرنٹ کرتا ہے۔
| ویریفیکیشن | Torii، gateways، اور `sorafs-node` کے لیے مشترکہ verifier نافذ کریں۔ unit + CLI integration tests فراہم کریں۔【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Networking TL / Storage | ✅ مکمل |
| Torii انضمام | verifier کو Torii میں adverts ingest کرنے کے دوران شامل کریں، پالیسی سے باہر adverts مسترد کریں، اور telemetry جاری کریں۔ | Networking TL | ✅ مکمل | Torii اب governance envelopes (`torii.sorafs.admission_envelopes_dir`) لوڈ کرتا ہے، ingest کے دوران digest/signature match کی تصدیق کرتا ہے، اور admission telemetry ظاہر کرتا ہے。【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| تجدید | تجدید/منسوخی اسکیمہ + CLI helpers شامل کریں، اور lifecycle guide کو docs میں شائع کریں (نیچے runbook اور `provider-admission renewal`/`revoke` CLI کمانڈز دیکھیں)。【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Storage / Governance | ✅ مکمل |
| ٹیلیمیٹری | `provider_admission` dashboards اور alerts کی تعریف کریں (تجدید کی کمی، envelope expiry)۔ | Observability | 🟠 جاری | کاؤنٹر `torii_sorafs_admission_total{result,reason}` موجود ہے؛ dashboards/alerts زیر التوا ہیں۔【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### تجدید اور منسوخی کا رن بُک

#### شیڈول شدہ تجدید (stake/topology اپڈیٹس)
1. `provider-admission proposal` اور `provider-admission sign` کے ذریعے جانشین proposal/advert جوڑا بنائیں،
   `--retention-epoch` بڑھائیں اور ضرورت کے مطابق stake/endpoints اپڈیٹ کریں۔
2. چلائیں
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   یہ کمانڈ `AdmissionRecord::apply_renewal` کے ذریعے capability/profile فیلڈز کو غیر تبدیل شدہ ہونے کی تصدیق کرتی ہے،
   `ProviderAdmissionRenewalV1` جاری کرتی ہے، اور گورننس لاگ کے لیے digests پرنٹ کرتی ہے۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. `torii.sorafs.admission_envelopes_dir` میں پچھلا envelope تبدیل کریں، renewal Norito/JSON کو گورننس ریپوزٹری میں commit کریں،
   اور renewal hash + retention epoch کو `docs/source/sorafs/migration_ledger.md` میں شامل کریں۔
4. آپریٹرز کو بتائیں کہ نیا envelope فعال ہے اور ingest کی تصدیق کے لیے
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` مانیٹر کریں۔
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` کے ذریعے canonical fixtures دوبارہ بنائیں اور commit کریں؛
   CI (`ci/check_sorafs_fixtures.sh`) Norito آؤٹ پٹس کی stability چیک کرتا ہے۔

#### ہنگامی منسوخی
1. متاثرہ envelope کی شناخت کریں اور منسوخی جاری کریں:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI `ProviderAdmissionRevocationV1` پر دستخط کرتا ہے، `verify_revocation_signatures` کے ذریعے signatures کا سیٹ ویریفائی کرتا ہے،
   اور revocation digest رپورٹ کرتا ہے۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. `torii.sorafs.admission_envelopes_dir` سے envelope ہٹا دیں، revocation Norito/JSON کو admission caches میں تقسیم کریں،
   اور وجہ کا hash گورننس منٹس میں ریکارڈ کریں۔
3. `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` دیکھیں تاکہ تصدیق ہو سکے کہ caches نے revoked advert کو drop کر دیا ہے؛
   revocation artefacts کو incident retrospectives میں محفوظ رکھیں۔

## ٹیسٹنگ اور ٹیلیمیٹری

- admission proposals اور envelopes کے لیے golden fixtures کو `fixtures/sorafs_manifest/provider_admission/` کے تحت شامل کریں۔
- CI (`ci/check_sorafs_fixtures.sh`) کو وسعت دیں تاکہ proposals دوبارہ بنائے جائیں اور envelopes ویریفائی ہوں۔
- تیار شدہ fixtures میں canonical digests کے ساتھ `metadata.json` شامل ہوتا ہے؛ downstream tests یہ assert کرتے ہیں کہ
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- integration tests فراہم کریں:
  - Torii ایسے adverts مسترد کرتا ہے جن کے admission envelopes غائب یا میعاد ختم ہو چکے ہوں۔
  - CLI proposal → envelope → verification کا round-trip چلاتا ہے۔
  - گورننس تجدید provider ID تبدیل کیے بغیر endpoint توثیق کو rotate کرتی ہے۔
- ٹیلیمیٹری کی ضروریات:
  - Torii میں `provider_admission_envelope_{accepted,rejected}` کاؤنٹرز emit کریں۔ ✅ `torii_sorafs_admission_total{result,reason}` اب accepted/rejected نتائج دکھاتا ہے۔
  - observability dashboards میں expiry warnings شامل کریں (7 دن کے اندر تجدید درکار ہونے پر)۔

## اگلے اقدامات

1. ✅ Norito اسکیمہ تبدیلیاں حتمی ہو چکی ہیں اور `sorafs_manifest::provider_admission` میں validation helpers شامل ہو گئے ہیں۔ کسی feature flags کی ضرورت نہیں۔
2. ✅ CLI فلو (`proposal`, `sign`, `verify`, `renewal`, `revoke`) دستاویزی ہیں اور integration tests سے گزارے گئے ہیں؛ گورننس اسکرپٹس کو runbook کے ساتھ ہم آہنگ رکھیں۔
3. ✅ Torii admission/discovery envelopes ingest کرتا ہے اور قبولیت/رد کے telemetry counters دکھاتا ہے۔
4. observability پر توجہ: admission dashboards/alerts مکمل کریں تاکہ سات دن کے اندر واجب التجدید معاملات پر وارننگ ہو (`torii_sorafs_admission_total`, expiry gauges)۔
