---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: SoraFS SF1 決定論ドライラン
概要: 正規の `sorafs.sf1@1.0.0` チャンカー プロファイルと、予想されるダイジェストのチェックリストを検証します。
---

# SoraFS SF1 決定論ドライラン

正規 `sorafs.sf1@1.0.0` チャンカー プロファイル ベースライン ドライラン
キャプチャーツーリング WG 設備の更新、消費者パイプラインの更新
検証チェックリストチェックリストチェックリストチェックリストفر کمانڈ کا نتیجہ
ٹیبل میں ریکارڈ کریں تاکہ 監査可能な証跡 برقرار رہے۔

## チェックリスト

|ステップ |コマンド |期待される結果 |メモ |
|------|------|------||------|
| 1 | `cargo test -p sorafs_chunker` |テストをテストします`vectors` パリティ テスト|正規フィクスチャをコンパイルする ہیں Rust を実装する 一致する ہیں۔ |
| 2 | `ci/check_sorafs_fixtures.sh` | 0 0 出口を出るマニフェスト ダイジェスト|フィクスチャを検証する フィクスチャを検証する 再生成する 添付された署名を再生成する|
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` エントリ レジストリ記述子 (`profile_id=1`) の一致| یقینی بناتا ہے کہ レジストリ メタデータ同期 رہے۔ |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` |再生 `--allow-unsigned` 成功しましたマニフェスト 署名 変更なし رہیں۔ |チャンク境界の明示、決定論の証明、決定論の証明|
| 5 | `node scripts/check_sf1_vectors.mjs` | TypeScript フィクスチャ Rust JSON との差分レポート|オプションのヘルパーランタイム ٩ے درمیان パリティ یقینی بنائیں (スクリプト ツール WG が維持 کرتا ہے)۔ |

## 予想されるダイジェスト

- チャンクダイジェスト (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## サインオフログ

|日付 |エンジニア |チェックリストの結果 |メモ |
|------|----------|------|------|
| 2026-02-12 |ツーリング (LLM) | ✅ 合格 | `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` フィクスチャの再生成 正規ハンドル + エイリアス リスト マニフェスト ダイジェスト `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55` フィクスチャ`cargo test -p sorafs_chunker` テスト `ci/check_sorafs_fixtures.sh` 実行テスト、検証テスト (フィクスチャー チェック、ステージング、テスト)。ステップ 5 保留中 ノード パリティ ヘルパーの保留中|
| 2026-02-20 |ストレージ ツール CI | ✅ 合格 |議会封筒 (`fixtures/sorafs_chunker/manifest_signatures.json`) `ci/check_sorafs_fixtures.sh` کے ذریعے حاصل ہوا؛フィクスチャを再生成する マニフェスト ダイジェスト `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` を確認する Rust ハーネスを確認する (Go/Node ステップを実行する) ہیں) 差分 کے۔ |

ツーリング WG チェックリスト بعد تاریخ کے ساتھ قطار شامل کرنی چاہیے۔ああ
失敗しました リンクされた問題 解決策 解決策
プロフィール 承認 プロフィール 承認