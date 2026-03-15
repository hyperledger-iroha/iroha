---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ملٹی سورس پرووائیڈر 広告 اور شیڈولنگ

正規仕様の標準仕様:
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)。
Norito スキーマ 変更履歴 ٩ے لیے اسی دستاویز کو حرف بہ حرف استعمال کریں؛ポータル
SDK の開発と開発 SoraFS のランブックの開発୨୧୨୧୨୧

## Norito スキーマ

### 範囲機能 (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – スパン (バイト) `>= 1`。
- `min_granularity` – `1 <= قدر <= max_chunk_span` を検索します。
- `supports_sparse_offsets` – オフセットのオフセット
- `requires_alignment` – 本当のオフセット `min_granularity` 位置合わせ ہونا لازم ہے۔
- `supports_merkle_proof` – PoR 証人 سپورٹ ظاہر کرتا ہے۔

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` 正規エンコーディング
ゴシップのペイロードが決定的になる

### `StreamBudgetV1`
- 番号: `max_in_flight`、`max_bytes_per_sec`、`burst_bytes`。
- 検証ルール (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`、`max_bytes_per_sec > 0`。
  - `burst_bytes` موجود ہو تو `> 0` اور `<= max_bytes_per_sec` ہونا چاہیے۔

### `TransportHintV1`
- 番号: `protocol: TransportProtocol`、`priority: u8` (0-15 秒)
  `TransportHintV1::validate` فافذ کرتا ہے)。
- プロトコル: `torii_http_range`、`quic_stream`、`soranet_relay`、
  `vendor_reserved`。
- プロバイダー ڈپلیکیٹ プロトコル エントリ مسترد کی جاتی ہیں۔

### `ProviderAdvertBodyV1` میں اضافے
- `stream_budget: Option<StreamBudgetV1>`。
- `transport_hints: Option<Vec<TransportHintV1>>`。
- 管理 `ProviderAdmissionProposalV1` ガバナンス エンベロープ、CLI フィクスチャ、テレメトリック JSON の管理

## 検証とガバナンスバインディング

`ProviderAdvertBodyV1::validate` `ProviderAdmissionProposalV1::validate`
メタデータの説明:

- 範囲機能、デコード、スパン/粒度制限、およびデコード機能
- ストリームの予算/トランスポートのヒント `CapabilityType::ChunkRangeFetch` TLV 空でないヒントのリスト
- トランスポート プロトコル、優先順位、ゴシップ、検証エラー、および検証エラー
- 入学封筒 `compare_core_fields` 提案/広告 範囲メタデータ 比較 ゴシップ ペイロード 不一致 ゴシップ ペイロード

回帰カバレッジ ٌہاں موجود ہے:
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`。

## ツーリング治具

- プロバイダー広告ペイロード `range_capability`、`stream_budget`、`transport_hints` ペイロード
  `/v2/sorafs/providers` 応答 入場設備を検証するJSON サマリー、解析機能、ストリーム予算、ヒント配列、テレメトリの取り込み、
- `cargo xtask sorafs-admission-fixtures` JSON アーティファクト ストリーム予算 輸送ヒント ダッシュボード機能 導入トラック
- `fixtures/sorafs_manifest/provider_admission/` のフィクスチャの数:
  - 正規のマルチソース広告
  - `multi_fetch_plan.json` SDK スイートの決定論的なマルチピア フェッチ プランの再生

## オーケストレーター Torii 認証

- Torii `/v2/sorafs/providers` 解析範囲機能メタデータ `stream_budget` 解析 `transport_hints` 解析済み範囲機能メタデータ
  プロバイダー メタデータ ダウングレード警告 ゲートウェイ範囲エンドポイント クライアント 制約 制限やあ
- マルチソース オーケストレーター (`sorafs_car::multi_fetch`) 範囲制限、機能調整、ストリーム予算、作業割り当て、強制適用単体テスト、チャンクが大きすぎる、スパース シーク、スロットル シナリオなど
- `sorafs_car::multi_fetch` ダウングレード シグナル (アライメントの失敗、スロットルされたリクエスト) ストリーム オペレーター オペレーター スキップ プロバイダー スキップفوئے۔

## テレメトリのリファレンス

Torii 範囲フェッチ インストルメンテーション **SoraFS フェッチ オブザーバビリティ** Grafana ダッシュボード
(`dashboards/grafana/sorafs_fetch_observability.json`) アラート ルール
(`dashboards/alerts/sorafs_fetch_rules.yml`) フィード ہے۔

|メトリック |タイプ |ラベル |説明 |
|----------|------|----------|---------------|
| `torii_sorafs_provider_range_capability_total` |ゲージ | `feature` (`providers`、`supports_sparse_offsets`、`requires_alignment`、`supports_merkle_proof`、`stream_budget`、`transport_hints`)範囲機能機能によりプロバイダーを宣伝します|
| `torii_sorafs_range_fetch_throttle_events_total` |カウンター | `reason` (`quota`、`concurrency`、`byte_rate`) |ポリシーによる範囲フェッチ試行の制限|
| `torii_sorafs_range_fetch_concurrency_current` |ゲージ | — |共有同時実行バジェット (アクティブな保護されたストリーム) |

PromQL スニペットの例:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

クォータの適用 スロットリング カウンター スロットリング カウンター マルチソース オーケストレーターのデフォルト 同時実行性艦隊のストリーム予算最大値の警告アラート