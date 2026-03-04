---
lang: ja
direction: ltr
source: docs/source/taikai_cache_hierarchy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c5de9b350eb15f31f54ec4f4eca4fb89ac89138b32deea1f60d90510dbf65974
source_last_modified: "2026-01-04T10:50:53.693968+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/taikai_cache_hierarchy.md -->

# Taikai キャッシュ階層

_ステータス: 完了（信頼性ゲート + exit hedging が稼働）_ — オーナー: SRE / Networking TL / DA Program  
ロードマップ項目: **SNNet-14 — Taikai キャッシュ階層 & QoS 強制**

Taikai キャッシュ階層は、SoraNet 配布パイロット向けの決定論的な多層ストレージ層を
提供します。`sorafs_orchestrator` クレートに本番品質のキャッシュを組み込み、
容量・保持・QoS の保証を設定可能にすることで SNNet-14 のバックログを支えます。

## Highlights

- **3 層キャッシュ (hot/warm/cold)** — 各ティアは独立した容量と保持設定を持ち、
  ライブセグメントは hot に、最近アクセスされたデータは warm に、アーカイブは cold に
  保存される。軽量 LRU 実装が昇格/降格と追い出しを管理する
  (`CacheEviction`, `CachePromotion`)。
- **QoS トークンバケット** — キャッシュは priority/standard/bulk のクラス別に
  決定論的なトークンバケット (`QosEnforcer`) を適用し、hot/warm/cold が一貫して
  受け入れ/昇格を行う。キューのディスパッチは shaping 予算を消費せず、
  シャードの信頼性ゲートがルーティングを担当する一方で、キャッシュ側の QoS が
  insert と promotion の負荷を安定化する。
- **信頼性ゲート** — シャード単位のサーキットブレーカは連続失敗で作動し
  （デフォルト 2 秒のオープンウィンドウ）、一貫ハッシュリング上の健全なシャードへ
  バッチを迂回させる。状態は gauge と failover カウンタで可視化され、JSON サマリや
  Grafana の両方に反映される。
- **Exit hedging** — Taikai のプルキューは orchestrator ラッパーから直接ヘッジ
  フェッチを駆動する。`hedge_after`（デフォルト 125ms、`TaikaiPullQueueConfig` で調整）
  を超えた in-flight バッチは 2 回目のフェッチが走り、元の要求は継続する。
  ヘッジのカウンタはキュースタッツと `taikai_cache_queue` サマリに反映される。
  この経路は `taikai_fetch_wrapper_hedges_overdue_batches` でカバーされる。
- **Instrumentation-first** — insert/fetch/QoS 操作は構造化された結果
  (`TaikaiCacheInsertOutcome`, `TaikaiCacheQueryOutcome`, `QosError`) を返し、
  orchestrator は追い出し理由や昇格経路、レート制限イベントをテレメトリに反映できる。
- **Consistent hashing** — `TaikaiShardRing` は仮想ノードを含む一貫ハッシュを
  提供する。SNNet-14 の CAA gossip ネットワークがオンラインになると、同じリングで
  シャード割り当てと再バランスを行う。
- **CAA gossip レコード** — `CacheAdmissionRecord` + `CacheAdmissionEnvelope` が
  hot/warm/cold の入退を記録し、決定論的な payload digest と TTL メタデータを持ち、
  guard-directory キーで署名される。gossip 通知は `CacheAdmissionGossipBody`/
  `CacheAdmissionGossip` に nonce + 独立 TTL を付与し、`CacheAdmissionReplayFilter`
  が決定論的 replay ウィンドウを強制する（`crates/sorafs_orchestrator/src/taikai_cache.rs`）。
- **チャンク単位の Taikai ヒント** — `ChunkFetchSpec` はオプションの
  `taikai_segment_hint`（JSON + バイナリ）を公開し、Taikai manifests が CMAF セグメントに
  キュー経由の指示を付けられる。ヒントが存在する場合のみキューブリッジを有効化し、
  一般的な SoraFS フェッチへの影響を避ける。DA manifests は Taikai メタデータを
  fetch プランへ直接伝播するため、Torii が Taikai セグメントを返すと自動的に有効化される。
