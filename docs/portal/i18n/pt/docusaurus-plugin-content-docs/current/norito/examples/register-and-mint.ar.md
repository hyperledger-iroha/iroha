---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/register-and-mint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/register-and-mint
título: تسجيل نطاق وسك الأصول
description: يوضح إنشاء النطاقات المصرح بها وتسجيل الأصول والسك الحتمي.
fonte: crates/ivm/docs/examples/13_register_and_mint.ko
---

Você pode fazer isso sem precisar de mais nada.

## جولة دفتر الأستاذ

- Você pode usar o software (como `<i105-account-id>`) para instalar o SDK no SDK.
- Verifique o valor `register_and_mint` para usar o ROSE 250 e resolva o problema.
- Verifique se o `client.request(FindAccountAssets)` ou `iroha_cli ledger assets list --account <i105-account-id>` está funcionando.

## O SDK está disponível

- [Atualizado para Rust SDK](/sdks/rust)
- [Implementar para Python SDK](/sdks/python)
- [Escolha o JavaScript SDK](/sdks/javascript)

[Kotodama](/norito-snippets/register-and-mint.ko)

```text
// Register a new asset and mint some to the specified account.
seiyaku RegisterAndMint {
  kotoage fn register_and_mint() permission(AssetManager) {
    // name, symbol, quantity (precision or supply depending on host), mintable flag
    let name = "rose";
    let symbol = "ROSE";
    let qty = 1000;      // interpretation depends on data model (example only)
    let mintable = 1;    // 1 = mintable, 0 = fixed
    register_asset(name, symbol, qty, mintable);

    // Mint 250 ROSE to Alice
    let to = account!("<i105-account-id>");
    let asset = asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
    mint_asset(to, asset, 250);
  }
}
```