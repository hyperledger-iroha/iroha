---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-fee-model
タイトル: Nexus のタリファモデルの実際
説明: `docs/source/nexus_fee_model.md` の文書、清算書と調停の権限に関する文書。
---

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/nexus_fee_model.md`。日本、ヘブリオ、エスパニョール、ポルトガル、フランス、ルソ、アラブ、ウルドゥー語の移民を満天。
:::

# タリファスの実際のモデル Nexus

ルータ デ リキッドダシオン ユニフィカド アホラ キャップチュラ レシボス 決定事項 ポーレーン パラ ケ ロス オペラドール プエダン公会議ロス デビトス デ ガス コントラ エル モデルロ デ タリファス Nexus。

- ルーターの完全な建築、バッファーの政治、テレメトリーのマトリクス、およびセキュリティーの監視、`docs/settlement-router.md`。 NX-3 と SRE の監視、ルーターの生産に関する重要なロードマップに関する詳細な説明が記載されています。
- ガスの動作設定 (`pipeline.gas.units_per_gas`) には、10 進数の `twap_local_per_xor`、`liquidity_profile` (`tier1`、`tier2`、または `tier3`) が含まれます。 `volatility_class` (`stable`、`elevated`、`dislocated`)。ヘアカットの際のヘアカットの段階で、TWAP と一致する XOR 結果が一致します。
- `LaneSettlementReceipt` によるパガガス登録の登録。発信者を特定するアルマセナの情報、マイクロモントのローカル情報、XOR デビド メディア、XOR エスペラード デル ヘアカット、ラ バリアンサ リアルザダ (`xor_variance_micro`)、ミリセグンドスのマルカ デ ティエンポ デル ブロック。
- `lane_settlement_commitments` および `/v1/sumeragi/status` 経由で、レーン/データスペースの集合体が公開されます。損失の合計指数 `total_local_micro`、`total_xor_due_micro`、y `total_xor_after_haircut_micro` は、夜行性の輸出と和解のための非常に複雑なブロックです。
- 新しいコンタドール `total_xor_variance_micro` rastrea cuanto margen de seguridad se consumio (ヘアカット後のディフェレンシア エントレ エル XOR デビド y ラ 期待値)、y `swap_metadata` ドキュメンタ ロス パラメトロス デ コンバージョン決定論 (TWAP、イプシロン、流動性プロファイル、y volatility_class) は、実行時の構成を独立して検証するための監査を実行します。

観測者 `lane_settlement_commitments` は、タリファのバッファー、ヘアカットのヘアカット、スワップの一致と Nexus 設定のスナップショットの存在とコミットメントのレーンとデータスペースの確認を実行します。