---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug : /norito/examples/transfer-asset
titre : اکاؤنٹس کے درمیان اثاثہ منتقل کریں
description : Voici les étapes à suivre et les démarrages rapides du SDK ainsi que les procédures pas à pas et les procédures pas à pas.
source : exemples/transfert/transfer.ko
---

Découvrez les étapes à suivre et les démarrages rapides du SDK ainsi que les procédures pas à pas et les étapes à suivre pour résoudre ce problème.

## لیجر واک تھرو

- Alice est en train de démarrer le démarrage rapide du SDK (avec `register and mint` pour le démarrage rapide du SDK)
- `do_transfer` Il y a 10 ans d'amour avec Alice et Bob et `AssetTransferRole` اجازت پوری ہو۔
- بیلنس (`FindAccountAssets`, `iroha_cli ledger assets list`) چیک کریں یا پائپ لائن ایونٹس سبسکرائب کریں تاکہ ٹرانسفر کے نتیجے کا مشاہدہ ہو۔

## Utiliser le SDK

- [Démarrage rapide du SDK Rust](/sdks/rust)
- [Démarrage rapide du SDK Python](/sdks/python)
- [Démarrage rapide du SDK JavaScript](/sdks/javascript)

[Kotodama سورس ڈاؤن لوڈ کریں](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("<katakana-i105-account-id>"),
      account!("<katakana-i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```