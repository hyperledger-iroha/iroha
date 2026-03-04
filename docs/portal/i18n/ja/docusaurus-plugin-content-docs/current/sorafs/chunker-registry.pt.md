---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリ
タイトル: SoraFS のチャンカーの登録情報
Sidebar_label: チャンカーの登録
説明: ID のパーフィル、パラメータ、および計画のネゴシアカオ レジストリ チャンカー SoraFS。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/chunker_registry.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

## チャンカーの登録情報 SoraFS (SF-2a)

スタック SoraFS は、um registro pequeno com 名前空間を介したチャンク化をネゴシアします。
CDC 決定パラメータ、メタデータ サーバー、ダイジェスト/マルチコーデック エスペラード、マニフェスト CAR の詳細を確認できます。

自動開発コンサルタント
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
必要なメタデータ、サブメーターの新しいエントリを検証するためのプロポスタとモデルのチェックリスト。
ウマ・ヴェス・ケ・ア・ガバナンカはウマ・ムダンカ、シンガポールを承認する
[ロールアウト登録チェックリスト](./chunker-registry-rollout-checklist.md) e o
[マニフェスト ステージングのプレイブック](./staging-manifest-playbook) パラ プロモーター
OS の備品、ステージング、プロデュース。

### パーフィス

|ネームスペース |ノーム |セミバージョン | ID のパーフィル |最小 (バイト) |ターゲット (バイト) |最大 (バイト) |マスカラ デ ケブラ |マルチハッシュ |別名 |メモ |
|----------|------|----------|---------------|-------------|-----|-------------|-----------|-----------|----------|----------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (ブレイク3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Perfil canonico usado em フィクスチャ SF-1 |

`sorafs_manifest::chunker_registry` のコードは登録されていません ([`chunker_registry_charter.md`](./chunker-registry-charter.md))。カーダ・エントラーダ
エクスプレス コム `ChunkerProfileDescriptor` com:

* `namespace` - パフォーマンス関連の論理管理 (例、`sorafs`)。
* `name` - 人為的合法性 (`sf1`、`sf1-fast`、...)。
* `semver` - パラメトロスと接続されたセマンティカの機能。
* `profile` - o `ChunkProfile` 実数 (最小/ターゲット/最大/マスク)。
* `multihash_code` - マルチハッシュを使用し、チャンクの製品ダイジェストを実行します (`0x1f`
  デフォルトは SoraFS)。

O マニフェストは `ChunkingProfileV1` 経由でシリアル化されます。メタダドスの登録
 do registro (名前空間、名前、semver) junto com os parametros CDC brutos
これは、mostrada acima の別名リストです。 Consumidores devem primeiro tentar uma
Busca にレジストリがありません `profile_id` e recorrer aos parametros inline quando
ID デスコンヘシドス アパレセレム。クライアントの HTTP possam を保証するエイリアスのリスト
継続的な enviando は、`Accept-Chunker` sem adivinhar の代替処理を処理します。レグラがそうするように
canonico を登録する憲章 (`namespace.name@semver`) を登録します。
primeira entrada em `profile_aliases`、seguida por quaisquer の別名 alternativos。

検査、レジストリ、ツールの実行、CLI ヘルパーの実行:

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

フラグとしての Todas は、CLI que escrevem JSON (`--json-out`、`--por-json-out`、`--por-proof-out`、
`--por-sample-out`) aceitam `-` como caminho、標準出力のペイロードを送信できません
クリアー・ウム・アルキーヴォ。 Isso torna facil encadear os ダドス パラ ツーリング マンテンド o
コンポルタメント・パドラオ・デ・インプリミル・オ・リラトリオ・プリンシパル。

### 展開とインプラント計画のマトリクス


`sorafs.sf1@1.0.0` 番号のサポートステータスを参照してください。
主要なコンポーネント。 「ブリッジ」とは、faixa CARv1 + SHA-256 を指します。
要求はクライアントに明示的に要求します (`Accept-Chunker` + `Accept-Digest`)。|コンポネ |ステータス |メモ |
|----------|----------|----------|
| `sorafs_manifest_chunk_store` | ✅ サポート | `--json-out=-` を介して canonico + エイリアスを処理し、`--json-out=-` を介して関連ストリームを処理し、`ensure_charter_compliance()` を介して登録を行うアプリケーションを検証します。 |
| `sorafs_manifest_stub` | ⚠️レティラード |サポート用のマニフェストのビルダー。 CAR/マニフェストの `iroha app sorafs toolkit pack` パラメータを使用して、確定的な `--plan=-` パラメータを確認してください。 |
| `sorafs_provider_advert_stub` | ⚠️レティラード |オフラインでの検証のヘルパー。プロバイダーは、`/v1/sorafs/providers` 経由で開発者製品のパイプラインと公開有効性を広告します。 |
| `sorafs_fetch` (開発者オーケストレーター) | ✅ サポート | Le `chunk_fetch_specs`、容量 `range` のペイロードは CARv2 に準拠しています。 |
| SDK のフィクスチャ (Rust/Go/TS) | ✅ サポート | `export_vectors` 経由の Regeneradas; o canonico aparece primeiro em cada lista de aliases e e assinado por envelops do conselho を処理します。 |
|ゲートウェイなしのネゴシアカオ デ パーフィル Torii | ✅ サポート |ヘッダー `Content-Chunker` を含む、`Accept-Chunker` の完全な文法を実装して、CARv1 ブリッジの公開要求を明示的にダウングレードします。 |

テレメトリのロールアウト:

- **チャンクの取得テレメトリア** - CLI Iroha `sorafs toolkit pack` はチャンクのダイジェストを出力し、メタデータ CAR はダッシュボードから PoR を取得します。
- **プロバイダー広告** - 広告の OS ペイロードには、容量性のメタデータやエイリアスが含まれます。 `/v1/sorafs/providers` 経由の valide cobertura (例、presenca da capacidade `range`)。
- **ゲートウェイの監視** - オペランドの開発レポート OS パラメータ `Content-Chunker`/`Content-Digest` パラ検出器が inesperados をダウングレードします。エスペラーゼ・ケ・オ・ウソ・ド・ブリッジ・テンダ・ゼロ・アンテス・ダ・デプレカカオ。

非推奨の政治: 後継者に対する政府の決定、公共政策の議題
ブリッジ CARv1 はゲートウェイを製造します。

特定の PoR 検査、チャンク/セグメント/フォルハのオプションのインデックス
ディスコを続けないでください:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

登録ハンドルの数値 (`--profile-id=1`) を選択してください
(`--profile=sorafs.sf1@1.0.0`);スクリプトクエリに便利な形式ハンドル
encadeiam namespace/name/semver diretamente dos metadados de Government.

メタダドの JSON パラメータとして `--promote-profile=<handle>` を使用します (todos os エイリアスを含む)
登録者) que pode sercolado em `chunker_registry_data.rs` ao promover um novo perfil Padrao:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

