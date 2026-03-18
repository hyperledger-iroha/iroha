---
lang: uz
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha ulanish misollari (Rust ilovasi/hamyon)

Ikkita Rust misolini Torii tugun bilan uchma-uch ishlating.

Old shartlar
- `http://127.0.0.1:8080` da yoqilgan `connect` bilan Torii tugun.
- Rust asboblar zanjiri (barqaror).
- `iroha-python` to'plami o'rnatilgan Python 3.9+ (quyida CLI yordamchisi uchun).

Misollar
- Ilova misoli: `crates/iroha_torii_shared/examples/connect_app.rs`
- Hamyon misoli: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI yordamchisi: `python -m iroha_python.examples.connect_flow`

Ishga tushirish tartibi
1) A terminali - Ilova (sid + tokenlarni chop etadi, WS-ni ulaydi, SignRequestTx-ni yuboradi):

    yuk tashish -p iroha_torii_shared --misol connect_app -- --tugun http://127.0.0.1:8080 --rol ilovasi

   Chiqish namunasi:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS ulangan
    ilova: SignRequestTx yuborildi
    (javob kutilmoqda)

2) Terminal B — Wallet (token_wallet bilan ulaning, SignResultOk bilan javob beradi):

    yuk tashish -p iroha_torii_shared --misol connect_wallet -- --tugun http://127.0.0.1:8080 --sid Z4... --token K0...

   Chiqish namunasi:

    hamyon: ulangan WS
    hamyon: SignRequestTx len=3 1-qatorda
    hamyon: SignResultOk yubordi

3) Ilova terminali natijani chop etadi:

    ilova: SignResultOk algo=ed25519 sig=deadbeef oldi

  Yangi `connect_norito_decode_envelope_sign_result_alg` yordamchisidan foydalaning (va
  Ushbu jilddagi Swift/Kotlin o'ramlari) algoritm qatorini olish uchun
  foydali yukni dekodlash.

Eslatmalar
- Misollar `sid` dan demo efemerlarni keltirib chiqaradi, shuning uchun ilova/hamyon avtomatik ravishda o'zaro ishlaydi. Ishlab chiqarishda foydalanmang.
- SDK AEAD AAD ulanishini va ketma-ketlikni ta'minlaydi; Tasdiqdan keyin boshqaruv ramkalari shifrlangan bo'lishi kerak.
- Swift mijozlari uchun `docs/connect_swift_integration.md` / `docs/connect_swift_ios.md` ga qarang va `make swift-ci` bilan tasdiqlang, shunda asboblar paneli telemetriyasi Rust misollari bilan mos keladi va Buildkite metamaʼlumotlari (`ci/xcframework-smoke:<lane>:device_tag`) oʻzgarmas qoladi.
- Python CLI yordamchisidan foydalanish:

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

  CLI terilgan seans ma'lumotlarini chop etadi, Ulanish holati suratini o'chiradi va Norito kodli `ConnectControlOpen` ramkasini chiqaradi. Foydali yukni Torii ga qaytarish uchun `--send-open` dan o'ting, xom baytlarni yozish uchun `--frame-output-format binary` dan, base64-ga mos keladigan JSON blobi uchun `--frame-json-output` dan foydalaning va sizga avtomatik ravishda yozib olish uchun I18NI00000025 dan foydalaning. `name`, `url` va `icon_hash` maydonlarini oʻz ichiga olgan `--app-metadata-file metadata.json` orqali JSON faylidan ilova metamaʼlumotlarini ham yuklashingiz mumkin (qarang: `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`). `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json` bilan yangi shablonni yarating. Faqat telemetriya bilan ishlash uchun siz `--status-only` bilan seans yaratishni butunlay o'tkazib yuborishingiz va ixtiyoriy ravishda `--status-json-output status.json` orqali JSONni o'chirib tashlashingiz mumkin.