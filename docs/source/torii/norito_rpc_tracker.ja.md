---
lang: ja
direction: ltr
source: docs/source/torii/norito_rpc_tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ddc611ab0c68b0b4c5412056d5d93b966a2a99d7657516f7e49c05753f13f331
source_last_modified: "2026-04-22T09:12:55.053896+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/torii/norito_rpc_tracker.md -->

## Norito-RPC アクショントラッカー

| ID | 説明 | 担当 | ステータス | 目標 | 備考 |
|----|-------------|----------|--------|--------|-------|
| NRPC-2A | dual-stack, mTLS, fallback ガードを含む Torii ingress/auth ロールアウト計画のドラフト | Torii Platform TL, NetOps | 🈴 完了 | 2026-03-21 | `docs/source/torii/norito_rpc_rollout_plan.md` のセクション 3 に ingress ポリシー、mTLS 要件、レートリミット整合、エラーハンドリングを記載。 |
| NRPC-2B | テレメトリ + アラート更新（Grafana, Alertmanager, chaos scripts）定義 | Observability liaison | 🈴 完了 | 2026-03-19 | テレメトリ仕様 + アラート一式を公開（`docs/source/torii/norito_rpc_telemetry.md`, `dashboards/grafana/torii_norito_rpc_observability.json`, `dashboards/alerts/torii_norito_rpc_rules.yml`, `scripts/telemetry/test_torii_norito_rpc_alerts.sh`）。 |
| NRPC-2C | ロールアウトチェックリスト（stage, canary, GA, rollback）作成 | Platform Ops | 🈴 完了 | 2026-03-21 | `docs/source/torii/norito_rpc_rollout_plan.md` のセクション 5 と 8 が段階チェックリストとロールバック早見を公開。 |
| NRPC-2R | dual-stack 準備レビュー（仕様固定 + ロールアウトノブ） | Torii Platform TL, SDK Program Lead, Android Networking TL | 🈴 完了 | 2025-06-19 | 2025-06-19 にレビュー実施。仕様 + フィクスチャ固定（`nrpc_spec.md` SHA-256 `0bb9d2c225b5485fd0b7c6ef28a3ecea663afea76b09a248701d0b50c25982b1`, `fixtures/norito_rpc/schema_hashes.json` SHA-256 `343f4c7991e6bcfbda894b3b2422a0241def279fc79db7563da618d31763ba1c`）。Canary ルール: `transport.norito_rpc.stage=canary` で mTLS + `allowed_clients` 必須、JSON はロールバック用に保持、テレメトリゲートは `dashboards/grafana/torii_norito_rpc_observability.json`。議事録は `docs/source/torii/norito_rpc_sync_notes.md`。 |
| NRPC-3A | JavaScript SDK の Norito-RPC ヘルパー + テスト | JS SDK Lead | 🈴 完了 | 2026-04-15 | SDK に `NoritoRpcClient` + `NoritoRpcError` が同梱され、TypeScript 定義・ドキュメント・ヘッダー/パラメータ/リトライ/タイムアウトのユニットテストを追加（`javascript/iroha_js/src/noritoRpcClient.js`,`javascript/iroha_js/test/noritoRpcClient.test.js`,`javascript/iroha_js/index.d.ts`,`javascript/iroha_js/README.md:342`）。 |
| NRPC-3B | Swift SDK の Norito transport ヘルパー | Swift SDK Lead | 🈴 完了 | 2026-04-17 | `NoritoRpcClient` が `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift` に実装され、回帰テスト（`IrohaSwift/Tests/IrohaSwiftTests/NoritoRpcClientTests.swift`）と docs/README 更新を実施。Swift が JS helper と揃った。 |
| NRPC-3C | クロス SDK の fixture cadence 合意 | SDK Program Lead | 🈴 完了 | 2026-04-19 | 毎週水曜 17:00 UTC のローテーションと手順が `docs/source/torii/norito_rpc_fixture_cadence.md` に記録。`scripts/run_norito_rpc_fixtures.sh`、`artifacts/norito_rpc/<stamp>/` のパス、ステータス記録期待値を提示。 |
| NRPC-4 | クロス SDK 採用スケジュール & チェックリスト公開 | SDK Program Lead / Swift & JS Leads | 🈴 完了 | 2026-03-22 | `docs/source/torii/norito_rpc_adoption_schedule.md` が段階タイムライン、SDK 成果物、fixture 計画、NRPC GA と AND4 の整合に必要な telemetry/reporting hooks を記載。 |
| DOC-NRPC | 開発者ポータル更新と Try‑It コンソールサンプル | Docs/DevRel | 🈴 完了 | 2026-03-25 | ポータル docs が Norito payload のエンドツーエンドをカバー（`docs/portal/docs/devportal/try-it.md`,`docs/portal/docs/norito/try-it-console.md`）。`TRYIT_PROXY_CLIENT_ID` → `X-TryIt-Client` のプロキシタグを `tryit-proxy-lib.mjs` テストで配線/検証。ブラウザの “Try it” は `application/x-norito` fixtures を利用し、署名マニフェストガードを維持。 |
| OPS-NRPC | オペレーター FAQ 更新（status.md, リリースノート） | DevRel, Ops | 🈴 完了 | 2026-03-29 | オペレーター FAQ の §5 に、公開用リリースノート文面と公開手順を追加（`docs/source/runbooks/torii_norito_rpc_faq.md:97`–`116`）。当番が ad-hoc の文案なしで発表できる。 |
| QA-NRPC | Norito vs JSON パリティのスモークテスト拡張 | QA Guild | 🈴 完了 | 2026-04-22 | `cargo xtask norito-rpc-verify` が JSON と Norito payload の両方でエイリアスエンドポイントを検証し、ロールアウト前の退行を検出。【xtask/src/main.rs:492】【xtask/src/main.rs:3517】 |
| NRPC-4F1 | fixture cadence & 証拠の自動化 | SDK Program Lead / Android Networking TL | 🈴 完了 | 毎週 (水 17:00 UTC) | cadence ラッパーが自動ローテーションサマリを出力（`--auto-report` → `artifacts/norito_rpc/rotation_status.{json,md}` で 7 日鮮度）し、各実行のログ/JSON/xtask 出力も生成。担当は週次で交代し、`artifacts/norito_rpc/` パスは `status.md` とガバナンスパケット向けに維持。 |

### 更新 cadence
- Torii プラットフォーム同期で毎週レビュー。
- トラッカー担当者は火曜の dry run 会議前にステータス更新。