主要な関係 (オプションのアルキーボ)、ダイジェスト ライズ、アモストラドスのバイト数を含む
(codificados em hex) e os ダイジェスト セグメント/チャンク パラケ オス 検証済みポッサム
64 KiB/4 KiB の対価 `por_root_hex` の再計算。

有効な有効な有効な有効なコントラ ウム ペイロードが存在する場合は、経由でカミーニョを渡します
`--por-proof-verify` (CLI アディシオナ `"por_proof_verified": true` Quando o testemunho)
計算に対応します):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

パラメータを設定するには、シード/サイダのオプションとして `--por-sample=<count>` を使用してください。
O CLI 保証または決定性 (シードされた COM `splitmix64`) と自動実行の完全な実行
要求を満たさない限り超過する必要があります:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

O manifest stub espelha os mesmos dados, o que e conveniente ao automatizar a selecao de
`--chunker-profile-id` em pipelines. Ambos os CLIs de chunk store tambem aceitam a forma de handle canonico
(`--profile=sorafs.sf1@1.0.0`) para que scripts de build evitem hard-codear IDs numericos:

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

O campo `handle` (`namespace.name@semver`) corresponde ao que os CLIs aceitam via
`--profile=...`, tornando seguro copiar direto para automacao.

### Negociar chunkers

Gateways e clientes anunciam perfis suportados via provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (レジストリ経由で暗黙的に)
    能力: [...]
}
```

O agendamento de chunks multi-source e anunciado via a capacidade `range`. O CLI aceita isso com
`--capability=range[:streams]`, onde o sufixo numerico opcional codifica a concorrencia preferida
para fetch por range do provider (por exemplo, `--capability=range:64` anuncia um budget de 64 streams).
Quando omitido, consumidores recorrem ao hint geral `max_streams` publicado em outro ponto do advert.

Ao solicitar dados CAR, clientes devem enviar um header `Accept-Chunker` listando tuplas
`(namespace, name, semver)` em ordem de preferencia:

```

ゲートウェイの選択と互換性のサポート (デフォルトは `sorafs.sf1@1.0.0`)
ヘッダー デ レスタ `Content-Chunker` を介して決定を反映します。マニフェスト
ダウンストリームのポッサム検証、チャンクのレイアウトの実行、パフォーマンスの確認
sem は、negociacao HTTP に依存します。

### サポートCAR

CARv1+SHA-2 のエクスポート機能の管理:

* **Caminho primario** - CARv2、ペイロード BLAKE3 のダイジェスト (`0x1f` マルチハッシュ)、
  `MultihashIndexSorted`、チャンク登録を実行します。
  PODEM エクスポート エスタ バリアント Quando or cliente省略 `Accept-Chunker` ou solicita
  `Accept-Digest: sha2-256`。

トランジションに合わせて、カノニコを消化するために代替品を開発してください。

### コンフォルミダード* O perfil `sorafs.sf1@1.0.0` マペイア パラ OS フィクスチャの公開
  `fixtures/sorafs_chunker` e os corpora registrados em
  `fuzz/sorafs_chunker`。 Rust、Go、Node のエンドツーエンドの実行のパリダー
  オス精巣フォルネシドスを介して。
* `chunker_registry::lookup_by_profile` は記述子を確認します
  `ChunkProfile::DEFAULT` パラ エビタール ダイバージェンシア アシデンタルに対応します。
* `iroha app sorafs toolkit pack` および `sorafs_manifest_stub` の製品マニフェストには、レジストリのメタデータが含まれます。