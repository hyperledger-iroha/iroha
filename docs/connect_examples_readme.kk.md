---
lang: kk
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha Қосылу мысалдары (Rust App/Wallet)

Екі Rust мысалын Torii түйінімен соңына дейін іске қосыңыз.

Алғы шарттар
- `connect` бар Torii түйіні `http://127.0.0.1:8080` параметрінде қосылған.
- Тоттан жасалған құралдар тізбегі (тұрақты).
- `iroha-python` пакеті орнатылған Python 3.9+ (төмендегі CLI көмекшісі үшін).

Мысалдар
- Қолданба мысалы: `crates/iroha_torii_shared/examples/connect_app.rs`
- Әмиян мысалы: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI көмекшісі: `python -m iroha_python.examples.connect_flow`

Іске қосу тәртібі
1) A терминалы — қолданба (sid + таңбалауыштарды басып шығарады, WS қосады, SignRequestTx жібереді):

    жүк тасымалдау -p iroha_torii_shared --мысал connect_app -- --түйін http://127.0.0.1:8080 --рөл қолданбасы

   Шығару үлгісі:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS қосылды
    қолданба: SignRequestTx жіберілді
    (жауап күту)

2) B терминалы — әмиян (token_wallet арқылы қосылыңыз, SignResultOk арқылы жауап береді):

    жүк жіберу -p iroha_torii_shared --мысал connect_wallet -- --түйін http://127.0.0.1:8080 --sid Z4... --token K0...

   Шығару үлгісі:

    әмиян: қосылған WS
    әмиян: SignRequestTx len=3 1-кезеңде
    әмиян: SignResultOk жіберілді

3) Қолданба терминалы нәтижені басып шығарады:

    қолданба: SignResultOk алды algo=ed25519 sig=deadbeef

  Жаңа `connect_norito_decode_envelope_sign_result_alg` көмекшісін (және
  Осы қалтадағы Swift/Kotlin орауыштары) алгоритм жолын алу үшін
  пайдалы жүктемені декодтау.

Ескертпелер
- Мысалдар `sid` нұсқасынан демонстрациялық эфемерлерді алады, осылайша қолданба/әмиян автоматты түрде өзара әрекеттеседі. Өндірісте қолдануға болмайды.
- SDK AEAD AAD байланыстыруын және ретсіздігін қамтамасыз етеді; мақұлданғаннан кейін басқару кадрлары шифрлануы керек.
- Swift клиенттері үшін `docs/connect_swift_integration.md` / `docs/connect_swift_ios.md` бөлімін қараңыз және `make swift-ci` арқылы растаңыз, осылайша бақылау тақтасының телеметриясы Rust мысалдарымен және Buildkite метадеректері (`ci/xcframework-smoke:<lane>:device_tag`) өзгеріссіз қалады.
- Python CLI көмекшісін пайдалану:

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

  CLI терілген сеанс ақпаратын басып шығарады, Қосылу күйінің суретін шығарады және Norito кодталған `ConnectControlOpen` кадрын шығарады. Пайдалы жүктемені Torii ішіне қайта жіберу үшін `--send-open` өтіңіз, өңделмеген байтты жазу үшін `--frame-output-format binary` пайдаланыңыз, base64 үшін қолайлы JSON blob үшін `--frame-json-output` және автоматты түрде теру үшін I18NI00000025. `name`, `url` және `icon_hash` өрістерін қамтитын `--app-metadata-file metadata.json` арқылы JSON файлынан қолданба метадеректерін жүктей аласыз (`python/iroha_python/src/iroha_python/examples/connect_app_metadata.json` қараңыз). `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json` көмегімен жаңа үлгіні жасаңыз. Тек телеметрияға арналған іске қосулар үшін `--status-only` арқылы сеанс жасауды толығымен өткізіп жіберуге және қосымша JSON `--status-json-output status.json` арқылы тастауға болады.