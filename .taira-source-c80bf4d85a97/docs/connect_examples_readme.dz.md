---
lang: dz
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha མཐུད་སྦྲེལ་གྱི་དཔེ་ཚུ་ (Rust App/Wallet)

རཱསིཊི་གི་དཔེ་གཉིས་འདི་ I18NT0000002X མཛུབ་གནོན་དང་གཅིག་ཁར་མཇུག་བསྡུ་ཚུན་ཚོད་མཇུག་བསྡུ་དགོ།

སྔོན་འགྲོའི་ཆ་རྐྱེན།
- I18NI0000003X གིས་ I18NI000000009X ལྕོགས་ཅན་བཟོ་སྟེ་ I18NI000000010X ལུ་ལྕོགས་ཅན་བཟོ་ཡོདཔ་ཨིན།
- རསཊི་ལག་ཆས་རྒྱུན་ (བརྟན་ཏོག་ཏོ་)།
- I18NI000000011X ཐུམ་སྒྲིལ་གཞི་བཙུགས་འབད་མི་ Python 3.9+ (འོག་གི་སི་ཨེལ་ཨའི་གྲོགས་རམ་གྱི་དོན་ལུ་)།

དཔེ་ཚུ།
- App དཔེར་ན: `crates/iroha_torii_shared/examples/connect_app.rs`
- ཝ་ལེཊ་དཔེ་: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- པའི་ཐོན་སི་ཨེལ་ཨའི་གྲོགས་རམ་པ།: I18NI000000014X

འགོ་འབྱེད་ཀྱི་གོ་རིམ་
༡༽ ཊར་མི་ནཱལ་ཨེ་ — ཨེཔ་ (དཔར་བསྐྲུན་གྱི་སིཌི་ + ཊོ་ཀེན་ཚུ་ ཌབ་ལུ་ཨེསི་མཐུད) SignRequestTx གཏངམ་ཨིན།

    སྐྱེལ་འདྲེན་ -p iroha_tori_shared --དཔེར་ན་ མཐུད་བྱེད་_ཨེཔ་ ---node http://127.0.0.1:8080 --role app .

   དཔེ་ཚད་ཀྱི་ཐོན་འབྲས་:

    sid=Z4... ཊོ་ཀེན་_ཨེཔ་=ཀེ་ཇེ་... ཊོ་ཀེན་_ཝ་ལེཊི་=ཀེ་༠....
    WS མཐུད་སྦྲེལ།
    གློག་རིམ་: Sign RequestTx
    །ལན་ལ་སྒུག་པའོ།

༢༽ ཊར་མི་ནཱལ་ བི་ — ཝ་ལེཊི་ (ཊོ་ཀེན་_ཝ་ལེཊི་དང་གཅིག་ཁར་ མཐུད་དེ་ ཨེསི་ཊི་གཱར་ རི་སཱལཊི་ཨོག་དང་གཅིག་ཁར་ ལན་གསལ་འབདཝ་ཨིན།):

    སྐྱེལ་འདྲེན་ -p iroha_torie_shared --དཔེར་ན་ མཐུད་བྱེད་_གྱང་----node I18NU0000008X --sid Z4... --token K0..

   དཔེ་ཚད་ཀྱི་ཐོན་འབྲས་:

    དངུལ་ཁུག་: མཐུད་ཡོད་པའི་ WS
    warlet: sign Requestx len=3 སེག་ ༡ ལུ།
    warlet: sign ResultOk

༣༽ App terminal གིས་ གྲུབ་འབྲས་འདི་དཔར་བསྐྲུན་འབདཝ་ཨིན།

    app: sign ResultOk algo=ed25519 སིག་= deadbeeef

  `connect_norito_decode_envelope_sign_result_alg` གྲོགས་རམ་པ་ (དང་ དེ་ལས་
  སྣོད་འཛིན་འདི་ནང་ སོར་བསྒྱུར་/ཀོཊི་ལིན་གྱི་ wrappers) དེ་ ག་དེམ་ཅིག་སྦེ་ ཨེལ་གོ་རི་དམ་ཡིག་རྒྱུན་འདི་ ལོག་ཐོབ་ནིའི་དོན་ལུ་ཨིན།
  གླ་ཆ་སྤྲོད་ནི།

