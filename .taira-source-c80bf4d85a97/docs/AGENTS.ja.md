<!-- Japanese translation of docs/AGENTS.md -->

---
lang: ja
direction: ltr
source: docs/AGENTS.md
status: complete
translator: manual
---

# AGENTS ガイドライン（docs/ ディレクトリ）

このドキュメントは `docs/` 以下で作業する際のルールをまとめています。ルートの `AGENTS.md` と併せて参照してください。

## ドキュメント構成
- `docs/README.md` は外部ドキュメントへの入口を提供します。
- サイト向けの Markdown ソースは `docs/source/` 配下に配置してください。

## 開発フロー
- ドキュメントは Markdown で記述し、可能な限り相対リンクを使用します。
- コード例を更新した場合は `cargo test` を実行し、ビルド／テストが成功することを確認してください。
- 実行可能なサンプルを優先し、可能であれば該当クレート配下の `examples/` に置いてここから参照します。
- パスを変更した際はリンクとアンカーを必ず検証してください。必要に応じてリポジトリルートの `lychee.toml` を使って `lychee` でリンクチェックを行えます。
- フォーマットや一般的な規約についてはリポジトリルートの `AGENTS.md` に従ってください。

## ステータスとロードマップ
- 最新の進捗や計画についてはリポジトリルートの `status.md` と `roadmap.md` を参照してください。

## 便利なコマンド
- 個別クレートのドキュメントをローカルでビルド: `cargo doc -p <crate> --no-deps --open`

## シリアライゼーション方針
- `docs/` 配下のコード例やツールで新たに `serde` / `serde_json` を直接利用してはいけません。Norito のラッパーを使って JSON／バイナリを処理してください。
  - JSON: `norito::json::{from_*, to_*, json!, Value}`
  - バイナリ: `norito::{Encode, Decode}`
- 外部型との互換性のためにどうしても Serde が必要な場合は、Norito ラッパー内部に閉じ込め、新たな `serde` 依存を追加しないでください。
