---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# オーケストレーター SoraFS との関係

SDK のリリース確認のためのマルチフェッチ アゴラと監視の決定性の確認
ペイロードのバイト、チャンク受信、プロバイダーレポート、スコアボードの永続的な結果
実装します。 Cada ハーネス コンソームまたはバンドル canonico マルチプロバイダー em
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`、クエリ エンパコタ オブ プラノ SF1、プロバイダー メタデータ、テレメトリ スナップショット
opcos はオーケストレーターを行います。

## ベース錆び

- **コマンド:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **Escopo:** インプロセスのオーケストレーター経由で `MultiPeerFixture` デュアス ヴェゼスを実行、検証
  ペイロード モンタドのバイト数、チャンク レシート、プロバイダー レポート、およびスコアボードの結果。楽器
  作業セット (`max_parallel x max_chunk_length`) を参照してください。
- **パフォーマンス ガード:** CI ではハードウェアがないと結論付けるために、Cada を実行します。
- **作業セットの上限:** Com o perfil SF1 o harness aplica `max_parallel = 3`、結果は em uma
  ジャネラ <= 196608 バイト。

ログの例:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## SDK JavaScript を利用する

- **コマンド:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **Escopo:** `iroha_js_host::sorafsMultiFetchLocal` 経由のメスモ フィクスチャの再現、コンパランド ペイロード、
  レシート、プロバイダーレポート、スナップショットはスコアボードを連続して実行します。
- **パフォーマンス ガード:** 2 秒間でファイナライズを実行します。 o ハーネス インプライム a デュラソー メディダ e o
  バイト予約 (`max_parallel = 3`、`peak_reserved_bytes <= 196608`)。

レスモの例:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## SDK Swift のハーネス

- **コマンド:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **エスコポ:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` を定義するスイートを実行します。
  治具 SF1 デュアス ヴェゼス ペロ ブリッジ Norito (`sorafsLocalFetch`) を再生産します。ペイロードの検証バイトを利用し、
  チャンクの受信、プロバイダーのレポート、エントラダ、スコアボードの使用、メスマプロバイダーのメタデータの決定性
  Rust/JS スイートのテレメトリ スナップショット。
- **ブリッジ ブートストラップ:** ハーネスの圧縮解除 `dist/NoritoBridge.xcframework.zip` のすすり泣き要求、カレガのスライス macOS 経由
  `dlopen`。 xcframework を使用してバインディング SoraFS を実行し、フォールバック パラメタを実行します。
  `cargo build -p connect_norito_bridge --release` リンクカコントラ `target/release/libconnect_norito_bridge.dylib`、
  CI が必要なセットアップ マニュアルを入力してください。
- **パフォーマンス ガード:** ハードウェア デ CI なしでターミナルを実行するための Cada 実行。 o ハーネス インプライム a デュラソー メディダ e o
  バイト予約 (`max_parallel = 3`、`peak_reserved_bytes <= 196608`)。

レスモの例:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## バインディング Python のハーネス

- **コマンド:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **Escopo:** データクラスに関する詳細なラッパー `iroha_python.sorafs.multi_fetch_local` の演習
  フィクスチャーのカノニコ・パス・ペラ・メスマ API を使用してホイールを使用するための情報を提供します。精巣よ
  `providers.json` の一部のプロバイダー メタデータを再構成し、テレメトリ スナップショットと検証を挿入します
  ペイロードのバイト数、チャンク受信、プロバイダーレポート、および Rust/JS/Swift スイートと同様のスコアボードを実行します。
- **前提条件:** `_crypto` を実行して `maturin develop --release` (ホイールのインストール) を実行します。
  バインディング `sorafs_multi_fetch_local` アンテ・デ・チャマール pytest; o ハーネス se auto-ignora quando o バインディング
  ナオ・エスティバー・ディスポニベル。
- **パフォーマンス ガード:** Rust の場合は 2 秒以内。 pytest レジストラのバイト感染
  プロバイダーがリリースに参加するためのリソースを提供します。

ゲート開発キャプチャーのリリース、CAD ハーネス (Rust、Python、JS、Swift) パラメータの概要出力
ペイロードとプロモーターの均一な事前準備のメトリクスの比較レシートとの関係
ええと、ビルドします。 `ci/sdk_sorafs_orchestrator.sh` パラロッドをスイートとして実行します (Rust、Python)
バインディング、JS、Swift) em uma unica passada; CI 開発のアーティファト、ログ デッセ ヘルパーのアネクサーの作成
mais o `matrix.md` gerado (tabela SDK/ステータス/期間) リリース パラケ レビュアー ポッサムのチケット
監査は、ローカルスイートの再実行を監査します。