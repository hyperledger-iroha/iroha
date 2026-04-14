<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
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
title: NFT-г тайлах, шилжүүлэх, шатаах
description: NFT-ийн амьдралын мөчлөгийг эцэс төгсгөл хүртэл нь дамждаг: эзэмшигч рүү шилжүүлэх, мета өгөгдлийг шилжүүлэх, шошголох, шатаах.
source: crates/ivm/docs/examples/12_nft_flow.ko
---

NFT-ийн амьдралын мөчлөгийг эцэс төгсгөл хүртэл нь дамждаг: эзэмшигч рүү шилжүүлэх, мета өгөгдлийг шилжүүлэх, шошголох, шатаах.

## Бүртгэлийн дэвтэр

- Хэсэгт ашигласан эзэмшигч/хүлээн авагчийн дансны хажууд NFT тодорхойлолт (жишээ нь `n0#wonderland`) байгаа эсэхийг шалгаарай (Алисын хувьд `<i105-account-id>`, Бобын хувьд `<i105-account-id>`).
- NFT-г гаргахын тулд `nft_issue_and_transfer` нэвтрэх цэгийг дуудаж, Алисаас Боб руу шилжүүлж, олголтыг тайлбарласан мета өгөгдлийн тугийг хавсаргана уу.
- Шилжүүлгийг баталгаажуулахын тулд NFT дэвтэрийн төлөвийг `iroha_cli ledger nft list --account <id>` эсвэл SDK түүнтэй адилтгах хэрэглүүрээр шалгаад дараа нь шатаах заавар ажилласны дараа хөрөнгийг устгасан эсэхийг баталгаажуулна уу.

## Холбогдох SDK гарын авлага

- [Зэв SDK хурдан эхлүүлэх](/sdks/rust)
- [Python SDK хурдан эхлүүлэх](/sdks/python)
- [JavaScript SDK хурдан эхлүүлэх](/sdks/javascript)

[Kotodama эх сурвалжийг татаж авах](/norito-snippets/nft-flow.ko)

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