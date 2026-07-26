---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/operations.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスオペレーション
タイトル: Runbook 操作 Nexus
説明: ワークフロー オペレーター Nexus、リフレタント `docs/source/nexus_operations.md`。
---

`docs/source/nexus_operations.md` の参照ページを参照してください。チェックリストの操作を凝縮し、変更点の制御と操作の実行を制御する Nexus の操作を凝縮します。

## サイクル設定のチェックリスト

|エテープ |アクション |プルーヴ |
|----------|----------|----------|
|プレボリューム |リリースのハッシュ/署名の検証、確認者 `profile = "iroha3"` および構成のモデルの準備。 | `scripts/select_release_profile.py` の出撃、チェックサムのジャーナル、マニフェスト署名のバンドル。 |
|カタログの配置 | Mettre à jour le カタログ `[nexus]`、la politique de routage et les seuils DA selon leマニフェスト émis par le conseil、puis Capturer `--trace-config`。 |出撃 `irohad --sora --config ... --trace-config` 搭乗チケットの在庫を取得しました。 |
|スモークとカットオーバー |ランサー `irohad --sora --config ... --trace-config`、CLI の煙の執行者 (`FindNetworkStatus`)、許可証の輸出と許可の要求。 |煙テストのログと確認アラートマネージャー。 |
|体制は安定 | Surveiller ダッシュボード/アラート、フェア ツアーナー レ クレ セロン、ガバナンス コントロール、同期設定/ランブック、マニフェストの変更。 |レビュー トリメストリエルの分、ダッシュボードのキャプチャ、チケットの ID のローテーション。 |

オンボーディングの詳細 (交換のクレ、モデルのルーティング、リリースのプロフィール) の残りは `docs/source/sora_nexus_operator_onboarding.md` です。

## 変化のジェスション

1. **公開までの期間** - `status.md`/`roadmap.md` の発表内容。チェックリストとオンボーディング、PR、リリースに参加します。
2. **マニフェストの変更** - スペース ディレクトリの署名バンドルとアーカイバ `docs/source/project_tracker/nexus_config_deltas/` の検証。
3. **設定の差分** - レーン/データスペースのチケット参照に必要な `config/config.toml` の変更を宣伝します。有効な構成をコピーして、結合/アップグレードを保存します。
4. **ロールバックの演習** - 停止/復元/スモークの手順を繰り返します。荷主 les resultats dans `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`。
5. **適合承認** - レーン特権/CBDC は、政治的修飾法に対する前衛的な適合性を保持します (`docs/source/cbdc_lane_playbook.md`)。

## テレメトリと SLO

- ダッシュボード: `dashboards/grafana/nexus_lanes.json`、`nexus_settlement.json`、および SDK の仕様 (例: `android_operator_console.json`)。
- 警告: `dashboards/alerts/nexus_audit_rules.yml` およびトランスポート Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`)。
- 監視者のメトリック:
  - `nexus_lane_height{lane_id}` - 進行状況ペンダント トロワ スロットのアラート。
  - `nexus_da_backlog_chunks{lane_id}` - レーンごとにアラートを出します (パブリック 64 / プライベート 8)。
  - `nexus_settlement_latency_seconds{lane_id}` - アラート Quand le P99 900 ミリ秒 (パブリック) または 1200 ミリ秒 (プライベート)。
  - `torii_request_failures_total{scheme="norito_rpc"}` - アラート サイル エラー率 à 5 分デパス 2%。
  -`telemetry_redaction_override_total` - セクション 2 即時;チケットの適合性を保証し、オーバーライドを保証します。
- 改善策のチェックリストの実行者は、[改善策の計画 Nexus](./nexus-telemetry-remediation) のトリメストリエールメントと結合方法を再確認し、レビュー操作の補助メモを作成します。

## マトリスの事件

|グラビテ |定義 |応答 |
|----------|-----------|----------|
|セクション 1 |データ空間を完全に隔離し、和解まで 15 分以上経過し、政府の投票で汚職が発生しました。 |アラート Nexus プライマリ + リリース エンジニアリング + コンプライアンス、入場許可、コレクターの成果物、出版社との通信 <= 60 分、RCA <= 5 時間。 |
|セクション 2 |バックログのレーンでの SLA 違反、30 分を超える角度モルト、マニフェストのロールアウト。 |アラート Nexus プライマリ + SRE、減衰時間 <=4 時間、保管時間 2 時間。 |
|セクション 3 |非ブロカントの派生 (ドキュメント、アラート)。 |トラッカーによる登録者、スプリントによる修正の計画者。 |

レーン/データスペースの影響に関するインシデントのチケット、登録者の ID、マニフェストのハッシュ、年代記、サポートおよびデータの管理/管理の記録/ログ。

## アーカイブ・ド・プルーヴ- ストッカー バンドル/マニ​​フェスト/輸出情報 `artifacts/nexus/<lane>/<date>/`。
- Conserver configs expurgées + sortie `--trace-config` pour Chaque release.
- 会議の議事録と変更内容の署名とマニフェスト アップリケの決定に参加します。
- Conserver les snapshots Prometheus hebdomadaires des métriques Nexus ペンダント 12 モア。
- `docs/source/project_tracker/nexus_config_deltas/README.md` で Runbook の変更を登録し、変更に対する監査の責任を負います。

## マテリエルリエ

- アンサンブル : [Nexus 概要](./nexus-overview)
- 仕様 : [Nexus仕様](./nexus-spec)
- レーンの形状: [Nexus レーン モデル](./nexus-lane-model)
- 移行とルートのシム: [Nexus 移行メモ](./nexus-transition-notes)
- オンボーディング オペレーター: [Sora Nexus オペレーター オンボーディング](./nexus-operator-onboarding)
- 修復テレメトリ: [Nexus テレメトリ修復計画](./nexus-telemetry-remediation)