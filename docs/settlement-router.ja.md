---
lang: ja
direction: ltr
source: docs/settlement-router.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 782429c90ac5df034fd7c8ff2c3acf4f9f11348f14f15fcd321f343b22b154b8
source_last_modified: "2025-11-22T12:03:21.814470+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/settlement-router.md -->

# 決定論的決済ルーター (NX-3)

**ステータス:** 完了 (NX-3)  
**オーナー:** Economics WG / Core Ledger WG / Treasury / SRE  
**スコープ:** すべての lane/dataspace が利用する正規の XOR 決済パス。ルーター crate、
lane レベルのレシート、バッファのガードレール、テレメトリ、運用証跡の
サーフェスを提供する。

## 目標
- 単一 lane と Nexus ビルド間で XOR 変換とレシート生成を統一する。
- 決定論的なヘアカットとボラティリティマージンを、ガードレール付きバッファで
  適用し、オペレーターが安全に決済を調整できるようにする。
- 監査者が専用ツールなしで再生できるレシート、テレメトリ、ダッシュボードを公開する。

## アーキテクチャ
| コンポーネント | 位置 | 役割 |
|-----------|----------|----------------|
| ルーターのプリミティブ | `crates/settlement_router/` | シャドープライス計算、ヘアカット階層、バッファポリシー補助、決済レシート型。【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】【crates/settlement_router/src/policy.rs:1】 |
| ランタイム・ファサード | `crates/iroha_core/src/settlement/mod.rs:1` | ルーター設定を `SettlementEngine` に包み、ブロック実行で使う `quote` と accumulator を公開。 |
| ブロック統合 | `crates/iroha_core/src/block.rs:120` | `PendingSettlement` を排出し、`LaneSettlementCommitment` を lane/dataspace ごとに集約、lane のバッファメタデータを解析しテレメトリを出力。 |
| テレメトリとダッシュボード | `crates/iroha_telemetry/src/metrics.rs:4847`, `dashboards/grafana/settlement_router_overview.json:1` | バッファ、分散、ヘアカット、変換回数の Prometheus/OTLP メトリクス。SRE 向け Grafana ボード。 |
| 参照スキーマ | `docs/source/nexus_fee_model.md:1` | `LaneBlockCommitment` に保存される決済レシート項目を文書化。 |

## 設定
ルーター設定は `[settlement.router]` にあり、`iroha_config` が検証します。

```toml
[settlement.router]
twap_window_seconds = 60      # TWAP window used to derive local→XOR conversions
epsilon_bps = 25              # Base margin added to every quote (basis points)
buffer_alert_pct = 75         # Remaining-buffer % that opens an alert
buffer_throttle_pct = 25      # Remaining-buffer % where throttling begins
buffer_xor_only_pct = 10      # Remaining-buffer % where XOR-only mode is enforced
buffer_halt_pct = 2           # Remaining-buffer % where settlement halts
buffer_horizon_hours = 72     # Horizon (hours) represented by the XOR buffer
```

lane メタデータは dataspace ごとのバッファアカウントを紐付けます。
- `settlement.buffer_account` — リザーブを保持するアカウント（例: `buffer::cbdc_treasury`）。
- `settlement.buffer_asset` — ヘッドルーム控除に使う資産定義（通常は `xor#sora`）。
- `settlement.buffer_capacity_micro` — マイクロ XOR で指定する容量（10 進文字列）。

メタデータが無い場合、その lane のバッファスナップショットは無効になり、
テレメトリは容量/状態のゼロ値にフォールバックします。

## 変換パイプライン
1. **Quote:** `SettlementEngine::quote` は TWAP クォートに対して
   設定された epsilon とボラティリティマージン、ヘアカット階層を適用し、
   `xor_due` と `xor_after_haircut`、タイムスタンプ、呼び出し側の `source_id` を含む
   `SettlementReceipt` を返します。【crates/settlement_router/src/price.rs:1】【crates/settlement_router/src/haircut.rs:1】
2. **Accumulate:** ブロック実行中に executor が `PendingSettlement` を記録します
   （ローカル金額、TWAP、epsilon、ボラティリティバケット、流動性プロファイル、
   オラクルタイムスタンプ）。`LaneSettlementBuilder` はブロック封印前に
   `(lane, dataspace)` 単位で合計と swap メタデータを集約します。
   【crates/iroha_core/src/settlement/mod.rs:34】【crates/iroha_core/src/block.rs:3460】
3. **バッファスナップショット:** lane メタデータにバッファが宣言されている場合、
   `BufferPolicy` のしきい値を使い `SettlementBufferSnapshot`
   （残余ヘッドルーム、容量、状態）を取得します。【crates/iroha_core/src/block.rs:203】
