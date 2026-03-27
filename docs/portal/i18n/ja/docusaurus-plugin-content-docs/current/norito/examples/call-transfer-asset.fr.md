---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/call-transfer-asset.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
スラッグ: /norito/examples/call-transfer-asset
title: Invoquer le transfert hôte depuis Kotodama
説明: Démontre コメント un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne。
ソース: crates/ivm/docs/examples/08_call_transfer_asset.ko
---

Démontre コメント un point d'entrée Kotodama peut appeler l'instruction hôte `transfer_asset` avec validation des métadonnées en ligne。

## 登録公園

- Approvisionnez l'autorité du contrat (par example `<i105-account-id>`) avec l'actif qu'elle transfer et accordez-lui le rôle `CanTransfer` ou une permission équivalente.
- Appelez le point d'entrée `call_transfer_asset` pour transferer 5 Unités du compte du contrat vers `<i105-account-id>`、en reflétant la manière dont l'automatization on-chain peut encapsuler des appels hôte。
- `FindAccountAssets` または `iroha_cli ledger assets list --account <i105-account-id>` を介して、情報を確認し、ジャーナリゼ ル コンテキスト デュ トランスファーを検査します。

## SDK アソシエをガイドします

- [クイックスタート SDK Rust](/sdks/rust)
- [クイックスタート SDK Python](/sdks/python)
- [クイックスタート SDK JavaScript](/sdks/javascript)

[情報源からの電話番号 Kotodama](/norito-snippets/call-transfer-asset.ko)

```text
// Direct builtin call (no contract-style call syntax) inside a contract.
seiyaku TransferCall {
  kotoage fn pay() permission(AssetTransferRole) {
    transfer_asset(
      account!("<i105-account-id>"),
      account!("<i105-account-id>"),
      asset_definition!("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"),
      10
    );
  }
}
```