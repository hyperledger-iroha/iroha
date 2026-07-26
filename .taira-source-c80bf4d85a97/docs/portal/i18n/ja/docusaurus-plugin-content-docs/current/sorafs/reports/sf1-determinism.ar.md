---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: SF1 في SoraFS
概要: ダイジェスト メッセージ、チャンカー、`sorafs.sf1@1.0.0`。
---

# SF1 في SoraFS

チャンカー チャンカー チャンカー チャンカー チャンカー チャンカー チャンカー チャンカー
`sorafs.sf1@1.0.0`。ツーリング WG ツール WG ツール WG ツール WG ツール WG
フィクスチャーを確認してください。ニュース ニュース ニュース
重要な問題は、次のとおりです。

## قائمة التحقق

|ああ |ああ |ログイン | ログイン重要 |
|------|------|------||------|
| 1 | `cargo test -p sorafs_chunker` |テストを行ってください。パリティ `vectors`。 |錆びた備品。 |
| 2 | `ci/check_sorafs_fixtures.sh` | خرج السكربت بـ 0؛マニフェストをダイジェストします。 |試合の試合結果、試合結果、試合結果、試合結果などを確認してください。 |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` レジストリ (`profile_id=1`)。 |メタデータとレジストリ。 |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` |テスト `--allow-unsigned`;マニフェストを表示します。 |マニフェストのチャンクをマニフェストします。 |
| 5 | `node scripts/check_sf1_vectors.mjs` | TypeScript と Rust JSON の差分フィクスチャ。 |ヘルパーランタイムのパリティ (ツール WG)。 |

## ダイジェスト

- チャンクダイジェスト (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## いいえ

|認証済み |エンジニア |ログインしてください。重要 |
|------|----------|----------|----------|
| 2026-02-12 |ツーリング (LLM) | ❌ 失敗 |バージョン 1: `cargo test -p sorafs_chunker` プロファイル `vectors` フィクスチャ ハンドル ハンドル `sorafs.sf1@1.0.0` プロファイルエイリアス/ダイジェスト (`fixtures/sorafs_chunker/sf1_profile_v1.*`)。バージョン 2: يتوقف `ci/check_sorafs_fixtures.sh` — الملف `manifest_signatures.json` مفقود في حالة リポジトリ (محذوف في ワーキング ツリー)。 4: `export_vectors` はマニフェストを表示します。説明: フィクスチャ (市議会) バインディング ハンドル ハンドル +別名 كما تتطلب الاختبارات。 |
| 2026-02-12 |ツーリング (LLM) | ✅ 合格 |フィクスチャ `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` のハンドルとエイリアス リスト、マニフェスト ダイジェスト `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55` が表示されます。テストは `cargo test -p sorafs_chunker` وتشغيل نظيف لـ `ci/check_sorafs_fixtures.sh` (フィクスチャは تجهيزها للتحقق) 5 番目のヘルパー パリティ ノード。 |
| 2026-02-20 |ストレージ ツール CI | ✅ 合格 |議会の封筒 (`fixtures/sorafs_chunker/manifest_signatures.json`) は `ci/check_sorafs_fixtures.sh`;フィクスチャの修正 `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`、ハーネス Rust (Go/Node のテスト)ああ。 |

ツーリング WG は、ツール WG をサポートしています。 और देखें
問題の問題の解決、改善の解決、修正の実行
重要です。