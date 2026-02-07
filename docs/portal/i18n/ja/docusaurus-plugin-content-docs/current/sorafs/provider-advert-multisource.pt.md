---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 複数のオリジェムとアジェンダメントを証明する広告

Esta pagina 履歴書 a e specificacao canonica em
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)。
esse documento para schemas Norito 逐語的な変更ログを使用します。コピアドゥポータル
オペラの方向性を管理し、SDK とテレメトリの参照情報を保存します。
dos ランブック SoraFS。

## アディコエス アオ エスケマ Norito

### 範囲機能 (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - 必要な主要な連続スパン (バイト)、`>= 1`。
- `min_granularity` - 解決策、`1 <= valor <= max_chunk_span`。
- `supports_sparse_offsets` - 連続するオフセットを許可します。
- `requires_alignment` - Quando true、オフセット devem alinhar com `min_granularity`。
- `supports_merkle_proof` - インドはテストの PoR をサポートしています。

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` aplicam エンコーディング canonico
ゴシップのペイロードを永続的に決定します。

### `StreamBudgetV1`
- Campos: `max_in_flight`、`max_bytes_per_sec`、`burst_bytes` オプション。
- 有効期限 (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`、`max_bytes_per_sec > 0`。
  - `burst_bytes`、Quando presente、開発者 `> 0` e `<= max_bytes_per_sec`。

### `TransportHintV1`
- カンポス: `protocol: TransportProtocol`、`priority: u8` (ジャネラ 0-15 アプリケーション)
  `TransportHintV1::validate`)。
- プロトコルコンヘシド: `torii_http_range`、`quic_stream`、`soranet_relay`、
  `vendor_reserved`。
- 証明されたプロトコルの重複を記録します。

### アディコ a `ProviderAdvertBodyV1`
- `stream_budget` オプション: `Option<StreamBudgetV1>`。
- `transport_hints` オプション: `Option<Vec<TransportHintV1>>`。
- アンボス オス カンポス アゴラ パサム ポル `ProviderAdmissionProposalV1`、政府の封筒、
  CLI のフィクスチャとテレメトリの JSON。

## Validacao e vinculacao com ガバナンカ

`ProviderAdvertBodyV1::validate` e `ProviderAdmissionProposalV1::validate`
メタデータの不正な形式:

- 範囲機能は、範囲/粒度の限界を解読し、制限を決定します。
- ストリーム バジェット/トランスポート ヒント exigem um TLV `CapabilityType::ChunkRangeFetch`
  ナオ・ヴァジア特派員とヒントのリスト。
- 重複した輸送プロトコルと有効性を無効にする優先順位
  広告の前にセレムがうわさ話しました。
- 入場封筒は、提案書/広告のメタデータをさまざまな範囲で比較します。
  `compare_core_fields` ゴシップのペイロードは多岐にわたります。

回復の危機を乗り越える
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`。

## ツーリングと治具

- 広告や証明された開発者のペイロードにはメタデータ `range_capability` が含まれます。
  `stream_budget` と `transport_hints`。 `/v1/sorafs/providers` の返信経由で有効
  e 入場設備。 JSON 開発には、解析機能、ストリーム機能が含まれています
  予算とテレメトリの取り込みに関するヒントの配列。
- `cargo xtask sorafs-admission-fixtures` mostra ストリーム バジェットとトランスポート ヒント dentro
  JSON の優れた機能は、ダッシュボードに付属しています。
- 備品すすり泣き `fixtures/sorafs_manifest/provider_admission/` アゴラの付属品:
  - マルチオリジェムカノニコスの広告、
  - `multi_fetch_plan.json` フェッチ用の SDK 再生産パラメータ
    マルチピアの決定性。

## Integracao com オーケストレーター e Torii

- Torii `/v1/sorafs/providers` 範囲解析メタデータ ジュント コム
  `stream_budget` と `transport_hints`。ダウングレードの不均衡を回避するための手段
  証明は nova メタデータを省略し、エンドポイントの範囲はゲートウェイ アプリケーションとして実行されます。
  クライアントのディレトスを管理します。
- オーケストレーター マルチオリジェム (`sorafs_car::multi_fetch`) のアプリケーションの限界
  通信範囲、機能、ストリーム予算を考慮して調整します。単位
  チャンク ムイト グランデのコブレム シナリオ、スパース シークとスロットリングをテストします。
- `sorafs_car::multi_fetch` ダウングレード送信 (ファルハス デ アリンハメント、
  requisicoes throttled) パラ ケ オペラドール ラストレイム ポル ケ プローベドール エスペシコス
  フォルム・イノラドス・デュランテ・オ・プレーンジャメント。

## テレメトリ参照

ダッシュボード Grafana の範囲フェッチ Torii の計器
**SoraFS フェッチ可観測性** (`dashboards/grafana/sorafs_fetch_observability.json`) e
関連情報として (`dashboards/alerts/sorafs_fetch_rules.yml`)。

|メトリカ |ティポ |ラベル |説明 |
|----------|------|----------|----------|
| `torii_sorafs_provider_range_capability_total` |ゲージ | `feature` (`providers`、`supports_sparse_offsets`、`requires_alignment`、`supports_merkle_proof`、`stream_budget`、`transport_hints`)範囲機能の機能を発表します。 |
| `torii_sorafs_range_fetch_throttle_events_total` |カウンター | `reason` (`quota`、`concurrency`、`byte_rate`)政治的な観点から、範囲フェッチの範囲が制限される可能性があります。 |
| `torii_sorafs_range_fetch_concurrency_current` |ゲージ | - |コンコルレンシアの予算を管理するためのストリームを管理します。 |

PromQL の例:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```クォータを事前に確認するためにスロットリングを制御する機能を使用します。
AOS のデフォルトでは、オーケストレーターのマルチオリジェムとアラートがほぼ一致しています。
最大限のストリーム予算を実行します。