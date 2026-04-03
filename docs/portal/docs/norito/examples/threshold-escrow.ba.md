<!-- Auto-generated stub for Bashkir (ba) translation. Replace this content with the full translation. -->

---
lang: ba
direction: ltr
source: docs/portal/docs/norito/examples/threshold-escrow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 54b6d543cff8df6e8fd50632cfed6265770edc33855f06912be603457c5b517e
source_last_modified: "2026-04-02T18:31:54.074495+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/threshold-escrow
title: Порог эскроу
description: Бер түләүсе эскроу, тип ҡабул итә, тултырыу өсөн теүәл маҡсатлы сумма, һуңынан сығара йәки аҡса ҡайтара.
source: crates/kotodama_lang/src/samples/threshold_escrow.ko
---

Бер түләүсе эскроу, тип ҡабул итә, тултырыу өсөн теүәл маҡсатлы сумма, һуңынан сығара йәки аҡса ҡайтара.

## Баш китабы үткәреү

- Алдан эскроу-иҫәп һәм һанлы активтарҙы билдәләү булдырыу, һуңынан түләүсе иҫәбен финанслау, тип тапшырасаҡ килешеп шылтыратыуҙар. Өлгө автоматик рәүештә был түләүсе менән `open_escrow` ваҡытында `authority()` менән бәйләй.
- Шылтыратыу `open_escrow(recipient, escrow_account, asset_definition, target_amount)` бер тапҡыр яҙып алыу өсөн түләүсе, алыусы, эскроу иҫәбенә, активтарҙы билдәләү, теүәл маҡсат, һәм асыҡ/бушатылған/ҡайтарылған флагтар ныҡлы килешеп дәүләт.
- Шул уҡ түләүсенән `funded_amount_value == target_amount_value` тиклем `deposit(amount)` шылтыратыу; депозиттар ыңғай ҡалырға тейеш һәм теләһә ниндәй тултырыу, тип артыҡ финанслау эскроу кире ҡағыла.
- `release_if_ready()` шылтыратығыҙ, маҡсатҡа ирешкәс, эскроу аҡсаһын алыусыға күсерергә, йәки эскроу асыҡ булғанда `refund()` шылтыратығыҙ, финансланған сумманы түләүсегә кире ҡайтарығыҙ.
- 18НИ00000013Х / 18НИ00000014Х менән баланстарҙы тикшерергә һәм 18НИ00000015Х менән килешелгән хәлде тикшерергә.

## SDK-ға бәйле ҡулланмалар

- [Раст SDK тиҙ башлау] (/sdks/rust)
- [Питон SDK тиҙ башлау] (/sdks/python)
- [Яваскрипт SDK тиҙ башлау] (/sdks/javascript)

[Скачать источник Kotodama] (/norito-snippets/threshold-escrow.ko)

18НФ00000001Х