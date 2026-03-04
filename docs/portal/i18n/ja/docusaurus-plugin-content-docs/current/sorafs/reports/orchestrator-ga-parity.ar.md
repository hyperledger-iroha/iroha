---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# عرض المزيد كافؤ GA لمنسق SoraFS

マルチフェッチ SDK のマルチフェッチの実行
ペイロード チャンク プロバイダ スコアボード スコアボード
ああ。ハーネス يستهلك الحزمة القياسية متعددة المزوّدين تحت
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/` SF1 の世界
プロバイダー、テレメトリ、オーケストレーター。

## 錆びる

- **回答:** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **النطاق:** يشغّل خطة `MultiPeerFixture` مرتين عبر Orchestrator داخل العملية، مع التحقق
  ペイロード、チャンク、プロバイダー、スコアボード。 كما
  ワーキング セット (`max_parallel x max_chunk_length`) が必要です。
- **حاجز الأداء:** يجب أن يكتمل كل تشغيل خلال 2 s على عتاد CI.
- ** سقف مجموعة العمل:** ملف تعريف SF1 يفرض ハーネス `max_parallel = 3`، مما ينتج
  فافذة <= 196608 年。

重要事項:

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK をハーネスする

- **回答:** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **النطاق:** يعيد تشغيل نفس fixture عبر `iroha_js_host::sorafsMultiFetchLocal`، ويقارن
  ペイロード、領収書、プロバイダー、スコアボード、スコアボード。
- **حاجز الأداء:** يجب أن ينتهي كل تنفيذ خلال 2 s؛ハーネス المقاسة وسقف
  (`max_parallel = 3`、`peak_reserved_bytes <= 196608`)。

重要な点:

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## ハーネス Swift SDK

- **回答:** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **النطاق:** يشغّل مجموعة التكافؤ المعرّفة في `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift`،
  フィクスチャ SF1 は Norito (`sorafsLocalFetch`) です。ハーネス
  ペイロード チャンク プロバイダ スコアボード スコアボード
  プロバイダー、テレメトリー、Rust/JS。
- ** تهيئة الجسر:** يفك ハーネス ضغط `dist/NoritoBridge.xcframework.zip` عند الطلب ويحمل
  macOS `dlopen`。 xcframework を使用して、SoraFS を使用してください。
  `cargo build -p connect_norito_bridge --release` ログイン
  `target/release/libconnect_norito_bridge.dylib` は、CI です。
- **حاجز الأداء:** يجب أن ينتهي كل تنفيذ خلال 2 s على عتاد CI؛ハーネス
  (`max_parallel = 3`、`peak_reserved_bytes <= 196608`)。

重要な点:

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## ハーネス パイソン

- **回答:** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- ** النطاق:** يمارس ラッパー عالي المستوى `iroha_python.sorafs.multi_fetch_local` وdataclasses
  試合結果、試合結果、試合結果、試合結果、試合結果、試合結果、試合結果などを記録
  車輪。 يعيد الاختبار بناء بيانات プロバイダー الوصفية من `providers.json`، ويحقن
  テレメトリ ペイロード チャンク プロバイダー ペイロード ペイロード チャンク プロバイダー
  スコアボードは Rust/JS/Swift に対応します。
- **متطلب مسبق:** شغّل `maturin develop --release` (ホイール) كي يعرض `_crypto` ربط
  `sorafs_multi_fetch_local` pytest のテストハーネス تلقائيا عندما لا يكون
  そうです。
- ** 評価:** 評価 <= 2 秒 評価pytest を実行する
  プロバイダーは、プロバイダーをサポートします。

ハーネス (Rust、Python、JS、Swift)
ペイロード ペイロード メッセージ ペイロード メッセージ ペイロード ペイロード ペイロード ペイロード ペイロード ペイロード ペイロード ペイロード
構築します。 `ci/sdk_sorafs_orchestrator.sh` は、Rust と Python を実行します。
バインディング وJS وSwift) في تمريرة واحدة؛ ينبغي أن تُرفق مخرجات CI مقتطف السجل من هذا
`matrix.md` バージョン (SDK/バージョン/バージョン) バージョン
最高のパフォーマンスを見せてください。