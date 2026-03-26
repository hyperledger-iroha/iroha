---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
título: Вызвать перенос с хоста из Kotodama
description: Показывает, как точка входа Kotodama pode ser usado para instruir este `transfer_asset` com um teste comprovado метаданных.
fonte: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Verifique se o Kotodama pode ser usado para obter instruções sobre o `transfer_asset` com um teste comprovado метаданных.

## Пошаговый обход реестра

- Abra o contrato de política (por exemplo `<katakana-i105-account-id>`) ativo, feche o orçamento e выдайте use o papel `CanTransfer` ou a configuração atual.
- Вызовите точку входа `call_transfer_asset`, чтобы перевести 5 единиц аккаунта контракта на `<katakana-i105-account-id>`, отражая то, Como o aplicativo on-line pode ser usado para você.
- Verifique os saldos de `FindAccountAssets` ou `iroha_cli ledger assets list --account <katakana-i105-account-id>` e instale a proteção, чтобы подтвердить, что guarda O metadanных записал контекст перевода.

## Como usar o SDK

- [Início rápido Rust SDK](/sdks/rust)
- [Início rápido do Python SDK](/sdks/python)
- [SDK JavaScript de início rápido](/sdks/javascript)

[Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("<katakana-i105-account-id>"),
      account!("<katakana-i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```