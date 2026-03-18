---
lang: ja
direction: ltr
source: docs/portal/docs/norito/overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c28a429f0ade5a5e93c063dc7eda4b95fd0c379a7598b72f19367ca13734e443
source_last_modified: "2026-01-03T18:07:57+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito 概要

Norito は、Iroha 全体で使用されるバイナリ シリアル化レイヤーです。データがどのように処理されるかを定義します。
構造はネットワーク上でエンコードされ、ディスク上に保持され、相互に交換されます。
契約とホスト。ワークスペース内のすべてのクレートは、代わりに Norito に依存します。
`serde` なので、異なるハードウェア上のピアは同一のバイトを生成します。

この概要には、核となる部分と正規の参照へのリンクがまとめられています。

## アーキテクチャの概要

- **ヘッダー + ペイロード** – 各 Norito メッセージは機能ネゴシエーションで始まります
  ヘッダー (フラグ、チェックサム) とその後にベア ペイロードが続きます。パックされたレイアウトと
  圧縮はヘッダー ビットを介してネゴシエートされます。
- **決定論的エンコーディング** – `norito::codec::{Encode, Decode}` は
  裸のエンコーディング。ヘッダーでペイロードをラップするときに同じレイアウトが再利用されるため、
  ハッシュと署名は決定的なままです。
- **スキーマ + 派生** – `norito_derive` は `Encode`、`Decode`、および
  `IntoSchema` の実装。パックされた構造体/シーケンスはデフォルトで有効になります
  `norito.md` に文書化されています。
- **マルチコーデック レジストリ** – ハッシュ、キー タイプ、ペイロードの識別子
  記述子は `norito::multicodec` にあります。権威のあるテーブルは
  `multicodec.md` で維持されます。

## ツーリング

|タスク |コマンド/API |メモ |
| --- | --- | --- |
|ヘッダー/セクションを検査する | `ivm_tool inspect <file>.to` | ABI のバージョン、フラグ、およびエントリポイントを表示します。 |
| Rustでのエンコード/デコード | `norito::codec::{Encode, Decode}` |すべてのコア データ モデル タイプに実装されます。 |
| JSON 相互運用 | `norito::json::{to_json_pretty, from_json}` | Norito 値に裏付けられた決定論的な JSON。 |
|ドキュメント/仕様を生成する | `norito.md`、`multicodec.md` |リポジトリのルートにある信頼できる情報源のドキュメント。 |

## 開発ワークフロー

1. **派生を追加** – 新しいデータには `#[derive(Encode, Decode, IntoSchema)]` を優先します
   構造物。どうしても必要な場合を除き、手書きのシリアライザーは避けてください。
2. **パックされたレイアウトを検証する** – `cargo test -p norito` (およびパックされたレイアウトを検証する) を使用します。
   `scripts/run_norito_feature_matrix.sh` の機能マトリックス) を使用して、新しいことを保証します。
   レイアウトは安定したままです。
3. **ドキュメントを再生成** – エンコードが変更された場合は、`norito.md` と
   マルチコーデック テーブルを確認してから、ポータル ページを更新します (`/reference/norito-codec`)
   およびこの概要）。
4. **テスト Norito-first を維持します** – 統合テストでは Norito JSON を使用する必要があります
   `serde_json` の代わりにヘルパーを使用するため、本番環境と同じパスを実行します。

## クイックリンク

- 仕様: [`norito.md`](https://github.com/hyperledger-iroha/iroha/blob/master/norito.md)
- マルチコーデックの割り当て: [`multicodec.md`](https://github.com/hyperledger-iroha/iroha/blob/master/multicodec.md)
- 機能マトリックス スクリプト: `scripts/run_norito_feature_matrix.sh`
- パックされたレイアウトの例: `crates/norito/tests/`

この概要とクイックスタート ガイド (`/norito/getting-started`) を組み合わせて、
Norito を使用するバイトコードのコンパイルと実行の実践的なチュートリアル
ペイロード。