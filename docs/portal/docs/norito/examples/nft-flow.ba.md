<!-- Auto-generated stub for Bashkir (ba) translation. Replace this content with the full translation. -->

---
lang: ba
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09ff44a2df8cbcb9f57017239070a16f5287cbfc59a8289ce54933e84f90a5e8
source_last_modified: "2026-03-26T13:01:47.374572+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/nft-flow
title: Мята, күсерергә, һәм яндырыу NFT
description: NFT тормош циклы аша йөрөп, аҙағынан аҙағына тиклем: хужаһына ҡойоу, тапшырыу, метамағлүмәттәрҙе билдәләү һәм яндырыу.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT тормош циклы аша йөрөп, аҙағынан аҙағына тиклем: хужаһына ҡойоу, тапшырыу, метамағлүмәттәрҙе билдәләү һәм яндырыу.

## Баш китабы үткәреү

- НФТ билдәләмәһе (мәҫәлән, `n0#wonderland`) фрагментта ҡулланылған хужа/алыусы иҫәптәре менән бер рәттән барлығын тәьмин итеү (Элис өсөн `<i105-account-id>`, Боб өсөн `<i105-account-id>`).
- `nft_issue_and_transfer` инеү нөктәһен саҡырып, NFT ҡойоу, уны Алиса Бобҡа күсерергә һәм сығарыуҙы һүрәтләгән метамағлүмәттәр флагын беркетергә.
- Тикшерергә NFT баш китабы дәүләте менән `iroha_cli ledger nft list --account <id>` йәки SDK эквиваленттары раҫлау өсөн тапшырыу, һуңынан раҫлау актив юйылған бер тапҡыр яндырыу инструкцияһы эшләй.

## SDK-ға бәйле ҡулланмалар

- [Раст SDK тиҙ башлау] (/sdks/rust)
- [Питон SDK тиҙ башлау] (/sdks/python)
- [Яваскрипт SDK тиҙ башлау] (/sdks/javascript)

[Скачать источник Kotodama] (/norito-snippets/nft-flow.ko)

18НФ00000001Х