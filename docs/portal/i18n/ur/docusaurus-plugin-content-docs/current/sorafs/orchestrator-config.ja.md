---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sorafs/orchestrator-config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e25d2e81d8db6bffa785cf7844636b6af18f3ef1ec73c554a4b3e6f2e6ce16c6
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
id: orchestrator-config
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-config.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/developer/orchestrator.md` کی عکاسی کرتا ہے۔ جب تک پرانی ڈاکیومنٹیشن ریٹائر نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

# ملٹی سورس fetch آرکسٹریٹر گائیڈ

SoraFS کا ملٹی سورس fetch آرکسٹریٹر گورننس سپورٹڈ adverts میں شائع شدہ
پرووائیڈرز سیٹ سے ڈٹرمنسٹک اور متوازی ڈاؤن لوڈز چلاتا ہے۔ یہ گائیڈ وضاحت کرتی
ہے کہ آرکسٹریٹر کیسے کنفیگر کیا جائے، رول آؤٹس کے دوران کون سی ناکامی سگنلز
متوقع ہیں، اور کون سے ٹیلیمیٹری اسٹریمز صحت کے اشارے ظاہر کرتے ہیں۔

## 1. کنفیگریشن اوور ویو

آرکسٹریٹر تین سورسز کی کنفیگریشن کو مرج کرتا ہے:

| سورس | مقصد | نوٹس |
|------|------|------|
| `OrchestratorConfig.scoreboard` | پرووائیڈر weights کو نارملائز کرتا ہے، ٹیلیمیٹری کی تازگی ویریفائی کرتا ہے، اور آڈٹس کے لیے JSON scoreboard محفوظ کرتا ہے۔ | `crates/sorafs_car::scoreboard::ScoreboardConfig` پر مبنی۔ |
| `OrchestratorConfig.fetch` | رن ٹائم حدود لگاتا ہے (retry budgets، concurrency bounds، verification toggles)۔ | `crates/sorafs_car::multi_fetch` میں `FetchOptions` سے میپ ہوتا ہے۔ |
| CLI / SDK پیرامیٹرز | peers کی تعداد محدود کرتے ہیں، telemetry regions لگاتے ہیں، اور deny/boost پالیسیز ظاہر کرتے ہیں۔ | `sorafs_cli fetch` یہ flags براہ راست دیتا ہے؛ SDKs انہیں `OrchestratorConfig` کے ذریعے پاس کرتے ہیں۔ |

`crates/sorafs_orchestrator::bindings` میں JSON helpers پوری کنفیگریشن کو Norito
JSON میں serialize کرتے ہیں، جس سے اسے SDK bindings اور آٹومیشن میں پورٹیبل
بنایا جا سکے۔

### 1.1 نمونہ JSON کنفیگریشن

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

فائل کو معمول کے `iroha_config` layering (`defaults/`, user, actual) کے ذریعے
محفوظ کریں تاکہ ڈٹرمنسٹک deployments تمام نوڈز پر ایک جیسے limits استعمال کریں۔
SNNet-5a rollout سے ہم آہنگ direct-only fallback پروفائل کے لیے
`docs/examples/sorafs_direct_mode_policy.json` اور
`docs/source/sorafs/direct_mode_pack.md` دیکھیں۔

### 1.2 Compliance overrides

SNNet-9 آرکسٹریٹر میں گورننس ڈرائیون کمپلائنس شامل کرتا ہے۔ Norito JSON
کنفیگریشن میں ایک نیا `compliance` آبجیکٹ carve-outs محفوظ کرتا ہے جو fetch
پائپ لائن کو direct-only موڈ پر مجبور کرتے ہیں:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` ISO‑3166 alpha‑2 کوڈز ڈکلیئر کرتا ہے جہاں یہ
  آرکسٹریٹر انسٹینس آپریٹ کرتا ہے۔ parsing کے دوران کوڈز uppercase میں normalize
  ہوتے ہیں۔
