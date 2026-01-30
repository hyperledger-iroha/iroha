---
lang: ur
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/reference/norito-codec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9409408d889029b4ce50f8fc5345e85baeb93a30cc9e74dbb880846fd93a5ed7
source_last_modified: "2025-11-04T12:03:18.011902+00:00"
translation_last_reviewed: 2026-01-30
---

# Norito コーデック リファレンス

Norito は Iroha のカノニカルなシリアライゼーション層です。オンワイヤのメッセージ、オンディスクの payload、コンポーネント間 API のすべてが Norito を使うため、異なるハードウェア上でもノードは同一のバイト列に合意できます。このページは要点をまとめ、`norito.md` の完全な仕様にリンクします。

## コアレイアウト

| コンポーネント | 目的 | ソース |
| --- | --- | --- |
| **ヘッダー** | magic/version/schema hash、CRC64、長さ、圧縮タグで payload をフレーミングする。v1 は `VERSION_MINOR = 0x00` を必須とし、ヘッダーフラグをサポート済みマスク（既定 `0x00`）で検証する。 | `norito::header` — `norito.md`（"Header & Flags"、リポジトリルート）を参照 |
| **ベア payload** | ハッシュ/比較に使う決定的な値エンコード。オンワイヤ輸送は常にヘッダーを使い、ベアなバイト列は内部専用。 | `norito::codec::{Encode, Decode}` |
| **圧縮** | ヘッダーの圧縮バイトで選択される任意の Zstd（および実験的 GPU アクセラレーション）。 | `norito.md`, “Compression negotiation” |

レイアウトフラグのレジストリ（packed-struct, packed-seq, field bitset, compact lengths）は `norito::header::flags` にあります。V1 は既定で `0x00` を使いますが、サポート済みマスク内の明示フラグを受け入れます。未知ビットは拒否されます。`norito::header::Flags` は内部検査と将来バージョン向けに保持されます。

## derive サポート

`norito_derive` は `Encode`, `Decode`, `IntoSchema` と JSON helper derive を提供します。主な規約:

- derive は AoS と packed の両方のコードパスを生成する。v1 はヘッダーフラグで packed を指定しない限り AoS レイアウト（フラグ `0x00`）を使う。実装は `crates/norito_derive/src/derive_struct.rs`。
- レイアウトに影響する機能（`packed-struct`, `packed-seq`, `compact-len`）はヘッダーフラグで opt-in し、ノード間で一貫してエンコード/デコードされる必要がある。
- JSON helpers（`norito::json`）は公開 API 向けに Norito 裏付けの決定的 JSON を提供する。`norito::json::{to_json_pretty, from_json}` を使い、`serde_json` は使わない。

## Multicodec と識別子テーブル

Norito は multicodec の割り当てを `norito::multicodec` に保持します。参照テーブル（ハッシュ、鍵タイプ、payload 記述子）はリポジトリルートの `multicodec.md` で管理されます。新しい識別子を追加する場合:

1. `norito::multicodec::registry` を更新する。
2. `multicodec.md` のテーブルを拡張する。
3. マップを消費するなら downstream のバインディング（Python/Java）を再生成する。

## docs と fixtures の再生成

ポータルは現在プローズ要約をホストしているため、アップストリームの Markdown ソースを正としてください:

- **Spec**: `norito.md`
- **Multicodec table**: `multicodec.md`
- **Benchmarks**: `crates/norito/benches/`
- **Golden tests**: `crates/norito/tests/`

Docusaurus の自動化が稼働したら、ポータルは `docs/portal/scripts/` で追跡される sync スクリプト経由でこれらのファイルからデータを取得して更新されます。それまでの間、仕様が変わるたびにこのページを手動で整合させてください。
