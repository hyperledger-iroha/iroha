---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-fee-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-fee-model
title: Nexus料金モデルの更新
description: `docs/source/nexus_fee_model.md` のミラーで、lane決済レシートと照合サーフェスを文書化します。
---

:::note 正本
このページは `docs/source/nexus_fee_model.md` を反映しています。日本語、ヘブライ語、スペイン語、ポルトガル語、フランス語、ロシア語、アラビア語、ウルドゥー語の翻訳が移行するまで両方のコピーを揃えてください。
:::

# Nexus料金モデルの更新

統合決済ルーターは、オペレーターがNexus料金モデルに対してガスのデビットを照合できるよう、laneごとの決定的なレシートを収集するようになりました。

- ルーターの全体アーキテクチャ、バッファポリシー、テレメトリ行列、ロールアウト順序については `docs/settlement-router.md` を参照してください。このガイドは、ここで示すパラメータがNX-3のロードマップ成果物にどう結びつくか、そしてSREが本番でどのようにルーターを監視すべきかを説明します。
- ガス資産の設定 (`pipeline.gas.units_per_gas`) には、`twap_local_per_xor` の小数値、`liquidity_profile` (`tier1`, `tier2`, `tier3`)、および `volatility_class` (`stable`, `elevated`, `dislocated`) が含まれます。これらのフラグが決済ルーターに渡され、結果のXOR見積もりが正規のTWAPとlaneのhaircut階層に一致します。
- ガスを支払う各トランザクションは `LaneSettlementReceipt` を記録します。各レシートには、呼び出し元が提供したソース識別子、ローカルのマイクロ量、即時に支払うXOR、haircut後に期待されるXOR、実現した分散 (`xor_variance_micro`)、およびブロックのタイムスタンプ（ミリ秒）が保存されます。
- ブロック実行はlane/dataspaceごとにレシートを集計し、`/v2/sumeragi/status` の `lane_settlement_commitments` で公開します。合計値は `total_local_micro`、`total_xor_due_micro`、`total_xor_after_haircut_micro` をブロック単位で合算して夜間の照合エクスポートに提供します。
- 新しい `total_xor_variance_micro` カウンターは、安全マージンがどれだけ消費されたか（即時XORとhaircut後の期待値との差）を追跡します。`swap_metadata` は決定的な換算パラメータ（TWAP、epsilon、liquidity profile、volatility_class）を記録し、監査者が実行時設定とは独立に見積もり入力を検証できるようにします。

利用者は、既存のlane/dataspace commitmentスナップショットと並べて `lane_settlement_commitments` を監視し、料金バッファ、haircut階層、スワップ実行が構成済みのNexus料金モデルと一致することを確認できます。