- `jurisdiction_opt_outs` گورننس رجسٹر کو mirror کرتا ہے۔ جب کوئی آپریٹر
  jurisdiction فہرست میں ہو، آرکسٹریٹر `transport_policy=direct-only` نافذ
  کرتا ہے اور fallback reason `compliance_jurisdiction_opt_out` emit کرتا ہے۔
- `blinded_cid_opt_outs` manifest digests (blinded CIDs، uppercase hex) لسٹ کرتا
  ہے۔ matching payloads direct-only scheduling مجبور کرتی ہیں اور ٹیلیمیٹری میں
  `compliance_blinded_cid_opt_out` fallback دکھاتی ہیں۔
- `audit_contacts` ان URI کو ریکارڈ کرتا ہے جنہیں گورننس آپریٹرز کے GAR
  playbooks میں شائع کرنے کی توقع کرتی ہے۔
- `attestations` کمپلائنس کی سائنڈ پیکٹس محفوظ کرتا ہے۔ ہر entry ایک optional
  `jurisdiction` (ISO-3166 alpha-2)، `document_uri`, 64-character `digest_hex`,
  issuance timestamp `issued_at_ms`, اور optional `expires_at_ms` بیان کرتی ہے۔
  یہ artifacts آرکسٹریٹر کی audit checklist میں جاتے ہیں تاکہ governance tooling
  overrides کو سائنڈ دستاویزات سے جوڑ سکے۔

کمپلائنس بلاک کو معمول کے کنفیگریشن layering کے ذریعے دیں تاکہ آپریٹرز کو
ڈٹرمنسٹک overrides ملیں۔ آرکسٹریٹر کمپلائنس کو write-mode hints کے _بعد_
لاگو کرتا ہے: اگر SDK `upload-pq-only` بھی مانگے تو jurisdiction یا manifest
opt-outs پھر بھی direct-only transport پر لے جاتے ہیں اور جب کوئی compliant
provider نہ ہو تو فوری فیل کرتے ہیں۔

Canonical opt-out catalogs `governance/compliance/soranet_opt_outs.json` میں
موجود ہیں؛ Governance Council tagged releases کے ذریعے اپڈیٹس شائع کرتی ہے۔
Attestations سمیت مکمل مثال `docs/examples/sorafs_compliance_policy.json` میں
دستیاب ہے، اور آپریشنل پروسیس
[GAR compliance playbook](../../../source/soranet/gar_compliance_playbook.md)
میں درج ہے۔

### 1.3 CLI اور SDK knobs

| Flag / Field | اثر |
|--------------|-----|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | اس بات کی حد مقرر کرتا ہے کہ کتنے پرووائیڈر scoreboard فلٹر سے گزر سکیں۔ `None` پر سیٹ کریں تاکہ تمام eligible پرووائیڈرز استعمال ہوں۔ |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | فی-chunk retry حد مقرر کرتا ہے۔ حد سے تجاوز `MultiSourceError::ExhaustedRetries` اٹھاتا ہے۔ |
| `--telemetry-json` | latency/failure snapshots کو scoreboard builder میں inject کرتا ہے۔ `telemetry_grace_secs` سے پرانی ٹیلیمیٹری پرووائیڈرز کو ineligible کر دیتی ہے۔ |
| `--scoreboard-out` | حساب شدہ scoreboard (eligible + ineligible) کو پوسٹ رن معائنے کے لیے محفوظ کرتا ہے۔ |
| `--scoreboard-now` | scoreboard timestamp (Unix seconds) override کرتا ہے تاکہ fixture captures deterministic رہیں۔ |
| `--deny-provider` / score policy hook | adverts ڈیلیٹ کیے بغیر پرووائیڈرز کو deterministic طور پر exclude کرتا ہے۔ فوری بلیک لسٹنگ کے لیے مفید۔ |
| `--boost-provider=name:delta` | گورننس weights برقرار رکھتے ہوئے پرووائیڈر کے weighted round-robin credits ایڈجسٹ کرتا ہے۔ |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | میٹرکس اور structured logs کو لیبل دیتا ہے تاکہ ڈیش بورڈز جغرافیہ یا rollout wave کے لحاظ سے pivot کر سکیں۔ |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | اب ڈیفالٹ `soranet-first` ہے کیونکہ ملٹی سورس آرکسٹریٹر baseline ہے۔ downgrade یا compliance directive کے لیے `direct-only` استعمال کریں، اور PQ-only pilots کے لیے `soranet-strict` محفوظ رکھیں؛ compliance overrides پھر بھی سخت حد ہیں۔ |

