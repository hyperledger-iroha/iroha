---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 複数の起源と計画の証明に関する発表

特定の目的を達成するためのページ
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)。
米国のドキュメント パラ ロス エスケマス Norito は変更ログを逐語的に記録しています。ラ コピア デル ポータル
Mantiene la guía de operadores、las notas de SDK、las referencias de telemetria cerca delresto
SoraFS の Runbook です。

## アディシオネス アル エスケマ Norito

### ランゴ大作戦 (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – 市長の連絡先 (バイト) の要請、`>= 1`。
- `min_granularity` – タスクの解決策、`1 <= valor <= max_chunk_span`。
- `supports_sparse_offsets` – 要求された連続性のないオフセットを許可します。
- `requires_alignment` – cuando es true、los offsets deben alinearse con `min_granularity`.
- `supports_merkle_proof` – インドの検査機関 PoR。

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` 正規のコード
ゴシップ情報のペイロードを決定する必要があります。

### `StreamBudgetV1`
- Campos: `max_in_flight`、`max_bytes_per_sec`、`burst_bytes` オプション。
- 検証規則 (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`、`max_bytes_per_sec > 0`。
  - `burst_bytes`、クアンド・エスタ・プレゼンテ、デベ・サー `> 0` y `<= max_bytes_per_sec`。

### `TransportHintV1`
- Campos: `protocol: TransportProtocol`、`priority: u8` (ベンタナ 0-15 アプリケーション ポート
  `TransportHintV1::validate`)。
- プロトコル: `torii_http_range`、`quic_stream`、`soranet_relay`、
  `vendor_reserved`。
- 証明するプロトコルの複製を確認してください。

### アディシオネス a `ProviderAdvertBodyV1`
- `stream_budget` オプション: `Option<StreamBudgetV1>`。
- `transport_hints` オプション: `Option<Vec<TransportHintV1>>`。
- Ambos Campos ahora fluyen por `ProviderAdmissionProposalV1`、ロス・エンベロープ・デ・ゴベルナンザ、
  CLI と JSON テレメトリコのフィクスチャが失われます。

## 有効性と有効性を検証

`ProviderAdvertBodyV1::validate` y `ProviderAdmissionProposalV1::validate`
不正なメタデータのレチャザン:

- ランゴデベンの暗号化とスパン/粒度の制限に関する制限。
- TLV `CapabilityType::ChunkRangeFetch` に必要なロス ストリーム バジェット/トランスポート ヒント
  偶然の一致とリストのヒントはありません。
- 重複した輸送プロトコルと検証エラーの無効化
  うわさ話をする前に、広告を掲載してください。
- ランゴ経由のメタデータを比較した提案/広告の封筒
  `compare_core_fields` ゴシップのペイロードは、一時的なものです。

人生の回復期
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`。

## ツーリングと治具

- 証明者または通知のペイロード損失にはメタデータ `range_capability` が含まれます。
  `stream_budget` と `transport_hints`。 `/v1/sorafs/providers` y のレスキューを介して検証
  入場設備。 los resúmenes JSON deben incluir la capacidad parseada、
  ストリームの予算と、テレメトリの取り込みに関するヒントの配列。
- `cargo xtask sorafs-admission-fixtures` ストリーム バジェットとトランスポート ヒントを説明します
  ダッシュボードにある JSON の成果物は、機能を採用しています。
- ロス備品バホ `fixtures/sorafs_manifest/provider_admission/` アホラに含まれるもの:
  - 複数の起源を持つカノニコスの広告、
  - 別のバージョンのダウングレード パラ プルエバスを実行します。
  - `multi_fetch_plan.json` フェッチの計画を立てて SDK を再生産できるパラケラ
    マルチピア決定者。

## 統合コントラクターと Torii

- Torii `/v1/sorafs/providers` ランゴ解析の容量のメタデータを取得します
  `stream_budget` y `transport_hints`。クアンドロスのダウングレードに関する広告の表示
  新しいメタデータを省略し、ランゴ デル ゲートウェイ アプリケーションのエンドポイントを失うことを証明しました
  顧客の直接のミスマス制限。
- El orquestador multi-origen (`sorafs_car::multi_fetch`) アホラ・ハセ・クンプリル・リミテス
  ランゴ、容量の制限とストリームの予算を割り当ててください。ラス・プルエバス・ユニタリアス
  大量のデマシアドのシナリオ、分散処理やスロットルの処理。
- `sorafs_car::multi_fetch` はダウングレードのセニャレスを発行します (フォールス デ アラインアシオン、
  制限された注意) パラ・ケ・ロス・オペラドーレス・ラスティーン・ポル・ケ・セ・オミティエロン
  計画の期間に特有の証明。

## テレメトリアの参照

Grafana のダッシュボードから Torii を取得するための機器
**SoraFS フェッチ可観測性** (`dashboards/grafana/sorafs_fetch_observability.json`) y
アソシアダに関する規則 (`dashboards/alerts/sorafs_fetch_rules.yml`)。|メトリカ |ティポ |エチケット |説明 |
|-----------|------|-----------|-------------|
| `torii_sorafs_provider_range_capability_total` |ゲージ | `feature` (`providers`、`supports_sparse_offsets`、`requires_alignment`、`supports_merkle_proof`、`stream_budget`、`transport_hints`) Proveedores que anuncian features de capacidad de rango. |
| `torii_sorafs_range_fetch_throttle_events_total` |カウンター | `reason` (`quota`、`concurrency`、`byte_rate`)政治的アグルパドのスロットリングを目的としたフェッチ・デ・ランゴ。 |
| `torii_sorafs_range_fetch_concurrency_current` |ゲージ | — |コンパルティド デ コンカレンシアの消費者向けのプロテギドス アクティビティをストリームします。 |

PromQL の使い方:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

米国のスロットリングに関する確認は、実際の使用状況を確認するために行われます。
損失は、不正なオルケスタドールの多元性と警告、同意を得るために必要です
フロータの最大のストリームを確認します。