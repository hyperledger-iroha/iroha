---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ディールエンジン
タイトル: SoraFS 取引エンジン
Sidebar_label: 取引エンジン
説明: SF-8 ディール エンジン、Torii 統合、テレメトリ サーフェス
---

:::note メモ
:::

# SoraFS ディール エンジン

SF-8 ロードマップ トラック SoraFS ディール エンジン
クライアントとプロバイダーとストレージと取得契約との関係
決定論的会計契約 Norito ペイロード数
بیان کیا جاتا ہے جو `crates/sorafs_manifest/src/deal.rs` میں تعریف شدہ ہیں، اور
取引条件、債券ロック、確率的少額決済、決済記録、データ

組み込み SoraFS ワーカー (`sorafs_node::NodeHandle`) ノード プロセスの処理
`DealEngine` インスタンス بناتا ہے۔エンジン:

- `DealTermsV1` 取引情報の検証、登録の確認
- レプリケーション使用レポート XOR ベースの料金が累積される
- 決定論的な BLAKE3 ベースのサンプリングによる確率的なマイクロペイメント ウィンドウの評価ああ
- 台帳スナップショット、ガバナンス発行、決済ペイロード、および決済ペイロード

単体テストの検証、マイクロペイメントの選択、決済フロー、カバーのテスト
演算子 API の演習 テスト和解 `DealSettlementV1` ガバナンス
ペイロードは SF-12 パブリッシング パイプラインを発行し、ワイヤを発行します。
OpenTelemetry `sorafs.node.deal_*` シリーズ
(`deal_settlements_total`、`deal_expected_charge_nano`、`deal_client_debit_nano`、
`deal_outstanding_nano`、`deal_bond_slash_nano`、`deal_publish_total`) Torii ダッシュボード
SLO の施行に関する更新情報フォローアップ項目 監査人主導のスラッシュ自動化
キャンセル セマンティクス ガバナンス ポリシー 座標 座標 پر مرکوز ہیں۔

使用状況テレメトリ `sorafs.node.micropayment_*` メトリクス セット フィード:
`micropayment_charge_nano`、`micropayment_credit_generated_nano`、
`micropayment_credit_applied_nano`、`micropayment_credit_carry_nano`、
`micropayment_outstanding_nano`、チケットカウンター
(`micropayment_tickets_processed_total`、`micropayment_tickets_won_total`、
`micropayment_tickets_duplicate_total`)。合計の確率的宝くじフロー ہیں تاکہ
オペレーターによるマイクロペイメントの獲得 クレジット キャリーオーバー 決済結果の相関関係

## Torii 統合

Torii 専用エンドポイントがプロバイダーの使用状況レポートを公開する
特注の配線とライフサイクル ドライブの関係:

- `POST /v1/sorafs/deal/usage` `DealUsageReport` テレメトリを受け入れます
  決定的な会計結果 (`UsageOutcome`) が返されます
- `POST /v1/sorafs/deal/settle` 現在のウィンドウを終了します
  `DealSettlementRecord` Base64 エンコードされた `DealSettlementV1` ストリーム ストリーム
  ガバナンス DAG 出版物 جیار ہوتا ہے۔
- Torii `/v1/events/sse` フィード `SorafsGatewayEvent::DealUsage` ブロードキャストを記録します
  使用状況の送信 (エポック、従量制 GiB 時間、チケット カウンター、
  確定的請求)、`SorafsGatewayEvent::DealSettlement` レコード、正規決済台帳のスナップショット
  ディスク上のガバナンス アーティファクト BLAKE3 ダイジェスト/サイズ/base64 の処理
  `SorafsGatewayEvent::ProofHealth` アラート「PDP/PoTR しきい値が超過 (プロバイダー、ウィンドウ、ストライク/クールダウン状態、ペナルティ金額)」
  消費者プロバイダーのフィルタリング、テレメトリー、決済、証明健康アラート
  投票 投票 反応 投票 投票

エンドポイント SoraFS クォータ フレームワーク `torii.sorafs.quota.deal_telemetry` ウィンドウ
オペレータの展開、許可された送信レートの調整、およびサポートの調整