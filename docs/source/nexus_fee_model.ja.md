---
lang: ja
direction: ltr
source: docs/source/nexus_fee_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 532c57a0dae54224af0d30640edf8a3cbc8ac9a1df7d73b563bd16c3a635aec1
source_last_modified: "2026-01-08T19:45:50.411145+00:00"
translation_last_reviewed: 2026-01-08
---

# Nexus 手数料モデルの更新

統一 settlement router は、lane ごとの決定論的な receipts を記録するようになり、
オペレーターが gas のデビットを Nexus の手数料モデルに照合できるようになった。

- ルーターの完全なアーキテクチャ、バッファポリシー、テレメトリ行列、ロールアウト手順は
  `docs/settlement-router.md` を参照。ここでは、記載パラメータが NX-3 のロードマップ成果物に
  どう結び付くか、そして SRE が本番でルーターをどのように監視すべきかを説明している。
- gas asset 設定 (`pipeline.gas.units_per_gas`) には `twap_local_per_xor` の小数値、
  `liquidity_profile` (`tier1`, `tier2`, `tier3`)、`volatility_class` (`stable`, `elevated`,
  `dislocated`) が含まれる。これらのフラグが settlement router に入力され、
  lane の正規 TWAP と haircut tier に一致する XOR 見積りが得られる。
- IVM トランザクションは手数料の露出を抑えるために `gas_limit` (`u64`) メタデータを含める必要がある。
  `/v1/contracts/call` エンドポイントは `gas_limit` を明示的に要求し、無効な値は拒否される。
- `fee_sponsor` メタデータを設定する場合、スポンサーは呼び出し元に
  `CanUseFeeSponsor { sponsor }` を付与する必要がある。未許可のスポンサーシップ試行は拒否され記録される。
- gas を支払う各トランザクションは `LaneSettlementReceipt` を記録する。各 receipt は、
  呼び出し元が指定した source identifier、ローカルの micro-amount、即時に支払う XOR、
  haircut 後に期待される XOR、実現された safety margin (`xor_variance_micro`)、
  そしてミリ秒単位のブロックタイムスタンプを保持する。
- ブロック実行は lane/dataspace ごとに receipts を集計し、`/v1/sumeragi/status` の
  `lane_settlement_commitments` に公開する。合計には `total_local_micro`,
  `total_xor_due_micro`, `total_xor_after_haircut_micro` が含まれ、夜間の reconciliation
  exports 用にブロック単位で合算される。
- 新しい `total_xor_variance_micro` カウンタは消費された safety margin を追跡する
  (due XOR と post-haircut 期待値の差)。`swap_metadata` は決定論的な変換パラメータ
  (TWAP, epsilon, liquidity profile, volatility_class) を記録し、監査人が runtime 設定に
  依存せず見積り入力を検証できるようにする。

利用者は既存の lane/dataspace commitment snapshots とあわせて `lane_settlement_commitments`
を監視し、fee buffers、haircut tiers、swap 実行が設定済みの Nexus 手数料モデルと
一致することを確認できる。
