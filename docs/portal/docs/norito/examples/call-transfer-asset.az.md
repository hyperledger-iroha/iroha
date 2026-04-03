<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dcd8de175a7c5172158a03e1a25b254c90a11e62c173f95b8d9e4a387df6ba09
source_last_modified: "2026-03-26T13:01:47.372931+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/call-transfer-asset
title: Kotodama-dən host transferini çağırın
description: Kotodama giriş nöqtəsinin daxili metadata yoxlaması ilə `transfer_asset` təlimatına necə zəng edə biləcəyini nümayiş etdirir.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Kotodama giriş nöqtəsinin daxili metadata yoxlaması ilə `transfer_asset` təlimatına necə zəng edə biləcəyini nümayiş etdirir.

## Ledger prospekti

- Müqavilə orqanını (məsələn, müqavilə hesabı üçün `<i105-account-id>`) köçürəcəyi aktivlə maliyyələşdirin və səlahiyyətli orqana `CanTransfer` rolu və ya ekvivalent icazə verəcək.
- Zəncirvari avtomatlaşdırmanın ev sahibi zənglərini əhatə edə bilməsini əks etdirərək, müqavilə hesabından 5 vahidi Boba (`<i105-account-id>`) köçürmək üçün `call_transfer_asset` giriş nöqtəsinə zəng edin.
- `FindAccountAssets` və ya `iroha_cli ledger asset list --account <i105-account-id>` vasitəsilə balansları yoxlayın və köçürmə kontekstinə daxil edilmiş metadata qoruyucusunu təsdiqləmək üçün hadisələri yoxlayın.

## Əlaqədar SDK təlimatları

- [Rust SDK sürətli başlanğıc](/sdks/rust)
- [Python SDK sürətli başlanğıc](/sdks/python)
- [JavaScript SDK sürətli başlanğıc](/sdks/javascript)

[Kotodama mənbəyini endirin](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"),
      account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```