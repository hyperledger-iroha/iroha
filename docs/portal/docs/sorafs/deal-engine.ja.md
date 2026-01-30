---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a85de93fe3864e11479242bbe76b6e1b96f854b548dc0ef78c4a6a4504d31bba
source_last_modified: "2025-11-15T07:12:49.005624+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: deal-engine
title: SoraFS Deal Engine
sidebar_label: Deal Engine
description: SF-8 の Deal Engine、Torii 連携、テレメトリ面の概要。
---

:::note 正規ソース
このページは `docs/source/sorafs/deal_engine.md` を反映しています。レガシーのドキュメントが有効な間は両方を整合させてください。
:::

# SoraFS Deal Engine

SF-8 のロードマップトラックは SoraFS の Deal Engine を導入し、
クライアントとプロバイダの間のストレージ/リトリーブ合意に対して
決定的な会計を提供します。合意は `crates/sorafs_manifest/src/deal.rs` で定義される
Norito のペイロードで記述され、契約条項、ボンドのロック、確率的マイクロペイメント、
精算記録をカバーします。

組み込み SoraFS ワーカー (`sorafs_node::NodeHandle`) は、各ノードプロセスごとに
`DealEngine` を生成します。エンジンは次を行います:

- `DealTermsV1` を用いて合意を検証・登録する。
- レプリケーション使用量の報告時に XOR 建ての課金を積算する。
- BLAKE3 に基づく決定的サンプリングで確率的マイクロペイメントのウィンドウを評価する。
- ガバナンス公開に適した台帳スナップショットと精算ペイロードを生成する。

単体テストは検証、マイクロペイメント選択、精算フローを網羅しており、
オペレーターが安心して API を検証できます。精算は `DealSettlementV1` ガバナンス
ペイロードを出力するようになり、SF-12 の公開パイプラインへ直結します。さらに
OpenTelemetry の `sorafs.node.deal_*` シリーズ
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) を更新し、
Torii ダッシュボードと SLO の適用に利用されます。後続項目では監査人発の
スラッシング自動化と、キャンセルのセマンティクスをガバナンス方針と調整する作業に
焦点を当てます。

利用テレメトリは `sorafs.node.micropayment_*` のメトリクスにも供給されます:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`、およびチケットカウンタ
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`)。これらの合計は確率的な抽選フローを可視化し、
マイクロペイメントの当選やクレジット繰越と精算結果の相関を追えるようにします。

## Torii 連携

Torii は専用エンドポイントを公開し、プロバイダが使用量を報告して
合意のライフサイクルを特別な配線なしで進められるようにします:

- `POST /v1/sorafs/deal/usage` は `DealUsageReport` のテレメトリを受け取り、
  決定的な会計結果 (`UsageOutcome`) を返します。
- `POST /v1/sorafs/deal/settle` は現在のウィンドウを確定し、結果の
  `DealSettlementRecord` と、ガバナンス DAG 公開用に base64 エンコードされた
  `DealSettlementV1` をストリームします。
- Torii の `/v1/events/sse` フィードは、各使用量提出を要約する
  `SorafsGatewayEvent::DealUsage`（epoch、計測 GiB時間、チケットカウンタ、決定的課金）、
  正規の精算台帳スナップショットとオンディスクのガバナンス成果物に対する
  BLAKE3 digest/size/base64 を含む `SorafsGatewayEvent::DealSettlement`、および
  PDP/PoTR の閾値超過時に発報する `SorafsGatewayEvent::ProofHealth`
  （プロバイダ、ウィンドウ、strike/cooldown 状態、ペナルティ額）を配信します。
  利用者はプロバイダ単位でフィルタし、ポーリングなしで新しいテレメトリ、精算、
  proof 健康アラートに反応できます。

両エンドポイントは新しい `torii.sorafs.quota.deal_telemetry` ウィンドウを通じて
SoraFS のクォータフレームワークに参加しており、デプロイごとの許可送信率を
オペレーターが調整できます。
