---
lang: ja
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-11-25T16:21:41.333147+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/compute_lane.md -->

# コンピュートレーン (SSC-1)

Compute Lane は決定論的な HTTP 風の呼び出しを受け付け、Kotodama の
エントリポイントにマップし、課金とガバナンスレビューのために
メータリング/レシートを記録する。この RFC は初回リリース向けに
マニフェストのスキーマ、呼び出し/レシートのエンベロープ、
サンドボックスのガードレール、構成デフォルトを固定する。

## マニフェスト

- スキーマ: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` は `1` に固定され、異なるバージョンのマニフェストは
  検証時に拒否される。
- 各ルートは以下を宣言する:
  - `id` (`service`, `method`)
  - `entrypoint` (Kotodama エントリポイント名)
  - codec の許可リスト (`codecs`)
  - TTL/gas/リクエスト/レスポンスの上限 (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - 決定論/実行クラス (`determinism`, `execution_class`)
  - SoraFS の ingress/モデル記述子 (`input_limits`, 任意の `model`)
  - 価格ファミリー (`price_family`) + リソースプロファイル (`resource_profile`)
  - 認証ポリシー (`auth`)
- サンドボックスのガードレールはマニフェストの `sandbox` ブロックにあり、
  すべてのルートで共有される（モード/乱数/ストレージ、非決定論的 syscall の拒否）。

例: `fixtures/compute/manifest_compute_payments.json`。

## コール、リクエスト、レシート

- スキーマ: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` は
  `crates/iroha_data_model/src/compute/mod.rs` に定義される。
- `ComputeRequest::hash()` は正準の request hash を生成する（ヘッダは決定論的な
  `BTreeMap` に保持され、ペイロードは `payload_hash` として持つ）。
- `ComputeCall` は namespace/route、codec、TTL/gas/レスポンス上限、リソース
  プロファイル + 価格ファミリー、認証（`Public` または UAID に紐づく
  `ComputeAuthn`）、決定論（`Strict` vs `BestEffort`）、実行クラスのヒント
  （CPU/GPU/TEE）、宣言された SoraFS 入力バイト/チャンク、任意のスポンサー
  予算、そして正準リクエストエンベロープを保持する。request hash はリプレイ
  防止とルーティングに使われる。
- ルートは任意の SoraFS モデル参照と入力制限（inline/chunk の上限）を
  埋め込める。マニフェストのサンドボックス規則が GPU/TEE ヒントをゲートする。
- `ComputePriceWeights::charge_units` はメータリングデータを cycles と egress
  バイトの切り上げ除算で課金用 compute units に変換する。
- `ComputeOutcome` は `Success`, `Timeout`, `OutOfMemory`, `BudgetExhausted`,
  `InternalError` を報告し、監査のためにレスポンスのハッシュ/サイズ/codec を
  追加で含められる。

例:
- Call: `fixtures/compute/call_compute_payments.json`
- Receipt: `fixtures/compute/receipt_compute_payments.json`

## サンドボックスとリソースプロファイル

- `ComputeSandboxRules` は実行モードをデフォルトで `IvmOnly` に固定し、
  request hash から決定論的乱数をシードし、SoraFS 読み取り専用アクセスを許可し、
  非決定論的 syscall を拒否する。GPU/TEE ヒントは `allow_gpu_hints`/`allow_tee_hints`
  でゲートされ、実行の決定論性を維持する。
- `ComputeResourceBudget` はプロファイルごとに cycles、線形メモリ、スタックサイズ、
  IO 予算、egress の上限を設定し、GPU ヒントと WASI-lite ヘルパーのトグルも持つ。
- デフォルトは `defaults::compute::resource_profiles` に `cpu-small` と `cpu-balanced`
  の 2 プロファイルを提供し、決定論的フォールバックを備える。

## 価格と課金単位

- 価格ファミリー (`ComputePriceWeights`) は cycles と egress バイトを compute units に
  変換する。デフォルトは `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` を
  料金とし、`unit_label = "cu"` を使用する。ファミリーはマニフェストの
  `price_family` で選択され、admission で強制される。
