---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/operations.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスオペレーション
タイトル: オペラのランブック Nexus
説明: Resumo pronto para uso em campo do fluxo de trabalho do operador Nexus、espelhando `docs/source/nexus_operations.md`。
---

`docs/source/nexus_operations.md` を参照してください。チェックリスト操作の目的は、テレメトリアのコベルトゥーラに必要な操作、Nexus 開発者によるムダンカの操作です。

## リスト・デ・シクロ・デ・ヴィダ

|エタパ |アコス |証拠 |
|----------|----------|----------|
|プレブー |リリースのハッシュ/組み合わせを確認し、`profile = "iroha3"` を確認して、設定用のテンプレートを準備します。 | `scripts/select_release_profile.py` のログ、チェックサムのログ、マニフェストのバンドル。 |
|アリンハメント ド カタログ | `[nexus]` のカタログ、DA 準拠の政治、マニフェストの提出、ペロコンセルホの政治、`--trace-config` のキャプチャを実現します。 | `irohad --sora --config ... --trace-config` のチケットをオンボーディングで購入できます。 |
|スモークとカットオーバー | `irohad --sora --config ... --trace-config` を実行し、CLI (`FindNetworkStatus`) を実行し、テレメトリの輸出許可と申請許可を有効にします。 |煙テストのログを作成し、アラートマネージャーを確認します。 |
|エスタド・エスタベル |ダッシュボード/アラートを監視し、ガバナンスの要綱に従って定期的に行動し、構成/ランブックを確実にマニフェストに同期します。 |トリメストラルの見直し、ダッシュボードのキャプチャ、回転チケットの ID。 |

オンボーディングの詳細 (選手の交代、チームのテンプレート、リリースの実行) は `docs/source/sora_nexus_operator_onboarding.md` で永続的に行われます。

## ゲシュタオ デ ムダンカ

1. **リリースの概要** - `status.md`/`roadmap.md` の発表;リリースの CADA PR のオンボーディングのチェックリストの別紙。
2. **Mudancas de Manifesto de LANE** - スペース ディレクトリのアーカイブ ＯＳ EM `docs/source/project_tracker/nexus_config_deltas/` を検証するバンドル。
3. **設定デルタ** - レーン/データスペースのチケット参照を必要とする `config/config.toml` です。安全な環境を設定し、安全な環境を維持してください。
4. **ロールバックのトレイノ** - 停止/復元/スモークの手順を実行します。 `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md` の結果を登録します。
5. **コンプライアンスの承認** - レーンのプライバシー/CBDC 開発は、コンプライアンスの有効性を確認し、DA およびテレメトリの管理を行います (バージョン `docs/source/cbdc_lane_playbook.md`)。

## テレメトリアと SLO

- ダッシュボード: `dashboards/grafana/nexus_lanes.json`、`nexus_settlement.json`、SDK 固有の仕様 (例、`android_operator_console.json`)。
- アラート: `dashboards/alerts/nexus_audit_rules.yml` 輸送規則 Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`)。
- 観測者であるメトリカス:
  - `nexus_lane_height{lane_id}` - ゼロ進行ポート スロットに関するアラート。
  - `nexus_da_backlog_chunks{lane_id}` - レーンに関する警告 (パブリック 64 / プライベート 8)。
  - `nexus_settlement_latency_seconds{lane_id}` - アラート クアンド o P99 が 900 ミリ秒 (パブリック) または 1200 ミリ秒 (プライベート) を超えています。
  - `torii_request_failures_total{scheme="norito_rpc"}` - 2% を超える分類群を 5 分以内に警告します。
  - `telemetry_redaction_override_total` - セブ 2 即時;ガランタ クエリはテナム チケットのコンプライアンスを無効にします。
- テレメトリの修復チェックリスト [Nexus テレメトリ修復計画](./nexus-telemetry-remediation) を実行し、修正操作の公式を作成します。

## 事件の発生

|セヴェリダーデ |定義 |レスポスタ |
|----------|-----------|----------|
|セクション 1 |データスペースの隔離、決済のパラダ > 15 分、政府の投票を完了します。 | Acione Nexus プライマリ + リリース エンジニアリング + コンプライアンス、登録承認、最新情報の公開、公開コミュニケーション <=60 分、RCA <=5 dias uteis。 |
|セクション 2 | SLA のバックログのレーン、テレメトリのポントセゴ > 30 分、マニフェスト ファルホのロールアウト。 | Acion Nexus プライマリ + SRE、4 時間以内の緩和、2 日間のフォローアップ登録。 |
|セクション 3 | Deriva nao bloqueante (ドキュメント、アラート)。 |スプリントを修正する予定のトラッカーを登録しません。 |

インシデントのチケット、レーン/データスペースのアフェタダ、ハッシュのマニフェスト、タイムライン、メトリクス/ログのサポートおよびタレファ/所有者のフォローアップのレジストラ ID。

## 証拠の保管

- Armazene バンドル/マニフェスト/テレメトリアのエクスポート `artifacts/nexus/<lane>/<date>/`。
- Mantenha configs redigidas + Saida de `--trace-config` para cada release.
- コンセルホの議事録を作成し、設定とマニフェストを決定する必要があります。
- Nexus の 12 メトリクスに関連する Prometheus のスナップショットを保存します。
- 管理責任者として監査の対象となるランブック `docs/source/project_tracker/nexus_config_deltas/README.md` を実行するための教育を登録します。## 素材関係

- ヴィサオ ジェラル: [Nexus 概要](./nexus-overview)
- 詳細: [Nexus 仕様](./nexus-spec)
- レーンのジオメトリア: [Nexus レーン モデル](./nexus-lane-model)
- 移行と移行: [Nexus 移行メモ](./nexus-transition-notes)
- オペレーターのオンボーディング: [Sora Nexus オペレーターのオンボーディング](./nexus-operator-onboarding)
- テレメトリの修復: [Nexus テレメトリ修復計画](./nexus-telemetry-remediation)