དྲན་ཐོ་ཚུ།
- དཔེ་འབད་བ་ཅིན་ I18NI000000016X ལས་ བརྡ་སྟོནམ་ཨིན། ཐོན་སྐྱེད་ནང་ལག་ལེན་མ་འཐབ།
- SDK AAD AAD བཅིངས་དང་ seq‐as sonce བསྟར་སྤྱོད་འབདཝ་ཨིན། ཚར་ཚད་ཚད་འཛིན་གཞི་ཁྲམ་ཚུ་ གསང་བཟོ་འབད་དགོ།
- སུའིཕཊི་མཁོ་སྤྲོད་འབད་མི་ཚུ་གི་དོན་ལུ་ `docs/connect_swift_integration.md` ལུ་བལྟ་ཞིནམ་ལས་ I18NI0000018X གིས་ `make swift-ci` དང་ཅིག་ཁར་ བདེན་དཔྱད་འབད་ཡོདཔ་ལས་ ཌེཤ་བོརཌ་བརྡ་འཕྲིན་ཚུ་ རཱསིཊ་དཔེ་དང་ བཱའིལ་ཌི་ཀའིཊ་མེ་ཊ་ཌེ་ཊ་ (`ci/xcframework-smoke:<lane>:device_tag`) དང་ཅིག་ཁར་ ཕྲང་སྒྲིག་འབད་དེ་ཡོདཔ་ཨིན།
- པའི་ཐོན་སི་ཨེལ་ཨའི་ གྲོགས་རམ་ལག་ལེན།

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

  སི་ཨེལ་ཨའི་གིས་ ཡིག་དཔར་རྐྱབས་ཡོད་པའི་བརྡ་དོན་འདི་དཔར་བསྐྲུན་འབདཝ་ཨིནམ་དང་ མཐུད་ལམ་གནས་རིམ་གྱི་པར་བརྐོ་སྟེ་ I18NT0000000000X-encoded I18NI000000021X གཞི་ཁྲམ་འདི་བཏོན་གཏངམ་ཨིན། I18NI000000002X འདི་ Torii ལུ་ ལོག་བཙུགས་ནིའི་དོན་ལུ་ I18NI000000023X ལག་ལེན་འཐབ། bytes, I18NI0000000024X དང་ I18NI000000024X འདི་ གཞི་རྟེན་༦༦༤-མཐུན་སྒྲིག་ཅན་གྱི་ JSON blob དང་ I18NI000025X ཚུ་ ཡིག་དཔར་རྐྱབ་དགོཔ་ད་ ཡིག་དཔར་རྐྱབས་དགོ། རང་འགོད། དེ་མ་ཚད་ ཁྱོད་ཀྱིས་ I18NI000000026X དང་ `name`, I18NI000000028X, དེ་ལས་ `icon_hash` ཚུ་བརྒྱུད་དེ་ JSON ཡིག་སྣོད་ཅིག་ལས་ གློག་རིམ་གྱི་མེ་ཊ་ཌེ་ཊ་མངོན་གསལ་འབད་ཚུགས། `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json` དང་བཅས་ ཊེམ་པེལེཊི་གསརཔ་ཅིག་ བཏོན་གཏང་། ཊེ་ལི་མི་ཊི་རྐྱངམ་ཅིག་གཡོག་བཀོལ་ནིའི་དོན་ལུ་ ཁྱོད་ཀྱིས་ ལཱ་ཡུན་གསར་བསྐྲུན་གྱི་དོན་ལུ་ I18NI000000032X དང་གཅིག་ཁར་ ཡོངས་རྫོགས་སྦེ་ གོམ་འགྱོ་ཚུགས།