SoraNet-first اب shipping default ہے، اور rollbacks کو متعلقہ SNNet blocker کا
حوالہ دینا چاہیے۔ SNNet-4/5/5a/5b/6a/7/8/12/13 کے گریجویٹ ہونے کے بعد گورننس
required posture کو مزید سخت کرے گی (`soranet-strict` کی طرف)؛ تب تک صرف
incident-driven overrides کو `direct-only` ترجیح دینی چاہیے، اور انہیں rollout
log میں ریکارڈ کرنا لازمی ہے۔

تمام flags اوپر `sorafs_cli fetch` اور developer-facing `sorafs_fetch` بائنری
میں `--` اسٹائل syntax قبول کرتے ہیں۔ SDKs یہی آپشنز typed builders کے ذریعے
ایکسپوز کرتے ہیں۔

### 1.4 Guard cache مینجمنٹ

CLI اب SoraNet guard selector کو wired کرتی ہے تاکہ آپریٹرز SNNet-5 کے مکمل
transport rollout سے پہلے entry relays کو deterministic انداز میں pin کر سکیں۔
تین نئے flags اس ورک فلو کو کنٹرول کرتے ہیں:

| Flag | مقصد |
|------|------|
| `--guard-directory <PATH>` | ایک JSON فائل کی طرف اشارہ کرتا ہے جو تازہ ترین relay consensus بیان کرتی ہے (نیچے subset دکھایا گیا ہے)۔ directory پاس کرنے سے fetch سے پہلے guard cache refresh ہوتی ہے۔ |
| `--guard-cache <PATH>` | Norito-encoded `GuardSet` محفوظ کرتا ہے۔ بعد کے رنز بغیر نئے directory کے بھی cache reuse کرتے ہیں۔ |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | entry guards کی تعداد (ڈیفالٹ 3) اور retention window (ڈیفالٹ 30 دن) کے لیے optional overrides۔ |
| `--guard-cache-key <HEX>` | optional 32-byte key جو Blake3 MAC کے ذریعے guard cache کو tag کرتی ہے تاکہ reuse سے پہلے فائل verify ہو سکے۔ |

Guard directory payloads ایک compact schema استعمال کرتی ہیں:

`--guard-directory` flag اب Norito-encoded `GuardDirectorySnapshotV2` payload
expect کرتا ہے۔ binary snapshot میں شامل ہے:

