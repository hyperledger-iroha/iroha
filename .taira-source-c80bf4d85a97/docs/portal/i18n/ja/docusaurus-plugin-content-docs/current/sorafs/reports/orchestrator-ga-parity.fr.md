---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# パリティ関係 GA SoraFS オーケストレーター

SDK に関するマルチフェッチの決定権の決定
リリース エンジニアは、ペイロードのバイト数、チャンク レシート、
プロバイダーのレポートとスコアボードの結果は、常に一致しています
実装。チャック ハーネス コンソメ ル バンドル マルチプロバイダー canonique dans
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`、計画 SF1 を再グループ化してください、
メタデータ プロバイダー、テレメトリ スナップショット、およびオーケストレーターのオプション。

## Rust ベースライン

- **コマンド:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **スコープ:** プロセス中のオーケストラ経由で計画 `MultiPeerFixture` を実行します。
  ペイロード アセンブリ、チャンク レシート、プロバイダー レポートなどのバイトの検証
  スコアボードの結果。オーストラリアの同意を示す計装スーツ
  et la taille 有効デュワーキングセット (`max_parallel × max_chunk_length`)。
- **パフォーマンス ガード:** ハードウェア CI の最終的な実行を実行します。
- **作業セットの上限:** Avec le profile SF1、ハーネスは `max_parallel = 3`、
  ドンナント ウネ フェネトル ≤ 196608 バイト。

ログ出力の例:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK ハーネス

- **コマンド:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **範囲:** `iroha_js_host::sorafsMultiFetchLocal` 経由の Rejoue la meme フィクスチャ、
  比較ペイロード、レシート、プロバイダーレポート、およびスコアボードのスナップショット全体
  連続処刑。
- **パフォーマンス ガード:** チャック実行は 2 秒で完了します。ル・ハーネス・インプリム・ラ
  耐久性とバイト数の確保 (`max_parallel = 3`、`peak_reserved_bytes ≤ 196 608`)。

概要行の例:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK ハーネス

- **コマンド:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **範囲:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` のパリ定義スイートを実行、
  ルブリッジ Norito (`sorafsLocalFetch`) 経由のフィクスチャー SF1 ドゥフォワを楽しみます。ル・ハーネス・ベリフィエ
  ペイロードのバイト、チャンク受信、プロバイダーレポート、スコアボードのユーティリティファイルなど
  Rust/JS スイートのメタデータ プロバイダーの決定およびテレメトリ スナップショット。
- **ブリッジ ブートストラップ:** ハーネス圧縮解除 `dist/NoritoBridge.xcframework.zip` 要求と料金
  `dlopen` 経由で macOS をファイルスライスします。 Lorsque l'xcframework est manquant ou n'a pas les bindings SoraFS、il
  bascule sur `cargo build -p connect_norito_bridge --release` et se lie à
  `target/release/libconnect_norito_bridge.dylib`、CI のセットアップ マニュアルはありません。
- **パフォーマンス ガード:** ハードウェア CI での 2 つの最終的な実行を実行します。ル・ハーネス・インプリム・ラ
  耐久性とバイト数の確保 (`max_parallel = 3`、`peak_reserved_bytes ≤ 196 608`)。

概要行の例:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python バインディング ハーネス

- **コマンド:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **スコープ:** `iroha_python.sorafs.multi_fetch_local` およびデータクラス タイプのラッパー オーニボーを実行します。
  API を使用してホイールの製造装置を調整します。テストの再構築
  メタデータ プロバイダーが `providers.json` をデプロイし、テレメトリ スナップショットを挿入してペイロードのバイトを検証し、
  チャンク受信、プロバイダーレポート、および Rust/JS/Swift スイートのスコアボードの内容。
- **前提条件:** `maturin develop --release` (ホイールのインストール) を実行して、`_crypto` ファイル バインディングを公開します
  `sorafs_multi_fetch_local` 事前に pytest を呼び出します。 le ハーネス auto-skip lorsque le バインディング est indisponible。
- **パフォーマンス ガード:** 予算 ≤ 2 sque la suite Rust ; pytest のバイト アセンブリのログ記録
  プロバイダーがリリースの成果物を注ぐ参加者の履歴書。

ハーネス (Rust、Python、JS、Swift) のリリース ゲート キャプチャー ファイルの概要出力
ペイロードとマニエールの制服の領収書を比較するためのアーカイブを作成します。
構築を促進します。 Exécutez `ci/sdk_sorafs_orchestrator.sh` ランサー チャック スイート ドゥ パリテ
(Rust、Python バインディング、JS、Swift) 必要はありません。 les artefacts CI doivent joindre l'extrait
ログ ヘルパー プラス ファイル `matrix.md` 一般 (テーブル SDK/ステータス/期間) リリース後のチケット
レビュー担当者は、ローカルな環境を考慮せずにパリテのマトリスを監査します。