- **CAA 駆動の shard ring** — `CacheAdmissionTracker` が `CacheAdmissionGossip` を
  取り込み、replay ウィンドウを維持し、古いエントリを失効させ、pull-queue の
  shard ring を書き換えて hedging/failover を gossip に基づくトポロジに合わせる。

## Configuration

orchestrator は JSON bindings 内で `taikai_cache` を受け付ける。
設定は `TaikaiCacheConfig` を反映する:

```json
{
  "taikai_cache": {
    "hot_capacity_bytes": 8388608,
    "hot_retention_secs": 45,
    "warm_capacity_bytes": 33554432,
    "warm_retention_secs": 180,
    "cold_capacity_bytes": 268435456,
    "cold_retention_secs": 3600,
    "qos": {
      "priority_rate_bps": 83886080,
      "standard_rate_bps": 41943040,
      "bulk_rate_bps": 12582912,
      "burst_multiplier": 4
    }
  }
}
```

全フィールドはオプション。セクションを省略するとキャッシュは無効化され、
個別キーは `TaikaiCacheConfig::default` のチューニング値を使用する。
orchestrator は `Orchestrator::taikai_cache()` を通じてライブ handle を公開し、
SNNet-14 の CAA gossip、exit hedging、observability sampling を統合できる。

## CAA Gossip Plane (SNNet-14A)

キャッシュはティアへの入退時に決定論的な通知レコードを出力する。各通知は
`CacheAdmissionRecord`（セグメントキー、ティア、QoS クラス、payload digest/size、
issuer shard、発行/失効タイムスタンプ）で表現され、guard directory の ID、署名者、
分離署名を持つ `CacheAdmissionEnvelope` に包まれる。

```rust
let record = CacheAdmissionRecord::from_segment(
    TaikaiShardId(3),
    GuardDirectoryId::new("soranet/canary"),
    &cached_segment,
    CacheTierKind::Hot,
    CacheAdmissionAction::Admit,
    issued_unix_ms,
    Duration::from_secs(30),
)?;
let envelope = CacheAdmissionEnvelope::sign(record, guard_key_pair)?;
envelope.verify(now_unix_ms)?;
```

- レコードは TTL を強制し（`issued_unix_ms + ttl`）、古いエントリは自動的に拒否される。
- キャッシュ/オーケストレータが処理する前に署名検証が必要で、失敗は
  `CacheAdmissionError::InvalidSignature` として表面化する。
- Norito のシリアライズ/デシリアライズは gossip payload とストレージ形式の
  決定論性を維持する。
- gossip 通知は nonce + 独立 TTL を持つ `CacheAdmissionGossipBody` として包まれ、
  `CacheAdmissionGossip::sign/verify` を通る。`CacheAdmissionReplayFilter` は
  設定ウィンドウ内の重複を拒否し、リプレイ氾濫を防ぐ:

```rust
let gossip_body =
    CacheAdmissionGossipBody::new(envelope.clone(), issued_unix_ms, Duration::from_secs(15))?;
let gossip = CacheAdmissionGossip::sign(gossip_body, guard_key_pair)?;
gossip.verify(now_unix_ms)?;
let mut replay = CacheAdmissionReplayFilter::new(Duration::from_millis(500), 128)?;
if replay.observe(&gossip, now_unix_ms)? {
    // safe to process admission/eviction event
}
```

round-trip、失効処理、改ざん検出の統合テストは
`crates/sorafs_orchestrator/tests/taikai_cache.rs` にあり、将来の変更でも
フォーマットが決定論的に保たれることを保証する。

## Telemetry

