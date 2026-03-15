---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリ
タイトル: プロファイル チャンカーの登録 SoraFS
Sidebar_label: レジストリチャンカー
説明: プロファイル、パラメータ、および登録チャンカー SoraFS の交渉計画の ID。
---

:::note ソースカノニク
Cette ページは `docs/source/sorafs/chunker_registry.md` を参照します。 Gardez les deux は、スフィンクスのヘリテージ セットを完全に再現したものをコピーします。
:::

## プロファイル チャンカーの登録 SoraFS (SF-2a)

スタック SoraFS は、プチ登録ネームスペース経由でチャンク化の処理を行います。
CDC 決定パラメータの割り当て、メタドンの設定、ダイジェスト/マルチコーデックの出席、マニフェスト、アーカイブ CAR のプロファイルを確認します。

Les auteurs de profiles doivent コンサルタント
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
必要なメタドンを注ぎ、検証のチェックリストと前衛的な提案モデルを作成します
ヌーヴェルのメインディッシュ。変更を承認する必要があります。
ガバナンス、スベスラ
[登録ロールアウトのチェックリスト](./chunker-registry-rollout-checklist.md) など
[ステージングのためのマニフェストのプレイブック](./staging-manifest-playbook) プロモーションを行う
フィクスチャーとステージング、プロダクション。

### プロフィール

|ネームスペース |ノム |セミバージョン |プロフィールID |最小 (オクテット) |聖書 (オクテット) |最大 (オクテット) |仮面の破裂 |マルチハッシュ |別名 |メモ |
|----------|-----|----------|---------------|--------------|--------------|--------------|----------|-----------|----------|----------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (ブレイク3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` |プロフィール SF-1 の使用法 |

コード `sorafs_manifest::chunker_registry` を登録します ([`chunker_registry_charter.md`](./chunker-registry-charter.md) を登録します)。チャケの前菜
`ChunkerProfileDescriptor` 平均の最大有効期限:

* `namespace` – プロファイル情報の再グループ化 (例、`sorafs`)。
* `name` – 自由に閲覧できます (`sf1`、`sf1-fast`、…)。
* `semver` – バージョン セマンティック プール ル ジュ ド パラメータ。
* `profile` – `ChunkProfile` ファイル (最小/ターゲット/最大/マスク)。
* `multihash_code` – マルチハッシュを使用してチャンクのダイジェストを生成します (`0x1f`
  デフォルト SoraFS)。

`ChunkingProfileV1` 経由でプロファイルをシリアル化するマニフェスト。メタドンネの構造
du registre (namespace, name, semver) aux côtés des paramètres CDC bruts et de la liste d'alias ci-dessus。
Les consommateurs doivent d'abord tenter une recherche dans le registre par `profile_id` et revenir aux
パラメーターは、装置内でインラインの ID を取得します。クライアント HTTP の別名保証リストを表示します
du registre exigent que le handle canonique (`namespace.name@semver`) soit la première entrée de
`profile_aliases`、別名ヘリテスです。

インスペクタをレジストリに登録し、CLI ヘルパーを実行します。

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

JSON による CLI とクリベントのフラグ (`--json-out`、`--por-json-out`、`--por-proof-out`、
`--por-sample-out`) 受け入れ可能 `-` comme chemin, ce qui streame le payload vs stdout au lieu de
クレアンフィシエ。 Cela は保守的な配管と外部の配管を容易にします。
コンポートメント・パー・デフォー・ダンプリマー・ル・ラポール・プリンシパル。

### 展開と展開計画のマトリックス


`sorafs.sf1@1.0.0` でサポート行為をサポートするルールをキャプチャします。
作曲家プリンシポー。 「ブリッジ」設計 CARv1 + SHA-256 qui
明示的な交渉クライアントが必要です (`Accept-Chunker` + `Accept-Digest`)。

|構成材 |法令 |メモ |
|----------|----------|----------|
| `sorafs_manifest_chunk_store` | ✅ サポート | canonique + alias のハンドルを有効にし、`--json-out=-` を介して関係をストリームし、`ensure_charter_compliance()` を介して登録のアップリケを有効にします。 |
| `sorafs_fetch` (開発者オーケストレーター) | ✅ サポート | `chunk_fetch_specs` が点灯し、`range` の容量のペイロードを把握し、出撃する CARv2 を組み立てます。 |
|フィクスチャ SDK (Rust/Go/TS) | ✅ サポート | `export_vectors` 経由での登録。 le は、管理上の主要なアプリケーションの別名および署名を扱います。 |
|ゲートウェイ Torii のプロファイルの交渉 | ✅ サポート |ヘッダー `Content-Chunker` を含む文法 `Accept-Chunker` を実装し、ダウングレードの明示的なブリッジ CARv1 の要求を公開します。 |

テレメトリーの展開 :- **チャンクのフェッチの詳細** — CLI Iroha `sorafs toolkit pack` チャンクのダイジェスト、メタドン、CAR およびレースの PoR の取り込みおよびダッシュボードの分析。
- **プロバイダー広告** — 広告のペイロードには、容量および別名を含むメタドンが含まれます。 `/v1/sorafs/providers` 経由でクーベルチュールを検証します (例、présence de la capacité `range`)。
- **監視ゲートウェイ** — 操作者は報告者を操作し、接続 `Content-Chunker`/`Content-Digest` は検出器をダウングレードし、不注意を検出します。橋の使用法は、価値の低下と前衛的なものを区別します。

批判の政治: 成功者のプロフィールを評価し、二重出版の計画を立てる
(documentée dans la proposition) avant de marquer `sorafs.sf1@1.0.0` comme déprécié dans le registre et de retirer le
本番環境のゲートウェイのブリッジ CARv1。

PoR 固有の検査官、チャンク/セグメント/フィーユなどのインデックスをオプションで提供します。
持続性ラ・プリューヴ・シュール・ディスク：

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

登録番号のプロファイル (`--profile-id=1`) のハンドルを選択する
(`--profile=sorafs.sf1@1.0.0`) ;ラ・フォーム・ハンドル・エスト・プラティック・プール・レ・スクリプト qui
transmettent 名前空間/名前/semver の指示は、管理のメタドンを管理します。

`--promote-profile=<handle>` ブロック JSON デ メタドンを使用します (エイリアスを構成します)
enregistrés) qui peut être collé dans `chunker_registry_data.rs` lors de la Promotion d'un nouveau profile
デフォルト:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

ル・ラポール・プリンシパル (およびプリーヴ・オプションネルのフィシエ)
(16 進数のエンコード) セグメント/チャンクのダイジェストと検証の詳細
64 KiB/4 KiB の顔は `por_root_hex` です。

ペイロードの有効性を確認し、既存のペイロードを注ぎます。
`--por-proof-verify` (CLI を参照してください `"por_proof_verified": true` lorsque le témoin
ラシーン計算に対応):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

たくさんのチャンスを注ぎ、`--por-sample=<count>` とシード/出撃のイベントを活用してください。
Le CLI garantit un ordre déterministe (seedé avec `splitmix64`) および tronque automatiquement lorsque
la requête dépasse les feuilles disponibles :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Le manifest stub reflète les mêmes données, ce qui est pratique pour scripter la sélection de
`--chunker-profile-id` dans les pipelines. Les deux CLIs de chunk store acceptent aussi la forme de handle canonique
(`--profile=sorafs.sf1@1.0.0`) afin que les scripts de build évitent de coder en dur des IDs numériques :

```
$ Cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    「プロファイルID」: 1、
    "名前空間": "sorafs",
    "名前": "sf1",
    "semver": "1.0.0",
    "ハンドル": "sorafs.sf1@1.0.0",
    "min_size": 65536、
    「ターゲットサイズ」: 262144、
    "最大サイズ": 524288、
    "ブレークマスク": "0x0000ffff",
    「マルチハッシュコード」: 31
  }
