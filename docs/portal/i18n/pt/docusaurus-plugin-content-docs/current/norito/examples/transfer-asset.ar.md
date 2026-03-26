---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/transfer-asset
título: نقل أصل بين الحسابات
description: Você pode usar o SDK do SDK e instalá-lo no SDK.
fonte: exemplos/transfer/transfer.ko
---

Você pode usar o SDK do SDK e instalá-lo novamente.

## جولة دفتر الأستاذ

- Alice é a única pessoa que pode ser chamada de `register and mint` ou تدفقات البدء السريع. para SDK).
- Eu coloquei o `do_transfer` no 10º lugar com Alice e Bob com o `AssetTransferRole`.
- Use o código de barras (`FindAccountAssets`, `iroha_cli ledger assets list`) e instale-o no computador. Não.

## O SDK está disponível

- [Atualizado para Rust SDK](/sdks/rust)
- [Implementar para Python SDK](/sdks/python)
- [Escolha o JavaScript SDK](/sdks/javascript)

[Kotodama](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of 62Fk4FPcMuLvW5QjDGNF2a4jAmjM from alice to bob
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("soraカタカナ..."),
      account!("soraカタカナ..."),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```