計測は `iroha_telemetry` レジストリに統合され、既存の `sorafs.fetch.*` パネルと
並行してキャッシュ効率を追跡できる。以下の Prometheus 系列（OTLP ミラーあり）を出力する。

- `sorafs_taikai_cache_query_total{result="hit|miss",tier}` — hit/miss カウント
- `sorafs_taikai_cache_insert_total{tier}` — ティア別 insert イベント。`sorafs_taikai_cache_bytes_total{event="insert|hit",tier}` と組み合わせてバイトスループットを表示。
- `sorafs_taikai_cache_evictions_total{tier,reason}` — 容量/失効による追い出し
- `sorafs_taikai_cache_promotions_total{from_tier,to_tier}` — warm/cold の昇格
- `sorafs_taikai_qos_denied_total{class}` — クラス別 QoS 拒否
- `sorafs_taikai_queue_depth{state}` — キュー深度の gauge（pending segments/bytes/batches、in-flight batches、open circuits）
- `sorafs_taikai_queue_events_total{event,class}` — issued/hedged/rate-limited/backpressure をクラス別に記録
- `sorafs_taikai_shard_circuits_open{shard}` と `sorafs_taikai_shard_failovers_total{preferred_shard,selected_shard}` — pull キューのサーキットブレーカ信頼性メトリクス

これらのメトリクスは SoraFS fetch observability pack
(`dashboards/grafana/sorafs_fetch_observability.json`) に表示され、SNNet-14
ロールアウト中の miss/brownout レートを監視できる。Taikai キャッシュ
ダッシュボード（`dashboards/grafana/taikai_cache.json`）には open circuits、
queue depth、hedging/rate-limit イベント、failover レートの専用パネルが追加され、
SRE は shard 不安定性と brownout を相関できる。

## Operator Controls & Rollout Overrides

### CLI (`sorafs_cli fetch`)

`sorafs_cli fetch` は `--taikai-cache-config=PATH` を受け付け、ベースの
orchestrator 設定を編集せずにキャッシュを有効化/調整できる。JSON ペイロードは
`TaikaiCacheConfig` に合わせた standalone か、`taikai_cache` セクションを含む
orchestrator JSON を使用できる。例:

```json
{
  "hot_capacity_bytes": 8388608,
  "hot_retention_secs": 45,
  "warm_capacity_bytes": 33554432,
  "warm_retention_secs": 180,
  "cold_capacity_bytes": 268435456,
  "cold_retention_secs": 3600,
  "qos": {
    "priority_rate_bps": 83886080,
    "standard_rate_bps": 41943040,
    "bulk_rate_bps": 12582912,
    "burst_multiplier": 4
  },
  "reliability": {
    "failures_to_trip": 3,
    "open_secs": 2
  }
}
```

実行例:

```bash
sorafs_cli fetch \
  --plan=fixtures/taikai_plan.json \
  --manifest-id=deadbeef... \
  --provider name=edge-a,provider-id=...,base-url=...,stream-token=... \
  --taikai-cache-config=configs/taikai-cache/hot-warm-warmup.json \
  --output=/tmp/payload.car
```

複数の JSON スニペットを配布すると、SRE はロールアウト段階（canary vs. ramp）ごとに
ティアサイズを固定できる。このオプションは
`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs` に接続されている。

信頼性フィールドは任意。省略すればデフォルト（3 回失敗でトリップ、2 秒オープン）を使用する。

fetch サマリは `taikai_cache_summary` ブロックを含み、Prometheus をスクレイプせずに
キャッシュ健全性を確認できる。例:

```json
"taikai_cache_summary": {
  "hits": {"hot": 12, "warm": 4, "cold": 0},
  "misses": 3,
  "inserts": {"hot": 16, "warm": 2, "cold": 1},
  "evictions": {
    "hot": {"expired": 0, "capacity": 5},
    "warm": {"expired": 0, "capacity": 1},
    "cold": {"expired": 0, "capacity": 0}
  },
  "promotions": {"warm_to_hot": 4, "cold_to_warm": 0, "cold_to_hot": 0},
  "qos_denials": {"priority": 1, "standard": 0, "bulk": 0}
}
```

