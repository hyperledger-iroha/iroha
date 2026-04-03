<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
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
title: ሚንት፣ ያስተላልፉ እና NFT ያቃጥሉ።
description: በኤንኤፍቲ የሕይወት ዑደት ውስጥ ከጫፍ እስከ ጫፍ ይራመዳል፡ ለባለቤቱ መጥራት፣ ማስተላለፍ፣ ዲበ ዳታ መለያ መስጠት እና ማቃጠል።
source: crates/ivm/docs/examples/12_nft_flow.ko
---

በኤንኤፍቲ የሕይወት ዑደት ውስጥ ከጫፍ እስከ ጫፍ ይራመዳል፡ ለባለቤቱ መጥራት፣ ማስተላለፍ፣ ዲበ ዳታ መለያ መስጠት እና ማቃጠል።

## የመመዝገቢያ መመሪያ

- የኤንኤፍቲ ትርጉም (ለምሳሌ `n0#wonderland`) ከባለቤቱ/የተቀባዩ መለያዎች ጋር በቅንጭቡ ውስጥ መኖራቸውን ያረጋግጡ (`<i105-account-id>` ለአሊስ፣ `<i105-account-id>` ለቦብ)።
- NFTን ለመስራት የ`nft_issue_and_transfer` የመግቢያ ነጥቡን ጥራ፣ ከአሊስ ወደ ቦብ ያስተላልፉት፣ እና መውጣቱን የሚገልጽ የሜታዳታ ባንዲራ ያያይዙ።
- ዝውውሩን ለማረጋገጥ የNFT ደብተር ሁኔታን በ`iroha_cli ledger nft list --account <id>` ወይም በኤስዲኬ አቻዎች ይመርምሩ፣ ከዚያም የተቃጠለ መመሪያው ከሄደ በኋላ ንብረቱ መወገዱን ያረጋግጡ።

## ተዛማጅ የኤስዲኬ መመሪያዎች

- [ዝገት ኤስዲኬ ፈጣን ጅምር](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[የKotodama ምንጭ ያውርዱ](/norito-snippets/nft-flow.ko)

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