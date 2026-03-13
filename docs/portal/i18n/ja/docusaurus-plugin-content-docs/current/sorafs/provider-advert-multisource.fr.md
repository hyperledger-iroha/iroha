---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 複数の情報源と計画に関する通知

カノニク ダンスの仕様を要約したページ
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)。
スキーマ Norito のドキュメントと変更ログをそのまま利用します。ラ・コピー・デュ・ポルテイル
保管期間中、オペレータの委託、メモ、SDK およびテレメトリのリファレンスを保存します。
Runbook SoraFS。

## スキーマ Norito を調整します

### キャパシテ ド プラージュ (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – リクエストごとにグランデ プラージュ コンティグ (オクテット) を加えたもの、`>= 1`。
- `min_granularity` – 解像度のシーク、`1 <= valeur <= max_chunk_span`。
- `supports_sparse_offsets` – 要求された非連続オフセットを許可します。
- `requires_alignment` – 本当です。`min_granularity` のアライナーのオフセットです。
- `supports_merkle_proof` – インディケ・ラ・プライズ・アン・チャージ・デ・テモアンPoR。

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` アップリケント アン エンコード 正規化
ゴシップのペイロードを流し込んで決定します。

### `StreamBudgetV1`
- チャンピオン: `max_in_flight`、`max_bytes_per_sec`、`burst_bytes` オプション。
- 検証規則 (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`、`max_bytes_per_sec > 0`。
  - `burst_bytes`、今日は最高です、`> 0` と `<= max_bytes_per_sec`。

### `TransportHintV1`
- チャンピオン: `protocol: TransportProtocol`、`priority: u8` (フェネトレ 0-15 アップリケ パー)
  `TransportHintV1::validate`)。
- プロトコル connus: `torii_http_range`、`quic_stream`、`soranet_relay`、
  `vendor_reserved`。
- 四重奏のプロトコルの準備は、非常に重要な問題です。

### `ProviderAdvertBodyV1` を回避します
- `stream_budget` オプション: `Option<StreamBudgetV1>`。
- `transport_hints` オプション: `Option<Vec<TransportHintV1>>`。
- Les deux champs transitent desormais (`ProviderAdmissionProposalV1` 経由)、les enveloppes
  統治、設備 CLI および JSON テレメトリック。

## 検証と統治上の連絡

`ProviderAdvertBodyV1::validate` と `ProviderAdmissionProposalV1::validate`
Rejettent les metadonnees mal forees:

- プラージュの容量と、デコーダの制限およびプラージュ/グラニュラライトの制限について説明します。
- TLV `CapabilityType::ChunkRangeFetch` でストリーム バジェット/トランスポート ヒントが不足しています
  特派員はビデオ以外のヒントをリストします。
- 二重輸送プロトコルおよび優先順位は、生成されたエラーを無効にします
  広告の普及の前に検証を行います。
- 入場料の比較提案/広告は、プラージュ経由のメタドンネを注ぎます
  `compare_core_fields` ゴシップのペイロードが一致しないため、拒否されました。

回帰分析のクーベルチュール
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`。

## 備品と備品

- メタドン `range_capability` を含む 4 件のペイロードの報告、
  `stream_budget` および `transport_hints`。応答 `/v2/sorafs/providers` et les 経由で検証します
  入場の準備。容量解析、ストリーム予算を含む JSON を再開します
  テレメトリックの摂取に関するヒントを表します。
- `cargo xtask sorafs-admission-fixtures` ストリーム バジェットとトランスポート ヒントを公開します
  JSON のアーティファクトは、ダッシュボードの機能の採用に適しています。
- `fixtures/sorafs_manifest/provider_admission/` の付属品の付属品:
  - マルチソースの正規版の広告、
  - `multi_fetch_plan.json` フェッチの計画を立てずに SDK のスイートを注ぐ
    マルチピア決定論。

## 統合 avec l'orchestrateur et Torii

- Torii `/v2/sorafs/providers` renvoie les metadonnees de capacite de plage parsees avec
  `stream_budget` と `transport_hints`。ダウングレードの危険な警告の回避
  新しいメタドネの監視、およびゲートウェイのエンドポイントの監視
  クライアントの指示に従ってミームの制約を適用します。
- L'orchestrateur マルチソース (`sorafs_car::multi_fetch`) アップリケの制限を解除
  計画、能力の調整と、労働の影響に対する予算の配分。
  大量のトロップのシナリオをテストし、分散などを求める
  スロットリング。
- `sorafs_car::multi_fetch` ダウングレードの拡散 (アライメント調整、
  スロットルを要求します) 操作者が puissent トレーサーを注ぎ、確実に fournisseurs を要求します
  以降、ペンダントの計画は無視されます。

## テレメトリのリファレンス

Torii ダッシュボード Grafana のフェッチの計装
**SoraFS フェッチ可観測性** (`dashboards/grafana/sorafs_fetch_observability.json`) et
les regles d'alerte associees (`dashboards/alerts/sorafs_fetch_rules.yml`)。|メトリク |タイプ |エチケット |説明 |
|----------|------|---------------|-------------|
| `torii_sorafs_provider_range_capability_total` |ゲージ | `feature` (`providers`、`supports_sparse_offsets`、`requires_alignment`、`supports_merkle_proof`、`stream_budget`、`transport_hints`)プラージュ機能の詳細を確認します。 |
| `torii_sorafs_range_fetch_throttle_events_total` |カウンター | `reason` (`quota`、`concurrency`、`byte_rate`)政治的に花嫁を取得するための仮の提案。 |
| `torii_sorafs_range_fetch_concurrency_current` |ゲージ | — |予算と同意の部分に応じて活動をストリームします。 |

PromQL の例:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

アクティブなクォータのアプリケーションのスロットリング確認ツールを利用します
複数のソースからのデフォルトのオーケストラの評価、および警告の同意
投票資金を注ぎ込む予算の最大値を決定します。