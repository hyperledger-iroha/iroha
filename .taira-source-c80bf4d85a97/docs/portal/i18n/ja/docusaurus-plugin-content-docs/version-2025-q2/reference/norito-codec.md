---
lang: ja
direction: ltr
source: docs/portal/docs/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8de31f9e066b729fda8324b8847badba23de926888574d02a44fb0e6d4472f77
source_last_modified: "2026-01-18T05:31:56+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito コーデック リファレンス

Norito は、Iroha の正規シリアル化レイヤーです。すべてのネットワーク上のメッセージ、ディスク上
ペイロード、およびクロスコンポーネント API は Norito を使用するため、ノードは同一のバイトで一致します
異なるハードウェアで実行されている場合でも。このページには可動部分がまとめられています
`norito.md` の完全な仕様を指します。

## コアレイアウト

|コンポーネント |目的 |出典 |
| --- | --- | --- |
| **ヘッダー** |マジック/バージョン/スキーマ ハッシュ、CRC64、長さ、圧縮タグを使用してペイロードをフレーム化します。 v1 では `VERSION_MINOR = 0x00` が必要で、サポートされているマスク (デフォルトは `0x00`) に対してヘッダー フラグを検証します。 | `norito::header` — `norito.md` (「ヘッダーとフラグ」、リポジトリ ルート) を参照してください。
| **ベアペイロード** |ハッシュ/比較に使用される決定論的な値エンコーディング。オンワイヤートランスポートでは常にヘッダーが使用されます。ベアバイトは内部専用です。 | `norito::codec::{Encode, Decode}` |
| **圧縮** |オプションの Zstd (および実験的な GPU アクセラレーション) は、ヘッダー圧縮バイトによって選択されます。 | `norito.md`, “圧縮ネゴシエーション” |

レイアウト フラグ レジストリ (packed-struct、packed-seq、フィールド ビットセット、コンパクト
長さ) は `norito::header::flags` にあります。 V1 のデフォルトのフラグは `0x00` ですが、
サポートされているマスク内の明示的なヘッダー フラグを受け入れます。未知のビットは
拒否されました。 `norito::header::Flags` は内部検査のために保持されており、
将来のバージョン。

## サポートを引き出す

`norito_derive` には、`Encode`、`Decode`、`IntoSchema`、および JSON ヘルパー派生が同梱されています。
主な規則:

- 派生は、AoS とパックされたコード パスの両方を生成します。 v1 のデフォルトは AoS です
  ヘッダー フラグがパックされたバリアントを選択しない限り、レイアウト (フラグ `0x00`)。
  実装は `crates/norito_derive/src/derive_struct.rs` にあります。
- レイアウトに影響する機能 (`packed-struct`、`packed-seq`、`compact-len`)
  ヘッダー フラグを介してオプトインし、ピア間で一貫してエンコード/デコードする必要があります。
- JSON ヘルパー (`norito::json`) は、決定論的な Norito ベースの JSON を提供します。
  オープンAPI。 `norito::json::{to_json_pretty, from_json}` を使用します。`serde_json` は使用しないでください。

## マルチコーデックと識別子テーブル

Norito は、そのマルチコーデック割り当てを `norito::multicodec` に保持します。参考資料
テーブル (ハッシュ、キー タイプ、ペイロード記述子) は `multicodec.md` で維持されます
リポジトリのルートにあります。新しい識別子が追加される場合:

1. `norito::multicodec::registry` を更新します。
2. `multicodec.md` のテーブルを拡張します。
3. ダウンストリーム バインディング (Python/Java) がマップを使用する場合は、それを再生成します。

## ドキュメントとフィクスチャを再生成する

現在散文の要約をホストしているポータルでは、上流の Markdown を使用します。
真実の情報源としての情報源:

- **仕様**: `norito.md`
- **マルチコーデック テーブル**: `multicodec.md`
- **ベンチマーク**: `crates/norito/benches/`
- **ゴールデン テスト**: `crates/norito/tests/`

Docusaurus オートメーションが稼働すると、ポータルは
これらからデータを取得する同期スクリプト (`docs/portal/scripts/` で追跡)
ファイル。それまでは、仕様が変更されるたびにこのページを手動で調整してください。