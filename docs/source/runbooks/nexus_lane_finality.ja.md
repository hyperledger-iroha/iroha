---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: ja
direction: ltr
source: docs/source/runbooks/nexus_lane_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1d9fbcf5301bd36a0bdc8a010ae214f7b1561373237054ab3c8a45a411cf6c0e
source_last_modified: "2025-12-14T09:53:36.241505+00:00"
translation_last_reviewed: 2025-12-28
---

# Nexus レーン最終化・オラクル運用ランブック

**状態:** Active — NX-18 のダッシュボード/ランブック成果物を満たします。  
**対象:** Core Consensus WG、SRE/Telemetry、Release Engineering、オンコールリード。  
**範囲:** 1 s 最終化の約束を守るためのスロット時間、DA クォーラム、オラクル、
決済バッファの SLO を扱います。`dashboards/grafana/nexus_lanes.json` と
`scripts/telemetry/` 配下のテレメトリヘルパーを併用してください。

## ダッシュボード

- **Grafana (`dashboards/grafana/nexus_lanes.json`)** — “Nexus Lane Finality & Oracles” を公開。パネルは以下を追跡します。
  - `histogram_quantile()` による `iroha_slot_duration_ms`（p50/p95/p99）と最新サンプルのゲージ。
  - `iroha_da_quorum_ratio` と `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` による DA churn。
  - オラクル関連: `iroha_oracle_price_local_per_xor`, `iroha_oracle_staleness_seconds`, `iroha_oracle_twap_window_seconds`, `iroha_oracle_haircut_basis_points`。
  - 決済バッファパネル（`iroha_settlement_buffer_xor`）は `LaneBlockCommitment` レシート由来のレーン別デビットを表示。
- **アラートルール** — `ans3.md` の Slot/DA SLO 条項を再利用。以下でページング:
  - スロット時間 p95 > 1000 ms が 5 m ウィンドウで 2 回連続、
  - DA クォーラム比 < 0.95 または `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m]) > 0`、
  - オラクル staleness > 90 s または TWAP 窓 ≠ 60 s、
  - 決済バッファ < 25 %（soft）/ 10 %（hard）※メトリクス有効化後。

## メトリクス早見表

| メトリクス | 目標 / アラート | メモ |
|--------|----------------|-------|
| `histogram_quantile(0.95, iroha_slot_duration_ms)` | ≤ 1000 ms（hard）、950 ms warning | ダッシュボードパネル、または `scripts/telemetry/check_slot_duration.py`（`--json-out artifacts/nx18/slot_summary.json`）を chaos 実行時の Prometheus export に対して実行。 |
| `iroha_slot_duration_ms_latest` | 最新スロット。quantile が正常でも > 1100 ms なら調査。 | インシデント起票時に値を添付。 |
| `iroha_da_quorum_ratio` | 30 m 移動窓で ≥ 0.95。 | ブロックコミット中の DA reschedule から派生。 |
| `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` | chaos リハ以外では 0 を維持。 | 持続的な増加は `missing-availability warning` と扱う。 |

各 reschedule は `kind = "missing-availability warning"` の Torii pipeline warning も発火します。メトリクスのスパイクと一緒にイベントを回収し、該当ブロックヘッダ、リトライ回数、requeue カウンタをバリデータログを追わずに特定してください。【crates/iroha_core/src/sumeragi/main_loop.rs:5164】
| `iroha_oracle_staleness_seconds` | ≤ 60 s。75 s でアラート。 | 60 s TWAP フィードの陳腐化。 |
| `iroha_oracle_twap_window_seconds` | 60 s ± 5 s。 | 逸脱はオラクル設定ミス。 |
| `iroha_oracle_haircut_basis_points` | レーンの流動性 tier（0/25/75 bps）に一致。 | 想定外の上昇はエスカレーション。 |
| `iroha_settlement_buffer_xor` | soft 25 %、hard 10 %。10 % 未満で XOR‑only を強制。 | パネルはレーン/データスペースごとの micro‑XOR デビットを表示。ポリシー変更前にエクスポート。 |

## 対応プレイブック

