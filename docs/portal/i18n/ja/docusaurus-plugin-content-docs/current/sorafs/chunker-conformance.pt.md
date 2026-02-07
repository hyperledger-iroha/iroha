---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: チャンカー準拠
タイトル: コンフォーミダード ドゥ チャンカー SoraFS
Sidebar_label: チャンカーの適合性
説明: チャンカー SF1 em フィクスチャと SDK の保存、パーフィル決定の要件。
---

:::note フォンテ カノニカ
エスタ・ページナ・エスペルハ`docs/source/sorafs/chunker_conformance.md`。マンテンハ・アンバスはコピア・シンクロニザダスとして。
:::

永続的に実装するために必要なコードを作成する必要があります
SoraFS (SF1) のチャンカーの確定性の比較。エレ・タンベム
再生のための記録、検証のための政治的活動
フィクスチャの管理OS、SDK のフィケム シンク。

## カノニコをパーフィルします

- ハンドル実行: `sorafs.sf1@1.0.0`
- シード デ エントラーダ (16 進数): `0000000000dec0ded`
- タマンホ アルボ: 262144 バイト (256 KiB)
- タマンホ ミニモ: 65536 バイト (64 KiB)
- タマンホ最大値: 524288 バイト (512 KiB)
- ローリングポリノミオ: `0x3DA3358B4DC173`
- シード・ダ・タベラギア：`sorafs-v1-gear`
- ブレークマスク：`0x0000FFFF`

参照の実装: `sorafs_chunker::chunk_bytes_with_digests_profile`。
Qualquer aceleracao SIMD は、製品の限界とダイジェストを同一に開発します。

## フィクスチャのバンドル

`cargo run --locked -p sorafs_chunker --bin export_vectors` として再生成
フィクスチャは、`fixtures/sorafs_chunker/` の OS セギンテス arquivos を出力します:

- `sf1_profile_v1.{json,rs,ts,go}` - コンスミドールのチャンクの制限
  Rust、TypeScript、Go。プライメイラでのカノニコ操作をサポート
  entrada em `profile_aliases`、seguido de quaisquer エイリアス alternativos (例:
  `sorafs.sf1@1.0.0`、デポジット `sorafs.sf1@1.0.0`)。命令的な詐欺行為
  `ensure_charter_compliance` NAO DEVE ser alterada。
- `manifest_blake3.json` - BLAKE3 コブリンドのフィクスチャのマニフェスト検証。
- `manifest_signatures.json` - assinaaturas do conselho (Ed25519) ソブレまたはダイジェストが明示されています。
- `sf1_profile_v1_backpressure.json` コーポラ ブルートス デントロ デ `fuzz/` -
  チャンカーのバックプレッシャーのテストにおけるストリーミングのシナリオは決定的です。

### 政治の政治

備品の再生成には、コンセルホの検証が含まれます。おお、ジェラドール
Rejeita は、sem assinatura a menos que `--allow-unsigned` seja passado明示的 (目的地)
apenas para Experimentacao local)。追加のみの拡張機能のエンベロープ
サン・デデュプリカドス・ポー・シグナタリオ。

コンセーリョを最大限に活用してください:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## ベリフィカオ

O helper de CI `ci/check_sorafs_fixtures.sh` reexecuta o gerador com
`--locked`。試合はディベルギレムかアッシナチュラス・ファルタレム、仕事はファルハ。使用する
ワークフローのスクリプトは、フィクスチャの環境に合わせて実行できます。

検証手順マニュアル:

1. `cargo test -p sorafs_chunker` を実行します。
2. `ci/check_sorafs_fixtures.sh` ローカルメンテを実行します。
3. que `git status -- fixtures/sorafs_chunker` esta limpo を確認します。

## アップグレードのプレイブック

SF1 の最新のチャンカー情報に応じて:

ヴェジャ タンベム: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) パラ
メタデータの要件、提案書のテンプレート、検証のチェックリスト。

1. Redija um `ChunkProfileUpgradeProposalV1` (veja RFC SF-1) の新しいパラメーター。
2. `export_vectors` レジストリまたはノボ ダイジェスト ドゥ マニフェストを介してフィクスチャを再生成します。
3. 定足数を明示し、コンセルホ エクスギドを割り当てます。 Todas として assinaaturas devem ser
   anexadas `manifest_signatures.json`。
4. SDK 機能 (Rust/Go/TS) のフィクスチャとして、ランタイムを超えて実現します。
5. コーパスファズを再生成して、パラメトロスムダレムを再生成します。
6. エステ ギア コンポーネント、パーフィル、シード、ダイジェストを新しいハンドルで実現します。
7. ロードマップを実行するためのテストと実際のテストが必要な状況をうらやましく思います。

大量のフェテムの限界を超え、安全なプロセスを消化する必要があります
SAO は無効であり、開発者はマージアダを開発します。