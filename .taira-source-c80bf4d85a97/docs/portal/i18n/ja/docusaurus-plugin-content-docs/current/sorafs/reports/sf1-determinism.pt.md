---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: SoraFS SF1 決定論ドライラン
概要: チェックリストは、Canonico `sorafs.sf1@1.0.0` の有効性に関するエスペラードや Perfil チャンカーをダイジェストしています。
---

# SoraFS SF1 決定論ドライラン

エステリラトリオキャプチャー、ドライランベースパラ、パーフィルチャンカーカノニコ
`sorafs.sf1@1.0.0`。ツール WG は、再実行者またはチェックリストの abaixo ao validar を開発します。
フィクスチャと新しいコンシューマ パイプラインを更新します。結果を登録する
コマンドー・ナ・タベラ・パラ・マンテル・ウム・トレイル・アウディタヴェル。

## チェックリスト

|パッソ |コマンド |エスペラードの結果 |メモ |
|------|------|------||------|
| 1 | `cargo test -p sorafs_chunker` | Todos os はパスサムをテストします。 o teste de paridade `vectors` tem sucesso。 | Rust の実装に対応するフィクスチャのコンパイルを確認します。 |
| 2 | `ci/check_sorafs_fixtures.sh` | O スクリプト sai com 0;マニフェスト アバイソの OS ダイジェストをレポートします。 |永続的な操作性を維持するために、治具の再生が困難であることを確認します。 |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` |エントリ `sorafs.sf1@1.0.0` は、レジストリ記述子 (`profile_id=1`) に対応します。 |メタデータはレジストリを永続的に保存します。 |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | `--allow-unsigned` を再生成します。マニフェスト・デ・マニフェスト・ナオ・ムダムの記録。 |チャンクのマニフェストの限界を決定することができます。 |
| 5 | `node scripts/check_sf1_vectors.mjs` | TypeScript と Rust JSON の相違点をレポートします。 |ヘルパーはオプションです。ランタイムを保証します (ツール WG のスクリプト管理)。 |

## エスペラードを消化します

- チャンクダイジェスト (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## サインオフログ

|データ |エンジニア |結果として実行するチェックリスト |メモ |
|------|----------|--------------------------|----------|
| 2026-02-12 |ツーリング (LLM) | OK |フィクスチャは `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f` 経由で再生成され、製品は canonico とエイリアス リストを処理し、新しいマニフェスト ダイジェスト `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55` を生成します。 Verificado com `cargo test -p sorafs_chunker` e um `ci/check_sorafs_fixtures.sh` limpo (検証のための準備準備)。 Passo 5pendenteateohelperdeparidadeNodechegar。 |
| 2026-02-20 |ストレージ ツール CI | OK |議会封筒 (`fixtures/sorafs_chunker/manifest_signatures.json`) `ci/check_sorafs_fixtures.sh` 経由で送信。 o スクリプトの再生成フィクスチャ、確認 o マニフェスト ダイジェスト `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`、再実行 o Rust ハーネス (Go/Node の実行とディスポンティブの実行) sem diff。 |

ツール WG は、追加の実行データと実行者チェックリストを開発します。セアルガム
ファルハール通り、問題を解決するための詳細を含めた問題を解決する
新しい備品を承認します。