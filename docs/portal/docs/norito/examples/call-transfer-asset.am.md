<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
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
title: የአስተናጋጅ ዝውውርን ከKotodama ጥራ
description: የKotodama የመግቢያ ነጥብ አስተናጋጁን `transfer_asset` መመሪያን በመስመር ውስጥ ሜታዳታ ማረጋገጫ እንዴት እንደሚደውል ያሳያል።
source: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

የKotodama መግቢያ ነጥብ የአስተናጋጁን `transfer_asset` መመሪያን በመስመር ውስጥ ሜታዳታ ማረጋገጫ እንዴት እንደሚደውል ያሳያል።

## የመመዝገቢያ መመሪያ

- የኮንትራት ባለስልጣን (ለምሳሌ `<i105-account-id>` ለኮንትራት ሒሳቡ) በንብረቱ ያዋዋል እና ለባለሥልጣኑ የ `CanTransfer` ሚና ወይም ተመጣጣኝ ፈቃድ ይሰጣል።
- በሰንሰለት አውቶማቲክ የአስተናጋጅ ጥሪዎችን መጠቅለል የሚችልበትን መንገድ በማንጸባረቅ 5 ክፍሎችን ከኮንትራቱ መለያ ወደ Bob (`<i105-account-id>`) ለማዛወር ወደ `call_transfer_asset` መግቢያ ነጥብ ይደውሉ።
- ሚዛኖችን በ`FindAccountAssets` ወይም `iroha_cli ledger asset list --account <i105-account-id>` በኩል ያረጋግጡ እና የሜታዳታ ጠባቂ የዝውውር አውድ መግባቱን ለማረጋገጥ ክስተቶችን ይፈትሹ።

## ተዛማጅ የኤስዲኬ መመሪያዎች

- [ዝገት ኤስዲኬ ፈጣን ጅምር](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[የKotodama ምንጭ አውርድ](/norito-snippets/call-transfer-asset.ko)

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