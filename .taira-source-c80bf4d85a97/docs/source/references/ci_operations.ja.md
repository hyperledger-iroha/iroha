<!-- Japanese translation of docs/source/references/ci_operations.md -->

---
lang: ja
direction: ltr
source: docs/source/references/ci_operations.md
status: complete
translator: manual
---

# CI 運用 — Nightly Red-Team レーン

## 概要

- **ワークフロー:** `.github/workflows/nightly-red-team.yml`
- **スケジュール:** 毎日 UTC 03:30（GitHub Actions の cron）。`workflow_dispatch` による手動再実行も可能。
- **実行対象:**  
  - コンセンサスリレー検証ハーネス — `cargo test -p integration_tests extra_functional::unstable_network::block_after_genesis_is_synced -- --nocapture`
  - コントラクトマニフェスト Torii パス — `cargo test -p integration_tests contracts::post_and_get_contract_manifest_via_torii -- --nocapture`
  - Torii レート制限ガード — `cargo test -p iroha_torii handshake_rate_limited -- --nocapture`
- **成果物:** 各ワークロードのログを `artifacts/red-team/` に書き出し、`red-team-logs` としてアップロード。
- **失敗時処理:** `scripts/report_red_team_failures.py` がログを収集し、テンプレート化した Issue 本文（対象サーフェス、緩和策、担当者、期限）を生成。いずれかのワークロードが失敗すると Issue を自動登録。

## SLA

- **トリアージ猶予:** 24 時間以内。担当者の割り当て、Issue の確認、緩和策の着手を 1 日以内に行う。
- **担当者ヒント:** リポジトリシークレット `RED_TEAM_OWNER` にチームメンション（例: `@sora-org/red-team`）を設定すると、自動生成される Issue に適切な担当が割り当てられる。空の場合はワークフロー内のデフォルト値を使用。

## 再実行手順

1. **Actions → Nightly Red Team** を開く。
2. **Run workflow** をクリックし、対象ブランチ（既定は `main`）を選択して実行。追加入力は不要で、上記コマンドがそのまま使われる。
3. 実行状況を監視し、緩和策が取り込まれたら再実行してレーンがグリーンであることを確認してから Issue をクローズする。

## 失敗時のトリアージチェックリスト

- アップロード済みアーティファクト `red-team-logs`（`consensus.log`、`manifest.log`、`torii-rate-limit.log`）を確認。
- デフォルト担当が不適切な場合は Issue の担当を更新または再割り当て。
- 原因と緩和策を Issue に記録し、関連 PR があればリンクを追加。
- 修正がマージされたらワークフローを手動で再実行し、成功を確認したら Issue をクローズする。

## 関連ダッシュボード

- Swift／モバイル CI の指標は `references/ios_metrics.md` にまとめた専用ダッシュボードに反映されます。赤チームのワークフローと併せて Buildkite で発生した失敗もトリアージし、SDK 間のパリティとリリース準備が維持されるようにしてください。
- Buildkite の XCFramework スモークレーンは `.buildkite/xcframework-smoke.yml` で定義され、`scripts/ci/run_xcframework_smoke.sh` が `mobile_ci` テレメトリを生成します。各実行後に `xcframework-smoke/report` アノテーションを確認し、失敗があればダッシュボード指標と併せて対応してください。また、ハーネスは Buildkite メタデータ `ci/xcframework-smoke:<lane>:device_tag` を記録するため、destination 文字列を解析しなくても対象デバイス（`iphone-sim`、`strongbox` など）を特定できます。
- Android の Norito フィクスチャは `make android-fixtures`（`scripts/android_fixture_regen.sh`）で再生成し、直後に `make android-fixtures-check`（`ci/check_android_fixtures.sh`）を実行してハッシュとマニフェストを検証します。CI レーンでも同じ順序で実行し、再生成した成果物がコミット前に検証されるようにしてください。
- デスクトップ JVM で Android テストを実行する場合、JDK に Ed25519 実装が無いときは BouncyCastle (`org.bouncycastle:bcprov`) を追加してください（ハーネスはスタブを用意していますが、ライブラリを導入するとローカル実行でも CI と同じ挙動になります）。
