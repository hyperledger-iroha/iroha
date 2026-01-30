---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/nexus/operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b69abea94ffb032bc0a2f18ef7ec824795da48a5df6cce699a31d614d9882b98
source_last_modified: "2025-11-14T04:43:20.522106+00:00"
translation_last_reviewed: 2026-01-30
---

このページは `docs/source/nexus_operations.md` のクイックリファレンス版です。運用チェックリスト、変更管理のフック、Nexus オペレーターが守るべきテレメトリカバレッジ要件を要約しています。

## ライフサイクルチェックリスト

| ステージ | アクション | エビデンス |
|-------|--------|----------|
| 事前確認 | リリースのハッシュ/署名を確認し、`profile = "iroha3"` を確認して設定テンプレートを準備する。 | `scripts/select_release_profile.py` の出力、checksum ログ、署名済みマニフェストバンドル。 |
| カタログ整合 | `[nexus]` カタログ、ルーティングポリシー、DA 閾値を評議会発行のマニフェストに合わせて更新し、`--trace-config` を取得する。 | `irohad --sora --config ... --trace-config` の出力をオンボーディングチケットに保管。 |
| スモーク & 切り替え | `irohad --sora --config ... --trace-config` を実行し、CLI のスモーク (`FindNetworkStatus`) を実施、テレメトリ出力を検証し、入場申請する。 | スモークテストログ + Alertmanager 確認。 |
| 定常運用 | ダッシュボード/アラートを監視し、ガバナンスの周期でキーをローテーションし、マニフェスト変更時に configs/runbooks を同期する。 | 四半期レビュー議事録、ダッシュボードのスクリーンショット、ローテーションチケット ID。 |

詳細なオンボーディング（キー交換、ルーティングテンプレート、リリースプロファイル手順）は `docs/source/sora_nexus_operator_onboarding.md` にあります。

## 変更管理

1. **リリース更新** - `status.md`/`roadmap.md` の告知を追跡し、各リリース PR にオンボーディングチェックリストを添付する。
2. **lane マニフェスト変更** - Space Directory の署名バンドルを検証し、`docs/source/project_tracker/nexus_config_deltas/` に保管する。
3. **設定差分** - `config/config.toml` の変更は必ず lane/data-space を参照するチケットが必要。ノード参加やアップグレード時に有効な config のレッドアクト版を保存する。
4. **ロールバック演習** - 四半期ごとに stop/restore/smoke をリハーサルし、`docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` に結果を記録する。
5. **コンプライアンス承認** - private/CBDC lane は DA ポリシーやテレメトリの redaction ノブを変更する前に compliance の承認が必要（`docs/source/cbdc_lane_playbook.md` を参照）。

## テレメトリと SLOs

- ダッシュボード: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`、および SDK 固有ビュー（例: `android_operator_console.json`）。
- アラート: `dashboards/alerts/nexus_audit_rules.yml` と Torii/Norito transport ルール（`dashboards/alerts/torii_norito_rpc_rules.yml`）。
- 監視するメトリクス:
  - `nexus_lane_height{lane_id}` - 3 スロット進捗ゼロでアラート。
  - `nexus_da_backlog_chunks{lane_id}` - lane 固有閾値を超えたらアラート（デフォルト 64 public / 8 private）。
  - `nexus_settlement_latency_seconds{lane_id}` - P99 が 900 ms（public）または 1200 ms（private）を超えたらアラート。
  - `torii_request_failures_total{scheme="norito_rpc"}` - 5 分エラー比率が 2% を超えたらアラート。
  - `telemetry_redaction_override_total` - Sev 2 即時; override には compliance チケットが必要。
- [Nexus telemetry remediation plan](./nexus-telemetry-remediation) のチェックリストを少なくとも四半期ごとに実行し、記入済みフォームを運用レビューのメモに添付する。

## インシデントマトリクス

| 重大度 | 定義 | 対応 |
|----------|------------|----------|
| Sev 1 | data-space の分離侵害、settlement 停止 >15 分、またはガバナンス投票の破損。 | Nexus Primary + Release Engineering + Compliance を呼び出し、入場を凍結、アーティファクトを収集、<=60 分で告知、RCA <=5 営業日。 |
| Sev 2 | lane backlog の SLA 違反、テレメトリ盲点 >30 分、マニフェスト rollout 失敗。 | Nexus Primary + SRE を呼び出し、<=4 時間で緩和、2 営業日以内にフォローアップ登録。 |
| Sev 3 | 非ブロッキングのドリフト（docs、alerts）。 | トラッカーに記録し、スプリント内で修正。 |

インシデントチケットには、影響した lane/data-space の ID、マニフェストハッシュ、タイムライン、支援メトリクス/ログ、フォローアップのタスク/オーナーを記録してください。

## 証拠アーカイブ

- bundles/manifestes/テレメトリ出力を `artifacts/nexus/<lane>/<date>/` に保管する。
- 各リリースでレッドアクト済み config と `--trace-config` 出力を保持する。
- config またはマニフェスト変更時に、評議会議事録 + 署名済み決定を添付する。
- Nexus メトリクスに関連する Prometheus の週次スナップショットを 12 か月保持する。
- runbook の編集を `docs/source/project_tracker/nexus_config_deltas/README.md` に記録し、責任変更の時期を監査できるようにする。

## 関連資料

- 概要: [Nexus overview](./nexus-overview)
- 仕様: [Nexus spec](./nexus-spec)
- Lane ジオメトリ: [Nexus lane model](./nexus-lane-model)
- 移行とルーティングシム: [Nexus transition notes](./nexus-transition-notes)
- オペレーターオンボーディング: [Sora Nexus operator onboarding](./nexus-operator-onboarding)
- テレメトリ修復: [Nexus telemetry remediation plan](./nexus-telemetry-remediation)
