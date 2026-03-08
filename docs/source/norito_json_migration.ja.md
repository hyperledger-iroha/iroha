<!-- Japanese translation of docs/source/norito_json_migration.md -->

---
lang: ja
direction: ltr
source: docs/source/norito_json_migration.md
status: complete
translator: manual
---

# Norito JSON マイグレーションガイド

このガイドはワークスペース全体を Serde / `serde_json` から Norito ネイティブシリアライゼーションへ移行する手順をまとめたものです。`scripts/inventory_serde_usage.py` が生成する自動インベントリを補完し、Serde の derive・ビジター・JSON DOM ヘルパー・設定ローダーを置き換える Norito API サーフェスを説明します。

> **ステータス（2025-11-09）:** 本番クレートからは Serde が排除済みです。このガイドは Norito ファーストな開発フローに慣れるための歴史的資料として残しています。

## なぜ Norito へ全面移行するのか

- **決定性:** Norito のバイナリ／JSON コーデックはオンチェーンハッシュやリプレイ保護の基盤となる正規化規則を共有しています。
- **機能同等性:** Norito の derive は `serde::{Serialize, Deserialize}` で扱っていたマップビジター、enum タグ付け、raw-value パススルーなどのパターンをすべてカバーします。
- **依存グラフの簡素化:** Serde を外すことで二重の derive やフィーチャーガードを削減し、依存関係全体を軽量化できます。

## Serde ユースケース対応表

| Serde ユースケース | Norito 置き換え | 参照セクション |
| --- | --- | --- |
| 構造体 derive / トレイト境界 | `NoritoSerialize` / `NoritoDeserialize` の derive および手書き impl | Struct + enum serialization |
| enum とタグ付け | `#[norito(tag = ...)]`, `#[norito(rename = ...)]`, `#[norito(other)]` | Enum tagging |
| マップ／コレクションアクセス | `json::MapVisitor`, `json::SeqVisitor`, `json::CoerceKey`, `#[norito(with = ...)]` | Visitor + builder hooks、Map key handling |
| オプションフィールドとデフォルト | `#[norito(default)]`, `#[norito(skip_serializing_if = Option::is_none)]`, `MapVisitor::read_optional` | Optional fields and defaults |
| 設定パースと IO | `norito_config::TomlValue`, `norito::json::from_reader`, `to_writer` | Config loading and Norito-backed TOML |
| スナップショット／バイナリ | `norito::core::{to_bytes, from_bytes}`, JSON pretty プリンタ | Snapshot + binary IO |
| ビジターフック／カスタムパース | `JsonSerialize`/`JsonDeserialize` トレイト、`json::Visitor` ヘルパー | Visitor + builder hooks |

## ツール類

- **インベントリ:** `scripts/inventory_serde_usage.py` は JSON と人間可読サマリを `docs/source/norito_json_inventory.{json,md}` に出力し、本番クレートに Serde が残っていないことを確認します。許可リストにはポリシードキュメント、ツール、テストフィクスチャのみが存在します。
- **CI ガード:** `scripts/deny_serde_json.sh` と `scripts/check_no_direct_serde.sh` は「本番コードに Serde 禁止」ルールを継続的に検証し、最終クリーンアップでは残存参照を検出した時点で失敗させる予定です。

## Struct / enum シリアライゼーション

| Serde パターン | Norito 置き換え |
| --- | --- |
| `#[derive(Serialize, Deserialize)]` | `#[derive(NoritoSerialize, NoritoDeserialize)]` |
| `serde::Serialize` トレイト境界 | `norito::NoritoSerialize` |
| `serde::DeserializeOwned` | `norito::NoritoDeserializeOwned` |
| `#[serde(rename = ...)]`, `#[serde(rename_all = ...)]` | 同名の `#[norito(rename = ...)]`, `#[norito(rename_all = ...)]` |
| `#[serde(default)]`, `#[serde(skip)]`, `#[serde(skip_serializing_if = ...)]` | Norito derive がそのまま解釈 |
| `#[serde(flatten)]` | `#[norito(flatten)]` を利用 |
| カスタム deserialize ビジター | `norito::NoritoDeserialize` 手書き実装、または `json::MapVisitor` / `json::SeqVisitor` |

> **ステータス（2025-11-07）:** 文字列ベースの ID 新型 (`DomainId`, `AssetDefinitionId`, `NftId`, `TriggerId` など) は `string_id!` マクロ経由で Norito の `FastJsonWrite` + `JsonDeserialize` を実装済み。これにより `norito::json::to_json` / `from_json` で正規表記を扱えます。
>
> **Status (Jan 25, 2026):** `AccountId`/`AssetId` JSON now includes an explicit canonical encoded literals (e.g., `IH58`, `norito:<hex>`) so decoding does not rely on a domain-selector resolver. Alias inputs such as `label@domain` are rejected by runtime parsers and should be used only in explicit negative tests.

> **ステータス（2025-11-07）:** `IpfsPath` も既存パーサを用いた Norito `FastJsonWrite` / `JsonDeserialize` を実装し、正／負のラウンドトリップテストを追加しました。互換目的で Serde 実装は一時的に残っています。

### Visitor / Builder フック

