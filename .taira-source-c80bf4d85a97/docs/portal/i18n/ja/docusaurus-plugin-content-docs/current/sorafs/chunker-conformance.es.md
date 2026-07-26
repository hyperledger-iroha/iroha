---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: チャンカー準拠
タイトル: SoraFS の適合性に関する情報
Sidebar_label: チャンカーの適合性
説明: チャンカー SF1 およびフィクスチャと SDK の保存およびパーフィルの決定に必要な要件。
---

:::メモ フエンテ カノニカ
`docs/source/sorafs/chunker_conformance.md` のページを参照してください。定期的にバージョンを確認し、退職後のドキュメントを保存してください。
:::

管理上の必要なコードを実装する必要があります
SoraFS (SF1) のチャンカーの詳細情報を確認します。タンビエン
再生のための文書、企業政治と検証のための損失
SDK の永久保存とフィクスチャの消費。

## カノニコをパーフィルします

- パーフィルのハンドル: `sorafs.sf1@1.0.0` (エイリアス heredado `sorafs.sf1@1.0.0`)
- シード デ エントラーダ (16 進数): `0000000000dec0ded`
- タマニョオブジェクト: 262144 バイト (256 KiB)
- タマニョ ミニモ: 65536 バイト (64 KiB)
- タマニョ最大値: 524288 バイト (512 KiB)
- ローリングポリノミオ: `0x3DA3358B4DC173`
- タブラギアのシード: `sorafs-v1-gear`
- ブレークマスク：`0x0000FFFF`

参照の実装: `sorafs_chunker::chunk_bytes_with_digests_profile`。
SIMD の高速化のプロデューサーは、同一性を制限し、ダイジェストします。

## フィクスチャのバンドル

`cargo run --locked -p sorafs_chunker --bin export_vectors` リジェネロス
フィクスチャとエミテ・ロス・シギエンテス・アーキボス・バホ`fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — 消費者向けのチャンクの制限
  Rust、TypeScript、Go。プリメーラのハンドルを保持するためのアーカイブ
  entrada en `profile_aliases`、seguido por cualquier alias heredado (p. ej.、
  `sorafs.sf1@1.0.0`、ルエゴ `sorafs.sf1@1.0.0`)。エル・オーデン・セ・インポネ・ポー
  `ensure_charter_compliance` y NO DEBE オルタナティブ。
- `manifest_blake3.json` — BLAKE3 のフィクスチャのアーカイブのマニフェスト検証。
- `manifest_signatures.json` — コンセホ会社 (Ed25519) マニフェストの要約。
- `sf1_profile_v1_backpressure.json` y corpora en bruto dentro de `fuzz/` —
  バック プレッシャー デル チャンカーのストリーミング サービスとプルエバスのシナリオを決定します。

### 企業政治

**デベ**の備品の再生には、コンセホの有効性が含まれます。エルジェネドール
rechaza la salida sin fimar a menos que se pase explícitamente `--allow-unsigned` (ペンサド)
ローカルでのソロパラ実験）。追加のみを行うための損失
企業による重複排除。

コンセホに関する合意事項:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## 検証

CI `ci/check_sorafs_fixtures.sh` を参照してください。
`--locked`。試合の日程はファルタンの企業と異なり、仕事は異なります。宇佐
ワークフローの夜行性とフィクスチャの環境に合わせたスクリプトの作成。

検証手順マニュアル:

1. エジェクタ `cargo test -p sorafs_chunker`。
2. `ci/check_sorafs_fixtures.sh` ローカルメンテを呼び出します。
3. `git status -- fixtures/sorafs_chunker` esté limpio を確認します。

## 現実化のハンドブック

SF1 の実際のチャンカーに関する新しい提案:

バージョン: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) パラ
メタデータの要件、プラントの管理、検証のチェックリスト。

1. `ChunkProfileUpgradeProposalV1` (RFC SF-1 版) の新しいパラメータを編集します。
2. `export_vectors` および新しい登録ダイジェスト マニフェストを介してフィクスチャを再生成します。
3. 要求を満たした上でマニフェストを提出する。トダス ラス ファームマス デベン
   `manifest_signatures.json` を調べます。
4. SDK の機能 (Rust/Go/TS) とクロスランタイムの実装を実現します。
5. カンビアン・ロス・パラメトロスのコーポラ・ファズを再生する。
6. パーフィル、シード、ダイジェストを実際に処理します。
7. ロードマップの実際の状況を確認します。

ロス・カンビオス・ケ・アフェクテン・ロス・リミテス・デ・チャンク・ロス・ダイジェスト・シン・セギール・エステ・プロセス
息子は無効であり、デベン融合はありません。