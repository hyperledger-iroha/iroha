<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
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
title: Mint, փոխանցել և այրել NFT
description: Քայլում է NFT-ի կյանքի ցիկլի միջով մինչև վերջ՝ հատում սեփականատիրոջը, փոխանցում, հատկորոշում մետատվյալներ և այրում:
source: crates/ivm/docs/examples/12_nft_flow.ko
---

Քայլում է NFT-ի կյանքի ցիկլի միջով մինչև վերջ՝ հատում սեփականատիրոջը, փոխանցում, հատկորոշում մետատվյալներ և այրում:

## Լեջերի քայլարշավ

- Համոզվեք, որ NFT սահմանումը (օրինակ՝ `n0#wonderland`) գոյություն ունի հատվածում օգտագործված սեփականատիրոջ/ստացողի հաշիվների կողքին (`<i105-account-id>` Ալիսի համար, `<i105-account-id>`՝ Բոբի համար):
- Զանգահարեք `nft_issue_and_transfer` մուտքի կետը՝ NFT-ն կտրելու համար, փոխանցեք այն Ալիսից Բոբին և կցեք մետատվյալների դրոշակ, որը նկարագրում է թողարկումը:
- Ստուգեք NFT մատյանային վիճակը `iroha_cli ledger nft list --account <id>`-ով կամ SDK-ի համարժեքներով՝ փոխանցումը ստուգելու համար, այնուհետև հաստատեք, որ ակտիվը հեռացված է, երբ գործարկվի այրման հրահանգը:

## Առնչվող SDK ուղեցույցներ

- [Rust SDK արագ մեկնարկ] (/sdks/rust)
- [Python SDK-ի արագ մեկնարկ] (/sdks/python)
- [JavaScript SDK արագ մեկնարկ] (/sdks/javascript)

[Ներբեռնեք Kotodama աղբյուրը](/norito-snippets/nft-flow.ko)

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