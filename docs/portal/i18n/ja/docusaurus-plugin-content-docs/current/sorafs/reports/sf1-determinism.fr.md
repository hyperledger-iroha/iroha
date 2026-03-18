---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: SoraFS SF1 決定論ドライラン
概要: チェックリストとダイジェストは、正規プロファイル チャンカー `sorafs.sf1@1.0.0` の検証に参加します。
---

# SoraFS SF1 決定論ドライラン

カノニクのプロフィールチャンカーをベースにドライランで関係をキャプチャします
`sorafs.sf1@1.0.0`。ツーリング WG はチェックリストを再実行し、CI-dessous lors de la を実行します
フィクスチャのリフレッシュとコンソマチュールの新しいパイプラインの検証。
保守作業を行うための任務を遂行するための任務を遂行する
トレースは監査可能です。

## チェックリスト

|エテープ |コマンド |出席結果 |メモ |
|------|------|------||------|
| 1 | `cargo test -p sorafs_chunker` |試験合格。 le test de parité `vectors` reussit。 | Rust の実装に準拠したフィクスチャのコンパイルと対応を確認します。 |
| 2 | `ci/check_sorafs_fixtures.sh` | Le script sort en 0 ;報告書は、マニフェストの内容を要約します。 |添付書類の保存と署名の確認を行います。 |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` のエントリは、レジストリの記述子 (`profile_id=1`) に対応します。 |レジストリのメタデータを確実に同期します。 |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | `--allow-unsigned` を無視した再生成マニフェストと署名の変更を保存します。 |チャンクとマニフェストの限界を決定するための完全な決定です。 |
| 5 | `node scripts/check_sf1_vectors.mjs` | TypeScript と JSON Rust の相違点との関係。 |ヘルパーオプション;クロスランタイムの保証 (ツール WG によるスクリプトの保守)。 |

## ダイジェスト出席

- チャンクダイジェスト (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## ジャーナルの承認

|日付 |エンジニア |チェックリストの結果 |メモ |
|------|----------|-----------|------|
| 2026-02-12 |ツーリング (LLM) | ✅ レウシ |フィクスチャは `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` 経由で登録され、正規リストのリストとエイリアスおよびマニフェスト ダイジェスト フレーム `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55` を作成します。 Vérifié avec `cargo test -p sorafs_chunker` et un `ci/check_sorafs_fixtures.sh` propre (検証を行うための設備ステージ)。ノードの到着を確認して 5 番目のノードを確認します。 |
| 2026-02-20 |ストレージ ツール CI | ✅ レウシ |議会封筒 (`fixtures/sorafs_chunker/manifest_signatures.json`) `ci/check_sorafs_fixtures.sh` 経由で受け取ります。スクリプトを修正し、マニフェスト ダイジェスト `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` を確認し、Rust をハーネスに関連付けます (Go/Node の実行機能を diff なしで実行できます)。 |

ツーリング WG は、チェックリストの実行後の日付を決定します。シウネ
問題を解決し、修復の詳細を含める
新しい備品やプロフィールを事前に承認します。