---
lang: ja
direction: ltr
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T15:38:30.661606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# データ可用性の賃貸料およびインセンティブ ポリシー (DA-7)

_ステータス: 起草中 — 所有者: 経済 WG / 財務 / ストレージ チーム_

ロードマップ項目 **DA-7** では、すべての BLOB に明示的な XOR 建てのレントが導入されます
`/v1/da/ingest` に送信され、PDP/PoTR の実行に報酬を与えるボーナスと、
egress はクライアントをフェッチするために提供されます。このドキュメントでは初期パラメータを定義します。
データ モデル表現、および Torii で使用される計算ワークフロー、
SDK、および財務ダッシュボード。

## ポリシー構造

ポリシーは [`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs) としてエンコードされます。
データモデル内で。 Torii とガバナンス ツールはポリシーを永続化します。
Norito ペイロード: 家賃見積とインセンティブ台帳を再計算できるようにする
決定論的に。スキーマは 5 つのノブを公開します。

|フィールド |説明 |デフォルト |
|----------|---------------|----------|
| `base_rate_per_gib_month` | XOR は、保持期間ごとに GiB ごとに課金されます。 | `250_000` マイクロ XOR (0.25 XOR) |
| `protocol_reserve_bps` |プロトコルリザーブ（ベーシスポイント）に送られる家賃のシェア。 | `2_000` (20%) |
| `pdp_bonus_bps` |成功した PDP 評価ごとのボーナスの割合。 | `500` (5%) |
| `potr_bonus_bps` |成功した PoTR 評価ごとのボーナスの割合。 | `250` (2.5%) |
| `egress_credit_per_gib` |プロバイダーが 1 GiB の DA データを提供するときに支払われるクレジット。 | `1_500` マイクロ XOR |

すべてのベーシスポイント値は、`BASIS_POINTS_PER_UNIT` (10000) に対して検証されます。
ポリシーの更新はガバナンスを経由する必要があり、すべての Torii ノードは
`torii.da_ingest.rent_policy` 構成セクションを介したアクティブなポリシー
(`iroha_config`)。演算子は `config.toml` のデフォルトをオーバーライドできます。

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

CLI ツール (`iroha app da rent-quote`) は、同じ Norito/JSON ポリシー入力を受け入れます
そして、到達することなくアクティブな `DaRentPolicyV1` をミラーリングするアーティファクトを放出します。
Torii 状態に戻ります。取り込みの実行に使用されるポリシーのスナップショットを指定します。
引用は再現可能です。

### 永続的な家賃見積アーティファクト

`iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` を実行して、
画面上の概要と、きれいに印刷された JSON アーティファクトの両方を出力します。ファイル
レコード `policy_source`、インライン化された `DaRentPolicyV1` スナップショット、計算された
`DaRentQuote`、および派生 `ledger_projection` (シリアル化されたもの)
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) 財務ダッシュボードと台帳 ISI に適しています
パイプライン。 `--quote-out` がネストされたディレクトリを指している場合、CLI は
親が欠落しているため、オペレーターは次のような場所を標準化できます。
`artifacts/da/rent_quotes/<timestamp>.json` と他の DA 証拠バンドル。
アーティファクトを賃貸承認または調整の実行に添付して、XOR が実行されるようにします。
内訳 (基本賃料、リザーブ、PDP/PoTR ボーナス、および下りクレジット) は次のとおりです。
再現可能。 `--policy-label "<text>"` を渡して、自動的にオーバーライドします
派生した `policy_source` の説明 (ファイル パス、埋め込まれたデフォルトなど)
ガバナンスチケットやマニフェストハッシュなどの人が読めるタグ。 CLI によるトリミング
この値は空/空白のみの文字列を拒否するため、記録された証拠は
監査可能のままです。

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```台帳投影セクションは、DA 賃貸料台帳 ISI に直接入力します。
プロトコル リザーブ、プロバイダーの支払い、および
特注のオーケストレーション コードを必要とせずに、プルーフごとのボーナス プールを利用できます。

### 家賃台帳計画の作成

`iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition xor#sora` を実行します
永続的な家賃見積を実行可能な台帳転送に変換します。コマンド
埋め込まれた `ledger_projection` を解析し、Norito `Transfer` 命令を発行します
基本賃貸料を国庫に徴収し、準備金/プロバイダーをルーティングします
部分を分割し、支払者から直接 PDP/PoTR ボーナスプールに事前に資金を提供します。の
出力 JSON は見積メタデータをミラーリングするため、CI および財務ツールが推論できるようになります。
同じアーティファクトについて:

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

最後の `egress_credit_per_gib_micro_xor` フィールドにより、ダッシュボードと支払いが可能になります
スケジューラは、下り料金の払い戻しを、
スクリプトグルーでポリシー計算を再計算せずに引用します。

## 引用例

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

この見積もりは、Torii ノード、SDK、および財務レポート全体で再現可能です。
アドホックな計算ではなく、決定論的な Norito 構造体を使用します。オペレーターは次のことができます
JSON/CBOR エンコードされた `DaRentPolicyV1` をガバナンス提案に添付するか、レンタルします
監査により、特定の BLOB に対してどのパラメーターが有効であったかを証明します。

## ボーナスと積立金

- **プロトコル リザーブ:** `protocol_reserve_bps` は、裏付けとなる XOR リザーブに資金を提供します。
  緊急の再複製と大幅な返金。財務省はこのバケットを追跡します
  個別に元帳残高が設定されたレートと一致することを確認します。
- **PDP/PoTR ボーナス:** 証明評価が成功するたびに、追加のボーナスが得られます。
  支払いは `base_rent × bonus_bps` から派生します。 DA スケジューラがプルーフを発行するとき
  レシートにはベーシスポイントタグが含まれているため、インセンティブを再利用できます。
- **下りクレジット:** プロバイダーはマニフェストごとに提供される GiB を記録し、乗算します。
  `egress_credit_per_gib`、`iroha app da prove-availability` 経由で領収書を送信します。
  レンタル ポリシーにより、GiB あたりの金額がガバナンスと同期して維持されます。

## 操作の流れ

1. **取り込み:** `/v1/da/ingest` はアクティブな `DaRentPolicyV1` をロードし、家賃を見積もります
   BLOB サイズと保持期間に基づいて、引用符を Norito に埋め込みます。
   マニフェストします。送信者は家賃ハッシュを参照する声明に署名し、
   ストレージチケットID。
2. **会計:** 財務省取り込みスクリプトがマニフェストをデコードし、呼び出します
   `DaRentPolicyV1::quote`、家賃台帳 (基本家賃、準備金、
   ボーナス、および予想される下りクレジット）。記録された家賃との差異
   そして再計算された引用符は CI に失敗します。
3. **報酬の証明:** PDP/PoTR スケジューラが成功をマークすると、領収書が発行されます。
   マニフェスト ダイジェスト、証明の種類、およびから派生した XOR ボーナスが含まれます。
   ポリシー。ガバナンスは、同じ見積もりを再計算することで支払いを監査できます。
4. **下りの払い戻し:** フェッチ オーケストレーターは、署名された下りサマリーを提出します。
   Torii は GiB 数を `egress_credit_per_gib` で乗算し、支払いを発行します。
   家賃エスクローに対する指示。

## テレメトリTorii ノードは、次の Prometheus メトリクス (ラベル:
`cluster`、`storage_class`):

- `torii_da_rent_gib_months_total` — `/v1/da/ingest` によって引用される GiB 月。
- `torii_da_rent_base_micro_total` — 取り込み時に発生した基本賃貸料 (マイクロ XOR)。
- `torii_da_protocol_reserve_micro_total` — プロトコル予約寄与。
- `torii_da_provider_reward_micro_total` — プロバイダー側​​の家賃の支払い。
- `torii_da_pdp_bonus_micro_total` および `torii_da_potr_bonus_micro_total` —
  PDP/PoTR ボーナス プールは取り込み見積もりから調達されます。

経済ダッシュボードはこれらのカウンターに依存して、台帳 ISI、予備タップ、
および PDP/PoTR ボーナス スケジュールはすべて、それぞれに有効なポリシー パラメーターと一致します。
クラスターとストレージクラス。 SoraFS キャパシティ ヘルス Grafana ボード
(`dashboards/grafana/sorafs_capacity_health.json`) 専用パネルをレンダリングするようになりました
賃貸料の分配、PDP/PoTR ボーナスの発生、および GiB 月のキャプチャのために、
取り込みを確認するときに、Torii クラスターまたはストレージ クラスでフィルタリングする財務省
ボリュームとペイアウト。

## 次のステップ

- ✅ `/v1/da/ingest` レシートには `rent_quote` が埋め込まれ、CLI/SDK サーフェスには引用符が表示されます。
  基本賃料、リザーブシェア、PDP/PoTR ボーナスにより、提出者は事前に XOR 義務を確認できます。
  ペイロードをコミットしています。
- 家賃台帳を今後の DA 評判/注文帳フィードと統合します
  高可用性プロバイダーが正しい支払いを受け取っているかを証明するため。