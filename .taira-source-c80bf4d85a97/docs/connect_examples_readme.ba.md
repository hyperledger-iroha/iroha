---
lang: ba
direction: ltr
source: docs/connect_examples_readme.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a79487a5e7e8268f3a94580716a603580f17fd0d0223f10646ecda6aad2e2482
source_last_modified: "2025-12-29T18:16:35.063907+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## I18NT000000001X Тоташтырыу миҫалдары (Рат App/Sucet)

Йүгереп ике Rust миҫалдары менән тамамлана‐ тамамлана I18NT000000002X төйөн.

Тәүшарттар
- `connect` X төйөнө I18NI000000010X-та мөмкинлек биргән.
- Руст инструменттар сылбыр (стабиль).
- 3.9+ Python `iroha-python` пакеты менән ҡуйылған (аҫта CLI ярҙамсыһы өсөн).

Миҫалдар
- App миҫалы: `crates/iroha_torii_shared/examples/connect_app.rs`
- Баллет миҫалы: `crates/iroha_torii_shared/examples/connect_wallet.rs`
- Питон CLI ярҙамсыһы: `python -m iroha_python.examples.connect_flow`

Стартап тәртибе
1) Терминал А — App (баҫмалар sid + жетондар, WS тоташтыра, SignRequestTx ебәрә):

    йөк йүгерә -p iroha_tori_shared --миҫал тоташтырыу_app -- төйөн http://127.0.0.1:8080 --ролле ҡушымта

   Өлгө сығыш:

    sid=Z4... токен_ап=К.Дж.
    WS тоташтырылған
    ҡушымта: SignRequestTx ебәрелгән
    (яуап көтә)

2) Терминал В — пробег (токен_волет менән тоташтырыу, SignResultOk менән яуап бирә):

    йөк йүгерә -p iroha_tori_shared --миҫал тоташтырыу_монак -- төйөн http://127.0.0.1:8080 --сид Z4... --токен К0...

   Өлгө сығыш:

    янсыҡ: тоташтырылған WS
    1-се эҙҙә SignRequestTx len=3
    янсыҡ: SignResultOk ебәрелгән

3) Ҡушымта терминал һөҙөмтәһен баҫтырып сығара:

    Ҡушымта: SignResultOk algo=d25519 сиг=үлсәге алынған

  Яңы `connect_norito_decode_envelope_sign_result_alg` ярҙамсыһы ҡулланыу (һәм был
  Свифт/Котлин был папкала урауҙар) алгоритм телмәрен алыу өсөн ҡасан
  файҙалы йөктө расшифровкалау.

Иҫкәрмәләр
- Миҫалдар I18NI000000016X-тан демо-эфемералдар килеп сыға, шуға күрә ҡушымта/концепт автоматик рәүештә үҙ-ара эш итә. Етештереүҙә ҡулланмағыҙ.
- SDK AEAD AAD бәйләү һәм артабанғы түгел; пост‐раҫлау контроль кадрҙары шифрларға тейеш.
- Свифт клиенттары өсөн ҡарағыҙ: I18NI000000017X / I18NI000000018X һәм I18NI000000019X менән раҫлау шулай приборҙар таҡтаһы телеметрияһы ҡалалары менән тура килтерелгән Rust миҫалдары һәм Buildkite метамағлүмәттәре (I18NI0000000020X) бөтөн булып ҡала.
- Питон CLI ярҙамсы ҡулланыу:

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

  CLI тип аталған сессия мәғлүмәтен баҫтырып сығара, тоташтыра Connect статусы снимок, һәм I18NT000000000000X-кодланған `ConnectControlOpen` кадрын сығара. Pass I18NI0000000022X файҙалы йөктө I18NT000000000004X-ға кире ҡайтарыу өсөн, сеймал байттарын яҙырға I18NI000000000023, base64-файл JSON блоб өсөн, һәм `--status-json-output` XI 1990 йылда тип яҙылған снимок кәрәк. автоматлаштырыу. Һеҙ шулай уҡ ҡушымта метамағлүмәттәрен тейәп була JSON файлы аша I18NI000000026X составында I18NI000000027X, I18NI000000028X, һәм I18NI000000029X яландары (ҡара: I18NI00000000300). I18NI000000031X менән яңы шаблон генерациялау. Телеметрия өсөн-тик йүгерә, һеҙ үткәрергә мөмкин сессия булдырыу тулыһынса I18NI000000032X һәм теләк буйынса JSON аша I18NI000000033X.