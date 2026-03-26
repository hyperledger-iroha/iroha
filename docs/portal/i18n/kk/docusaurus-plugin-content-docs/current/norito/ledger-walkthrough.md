---
slug: /norito/ledger-walkthrough
lang: kk
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Бұл шолу [Norito жылдам бастау](./quickstart.md) көрсету арқылы толықтырады.
`iroha` CLI көмегімен бухгалтерлік кітап күйін мутациялау және тексеру әдісі. Сіз тіркелесіз а
жаңа актив анықтамасы, кейбір бірліктерді әдепкі оператор шотына енгізу, аудару
теңгерімнің бір бөлігін басқа шотқа аударып, нәтижесінде алынған транзакцияларды тексеріңіз
және холдингтер. Әрбір қадам Rust/Python/JavaScript ішінде қамтылған ағындарды көрсетеді
CLI және SDK әрекеті арасындағы тепе-теңдікті растау үшін SDK жылдам іске қосылады.

## Алғышарттар

- Бір деңгейлі желіні жүктеу үшін [жылдам іске қосу](./quickstart.md) орындаңыз.
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- `iroha` (CLI) құрастырылғанына немесе жүктелгеніне және келесіге қол жеткізе алатыныңызға көз жеткізіңіз.
  `defaults/client.toml` арқылы тең.
- Қосымша көмекшілер: `jq` (JSON жауаптарын пішімдеу) және POSIX қабығы
  төменде пайдаланылатын ортаның айнымалы үзінділері.

Нұсқаулық бойынша `$ADMIN_ACCOUNT` және `$RECEIVER_ACCOUNT` ауыстырыңыз.
пайдалануды жоспарлап отырған тіркелгі идентификаторлары. Әдепкі жинақта екі тіркелгі бар
демонстрациялық кілттерден алынған:

```sh
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
```

Алғашқы бірнеше тіркелгілерді тізімдеу арқылы мәндерді растаңыз:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Генезис күйін тексеріңіз

CLI мақсатты кітапты зерттеуден бастаңыз:

```sh
# Domains registered in genesis
iroha --config defaults/client.toml domain list all --table

# Accounts inside wonderland (replace --limit with a higher number if needed)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions that already exist
iroha --config defaults/client.toml asset definition list all --table
```

Бұл пәрмендер Norito қолдайтын жауаптарға негізделген, сондықтан сүзу және беттеу
детерминирленген және SDK алатынына сәйкес келеді.

## 2. Актив анықтамасын тіркеңіз

`wonderland` ішінде `coffee` деп аталатын жаңа, шексіз сатылатын активті жасаңыз.
домен:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

CLI жіберілген транзакция хэшін басып шығарады (мысалы,
`0x5f…`). Күйді кейінірек сұрау үшін оны сақтаңыз.

## 3. Бірліктерді оператор шотына енгізу

Активтер саны `(asset definition, account)` жұбында тұрады. Жалбыз 250
`7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` бірліктері `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Тағы да CLI шығысынан транзакция хэшін (`$MINT_HASH`) түсіріңіз. Кімге
теңгерімді екі рет тексеріңіз, іске қосыңыз:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

немесе тек жаңа активті мақсатты ету үшін:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Баланстың бір бөлігін басқа шотқа аудару

Оператор тіркелгісінен `$RECEIVER_ACCOUNT` нөміріне 50 бірлік жылжытыңыз:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Транзакция хэшін `$TRANSFER_HASH` ретінде сақтаңыз. Екеуінде де холдингтерді сұраңыз
жаңа қалдықтарды тексеру үшін шоттар:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Бухгалтерлік кітаптың дәлелдемелерін тексеріңіз

Екі транзакцияның орындалғанын растау үшін сақталған хэштерді пайдаланыңыз:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Сондай-ақ, қай блоктың тасымалданғанын көру үшін соңғы блоктарды ағынмен жіберуге болады:

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Жоғарыдағы әрбір пәрмен SDK сияқты бірдей Norito пайдалы жүктемелерін пайдаланады. Егер сіз қайталасаңыз
бұл код арқылы ағын (төмендегі SDK жылдам бастауларын қараңыз), хэштер мен баланстар болады
бірдей желіні және әдепкі параметрлерді мақсат еткен кезде қатарға тұрыңыз.

## SDK паритеттік сілтемелері

- [Rust SDK жылдам іске қосу](../sdks/rust) — тіркеу нұсқауларын көрсетеді,
  транзакцияларды жіберу және Rust сайтынан сұрау күйі.
- [Python SDK жылдам іске қосу](../sdks/python) — бірдей регистрді/жалғанды көрсетеді
  Norito қолдайтын JSON көмекшілерімен операциялар.
- [JavaScript SDK жылдам іске қосу](../sdks/javascript) — Torii сұрауларын қамтиды,
  басқару көмекшілері және терілген сұрау орауыштары.

Алдымен CLI шолуын іске қосыңыз, содан кейін сценарийді қалаған SDK арқылы қайталаңыз
транзакция хэштері, баланстар және сұрау бойынша екі беттің де келісетініне көз жеткізу үшін
шығыстар.