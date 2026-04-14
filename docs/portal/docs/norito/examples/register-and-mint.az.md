<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
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
title: Domen və nanə aktivlərini qeydiyyatdan keçirin
description: İcazəli domen yaradılması, aktivlərin qeydiyyatı və deterministik zərb alətlərini nümayiş etdirir.
source: crates/ivm/docs/examples/13_register_and_mint.ko
---

İcazəli domen yaradılması, aktivlərin qeydiyyatı və deterministik zərb alətlərini nümayiş etdirir.

## Ledger prospekti

- Hər bir SDK sürətli başlanğıcda quraşdırma mərhələsini əks etdirərək təyinat hesabının (məsələn, Alice üçün `<i105-account-id>`) mövcud olduğundan əmin olun.
- ROSE aktiv tərifini yaratmaq üçün `register_and_mint` giriş nöqtəsini çağırın və bir əməliyyatda Aliceyə 250 vahid nanə edin.
- Nanənin uğur qazandığını təsdiqləmək üçün `client.request(FindAccountAssets)` və ya `iroha_cli ledger asset list --account <i105-account-id>` vasitəsilə balansları yoxlayın.

## Əlaqədar SDK təlimatları

- [Rust SDK sürətli başlanğıc](/sdks/rust)
- [Python SDK sürətli başlanğıc](/sdks/python)
- [JavaScript SDK sürətli başlanğıc](/sdks/javascript)

[Kotodama mənbəyini endirin](/norito-snippets/register-and-mint.ko)

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