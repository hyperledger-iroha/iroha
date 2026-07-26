<!-- Japanese translation for docs/source/nexus_operations.md -->

---
lang: ja
direction: ltr
source: docs/source/nexus_operations.md
status: draft
translator: LLM (Codex)
---

# Nexus 運用ランブック (NX-14)

**ロードマップ:** NX-14 — Nexus ドキュメント & オペレーターランブック  
**ステータス:** 2026-03-24 作成。`docs/source/nexus_overview.md` と `docs/source/sora_nexus_operator_onboarding.md` のオンボーディング手順と整合。  
**想定読者:** ネットワークオペレーター、SRE/オンコール、ガバナンス調整担当。

このランブックは Sora Nexus (Iroha 3) ノードの運用ライフサイクルをまとめます。詳細な仕様 (`docs/source/nexus.md`) やレーン固有ガイド (`docs/source/cbdc_lane_playbook.md` など) を置き換えるものではなく、チェックリスト・テレメトリフック・エビデンス要件を集約したリファレンスです。

## 1. オペレーションライフサイクル

| 段階 | チェックリスト | エビデンス |
|------|----------------|------------|
| **プレフライト** | アーティファクトのハッシュ/署名を検証し、`profile = "iroha3"` を確認し、設定テンプレートを配置。 | `scripts/select_release_profile.py` の出力、チェックサムログ、署名済みマニフェストバンドル。 |
| **カタログ整合** | `[nexus]` のレーン/データスペースカタログ、ルーティングポリシー、DA しきい値を議会配布のマニフェストに合わせる。 | `irohad --sora --config … --trace-config` の出力をチケットに保存。 |
| **スモーク & カットオーバー** | `irohad --sora --config … --trace-config`、CLI スモーク（例: `FindNetworkStatus`）を実行し、テレメトリエンドポイントを検証してから admission を要求。 | スモークテストログ + Alertmanager サイレンスの確認。 |
| **定常運用** | ダッシュボード/アラートを監視し、ガバナンス指示に沿って鍵をローテーションし、設定とランブックをマニフェスト改訂に合わせて保守。 | 四半期レビュー議事録、ダッシュボードスクリーンショット、ローテーションチケット ID。 |

詳細なオンボーディング（鍵交換、ルーティングポリシー例、リリースプロファイル検証）は `docs/source/sora_nexus_operator_onboarding.md` に掲載されています。アーティファクトやスクリプトが変わった場合は必ず参照してください。

## 2. 変更管理とガバナンス連携

1. **リリース更新**
   - `status.md` と `roadmap.md` の告知を追跡。
   - 各リリース PR には `docs/source/sora_nexus_operator_onboarding.md` のチェックリストを添付。
2. **レーンマニフェスト変更**
   - ガバナンスが Space Directory 経由で署名済みマニフェストを配布。
   - オペレーターは署名を検証し、カタログを更新し、`docs/source/project_tracker/nexus_config_deltas/` にアーカイブ。
3. **設定差分**
   - `config/config.toml` への変更は必ずレーン ID とデータスペース別名を記載したチケットが必要。
   - ノードが参加/アップグレードする際は効果的な設定の編集版をチケットへ保存。
4. **ロールバック訓練**
   - 四半期ごとにロールバック（停止→前バンドル復元→再設定→再スモーク）を実施し、結果を `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` に記録。
5. **コンプライアンス承認**
   - プライベート/CBDC レーンで DA ポリシーやテレメトリ匿名化を変更する場合は、`docs/source/cbdc_lane_playbook.md#governance-hand-offs` に沿ってコンプライアンス承認を取得。

## 3. テレメトリと SLO カバレッジ

ダッシュボードとアラートは `dashboards/` で版管理され、`docs/source/nexus_telemetry_remediation_plan.md` に記録されています。オペレーターは以下を必須とします。

- `dashboards/alerts/nexus_audit_rules.yml` と `dashboards/alerts/torii_norito_rpc_rules.yml`（Torii/Norito 伝送向け）を PagerDuty/on-call へ接続。
- 次の Grafana ボードを運用ポータルで共有:
  - `nexus_lanes.json`（レーン高・バックログ・DA 健全性）。
  - `nexus_settlement.json`（決済遅延・財務省デルタ）。
  - レーンがモバイルテレメトリへ依存する場合は `android_operator_console.json` などの SDK ボード。
