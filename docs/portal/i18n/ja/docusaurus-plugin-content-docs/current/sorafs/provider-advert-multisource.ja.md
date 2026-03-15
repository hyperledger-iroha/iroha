---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/provider-advert-multisource.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 28f14d7796bb33fea2240d4ba9e2fbba9a54dd85cf732a31a245aa9b9daafb59
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: provider-advert-multisource
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


# マルチソースのプロバイダー広告とスケジューリング

このページは、次の文書にある正規仕様を要約する。
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)。
Norito スキーマの原文や変更履歴はそちらを参照し、ポータル版は運用ガイダンス、
SDK メモ、テレメトリ参照を SoraFS の他の runbook に近い場所で整理する。

## Norito スキーマの追加

### レンジ能力 (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – 1 リクエストあたりの最大連続スパン (bytes)、`>= 1`。
- `min_granularity` – seek の分解能、`1 <= 値 <= max_chunk_span`。
- `supports_sparse_offsets` – 1 リクエストで非連続オフセットを許可する。
- `requires_alignment` – true の場合、オフセットは `min_granularity` に整列する必要がある。
- `supports_merkle_proof` – PoR witness のサポートを示す。

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` は正規エンコードを強制し、
ゴシップ payload が決定論的であることを保証する。

### `StreamBudgetV1`
- フィールド: `max_in_flight`, `max_bytes_per_sec`, 任意の `burst_bytes`。
- 検証ルール (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`。
  - `burst_bytes` がある場合、`> 0` かつ `<= max_bytes_per_sec`。

### `TransportHintV1`
- フィールド: `protocol: TransportProtocol`, `priority: u8` (0-15 の範囲を
  `TransportHintV1::validate` が強制)。
- 既知のプロトコル: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`。
- プロバイダーごとの重複プロトコルは拒否される。

### `ProviderAdvertBodyV1` の追加
- 任意の `stream_budget: Option<StreamBudgetV1>`。
- 任意の `transport_hints: Option<Vec<TransportHintV1>>`。
- 両フィールドは `ProviderAdmissionProposalV1`、ガバナンス envelope、CLI fixtures、
  テレメトリ JSON を通して流れる。

## 検証とガバナンス結合

`ProviderAdvertBodyV1::validate` と `ProviderAdmissionProposalV1::validate` は
不正なメタデータを拒否する:

- レンジ能力はデコードでき、スパン/粒度の制限を満たす必要がある。
- stream budgets / transport hints は `CapabilityType::ChunkRangeFetch` の TLV と
  空ではない hint リストが必須。
- 重複プロトコルや不正な優先度は、adverts をゴシップする前に検証エラーとなる。
- admission envelope は `compare_core_fields` で proposal/adverts のレンジメタデータを比較し、
  不一致のゴシップ payload を早期に拒否する。

回帰カバレッジは
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs` にある。

## ツールと fixtures

- プロバイダー advert の payload は `range_capability`、`stream_budget`、`transport_hints`
  を含める必要がある。`/v2/sorafs/providers` の応答と admission fixtures で検証し、
  JSON サマリには解析済みの能力、stream budget、hint 配列を含めてテレメトリ取り込みを行う。
- `cargo xtask sorafs-admission-fixtures` は JSON artefact に stream budget と transport hints を出力し、
  ダッシュボードが機能採用を追跡できるようにする。
- `fixtures/sorafs_manifest/provider_admission/` 配下の fixtures に以下が含まれる:
  - 正規のマルチソース advert、
  - SDK suites が決定論的な multi-peer fetch 計画を再生するための `multi_fetch_plan.json`。

## オーケストレータと Torii の統合

- Torii `/v2/sorafs/providers` は解析済みレンジ能力メタデータと `stream_budget`、`transport_hints`
  を返す。新しいメタデータを欠くプロバイダーにはダウングレード警告が発火し、
  gateway のレンジエンドポイントは直接クライアントにも同じ制約を適用する。
- マルチソース・オーケストレータ (`sorafs_car::multi_fetch`) は、レンジ制限、能力整合、
  stream budgets を仕事割り当て時に強制する。ユニットテストは chunk が大きすぎる場合、
  sparse seek、スロットリングのシナリオをカバーする。
- `sorafs_car::multi_fetch` はダウングレード信号 (整列失敗、スロットリングされたリクエスト)
  をストリームし、オペレーターが計画時に特定のプロバイダーが省かれた理由を追跡できる。

## テレメトリ参照

Torii のレンジ fetch 計測は **SoraFS Fetch Observability** の Grafana ダッシュボード
(`dashboards/grafana/sorafs_fetch_observability.json`) と対応するアラートルール
(`dashboards/alerts/sorafs_fetch_rules.yml`) を支える。

| メトリクス | 種別 | ラベル | 説明 |
|-----------|------|--------|------|
| `torii_sorafs_provider_range_capability_total` | Gauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | レンジ能力の機能を広告するプロバイダー数。 |
| `torii_sorafs_range_fetch_throttle_events_total` | Counter | `reason` (`quota`, `concurrency`, `byte_rate`) | ポリシーごとのレンジ fetch スロットリング試行数。 |
| `torii_sorafs_range_fetch_concurrency_current` | Gauge | — | 共有並行性予算を消費するアクティブな保護ストリーム数。 |

PromQL の例:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

スロットリングカウンタでクォータ適用を確認してからマルチソースの
オーケストレータのデフォルトを有効化し、並行性がフリートの stream budget 上限に
近づいたらアラートを出す。
