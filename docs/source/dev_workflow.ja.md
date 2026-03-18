---
lang: ja
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-18T17:14:31.034360+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# エージェント開発ワークフロー

この Runbook は、AGENTS ロードマップからのコントリビューター ガードレールを統合します。
新しいパッチは同じデフォルト ゲートに従います。

## クイックスタート ターゲット

- `make dev-workflow` (`scripts/dev_workflow.sh` のラッパー) を実行して、以下を実行します。
  1.`cargo fmt --all`
  2.`cargo clippy --workspace --all-targets --locked -- -D warnings`
  3.`cargo build --workspace --locked`
  4.`cargo test --workspace --locked`
  5. `swift test` から `IrohaSwift/`
- `cargo test --workspace` は長時間実行されます (多くの場合、数時間)。素早い繰り返しを行うには、
  `scripts/dev_workflow.sh --skip-tests` または `--skip-swift` を使用し、完全なコマンドを実行します。
  出荷前のシーケンス。
- `cargo test --workspace` がビルド ディレクトリ ロックで停止した場合は、次のように再実行します。
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (または設定
  `CARGO_TARGET_DIR` を分離されたパスに追加して、競合を回避します。
- すべてのカーゴ ステップでは、保持するリポジトリ ポリシーを尊重するために `--locked` を使用します。
  `Cargo.lock` は手付かずです。クレートを追加するよりも、既存のクレートを拡張することを好みます
  新しいワークスペースメンバー。新しいクレートを導入する前に承認を求めてください。

## ガードレール- `make check-agents-guardrails` (または `ci/check_agents_guardrails.sh`) は、
  ブランチは `Cargo.lock` を変更し、新しいワークスペース メンバーを導入するか、新しいワークスペース メンバーを追加します
  依存関係。スクリプトは作業ツリーと `HEAD` を比較します。
  デフォルトでは `origin/main`。 `AGENTS_BASE_REF=<ref>` を設定してベースをオーバーライドします。
- `make check-dependency-discipline` (または `ci/check_dependency_discipline.sh`)
  `Cargo.toml` の依存関係をベースと比較し、新しいクレートでは失敗します。セット
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` 意図的であることを認識する
  追加。
- `make check-missing-docs` (または `ci/check_missing_docs_guard.sh`) は新しいブロックをブロックします
  `#[allow(missing_docs)]` エントリ、クレートに触れたフラグ (最も近い `Cargo.toml`)
  その `src/lib.rs`/`src/main.rs` にはクレートレベルの `//!` ドキュメントが不足しており、新しいドキュメントは拒否されます
  基本参照に関連する `///` ドキュメントのない公開アイテム。セット
  `MISSING_DOCS_GUARD_ALLOW=1` はレビュー担当者の承認がある場合のみ。警備員も
  `docs/source/agents/missing_docs_inventory.{json,md}` が新しいことを確認します。
  `python3 scripts/inventory_missing_docs.py` で再生成します。
- `make check-tests-guard` (または `ci/check_tests_guard.sh`) は、クレートにフラグを立てます。
  変更された Rust 関数には単体テストの証拠がありません。ガードマップのラインが変更されました
  関数に渡し、diff でクレートテストが変更された場合は渡し、そうでない場合はスキャンします
  関数呼び出しを照合するための既存のテスト ファイルなので、既存のカバレッジ
  数を数えます。一致するテストがないクレートは失敗します。 `TEST_GUARD_ALLOW=1` を設定します
  変更が真にテスト中立であり、レビュー担当者が同意した場合に限ります。
- `make check-docs-tests-metrics` (または `ci/check_docs_tests_metrics_guard.sh`)
  マイルストーンはドキュメントと並行して移動するというロードマップ ポリシーを強制します。
  テスト、メトリクス/ダッシュボード。 `roadmap.md` が相対的に変化するとき
  `AGENTS_BASE_REF`、ガードは少なくとも 1 つのドキュメント変更、1 つのテスト変更を期待しています。
  メトリクス/テレメトリ/ダッシュボードの変更が 1 つあります。 `DOC_TEST_METRIC_GUARD_ALLOW=1` を設定します
  査読者の承認がある場合のみ。
- TODO マーカーがある場合、`make check-todo-guard` (または `ci/check_todo_guard.sh`) が失敗する
  ドキュメント/テストの変更を伴うことなく消えます。カバレッジを追加または更新する
  TODO を解決する場合、または意図的な削除の場合は `TODO_GUARD_ALLOW=1` を設定します。
- `make check-std-only` (または `ci/check_std_only.sh`) は `no_std`/`wasm32` をブロックします
  cfgs を使用すると、ワークスペースは `std` のみのままになります。 `STD_ONLY_GUARD_ALLOW=1` のみを設定します
  認可されたCI実験。
- `make check-status-sync` (または `ci/check_status_sync.sh`) はロードマップを開いたままにします
  セクションには完了したアイテムが含まれておらず、`roadmap.md`/`status.md` が必要です
  一緒に変更して計画とステータスを一致させます。セット
  `STATUS_SYNC_ALLOW_UNPAIRED=1` は、まれなステータスのみのタイプミスの修正のみです。
  `AGENTS_BASE_REF` を固定します。
- `make check-proc-macro-ui` (または `ci/check_proc_macro_ui.sh`) は trybuild を実行します
  派生/proc-マクロ クレート用の UI スイート。 proc-マクロに触れたときに実行します。
  `.stderr` 診断を安定に保ち、パニックを引き起こす UI 回帰を捕捉します。セット
  特定のクレートに焦点を当てる `PROC_MACRO_UI_CRATES="crate1 crate2"`。
- `make check-env-config-surface` (または `ci/check_env_config_surface.sh`) のリビルド
  env-toggle インベントリ (`docs/source/agents/env_var_inventory.{json,md}`)、
  古い場合は失敗し、**および** 新しい運用環境シムが表示されると失敗します
  `AGENTS_BASE_REF` を基準とした値 (自動検出; 必要に応じて明示的に設定)。
  環境ルックアップを追加/削除した後、トラッカーを更新します。
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md`;
  `ENV_CONFIG_GUARD_ALLOW=1` は、意図的な環境ノブを文書化した後にのみ使用してください。移行トラッカーで。
- `make check-serde-guard` (または `ci/check_serde_guard.sh`) は Serde を再生成します
  使用状況インベントリ (`docs/source/norito_json_inventory.{json,md}`) を一時ファイルに保存
  コミットされたインベントリが古い場合は失敗し、新しいインベントリは拒否されます。
  実動 `serde`/`serde_json` は、`AGENTS_BASE_REF` と比較してヒットします。セット
  `SERDE_GUARD_ALLOW=1` は、移行計画を提出した後の CI 実験のみです。
- `make guards` は、Norito シリアル化ポリシーを強制します。新しいシリアル化を拒否します。
  `serde`/`serde_json` の使用法、アドホック AoS ヘルパー、および外部の SCALE 依存関係
  Norito ベンチ (`scripts/deny_serde_json.sh`、
  `scripts/check_no_direct_serde.sh`、`scripts/deny_handrolled_aos.sh`、
  `scripts/check_no_scale.sh`)。
- **Proc-macro UI ポリシー:** すべての proc-macro クレートは `trybuild` を出荷する必要があります
  `trybuild-tests` の背後にあるハーネス (`tests/ui.rs`、合格/失敗グロブ付き)
  特徴。ハッピーパス サンプルは `tests/ui/pass` の下に、拒否ケースは以下に配置します。
  `tests/ui/fail` とコミットされた `.stderr` 出力、および診断を保持
  パニックにならず、安定しています。フィクスチャを更新します
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (オプションで
  `CARGO_TARGET_DIR=target-codex` (既存のビルドの破壊を避けるため)、および
  カバレッジ ビルドに依存することは避けてください (`cfg(not(coverage))` ガードが期待されます)。
  バイナリ エントリポイントを発行しないマクロの場合は、
  エラーに焦点を当て続けるためのフィクスチャ内の `// compile-flags: --crate-type lib`。追加
  診断が変わるたびに新たな陰性症例が発生します。
- CI は `.github/workflows/agents-guardrails.yml` 経由でガードレール スクリプトを実行します
  そのため、ポリシーに違反するとプル リクエストはすぐに失敗します。
- サンプル git フック (`hooks/pre-commit.sample`) は、ガードレール、依存関係、
  missing-docs、std-only、env-config、および status-sync スクリプトがないため、貢献者
  CI の前にポリシー違反を捕捉します。意図的な場合は TODO のパンくずリストを保持します
  大きな変更を黙って延期するのではなく、フォローアップを行います。