### スロット時間の逸脱
1. ダッシュボード + `promql`（p95/p99）で確認。  
2. `scripts/telemetry/check_slot_duration.py --json-out <path>` の出力（とメトリクススナップショット）を取得し、CXO レビューで 1 s ゲートを検証可能にする。  
3. RCA の入力を確認: mempool 深度、DA reschedule、IVM トレース。  
4. インシデントを起票し、Grafana スクリーンショットを添付。再発時は chaos ドリルを計画。

### DA クォーラム低下
1. `iroha_da_quorum_ratio` と reschedule カウンタを確認し、`missing-availability warning` ログと突合。  
2. ratio <0.95 の場合、失敗 attester を特定、サンプリングを拡大、または XOR‑only モードへ。  
3. `scripts/telemetry/check_nexus_audit_outcome.py` を routed‑trace リハで実行し、`nexus.audit.outcome` が回復後も通ることを証明。  
4. DA レシート bundle をインシデントチケットに添付。

### オラクル staleness / haircut 乖離
1. パネル 5–8 で価格・staleness・TWAP 窓・haircut を確認。  
2. staleness >90 s: オラクルフィードを再起動/フェイルオーバーし、chaos harness を再実行。  
3. haircut mismatch: 流動性プロファイルと直近のガバナンス変更を確認し、必要なら treasury に通知。

### 決済バッファアラート
1. `iroha_settlement_buffer_xor`（＋夜間レシート）でヘッドルームを確認してからルーターポリシーを変更。  
2. 閾値越え時の対応:
   - **soft breach (<25 %)**: treasury を招集し、swap lines の活用検討とアラート記録。  
   - **hard breach (<10 %)**: XOR‑only を強制し、補助付きレーンを拒否、`ops/drill-log.md` に記録。  
3. `docs/source/settlement_router.md` の repo/reverse‑repo レバーを参照。

## エビデンスと自動化

- **CI** — `scripts/telemetry/check_slot_duration.py --json-out artifacts/nx18/slot_summary.json` と
  `scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` を RC 受け入れワークフローへ接続し、各 RC がスロット時間サマリと DA/オラクル/バッファのゲート結果をメトリクススナップショットと共に出力するようにします。ヘルパーは `ci/check_nexus_lane_smoke.sh` から呼ばれています。  
- **ダッシュボード整合性** — `scripts/telemetry/compare_dashboards.py dashboards/grafana/nexus_lanes.json <prod-export.json>` を実行し、公開ボードが staging/prod export と一致することを確認。  
- **トレースアーカイブ** — TRACE リハや NX-18 chaos ドリルでは `scripts/telemetry/check_nexus_audit_outcome.py` を実行し、最新の `nexus.audit.outcome` payload（`docs/examples/nexus_audit_outcomes/`）を保存。Grafana スクショとともに drill log に添付。
- **スロット証跡のバンドル** — summary JSON 生成後に `scripts/telemetry/bundle_slot_artifacts.py --metrics <prometheus.tgz-extract>/metrics.prom --summary artifacts/nx18/slot_summary.json --out-dir artifacts/nx18` を実行し、`slot_bundle_manifest.json` に 2 つのアーティファクトの SHA‑256 digest を記録。RC 証跡バンドルとしてディレクトリをそのままアップロード。リリースパイプラインは自動実行（`--skip-nexus-lane-smoke` でスキップ可）し、`artifacts/nx18/` をリリース出力へコピーします。

## メンテナンスチェックリスト

- `dashboards/grafana/nexus_lanes.json` を Grafana export と同期し続け、スキーマ変更後は NX-18 を参照したコミットメッセージで更新を記録。  
- 新メトリクス（例: 決済バッファ gauge）や閾値の変更が入ったら本ランブックも更新。  
- chaos リハ（スロット遅延、DA jitter、オラクル停止、バッファ枯渇）は `scripts/telemetry/log_sorafs_drill.sh --log ops/drill-log.md --program NX-18 --status <status>` で記録。

このランブックに従うことで NX-18 が要求する “operator dashboards/runbooks” の証跡が整い、Nexus GA 前に finality SLO を検証できます。
