---
lang: az
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha Qoşulma Nümunələri (Rust App/Wallet)

Torii node ilə iki Rust nümunəsini başdan-başa işlədin.

İlkin şərtlər
- `http://127.0.0.1:8080`-də aktivləşdirilmiş `connect` ilə Torii qovşağı.
- Pas alət zənciri (sabit).
- `iroha-python` paketi quraşdırılmış Python 3.9+ (aşağıdakı CLI köməkçisi üçün).

Nümunələr
- Tətbiq nümunəsi: `crates/iroha_torii_shared/examples/connect_app.rs`
- Pul kisəsi nümunəsi: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI köməkçisi: `python -m iroha_python.examples.connect_flow`

Başlanğıc sifarişi
1) Terminal A — Tətbiq (sid + tokenləri çap edir, WS-ni birləşdirir, SignRequestTx göndərir):

    cargo run -p iroha_torii_shared --misal connect_app -- --node http://127.0.0.1:8080 --rol proqramı

   Nümunə çıxışı:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS bağlıdır
    proqram: SignRequestTx göndərildi
    (cavab gözləyir)

2) Terminal B — Pul kisəsi (token_wallet ilə əlaqə qurun, SignResultOk ilə cavab verir):

    yük axını -p iroha_torii_shared --misal connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   Nümunə çıxışı:

    cüzdan: qoşulmuş WS
    pul kisəsi: SignRequestTx len=3 ardıcıl 1-də
    pul kisəsi: SignResultOk göndərildi

3) Tətbiq terminalı nəticəni çap edir:

    proqram: SignResultOk algo=ed25519 sig=deadbeef əldə etdi

  Yeni `connect_norito_decode_envelope_sign_result_alg` köməkçisindən (və
  Bu qovluqdakı Swift/Kotlin sarğıları) alqoritm sətirini əldə etmək üçün
  faydalı yükün dekodlanması.

Qeydlər
- Nümunələr `sid`-dən demo efemerləri əldə edir ki, proqram/pul kisəsi avtomatik olaraq qarşılıqlı fəaliyyət göstərsin. İstehsalda istifadə etməyin.
- SDK AEAD AAD bağlamasını və ardıcıllığı təmin edir; təsdiqdən sonra nəzarət çərçivələri şifrələnməlidir.
- Swift müştəriləri üçün `docs/connect_swift_integration.md` / `docs/connect_swift_ios.md`-ə baxın və `make swift-ci` ilə təsdiq edin ki, tablosuna telemetriya Rust nümunələri ilə uyğunlaşdırılsın və Buildkite metadatası (`ci/xcframework-smoke:<lane>:device_tag`) təsirsiz qalsın.
- Python CLI köməkçisinin istifadəsi:

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

  CLI yığılmış sessiya məlumatını çap edir, Qoşulma statusunun şəklini atır və Norito kodlu `ConnectControlOpen` çərçivəsini buraxır. Faydalı yükü yenidən Torii-ə göndərmək üçün `--send-open`-i keçin, xam baytları yazmaq üçün `--frame-output-format binary`-dən, base64-ə uyğun JSON blob üçün `--frame-json-output`-dən istifadə edin və sizə avtomatik rejimdə çəkiliş üçün lazım olduqda I18NI00000025. Siz həmçinin `name`, `url` və `icon_hash` sahələrini ehtiva edən `--app-metadata-file metadata.json` vasitəsilə JSON faylından tətbiq metadatasını yükləyə bilərsiniz (bax: `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`). `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json` ilə təzə şablon yaradın. Yalnız telemetriya ilə işləyənlər üçün siz `--status-only` ilə sessiya yaradılmasını tamamilə atlaya və isteğe bağlı olaraq `--status-json-output status.json` vasitəsilə JSON-u buraxa bilərsiniz.