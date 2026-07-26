---
lang: hy
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Iroha Միացման օրինակներ (Rust հավելված/դրամապանակ)

Գործարկեք Rust-ի երկու օրինակները ծայրից ծայր Torii հանգույցով:

Նախադրյալներ
- Torii հանգույց՝ `connect`-ով միացված `http://127.0.0.1:8080`-ում:
- Rust գործիքաշղթա (կայուն):
- Python 3.9+՝ տեղադրված `iroha-python` փաթեթով (ներքևում գտնվող CLI օգնականի համար):

Օրինակներ
- Հավելվածի օրինակ՝ `crates/iroha_torii_shared/examples/connect_app.rs`
- Դրամապանակի օրինակ՝ `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Python CLI օգնական՝ `python -m iroha_python.examples.connect_flow`

Գործարկման կարգը
1) Տերմինալ A — Հավելված (տպում է sid + նշաններ, միացնում է WS-ը, ուղարկում SignRequestTx):

    բեռի գործարկում -p iroha_torii_shared -- օրինակ connect_app -- -- հանգույց http://127.0.0.1:8080 -- դերային հավելված

   Նմուշի ելք.

    sid=Z4... token_app=KJ... token_wallet=K0...
    WS միացված է
    հավելված՝ ուղարկված SignRequestTx
    (սպասում եմ պատասխանի)

2) Տերմինալ B — Դրամապանակ (միացեք token_wallet-ի հետ, պատասխաններ SignResultOk-ով):

    cargo run -p iroha_torii_shared --օրինակ connect_wallet -- -- հանգույց http://127.0.0.1:8080 --sid Z4... --token K0...

   Նմուշի ելք.

    դրամապանակ՝ միացված WS
    դրամապանակ՝ SignRequestTx len=3 հաջորդ 1-ում
    դրամապանակ՝ ուղարկված SignResultOk

3) Հավելվածի տերմինալը տպում է արդյունքը.

    հավելված՝ ստացել է SignResultOk algo=ed25519 sig=deadbeef

  Օգտագործեք նոր `connect_norito_decode_envelope_sign_result_alg` օգնականը (և
  Swift/Kotlin wrappers այս թղթապանակում) ալգորիթմի տողը առբերելու համար, երբ
  օգտակար բեռի վերծանում.

Նշումներ
- Օրինակները բխում են ցուցադրական էֆեմերալներից `sid`-ից, որպեսզի հավելվածը/դրամապանակը փոխգործակցեն ինքնաբերաբար: Մի օգտագործեք արտադրության մեջ:
- SDK-ն պարտադրում է AEAD AAD-ի պարտադիր և հետևողականությունը. հետհաստատման հսկողության շրջանակները պետք է գաղտնագրված լինեն:
- Swift-ի հաճախորդների համար տե՛ս `docs/connect_swift_integration.md` / `docs/connect_swift_ios.md` և վավերացրե՛ք `make swift-ci`-ով, որպեսզի վահանակի հեռաչափությունը համահունչ մնա Rust օրինակների հետ, իսկ Buildkite մետատվյալները (I18NI000000020X) մնան անփոփոխ:
- Python CLI օգնականի օգտագործումը.

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

  CLI-ն տպում է մուտքագրված նստաշրջանի տվյալները, տեղադրում է Connect կարգավիճակի լուսանկարը և թողարկում Norito կոդավորված `ConnectControlOpen` շրջանակը: Անցեք `--send-open`՝ ծանրաբեռնվածությունը հետ ուղարկելու համար Torii, օգտագործեք `--frame-output-format binary`՝ չմշակված բայթեր գրելու համար, `--frame-json-output`՝ base64-ի համար հարմար JSON բլբի համար, և I18NI000000025X-ի համար օգտագործեք ավտոմատացում: Կարող եք նաև բեռնել հավելվածի մետատվյալները JSON ֆայլից `--app-metadata-file metadata.json`-ի միջոցով, որը պարունակում է `name`, `url` և `icon_hash` դաշտերը (տես `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`): Ստեղծեք թարմ ձևանմուշ `python -m iroha_python.examples.connect_flow --write-app-metadata-template app_metadata.json`-ով: Միայն հեռաչափության գործարկումների համար դուք կարող եք ամբողջությամբ բաց թողնել նիստի ստեղծումը `--status-only`-ով և ցանկության դեպքում հեռացնել JSON-ը `--status-json-output status.json`-ի միջոցով: