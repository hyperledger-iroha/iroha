---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラッグ: /norito/examples/call-transfer-asset
タイトル: Вызвать перенос с хоста из Kotodama
説明: Показывает、как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` с встроенной проверкойメッセージ。
ソース: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Показывает, как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` с встроенной проверкойメッセージ。

## Полаговый обход рестра

- Пополните полномочия контракта (например `<katakana-i105-account-id>`) активом, который он будет переводить, и выдайте `CanTransfer` は、これを実行します。
- Вызовите точку входа `call_transfer_asset`, чтобы перевести 5 единиц с аккаунта контракта на `<katakana-i105-account-id>`, отражая то, Сончейн-автоматизация может оборачивать вызовы хоста.
- Проверьте балансы через `FindAccountAssets` または `iroha_cli ledger assets list --account <katakana-i105-account-id>` и просмотрите события、чтобы подтвердить、что ガードПонтекст перевода です。

## Связанные руководства SDK

- [クイックスタート Rust SDK](/sdks/rust)
- [クイックスタート Python SDK](/sdks/python)
- [クイックスタート JavaScript SDK](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/call-transfer-asset.ko)

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