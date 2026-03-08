---
slug: /norito/ledger-walkthrough
lang: ba
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Был проходка тулыландыра [I18NT00000000000X fawstart] (./quickstart.md) күрһәтеү аша .
нисек мутация һәм тикшерергә леджер дәүләт менән I18NI000000021X CLI. Һеҙ теркәләсәк
яңы активтарҙы билдәләү, ҡайһы бер агрегаттарҙы оператор иҫәбенә индереү, күсерергә
баланстың бер өлөшө икенсе иҫәпкә, һәм һөҙөмтәлә операцияларҙы раҫлау
һәм холдингтар. Һәр аҙым көҙгө ағымдар ҡапланған руст/Питон/JavaScript .
SDK тиҙ башлай, шулай итеп, һеҙ раҫлай ала паритет араһында CLI һәм SDK тәртибе.

## Алдан шарттар

- Һуғыш [тиҙstart](I18NU000000017X) бер тиңдәш селтәр аша загрузка .
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- `iroha` (CLI) төҙөлгән йәки скачать һәм һеҙ етергә мөмкин
  тиңдәштәрен ҡулланып I18NI000000024X.
- Һорау алыу ярҙамсылары: I18NI000000025X (форматлаштырыу JSON яуаптары) һәм POSIX снаряды өсөн
  тирә-яҡ мөхитте үҙгәртеүсе өҙөктәр аҫтында ҡулланыла.

Бөтә экскурсовод, алмаштырыу I18NI0000000026X һәм I18NI000000027X менән .
иҫәп идентификаторҙары һеҙ ҡулланырға уйлайһығыҙ. Ғәҙәттәгесә пакет инде ике иҫәп яҙмаһы инә
демо-асҡыстарҙан алынған:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

Ҡиммәттәрҙе раҫлау өсөн тәүге бер нисә иҫәп яҙмаһы:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Генезис хәлен тикшерергә

Башланғыс тикшергәндән һуң, баш китабы CLI маҡсатлы:

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

Был командалар I18NT0000000001X-ярҙам яуаптарына таяна, шуға күрә фильтрлау һәм сәфәр .
детерминистик һәм тап килә, нимә ала SDKs.

## 2. Актив билдәләмәһен теркәү

Яңы, сикһеҙ ҡырҡыу актив булдырыу тип аталған I18NI000000028X эсендә I18NI000000029X .
домен:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI тапшырылған транзакция хеш баҫтыра (мәҫәлән,
I18NI000000300Х). Һаҡлағыҙ, шулай итеп, һеҙ һуң статус эҙләү мөмкин.

## 3. Оператор иҫәбенә индереү агрегаттары

Активтар күләме I18NI000000031X пары аҫтында йәшәй. Минт 250
I18NI0000000032X берәмектәре I18NI0000000333 X:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
``` X

Тағы ла, транзакция хеш (`$MINT_HASH`) CLI сығышынан тотоу. 1990 й.
икеләтә тикшерергә баланс, йүгерергә:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

йәки, маҡсатҡа ғына яңы актив:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Баланстың бер өлөшөн икенсе иҫәпкә күсерергә

Оператор иҫәбенән 50 берәмекте I18NI000000035X тиклем күсерергә:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Транзакция хешын `$TRANSFER_HASH` тип һаҡларға. Икеһендә лә холдингтарҙы һорағыҙ
яңы баланстарҙы раҫлау өсөн иҫәптәр:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Баш китабын раҫлау дәлилдәре

Һаҡланған хештарҙы ҡулланыу өсөн раҫлау өсөн, ике операция ҡылған:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Һеҙ шулай уҡ һуңғы блоктар ағымын күрергә мөмкин, ниндәй блок күсерергә инә:

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Өҫтәге һәр команда шул уҡ I18NT000000002X файҙалы йөктәрҙе ҡуллана, SDKs кеүек. Әгәр һеҙ ҡабатлайһығыҙ
был ағым аша код (ҡарағыҙ SDK upstarts түбән), хештар һәм баланстар буласаҡ
рәткә тиклем, әгәр һеҙ маҡсатлы бер үк селтәр һәм ғәҙәттәгесә.

## SDK паритет һылтанмалары

- [SDK тиҙ старт](I18NU000000018X) — теркәү күрһәтмәләрен күрһәтә,
  операциялар тапшырыу, һәм һорау алыу статусынан Rust.
- [Python SDK quickstart](I18NU0000000019X) — шул уҡ регистр/минт күрһәтә
  операциялары менән I18NT000000003X-арҡа JSON ярҙамсылары.
- [JavaScript SDK тиҙ старт] (../sdks/javascript) — I18NT000000004X запростарын үҙ эсенә ала,
  идара итеү ярҙамсылары, һәм тип эҙләү wrappers.

CLI проходка йүгерә тәүҙә, һуңынан ҡабатлау сценарий менән һеҙҙең өҫтөнлөк SDK .
ике ер өҫтө лә транзакция хештары, баланстары һәм эҙләү тураһында килешергә тырышыу өсөн
сығыштар.