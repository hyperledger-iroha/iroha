<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e686495c642a08740504c4bb5f88e623c89a896787388b61e4451f550f87af6
source_last_modified: "2026-03-26T13:01:47.376183+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/register-and-mint
title: ጎራ እና ሚንት ንብረቶችን ይመዝገቡ
description: የተፈቀደ ጎራ መፍጠርን፣ የንብረት ምዝገባን እና የሚወስን አሰራርን ያሳያል።
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

የተፈቀደ ጎራ መፍጠርን፣ የንብረት ምዝገባን እና የሚወስን አሰራርን ያሳያል።

## የመመዝገቢያ መመሪያ

- የመድረሻ መለያው (ለምሳሌ `<i105-account-id>` ለ Alice) መኖሩን ያረጋግጡ፣ በእያንዳንዱ የኤስዲኬ ፈጣን ጅምር ውስጥ የማዋቀር ደረጃን ያንፀባርቃል።
- በአንድ ግብይት የ ROSE ንብረት ትርጉም እና mint 250 አሃዶችን ወደ አሊስ ለመፍጠር የ `register_and_mint` መግቢያ ነጥብን ጥራ።
- ሚዛኑን በ`client.request(FindAccountAssets)` ወይም `iroha_cli ledger asset list --account <i105-account-id>` በኩል ያረጋግጡ mint መሳካቱን ለማረጋገጥ።

## ተዛማጅ የኤስዲኬ መመሪያዎች

- [ዝገት ኤስዲኬ ፈጣን ጅምር](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[የKotodama ምንጭ ያውርዱ](/norito-snippets/register-and-mint.ko)

```text
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
  #[access(read="*", write="*")]
  kotoage fn register_and_mint() permission(AssetManager) {
    // name, symbol, quantity (precision or supply depending on host), mintable flag
    let name = "rose";
    let symbol = "ROSE";
    let qty = 1000;      // interpretation depends on data model (example only)
    let mintable = 1;    // 1 = mintable, 0 = fixed
    register_asset(name, symbol, qty, mintable);

    // Mint 250 ROSE to Alice
    let to = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```