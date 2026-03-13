---
lang: ur
direction: rtl
source: docs/norito_demo_contributor.md
status: complete
translator: manual
source_hash: b11d23ecafbc158e0c83cdb6351085fde02f362cfc73a1a1a33555e90cc556ef
source_last_modified: "2025-11-09T09:04:55.207331+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- اردو ترجمہ: docs/norito_demo_contributor.md (Norito SwiftUI Demo Contributor Guide) -->

# Norito SwiftUI ڈیمو کنٹریبیوٹر گائیڈ

یہ دستاویز ان دستی (manual) سیٹ اپ steps کو بیان کرتی ہے جو SwiftUI demo کو مقامی
Torii node اور mock ledger کے خلاف چلانے کے لیے درکار ہیں۔ یہ
`docs/norito_bridge_release.md` کی تکمیل کرتی ہے اور روزمرہ development tasks پر
فوکس کرتی ہے۔ Xcode پروجیکٹس میں Norito bridge/Connect stack کو integrate کرنے کے
لیے مزید تفصیلی walkthrough، `docs/connect_swift_integration.md` میں موجود ہے۔

## ماحول کی ترتیب (Environment setup)

1. Rust toolchain انسٹال کریں جیسا کہ `rust-toolchain.toml` میں define کیا گیا ہے۔
2. Swift 5.7+ اور Xcode Command Line Tools کو macOS پر انسٹال کریں۔
3. (اختیاری) Linting کے لیے [SwiftLint](https://github.com/realm/SwiftLint) انسٹال
   کریں۔
4. `cargo build -p irohad` چلائیں تاکہ یہ یقین کیا جا سکے کہ node آپ کے host پر کامیابی
   سے build ہو جاتی ہے۔
5. `examples/ios/NoritoDemoXcode/Configs/demo.env.example` کو `.env` میں copy کریں اور اپنے
   ماحول کے مطابق values کو adjust کریں۔ ایپ launch کے وقت ان متغیرات کو پڑھتی ہے:
   - `TORII_NODE_URL` — base REST URL (WebSocket URLs اسی سے derive ہوتے ہیں)۔
   - `CONNECT_SESSION_ID` — 32‑byte سیشن آئی ڈی (base64/base64url)۔
   - `CONNECT_TOKEN_APP` / `CONNECT_TOKEN_WALLET` — tokens جو `/v2/connect/session` سے
     واپس آتے ہیں۔
   - `CONNECT_CHAIN_ID` — chain identifier جو control handshake کے دوران announce ہوتا ہے۔
   - `CONNECT_ROLE` — UI میں pre‑selected default role (`app` یا `wallet`)۔
   - manual testing کے لیے optional helpers:
     `CONNECT_PEER_PUB_B64`, `CONNECT_SHARED_KEY_B64`, `CONNECT_APPROVE_ACCOUNT_ID`,
     `CONNECT_APPROVE_PRIVATE_KEY_B64`, `CONNECT_APPROVE_SIGNATURE_B64`۔

## Torii + mock ledger کو bootstrap کرنا

ریپوزٹری میں helper scripts شامل ہیں جو Torii node کو in‑memory ledger کے ساتھ start
کرتے ہیں جس میں demo accounts پہلے سے load ہوتے ہیں:

```bash
./scripts/ios_demo/start.sh --config examples/ios/NoritoDemoXcode/Configs/SampleAccounts.json
```

یہ اسکرپٹ درج ذیل جنریٹ کرتا ہے:

- Torii node logs، `artifacts/torii.log` میں۔
- ledger metrics (Prometheus format)، `artifacts/metrics.prom` میں۔
- client access tokens، `artifacts/torii.jwt` میں۔

`start.sh`، demo peer کو چلتا ہوا رکھتا ہے جب تک آپ `Ctrl+C` نہ دبائیں۔ یہ
`artifacts/ios_demo_state.json` میں ready‑state snapshot لکھتا ہے (جو دیگر artefacts کے
لیے source of truth ہے)، فعال Torii stdout log کو copy کرتا ہے، `/metrics` پر polling
کرتا ہے جب تک Prometheus scrape دستیاب نہ ہو جائے، اور configured accounts کو
`torii.jwt` میں render کرتا ہے (اگر config میں private keys دی گئی ہوں تو وہ بھی شامل
ہوتی ہیں)۔ یہ اسکرپٹ `--artifacts` (output directory override)، `--telemetry-profile`
اور non‑interactive CI jobs کے لیے `--exit-after-ready` arguments کو بھی سپورٹ کرتا ہے۔

ہر entry in `SampleAccounts.json` درج ذیل فیلڈز کو سپورٹ کرتی ہے:

- `name` (string, optional) — account metadata میں `alias` کے نام سے محفوظ ہوتا ہے۔
- `public_key` (multihash string, required) — account کے signatory کے طور پر استعمال
  ہوتا ہے۔
- `private_key` (optional) — client credentials generation کے لیے `torii.jwt` میں شامل
  کیا جاتا ہے۔
- `domain` (optional) — اگر چھوڑ دیا جائے تو asset domain default کے طور پر استعمال ہوتا
  ہے۔
- `asset_id` (string, required) — asset definition جسے اس account کے لیے mint کیا جائے گا۔
- `initial_balance` (string, required) — numeric amount جو account میں mint ہوتی ہے۔

## SwiftUI ڈیمو چلانا

1. XCFramework کو `docs/norito_bridge_release.md` میں بیان کردہ steps کے مطابق build
   کریں اور اسے demo project میں bundle کریں (project references، `NoritoBridge.xcframework`
   کو project root میں assume کرتی ہیں)۔
2. `NoritoDemoXcode` project کو Xcode میں کھولیں۔
3. `NoritoDemo` scheme منتخب کریں اور target کو iOS simulator یا device پر سیٹ کریں۔
4. اس بات کی تصدیق کریں کہ `.env`، scheme environment variables کے ذریعے reference ہو
   رہا ہے۔ `CONNECT_*` values کو `/v2/connect/session` سے حاصل شدہ tokens سے populate
   کریں تاکہ app launch کے وقت UI پہلے سے filled ہو۔
5. hardware acceleration defaults کی تصدیق کریں: `App.swift` میں
   `DemoAccelerationConfig.load().apply()` کال کی جاتی ہے جس کے نتیجے میں demo، environment
   override `NORITO_ACCEL_CONFIG_PATH` یا bundled `acceleration.{json,toml}`/
   `client.{json,toml}` فائلوں کو pick کرتا ہے۔ اگر آپ زبردستی CPU fallback موڈ کو
   enforce کرنا چاہتے ہیں تو ان inputs کو ہٹا دیں یا adjust کریں۔
6. application کو build اور launch کریں۔ home screen، Torii URL/token کے لیے prompts
   دکھائے گی اگر وہ `.env` کے ذریعے پہلے سے سیٹ نہ کیے گئے ہوں۔
7. اکاؤنٹ اپ ڈیٹس کو subscribe کرنے یا requests کو approve کرنے کے لیے ایک “Connect”
   session شروع کریں۔
8. ایک IRH transfer submit کریں اور on‑screen log output کے ساتھ Torii logs بھی چیک
   کریں۔

### Hardware acceleration toggles (Metal / NEON)

`DemoAccelerationConfig`، Rust node configuration کو reflect کرتا ہے تاکہ developers
Metal/NEON code paths کو exercise کر سکیں، بغیر thresholds کو hard‑code کیے۔ loader،
launch کے وقت کنفیگریشنز کو درج ذیل ترتیب سے تلاش کرتا ہے:

1. `NORITO_ACCEL_CONFIG_PATH` (جو `.env`/scheme arguments میں define ہوتی ہے) — ایک
   absolute path یا `~` سے شروع ہونے والا path جو `iroha_config` JSON/TOML فائل کی
   طرف اشارہ کرتا ہے۔
2. bundled config فائلز جن کے نام `acceleration.{json,toml}` یا `client.{json,toml}` ہوں۔
3. اگر کوئی source نہ ملے تو default settings (`AccelerationSettings()`) استعمال میں
   رہتی ہیں۔

`acceleration.toml` کی مثال:

```toml
[accel]
enable_metal = true
merkle_min_leaves_metal = 256
prefer_cpu_sha2_max_leaves_aarch64 = 128
```

فیلڈز کو nil چھوڑنے سے workspace defaults inherit ہو جاتے ہیں۔ منفی نمبرز کو ignore
کیا جاتا ہے، اور `[accel]` سیکشن نہ ہونے کی صورت میں deterministic CPU behaviour
لاگو رہتا ہے۔ جب Metal support کے بغیر simulator پر چلایا جائے تو bridge خاموشی سے
scalar path ہی رکھتا ہے، چاہے config، Metal کی درخواست کرے۔

## Integration tests

- Integration tests کا منصوبہ `Tests/NoritoDemoTests` میں ہے (macOS CI دستیاب ہونے کے
  بعد شامل کیے جائیں گے)۔
- Tests، Torii کو مذکورہ بالا scripts کے ذریعے spin up کریں گے اور WebSocket
  subscriptions، token balances اور transfer flows کو Swift package کے ذریعے exercise
  کریں گے۔
- Test runs کے logs کو `artifacts/tests/<timestamp>/` میں metrics اور sample ledger dumps
  کے ساتھ محفوظ کیا جائے گا۔

## CI parity checks

- کسی بھی PR سے پہلے جو demo یا shared fixtures میں تبدیلی کرے، `make swift-ci` چلائیں۔
  یہ target fixture parity checks انجام دیتا ہے، dashboard feeds کو validate کرتا ہے،
  اور summaries مقامی طور پر render کرتا ہے۔ CI میں یہی workflow Buildkite metadata
  (`ci/xcframework-smoke:<lane>:device_tag`) پر انحصار کرتا ہے، تاکہ نتائج کو صحیح
  simulator یا StrongBox lane کے نام کے ساتھ associate کیا جا سکے؛ اگر آپ pipeline یا
  agent tags کو adjust کریں تو metadata کی موجودگی ضرور verify کریں۔
- اگر `make swift-ci` fail ہو جائے تو `docs/source/swift_parity_triage.md` میں دی گئی
  ہدایات پر عمل کریں اور `mobile_ci` آؤٹ پٹ کا جائزہ لیں تاکہ معلوم ہو سکے کہ کون سا
  lane دوبارہ generate یا مزید incident follow‑up کا محتاج ہے۔

## Troubleshooting

- اگر demo، Torii سے connect نہ کر سکے تو node URL اور TLS settings verify کریں۔
- یقین کر لیں کہ JWT token (اگر استعمال میں ہے) valid ہے اور expire نہیں ہو چکا۔
- `artifacts/torii.log` میں server‑side errors تلاش کریں۔
- WebSocket issues کی صورت میں client کی log window یا Xcode console output چیک کریں۔

</div>