キューのスナップショットは信頼性テレメトリを補完する:

```json
"taikai_cache_queue": {
  "pending_segments": 18,
  "pending_bytes": 9437184,
  "pending_batches": 4,
  "in_flight_batches": 2,
  "hedged_batches": 1,
  "dropped_segments": 0,
  "failovers": 1,
  "open_circuits": 1
}
```

このスナップショットとダッシュボードを併用し、miss バーストや QoS 拒否、
追い出し churn を相関してから transport ポリシーを切り替える。

### Coalesced pull/push queue

SNNet-14 は `TaikaiPullQueue` による統合 pull/push オーケストレーションの第一弾を
提供する。`taikai_cache` を有効化した orchestrator は以下を自動で準備する:

- **決定論的バッチング** — 入力 `SegmentKey`（QoS/サイズヒント付き）は
  シャード対応バッチ `TaikaiPullBatch` に統合され、キャッシュティアの制限を尊重する。
  `max_batch_segments`、`max_batch_bytes`、`max_in_flight_batches` のしきい値により
  upstream へのファンアウトが無制限にならない。
- **バックプレッシャーのフィードバック** — backlog が設定上限（デフォルト: 256 pending
  segments）を超えると `TaikaiQueueError::Backpressure` を即時返し、呼び出し側が
  負荷を削減するか一時的にキャッシュをバイパスできる。
- **Exit hedging** — `hedge_after`（デフォルト 125ms）を超えた in-flight バッチは
  `hedged=true` で再発行される。オーケストレータの fetch ラッパーはこの信号を
  尊重し、元の要求を維持したまま 2 回目のフェッチを競合させる。ヘッジカウンタは
  CLI/Jenkins の JSON サマリへ反映される。
- **サーキットブレーカ + failover** — 同一シャードへの 3 連続失敗で 2 秒の
  サーキットブレーカが作動。開放中は一致ハッシュリングの次の健全なシャードへ
  バッチを再割当し、開放状態（`sorafs_taikai_shard_circuits_open`）と failover
  経路（`sorafs_taikai_shard_failovers_total`）を記録する。
- **Shard ring の統合** — `TaikaiCacheHandle::configure_shards` は一貫ハッシュリングを
  そのまま置き換えられるため、CAA gossip の再バランスをキュー配線の変更なしで行える。
- **Observability フック** — `FetchSession` は `taikai_cache_summary` と
  `taikai_cache_queue` を捕捉し、`sorafs_cli --json-out` が更新ブロックを出力する。
  Grafana pack も同じフィールドを反映し、shaping カウンタが廃止された後も
  キュー深度や hedging 挙動、シャード健全性を可視化する。

#### Orchestrator 統合フック

`Orchestrator::taikai_cache_handle()` は内部 `TaikaiCacheHandle` のクローンを
返し、キャッシュとプルキューの安全なファサードを提供する。フェッチ経路は
`taikai_segment_hint`（Taikai manifests が出力する `ChunkFetchSpec` フィールド）
を検出した時にキューを直接使用する。ヒントが無い限りキューは idle だが、
現れた時点で通常の HTTP フェッチと並行してキューの発行/キャンセルを行う。
新しい helpers:

- `enqueue_pull(..)` は `TaikaiPullRequest` を追加し、backlog 超過時は
  `TaikaiQueueError::Backpressure` を返す。ロック破損は
  `TaikaiQueueError::Unavailable` を返し、直接フェッチへフォールバック可能。
- `issue_ready_batch[_at](..)` は次の `TaikaiPullBatch` を発行し、
  `hedge_overdue_batches[_at](..)` は `hedged=true` の重複バッチを生成する。
