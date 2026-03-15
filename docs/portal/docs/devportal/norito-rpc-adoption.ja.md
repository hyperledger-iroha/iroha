---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 39cbd5e448c8a868c50401c15466d7159cb08ad7be52dafb9e9dc66d5bba979d
source_last_modified: "2025-11-17T18:34:13.669847+00:00"
translation_last_reviewed: 2026-01-01
---

# Norito-RPC 採用スケジュール

> 正規の計画メモは `docs/source/torii/norito_rpc_adoption_schedule.md` にあります。  
> このポータル版は、SDK 作成者、オペレーター、レビュー担当向けの展開期待値を要約しています。

## 目的

- AND4 の本番切替前に、すべての SDK (Rust CLI, Python, JavaScript, Swift, Android) をバイナリ Norito-RPC トランスポートに揃える。
- フェーズゲート、証拠バンドル、テレメトリーフックを決定的に保ち、ガバナンスが展開を監査できるようにする。
- ロードマップ NRPC-4 が挙げる共有ヘルパーで、fixture と canary の証拠収集を容易にする。

## フェーズタイムライン

| フェーズ | 期間 | 範囲 | 完了条件 |
|----------|------|------|----------|
| **P0 - ラボパリティ** | Q2 2025 | Rust CLI + Python の smoke スイートが CI で `/v1/norito-rpc` を実行し、JS ヘルパーが単体テストを通過し、Android の mock harness が二重トランスポートを検証する。 | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` と `javascript/iroha_js/test/noritoRpcClient.test.js` が CI で green、Android harness が `./gradlew test` に接続されている。 |
| **P1 - SDK プレビュー** | Q3 2025 | 共有 fixture バンドルがチェックインされ、`scripts/run_norito_rpc_fixtures.sh --sdk <label>` が `artifacts/norito_rpc/` にログ + JSON を記録し、SDK サンプルで Norito トランスポートのオプションフラグが公開される。 | fixture マニフェストに署名、README 更新で opt-in 利用を明示、Swift のプレビュー API が IOS2 フラグの背後で利用可能。 |
| **P2 - Staging / AND4 プレビュー** | Q1 2026 | staging の Torii プールは Norito を優先し、Android AND4 preview クライアントと Swift IOS2 パリティスイートはバイナリトランスポートを既定にし、テレメトリーダッシュボード `dashboards/grafana/torii_norito_rpc_observability.json` が埋まる。 | `docs/source/torii/norito_rpc_stage_reports.md` が canary を記録し、`scripts/telemetry/test_torii_norito_rpc_alerts.sh` が通過し、Android mock harness のリプレイが成功/失敗ケースを捕捉する。 |
| **P3 - 本番 GA** | Q4 2026 | Norito が全 SDK の既定トランスポートになる。JSON は brownout の fallback のまま。リリースジョブは各タグでパリティ成果物をアーカイブする。 | リリースチェックリストが Rust/JS/Python/Swift/Android の Norito smoke 出力を束ねる。Norito vs JSON のエラーレート SLO のアラート閾値が適用され、`status.md` とリリースノートが GA 証拠を引用する。 |

## SDK 成果物と CI フック

- **Rust CLI と統合ハーネス** - `iroha_cli pipeline` の smoke テストを拡張し、`cargo xtask norito-rpc-verify` が利用可能になったら Norito トランスポートを強制する。`cargo test -p integration_tests -- norito_streaming` (lab) と `cargo xtask norito-rpc-verify` (staging/GA) でガードし、成果物を `artifacts/norito_rpc/` に保存する。
- **Python SDK** - リリース smoke (`python/iroha_python/scripts/release_smoke.sh`) を Norito RPC デフォルトにし、`run_norito_rpc_smoke.sh` を CI エントリとして維持し、`python/iroha_python/README.md` にパリティ対応を記載する。CI ターゲット: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`。
- **JavaScript SDK** - `NoritoRpcClient` を安定化し、`toriiClientConfig.transport.preferred === "norito_rpc"` のとき governance/query ヘルパーが Norito を既定にするようにし、`javascript/iroha_js/recipes/` に end-to-end サンプルを記録する。CI は `npm test` と docker 化された `npm run test:norito-rpc` を公開前に実行すること。provenance は Norito smoke ログを `javascript/iroha_js/artifacts/` にアップロードする。
- **Swift SDK** - Norito bridge トランスポートを IOS2 フラグの背後に接続し、fixture の cadence を合わせ、`docs/source/sdk/swift/index.md` に記載された Buildkite レーンで Connect/Norito パリティスイートが走ることを保証する。
- **Android SDK** - AND4 preview クライアントと Torii mock ハーネスが Norito を採用し、retry/backoff テレメトリは `docs/source/sdk/android/networking.md` に記載する。ハーネスは `scripts/run_norito_rpc_fixtures.sh --sdk android` 経由で他 SDK と fixtures を共有する。

## エビデンスと自動化

- `scripts/run_norito_rpc_fixtures.sh` は `cargo xtask norito-rpc-verify` をラップし、stdout/stderr を収集して `fixtures.<sdk>.summary.json` を出力する。SDK オーナーが `status.md` に添付できる決定的な成果物として活用する。`--sdk <label>` と `--out artifacts/norito_rpc/<stamp>/` を使って CI バンドルを整頓する。
- `cargo xtask norito-rpc-verify` はスキーマハッシュの整合性 (`fixtures/norito_rpc/schema_hashes.json`) を強制し、Torii が `X-Iroha-Error-Code: schema_mismatch` を返した場合は失敗させる。各失敗にはデバッグ用の JSON fallback キャプチャを添える。
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` と `dashboards/grafana/torii_norito_rpc_observability.json` は NRPC-2 のアラート契約を定義する。ダッシュボード更新のたびにスクリプトを実行し、`promtool` の出力を canary バンドルに保存する。
- `docs/source/runbooks/torii_norito_rpc_canary.md` は staging と本番のドリルを説明する。fixture ハッシュやアラートゲートが変わったら更新する。

## レビュアーチェックリスト

NRPC-4 のマイルストーンにチェックを付ける前に、次を確認する:

1. 最新の fixture バンドルのハッシュが `fixtures/norito_rpc/schema_hashes.json` と一致し、対応する CI 成果物が `artifacts/norito_rpc/<stamp>/` に記録されている。
2. SDK の README / ポータルドキュメントが JSON fallback の強制方法を説明し、Norito トランスポートが既定であることを明記している。
3. テレメトリーダッシュボードに dual-stack のエラーレートパネルとアラートリンクがあり、Alertmanager の dry run (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) がトラッカーに添付されている。
4. ここでの採用スケジュールがトラッカー (`docs/source/torii/norito_rpc_tracker.md`) と一致し、ロードマップ (NRPC-4) が同じ証拠バンドルを参照している。

スケジュールを厳守することで SDK 間の挙動が予測可能になり、ガバナンスが個別依頼なしで Norito-RPC の採用を監査できるようになります。
