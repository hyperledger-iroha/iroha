---
id: chunker-registry
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note 正規ソース
`docs/source/sorafs/chunker_registry.md` を反映しています。レガシーの Sphinx ドキュメントセットが退役するまで、両方のコピーを同期してください。
:::

## SoraFS チャンカー・プロファイル・レジストリ (SF-2a)

SoraFS スタックは、小さな名前空間付きレジストリを通じてチャンク化の挙動を交渉します。
各プロファイルは決定的な CDC パラメータ、semver メタデータ、および manifest と CAR アーカイブで使われる想定の digest/multicodec を割り当てます。

プロファイルの作者は
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
を参照し、必要なメタデータ、検証チェックリスト、提案テンプレートを確認してから
新しいエントリを提出してください。ガバナンスが変更を承認したら、
[レジストリのロールアウトチェックリスト](./chunker-registry-rollout-checklist.md) と
[staging manifest のプレイブック](./staging-manifest-playbook) に従って、
fixtures を staging と production に昇格させます。

### プロファイル

| Namespace | 名前 | SemVer | プロファイル ID | 最小 (bytes) | ターゲット (bytes) | 最大 (bytes) | ブレークマスク | Multihash | エイリアス | 備考 |
|-----------|------|--------|-----------------|--------------|--------------------|--------------|----------------|-----------|-----------|------|
| `sorafs`  | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | SF-1 fixtures で使う正規プロファイル |

レジストリはコード上で `sorafs_manifest::chunker_registry` として存在します（[`chunker_registry_charter.md`](./chunker-registry-charter.md) が統治）。各エントリは次の内容を持つ `ChunkerProfileDescriptor` として表現されます:

* `namespace` – 関連するプロファイルの論理的なグループ化（例: `sorafs`）。
* `name` – 人が読めるプロファイル名 (`sf1`, `sf1-fast`, …)。
* `semver` – パラメータセットのセマンティックバージョン文字列。
* `profile` – 実際の `ChunkProfile`（min/target/max/mask）。
* `multihash_code` – チャンク digest を生成する際に使う multihash (`0x1f`
  は SoraFS のデフォルト)。

manifest は `ChunkingProfileV1` を介してプロファイルをシリアライズします。構造体は
レジストリメタデータ（namespace, name, semver）を raw CDC パラメータと上記の
エイリアス一覧と共に記録します。利用側はまず `profile_id` でレジストリ検索を試し、
未知の ID が現れた場合はインラインのパラメータへフォールバックするべきです;
推測なしで送信できます。レジストリのチャーター規則は、正規ハンドル
(`namespace.name@semver`) を `profile_aliases` の最初のエントリとし、以降に

ツールからレジストリを確認するには、次の helper CLI を実行します:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

JSON を書き出す CLI フラグ (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) は `-` をパスとして受け付け、payload を stdout にストリームします。
これにより、メインレポートを出力する既定の挙動を保ちながら、データをツールにパイプできます。

### 互換性マトリクスとロールアウト計画


以下の表は、`sorafs.sf1@1.0.0` の現在の対応状況を主要コンポーネント別にまとめたものです。
"Bridge" は CARv1 + SHA-256 の互換レーンを指し、明示的なクライアント交渉
（`Accept-Chunker` + `Accept-Digest`）が必要です。

| コンポーネント | 状態 | 備考 |
|---------------|------|------|
| `sorafs_manifest_chunk_store` | ✅ サポート | 正規ハンドル + エイリアスを検証し、`--json-out=-` でレポートをストリームし、`ensure_charter_compliance()` でレジストリチャーターを強制します。 |
| `sorafs_manifest_stub` | ⚠️ レガシー | レガシー manifest builder。CAR/manifest のパッケージングには `iroha app sorafs toolkit pack` を使い、決定的な再検証のために `--plan=-` を維持します。 |
| `sorafs_provider_advert_stub` | ⚠️ レガシー | オフライン検証 helper のみ。provider advert は公開パイプラインで生成し、`/v1/sorafs/providers` で検証してください。 |
| `sorafs_fetch`（developer orchestrator） | ✅ サポート | `chunk_fetch_specs` を読み取り、`range` 能力 payload を理解し、CARv2 出力を組み立てます。 |
| SDK fixtures（Rust/Go/TS） | ✅ サポート | `export_vectors` で再生成。正規ハンドルがエイリアス一覧の先頭に入り、council envelopes により署名されます。 |
| Torii gateway のプロファイル交渉 | ✅ サポート | `Accept-Chunker` の完全な文法を実装し、`Content-Chunker` ヘッダーを含め、明示的なダウングレード要求でのみ CARv1 bridge を公開します。 |

テレメトリのロールアウト:

