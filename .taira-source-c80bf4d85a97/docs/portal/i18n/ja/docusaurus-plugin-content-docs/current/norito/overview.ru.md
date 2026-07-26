---
lang: ru
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 78244617c92b12b60ecbeb3de4aff8346c6f38d2cc72c01cbfedb2d6af65d982
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/norito/overview.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Norito 概要

Norito は Iroha 全体で使われるバイナリシリアライズ層であり、データ構造がネットワーク上でどのようにエンコードされ、ディスクに永続化され、コントラクトとホスト間で交換されるかを定義します。ワークスペース内のすべてのクレートは `serde` ではなく Norito に依存しており、異なるハードウェア上のピアでも同一のバイト列を生成します。

この概要では主要な要素をまとめ、公式リファレンスへのリンクを示します。

## アーキテクチャ概要

- **ヘッダ + ペイロード** – 各 Norito メッセージは機能ネゴシエーション用ヘッダ（フラグ、チェックサム）で始まり、その後に生のペイロードが続きます。パックされたレイアウトや圧縮はヘッダのビットでネゴシエートされます。
- **決定的なエンコード** – `norito::codec::{Encode, Decode}` が素のエンコードを実装します。ペイロードをヘッダで包む際も同じレイアウトを再利用するため、ハッシュと署名の決定性が保たれます。
- **スキーマ + derives** – `norito_derive` が `Encode`、`Decode`、`IntoSchema` の実装を生成します。パックされた構造体/シーケンスはデフォルトで有効で、`norito.md` に記載されています。
- **マルチコーデック登録簿** – ハッシュ、鍵種別、ペイロード記述子の識別子は `norito::multicodec` にあります。権威ある表は `multicodec.md` で管理されています。

## ツール

| タスク | コマンド / API | 補足 |
| --- | --- | --- |
| ヘッダ/セクションの検査 | `ivm_tool inspect <file>.to` | ABI バージョン、フラグ、エントリポイントを表示します。 |
| Rust でのエンコード/デコード | `norito::codec::{Encode, Decode}` | 主要な data model 型すべてに実装済みです。 |
| JSON 相互運用 | `norito::json::{to_json_pretty, from_json}` | Norito 値に基づく決定的 JSON です。 |
| docs/spec の生成 | `norito.md`, `multicodec.md` | リポジトリルートの一次情報ドキュメントです。 |

## 開発ワークフロー

1. **derive の追加** – 新しいデータ構造には `#[derive(Encode, Decode, IntoSchema)]` を優先してください。よほど必要でない限り手書きのシリアライザは避けます。
2. **パック済みレイアウトの検証** – `cargo test -p norito`（および `scripts/run_norito_feature_matrix.sh` のパック機能マトリクス）を使い、新しいレイアウトが安定していることを確認します。
3. **docs の再生成** – エンコードが変わったら `norito.md` と multicodec 表を更新し、ポータルページ（`/reference/norito-codec` とこの概要）を更新します。
4. **Norito-first のテスト** – 統合テストは `serde_json` ではなく Norito の JSON ヘルパーを使い、本番と同じ経路を通すようにします。

## クイックリンク

- 仕様: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- Multicodec 割り当て: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- フィーチャーマトリクススクリプト: `scripts/run_norito_feature_matrix.sh`
- パック済みレイアウトの例: `crates/norito/tests/`

この概要はクイックスタートガイド（`/norito/getting-started`）と併せて参照してください。Norito ペイロードを使うバイトコードをコンパイルして実行する実践的な手順を示します。
