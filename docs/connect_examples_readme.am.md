---
lang: am
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha አገናኝ ምሳሌዎች (ዝገት መተግበሪያ/Wallet)

ሁለቱን የዝገት ምሳሌዎች ከመጨረሻ-ወደ-መጨረሻ በI18NT0000002X መስቀለኛ መንገድ ያሂዱ።

ቅድመ-ሁኔታዎች
- Torii መስቀለኛ መንገድ ከ I18NI0000009X ጋር በ `http://127.0.0.1:8080` ነቅቷል።
- ዝገት የመሳሪያ ሰንሰለት (የተረጋጋ).
- Python 3.9+ በ `iroha-python` ጥቅል ከተጫነ (ከታች ላለው የ CLI አጋዥ)።

ምሳሌዎች
- የመተግበሪያ ምሳሌ: `crates/iroha_torii_shared/examples/connect_app.rs`
- የኪስ ቦርሳ ምሳሌ: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI አጋዥ: `python -m iroha_python.examples.connect_flow`

የማስጀመሪያ ቅደም ተከተል
1) ተርሚናል A — መተግበሪያ (ሲድ + ቶከኖች ያትማል፣ WS ያገናኛል፣ SignRequestTx ይልካል)

    የካርጎ ሩጫ -p iroha_torii_የተጋራ --ምሳሌ connect_app -- --node http://127.0.0.1:8080 --role መተግበሪያ

   የናሙና ውጤት፡

    sid=Z4... ማስመሰያ_መተግበሪያ=ኪጄ... ማስመሰያ_Wallet=K0...
    WS ተገናኝቷል።
    መተግበሪያ: SignRequestTx ተልኳል።
    (ምላሽ በመጠባበቅ ላይ)

2) ተርሚናል ለ - የኪስ ቦርሳ (ከቶከን_ዋሌት ጋር ይገናኙ፣ በSignResultOk ምላሾች)

    የካርጎ ሩጫ -p iroha_torii_shared --ምሳሌ connect_wallet -- --node http://127.0.0.1:8080 --sid Z4... --ቶከን K0...

   የናሙና ውጤት፡

    ቦርሳ: የተገናኘ WS
    ቦርሳ፡ SignRequestTx len=3 በሰከንድ 1
    ቦርሳ፡ SignResultOk ተልኳል።

3) የመተግበሪያ ተርሚናል ውጤቱን ያትማል፡-

    መተግበሪያ: SignResultOk algo=ed25519 sig=የሞተ ሥጋ አግኝቷል

  አዲሱን `connect_norito_decode_envelope_sign_result_alg` አጋዥ ይጠቀሙ (እና የ
  በዚህ አቃፊ ውስጥ የስዊፍት/Kotlin መጠቅለያዎች) መቼ የአልጎሪዝም ሕብረቁምፊን ለማውጣት
  የተከፈለውን ጭነት መፍታት.

ማስታወሻዎች
- ምሳሌዎች ከ`sid` የ demo ephemerals ያገኙታል ስለዚህ መተግበሪያ/Wallet በራስ-ሰር ይተባበራል። በምርት ውስጥ አይጠቀሙ.
- ኤስዲኬ የ AEAD AAD ትስስርን እና ተከታታይነትን ያስፈጽማል፤ የድህረ ማጽደቅ መቆጣጠሪያ ፍሬሞች መመስጠር አለባቸው።
- ለስዊፍት ደንበኞች፣ `docs/connect_swift_integration.md`/`docs/connect_swift_integration.md`/`docs/connect_swift_ios.md` ን ይመልከቱ እና በ`make swift-ci` አረጋግጥ
- Python CLI አጋዥ አጠቃቀም፡-

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

  CLI የተተየበው የክፍለ ጊዜ መረጃን ያትማል፣ የግንኙነት ሁኔታ ቅጽበተ-ፎቶውን ይጥላል እና Norito-encoded `ConnectControlOpen` ፍሬም ያወጣል። ክፍያውን ወደ Torii ለመለጠፍ `--send-open` ይለፉ፣ ጥሬ ባይት ለመጻፍ `--frame-output-format binary` ይጠቀሙ፣ `--frame-json-output` ለbase64-ተስማሚ JSON ብሎብ፣ እና ለአይ18NI000000025X ይጠቀሙ። እንዲሁም የመተግበሪያ ሜታዳታ ከJSON ፋይል በ`--app-metadata-file metadata.json`፣ `name`፣ `url` እና `icon_hash` መስኮችን (`python/iroha_python/src/iroha_python/examples/connect_app_metadata.json` ይመልከቱ) መጫን ይችላሉ። አዲስ አብነት በI18NI0000031X ይፍጠሩ። ለቴሌሜትሪ-ብቻ ሩጫዎች የክፍለ-ጊዜ መፍጠርን ሙሉ በሙሉ በI18NI0000032X መዝለል እና እንደ አማራጭ JSON በ`--status-json-output status.json` መጣል ይችላሉ።