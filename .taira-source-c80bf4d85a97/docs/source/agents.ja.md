---
lang: ja
direction: ltr
source: docs/source/agents.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f35a28d00188a3e1f3db76b56e6b29c708dbb75afa3dd009d416b7cd4314754
source_last_modified: "2025-11-14T19:32:43.578041+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/agents.md -->

# 自動化エージェント実行ガイド

このページは、Hyperledger Iroha ワークスペース内で作業する自動化エージェント向けの
運用ガードレールを要約する。正準の `AGENTS.md` ガイダンスとロードマップ参照を反映し、
ビルド・ドキュメント・テレメトリの変更が人間の作業か自動化かに関わらず同一に見えるようにする。

各タスクは決定論的なコードに加え、対応するドキュメント、テスト、運用証拠を含めて
着地することが期待される。`roadmap.md` の項目に触れる前や振る舞いに関する質問へ
回答する前に、以下のセクションを参照として扱うこと。

## クイックスタート コマンド

| アクション | コマンド |
|--------|---------|
| ワークスペースのビルド | `cargo build --workspace` |
| 全テストの実行 | `cargo test --workspace` *(通常数時間かかる)* |
| 既定で警告を deny する clippy | `cargo clippy --workspace --all-targets -- -D warnings` |
| Rust フォーマット | `cargo fmt --all` *(edition 2024)* |
| 単一クレートのテスト | `cargo test -p <crate>` |
| 単一テストの実行 | `cargo test -p <crate> <test_name> -- --nocapture` |
| Swift SDK テスト | `IrohaSwift/` で `swift test` |

## ワークフローの基本

- 質問に答えたりロジックを変更する前に、関連するコードパスを読む。
- 大きなロードマップ項目は実行可能な単位に分割する。作業を拒否しない。
- 既存のワークスペース構成を維持し、内部クレートを再利用し、明示的な指示がない限り
  `Cargo.lock` を変更しない。
- ハードウェアアクセラレーションで必要な場合を除き、feature flag や capability toggle を
  追加しない。すべてのプラットフォームで決定論的フォールバックを維持する。
- 機能変更のたびにドキュメントと Markdown 参照を更新し、現行動作を常に反映させる。
- 新規/変更された関数ごとに少なくとも 1 つのユニットテストを追加する。範囲に応じて
  `#[cfg(test)]` もしくはクレートの `tests/` を選ぶ。
- 作業完了後は `status.md` に短い要約と関連ファイルを追記し、`roadmap.md` は
  未完了項目に集中させる。

## 実装ガードレール

### シリアライズとデータモデル
- Norito コーデックを全体で使用する（バイナリは `norito::{Encode, Decode}`、JSON は
  `norito::json::*`）。`serde`/`serde_json` を直接使わない。
- Norito ペイロードはレイアウトを明示する（バージョンバイトまたはヘッダーフラグ）。
  新フォーマット追加時は対応ドキュメント（例: `norito.md`, `docs/source/da/*.md`）を更新する。
- Genesis データ、マニフェスト、ネットワークペイロードは決定論的であるべきで、
  同じ入力から同じハッシュが生成される必要がある。

### 設定とランタイム挙動
- 新しい環境変数よりも `crates/iroha_config` のノブを優先し、
  値はコンストラクタや依存性注入で明示的に渡す。
- IVM の syscall/opcode をゲートしない。ABI v1 は常に出荷される。
- 新しい設定オプションを追加したら、defaults、docs、関連テンプレート
  (`peer.template.toml`, `docs/source/configuration*.md` など) を更新する。

### ABI / Syscall / Pointer 型
- ABI ポリシーは無条件。syscall や pointer 型を追加/削除した場合は以下を更新する:
  - `ivm::syscalls::abi_syscall_list` と `crates/ivm/tests/abi_syscall_list_golden.rs`
  - `ivm::pointer_abi::PointerType` と golden テスト
  - ABI hash が変わるたびに `crates/ivm/tests/abi_hash_versions.rs`
- 未知の syscall は `VMError::UnknownSyscall` にマップされる必要があり、
  マニフェストは admission テストで署名済み `abi_hash` の一致を維持する。

### ハードウェアアクセラレーションと決定性
- 新しい暗号プリミティブや重い数値計算は METAL/NEON/SIMD/CUDA の加速パスを
  提供し、同時に決定論的フォールバックを維持する。
- 非決定論的な並列リダクションは避ける。ハードウェア差があっても出力一致を最優先。
- Norito および FASTPQ のフィクスチャは再現可能に保ち、SRE がフリート全体の
  テレメトリを監査できるようにする。

### ドキュメントと証拠
- 公開ドキュメント変更がある場合は、`docs/portal/...` のミラーも更新して
  ドキュメントサイトを最新に保つ。
- 新しいワークフローには runbook、ガバナンスメモ、チェックリストを追加し、
  リハーサル/ロールバック/証拠収集手順を説明する。
- アッカド語に翻訳する場合は、音写ではなく楔形文字による意味翻訳を提供する。

### テストとツールの期待値
- `cargo test`、`swift test`、統合ハーネスなど関連するテストをローカル実行し、
  PR の Testing セクションにコマンドを記載する。
- CI ガード (`ci/*.sh`) とダッシュボードは新しいテレメトリに合わせて更新する。
- proc-macro にはユニットテストと `trybuild` UI テストを組み合わせ、診断を固定する。

## 出荷準備チェックリスト

1. コードがコンパイルでき、`cargo fmt` が差分を作らないこと。
2. 更新済みドキュメント（ワークスペース Markdown とポータルミラー）が新しい挙動、
   CLI フラグ、または設定ノブを説明していること。
3. テストが新しいコードパスをすべてカバーし、回帰時に決定論的に失敗すること。
4. テレメトリ、ダッシュボード、アラート定義が新しいメトリクスやエラーコードに言及すること。
5. `status.md` に関連ファイルとロードマップ項目を参照した短い要約が含まれること。

このチェックリストに従うことでロードマップ実行の監査可能性が保たれ、
すべてのエージェントが信頼できる証拠を提供できる。
