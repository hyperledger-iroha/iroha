---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/deal-engine.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ディールエンジン
title: Motor de acordos da SoraFS
サイドバーラベル: モーター・デ・アコルド
説明: Visao geral は、SF-8 モーター、Integracao com Torii テレメトリ管理機能を備えています。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/deal_engine.md`。 Mantenha ambos os locais alinhados enquanto a documentacao alternativa permanecer ativa.
:::

# SoraFS のモーター

ロードマップ SF-8 の紹介、モーター デ アコード、SoraFS、フォルネチェンドを追跡します。
安全性と回復性を考慮した確実な感染
クライアントと証明者。 OS ペイロード Norito
`crates/sorafs_manifest/src/deal.rs` の定義、コブリンド テルモス ド アコルド、
債券ブロック、確率的マイクロパガメント、液体レジストレーション。

おお、労働者よ、SoraFS (`sorafs_node::NodeHandle`) 瞬間を迎えてください
`DealEngine` は、ノードのプロセッサーをサポートします。 Oモーター：

- `DealTermsV1` を使用して登録を有効にします。
- XOR を使用したレプリカとレポートの累積コブランカス デノミナダ。
- アヴァリア・ジャネラス・デ・マイクロパガメント確率論的アメリカアモストラジェム決定論
  BLAKE3のベースアダ。 e
- 公共の液体の台帳とペイロードのスナップショットを作成します
  デ・ガバナンカ。

精巣ユニタリオス コブレム バリダカオ、マイクロパガメントスとリキッドダカオ パラの選択
API com confianca としての que operadores possam exercitar。リクイダコス アゴラ エミテム
`DealSettlementV1` のペイロード管理、パイプラインの管理
publicacao SF-12、シリーズ OpenTelemetry `sorafs.node.deal_*` の標準化
(`deal_settlements_total`、`deal_expected_charge_nano`、`deal_client_debit_nano`、
`deal_outstanding_nano`、`deal_bond_slash_nano`、`deal_publish_total`) パラ ダッシュボードは Torii e
SLO のアプリケーション。攻撃を自動化して攻撃を開始します
政治的政策の取り消しと意味の調整を監査します。

`sorafs.node.micropayment_*` のテレメトリを使用して、測定結果を確認する:
`micropayment_charge_nano`、`micropayment_credit_generated_nano`、
`micropayment_credit_applied_nano`、`micropayment_credit_carry_nano`、
`micropayment_outstanding_nano`、チケットのコントロール
(`micropayment_tickets_processed_total`、`micropayment_tickets_won_total`、
`micropayment_tickets_duplicate_total`)。 Esses totais expoem o fluxo de Loteria
ポッサムとマイクロパガメントの相関関係を示す確率論
クレジットの繰越は、液体の結果をもたらします。

## インテグラカオ コム Torii

Torii エンドポイントのデディカドス パラ ケ 証明を報告するレポートを使用してコンドゥザムを公開します
ciclo de vida do acordo semwiring sob medida:

- `POST /v2/sorafs/deal/usage` アセイタ テレメトリア `DealUsageReport` 電子レトルナ
  感染の確定的結果 (`UsageOutcome`)。
- `POST /v2/sorafs/deal/settle` ジャネラの最終結果、送信
  `DealSettlementRecord` 結果 junto com um `DealSettlementV1` em Base64
  すぐに政府の DAG を公開しません。
- O フィード `/v2/events/sse` do Torii アゴラ送信レジストリ `SorafsGatewayEvent::DealUsage`
  Resumindo cada envio de uso (エポック、GiB 時間のメディド、コンタドール デ チケット、
  コブランカス決定論)、レジストロス `SorafsGatewayEvent::DealSettlement`
  ダイジェスト/tamanho/base64 を含むスナップショット canonico の液体元帳が含まれています
  BLAKE3 はディスコやアラートを管理する `SorafsGatewayEvent::ProofHealth`
  semper que limiares PDP/PoTR sao excedidos (プローブ、ジャネラ、ストライク/クールダウン、
  勇気ある刑罰）。 Consumidores podem filtrar por Provedor para reagir a nova
  テレメトリア、液体センサー、および安全な証拠によるアラートの世論調査。

Ambos os エンドポイントは、nova janela 経由で SoraFS のフレームワークに参加します
`torii.sorafs.quota.deal_telemetry`、環境分類の調整を許可する
デプロイを許可します。