---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリ
タイトル: SoraFS チャンカー プロファイル レジストリ
Sidebar_label: チャンカー レジストリ
説明: SoraFS チャンカー レジストリ、プロファイル ID、パラメータ、ネゴシエーション プラン
---

:::note メモ
:::

## SoraFS チャンカー プロファイル レジストリ (SF-2a)

SoraFS スタック チャンク動作 名前空間レジストリ ネゴシエート ہے۔
プロファイル決定論的 CDC パラメータ、サーバー メタデータ、予想されるダイジェスト/マルチコーデックの割り当て、マニフェスト、CAR アーカイブ、および

著者プロフィール
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
メタデータの検証チェックリスト 提案テンプレートの作成 入力の送信 入力の送信
ガバナンスを承認する
[レジストリ展開チェックリスト](./chunker-registry-rollout-checklist.md)
[ステージング マニフェスト プレイブック](./staging-manifest-playbook) 備品、ステージング、制作、プロモーション、プロモーション

### プロフィール

|ネームスペース |名前 |セミバージョン |プロフィールID |最小 (バイト) |ターゲット (バイト) |最大 (バイト) |マスクをブレイクする |マルチハッシュ |別名 |メモ |
|----------|------|----------|---------------|-------------|-----|-------------|---------------|-----------|-----------|----------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (ブレイク3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | SF-1 の試合の標準プロフィール | SF-1 の試合結果

レジストリ コード میں `sorafs_manifest::chunker_registry` کے طور پر موجود ہے (جسے [`chunker_registry_charter.md`](./chunker-registry-charter.md) 管理 کرتا ہے)۔ ہر エントリ ایک `ChunkerProfileDescriptor` کے طور پر ظاہر ہوتی ہے جس میں:

* `namespace` – プロファイルの論理グループ化 (`sorafs`)
* `name` – 読み取り可能なプロファイル ラベル (`sf1`、`sf1-fast`、…)۔
* `semver` – パラメータ セット、セマンティック バージョン文字列
* `profile` – `ChunkProfile` (最小/ターゲット/最大/マスク)
* `multihash_code` – チャンク ダイジェスト、マルチハッシュ (`0x1f`)
  SoraFS デフォルト

マニフェスト `ChunkingProfileV1` プロファイル シリアル化 ہے۔構造レジストリのメタデータ
(名前空間、名前、サーバー) 生の CDC パラメーター 名前 エイリアス リスト レコード ラベル
コンシューマ `profile_id` レジストリ ルックアップ 不明な ID のインライン パラメータ フォールバック

レジストリとツール、検査、ヘルパー CLI、および

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

CLI のフラグと JSON のフラグ (`--json-out`、`--por-json-out`、`--por-proof-out`、
`--por-sample-out`) パス `-` パス パス パス パス `-` ペイロード stdout ストリーム ストリームああ、
ツール ツール データ パイプ データ パイプ メイン レポート デフォルトの動作 デフォルトの動作

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub یہی data mirror کرتا ہے، جو pipelines میں `--chunker-profile-id` selection کو script کرنے کے لیے convenient ہے۔ دونوں chunk store CLIs canonical handle form (`--profile=sorafs.sf1@1.0.0`) بھی accept کرتے ہیں تاکہ build scripts numeric IDs hard-code کرنے سے بچ سکیں:

```
```

`handle` field (`namespace.name@semver`) وہی ہے جو CLIs `--profile=…` کے ذریعے accept کرتے ہیں، اس لیے اسے automation میں براہ راست copy کرنا محفوظ ہے۔

### Negotiating chunkers

Gateways اور clients provider adverts کے ذریعے supported profiles advertise کرتے ہیں:

```
```

Multi-source chunk scheduling `range` capability کے ذریعے announce ہوتی ہے۔ CLI اسے `--capability=range[:streams]` کے ساتھ accept کرتا ہے، جہاں optional numeric suffix provider کی preferred range-fetch concurrency encode کرتا ہے (مثلاً `--capability=range:64` 64-stream budget advertise کرتا ہے)۔ جب یہ omit ہو تو consumers advert میں کہیں اور شائع شدہ general `max_streams` hint پر fallback کرتے ہیں۔

CAR data request کرتے وقت clients کو `Accept-Chunker` header بھیجنا چاہیے جو preference order میں `(namespace, name, semver)` tuples list کرے:

```

ゲートウェイ相互サポート プロファイル منتخب کرتے ہیں (デフォルト `sorafs.sf1@1.0.0`) اور فیصلہ `Content-Chunker` 応答ヘッダー کے ذریعے 反映 کرتے ہیں۔マニフェスト プロファイルの埋め込み ダウンストリーム ノード HTTP ネゴシエーション チャンク レイアウトの検証 チャンク レイアウトの検証



* **プライマリ パス** – CARv2、BLAKE3 ペイロード ダイジェスト (`0x1f` マルチハッシュ)
  `MultihashIndexSorted`، اور チャンク プロファイル اوپر کے مطابق レコード ہوتا ہے۔


### 適合性

* `sorafs.sf1@1.0.0` プロファイル公共設備 (`fixtures/sorafs_chunker`) اور `fuzz/sorafs_chunker` ٩ے تحت register corpora سے match کرتا ہے۔エンドツーエンドのパリティ Rust Go ノードのテスト 演習のテスト
* `chunker_registry::lookup_by_profile` 記述子パラメータをアサート `ChunkProfile::DEFAULT` 一致 偶発的発散 偶発的発散
* `iroha app sorafs toolkit pack` اور `sorafs_manifest_stub` は、レジストリ メタデータをマニフェストします。