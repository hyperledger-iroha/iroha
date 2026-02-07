---
lang: ka
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha დაკავშირების მაგალითები (Rust აპლიკაცია/საფულე)

გაუშვით Rust-ის ორი მაგალითი ბოლომდე-ბოლომდე Torii კვანძით.

წინაპირობები
- Torii კვანძი `connect`-ით ჩართული `http://127.0.0.1:8080`-ზე.
- ჟანგის ხელსაწყოების ჯაჭვი (სტაბილური).
- Python 3.9+ დაყენებული `iroha-python` პაკეტით (ქვემოთ CLI დამხმარესთვის).

მაგალითები
- აპლიკაციის მაგალითი: `crates/iroha_torii_shared/examples/connect_app.rs`
- საფულის მაგალითი: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI დამხმარე: `python -m iroha_python.examples.connect_flow`

გაშვების ბრძანება
1) ტერმინალი A — აპლიკაცია (ბეჭდავს sid + ჟეტონებს, აკავშირებს WS-ს, აგზავნის SignRequestTx):

    ტვირთის გაშვება -p iroha_torii_shared --მაგალითი connect_app -- --node http://127.0.0.1:8080 -- როლური აპლიკაცია

   ნიმუშის გამომავალი:

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS დაკავშირებულია
    აპლიკაცია: გაგზავნილი SignRequestTx
    (ველოდები პასუხს)

2) ტერმინალი B — საფულე (დაკავშირება token_wallet-თან, პასუხები SignResultOk-ით):

    ტვირთის გაშვება -p iroha_torii_shared --მაგალითი connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --token K0...

   ნიმუშის გამომავალი:

    საფულე: დაკავშირებული WS
    საფულე: SignRequestTx len=3 მე-1-ზე
    საფულე: გაიგზავნა SignResultOk

3) აპლიკაციის ტერმინალი ბეჭდავს შედეგს:

    აპლიკაცია: მივიღე SignResultOk algo=ed25519 sig=deadbeef

  გამოიყენეთ ახალი `connect_norito_decode_envelope_sign_result_alg` დამხმარე (და
  Swift/Kotlin wrappers ამ საქაღალდეში) ალგორითმის სტრიქონის მოსაძიებლად, როდესაც
  ტვირთის გაშიფვრა.

შენიშვნები
- მაგალითები იღებენ დემო ეფემერულებს `sid`-დან, ასე რომ აპლიკაცია/საფულე ავტომატურად ურთიერთქმედებს. არ გამოიყენოთ წარმოებაში.
- SDK ახორციელებს AEAD AAD სავალდებულო და თანმიმდევრულად არაერთხელ; დამტკიცების შემდგომი კონტროლის ჩარჩოები უნდა იყოს დაშიფრული.
- Swift-ის კლიენტებისთვის იხილეთ `docs/connect_swift_integration.md` / `docs/connect_swift_ios.md` და დაადასტურეთ `make swift-ci`-ით, რათა დაფის ტელემეტრია დარჩეს Rust-ის მაგალითებთან და Buildkite მეტამონაცემები (I18NI000000020X) დარჩეს.
- Python CLI დამხმარე გამოყენება:

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

  CLI ბეჭდავს აკრეფილ სესიის ინფორმაციას, ათავსებს Connect სტატუსის სურათს და გამოსცემს Norito-ში დაშიფრულ `ConnectControlOpen` ჩარჩოს. გადაიტანეთ `--send-open` ტვირთის დასაბრუნებლად Torii-ზე, გამოიყენეთ `--frame-output-format binary` ნედლეული ბაიტების დასაწერად, `--frame-json-output` base64-ისთვის შესაფერისი JSON blob-ისთვის და I18NI000000025-ისთვის ავტომატიზაცია. თქვენ ასევე შეგიძლიათ ჩატვირთოთ აპლიკაციის მეტამონაცემები JSON ფაილიდან `--app-metadata-file metadata.json`-ით, რომელიც შეიცავს `name`, `url` და `icon_hash` ველებს (იხ. `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`). შექმენით ახალი შაბლონი `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json`-ით. მხოლოდ ტელემეტრიით გაშვებისთვის, შეგიძლიათ მთლიანად გამოტოვოთ სესიის შექმნა `--status-only`-ით და სურვილისამებრ გამოაგდოთ JSON `--status-json-output status.json`-ის საშუალებით.