- メータリング記録は `charged_units` と raw の cycles/ingress/egress/duration 合計を
  持ち、突合に使う。課金は実行クラス/決定論の倍率 (`ComputePriceAmplifiers`) で
  増幅され、`compute.economics.max_cu_per_call` で上限を掛ける。egress は
  `compute.economics.max_amplification_ratio` でクランプされ、応答増幅を制限する。
- スポンサー予算 (`ComputeCall::sponsor_budget_cu`) は 1 回/日次の上限に対して
  強制され、課金 units は宣言済みスポンサー予算を超えてはならない。
- ガバナンスの価格更新は `compute.economics.price_bounds` のリスククラス境界と、
  `compute.economics.price_family_baseline` に記録されたベースラインファミリーを
  使う。`ComputeEconomics::apply_price_update` で差分を検証してからアクティブな
  ファミリーマップを更新する。Torii の設定更新は `ConfigUpdate::ComputePricing`
  を使い、kiso は同じ境界で適用してガバナンス編集の決定論性を保つ。

## 設定

新しい compute 設定は `crates/iroha_config/src/parameters` にある:

- User view: `Compute` (`user.rs`) と環境変数の上書き:
  - `COMPUTE_ENABLED` (デフォルト `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- Pricing/economics: `compute.economics` は
  `max_cu_per_call`/`max_amplification_ratio`、手数料分配、スポンサー cap
  （1 回/日次 CU）、価格ファミリーのベースライン + リスククラス境界、
  実行クラス倍率（GPU/TEE/best-effort）を含む。
- Actual/defaults: `actual.rs` / `defaults.rs::compute` が解析済みの `Compute`
  設定（namespaces、プロファイル、価格ファミリー、sandbox）を公開する。
- 不正な設定（空の namespaces、デフォルトのプロファイル/ファミリー不足、
  TTL 上限の逆転）はパース時に `InvalidComputeConfig` として報告される。

## テストとフィクスチャ

- 決定論的ヘルパー（`request_hash`, 価格計算）と fixture の往復は
  `crates/iroha_data_model/src/compute/mod.rs` にあり、`fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units` を参照。
- JSON フィクスチャは `fixtures/compute/` にあり、データモデルのテストで
  リグレッションをカバーする。

## SLO ハーネスと予算

- `compute.slo.*` 設定はゲートウェイの SLO ノブ（in-flight キュー深さ、
  RPS 上限、レイテンシ目標）を
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs` に露出する。
  デフォルト: in-flight 32、ルートあたり 512 queued、200 RPS、p50 25 ms、
  p95 75 ms、p99 120 ms。
- 軽量ベンチハーネスを実行して SLO サマリーと request/egress のスナップショットを
  取得する: `cargo run -p xtask --bin compute_gateway -- bench [manifest_path]
  [iterations] [concurrency] [out_dir]`（デフォルト:
  `fixtures/compute/manifest_compute_payments.json`, 128 iterations,
  concurrency 16, 出力先 `artifacts/compute_gateway/bench_summary.{json,md}`）。
  ベンチは決定論的ペイロード（`fixtures/compute/payload_compute_payments.json`）と
  リクエスト毎のヘッダを使用してリプレイ衝突を避けつつ、
  `echo`/`uppercase`/`sha3` エントリポイントを実行する。

## SDK/CLI パリティフィクスチャ

- 正準フィクスチャは `fixtures/compute/` 配下にあり、マニフェスト、コール、
  ペイロード、ゲートウェイ形式のレスポンス/レシートを含む。ペイロードハッシュは
  コールの `request.payload_hash` と一致する必要がある。ヘルパーペイロードは
  `fixtures/compute/payload_compute_payments.json` にある。
- CLI は `iroha compute simulate` と `iroha compute invoke` を提供する:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` は
  `javascript/iroha_js/src/compute.js` にあり、リグレッションテストは
  `javascript/iroha_js/test/computeExamples.test.js`。
- Swift: `ComputeSimulator` は同じフィクスチャを読み込み、ペイロードハッシュを検証し、
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift` で
  エントリポイントをシミュレーションする。
- CLI/JS/Swift の各ヘルパーは同じ Norito フィクスチャを共有し、SDK は
  起動中のゲートウェイに接続せずにリクエスト構築とハッシュ処理をオフラインで検証できる。
