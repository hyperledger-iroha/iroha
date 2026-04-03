<!-- Auto-generated stub for Amharic (Ethiopian) (am) translation. Replace this content with the full translation. -->

---
lang: am
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
title: በመለያዎች መካከል ያለውን ንብረት ያስተላልፉ
description: የኤስዲኬ ፈጣን ጅምርን እና የሂሳብ መዛግብትን የሚያንፀባርቅ ቀጥተኛ የንብረት ማስተላለፍ የስራ ፍሰት።
source: examples/transfer/transfer.ko
---

የኤስዲኬ ፈጣን ጅምርን እና የሂሳብ መዛግብትን የሚያንፀባርቅ ቀጥተኛ የንብረት ማስተላለፍ የስራ ፍሰት።

## የመመዝገቢያ መመሪያ

- አሊስን በዒላማው ንብረት (ለምሳሌ በ"መመዝገቢያ እና ሚንት" ቅንጣቢ ወይም በኤስዲኬ ፈጣን ማስጀመሪያ ፍሰቶች) ቀድመው ይግዙ።
- 10 ክፍሎችን ከአሊስ ወደ ቦብ ለማንቀሳቀስ የ`do_transfer` የመግቢያ ነጥቡን ያስፈጽሙ፣ የ`AssetTransferRole` ፍቃድን ያረካሉ።
- የመጠይቅ ሚዛኖች (`FindAccountAssets`, `iroha_cli ledger asset list`) ወይም የዝውውር ውጤቱን ለመመልከት የቧንቧ መስመር ዝግጅቶችን ይመዝገቡ።

## ተዛማጅ የኤስዲኬ መመሪያዎች

- [ዝገት ኤስዲኬ ፈጣን ጅምር](/sdks/rust)
- [Python SDK quickstart](/sdks/python)
- [JavaScript SDK quickstart](/sdks/javascript)

[የKotodama ምንጭ ያውርዱ](/norito-snippets/transfer-asset.ko)

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