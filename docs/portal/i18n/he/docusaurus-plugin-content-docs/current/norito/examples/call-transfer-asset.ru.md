---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/call-transfer-asset.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/call-transfer-asset
כותרת: Вызвать перенос с хоста из Kotodama
תיאור: Показывает, как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` עם востройно метаданных.
מקור: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Показывает, как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` с встроеннок метаданных.

## Пошаговый обход реестра

- Пополните полномочия контракта (например `<katakana-i105-account-id>`) אקטיב, который он будет переводить, ивыдить `CanTransfer` или эквивалентное разрешение.
- צור קשר עם `call_transfer_asset`, מתקנים 5 מכשירים עם תקשורת בתקן `<katakana-i105-account-id>`, ончейн-автоматизация может оборачивать вызовы хоста.
- Проверьте балансы через `FindAccountAssets` או `iroha_cli ledger assets list --account <katakana-i105-account-id>` и просмотрите события, чтобы подтвердитан, чтобы подтвердитан, чтобы подтвердитан, чтобы контекст перевода.

## Связанные руководства SDK

- [Quickstart Rust SDK](/sdks/rust)
- [Quickstart Python SDK](/sdks/python)
- [Quickstart JavaScript SDK](/sdks/javascript)

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