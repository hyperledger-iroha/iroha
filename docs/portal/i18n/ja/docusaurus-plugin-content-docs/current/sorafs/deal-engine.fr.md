---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ディールエンジン
タイトル: 協定 SoraFS
サイドバーラベル: 協定の締結
説明: アコード SF-8 のアンサンブル、統合 Torii および表面の表面。
---

:::note ソースカノニク
:::

# 協定 SoraFS

ロードマップ SF-8 の概要 SoraFS、フォーニッサン
在庫と回収の調和を決定するための互換性
クライアントとフルニサー。ペイロード Norito を介した合意の内容
`crates/sorafs_manifest/src/deal.rs` の定義、合意条件の規定、
債券の評価、微額確率、規制の登録。

ワーカー SoraFS は (`sorafs_node::NodeHandle`) インスタンスを解除します
`DealEngine` チャックプロセスを注いでください。ル・モーター:

- `DealTermsV1` 経由で協定を有効および登録します。
- XOR による複製の利用に関する累積的な請求額と信頼性。
- un échantillonnage déterministe によるマイクロペイメント確率の価値
  BLAKE3 のベース ;など
- 台帳のスナップショットとペイロードを出版物に合わせて作成します
  統治。

検証のための単位テスト、マイクロペイメントの選択、および流動性の確認
安全な API を使用して操作を制限する必要があります。レ・レグルマン
ペイロードの管理 `DealSettlementV1`、統合指示
SF-12 の出版パイプライン、および現在進行中の OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`、`deal_expected_charge_nano`、`deal_client_debit_nano`、
`deal_outstanding_nano`、`deal_bond_slash_nano`、`deal_publish_total`) ダッシュボード Torii など
SLO のアプリケーション。 Les travaux suivants ciblent l'automatization du slashing initiée par
政府の政治に関する監査と意味の調整。

オーストラリアでの使用方法の詳細 `sorafs.node.micropayment_*` :
`micropayment_charge_nano`、`micropayment_credit_generated_nano`、
`micropayment_credit_applied_nano`、`micropayment_credit_carry_nano`、
`micropayment_outstanding_nano`、チケットの計算
(`micropayment_tickets_processed_total`、`micropayment_tickets_won_total`、
`micropayment_tickets_duplicate_total`)。宝くじの情報を公開する
マイクロペイメントなどの利益を得るために、操作の確率を計算します。
信用の繰り越し、結果の評価。

## 統合 Torii

Torii エンドポイントを公開し、4 人乗りの信号を使用してパイロットを監視します
特別な配線を必要としないサイクル・ド・ヴィー・デ・アコード:

- `POST /v2/sorafs/deal/usage` テレメトリーを受け入れます `DealUsageReport` と renvoie
  互換性決定結果の結果 (`UsageOutcome`)。
- `POST /v2/sorafs/deal/settle` la fenêtre courante、en streamant le を終了します
  `DealSettlementRecord` 結果の平均 `DealSettlementV1` エンコードと Base64
  prêt pour 出版物 dans le DAG de gouvernance。
- `/v2/events/sse` と Torii の拡散被弾登録
  `SorafsGatewayEvent::DealUsage` 使用状況の履歴書 (エポック、GiB 時間測定、
  チケットの計算、料金決定)、登録
  `SorafsGatewayEvent::DealSettlement` スナップショットの基準を含む登録簿
  ainsi que le Digest/taille/base64 BLAKE3 de l'artefact de gouvernance sur disque, et des alles
  `SorafsGatewayEvent::ProofHealth` dès que les seuils PDP/PoTR ソント デパス (fournisseur、fenêtre、
  ストライク/クールダウンのエタット、ペナリテのモンタント)。 Les consommateurs peuvent filtler par fournisseur
  投票なしで新しい情報を収集し、監視や警告を確認できます。

Les deux エンドポイント参加者 au フレームワーク クォータ SoraFS via la nouvelle fenêtre
`torii.sorafs.quota.deal_telemetry`、永続的補助操作者調整者ル・トゥー・ド・スーミッション
autorisé par déployement。