4. **Commit + テレメトリ:** レシートと swap 証跡は `LaneBlockCommitment` に格納され、
   ステータススナップショットにも反映されます。テレメトリはバッファゲージ、
   分散（`iroha_settlement_pnl_xor`）、適用マージン（`iroha_settlement_haircut_bp`）、
   オプションの swapline 利用率、資産ごとの変換/ヘアカットカウンタを記録し、
   ダッシュボードとアラートがブロック内容と同期するようにします。
   【crates/iroha_core/src/block.rs:298】【crates/iroha_core/src/telemetry.rs:844】
5. **証跡サーフェス:** `status::set_lane_settlement_commitments` がリレー/DA 消費者向けに
   コミットメントを公開し、Grafana ダッシュボードは Prometheus メトリクスを参照。
   オペレーターは `ops/runbooks/settlement-buffers.md` と
   `dashboards/grafana/settlement_router_overview.json` を使って補充/抑制イベントを監視します。

## テレメトリと証跡
- `iroha_settlement_buffer_xor`, `iroha_settlement_buffer_capacity_xor`, `iroha_settlement_buffer_status` — lane/dataspace ごとのバッファスナップショット（マイクロ XOR + エンコード状態）。【crates/iroha_telemetry/src/metrics.rs:6212】
- `iroha_settlement_pnl_xor` — バッチ内の決済差分（ヘアカット前後の XOR 差分）。【crates/iroha_telemetry/src/metrics.rs:6236】
- `iroha_settlement_haircut_bp` — バッチに適用された実効 epsilon/ヘアカットのベーシスポイント。【crates/iroha_telemetry/src/metrics.rs:6244】
- `iroha_settlement_swapline_utilisation` — swap 証跡がある場合の流動性プロファイル別利用率。【crates/iroha_telemetry/src/metrics.rs:6252】
- `settlement_router_conversion_total` / `settlement_router_haircut_total` — lane/dataspace ごとの決済変換回数と累積ヘアカット（XOR 単位）のカウンタ。【crates/iroha_telemetry/src/metrics.rs:6260】【crates/iroha_core/src/block.rs:304】
- Grafana ボード: `dashboards/grafana/settlement_router_overview.json`（バッファ余力、分散、ヘアカット）と、Nexus lane アラートパックに埋め込まれた Alertmanager ルール。
- オペレーター向けランブック: `ops/runbooks/settlement-buffers.md`（補充/アラート運用）と `docs/source/nexus_settlement_faq.md` の FAQ。

## 開発者 & SRE チェックリスト
- `[settlement.router]` の値を `config/config.json5`（または TOML）に設定し、`irohad --version` のログで検証する。しきい値が `alert > throttle > xor_only > halt` を満たすことを確認する。
- lane メタデータにバッファのアカウント/資産/容量を設定し、バッファゲージが実際のリザーブを反映するようにする。バッファを追跡しない lane ではフィールドを省略する。
- `dashboards/grafana/settlement_router_overview.json` で `settlement_router_*` と `iroha_settlement_*` を監視し、throttle/XOR-only/halt 状態でアラートを発報する。
- 価格/ポリシーのカバレッジには `cargo test -p settlement_router` を実行し、`crates/iroha_core/src/block.rs` のブロック集約テストも確認する。
- 設定変更のガバナンス承認を `docs/source/nexus_fee_model.md` に記録し、しきい値やテレメトリサーフェスを変更したら `status.md` を更新する。

## ロールアウト計画スナップショット
- ルーターとテレメトリはすべてのビルドに含まれ、機能ゲートはありません。lane メタデータがバッファスナップショットの公開可否を制御します。
- デフォルト設定はロードマップ値（60 秒 TWAP、25 bp の基本 epsilon、72 時間のバッファ地平線）に一致します。設定で調整し、`irohad` を再起動して反映します。
- 証跡バンドル = lane の決済コミットメント + `settlement_router_*`/`iroha_settlement_*` 系列の Prometheus スクレイプ + 該当期間の Grafana スクリーンショット/JSON エクスポート。

## 証跡と参照
- NX-3 決済ルーターの受け入れノート: `status.md`（NX-3 セクション）。
- オペレーターサーフェス: `dashboards/grafana/settlement_router_overview.json`, `ops/runbooks/settlement-buffers.md`.
- レシートスキーマと API サーフェス: `docs/source/nexus_fee_model.md`, `/v1/sumeragi/status` -> `lane_settlement_commitments`.