】
```

Le champ `handle` (`namespace.name@semver`) correspond à ce que les CLIs acceptent via
`--profile=…`, ce qui permet de le copier directement dans l'automatisation.

### Négocier les chunkers

Les gateways et les clients annoncent les profils supportés via des provider adverts :

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (レジストリ経由で暗黙的)
    能力: [...]
}
```

La planification multi-source des chunks est annoncée via la capacité `range`. Le CLI l'accepte avec
`--capability=range[:streams]`, où le suffixe numérique optionnel encode la concurrence de fetch par range préférée
par le provider (par exemple, `--capability=range:64` annonce un budget de 64 streams).
Lorsqu'il est omis, les consommateurs reviennent à l'indication générale `max_streams` publiée ailleurs dans l'advert.

Lorsqu'ils demandent des données CAR, les clients doivent envoyer un header `Accept-Chunker` listant des tuples
`(namespace, name, semver)` par ordre de préférence :

```

ゲートウェイの選択とプロファイルのサポートの相互作用 (デフォルト `sorafs.sf1@1.0.0`)
ヘッダー デ レスポンス `Content-Chunker` を介して決定を反映します。マニフェスト
チャンクのレイアウトのダウンストリームの検証を統合し、プロファイルを選択します
HTTP によるネゴシエーションなしのアプリ。

### サポートカー

CARv1+SHA-2 のエクスポートを許可しない保守者:

* **Chemin プリンシパル** – CARv2、ペイロード BLAKE3 のダイジェスト (`0x1f` マルチハッシュ)、
  `MultihashIndexSorted`、チャンクのプロファイルは ci-dessus に登録されています。
  PEUVENT エクスポーザは、クライアント omet `Accept-Chunker` を要求します。
  `Accept-Digest: sha2-256`。

移行を補うために、正規のダイジェストを置き換えます。

### 適合

* Le profil `sorafs.sf1@1.0.0` は補助器具 publiques dans に対応します
  `fixtures/sorafs_chunker` et aux corpora enregistrés sous
  `fuzz/sorafs_chunker`。 Rust での試合と試合の練習、Go et Node のパリテ
  レ・テスト・フルニス経由。
* `chunker_registry::lookup_by_profile` 説明のパラメータを確認します
  特派員 à `ChunkProfile::DEFAULT` 突然の分岐を経験しました。
* `iroha app sorafs toolkit pack` および `sorafs_manifest_stub` による製品マニフェストには、レジストリのメタドンが含まれます。