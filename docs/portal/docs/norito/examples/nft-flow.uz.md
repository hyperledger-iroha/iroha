<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
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
title: NFTni zarb qiling, uzating va yoqing
description: NFT hayotiy tsiklini oxirigacha bosib o'tadi: egasiga zarb qilish, metama'lumotlarni uzatish, teglash va yoqish.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT hayotiy tsiklini oxirigacha bosib o'tadi: egasiga zarb qilish, metama'lumotlarni uzatish, teglash va yoqish.

## Buxgalteriya kitobi bo'yicha ko'rsatmalar

- NFT ta'rifi (masalan, `n0#wonderland`) parchada ishlatiladigan egasi/qabul qiluvchi hisoblari (Elis uchun `<i105-account-id>`, Bob uchun `<i105-account-id>`) bilan birga mavjudligiga ishonch hosil qiling.
- NFTni zarb qilish uchun `nft_issue_and_transfer` kirish nuqtasini chaqiring, uni Elisdan Bobga o'tkazing va emissiyani tavsiflovchi metadata bayrog'ini qo'shing.
- O'tkazmani tekshirish uchun `iroha_cli ledger nft list --account <id>` yoki SDK ekvivalentlari bilan NFT daftarining holatini tekshiring, so'ngra yoqish bo'yicha ko'rsatma bajarilgandan so'ng aktiv o'chirilganligini tasdiqlang.

## Tegishli SDK qo'llanmalari

- [Rust SDK tezkor ishga tushirish](/sdks/rust)
- [Python SDK tezkor ishga tushirish](/sdks/python)
- [JavaScript SDK tezkor ishga tushirish](/sdks/javascript)

[Kotodama manbasini yuklab oling](/norito-snippets/nft-flow.ko)

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