- `complete_batch(..)` は in-flight スロットを解放し、`fail_batch(..)` は
  シャードを不健康とみなしブレーカ/メトリクスを更新する。フェッチループは
  ブリッジ経由でこれらを自動処理するため、オペレーターは
  `taikai_segment_hint` を manifests に組み込むだけでキューを起動できる。
  `wrap_fetcher_with_taikai_queue` は Taikai タグ付きフェッチのチケット
  ライフサイクルと hedging を処理し、期限超過バッチで 2 回目のリクエストを
  競合させる。

オーケストレーション例:

```rust
if let Some(handle) = orchestrator.taikai_cache_handle() {
    let key = TaikaiSegmentKey::from_envelope(&segment_envelope);
    let request = TaikaiPullRequest::new(key, TaikaiQosClass::Priority, segment_bytes);
    if let Err(TaikaiQueueError::Backpressure { .. }) = handle.enqueue_pull(request) {
        downgrade_to_single_source();
    }

    if let Ok(Some(batch)) = handle.issue_ready_batch() {
        dispatch_to_primary_exit(batch.clone());
        for hedged in handle.hedge_overdue_batches().unwrap_or_default() {
            dispatch_to_secondary_exit(hedged);
        }
        let _ = handle.complete_batch(batch.id.into());
    }
}
```

Cache admission gossip は shard ring を直接制御する。`CacheAdmissionTracker`
（`Orchestrator::taikai_cache_tracker()` または
`Orchestrator::apply_cache_admission_gossip(...)`）が `CacheAdmissionGossip`
を検証し、replay/expiry ウィンドウを尊重し、一貫ハッシュリングを
書き換えるため、hedging と failover が CAA 平面のトポロジに従う。

### SDK overrides

JavaScript クライアントは `sorafsGatewayFetch(...)` に `taikaiCache` を渡せる。
TypeScript の表面は `SorafsTaikaiCacheOptions`/`SorafsTaikaiCacheQosOptions`
（`javascript/iroha_js/index.d.ts`）から公開され、Rust 構造体と同等。

```ts
await sorafsGatewayFetch(manifestId, chunker, planJson, providers, {
  taikaiCache: {
    hotCapacityBytes: 8_388_608,
    hotRetentionSecs: 45,
    warmCapacityBytes: 33_554_432,
    warmRetentionSecs: 180,
    coldCapacityBytes: 268_435_456,
    coldRetentionSecs: 3_600,
    qos: {
      priorityRateBps: 83_886_080,
      standardRateBps: 41_943_040,
      bulkRateBps: 12_582_912,
      burstMultiplier: 4,
    },
  },
});
```

Bindings は無効な（ゼロ/負）容量や burst multiplier を拒否し、SDK の強制が
CLI と対称になる（`javascript/iroha_js/src/sorafs.js`, `crates/iroha_js_host/src/lib.rs`）。

Swift アプリは `SorafsGatewayFetchOptions` の `taikaiCache` プロパティで
同じ payload を渡せる:

```swift
let cache = SorafsTaikaiCacheOptions(
    hotCapacityBytes: 8_388_608,
    hotRetentionSecs: 45,
    warmCapacityBytes: 33_554_432,
    warmRetentionSecs: 180,
    coldCapacityBytes: 268_435_456,
    coldRetentionSecs: 3_600,
    qos: SorafsTaikaiCacheQosOptions(
        priorityRateBps: 83_886_080,
        standardRateBps: 41_943_040,
        bulkRateBps: 12_582_912,
        burstMultiplier: 4
    )
)
let options = SorafsGatewayFetchOptions(taikaiCache: cache)
```

Python バインディング（`iroha_python.sorafs_gateway_fetch`）は、ネストした
マッピングで同じ構造を受け付ける:

