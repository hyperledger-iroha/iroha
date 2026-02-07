---
lang: mn
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha Холбох жишээнүүд (Rust App/Wallet)

Хоёр Rust жишээг Torii зангилаагаар төгсгөлөөс нь ажиллуул.

Урьдчилсан нөхцөл
- `connect`-тэй Torii зангилаа `http://127.0.0.1:8080` дээр идэвхжсэн.
- Зэв багажны оосор (тогтвортой).
- `iroha-python` багц суулгасан Python 3.9+ (доорх CLI туслагчийн хувьд).

Жишээ
- Хэрэглээний жишээ: `crates/iroha_torii_shared/examples/connect_app.rs`
- Түрийвчний жишээ: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI туслах: `python -m iroha_python.examples.connect_flow`

Эхлэх дараалал
1) Терминал А - Апп (sid + жетон хэвлэдэг, WS-г холбодог, SignRequestTx илгээдэг):

    cargo run -p iroha_torii_shared --example connect_app -- --node http://127.0.0.1:8080 --role app

   Жишээ гаралт:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS холбогдсон
    програм: SignRequestTx илгээсэн
    (хариу хүлээж байна)

2) Терминал В — Түрийвч (token_wallet-тай холбогдож, SignResultOk-ээр хариулна):

    cargo run -p iroha_torii_shared --example connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   Жишээ гаралт:

    түрийвч: холбогдсон WS
    түрийвч: SignRequestTx len=3 дараалал 1
    түрийвч: SignResultOk илгээсэн

3) Програмын терминал үр дүнг хэвлэнэ:

    програм: SignResultOk algo=ed25519 sig=deadbeef авсан

  Шинэ `connect_norito_decode_envelope_sign_result_alg` туслах (болон
  Энэ хавтсанд байгаа Swift/Kotlin боодол) алгоритмын мөрийг сэргээх
  ачааллын кодыг тайлах.

Тэмдэглэл
- Жишээ нь `sid`-ээс түр зуурын демо хувилбаруудыг гаргаж авсан тул програм/түрийвч автоматаар харилцан ажилладаг. Үйлдвэрлэлд бүү ашигла.
- SDK нь AEAD AAD-ийн холболт болон дарааллыг хэрэгжүүлдэг; Зөвшөөрлийн дараах хяналтын хүрээг шифрлэсэн байх ёстой.
- Swift-н үйлчлүүлэгчдийн хувьд `docs/connect_swift_integration.md` / `docs/connect_swift_ios.md`-г харж, `make swift-ci`-р баталгаажуулснаар хяналтын самбарын телеметр нь Rust жишээнүүдтэй нийцэж, Buildkite мета өгөгдөл (`ci/xcframework-smoke:<lane>:device_tag`) хэвээр үлдэнэ.
- Python CLI туслах хэрэглээ:

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

  CLI нь бичсэн сессийн мэдээллийг хэвлэж, Холболтын төлөвийн агшин зуурын зургийг буулгаж, Norito кодлогдсон `ConnectControlOpen` фреймийг гаргадаг. Ачааллыг Torii руу буцаан байршуулахын тулд `--send-open`-ийг дамжуулж, түүхий байт бичихийн тулд `--frame-output-format binary`, base64-д ээлтэй JSON blob-ийн хувьд `--frame-json-output`, мөн автоматаар шивэх шаардлагатай үед I18NI00000025. Та мөн `name`, `url`, `icon_hash` талбаруудыг агуулсан `--app-metadata-file metadata.json`-ээр дамжуулан JSON файлаас програмын мета өгөгдлийг ачаалж болно (`python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`-г үзнэ үү). `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json`-ээр шинэ загвар үүсгээрэй. Зөвхөн телеметрийн гүйлтүүдийн хувьд та `--status-only`-р сесс үүсгэхийг бүхэлд нь алгасаж, JSON-ийг `--status-json-output status.json`-ээр дамжуулан хаях боломжтой.