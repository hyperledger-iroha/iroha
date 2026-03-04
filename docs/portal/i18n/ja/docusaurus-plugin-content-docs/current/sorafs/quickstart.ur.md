---
lang: ur
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a88906cad02fa52840df72f7067820bf80051fed6cf4726f7cba9d880d5e27b7
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
---

# SoraFS クイックスタート

このハンズオンガイドでは、SoraFS のストレージ・パイプラインを支える
決定的な SF-1 チャンカー・プロファイル、マニフェスト署名、複数プロバイダ
取得フローを順に確認します。設計メモや CLI フラグの参照には、
[マニフェスト・パイプラインの詳細解説](manifest-pipeline.md)も併せて確認してください。

## 前提条件

- Rust ツールチェーン（`rustup update`）と、workspace をローカルにクローン済みであること。
- 任意: マニフェスト署名用の [OpenSSL 互換 Ed25519 キーペア](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)。
- 任意: Docusaurus ポータルをプレビューする場合は Node.js ≥ 18。

検証中は `export RUST_LOG=info` を設定すると、CLI の有用なメッセージが表示されます。

## 1. 決定的フィクスチャを更新

正規の SF-1 チャンクベクトルを再生成します。`--signing-key` を指定すると署名済み
マニフェスト封筒も生成されます。ローカル開発時のみ `--allow-unsigned` を使用してください。

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

出力:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json`（署名ありの場合）
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. ペイロードをチャンク化して計画を確認

`sorafs_chunker` を使って任意のファイルまたはアーカイブをチャンク化します。

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

主要フィールド:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` のパラメータを確認します。
- `chunks[]` – オフセット、長さ、BLAKE3 ダイジェストの順序付きリストです。

より大きなフィクスチャでは、proptest ベースの回帰テストを実行して、
ストリーミングとバッチのチャンク化が同期していることを確認します。

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. マニフェストを作成して署名

チャンク計画、エイリアス、ガバナンス署名を `sorafs-manifest-stub` でマニフェストに
まとめます。以下のコマンドは単一ファイルのペイロード例です。ディレクトリパスを
渡すとツリーをパッケージ化できます（CLI は辞書順で走査します）。

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` を確認するポイント:

- `chunking.chunk_digest_sha3_256` – オフセット/長さの SHA3 ダイジェストで、
  チャンクフィクスチャと一致します。
- `manifest.manifest_blake3` – マニフェスト封筒で署名される BLAKE3 ダイジェストです。
- `chunk_fetch_specs[]` – オーケストレーター向けの順序付き取得指示です。

実署名を入れる準備ができたら `--signing-key` と `--signer` を追加します。
コマンドは封筒を書き込む前にすべての Ed25519 署名を検証します。

## 4. 複数プロバイダ取得をシミュレート

開発用 fetch CLI を使い、チャンク計画を一つ以上のプロバイダに対してリプレイします。
CI のスモークテストやオーケストレーターのプロトタイプに最適です。

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

確認項目:

- `payload_digest_hex` はマニフェストレポートと一致する必要があります。
- `provider_reports[]` はプロバイダごとの成功/失敗件数を示します。
- `chunk_retry_total` が 0 以外の場合、バックプレッシャー調整が入っていることを示します。
- `--max-peers=<n>` を指定して、実行でスケジュールするプロバイダ数を制限し、
  CI シミュレーションを主要候補に集中させます。
- `--retry-budget=<n>` はチャンクごとの既定リトライ回数 (3) を上書きし、
  故障注入時にオーケストレーターの回帰をより早く検出します。

`--expect-payload-digest=<hex>` と `--expect-payload-len=<bytes>` を追加すると、
再構築ペイロードがマニフェストとずれた場合に即失敗します。

## 5. 次のステップ

- **ガバナンス統合** – マニフェストのダイジェストと `manifest_signatures.json` を
  評議会ワークフローに渡し、Pin Registry が可用性を告知できるようにします。
- **レジストリ交渉** – 新しいプロファイルを登録する前に
  [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md) を参照してください。
  自動化では数値 ID よりもカノニカルなハンドル（`namespace.name@semver`）を優先します。
- **CI 自動化** – 上記のコマンドをリリースパイプラインに追加し、ドキュメント、
  フィクスチャ、アーティファクトが署名済みメタデータ付きの決定的マニフェストを
  公開するようにします。
