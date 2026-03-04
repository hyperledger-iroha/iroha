---
lang: hy
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2c61035c0e4b0fd478f08beeef34d7ae41415f55b09dc93dfda9490efe94fb91
source_last_modified: "2026-01-22T16:26:46.505734+00:00"
translation_last_reviewed: 2026-02-07
title: Ledger Walkthrough
description: Reproduce a deterministic register → mint → transfer flow with the `iroha` CLI and verify the resulting ledger state.
slug: /norito/ledger-walkthrough
translator: machine-google-reviewed
---

Այս ուղեցույցը լրացնում է [Norito արագ մեկնարկը](./quickstart.md)՝ ցույց տալով.
ինչպես մուտացիայի ենթարկել և ստուգել մատյանային վիճակը `iroha` CLI-ով: Դուք գրանցվելու եք ա
ակտիվների նոր սահմանում, որոշ միավորներ մուտքագրել լռելյայն օպերատորի հաշվին, փոխանցում
մնացորդի մի մասը մեկ այլ հաշվի վրա և ստուգեք արդյունքում ստացված գործարքները
և հոլդինգներ: Յուրաքանչյուր քայլ արտացոլում է Rust/Python/JavaScript-ով ծածկված հոսքերը
SDK-ն արագ մեկնարկում է, որպեսզի կարողանաք հաստատել հավասարությունը CLI-ի և SDK-ի վարքագծի միջև:

## Նախադրյալներ

- Հետևեք [արագ մեկնարկին] (./quickstart.md)՝ մեկ հասակակից ցանցը բեռնելու համար
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Համոզվեք, որ `iroha` (CLI) կառուցված կամ ներբեռնված է, և որ դուք կարող եք հասնել
  հասակակից՝ օգտագործելով `defaults/client.toml`:
- Լրացուցիչ օգնականներ՝ `jq` (ձևաչափող JSON պատասխաններ) և POSIX կեղև՝
  շրջակա միջավայրի փոփոխական հատվածներ, որոնք օգտագործվում են ստորև:

Ամբողջ ուղեցույցում `$ADMIN_ACCOUNT` և `$RECEIVER_ACCOUNT` փոխարինեք
հաշվի ID-ները, որոնք նախատեսում եք օգտագործել: Կանխադրված փաթեթն արդեն ներառում է երկու հաշիվ
ստացված ցուցադրական ստեղներից.

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

Հաստատեք արժեքները՝ թվարկելով առաջին մի քանի հաշիվները.

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Ստուգեք ծագման վիճակը

Սկսեք ուսումնասիրելով այն մատյանը, որը CLI-ին ուղղված է.

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

Այս հրամանները հիմնված են Norito պատասխանների վրա, ուստի զտումը և էջավորումը կատարվում են
որոշիչ և համընկնում է այն, ինչ ստանում են SDK-ները:

## 2. Գրանցեք ակտիվի սահմանում

`wonderland`-ի ներսում ստեղծեք `coffee` նոր, անվերջ արդյունահանվող ակտիվ:
տիրույթ:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

CLI-ն տպում է ներկայացված գործարքի հեշը (օրինակ՝
`0x5f…`): Պահպանեք այն, որպեսզի ավելի ուշ կարողանաք հարցնել կարգավիճակը:

## 3. Մուտքագրեք միավորներ օպերատորի հաշվին

Ակտիվների քանակները ապրում են `(asset definition, account)` զույգի ներքո: Անանուխ 250
`coffee#wonderland`-ի միավորները `$ADMIN_ACCOUNT`-ի մեջ.

```sh
iroha --config defaults/client.toml asset mint \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --quantity 250
```

Կրկին վերցրեք գործարքի հեշը (`$MINT_HASH`) CLI ելքից: Դեպի
կրկնակի ստուգեք մնացորդը, գործարկեք.

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

կամ ուղղակի նոր ակտիվը թիրախավորելու համար՝

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" \
  --limit 1 | jq .
```

## 4. Մնացորդի մի մասը փոխանցեք այլ հաշվի

Տեղափոխեք 50 միավոր օպերատորի հաշվից դեպի `$RECEIVER_ACCOUNT`.

```sh
iroha --config defaults/client.toml asset transfer \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Պահպանեք գործարքի հեշը որպես `$TRANSFER_HASH`: Հարցրեք երկուսի ունեցվածքը
հաշիվներ՝ նոր մնացորդները ստուգելու համար.

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${RECEIVER_ACCOUNT}\"}" --limit 1 | jq .
```

## 5. Ստուգեք մատյանային ապացույցները

Օգտագործեք պահպանված հեշերը՝ հաստատելու, որ երկու գործարքներն էլ կատարվել են.

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Կարող եք նաև հեռարձակել վերջին բլոկները՝ տեսնելու, թե որ բլոկն է ներառել փոխանցումը.

```sh
# Stream from the latest block and stop after ~5 seconds
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Վերևում գտնվող յուրաքանչյուր հրաման օգտագործում է նույն Norito ծանրաբեռնվածությունը, ինչ SDK-ները: Եթե դուք կրկնում եք
այս հոսքը կոդի միջոցով (տես ստորև բերված SDK-ի արագ մեկնարկները), հեշերն ու մնացորդները կկատարվեն
հերթ կանգնել այնքան ժամանակ, քանի դեռ թիրախավորում եք նույն ցանցը և լռելյայն:

## SDK հավասարության հղումներ

- [Rust SDK quickstart] (../sdks/rust) — ցույց է տալիս գրանցման հրահանգները,
  գործարքների ներկայացում և հարցման կարգավիճակ Rust-ից:
- [Python SDK արագ մեկնարկ] (../sdks/python) - ցույց է տալիս նույն ռեգիստրը/անանուխը
  գործողություններ Norito-ով ապահովված JSON օգնականներով:
- [JavaScript SDK արագ մեկնարկ] (../sdks/javascript) — ծածկում է Torii հարցումները,
  կառավարման օգնականներ և մուտքագրված հարցումների փաթաթաններ:

Սկզբում գործարկեք CLI քայլը, ապա կրկնեք սցենարը ձեր նախընտրած SDK-ով
համոզվելու համար, որ երկու մակերեսները համաձայն են գործարքի հեշերի, մնացորդների և հարցումների վերաբերյալ
ելքեր։