- `version` — schema ورژن (فی الحال `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  consensus metadata جو ہر embedded certificate سے match کرنا ضروری ہے۔
- `validation_phase` — certificate policy gate (`1` = single Ed25519 signature
  allow، `2` = dual signatures prefer، `3` = dual signatures require).
- `issuers` — governance issuers مع `fingerprint`, `ed25519_public`, `mldsa65_public`.
  fingerprints اس طرح بنائے جاتے ہیں:
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — SRCv2 bundles کی فہرست (`RelayCertificateBundleV2::to_cbor()` output).
  ہر bundle relay descriptor، capability flags، ML-KEM policy، اور Ed25519/ML-DSA-65
  dual signatures رکھتا ہے۔

CLI ہر bundle کو declared issuer keys کے خلاف verify کرتی ہے اور پھر directory کو
snapshots لازمی ہیں۔

`--guard-directory` کے ساتھ CLI چلائیں تاکہ تازہ ترین consensus کو موجودہ cache
کے ساتھ merge کیا جا سکے۔ selector pinned guards کو برقرار رکھتا ہے جو ابھی
retention window کے اندر اور directory میں eligible ہیں؛ نئے relays expired
entries کو replace کرتے ہیں۔ کامیاب fetch کے بعد updated cache `--guard-cache`
کے path پر لکھ دی جاتی ہے، جس سے اگلی سیشنز deterministic رہتی ہیں۔ SDKs یہی
behavior `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
کو کال کر کے اور نتیجہ `GuardSet` کو `SorafsGatewayFetchOptions` میں پاس کر کے
reproduce کر سکتی ہیں۔

`ml_kem_public_hex` selector کو PQ-capable guards کو ترجیح دینے دیتا ہے جب
SNNet-5 rollout جاری ہو۔ stage toggles (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) اب classical relays کو خودکار طور پر demote کرتے ہیں: جب PQ
guard دستیاب ہو تو selector اضافی classical pins ہٹا دیتا ہے تاکہ اگلی سیشنز
hybrid handshakes کو ترجیح دیں۔ CLI/SDK summaries نتیجہ خیز mix کو
`anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio` اور متعلقہ candidate/deficit/supply delta فیلڈز کے
ذریعے ظاہر کرتے ہیں، جس سے brownouts اور classical fallbacks واضح ہو جاتے ہیں۔

Guard directories اب `certificate_base64` کے ذریعے مکمل SRCv2 bundle embed کر
سکتی ہیں۔ آرکسٹریٹر ہر bundle کو decode کرتا ہے، Ed25519/ML-DSA signatures کو
re-validate کرتا ہے، اور parsed certificate کو guard cache کے ساتھ محفوظ رکھتا
ہے۔ جب certificate موجود ہو تو وہ PQ keys، handshake preferences، اور weighting
کا canonical source بن جاتا ہے؛ expired certificates discard کر دیے جاتے ہیں اور
management کے ذریعے propagate ہوتے ہیں اور `telemetry::sorafs.guard` اور
`telemetry::sorafs.circuit` کے ذریعے surface ہوتے ہیں، جو validity window،
handshake suites، اور dual signatures کے مشاہدے کو ریکارڈ کرتے ہیں۔

CLI helpers سے snapshots کو publishers کے ساتھ sync رکھیں:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` SRCv2 snapshot کو ڈاؤن لوڈ اور verify کر کے ڈسک پر لکھتا ہے، جبکہ `verify`
دوسری ٹیموں سے آئے artifacts کے لیے validation pipeline دوبارہ چلاتا ہے اور
CLI/SDK guard selector output سے match کرنے والا JSON summary emit کرتا ہے۔

### 1.5 Circuit lifecycle manager

جب relay directory اور guard cache دونوں فراہم ہوں تو آرکسٹریٹر circuit lifecycle
manager کو فعال کرتا ہے تاکہ ہر fetch سے پہلے SoraNet circuits کو pre-build اور
renew کیا جائے۔ کنفیگریشن `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) میں دو نئے فیلڈز کے ذریعے ہوتی ہے:

- `relay_directory`: SNNet-3 directory snapshot رکھتا ہے تاکہ middle/exit hops
  deterministic طریقے سے منتخب ہوں۔
- `circuit_manager`: optional کنفیگریشن (ڈیفالٹ طور پر enabled) جو circuit TTL
  کو کنٹرول کرتی ہے۔

