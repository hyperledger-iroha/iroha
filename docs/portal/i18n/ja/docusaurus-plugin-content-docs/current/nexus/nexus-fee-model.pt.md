---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-fee-model
タイトル: Nexus のモデルを作成する
説明: Espelho de `docs/source/nexus_fee_model.md`、文書および液体の記録と会議の管理に関する文書。
---

:::note フォンテ カノニカ
エスタページナリフレテ`docs/source/nexus_fee_model.md`。マンテンハは、日本、ヘブライカ、エスパニョーラ、ポルトガル、フランス、ロシア、アラベス、ウルドゥー語の翻訳として、デュアス コピアス アリンハダス エンクアントとして使用されます。
:::

# Atualizacos は Nexus をモデル化します

私は、Nexus を実行するために、オペラドールの大統領公会議官のデビトス デ ガス コンポジットを決定します。

- ロテアドールの完全なアーキテクチャ、バッファーの政治、テレメトリーのマトリックスと展開のシーケンス、Veja `docs/settlement-router.md`。 Esse guia は、OS パラメータのドキュメントを詳細に説明し、ロードマップ NX-3 と COM OS SRE の開発、監視、開発を行っています。
- 10 進数の `twap_local_per_xor`、`liquidity_profile` (`tier1`、`tier2`、ou `tier3`) を含むガス (`pipeline.gas.units_per_gas`) の設定`volatility_class` (`stable`、`elevated`、`dislocated`)。 Esses sinalizadores alimentam or roteador de liquidacao para que a cotacao XOR resultantecorrenda ao TWAP canonico e ao tier de Haircut daレーン。
- Cada transacao que paga ガス登録 `LaneSettlementReceipt`。正確な情報、オリジェム フォルネシド ペロ シャマドールの識別子、マイクロ ヴァラー ローカル、XOR デビド イメディアタメンテ、XOR エスペラード アポス、ヘアカット、バリアント リアルリザダ (`xor_variance_micro`)、ミリセグンドスのタイムスタンプ。
- `lane_settlement_commitments` または `/v1/sumeragi/status` を介して、レーン/データスペースの集合データを公開します。ほとんどの情報は `total_local_micro`、`total_xor_due_micro`、`total_xor_after_haircut_micro` であり、輸出に関する報告書は報告されません。
- 新たなコンタドール `total_xor_variance_micro` ラストレイア クォント デ マージェム デ セグランカ フォイ コンスミダ (ディフェレンカ エントレ オ XOR デビド エ オ エスペラード pos-haircut)、`swap_metadata` ドキュメント オス パラメトロス デ コンバーサオ デターミニスティック (TWAP、イプシロン、流動性プロファイルvolatility_class) は、ランタイムの構成を独立して管理するための監査権限を持っています。

OS コンスミドール `lane_settlement_commitments` は、OS バッファの検証、OS 層のヘアカットの実行、Nexus 設定のスワップ対応を実行し、コミットメントのレーンとデータスペースの存在を確認します。