- `norito::json::MapVisitor` / `SeqVisitor`: Serde の `MapAccess` / `SeqAccess` に相当する型安全な API。`missing_field`、`duplicate_field`、`unknown_field` などのヘルパーを提供。
- `norito::json::RawValue`: 解析を遅延するための raw JSON バッファ（`serde_json::value::RawValue` の代替）。
- `norito::json::CoerceKey`: マップキーの正規化を Serde のカスタムシリアライゼーションと同等に実現。
- `norito::json::Visitor` と `visit_value` は Serde のビジターディスパッチを模倣する型付きフック（`visit_bool`, `visit_i64`, `visit_map`, `visit_seq` 等）を提供。
- `#[derive(JsonSerialize, JsonDeserialize)]`（名前付き構造体）は決定的ライターと `MapVisitor` ベースのパーサーを生成し、手書きビジターの差し込みも可能。

### Enum タグ付け

- `#[norito(tag = "type")]`, `#[norito(rename = "...")]`, `#[norito(other)]` といった属性で Serde のスタイルを再現。
- 外部タグ／内部タグ／隠しタグなどのケースも derive がサポート。`#[norito(tag = "...", content = "...")]` が `serde(tag, content)` に対応。

### マップキー処理

- Norito は JSON DOM を経由せず決定的な順序を維持。`json::CoerceKey` によって Serde のカスタムキーシリアライゼーションを移植。
- `VisitStringKey` 相当は `MapVisitor::read_string_key` や `read_field` で置換。
- 未知キーの拒否は `MapVisitor::reject_unknown_fields()`、または derive の `deny_unknown` 属性を利用。

### オプションフィールドとデフォルト

- `#[norito(default)]` や `#[norito(default = "fn")]` が Serde の `default` と同等。
- `#[norito(skip_serializing_if = Option::is_none)]` で Option を省略。
- `MapVisitor::read_optional` で必須／任意フィールドを明示的に処理。

### スナップショットとバイナリ IO

- すべてのバイナリスナップショット経路は `norito::core::{to_bytes, from_bytes}` を使用し、derive で生成したシリアライザ／デシリアライザを活用。
- テストの JSON スナップショットは `norito::json::{to_json_pretty, from_json}` を利用。

### 設定読み込みと Norito バック TOML

1. `norito_config::TomlValue`（`toml_edit` ベース）で TOML をパース。
2. 既存の `iroha_config_base::read` を通じてデフォルト／環境変数を適用。ビルダーは `NoritoDeserialize` 値を受け渡し。
3. JSON 中間表現が必要な場合は `norito_config::toml::to_json_value` を利用。

`serde_with` のアダプタは `#[norito(with = ...)]` や手書きビジターで置き換え可能。`serde_json::from_reader` は `norito::json::from_reader` に、`serde_json::value::RawValue` は `norito::json::RawValue` に置換。

## エラーマッピング

- `serde_json::Error` の代わりに `norito::json::Error` を使用し、`?` で伝播。
- 設定／CLI のエラー型へは `From<norito::json::Error>` を実装し、既存の `eyre` / `thiserror` ベースの変換を継続。

## マイグレーション手順

1. 対象クレートで Serde の残存箇所をインベントリスクリプトで特定。まず本番モジュール（config → core → cli）を優先。
2. derive とインポートを Norito 系に差し替え、Norito のみでコンパイル／テストが通ることを確認。
3. `serde_json::Value` やマクロ、ビジターを Norito の対応 API に置換。
4. 移行した型ごとに Norito ラウンドトリップテスト（`to_bytes` / `from_bytes`、`to_json` / `from_json`）を追加。
5. クレートドキュメントやコメントを更新し、Norito が正規シリアライゼーションであることを明記。

最新状況は `status.md` のクレート別チェックリストを参照してください。

## クレート別プレイブック（抜粋）

### コンフィグスタック（`iroha_config`, `iroha_config_base`, `iroha_config_cli`）

- `serde::{Serialize, Deserialize}` derive を `NoritoSerialize` / `NoritoDeserialize` または手書き `JsonSerialize` / `JsonDeserialize` に置換。
- `norito_config::load_layers` を通じてユーザー／実値／デフォルト層を読み込み、Norito 構造体を既存ビルダーへ渡す。
- `serde_with` アダプタは `#[norito(with = ...)]` または `MapVisitor::read_string_key` などで置換。
- Norito パースエラーを `iroha_config_base::Error` 経由で公開し、CLI/サービスが決定的な診断を返すようにする。
- `crates/iroha_config/tests` に Norito で設定を読み込み、従来の Serde スナップショットと同等であることを保証する回帰テストを追加。

> **ステータス（2025-11-07）:** `iroha_config` は `serde` と `serde_with` を依存から除去。デフォルト／ビジター／エミッタは Norito ベースに移行済み。

### その他のハイライト

- CLI と Torii は `norito::json::{Parser, Reader, TapeWalker}` を利用し、`from_reader` / `from_slice` / `from_io_buf` と `to_writer` を Serde 相当 API として提供。
- バイナリスナップショット、JSON スナップショット、設定ロードなど全コードパスで Norito の `*_bytes` / `*_json` が利用されていることを確認。
- テスト／ツールの JSON DOM は `norito::json::Value` に統一済みで、`scripts/inventory_serde_usage.py` が `serde_json::Value` の再流入を継続監視。

---

ワークスペース移行が完了した現在も、新規型や機能追加では Norito の derive / ビジターをデフォルトで用い、Serde を再導入しないようにしてください。必要に応じて本ガイドを参照しながら、残存する Serde ベースの周辺ツール（例えば外部インポート用の互換レイヤー）も Norito へ移行していきましょう。
