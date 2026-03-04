---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Отчет о паритете GA для SoraFS オーケストレーター

複数のマルチフェッチ機能を備えた SDK、чтобы を使用して、マルチフェッチを実行できます。
リリース エンジニアのメッセージ、バイト ペイロード、チャンク レシート、プロバイダー
レポートとスコアボード остаются согласованными между реализациями。
Каждый ハーネス использует канонический マルチプロバイダー バンドル из
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`、SF1 のような、
プロバイダーのメタデータ、テレメトリ スナップショット、およびオーケストレーター。

## Rust ベースライン

- **コマンド:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **スコープ:** Запускает план `MultiPeerFixture` дважды через インプロセス オーケストレーター、
  ペイロード バイト、チャンク レシート、プロバイダー レポートなどを確認できます。
  スコアボード。 Инструментация также отслеживает пиковую конкуренцию и эффективный
  ワーキングセット (`max_parallel × max_chunk_length`)。
- **パフォーマンス ガード:** CI ハードウェアに対する 2 秒間のセキュリティ。
- **作業セットの天井:** SF1 ハーネス применяет `max_parallel = 3`、
  давая окно ≤ 196608 バイト。

ログ出力の例:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK ハーネス

- **コマンド:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **範囲:** フィクスチャ `iroha_js_host::sorafsMultiFetchLocal` を参照してください。
  ペイロード、レシート、プロバイダーレポート、スコアボードのスナップショットを確認する
  そうです。
- **パフォーマンス ガード:** Каждый запуск должен заверøиться за 2 s;ハーネス
  予約済みバイト (`max_parallel = 3`、`peak_reserved_bytes ≤ 196 608`) です。

概要行の例:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK ハーネス

- **コマンド:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **スコープ:** パリティ スイート、`IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`、
  SF1 フィクスチャ Norito ブリッジ (`sorafsLocalFetch`)。ハーネス
  ペイロード バイト、チャンク レシート、プロバイダー レポート、エントリ スコアボード、используя ту же
  プロバイダーのメタデータ、テレメトリ スナップショット、Rust/JS スイートなど。
- **ブリッジ ブートストラップ:** ハーネス распаковывает `dist/NoritoBridge.xcframework.zip` по требованию и
  macOS スライス `dlopen`。 xcframework の SoraFS バインディング、
  フォールバック `cargo build -p connect_norito_bridge --release` と линковка с
  `target/release/libconnect_norito_bridge.dylib`、CI を参照してください。
- **パフォーマンス ガード:** CI ハードウェアに対する 2 秒間のセキュリティ。ハーネス
  予約済みバイト (`max_parallel = 3`、`peak_reserved_bytes ≤ 196 608`) です。

概要行の例:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python バインディング ハーネス

- **コマンド:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **スコープ:** 高レベルラッパー `iroha_python.sorafs.multi_fetch_local` および型付き
  データクラス、フィクスチャ モジュール、API、モジュール
  消費者の車輪。プロバイダーのメタデータ、`providers.json`、および
  テレメトリ スナップショット、ペイロード バイト、チャンク受信、プロバイダー レポートなど
  スコアボードは Rust/JS/Swift に対応しており、スイートもサポートしています。
- **事前要件:** Запустите `maturin develop --release` (ホイール)、чтобы `_crypto`
  バインディング `sorafs_multi_fetch_local` pytest;ハーネス、
  когда binding недоступен.
- **パフォーマンス ガード:** 2 秒以内、Rust スイート; pytest число
  バイト数とプロバイダーのリリース アーティファクトの概要。

リリース ゲートの概要出力ハーネス (Rust、Python、JS、Swift)、чтобы
ペイロードの受信を確認するためのメッセージを受信します。
構築します。 `ci/sdk_sorafs_orchestrator.sh`、パリティ スイートをサポート
(Rust、Python バインディング、JS、Swift) CI アーティファクトのログの抜粋
из этого ヘルパー и сгенерированный `matrix.md` (таблица SDK/ステータス/期間) とリリース チケット、
レビュー担当者は、パリティ行列を確認してください。