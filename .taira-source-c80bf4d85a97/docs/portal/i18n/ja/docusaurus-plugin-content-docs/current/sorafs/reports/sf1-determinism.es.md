---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: SoraFS SF1 決定論ドライラン
概要: チェックリストは、Canonico `sorafs.sf1@1.0.0` のエスペラード パラバリダル エル パーフィル チャンカーをダイジェストしています。
---

# SoraFS SF1 決定論ドライラン

エステレポートキャプチャーエルドライランベースパラエルパーフィルチャンカーカノニコ
`sorafs.sf1@1.0.0`。ツーリング WG がアバホ クアンドの再排出チェックリストを作成
フィクスチャの有効なリフレッシュまたはコンスミドールの新しいパイプライン。レジストラエル
その結果、監査可能なトレイルでのタブラパラマンテナーのコマンドが実行されます。

## チェックリスト

|パソ |コマンド |エスペラードの結果 |メモ |
|------|------|------||------|
| 1 | `cargo test -p sorafs_chunker` |トドスはパサンのテストに負けます。テスト デ パリダード `vectors` を終了します。 | Rust でのフィクスチャのコンパイルと実装の一致を確認します。 |
| 2 | `ci/check_sorafs_fixtures.sh` | El スクリプト販売コン 0;アバホのマニフェストのダイジェストレポート。 |試合の備品を定期的に再生し、永久的な付属品を検証します。 |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` はレジストリ記述子 (`profile_id=1`) と一致します。 |レジストリのメタデータを確認します。 |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` |ラ・リジェネレーション・ティエン・エグジト・シン `--allow-unsigned`;ロス・アーカイブス・デ・マニフェスト・イ・ファーム・ノー・カンビアン。 |大量のマニフェストの限界を明確に決定することを証明します。 |
| 5 | `node scripts/check_sf1_vectors.mjs` | TypeScript と Rust JSON の違いをレポートします。 |ヘルパーはオプションです。ランタイムのセキュリティを確保します (ツール WG のスクリプト管理)。 |

## エスペラードを消化します

- チャンクダイジェスト (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## サインオフログ

|フェチャ |エンジニア |チェックリストの結果 |メモ |
|------|----------|---------------------------|------|
| 2026-02-12 |ツーリング (LLM) | OK | `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f` を介してフィクスチャが再生成され、canonico ハンドルとエイリアス リストが生成され、マニフェスト ダイジェスト フレスコ `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55` が生成されます。 Verificado con `cargo test -p sorafs_chunker` y una corrida limpia de `ci/check_sorafs_fixtures.sh` (検証のための準備の備品)。 Paso 5 pendiente hasta que llegue el helper de paridad ノード。 |
| 2026-02-20 |ストレージ ツール CI | OK |議会封筒 (`fixtures/sorafs_chunker/manifest_signatures.json`) `ci/check_sorafs_fixtures.sh` 経由で送信。スクリプトの再生成フィクスチャ、マニフェスト ダイジェスト `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` の確認、Rust のハーネスの再取り出し (Go/Node による取り出しの確認)、差分。 |

ツール WG は、チェックリストに追加されたファイルを取り出します。シ
アルグン・パソ・ファリャ、問題解決の詳細を含む問題を解決する
新しいフィクスチャやパーファイルを準備する必要があります。