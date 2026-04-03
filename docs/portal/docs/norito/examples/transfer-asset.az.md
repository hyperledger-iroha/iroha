<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
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
title: Aktivi hesablar arasında köçürün
description: SDK-nın sürətli başlanğıclarını və mühasibat kitablarının gedişatını əks etdirən sadə aktiv köçürmə iş axını.
source: examples/transfer/transfer.ko
---

SDK-nın sürətli başlanğıclarını və mühasibat kitablarının gedişatını əks etdirən sadə aktiv köçürmə iş axını.

## Ledger prospekti

- Hədəf aktivi ilə Aliceni əvvəlcədən maliyyələşdirin (məsələn, “qeydiyyatdan keç və nanə” fraqmenti və ya SDK sürətli başlanğıc axınları vasitəsilə).
- `AssetTransferRole` icazəsini təmin edərək, Alicedən Bob-a 10 vahid köçürmək üçün `do_transfer` giriş nöqtəsini icra edin.
- Qalıqları sorğulayın (`FindAccountAssets`, `iroha_cli ledger asset list`) və ya köçürmə nəticəsini müşahidə etmək üçün boru kəməri hadisələrinə abunə olun.

## Əlaqədar SDK təlimatları

- [Rust SDK sürətli başlanğıc](/sdks/rust)
- [Python SDK sürətli başlanğıc](/sdks/python)
- [JavaScript SDK sürətli başlanğıc](/sdks/javascript)

[Kotodama mənbəyini endirin](/norito-snippets/transfer-asset.ko)

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