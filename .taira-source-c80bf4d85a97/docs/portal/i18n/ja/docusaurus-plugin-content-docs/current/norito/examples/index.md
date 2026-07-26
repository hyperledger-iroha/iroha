---
lang: ja
direction: ltr
source: docs/portal/docs/norito/examples/index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito の例
description: 台帳ウォークスルー付きの厳選 Kotodama スニペット。
slug: /norito/examples
---

これらの例は SDK のクイックスタートと台帳ウォークスルーを反映しています。各スニペットには台帳チェックリストが含まれ、Rust、Python、JavaScript のガイドに戻れるので、同じシナリオを端から端まで再現できます。

- **[Hajimari エントリポイントの骨組み](./hajimari-entrypoint)** — 単一の公開エントリポイントと状態ハンドルを備えた、最小構成の Kotodama コントラクトの足場。
- **[ドメイン登録と資産のミント](./register-and-mint)** — 権限付きドメインの作成、資産登録、決定論的ミントを実演します。
- **[Kotodama からホスト転送を呼び出す](./call-transfer-asset)** — Kotodama のエントリポイントがホストの `transfer_asset` 命令を、インラインのメタデータ検証付きで呼び出せることを示します。
- **[アカウント間の資産移転](./transfer-asset)** — SDK クイックスタートと台帳ウォークスルーを反映した、わかりやすい資産移転フローです。
- **[NFT のミント、移転、バーン](./nft-flow)** — NFT のライフサイクルを端から端までたどります: オーナーへのミント、移転、メタデータのタグ付け、バーン。
