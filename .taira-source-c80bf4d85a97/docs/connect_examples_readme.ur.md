---
lang: ur
direction: rtl
source: docs/connect_examples_readme.md
status: complete
translator: manual
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-11-02T04:40:28.804038+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- اردو ترجمہ: docs/connect_examples_readme.md (Iroha Connect Examples) -->

## Iroha Connect کی مثالیں (Rust ایپ / والیٹ)

Torii نوڈ کے خلاف دو Rust مثالوں کو اینڈ ٹو اینڈ چلائیں۔

پیشگی ضروریات
- Torii نوڈ جس پر `connect` فعال ہو، پتہ: `http://127.0.0.1:8080`۔
- Rust toolchain (stable)۔
- Python 3.9+ اور `iroha-python` پیکیج انسٹال ہو (نیچے دیے گئے CLI helper کے لیے)۔

مثالیں
- ایپ کی مثال: `crates/iroha_torii_shared/examples/connect_app.rs`
- والیٹ کی مثال: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI helper: `python -m iroha_python.examples.connect_flow`

اسٹارٹ اپ کا ترتیب وار آرڈر
1) ٹرمینل A — ایپ (sid اور tokens پرنٹ کرتی ہے، WS کنیکٹ کرتی ہے، `SignRequestTx` بھیجتی ہے):

    cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   مثال آؤٹ پٹ:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS connected
    app: sent SignRequestTx
    (waiting for reply)

2) ٹرمینل B — والیٹ (`token_wallet` کے ساتھ کنیکٹ ہوتی ہے اور `SignResultOk` بھیجتی ہے):

    cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   مثال آؤٹ پٹ:

    wallet: connected WS
    wallet: SignRequestTx len=3 at seq 1
    wallet: sent SignResultOk

3) ایپ والا ٹرمینل نتیجہ پرنٹ کرتا ہے:

    app: got SignResultOk algo=ed25519 sig=deadbeef

  payload کو ڈی کوڈ کرتے وقت الگورتھم کی سٹرنگ حاصل کرنے کے لیے نیا helper
  `connect_norito_decode_envelope_sign_result_alg` استعمال کریں (اور اسی فولڈر میں Swift
  / Kotlin wrappers بھی)۔

نوٹس
- مثالوں میں ڈیمو کے لیے ephemeral keys، `sid` سے derive کی جاتی ہیں تاکہ ایپ اور والیٹ
  خود بخود interoperable رہیں۔ پروڈکشن میں ایسا مت کریں۔
- SDK، AEAD AAD binding اور `seq` کو nonce کے طور پر سختی سے نافذ کرتا ہے؛ approval کے
  بعد آنے والے control frames کو encrypted ہونا چاہیے۔
- Swift کلائنٹس کے لیے `docs/connect_swift_integration.md` /
  `docs/connect_swift_ios.md` دیکھیں اور `make swift-ci` چلا کر validate کریں، تاکہ
  dashboards کی telemetry، Rust مثالوں کے ساتھ aligned رہے اور Buildkite metadata
  (`ci/xcframework-smoke:<lane>:device_tag`) درست رہے۔
- Python CLI helper کے استعمال کی مثال:

    ```bash
    python -m iroha_python.examples.connect_flow \
      --base-url http://127.0.0.1:8080 \
      --sid demo-session \
      --chain-id dev-chain \
      --auth-token admin-token \
      --app-name "Demo App" \
      --frame-output connect-open.hex \
      --frame-json-output connect-open.json \
      --status-json-output connect-status.json
    ```

  یہ CLI سیشن کی typed معلومات پرنٹ کرتا ہے، Connect کا status snapshot بناتا ہے، اور
  Norito‑encoded `ConnectControlOpen` frame کو آؤٹ پٹ کرتا ہے۔ `--send-open` دینے سے
  payload کو Torii کی طرف واپس بھیجا جاتا ہے؛ `--frame-output-format binary` خام bytes
  لکھنے کے لیے، `--frame-json-output` base64‑friendly JSON blob کے لیے، اور
  `--status-json-output` اس وقت کام آتا ہے جب آپ کو automation کے لیے typed snapshot
  چاہیے۔ آپ `--app-metadata-file metadata.json` کے ذریعے JSON فائل سے application
  metadata بھی لوڈ کر سکتے ہیں جس میں `name`, `url`, اور `icon_hash` فیلڈز ہوں (مزید
  مثال `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json` میں
  دیکھیں)۔ نیا template بنانے کے لیے:
  `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json`
  چلائیں۔ صرف telemetry کے رنز کے لیے، آپ `--status-only` کے ساتھ سیشن کریئےشن کو مکمل
  طور پر اسکپ کر سکتے ہیں اور، اگر چاہیں، `--status-json-output status.json` کے ذریعے
  JSON ڈمپ کر سکتے ہیں۔

</div>