- Torii バイナリ輸送を有効にしている場合は `docs/source/torii/norito_rpc_telemetry.md` の OTEL エクスポータ要件を遵守。
- テレメトリ是正チェックリスト（`docs/source/nexus_telemetry_remediation_plan.md` セクション 5）を少なくとも四半期に一度実施し、記入済みフォームを運用レビューに添付。

### 主要メトリクス

| メトリクス | 説明 | アラート条件 |
|------------|------|--------------|
| `nexus_lane_height{lane_id}` | レーンごとのヘッド高。停止したバリデータを検知。 | 3 スロット連続で未更新なら警報。 |
| `nexus_da_backlog_chunks{lane_id}` | レーンごとの未処理 DA チャンク。 | デフォルト: 公開 64、プライベート 8 を超えたら警報。 |
| `nexus_settlement_latency_seconds{lane_id}` | レーンコミットからグローバル決済までの時間。 | 公開レーン P99 >900 ms、プライベート >1200 ms で警報。 |
| `torii_request_failures_total{scheme="norito_rpc"}` | Norito RPC のエラー数。 | 5 分間エラー率 >2 %。 |
| `telemetry_redaction_override_total` | テレメトリ匿名化のオーバーライド件数。 | 発生時に即 Sev 2 で通知し、コンプライアンスチケット必須。 |

## 4. インシデント対応

| 重大度 | 定義 | 必須アクション |
|--------|------|----------------|
| **Sev 1** | データスペース隔離の破綻、15 分超の決済停止、ガバナンス投票の破損。 | Nexus プライマリ + リリースエンジニアリング + コンプライアンスをページング。レーン admission を停止し、指標/ログを取得し、60 分以内に通達、5 営業日以内に RCA。 |
| **Sev 2** | レーンバックログの SLA 逸脱、30 分超のテレメトリ欠落、マニフェスト展開失敗。 | Nexus プライマリ + SRE をページングし、4 時間以内に緩和。2 営業日以内にフォローアップを記録。 |
| **Sev 3** | 文書ドリフトやアラート誤作動などの非ブロッキング事象。 | トラッカーに記録し、スプリント内で是正。 |

インシデントチケットには以下を含めます。

1. 影響レーン/データスペース ID とマニフェストハッシュ。
2. UTC タイムライン（検知・緩和・復旧・コミュニケーション）。
3. 検知を裏付ける指標/スクリーンショット。
4. フォローアップ（担当者/期日）と自動化/ランブック更新の要否。

## 5. エビデンスと監査トレイル

- **アーティファクトアーカイブ:** バンドル・マニフェスト・テレメトリエクスポートを `artifacts/nexus/<lane>/<date>/` に保存。
- **設定スナップショット:** 各リリースで編集済み `config.toml` と `trace-config` 出力を保存。
- **ガバナンス連携:** オンボーディングやインシデントチケットで参照される議会議事録と署名済み決定。
- **テレメトリエクスポート:** レーン関連の Prometheus TSDB チャンクを週次で取得し、最低 12 か月保管。
- **ランブック版管理:** このファイルを更新した場合は `docs/source/project_tracker/nexus_config_deltas/README.md` に変更を追記。

## 6. 関連資料

- `docs/source/nexus_overview.md` — アーキテクチャ概要。
- `docs/source/nexus.md` — 技術仕様。
- `docs/source/nexus_lanes.md` — レーン幾何。
- `docs/source/nexus_transition_notes.md` — 移行ロードマップ。
- `docs/source/cbdc_lane_playbook.md` — CBDC ポリシー。
- `docs/source/sora_nexus_operator_onboarding.md` — リリース/オンボーディング。
- `docs/source/nexus_telemetry_remediation_plan.md` — テレメトリガードレール。

ロードマップ NX-14 が進むたび、または新しいレーン種別・テレメトリルール・ガバナンスフックが導入された際は、このランブックも忘れずに更新してください。