Norito JSON اب `circuit_manager` بلاک قبول کرتا ہے:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDKs directory data کو
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`) کے ذریعے forward کرتی ہیں، اور CLI اسے اس وقت
خودکار طور پر wired کرتی ہے جب `--guard-directory` دیا جائے
(`crates/iroha_cli/src/commands/sorafs.rs:365`).

Manager circuits کو اس وقت renew کرتا ہے جب guard metadata (endpoint، PQ key یا
pinned timestamp) بدل جائے یا TTL ختم ہو۔ ہر fetch سے پہلے invoke ہونے والا
helper `refresh_circuits` (`crates/sorafs_orchestrator/src/lib.rs:1346`) `CircuitEvent`
لاگز emit کرتا ہے تاکہ آپریٹرز lifecycle فیصلوں کو ٹریس کر سکیں۔ soak test
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) تین guard rotations میں مستحکم
latency دکھاتا ہے؛ رپورٹ `docs/source/soranet/reports/circuit_stability.md:1` میں
دیکھیں۔

### 1.6 لوکل QUIC proxy

آرکسٹریٹر ایک لوکل QUIC proxy بھی چلا سکتا ہے تاکہ browser extensions اور SDK
adapters کو certificates یا guard cache keys manage نہ کرنا پڑیں۔ proxy loopback
ایڈریس پر bind ہوتا ہے، QUIC connections ختم کرتا ہے، اور کلائنٹ کو Norito
manifest واپس کرتا ہے جو certificate اور optional guard cache key بیان کرتا ہے۔
proxy کے emit کردہ transport events کو `sorafs_orchestrator_transport_events_total`
میں شمار کیا جاتا ہے۔

آرکسٹریٹر JSON میں نئے `local_proxy` بلاک سے proxy enable کریں:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` کنٹرول کرتا ہے کہ proxy کہاں listen کرے (پورٹ `0` سے ephemeral
  پورٹ مانگیں)۔
- `telemetry_label` میٹرکس میں propagate ہوتا ہے تاکہ dashboards proxies اور
  fetch sessions میں فرق کر سکیں۔
- `guard_cache_key_hex` (اختیاری) proxy کو وہی keyed guard cache دکھانے دیتا ہے
  جو CLI/SDKs استعمال کرتے ہیں، تاکہ browser extensions sync رہیں۔
- `emit_browser_manifest` اس بات کو toggle کرتا ہے کہ handshake ایسی manifest
  واپس کرے جسے extensions ذخیرہ/verify کر سکیں۔
- `proxy_mode` طے کرتا ہے کہ proxy مقامی طور پر bridge کرے (`bridge`) یا صرف
  metadata emit کرے تاکہ SDKs خود SoraNet circuits کھولیں (`metadata-only`).
  ڈیفالٹ `bridge` ہے؛ جب صرف manifest دینا ہو تو `metadata-only` سیٹ کریں۔
- `prewarm_circuits`, `max_streams_per_circuit`, `circuit_ttl_hint_secs` اضافی
  hints دیتے ہیں تاکہ براؤزر parallel streams بجٹ کر سکے اور proxy کے circuit
  reuse رویے کو سمجھ سکے۔
- `car_bridge` (اختیاری) مقامی CAR archive cache کی طرف اشارہ کرتا ہے۔ `extension`
  اس suffix کو کنٹرول کرتا ہے جو `*.car` غائب ہونے پر لگایا جاتا ہے؛ `allow_zst = true`
  سے `*.car.zst` براہ راست serve ہو سکتا ہے۔
- `kaigi_bridge` (اختیاری) Kaigi routes کو proxy پر expose کرتا ہے۔ `room_policy`
  بتاتا ہے کہ bridge `public` ہے یا `authenticated` تاکہ browser clients درست
  GAR labels چن سکیں۔
- `sorafs_cli fetch` `--local-proxy-mode=bridge|metadata-only` اور
  `--local-proxy-norito-spool=PATH` overrides فراہم کرتا ہے، جس سے JSON پالیسی
  بدلے بغیر runtime mode یا spool تبدیل ہو سکتے ہیں۔
