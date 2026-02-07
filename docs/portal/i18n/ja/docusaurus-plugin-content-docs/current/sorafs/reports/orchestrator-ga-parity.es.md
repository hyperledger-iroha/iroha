---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# オーケストレーター SoraFS GA デル パリダーのレポート

SDK によるマルチフェッチの決定性の確認
リリース エンジニアはペイロードのバイト数、チャンクの受信を確認します。
プロバイダーレポートとスコアボードの結果を永続的に表示
実装。 Cada ハーネス消費エルバンドルマルチプロバイダー canonico bajo
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`、ケ・エンパケタ・エル・プランSF1、
プロバイダーのメタデータ、テレメトリ スナップショット、オーケストレーターのオプション。

## Rust ベースライン

- **コマンド:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **範囲:** プロセス中のエル オーケストレーターを介した計画 `MultiPeerFixture` の実行、
  ペイロード アンサンブラドのバイト、チャンク レシート、プロバイダー レポートを検証します
  スコアボードの結果。ピコのコンカレンシア・タンビエン・ラストレア・インストルメント
  ワーキングセット (`max_parallel x max_chunk_length`) を有効にします。
- **パフォーマンス ガード:** ハードウェア CI で 2 秒間で Cada の排出が完了します。
- **作業セットの天井:** Con el perfil SF1 el ハーネス アプリケーション `max_parallel = 3`、
  ダンド ウナ ベンタナ <= 196608 バイト。

ログ出力の例:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK ハーネス

- **コマンド:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **範囲:** `iroha_js_host::sorafsMultiFetchLocal` 経由でエル ミスモ フィクスチャを再現します。
  コンパランドペイロード、レシート、プロバイダーレポート、およびスコアボードスナップショット全体
  連続排出。
- **パフォーマンス ガード:** 2 秒での Cada の排出と最終処理。エル・ハーネス・インプリメ・ラ
  デュラシオン メディア y エル テクノ デ バイト リザーバド (`max_parallel = 3`、`peak_reserved_bytes <= 196608`)。

概要行の例:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK ハーネス

- **コマンド:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **範囲:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` の定義済みファイルの出力、
  SF1 フィクスチャを再現し、ブリッジ Norito (`sorafsLocalFetch`) を移動します。エルハーネス
  ペイロードのバイト検証、チャンク受信、プロバイダー レポート、スコアボードのエントリの確認
  Missma プロバイダーのメタデータは、Rust/JS スイートのテレメトリ スナップショットを決定します。
- **ブリッジ ブートストラップ:** エル ハーネス ディスコンプライム `dist/NoritoBridge.xcframework.zip` バホ デマンダ y カーガ
  `dlopen` 経由で macOS をスライスします。 xcframework falta にバインディング SoraFS がありません。フォールバックがあります。
  `cargo build -p connect_norito_bridge --release` y リンクケア コントラ
  `target/release/libconnect_norito_bridge.dylib`、CI セットアップ マニュアル。
- **パフォーマンス ガード:** ハードウェア CI での 2 秒間の Cada の取り出しエル・ハーネス・インプリメ・ラ
  デュラシオン メディア y エル テクノ デ バイト リザーバド (`max_parallel = 3`、`peak_reserved_bytes <= 196608`)。

概要行の例:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python バインディング ハーネス

- **コマンド:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **スコープ:** `iroha_python.sorafs.multi_fetch_local` および SUS データクラスの詳細なラッパーの取り出し
  ティパダパラケエルフィクスチャカノニコフルヤポルラミスマAPIケ消費ロスホイール。エルテスト
  `providers.json` のプロバイダー メタデータを再構築し、テレメトリ スナップショットと検証を実行
  ペイロードのバイト数、チャンク受信、プロバイダーレポートとスコアボードの内容
  Rust/JS/Swift スイート。
- **事前要件:** Ejecuta `maturin develop --release` (ホイールの取り付け) para que `_crypto` exponga el
  バインディング `sorafs_multi_fetch_local` 呼び出し元の pytest;エル ハーネス SE オート サルタクアンド エル
  エスタに拘束力はありません。
- **パフォーマンス ガード:** エル ミスモ プレスプエスト <= 2 秒以内、Rust; pytest レジストラ エル コンテオ
  リリースに関連するプロバイダーの参加を再開するバイト数。

リリース ゲーティング デベ キャプチャ、サマリー出力、ハードウェア ハーネス (Rust、Python、JS、Swift) パラメータ
ペイロードとメトリクスの領収書を比較し、均一な形式でレポートを保存します。
プロムーバーを構築しません。 Ejecuta `ci/sdk_sorafs_orchestrator.sh` パラグラフ スイート デ パリダ
(Rust、Python バインディング、JS、Swift) を使用してください。ロス・アーティファクトス・デ・CI デベン・アジャンタル・エル
ヘルパー マス エル `matrix.md` ジェネラド (SDK/エスタド/デュラシオンのタブ) のチケットを抽出します。
リリースパラケロスレビュアーは、ローカルのスイートでマトリスデパリダーを監査します。