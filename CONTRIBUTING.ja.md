<!-- Japanese translation of CONTRIBUTING.md -->

---
lang: ja
direction: ltr
source: CONTRIBUTING.md
status: complete
translator: manual
---

# コントリビュートガイド

Iroha 2 への貢献を検討してくださりありがとうございます。ここでは、どのように貢献できるか、そして守っていただきたいガイドライン（コード／ドキュメントの方針や Git ワークフローの慣習など）を説明します。あらかじめ目を通しておくことで、後々の手戻りを減らすことができます。

## どのように貢献できますか？

プロジェクトに関わる方法は多数あります。

- [バグ報告](#バグ報告) や [脆弱性の報告](#脆弱性の報告) を行う
- [改善提案](#改善提案) を出し、実装する
- [質問する](#質問する)・コミュニティに参加して議論する

初めての方は [最初のコード貢献](#初めてのコード貢献) から始めてみてください。

### TL;DR（要約）

- [ZenHub](https://app.zenhub.com/workspaces/iroha-v2-60ddb820813b9100181fc060/board?repos=181739240) でチケットを確認
- [Iroha リポジトリ](https://github.com/hyperledger-iroha/iroha/tree/main) をフォーク
- 対応する Issue を選び修正
- コード／ドキュメントは [スタイルガイド](#スタイルガイド) に従う
- `cargo test --workspace` でテストを実行（必要なテストも追加）
  - IVM executor を扱うテストは `defaults/executor.to` が存在しなくても最小構成のバイトコードを自動生成します。整合性を取りたい場合は
    - `cargo run --manifest-path scripts/generate_executor_to/Cargo.toml`
    - `cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml`
    を実行するとカノニカルなバイトコードを再生成できます。
- プッシュ前に `make guards` を実行し、Norito 以外での `serde_json` 利用などをガード
- 任意: `make norito-matrix`（高速版）や `scripts/run_norito_feature_matrix.sh` で Norito フィーチャーマトリクスを検証
- フォーマットや成果物再生成などプレコミット処理（[`hooks/pre-commit.sample`](./hooks/pre-commit.sample) 参照）
- `git pull -r upstream main` / `git commit -s` / `git push <your-fork>` を行い、[PR を作成](https://github.com/hyperledger-iroha/iroha/compare)（PR ガイドラインに従う）

## バグ報告

バグとは、Iroha が不正確・予期しない結果や挙動を示すエラー／設計欠陥／障害を指します。  
GitHub Issue の `Bug` ラベルで追跡しています。

新規 Issue のテンプレートに沿って、以下を満たすようにしてください。

- [ ] `Bug` ラベルの付与
- [ ] 問題の説明
- [ ] 最小実行例（Minimal Working Example）の添付
- [ ] スクリーンショットの添付（可能であれば）

<details>
<summary>最小実行例について</summary>

各バグには [Minimal Working Example](https://ja.wikipedia.org/wiki/%E6%9C%80%E5%B0%8F%E7%8F%BE%E5%83%8F%E4%BE%8B) を添付してください。

```
# Numeric 値での負の資産 Mint

負の値をミントできてしまったため報告します。これは Iroha の仕様に反します。

# 前提

次のコードで負の値をミントできました:
<ここにコード>

# 期待した結果

負の値は受け付けられない

# 実際の結果

<負の値がミントされた証拠（コードやログ）>

<スクリーンショット>
```
</details>

> **注:** 古いドキュメント、情報不足、機能追加の要望などは `Documentation` や `Enhancement` ラベルを利用し、バグとして扱わないでください。

## 脆弱性の報告

セキュリティ問題を防ぐよう努めていますが、先に脆弱性に気づく可能性があります。

- 初回メジャーリリース（2.0）前は、脆弱性もバグとして扱います。前述の手順で Issue を登録してください。
- 2.0 以降は [bug bounty program](https://hackerone.com/hyperledger) から報告すると報奨が得られます。

:exclamation: 修正前の脆弱性は公開被害を最小化するため、**速やかに Hyperledger へ直接報告し、一定期間は公開しない** でください。  
対応方針について質問がある場合は、Rocket.Chat で現行メンテナーに DM してください。

## 改善提案

改善提案は Issue を立ててから進めてください。

- 問題点の説明・改善案
- 参考資料や設計メモ
- 予想される影響範囲

コード変更を伴う場合は、可能であれば提案とあわせて PR を作成してください。

## 質問する

[Discussions](https://github.com/hyperledger-iroha/iroha/discussions) または Rocket.Chat で質問できます。  
質問内容には、試したこと・期待した結果・得られた結果・バージョン情報などを含めてください。

## 初めてのコード貢献

`good first issue` や `help wanted` ラベルの Issue から取り組むのがおすすめです。

1. Issue を選択し、コメントで着手を表明
2. リポジトリをフォークし、ブランチを作成
3. 変更・テスト・`cargo fmt` を実行
4. DCO サインオフ付きでコミット (`git commit -s`)
5. PR を作成し、レビューを受ける

## スタイルガイド

- Rust コード: Rust Style Guide に準拠（整形は `cargo fmt`、Lint は `cargo clippy -- -D warnings`）
- ドキュメント: Markdown の一貫した書式、Norito まわりの用語統一、表記揺れに注意
- コミット／PR タイトル: [Conventional Commits](https://www.conventionalcommits.org/ja/v1.0.0/) に従う

## テストとガード

- `cargo build --workspace` でビルドが通ること
- `cargo test --workspace` で全テストを実行
- `make guards` で Norito コーデックのガードや serde 依存チェックを実行
- 必要に応じ `scripts/run_norito_feature_matrix.sh` でフィーチャーマトリクスを検証

## Git ワークフロー

- リポジトリをフォークし、作業用ブランチを作成
- `git remote add upstream https://github.com/hyperledger-iroha/iroha.git`
- こまめに `git pull --rebase upstream main` を実行（`git pull` は使用しない）
- すべてのコミットに `git commit -s` で DCO サインオフを付与（`-sS` で GPG 署名も推奨）
- コミット／PR タイトルは Conventional Commits を遵守し、簡潔かつ意図が分かるよう記述

## Pull Request の流れ

1. `git push <your-fork> <branch>` で自分のフォークにプッシュ
2. PR を作成し、概要・テスト結果・関連 Issue などを記入
3. Draft PR でレビューを早期に依頼することも可能
4. すべての CI（テスト・フォーマット・clippy 等）が通ることがマージの前提
5. 最低 2 名のメンテナー承認が必要。コードオーナーが自動通知されます。

### レビューの作法

- レビュアーのコメントには必ず応答し、自己判断でスレッドを解決しない
- レビュアーが何度も差分を追えるよう、再レビュー時に不要な履歴書き換えは避ける
- 軽微な指摘であっても敬意を払い、提案を取り入れるか理由を説明する

### Pull Request タイトル

PR タイトルは自動的に解析され、チェンジログ生成や `check-PR-title` チェックに利用されます。

1. Conventional Commits 形式で記述（例: `feat: add new asset query`）
2. PR にコミットが 1 つしかない場合、タイトルとコミットメッセージを揃える

### Git スタイル

- Feature ブランチを使い回さず PR ごとに作成
- コミット数は必要最小限にまとめ、最終的には squash merge がかかりますが履歴を読みやすく保つ
- `hooks/` にある Git フック（例: `commit-msg`）でサインオフを自動化できます

## 追加リソース

- [MAINTAINERS.md](MAINTAINERS.md) – 現在のメンテナー一覧
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) – コミュニティポリシー
- [CONTRIBUTING.md（英語版）](CONTRIBUTING.md) – 原文
