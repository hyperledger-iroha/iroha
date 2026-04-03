<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
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
title: Активтерді шоттар арасында аудару
description: SDK жылдам іске қосулары мен бухгалтерлік кітаптың қадамдарын көрсететін тікелей активтерді тасымалдау жұмыс процесі.
source: examples/transfer/transfer.ko
---

SDK жылдам іске қосулары мен бухгалтерлік кітаптың қадамдарын көрсететін тікелей активтерді тасымалдау жұмыс процесі.

## Бухгалтерлік кітапшаға шолу

- Алисаны мақсатты активпен алдын ала қаржыландырыңыз (мысалы, «тіркеу және енгізу» үзіндісі немесе SDK жылдам бастау ағындары арқылы).
- `AssetTransferRole` рұқсатын қанағаттандыра отырып, Алисадан Бобқа 10 бірлік жылжыту үшін `do_transfer` кіру нүктесін орындаңыз.
- Баланстарды (`FindAccountAssets`, `iroha_cli ledger asset list`) сұраңыз немесе тасымалдау нәтижесін бақылау үшін құбыр оқиғаларына жазылыңыз.

## Қатысты SDK нұсқаулықтары

- [Rust SDK жылдам іске қосу](/sdks/rust)
- [Python SDK жылдам іске қосу](/sdks/python)
- [JavaScript SDK жылдам іске қосу](/sdks/javascript)

[Kotodama көзін жүктеп алыңыз](/norito-snippets/transfer-asset.ko)

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