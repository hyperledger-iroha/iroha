---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ネクサスの概要
title: アペルク デ ソラ Nexus
説明: アーキテクチャの履歴書 Iroha 3 (Sora Nexus) モノリポジトリの標準ドキュメントと比較します。
---

Nexus (Iroha 3) etend Iroha 2 は、マルチレーンで実行され、SDK の管理と外部の一部を管理します。 Cette ページ le nouveau の概要 `docs/source/nexus_overview.md` は、モノレポートに関する講義を参照し、ポータルのコンポーネントの迅速なコメントを作成し、アーキテクチャの詳細を確認します。

## リーニュ・ド・バージョン

- **Iroha 2** - auto-heberges がコンソーシアムと reseaux prives に展開されます。
- **Iroha 3 / Sora Nexus** - 公共マルチレーンの運営者は、ドネ (DS) の登録と管理、規制、監視の義務を負っています。
- ミーム ワークスペース (IVM + ツールチェーン Kotodama)、修正 SDK のドンク、ABI およびフィクスチャ Norito の保存ポータブル。アーカイブ `iroha3-<version>-<os>.tar.zst` を再接続する Nexus を操作します。 `docs/source/sora_nexus_operator_onboarding.md` チェックリスト プレイン エクランを報告します。

## 建設ブロック

|構成材 |履歴書 |ポインツ デュ ポルテイル |
|----------|-----------|--------------|
|エスパス デ ドニーズ (DS) | Domaine d'execution/stockage defini par la gouvernance qui possede une ou plusieurs Lans、des ensemble de validateurs、la classe deconfidentialite et la politique de frais + DA を宣言します。 | [Nexus 仕様](./nexus-spec) マニフェストのスキーマを作成します。 |
|レーン |シャードによって実行が決定されます。 NPoS のグローバル オードンヌの活動との関わり。レーンのクラスは `default_public`、`public_custom`、`private_permissioned` および `hybrid_confidential` です。 | [レーンのモデル](./nexus-lane-model) は、ジオメトリ、在庫の接頭辞、および保持をキャプチャします。 |
|移行計画 | Nexus に対するモノレーン展開のプレースホルダーの識別、ルートの段階およびダブル プロファイル トレーセントのパッケージングのコメントの展開。 | [移行ノート](./nexus-transition-notes) 移行フェーズに関する文書。 |
|宇宙ディレクトリ | DS のマニフェスト + バージョンを保管するための登録を行います。カタログのレパートリーを前もって調整し、レパートリーを調整します。 |マニフェストの差分 `docs/source/project_tracker/nexus_config_deltas/` を参照してください。 |
|レーンのカタログ |設定 `[nexus]` セクションでは、レーンとエイリアスの ID、ルートと政治のポリシーをマップします。 `irohad --sora --config ... --trace-config` の主要なカタログの解決策の監査。 | `docs/source/sora_nexus_operator_onboarding.md` パルクール CLI を利用します。 |
|ルート・ド・レグルメント | XOR による転送のオーケストレーションにより、CBDC は公共の清算用レーンを独占します。 | `docs/source/cbdc_lane_playbook.md` 政治とテレメトリの詳細。 |
|テレメトリー/SLO |ボード + アラート `dashboards/grafana/nexus_*.json` は、レーンの管理、バックログ DA、規制の遅延、および統治ファイルの詳細をキャプチャします。 | Le [テレメトリーの修復計画](./nexus-telemetry-remediation) 詳細な表表、警告および監査の詳細。 |

## 瞬時に展開

|フェーズ |フォーカス |出撃基準 |
|------|------|------|
| N0 - ベータフェルメ |レジストラ gere par le conseil (`.sora`)、オンボーディング オペレータ マニュアル、カタログ デ レーン統計。 | DS 署名と統治権限の繰り返しをマニフェストします。 |
| N1 - ランセメント公開 | Ajoute les suffixes `.nexus`、les encheres、un registrar en libre-service、le cablage de reglement XOR。 |同期リゾルバー/ゲートウェイのテスト、調整の事実確認表、訴訟演習。 |
| N2 - 拡張 | `.dao`、API の収益、分析、訴訟ポータル、スチュワードのスコアカードの紹介。 |バージョンに準拠した成果物、政治のツールキット陪審、財務上の透明性の関係。 |
|ポルテ NX-12/13/14 |適合性の確認、テレメトリーのダッシュボード、およびパイロットの協力のためのアンサンブルのソートやドキュメントの管理を行います。 | [Nexus 概要](./nexus-overview) + [Nexus 操作](./nexus-operations) 出版物、ダッシュボード ケーブル、政治政策の融合。 |

## オペレーターの責任1. **衛生的な構成** - gardez `config/config.toml` は、レーンとデータスペースの公開カタログを同期します。 archivez 出撃 `--trace-config` アベック チケット リリース。
2. **マニフェストの表示** - カタログの内容を確認し、バンドルされたスペース ディレクトリを確認して、新しい情報を確認します。
3. **クーベルチュール テレメトリ** - ダッシュボード `nexus_lanes.json`、`nexus_settlement.json` などの補助 SDK を公開します。ケーブルは、PagerDuty のアラートと、テレメトリーの修復計画のトリメストリエールのレビューを現実にします。
4. **事件発生の信号** - [Nexus 操作](./nexus-operations) および RCA の緊急事態に関する情報。
5. **統治の準備** - コンセイル Nexus の影響を受けるレーンとロールバック チャク トリメストレの繰り返し指示を支援します (`docs/source/project_tracker/nexus_config_deltas/` 経由)。

## オーストラリアの声

- アペルク正規品 : `docs/source/nexus_overview.md`
- 仕様詳細: [./nexus-spec](./nexus-spec)
- レーンのジオメトリ: [./nexus-lane-model](./nexus-lane-model)
- 移行計画: [./nexus-transition-notes](./nexus-transition-notes)
- テレメトリーの修復計画: [./nexus-telemetry-remediation](./nexus-telemetry-remediation)
- Runbook オペレーション: [./nexus-operations](./nexus-operations)
- オンボーディング オペレーターのガイド : `docs/source/sora_nexus_operator_onboarding.md`