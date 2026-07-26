---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS オーケストレーター GA برابری رپورٹ

確定的マルチフェッチの開発 SDK の開発と開発ありがとうございます
ペイロード バイト、チャンクの受信、プロバイダー レポート、スコアボード、実装、および実装
ハーネス `fixtures/sorafs_orchestrator/multi_peer_parity_v1/` 正規マルチプロバイダー バンドル
SF1 プラン、プロバイダーのメタデータ、テレメトリ スナップショット、オーケストレーター オプション、詳細情報

## 錆びた

- **ヤク:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **アセンブル:** `MultiPeerFixture` プラン、インプロセス オーケストレーター、アセンブルされたペイロード バイト数
  チャンクレシート、プロバイダーレポート、スコアボード、スコアボード、スコアボード、スコアボードなどピーク時の同時実行数
  ワーキングセット (`max_parallel x max_chunk_length`) ٹریک کرتی ہے۔
- **パフォーマンス ガード:** ہر رن CI ہارڈویئر پر 2 s کے اندر مکمل ہونا چاہیے۔
- **ワーキング セットの上限:** SF1 ハーネス `max_parallel = 3` نافذ کرتا ہے، جس سے ونڈو <= 196608 バイト بنتی ہے۔

重要:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK ハーネス

- **੭ुଈ:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **اسکوپ:** フィクスチャ `iroha_js_host::sorafsMultiFetchLocal` ذریعے دوبارہ چلاتا ہے، اور مسلسل رنز کے درمیان
  ペイロード、領収書、プロバイダー レポート、スコアボードのスナップショット、データ
- **パフォーマンス ガード:** ہر اجرا 2 s میں مکمل ہونا چاہیے؛ハーネス ماپی گئی مدت اور 予約バイトの上限
  (`max_parallel = 3`、`peak_reserved_bytes <= 196608`) پرنٹ کرتا ہے۔

意味:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK ハーネス

- **ヤク:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **اسکوپ:** `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` میں تعریف کردہ パリティ スイート چلاتا ہے،
  Norito ブリッジ (`sorafsLocalFetch`) SF1 フィクスチャ پلے کرتا ہے۔ペイロードバイト、チャンク受信をハーネスします
  プロバイダーレポート スコアボードエントリー 決定論的プロバイダーメタデータ テレメトリスナップショット グラフィックス
  Rust/JS スイートの開発
- **ブリッジ ブートストラップ:** ハーネスの接続 `dist/NoritoBridge.xcframework.zip` 開梱の接続 `dlopen` 接続の解除
  macOS スライス فے۔ xcframework の SoraFS バインディング موجود نہ ہوں تو یہ
  `cargo build -p connect_norito_bridge --release` フォールバック پرتا ہے اور
  `target/release/libconnect_norito_bridge.dylib` کے ساتھ لنک کرتا ہے، اس لئے CI میں دستی سیٹ اپ درکار نہیں۔
- **パフォーマンス ガード:** ہر اجرا CI ہارڈویئر پر 2 s میں مکمل ہونا چاہیے؛ハーネス ماپی گئی مدت اور 予約バイトの上限
  (`max_parallel = 3`, `peak_reserved_bytes <= 196608`) پرنٹ کرتا ہے۔

意味:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python バインディング ハーネス

- **੭ुुु:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **回答:** 評価 `iroha_python.sorafs.multi_fetch_local` ラッパー 評価データクラスの型付きデータクラス
  正規のフィクスチャ API سے گزرے جسے ホイール صارفین استعمال کرتے ہیں۔ `providers.json` プロバイダー メタデータ بناتا ہے،
  テレメトリ スナップショットの注入、ペイロード バイト、チャンクの受信、プロバイダー レポート、スコアボード、Rust/JS/Swift スイート
  فی طرح ویریفائی کرتا ہے۔
- **前提条件:** `maturin develop --release` چلائیں (ホイール انسٹال کریں) تاکہ `_crypto` `sorafs_multi_fetch_local` バインディング ظاہر کرے؛
  結合バインディング دستیاب نہ ہو تو ハーネス خودکار طور پر スキップ ہو جاتا ہے۔
- **パフォーマンス ガード:** Rust スイート <= 2 秒pytest のアセンブルされたバイト数、プロバイダーの参加概要、リリース アーティファクト
  ٩ے لئے لاگ کرتا ہے۔

ハーネス (Rust、Python、JS、Swift) の概要出力の実行、ビルドの実行、プロモーションの実行
ペイロードの受信数、メトリクス、比較、および比較パリティ スイート (Rust、Python バインディング、JS、Swift)
کو ایک پاس میں چلانے کے لئے `ci/sdk_sorafs_orchestrator.sh` چلائیں؛ CI アーティファクトのヘルパー機能の開発
`matrix.md` (SDK/ステータス/継続時間) 評価レビュー担当者 レビュー担当者パリティ行列と監査
ありがとうございます