<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
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
title: გამოიძახეთ ჰოსტის გადაცემა Kotodama-დან
description: გვიჩვენებს, თუ როგორ შეუძლია Kotodama შესვლის წერტილს დაურეკოს ჰოსტის `transfer_asset` ინსტრუქციას მეტამონაცემების შიდსული დადასტურებით.
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

გვიჩვენებს, თუ როგორ შეუძლია Kotodama შესვლის წერტილს დაურეკოს ჰოსტის `transfer_asset` ინსტრუქციას მეტამონაცემების ვალიდაციის საშუალებით.

## ლეჯერის გზამკვლევი

- დააფინანსეთ საკონტრაქტო ორგანო (მაგალითად, `<i105-account-id>` კონტრაქტის ანგარიშისთვის) აქტივით, რომელიც გადასცემს და უფლებამოსილებას მიანიჭებს `CanTransfer` როლს ან ექვივალენტურ ნებართვას.
- დაურეკეთ `call_transfer_asset` შესასვლელ წერტილს, რომ გადაიტანოთ 5 ერთეული კონტრაქტის ანგარიშიდან ბობზე (`<i105-account-id>`), რაც ასახავს იმ გზას, თუ როგორ ასახავს ჯაჭვზედა ავტომატიზაციას ჰოსტის ზარების გადატანა.
- გადაამოწმეთ ნაშთები `FindAccountAssets` ან `iroha_cli ledger asset list --account <i105-account-id>` მეშვეობით და შეამოწმეთ მოვლენები, რათა დაადასტუროთ, რომ მეტამონაცემების მცველმა დაარეგისტრირა გადაცემის კონტექსტი.

## დაკავშირებული SDK სახელმძღვანელო

- [Rust SDK სწრაფი დაწყება] (/sdks/rust)
- [Python SDK სწრაფი დაწყება] (/sdks/python)
- [JavaScript SDK სწრაფი დაწყება] (/sdks/javascript)

[ჩამოტვირთეთ Kotodama წყარო](/norito-snippets/call-transfer-asset.ko)

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