- `downgrade_remediation` (اختیاری) خودکار downgrade hook کو کنفیگر کرتا ہے۔
  جب enabled ہو تو آرکسٹریٹر relay telemetry میں downgrade bursts دیکھتا ہے اور
  `window_secs` کے اندر `threshold` تجاوز ہونے پر local proxy کو `target_mode`
  (ڈیفالٹ `metadata-only`) پر مجبور کرتا ہے۔ جب downgrades رک جائیں تو proxy
  `cooldown_secs` کے بعد `resume_mode` پر واپس آتا ہے۔ `modes` array کو specific
  relay roles تک محدود کرنے کے لیے استعمال کریں (ڈیفالٹ entry relays)۔

جب proxy bridge mode میں چلتا ہے تو دو application services فراہم کرتا ہے:

- **`norito`** — کلائنٹ کا stream target `norito_bridge.spool_dir` کے نسبت resolve
  ہوتا ہے۔ targets کو sanitize کیا جاتا ہے (traversal اور absolute paths نہیں)
  اور اگر فائل میں extension نہیں تو configured suffix لگایا جاتا ہے، پھر payload
  براؤزر کو stream کیا جاتا ہے۔
- **`car`** — stream targets `car_bridge.cache_dir` کے اندر resolve ہوتے ہیں،
  default extension inherit کرتے ہیں، اور compressed payloads کو رد کرتے ہیں جب تک
  `allow_zst` فعال نہ ہو۔ کامیاب bridges `STREAM_ACK_OK` کے ساتھ جواب دیتے ہیں
  اور پھر archive bytes بھیجتے ہیں تاکہ کلائنٹس verification کو pipeline کر سکیں۔

دونوں صورتوں میں proxy cache-tag HMAC مہیا کرتا ہے (جب guard cache key handshake
کے دوران موجود ہو) اور `norito_*` / `car_*` telemetry reason codes ریکارڈ کرتا ہے
تاکہ dashboards کامیابی، missing files، اور sanitization failures کو فوری
سمجھ سکیں۔

`Orchestrator::local_proxy().await` چلتے ہوئے handle کو expose کرتا ہے تاکہ
کالرز certificate PEM پڑھ سکیں، browser manifest حاصل کر سکیں، یا ایپلیکیشن
ختم ہونے پر graceful shutdown درخواست کر سکیں۔

جب enabled ہو تو proxy اب **manifest v2** ریکارڈز سرور کرتا ہے۔ موجودہ certificate
اور guard cache key کے علاوہ v2 یہ شامل کرتا ہے:

- `alpn` (`"sorafs-proxy/1"`) اور `capabilities` array تاکہ کلائنٹس stream
  پروٹوکول کی تصدیق کر سکیں۔
- ہر handshake کے لیے `session_id` اور `cache_tagging` block تاکہ per-session
  guard affinities اور HMAC tags مشتق کیے جا سکیں۔
- circuit اور guard-selection hints (`circuit`, `guard_selection`, `route_hints`)
  تاکہ browser integrations streams کھلنے سے پہلے richer UI دکھا سکیں۔
- `telemetry_v2` مقامی instrumentation کے لیے sampling/Privacy knobs کے ساتھ۔
- ہر `STREAM_ACK_OK` میں `cache_tag_hex` شامل ہوتا ہے۔ کلائنٹس اسے
  `x-sorafs-cache-tag` header میں mirror کرتے ہیں تاکہ cached guard selections
  rest میں encrypted رہیں۔

اور v1 subset پر انحصار جاری رکھ سکتے ہیں۔

## 2. Failure semantics

آرکسٹریٹر ایک بائٹ منتقل ہونے سے پہلے سخت capability اور budget checks نافذ کرتا
ہے۔ ناکامیاں تین زمروں میں آتی ہیں:

1. **Eligibility failures (pre-flight).** range capability نہ رکھنے والے، expired
   adverts، یا stale telemetry والے providers کو scoreboard artifact میں log کیا
   جاتا ہے اور scheduling سے نکال دیا جاتا ہے۔ CLI summaries `ineligible_providers`
   array میں reasons بھر دیتی ہیں تاکہ operators logs scrape کیے بغیر governance
   drift دیکھ سکیں۔
