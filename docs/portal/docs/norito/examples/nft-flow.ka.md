<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
source: docs/portal/docs/norito/examples/nft-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09ff44a2df8cbcb9f57017239070a16f5287cbfc59a8289ce54933e84f90a5e8
source_last_modified: "2026-03-26T13:01:47.374572+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/nft-flow
title: ზარაფხანა, გადატანა და დაწვა NFT
description: გადის NFT-ის სასიცოცხლო ციკლის ბოლომდე: მიწოდება მფლობელთან, გადაცემა, მეტამონაცემების მონიშვნა და დაწვა.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

გადის NFT-ის სასიცოცხლო ციკლის ბოლომდე: მიწოდება მფლობელთან, გადაცემა, მეტამონაცემების მონიშვნა და დაწვა.

## ლეჯერის გზამკვლევი

- დარწმუნდით, რომ NFT განმარტება (მაგალითად, `n0#wonderland`) არსებობს მფლობელის/მიმღების ანგარიშებთან ერთად, რომლებიც გამოიყენება ნაწყვეტში (`<i105-account-id>` ალისისთვის, `<i105-account-id>` ბობისთვის).
- გამოიძახეთ `nft_issue_and_transfer` შესასვლელი წერტილი NFT-ის დასამზადებლად, გადაიტანეთ იგი ალისიდან ბობში და მიამაგრეთ მეტამონაცემების დროშა, რომელიც აღწერს გაცემას.
- შეამოწმეთ NFT ledger-ის მდგომარეობა `iroha_cli ledger nft list --account <id>`-ით ან SDK-ის ეკვივალენტებით გადაცემის დასადასტურებლად, შემდეგ დაადასტურეთ, რომ აქტივი ამოღებულია დამწვრობის ინსტრუქციის გაშვების შემდეგ.

## დაკავშირებული SDK სახელმძღვანელო

- [Rust SDK სწრაფი დაწყება] (/sdks/rust)
- [Python SDK სწრაფი დაწყება] (/sdks/python)
- [JavaScript SDK სწრაფი დაწყება] (/sdks/javascript)

[ჩამოტვირთეთ Kotodama წყარო](/norito-snippets/nft-flow.ko)

```text
// Mint an NFT, transfer it, update metadata, and burn it using typed IDs.
seiyaku NftFlow {
  #[access(read="*", write="*")]
  kotoage fn nft_issue_and_transfer() permission(NftAuthority) {
    let owner = account!("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB");
    let nft = nft_id!("n0$wonderland");
    nft_mint_asset(nft, owner);

    let to = account!("sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76");
    nft_transfer_asset(owner, nft, to);
    nft_set_metadata(nft, json!{ issued: "demo" });
    nft_burn_asset(nft);
  }
}
```