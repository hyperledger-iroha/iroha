---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリ
タイトル: SoraFS チャンカーのパーファイルの登録
Sidebar_label: チャンカーの登録
説明: SoraFS のチャンカー登録用のパーフィル ID、パラメータおよびネゴシエーション プランの ID。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/chunker_registry.md` のページを参照してください。スフィンクスの記録を保存し、記録を保存する必要があります。
:::

## SoraFS (SF-2a) のチャンカーのパーファイルのレジストリ

SoraFS のスタックは、登録番号の登録と分割中央処理の処理を必要とします。
CDC 決定パラメータ、メタデータ、ダイジェスト/マルチコーデック エスペラード、マニフェスト CAR のパラメータを指定します。

パーファイルのロスオートレスデベンコンサルタント
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
必要なメタデータ、検証チェックリスト、新しいプラントの管理を行うためのチェックリスト。
カンビオ、シグエル、プルエバを乗り越える
[登録ロールアウトのチェックリスト](./chunker-registry-rollout-checklist.md) y el
[ステージングのためのマニフェストのプレイブック](./staging-manifest-playbook) パラ プロムーバー
ロスフィクスチャーとステージングとプロダクション。

### パーファイル

|ネームスペース |ノンブル |セミバージョン | ID のパーフィル |最小 (バイト) |ターゲット (バイト) |最大 (バイト) |マスカラ デ コルテ |マルチハッシュ |別名 |メモ |
|----------|----------|----------|-------------|-------------|-----|---------------|----------|-----------|-----------|----------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (ブレイク3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfil canonic usado en fixtures SF-1 |

`sorafs_manifest::chunker_registry` ([`chunker_registry_charter.md`](./chunker-registry-charter.md)) を登録してください。カーダ・エントラーダ
`ChunkerProfileDescriptor` に関する説明:

* `namespace` – パーファイル関連の管理 (p. ej.、`sorafs`)。
* `name` – 人間の読みやすいエチケット (`sf1`、`sf1-fast`、…)。
* `semver` – 主要な機能とバージョンのセマンティカ。
* `profile` – `ChunkProfile` 実数 (最小/ターゲット/最大/マスク)。
* `multihash_code` – マルチハッシュの使用法、チャンクの生成ダイジェスト (`0x1f`)
  SoraFS のデフォルト）。

El マニフェスト シリアル ファイル メディア `ChunkingProfileV1`。構造登記簿
CDC のメタデータの登録 (名前空間、名前、サーバー) の詳細情報
en bruto y la lista de alias mostrada arriba。ロス・コンスミドレス・デベリアン・インタール・ウナ
`profile_id` の登録情報をインラインで再確認する
aparezcan ID デスコノシドス。別名 garantiza que los clientes HTTP puedan のリスト
seguir enviando は、`Accept-Chunker` sin adivinar のheredados を処理します。ラス・レグラス・デ・ラ
カルタ デル レジストロ エキシジェン ケ エル ハンドル カノニコ (`namespace.name@semver`) シーラ
primera entrada en `profile_aliases`、seguida de cualquier 別名 heredado。

登録ツールの検査、CLI ヘルパーの取り出し:

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

CLI クエリに関する JSON のフラグ (`--json-out`、`--por-json-out`、`--por-proof-out`、
`--por-sample-out`) アセプタン `-` 共通、標準出力のペイロード送信
アーカイブをクリアします。マンティエン エルのツールの拡張機能
校長の欠陥を監視します。

### 展開のマトリズとリーグ戦の計画


`sorafs.sf1@1.0.0` ja で実際のポートレートをキャプチャするためのタブラ
コンポーネントのプリンシパル。 「ブリッジ」セキュリティ CARril CARv1 + SHA-256
クライアントの明示的な交渉が必要です (`Accept-Chunker` + `Accept-Digest`)。|コンポネ |エスタード |メモ |
|----------|----------|----------|
| `sorafs_manifest_chunk_store` | ✅ ソポルタド |カノニコ + エイリアスを処理し、`--json-out=-` および登録アプリケーション `ensure_charter_compliance()` 経由でレポートを送信します。 |
| `sorafs_manifest_stub` | ⚠️レティラード |マニフェスト燃料デソルポートのコンストラクタ。米国 `iroha app sorafs toolkit pack` は、CAR/マニフェストと管理に関する `--plan=-` の再検証の決定版です。 |
| `sorafs_provider_advert_stub` | ⚠️レティラード |オフライン検証のヘルパー。プロバイダーは、`/v2/sorafs/providers` を介してパイプラインの公開パイプラインを広告します。 |
| `sorafs_fetch` (開発者オーケストレーター) | ✅ ソポルタド | Lee `chunk_fetch_specs`、容量ペイロード `range` と CARv2 のアンサンブル。 |
| SDK のフィクスチャ (Rust/Go/TS) | ✅ ソポルタド | `export_vectors` 経由のリジェネラーダ。別名リストとコンセホの会社の情報を処理できます。 |
|ゲートウェイ Torii でのパーファイルのネゴシオン | ✅ ソポルタド | `Accept-Chunker` の完全な文法を実装し、ヘッダー `Content-Chunker` を含めて、ブリッジ CARv1 ソロとダウングレードの説明を説明します。 |

テレメトリの解除:

- **チャンクのフェッチのテレメトリ** - CLI で Iroha `sorafs toolkit pack` はチャンクのダイジェスト、メタデータを出力し、CAR がダッシュボードで PoR を取り込みます。
- **プロバイダー広告** — 広告のペイロードの損失には、容量やエイリアスのメタデータが含まれます。 `/v2/sorafs/providers` 経由の検証 (p. ej.、presencia de la capacidad `range`)。
- **ゲートウェイの監視** — ロス オペラドールズ デベン レポーター ロス パレオス `Content-Chunker`/`Content-Digest` パラ検出器が inesperados をダウングレードします。廃止される前に橋を渡ってください。

非推奨の政治: 後継者に対する安全性評価、二重公開プログラムの廃止
(documentada en la propuesta) antes de marcar `sorafs.sf1@1.0.0` como deprecado en el registro y eliminar el
ブリッジ CARv1 のゲートウェイ製造。

特定の PoR 検査、チャンク/セグメント/法とオプションの指標のプロポーシオナ
パーシステ・ラ・プルエバ・ア・ディスコ:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

レジストリ ハンドルの番号 (`--profile-id=1`) を選択する
(`--profile=sorafs.sf1@1.0.0`);スクリプトの処理に便利な形式
名前空間/名前/安全なメタデータを直接指定します。

米国 `--promote-profile=<handle>` パラ エミッター アン ブロック JSON デ メタデータ (includedo todos los alias)
レジストラドス) que puede pegarse en `chunker_registry_data.rs` al promover un nuevo perfil pordefeto:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

プリンシパルのレポート (プルエバのアーカイブに関するオプション) には、ダイジェスト レポート、保存されたバイト数の情報が含まれます
(16 進数のコード) y los ダイジェスト ヘルマノス デ セグメント/チャンク パラ ケ ロス ベリフィカドレス プエダン リハシアー
64 KiB/4 KiB の対比 `por_root_hex` のラス キャパス。

ペイロードに基づいて、有効なプルエバが存在するかどうかを判断します。
`--por-proof-verify` (CLI を使用して `"por_proof_verified": true` を検査してください)
ライス計算と一致します):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

米国 `--por-sample=<count>` は種子/サリダのプロポーシオナ ウナ ルータのオプションです。
El CLI は決定権を保証 (`splitmix64` を選択) および透明な形式での処理
最高の責任を負う義務:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

El manifest stub refleja los mismos datos, lo que es conveniente al automatizar la selección de
`--chunker-profile-id` en pipelines. Ambos CLIs de chunk store también aceptan la forma de handle canónico
(`--profile=sorafs.sf1@1.0.0`) para que los scripts de build puedan evitar hard-codear IDs numéricos:

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

El campo `handle` (`namespace.name@semver`) coincide con lo que aceptan los CLIs vía
`--profile=…`, por lo que es seguro copiarlo directamente a la automatización.

### Negociar chunkers

Gateways y clientes anuncian perfiles soportados vía provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (レジストリ経由の暗黙的)
    能力: [...]
}
```