2. **Runtime exhaustion.** ہر provider مسلسل failures کو track کرتا ہے۔ جب
   `provider_failure_threshold` پہنچ جائے تو provider کو سیشن کے باقی حصے کے لیے
   `disabled` کر دیا جاتا ہے۔ اگر تمام providers `disabled` ہو جائیں تو آرکسٹریٹر
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }` واپس کرتا ہے۔
3. **Deterministic aborts.** سخت حدود structured errors کے طور پر ظاہر ہوتی ہیں:
   - `MultiSourceError::NoCompatibleProviders` — manifest ایسی chunk span یا
     alignment مانگتا ہے جسے باقی providers honor نہیں کر سکتے۔
   - `MultiSourceError::ExhaustedRetries` — per-chunk retry budget ختم ہو گیا۔
   - `MultiSourceError::ObserverFailed` — downstream observers (streaming hooks)
     نے verified chunk رد کر دیا۔

ہر error offending chunk index اور جہاں ممکن ہو final provider failure reason
کے ساتھ آتا ہے۔ انہیں release blockers سمجھیں — انہی inputs کے ساتھ retries
تب تک failure دہراتی رہیں گے جب تک advert، telemetry یا provider health نہ بدلے۔

### 2.1 Scoreboard persistence

جب `persist_path` کنفیگر ہو تو آرکسٹریٹر ہر رن کے بعد final scoreboard لکھتا ہے۔
JSON دستاویز میں شامل ہے:

- `eligibility` (`eligible` یا `ineligible::<reason>`).
- `weight` (اس رن کے لیے assigned normalized weight).
- `provider` metadata (identifier، endpoints، concurrency budget).

Scoreboard snapshots کو release artifacts کے ساتھ archive کریں تاکہ blacklist
اور rollout فیصلے audit-able رہیں۔

## 3. Telemetry اور Debugging

### 3.1 Prometheus metrics

آرکسٹریٹر `iroha_telemetry` کے ذریعے درج ذیل میٹرکس emit کرتا ہے:

| Metric | Labels | Description |
|--------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | in-flight orchestrated fetches کا gauge۔ |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | end-to-end fetch latency ریکارڈ کرنے والا histogram۔ |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | terminal failures کا counter (retries exhausted, no providers, observer failure)۔ |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | فی-provider retry attempts کا counter۔ |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | session-level provider failures کا counter جو disablement تک لے جائیں۔ |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | anonymity policy فیصلوں کی count (meet vs brownout) stage اور fallback reason کے مطابق۔ |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | منتخب SoraNet set میں PQ relay share کا histogram۔ |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | scoreboard snapshot میں PQ relay supply ratios کا histogram۔ |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | policy shortfall (target اور actual PQ share کے فرق) کا histogram۔ |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | ہر سیشن میں classical relay share کا histogram۔ |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | فی سیشن منتخب classical relays کی تعداد کا histogram۔ |

Production knobs آن کرنے سے پہلے metrics کو staging dashboards میں شامل کریں۔
تجویز کردہ layout SF-6 observability plan کو follow کرتا ہے:

1. **Active fetches** — اگر gauge completions کے بغیر بڑھے تو الرٹ۔
2. **Retry ratio** — جب `retry` counters تاریخی baselines سے بڑھیں تو warning۔
3. **Provider failures** — 15 منٹ میں کسی provider کے `session_failure > 0` پر
   pager alerts۔

### 3.2 Structured log targets

آرکسٹریٹر deterministic targets پر structured events شائع کرتا ہے:

- `telemetry::sorafs.fetch.lifecycle` — `start` اور `complete` lifecycle markers
  کے ساتھ chunk counts، retries، اور total duration۔
- `telemetry::sorafs.fetch.retry` — retry events (`provider`, `reason`, `attempts`).
- `telemetry::sorafs.fetch.provider_failure` — repeated errors کی وجہ سے disabled
  providers۔
- `telemetry::sorafs.fetch.error` — `reason` اور optional provider metadata کے
  ساتھ terminal failures۔

ان streams کو موجودہ Norito log pipeline میں forward کریں تاکہ incident response
کو ایک ہی source of truth ملے۔ lifecycle events PQ/classical mix کو
`anonymity_effective_policy`, `anonymity_pq_ratio`, `anonymity_classical_ratio`
اور متعلقہ counters کے ذریعے ظاہر کرتے ہیں، جس سے dashboards کو metrics scrape
کیے بغیر wire کرنا آسان ہوتا ہے۔ GA rollouts کے دوران lifecycle/retry events
کے لیے log level `info` رکھیں اور terminal errors کے لیے `warn` استعمال کریں۔

### 3.3 JSON summaries

`sorafs_cli fetch` اور Rust SDK دونوں structured summary واپس کرتے ہیں، جس میں:

- `provider_reports` مع success/failure counts اور provider disable ہونے کی حالت۔
- `chunk_receipts` جو دکھاتے ہیں کہ کس provider نے کون سا chunk پورا کیا۔
- `retry_stats` اور `ineligible_providers` arrays۔

مشکوک providers کو debug کرتے وقت summary file archive کریں — receipts اوپر
دیے گئے log metadata سے براہ راست جڑتے ہیں۔

## 4. Operational checklist

1. **CI میں کنفیگریشن stage کریں۔** `sorafs_fetch` کو target config کے ساتھ چلائیں،
   eligibility view کے لیے `--scoreboard-out` دیں، اور پچھلے release سے diff کریں۔
   کوئی غیر متوقع ineligible provider promotion روک دیتا ہے۔
2. **Telemetry ویلیڈیٹ کریں۔** multi-source fetches enable کرنے سے پہلے یقینی
   بنائیں کہ deployment `sorafs.fetch.*` metrics اور structured logs export کر رہا
   ہے۔ metrics کی عدم موجودگی عموماً اس بات کا اشارہ ہے کہ orchestrator facade
   invoke نہیں ہوئی۔
3. **Overrides دستاویزی بنائیں۔** ہنگامی `--deny-provider` یا `--boost-provider`
   لگاتے وقت JSON (یا CLI invocation) کو change log میں commit کریں۔ rollbacks کو
   override revert کرنا اور نیا scoreboard snapshot capture کرنا چاہیے۔
4. **Smoke tests دوبارہ چلائیں۔** retry budgets یا provider caps بدلنے کے بعد
   canonical fixture (`fixtures/sorafs_manifest/ci_sample/`) پر دوبارہ fetch کریں
   اور verify کریں کہ chunk receipts deterministic رہیں۔

اوپر دیے گئے مراحل آرکسٹریٹر کے رویے کو staged rollouts میں reproducible رکھتے
ہیں اور incident response کے لیے ضروری telemetry فراہم کرتے ہیں۔

### 4.1 Policy overrides

Operators transport/anonymity stage کو base configuration تبدیل کیے بغیر pin
کر سکتے ہیں، بس `policy_override.transport_policy` اور
`policy_override.anonymity_policy` کو اپنے `orchestrator` JSON میں سیٹ کریں (یا
`sorafs_cli fetch` کو `--transport-policy-override=` / `--anonymity-policy-override=`
پاس کریں)۔ جب override موجود ہو تو آرکسٹریٹر usual brownout fallback کو skip
کرتا ہے: اگر مطلوبہ PQ tier فراہم نہ ہو تو fetch `no providers` کے ساتھ فیل
ہوتا ہے بجائے خاموشی سے downgrade ہونے کے۔ default behavior پر واپسی اتنی ہی
آسان ہے جتنی override fields کو صاف کرنا۔
