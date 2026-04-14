<!-- Auto-generated stub for Bashkir (ba) translation. Replace this content with the full translation. -->

---
lang: ba
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dcd8de175a7c5172158a03e1a25b254c90a11e62c173f95b8d9e4a387df6ba09
source_last_modified: "2026-03-26T13:01:47.372931+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/call-transfer-asset
title: 18NT00000000X -тан хост тапшырыуҙы саҡырырға
description: Күрһәтә, нисек Kotodama инеү нөктәһе хост `transfer_asset` инструкцияһын рәт эсендәге метамағлүмәттәрҙе раҫлау менән шылтырата ала.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Күрһәтә, нисек Kotodama инеү нөктәһе хост `transfer_asset` инструкцияһын рәт эсендәге метамағлүмәттәрҙе раҫлау менән шылтырата ала.

## Баш китабы үткәреү

- Финанс килешеп орган (мәҫәлән, `<i105-account-id>` өсөн килешеп иҫәбенә) актив менән ул тапшырасаҡ һәм органға `CanTransfer` роле йәки эквивалентлы рөхсәт бирә.
- Шылтыратыу `call_transfer_asset` инеү нөктәһе тапшырыу өсөн 5 берәмектәр килешеп иҫәбенә Боб (`<i105-account-id>`), көҙгө ысулын автоматлаштырыу сылбырлы хост шылтыратыуҙарҙы урап ала.
- Баланстарҙы тикшерергә аша `FindAccountAssets` йәки `iroha_cli ledger asset list --account <i105-account-id>` һәм ваҡиғаларҙы тикшерергә раҫлау өсөн метамағлүмәттәр һаҡсыһы тапшырыу контекста теркәлгән.

## SDK-ға бәйле ҡулланмалар

- [Ржавчина SDK тиҙ башлау] (/sdks/rust)
- [Питон SDK тиҙ башлау] (/sdks/python)
- [Яваскрипт SDK тиҙ башлау] (/sdks/javascript)

[Скачать источник Kotodama](/norito-snippets/call-transfer-asset.ko)

18НФ00000002Х