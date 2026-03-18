---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
title: SoraFS SF1 決定論ドライラン
概要: Чеклист и ожидаемые ダイジェストと `sorafs.sf1@1.0.0` のチャンカー。
---

# SoraFS SF1 決定論ドライラン

Этот отчет фиксирует базовый ドライラン для канонического профиля chunker
`sorafs.sf1@1.0.0`。ツーリング WG が、今日の活動を開始します。
備品と消費者パイプライン。 Записывайте результат каждой
監査可能な証跡。

## Чеклист

| Шаг | Команда | Ожидаемый результат | Примечания |
|------|------|------||------|
| 1 | `cargo test -p sorafs_chunker` | Все тесты проходят; `vectors` パリティ テストが必要です。 | Подтверждает、что канонические フィクスチャー компилируются и совпадают с Rust реализацией。 |
| 2 | `ci/check_sorafs_fixtures.sh` | Скрипт завергается 0;マニフェストをダイジェストします。 | Проверяет、что 備品 регенерируются чисто и подписи остаются прикреплены。 |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` レジストリ記述子 (`profile_id=1`) を参照してください。 | Гарантирует、что メタデータ レジストリ остается синхронной。 |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | Регенерация проходит без `--allow-unsigned`;マニフェストと署名。 |チャンク境界とマニフェストを定義します。 |
| 5 | `node scripts/check_sf1_vectors.mjs` | TypeScript フィクスチャと Rust JSON の差分を確認します。 | Опциональный ヘルパー。ランタイム (スクリプト ツール WG)。 |

## ダイジェスト

- チャンクダイジェスト (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## サインオフログ

|ダタ |エンジニア | Результат чеклиста | Примечания |
|------|----------|--------|----------|
| 2026-02-12 |ツーリング (LLM) | ✅ 合格 |フィクスチャは `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`、ハンドル + エイリアス、マニフェスト ダイジェスト `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55` です。 Проверено `cargo test -p sorafs_chunker` と чистым прогоном `ci/check_sorafs_fixtures.sh` (備品 подготовлены для проверки)。説明 5 ノード パリティ ヘルパー。 |
| 2026-02-20 |ストレージ ツール CI | ✅ 合格 |議会封筒 (`fixtures/sorafs_chunker/manifest_signatures.json`) получен через `ci/check_sorafs_fixtures.sh`;フィクスチャ、マニフェスト ダイジェスト `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`、Rust ハーネス (Go/Node の詳細) を参照してください。差分。 |

ツーリング WG がテストを実行します。 Если
какой-либо заг падает, заведите 問題 со ссылкой здесь и добавьте детали
修復は治具の修復に役立ちます。