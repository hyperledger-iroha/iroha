<!-- Auto-generated stub for Bashkir (ba) translation. Replace this content with the full translation. -->

---
lang: ba
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e686495c642a08740504c4bb5f88e623c89a896787388b61e4451f550f87af6
source_last_modified: "2026-03-26T13:01:47.376183+00:00"
translation_last_reviewed: 2026-04-08
translator: machine-google-reviewed
---

---
slug: /norito/examples/register-and-mint
title: Домен һәм активтарҙы теркәү
description: Рөхсәт ителгән домен булдырыу, активтарҙы теркәү һәм детерминистик ҡойоу күрһәтә.
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

Рөхсәт ителгән домен булдырыу, активтарҙы теркәү һәм детерминистик ҡойоу күрһәтә.

## Баш китабы үткәреү

- Тәьмин итеү өсөн тәғәйенләнеш иҫәбе (мәҫәлән, `<i105-account-id>` өсөн Алиса) бар, көҙгө фазаһын ҡуйыу һәр SDK тиҙ башлау.
- `register_and_mint` инеү нөктәһен саҡырып, ROSE активтарын билдәләүҙе булдырыу һәм бер транзакцияла Алисаға 250 берәмек һуғыу.
- 18НИ00000008Х йәки 18НИ00000009Х аша баланстарҙы тикшерергә, аҡса һуғыу урыны уңышлы булыуын раҫлау өсөн.

## SDK-ға бәйле ҡулланмалар

- [Раст SDK тиҙ башлау] (/sdks/rust)
- [Питон SDK тиҙ башлау] (/sdks/python)
- [Яваскрипт SDK тиҙ башлау] (/sdks/javascript)

[Скачать источник Kotodama] (/norito-snippets/register-and-mint.ko)

18НФ00000001Х

```kotodama
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
    kotoage fn register_and_mint() authorize("AssetManager") {
        // name, symbol, quantity (precision or supply depending on host), mintable flag
        let asset = AssetDefinitionId::parse("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
        let symbol = "ROSE";
        let qty = 1000;
        // interpretation depends on data model (example only)
        let mintable = 1;
        // 1 = mintable, 0 = fixed
        ledger::asset::register(
            asset_definition: asset,
            name: symbol,
            scale: qty,
            mintable: mintable,
        );
        // Mint 250 ROSE to Alice
        let to = AccountId::parse("sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV", );
        ledger::asset::mint(account: to, asset_definition: asset, amount: 250);
    }
}
```
