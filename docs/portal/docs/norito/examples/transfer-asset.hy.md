<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
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
title: Ակտիվների փոխանցում հաշիվների միջև
description: Ակտիվների փոխանցման պարզ աշխատանքային հոսք, որը արտացոլում է SDK-ի արագ մեկնարկներն ու մատյանների շրջադարձերը:
source: examples/transfer/transfer.ko
---

Ակտիվների փոխանցման պարզ աշխատանքային հոսք, որը արտացոլում է SDK-ի արագ մեկնարկներն ու մատյանների շրջադարձերը:

## Լեջերի քայլարշավ

- Ալիսի նախնական ֆինանսավորումը թիրախային ակտիվի հետ (օրինակ՝ «գրանցել և դրամահատարան» հատվածի կամ SDK արագ մեկնարկի հոսքերի միջոցով):
- Գործարկեք `do_transfer` մուտքի կետը՝ 10 միավոր Ալիսից Բոբ տեղափոխելու համար՝ բավարարելով `AssetTransferRole` թույլտվությունը:
- Հարցրեք մնացորդները (`FindAccountAssets`, `iroha_cli ledger asset list`) կամ բաժանորդագրվեք խողովակաշարի իրադարձություններին՝ դիտարկելու փոխանցման արդյունքը:

## Առնչվող SDK ուղեցույցներ

- [Rust SDK արագ մեկնարկ] (/sdks/rust)
- [Python SDK-ի արագ մեկնարկ] (/sdks/python)
- [JavaScript SDK արագ մեկնարկ] (/sdks/javascript)

[Ներբեռնեք Kotodama աղբյուրը](/norito-snippets/transfer-asset.ko)

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