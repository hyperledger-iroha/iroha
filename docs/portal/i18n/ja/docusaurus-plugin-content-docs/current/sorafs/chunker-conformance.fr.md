---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: チャンカー準拠
タイトル: チャンカー適合ガイド SoraFS
Sidebar_label: 適合チャンカー
説明: サーバーのプロファイルチャンカー SF1 の決定とフィクスチャと SDK の拡張とワークフロー。
---

:::note ソースカノニク
:::

CE ガイドは、システムを構築するためのガイドを作成します。
SoraFS (SF1) のプロファイル チャンカーの決定結果を確認します。オーストラリアの文書
再生成のワークフロー、署名の政治、検証のテープ作成
SDK と同期を保持するフィクスチャの製造業者。

## カノニクのプロフィール

- シード デントレ (16 進数) : `0000000000dec0ded`
- タイユ ケーブル : 262144 バイト (256 KiB)
- タイユ最小値: 65536 バイト (64 KiB)
- タイユ最大値: 524288 バイト (512 KiB)
- ローリングポリノーム : `0x3DA3358B4DC173`
- シード・デ・テーブルギア：`sorafs-v1-gear`
- 仮面破裂 : `0x0000FFFF`

参照実装: `sorafs_chunker::chunk_bytes_with_digests_profile`。
アクセラレーション SIMD は、限界を開発し、同一性をダイジェストします。

## フィクスチャのバンドル

`cargo run --locked -p sorafs_chunker --bin export_vectors` レジェネール
備品とフィチエの備品 `fixtures/sorafs_chunker/` :

- `sf1_profile_v1.{json,rs,ts,go}` — 正規のチャンクの制限
  Rust、TypeScript、Go のコンソアマチュア。 Chaque ficier annonce le handle canonique
  `sorafs.sf1@1.0.0`、puis `sorafs.sf1@1.0.0`)。 L'ordre est imposé par
  `ensure_charter_compliance` および NE DOIT PAS être 修正。
- `manifest_blake3.json` — マニフェスト検証 BLAKE3 クーブラント チャック フィシエ デ フィクスチャ。
- `manifest_signatures.json` — マニフェストのダイジェストによる署名 (Ed25519)。
- `sf1_profile_v1_backpressure.json` およびコーパス ブルート ダン `fuzz/` —
  ストリーミングの決定シナリオは、チャンカーのバックプレッシャーのテストで使用されます。

### 署名政治

備品の更新 **doit** には、有効な署名が含まれます。ジェネラトゥール
`--allow-unsigned` 最も古い明示的表現 (前例)
実験ロケールの一意性）。署名の封筒は追加のみではありません
署名による重複排除。

アジュター ユネ シグネチャー デュ コンセイユを注ぐ：

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## 検証

Le helper CI `ci/check_sorafs_fixtures.sh` rejoue le générateur avec
`--locked`。備品が多岐にわたり、署名が必要な場合は、作業を進めてください。利用する
ワークフローのスクリプトとフィクスチャの変更の最新情報。

検証マニュアルの作成 :

1. `cargo test -p sorafs_chunker` を実行します。
2. Lancez `ci/check_sorafs_fixtures.sh` ロケール。
3. `git status -- fixtures/sorafs_chunker` が適切であることを確認します。

## ニボーの戦略

Lorsqu'on は、SF1 で出会った新しいプロファイル チャンカーを提案します。

オーストラリアの声: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) を注ぎます
メタドンの緊急事態、提案のテンプレート、検証のチェックリスト。

1. `ChunkProfileUpgradeProposalV1` (RFC SF-1 に関する) の新しいパラメータを更新します。
2. `export_vectors` 経由で備品を更新し、マニフェストのヌーボー ダイジェストを委託します。
3. マニフェストに要求される定足数を示します。トゥート レ シグニチャー ドワヴァン エートル
   付録は `manifest_signatures.json` です。
4. SDK に関する最新のフィクスチャ (Rust/Go/TS) とクロスランタイムの保証。
5. パラメータの変更によるコーポラファズの再生成。
6. プロフィール、種、ダイジェストを含む、最新の最新情報ガイド。
7. 現時点でのロードマップのテストとミスを修正します。

プロセスに影響を与える変更は、プロセスを介さずに、チャンクやダイジェストの制限に影響を与えます
無効な要素や融合した要素はありません。