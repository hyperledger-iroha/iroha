---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/operations.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスオペレーション
タイトル: Nexus の操作手順書
説明: Nexus の履歴書リスト、`docs/source/nexus_operations.md` のオペレーターの履歴書。
---

米国は `docs/source/nexus_operations.md` を参照してページを表示します。 Nexus デベン・セギールのオペラリストのリスト、カンビオスのガンチョスとテレメトリアのコベルトゥーラの要求を再開します。

## リスト・デ・シクロ・デ・ヴィダ

|エタパ |アクシオネス |証拠 |
|----------|----------|----------|
|プレブエロ |ランサミエントのハッシュ/企業を検証し、`profile = "iroha3"` を確認し、植物の構成を準備します。 | `scripts/select_release_profile.py`、チェックサムのレジストリ、マニフェスト ファームのバンドル。 |
|カタログの一覧 |実際のカタログ `[nexus]` は、政治的政治と安全保障に関する情報を管理し、`--trace-config` をキャプチャします。 |オンボーディングのチケットに関する `irohad --sora --config ... --trace-config` アルマセナダ。 |
|プルエバス・デ・ヒューモ・イ・コルテ | Ejecuta `irohad --sora --config ... --trace-config`、Ejecuta la prueba de Humo del CLI (`FindNetworkStatus`)、validal las exportaciones de telemetria y solicita admission. |煙テストのログとアラートマネージャーの確認。 |
|エスタド安定 |ダッシュボード/アラート、ロタ クラベス セグン ラ カデンシア デ ゴベルナンザとシンクロニザの設定/ランブックを監視し、カンビエン ロス マニフェストを監視します。 |改訂三学期の議事録、ダッシュボードのキャプチャ、チケットのID。 |

オンボーディング デタラード (クラーベスの再利用、植物園、ランザミエントのパソス デル パーフィル) は、`docs/source/sora_nexus_operator_onboarding.md` で永続的に適用されます。

## ジェスティオン デ カンビオス

1. **実際のランザミエント** - `status.md`/`roadmap.md` に関する発表;リリースの際の PR のオンボーディングに必要なチェックリスト。
2. **レーンのマニフェストのカンビオス** - スペース ディレクトリおよびアーカイブ `docs/source/project_tracker/nexus_config_deltas/` を検証します。
3. **構成のデルタ** - `config/config.toml` では、レーン/データスペースのチケット参照が必要です。実際の設定を有効にするには、コピーを編集してください。
4. **ロールバックのシミュレーション** - 停止/復元/スモークの手順を実行します。 `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` のレジストラ結果。
5. **コンプライアンス違反** - プライバシー/CBDC の安全性を確認するためのコンプライアンスの変更および DA またはテレメトリの再編集 (バージョン `docs/source/cbdc_lane_playbook.md`)。

## テレメトリアと SLO

- ダッシュボード: `dashboards/grafana/nexus_lanes.json`、`nexus_settlement.json`、SDK の詳細情報 (例、`android_operator_console.json`)。
- アラート: `dashboards/alerts/nexus_audit_rules.yml` および交通規制 Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`)。
- メトリカスは警戒します:
  - `nexus_lane_height{lane_id}` - 干し草の進行状況が持続するスロットがないことを警告します。
  - `nexus_da_backlog_chunks{lane_id}` - レーンに関する警告 (公開 64 件 / 非公開 8 件)。
  - `nexus_settlement_latency_seconds{lane_id}` - アラート クアンド エル P99 スーパー 900 ミリ秒 (パブリック) または 1200 ミリ秒 (プライベート)。
  - `torii_request_failures_total{scheme="norito_rpc"}` - エラー 5 分超過 2% を警告します。
  - `telemetry_redaction_override_total` - 中間レベル 2。アセグラ・ケ・ラス・アヌラシオネス・テンガン・チケット・デ・コンプライアンス。
- テレメトリアの修復チェックリスト [Nexus](./nexus-telemetry-remediation) のトリメストラルメントと補助的な公式の修正と操作の改訂の計画を確認します。

## 事件の発生

|セベリダッド |定義 |レスペスタ |
|----------|-----------|----------|
|セクション 1 |データスペースの衝突、和解のパロ、ゴベルナンザの破損まで 15 分以上。 | Nexus プライマリ + リリース エンジニアリング + コンプライアンス、コンジェラル アドミッション、収集アーチファクト、公開コミュニケーション <=60 分、RCA <=5 ディアス ハビレスを参照。 |
|セクション 2 |バックログの SLA、30 分を超えるテレメトリの実行、マニフェストのロールアウトを免除します。 | Nexus プライマリ + SRE、緩和策は 4 時間以内、レジストラのフォローアップは 2 日間で可能です。 |
|セクション 3 |デリバのブロカンテ (ドキュメント、アラート)。 |トラッカーと計画を管理するレジストラ。 |

インシデントのチケット、レーン/データスペースの影響、レジストラ ID、マニフェストのハッシュ、ティエンポの履歴、サービス/データ/所有者のメトリクス/ログ。

## 証拠のアーカイブ- Guarda はテレメトリア バホ `artifacts/nexus/<lane>/<date>/` をバンドル/マニ​​フェスト/エクスポートします。
- Conserva configs redactadas + salida de `--trace-config` para cada release.
- コンセホの調整会議 + 決定を確定するための設定を決定します。
- Prometheus に関するスナップショットは、Nexus のメトリクスに 12 日間保存されます。
- `docs/source/project_tracker/nexus_config_deltas/README.md` でのレジストラ エディシオン デル ランブックは、セパン クアンド カンビアロン ラス レスポンサビリダデスの監査を対象としています。

## 素材関係

- 履歴書: [Nexus 概要](./nexus-overview)
- 仕様: [Nexus仕様](./nexus-spec)
- レーンのジオメトリア: [Nexus レーン モデル](./nexus-lane-model)
- ルーティングの移行シム: [Nexus 移行メモ](./nexus-transition-notes)
- オペレーターのオンボーディング: [Sora Nexus オペレーターのオンボーディング](./nexus-operator-onboarding)
- テレメトリの修復: [Nexus テレメトリ修復計画](./nexus-telemetry-remediation)