- **チャンク取得テレメトリ** — Iroha CLI `sorafs toolkit pack` がチャンク digest、CAR メタデータ、PoR ルートを出力し、ダッシュボードへの取り込みに供します。
- **Provider adverts** — advert payload は能力とエイリアスメタデータを含みます。`/v1/sorafs/providers` でカバレッジを検証してください（例: `range` 能力の存在）。
- **Gateway 監視** — オペレーターは `Content-Chunker`/`Content-Digest` の組み合わせを報告し、予期しないダウングレードを検知するべきです。bridge の利用は廃止前にゼロへ収束することが期待されます。

廃止ポリシー: 後継プロファイルが批准されたら、提案書に記載された二重公開ウィンドウを設定し、

特定の PoR 証人を確認するには、chunk/segment/leaf のインデックスを指定し、必要に応じて証明をディスクに保存します:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

数値 ID (`--profile-id=1`) またはレジストリハンドル (`--profile=sorafs.sf1@1.0.0`) で
プロファイルを選択できます。ハンドル形式は、ガバナンスメタデータから namespace/name/semver
を直接渡すスクリプトに便利です。

`--promote-profile=<handle>` を使うと、登録済みのすべてのエイリアスを含む JSON メタデータ
ブロックを出力し、新しい既定プロファイルを昇格する際に `chunker_registry_data.rs` へ貼り付けられます:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

メインレポート（および任意の証明ファイル）には、ルート digest、サンプルされた leaf バイト
（16 進エンコード）、および segment/chunk の sibling digest が含まれ、検証者は 64 KiB/4 KiB の
層を `por_root_hex` 値に対して再ハッシュできます。

既存の証明を payload に対して検証するには、`--por-proof-verify` でパスを渡します
（CLI は証人が計算済みルートと一致したとき `"por_proof_verified": true` を追加します）:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

バッチサンプリングには `--por-sample=<count>` を使い、必要に応じて seed/出力パスを指定します。
CLI は決定的な順序（`splitmix64` seed）を保証し、要求が利用可能な leaf を超える場合は
自動的に切り詰めます:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

manifest stub は同じデータを反映しており、パイプラインで `--chunker-profile-id` の選択を
スクリプト化する際に便利です。両方の chunk store CLI は正規ハンドル形式
（`--profile=sorafs.sf1@1.0.0`）も受け付けるため、ビルドスクリプトで数値 ID を
ハードコードする必要がありません:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

`handle` フィールド（`namespace.name@semver`）は CLI が `--profile=…` で受け付ける形式と一致するため、
そのまま自動化へコピーして問題ありません。

### チャンカーの交渉

Gateways とクライアントは provider advert を通じて対応プロファイルを告知します:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (レジストリ経由で暗黙)
    capabilities: [...]
}
```

マルチソースのチャンクスケジューリングは `range` 能力で通知されます。CLI は
`--capability=range[:streams]` で受け付け、任意の数値サフィックスは provider の
range-fetch 並列度の希望を表します（例: `--capability=range:64` は 64-stream の
バジェットを広告します）。省略した場合、利用側は advert 内の別の場所にある
一般的な `max_streams` ヒントへフォールバックします。

CAR データを要求する際、クライアントは `Accept-Chunker` ヘッダーで
`(namespace, name, semver)` のタプルを優先順に列挙してください:

```

Gateways は相互に対応するプロファイルを選択（既定は `sorafs.sf1@1.0.0`）し、
`Content-Chunker` レスポンスヘッダーで決定を反映します。manifest には選択された
プロファイルが埋め込まれるため、下流ノードは HTTP 交渉に頼らずにチャンクの
レイアウトを検証できます。

### CAR 互換性

正規の manifest エンベロープは CIDv1 ルートと `dag-cbor` (`0x71`) を使用します。
レガシー互換のために、CARv1+SHA-2 のエクスポート経路を残しています:

* **プライマリ経路** – CARv2、BLAKE3 payload digest (`0x1f` multihash)、
  `MultihashIndexSorted`、上記のとおりチャンクプロファイルを記録。
* **レガシー bridge** – CARv1、SHA-256 payload digest (`0x12` multihash)。
  クライアントが `Accept-Chunker` を省略するか `Accept-Digest: sha2-256` を要求した場合に、
  サーバーはこのバリアントを公開してよい（MAY）。

manifest は常に CARv2/BLAKE3 のコミットメントを広告します。
レガシーレーンは互換性のための追加ヘッダーを提供しますが、正規 digest を置き換えてはいけません。

### 準拠性

* `sorafs.sf1@1.0.0` プロファイルは `fixtures/sorafs_chunker` の公開 fixtures と
  `fuzz/sorafs_chunker` に登録された corpora に対応します。エンドツーエンドの
  パリティは Rust、Go、Node のテストで検証されています。
* `chunker_registry::lookup_by_profile` は、descriptor のパラメータが
  `ChunkProfile::DEFAULT` に一致することをアサートし、偶発的な乖離を防ぎます。
* `iroha app sorafs toolkit pack` と `sorafs_manifest_stub` が生成する manifest にはレジストリメタデータが含まれます。
