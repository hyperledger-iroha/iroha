---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ディールエンジン
title: SoraFS のモーター
サイドバーラベル: モーター・デ・アクエルドス
説明: SF-8 モーターの再開、Torii とテレメトリの機能の統合。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/deal_engine.md` のページを参照してください。あらゆる活動を記録し、記録を残すことができます。
:::

# SoraFS のモーター

SF-8 が SoraFS のモーター デ アクエルドスを導入し、ポータルに
アルマセナミエントと回復期の感染状況を決定する
クライアントも証明者も。ペイロードに関する詳細情報 Norito
`crates/sorafs_manifest/src/deal.rs` の定義、正しい情報、
大量のブロック、ミクロパゴス確率と流動性登録。

SoraFS (`sorafs_node::NodeHandle`) の労働者がこの瞬間に勤務します
`DealEngine` はノードの処理を実行します。エルモーター：

- `DealTermsV1` の登録を有効にします。
- XOR による積荷デノミナドの複製。
- マイクロパゴの確率を評価し、決定的な決定を行う
  バサド en BLAKE3; y
- 公開用の清算用台帳とペイロードのスナップショットを生成します
  デ・ゴベルナンザ。

ラス・プルエバス・ユニタリアス・キュブレン・バリダシオン、マイクロパゴスとフルホス・デの選択
オペラドールの液体の噴出と API のコンフィアンサ。
ゴベルナンザ `DealSettlementV1` でペイロードを放出するラス リキッドアシオネス アホラ、
SF-12 の公開パイプラインを直接接続し、実際のシリーズに接続します。
OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`、`deal_expected_charge_nano`、`deal_client_debit_nano`、
`deal_outstanding_nano`、`deal_bond_slash_nano`、`deal_publish_total`) Torii のダッシュボード
SLO のアプリケーション。斬撃の自動化機能を失う
政治のキャンセルに関する調整を開始し、監査を行います。

メトリカス `sorafs.node.micropayment_*` の遠隔測定:
`micropayment_charge_nano`、`micropayment_credit_generated_nano`、
`micropayment_credit_applied_nano`、`micropayment_credit_carry_nano`、
`micropayment_outstanding_nano`、チケットを紛失しました
(`micropayment_tickets_processed_total`、`micropayment_tickets_won_total`、
`micropayment_tickets_duplicate_total`)。ロッテリアの合計指数
マイクロパゴス州のビクトリア州とプエダンの相関関係の可能性
結果の損失に対する信用のキャリーオーバー。

## 統合コン Torii

Torii エンドポイントのデディカドス パラケ ロス 証明のレポートが使用され、コンドゥスカン エル シクロを説明します
あなたの罪の配線のカスタマイズ:

- `POST /v2/sorafs/deal/usage` アセプタ テレメトリア `DealUsageReport` とレトルナ
  感染の確定結果 (`UsageOutcome`)。
- `POST /v2/sorafs/deal/settle` 最終的なイベントの実際、送信エル
  `DealSettlementRecord` 結果の junto con un `DealSettlementV1` en Base64
  DAG デ ゴベルナンザの出版リスト。
- `/v2/events/sse` から Torii への送信レジストリ `SorafsGatewayEvent::DealUsage`
  私たちの再開の情報 (エポック、GiB ホラ メディドス、チケットのコンタドール、
  貨物決定者)、登録者 `SorafsGatewayEvent::DealSettlement`
  スナップショットのキャッシュを含めるには、ダイジェスト/タマニョ/base64 の液体元帳が含まれます
  BLAKE3 del artefacto de gobernanza en disco、y アラート `SorafsGatewayEvent::ProofHealth`
  PDP/PoTR (プローブ、ベンタナ、ストライク/クールダウンの実行、
  モント・デ・ペナリザシオン）。ロス・コンスミドーレ・プエデン・フィルター・ポル・プローベダー・パラ・リアクショナー
  新しいテレメトリー、清算、プルエバスの警告など、ヘイサーの投票を監視します。

Ambos エンドポイントは、SoraFS の新しいフレームワークに参加しています
`torii.sorafs.quota.deal_telemetry`、ロスオペラドーレスの環境を許可する
許可を得てください。