La programación de chunks multi-source se anuncia vía la capacidad `range`. El CLI la acepta con
`--capability=range[:streams]`, donde el sufijo numérico opcional codifica la concurrencia preferida
de fetch por rango del proveedor (por ejemplo, `--capability=range:64` anuncia un presupuesto de 64 streams).
Cuando se omite, los consumidores vuelven al hint general `max_streams` publicado en otra parte del advert.

Al solicitar datos CAR, los clientes deben enviar un header `Accept-Chunker` que liste tuplas
`(namespace, name, semver)` en orden de preferencia:

```

損失ゲートウェイの選択と互換性の確認 (欠陥 `sorafs.sf1@1.0.0`)
ヘッダー デ レスペスタ `Content-Chunker` を介して決定を参照してください。損失の明示
エンベベン エル ペルフィル エレギド パラ ケ ロス ノードス ダウンストリーム プエダン バリダル エル レイアウト デ チャンク
HTTP との交渉に依存します。

### ソポルテ CAR

CARv1+SHA-2 の輸出記録:

* **Ruta primaria** – CARv2、ペイロード BLAKE3 のダイジェスト (`0x1f` マルチハッシュ)、
  `MultihashIndexSorted`、チャンク登録を実行します。
  PUEDEN 説明者は、さまざまな情報を提供し、顧客は除外されます `Accept-Chunker` o solicita
  `Accept-Digest: sha2-256`。

トランジションペロのデベンリプラザとダイジェストカノニコのアディショナレス。

### コンコンミダッド* El perfil `sorafs.sf1@1.0.0` se asigna a los fixtures públicos en
  `fixtures/sorafs_chunker` y a los corpora registrados en
  `fuzz/sorafs_chunker`。 Rust でのエンドツーエンドの接続、Go y Node
  メディアンテ・ラス・プルエバス・プロビスタス。
* `chunker_registry::lookup_by_profile` 記述子パラメータを確認します
  偶然の一致 `ChunkProfile::DEFAULT` 偶然の発散。
* ロスマニフェストは、`iroha app sorafs toolkit pack` および `sorafs_manifest_stub` の生成に、レジストリのメタデータを含みます。