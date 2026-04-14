<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
source: docs/portal/docs/norito/examples/transfer-asset.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 479d552d0f641875518c62059be1084af6ddf99213662a753c73ea57512b8e5f
source_last_modified: "2026-04-02T18:24:28.189405+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/transfer-asset
title: აქტივების გადაცემა ანგარიშებს შორის
description: პირდაპირი აქტივების გადაცემის სამუშაო პროცესი, რომელიც ასახავს SDK-ის სწრაფ სტარტებს და ბუღალტრულ მიმოხილვებს.
source: examples/transfer/transfer.ko
---

პირდაპირი აქტივების გადაცემის სამუშაო პროცესი, რომელიც ასახავს SDK-ის სწრაფ სტარტებს და ბუღალტრულ მიმოხილვებს.

## ლეჯერის გზამკვლევი

- წინასწარ დააფინანსეთ Alice სამიზნე აქტივით (მაგალითად, „რეგისტრაცია და ზარაფხანა“ სნიპეტის ან SDK სწრაფი დაწყების ნაკადების მეშვეობით).
- შეასრულეთ `do_transfer` შესასვლელი წერტილი, რომ გადაიტანოთ 10 ერთეული ალისიდან ბობში, რაც აკმაყოფილებს `AssetTransferRole` ნებართვას.
- მოითხოვეთ ნაშთები (`FindAccountAssets`, `iroha_cli ledger asset list`) ან გამოიწერეთ მილსადენის ღონისძიებები გადაცემის შედეგზე დასაკვირვებლად.

## დაკავშირებული SDK სახელმძღვანელო

- [Rust SDK სწრაფი დაწყება] (/sdks/rust)
- [Python SDK სწრაფი დაწყება] (/sdks/python)
- [JavaScript SDK სწრაფი დაწყება] (/sdks/javascript)

[ჩამოტვირთეთ Kotodama წყარო](/norito-snippets/transfer-asset.ko)

```text
// Transfer example: uses typed pointer constructors and transfer_asset syscall

seiyaku TransferDemo {
  // Public entrypoint to transfer 10 units of the canonical Base58 asset definition between canonical I105 accounts
  kotoage fn do_transfer() permission(AssetTransferRole) {
    transfer_asset(
      account!("sorauロ1QG1シタ3vN7ヒzトヘcミLKDCAイ5クエjヤリ2uトユmキユルeJBJW7X2N7"),
      account!("sorauロ1NksツJZミLツスjヨrUphCSホ8Wノスマチモr3ムLセヌヒYqwフノFTMDQE"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```