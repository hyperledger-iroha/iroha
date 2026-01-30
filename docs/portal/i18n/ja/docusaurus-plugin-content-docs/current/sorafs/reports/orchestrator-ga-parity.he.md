---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a9cd324e053905fc60a0fca0111383928a7f97236b383805302f203b04fc1ad5
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SoraFS Orchestrator GA パリティレポート

決定論的な multi-fetch パリティは SDK ごとに追跡され、リリースエンジニアが
payload bytes、chunk receipts、provider reports、scoreboard の結果が実装間で
一致していることを確認できるようになった。各ハーネスは
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/` の canonical multi-provider
bundle を使用し、SF1 plan、provider metadata、telemetry snapshot、orchestrator
options を同梱している。

## Rust ベースライン

- **Command:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Scope:** in-process orchestrator で `MultiPeerFixture` plan を2回実行し、assembled
  payload bytes、chunk receipts、provider reports、scoreboard の結果を検証する。
  計測は peak concurrency と working-set の有効サイズ (`max_parallel x max_chunk_length`) も追跡する。
- **Performance guard:** CI ハードウェアで各実行が 2 s 以内に完了すること。
- **Working set ceiling:** SF1 プロファイルでは harness が `max_parallel = 3` を
  強制し、<= 196608 バイトのウィンドウになる。

サンプルログ出力:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK ハーネス

- **Command:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Scope:** 同じ fixture を `iroha_js_host::sorafsMultiFetchLocal` で再生し、連続した実行間で
  payloads、receipts、provider reports、scoreboard snapshots を比較する。
- **Performance guard:** 各実行は 2 s 以内に完了すること。harness は計測した duration と
  予約バイト上限 (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) を出力する。

サマリー行の例:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK ハーネス

- **Command:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **Scope:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` で定義された
  パリティスイートを実行し、Norito bridge (`sorafsLocalFetch`) 経由で SF1 fixture を2回再生する。
  harness は payload bytes、chunk receipts、provider reports、scoreboard エントリを、Rust/JS
  スイートと同じ決定論的 provider metadata と telemetry snapshots を使って検証する。
- **Bridge bootstrap:** harness は必要に応じて `dist/NoritoBridge.xcframework.zip` を展開し、
  `dlopen` で macOS スライスを読み込む。xcframework が無い、または SoraFS bindings を含まない
  場合は `cargo build -p connect_norito_bridge --release` にフォールバックして
  `target/release/libconnect_norito_bridge.dylib` にリンクするため、CI での手動セットアップは不要。
- **Performance guard:** 各実行は CI ハードウェアで 2 s 以内に完了すること。harness は計測した
  duration と予約バイト上限 (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) を出力する。

サマリー行の例:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python バインディング ハーネス

- **Command:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Scope:** 高レベルの `iroha_python.sorafs.multi_fetch_local` ラッパーと型付き dataclasses を使い、
  canonical fixture が wheel 利用者と同じ API を通るようにする。テストは `providers.json`
  から provider metadata を再構築し、telemetry snapshot を注入し、Rust/JS/Swift と同様に
  payload bytes、chunk receipts、provider reports、scoreboard の内容を検証する。
- **Pre-req:** pytest 実行前に `maturin develop --release` (または wheel をインストール) して
  `_crypto` が `sorafs_multi_fetch_local` binding を公開する必要がある。binding が無い場合
  harness は自動的にスキップされる。
- **Performance guard:** Rust スイートと同じ <= 2 s の予算。pytest は組み立てたバイト数と
  provider 参加サマリーを release artefact 用に記録する。

リリースゲートでは各 harness (Rust, Python, JS, Swift) の summary output を取得し、
アーカイブされたレポートが payload receipts とメトリクスを一様に比較できるようにする。
`ci/sdk_sorafs_orchestrator.sh` を実行すると各パリティスイート (Rust, Python bindings,
JS, Swift) を一度に走らせられる。CI アーティファクトはこの helper のログ抜粋と
生成された `matrix.md` (SDK/status/duration の表) をリリースチケットに添付し、
レビュー担当がローカルで再実行せずにパリティ行列を監査できるようにする。
