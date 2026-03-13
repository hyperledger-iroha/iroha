---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-fee-model
タイトル: Mises a jour du modele de frais Nexus
説明: `docs/source/nexus_fee_model.md` のミロワール、レーンと和解の表面に関する記録。
---

:::note ソースカノニク
Cette ページは `docs/source/nexus_fee_model.md` を反映します。 Gardez les deux は、日本、ヘブライク、スペイン、ポルトガル、フランセーズ、ロシア、アラベ、およびウルドゥーの移民デトラダクションのアラインペンダントをコピーします。
:::

# Mises a jour du modele de frais Nexus

管理者は、Nexus のデビット・デ・ガス・アベック・ル・モデルを統合して、管理者が管理者と管理者との調整を調整します。

- ルート上の完全なアーキテクチャ、バッファーの政治、テレメトリのマトリックスと展開のシーケンス、`docs/settlement-router.md` を注ぎます。ガイドの説明コメントは、ロードマップ NX-3 の詳細なパラメータに関するドキュメントであり、SRE の監視や運用ルートのコメントも含まれています。
- ガス構成 (`pipeline.gas.units_per_gas`) (10 進数の `twap_local_per_xor`、`liquidity_profile` (`tier1`、`tier2`、ou `tier3`) などを含む) `volatility_class` (`stable`、`elevated`、`dislocated`)。 CES は、TWAP の正規のヘアカットとレーンへのヘアカットの XOR 対応を示します。
- トランザクションは `LaneSettlementReceipt` で登録されています。控訴者による特定情報源、マイクロモンタントローカル、レグラー即時のXOR、ヘアカット後のXOR出席、現実の差異(`xor_variance_micro`)、およびミリ秒単位のホロデータジュを調べます。
- `lane_settlement_commitments` と `/v2/sumeragi/status` を介して、レーン/データスペースに関するブロックの実行をまとめます。報告書は、`total_local_micro`、`total_xor_due_micro`、および `total_xor_after_haircut_micro` 追加のブロック レポートの夜想曲の和解をエクスポートします。
- 安全なコンソメのマルジェラ `total_xor_variance_micro` スーツ (ヘアカット後の XOR とアテンデュの差)、および `swap_metadata` コンバージョン決定パラメータ (TWAP、イプシロン、流動性プロファイル、ボラティリティ クラス) に関する文書監査者は、実行の構成を独立して検証するための主要な検証者です。

Les consommateurs peuvent suivre `lane_settlement_commitments` aux cotes des snapshots de commits de LANE et de dataspace pour verifier que les buffers de frais、les paliers de Haircut et l'execution du swapまず、au モデル Nexus を構成します。