```python
options = {
    "taikai_cache": {
        "hot_capacity_bytes": 8_388_608,
        "hot_retention_secs": 45,
        "warm_capacity_bytes": 33_554_432,
        "warm_retention_secs": 180,
        "cold_capacity_bytes": 268_435_456,
        "cold_retention_secs": 3_600,
        "qos": {
            "priority_rate_bps": 83_886_080,
            "standard_rate_bps": 41_943_040,
            "bulk_rate_bps": 12_582_912,
            "burst_multiplier": 4,
        },
    },
}
```

これらの overrides は Norito ブリッジ
（`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift:4`）と Python gateway helper
（`python/iroha_python/iroha_python_rs/src/lib.rs:2133`）を経由し、全 SDK が
手動 JSON 配線なしで SNNet-14 ロールアウトに参加できる。

### Governance DAG & 段階的ロールアウト・チェックリスト

- 承認済みキャッシュプロファイルを governance DAG に保存し
  (`configs/taikai_cache/…`)、ロールアウト manifest
  (`docs/source/sorafs_governance_dag_plan.md`) から参照する。
- 各エントリには上記 JSON と署名済み Norito payload を含め、SoraNS
  relays が新パラメータを受け入れる前に provenance を検証できるようにする。
- 新プロファイルの昇格時に DAG CID を公開し、同じ JSON を incident/runbook ログ
  に添付して brownout drill に決定論的入力を提供する。

`configs/taikai_cache/profiles/` ディレクトリは canonical JSON 入力
（`balanced-canary.json`, `ramp-2026q2.json`）を提供する。`cargo xtask
sorafs-taikai-cache-bundle` を実行すると、プロファイルを
`artifacts/taikai_cache/<profile>/` 配下のガバナンスバンドルへ変換し、
`profile.json`、`cache_config.json`、Norito payload
（`profile.taikai_cache_profile.to`）、署名済み manifest 用メタデータ
（`profile.manifest.json`）を生成する。コマンドは
`artifacts/taikai_cache/index.json` も更新し、変更記録に最新 hash を簡単に
添付できるようにする。

### トラブルシューティング・プレイブック

1. `sorafs_cli fetch --json-out …` で `taikai_cache_summary` を確認し、miss バーストを
   監視してから単一ソース transport へフォールバックする。
2. `sorafs_taikai_cache_query_total{result="miss"}` と
   `sorafs_taikai_cache_evictions_total{reason="capacity"}` を監視し、スパイクが
   あれば DAG に記録されたティアサイズを再確認し、`--taikai-cache-config` を
   修正 JSON で再実行する。
3. SDK override を検証するため、取得 payload の Norito サマリと governance
   エントリを突き合わせる（CLI は両方のアーティファクトを
   `artifacts/sorafs_orchestrator/<stamp>/` に書き出す）。
4. 手動 override（CLI フラグまたは SDK オプション）を SNNet-14 の運用ログに
   24 時間以内に記録し、インシデント解消後は公開済み DAG プロファイルへ戻す。

## Next Steps

1. *(Done)* キャッシュ結果を orchestrator テレメトリ
   (`sorafs.fetch.*`) とダッシュボードに接続。
2. *(Done)* 統合 pull/push キューを実装し、queue stats を orchestrator/CLI に公開、
   hedging/back-pressure メトリクスをテレメトリに保持し、
   `CacheAdmissionTracker`/`Orchestrator::apply_cache_admission_gossip(...)` 経由で
   CAA gossip 平面へ接続。Taikai manifests は `taikai_segment_hint` を fetch
   プランへ直接伝播し、Torii が Taikai セグメントを返すと自動でキューが有効化される。
   サーキットブレーカと failover テレメトリ（SNNet-14C）は稼働中。
3. イベント単位のスライス制御 (R_hot / R_cold) と registry 統合を追加。
4. *(Done)* CLI 以外の SDK サーフェスでキャッシュ健全性と QoS カウンタを公開
   （JS/Python/Swift の convenience wrapper が `taikai_cache_summary` +
   `taikai_cache_queue` を gateway fetch レポートに含める）。

これらのフォローアップは `roadmap.md` の SNNet-14 で継続追跡される。
