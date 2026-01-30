---
lang: ar
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d532e56e27673c6adc5544305e77634a9c0b3af3639bd3d754baaa0b894c7a3b
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
---

# SoraFS チャンク化 → マニフェストパイプライン

この quickstart の補足は、生のバイト列を SoraFS Pin Registry に適した Norito マニフェストへ
変換するエンドツーエンドのパイプラインを追います。内容は
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
をベースにしているため、正規仕様と変更履歴はそちらを参照してください。

## 1. 決定的にチャンク化する

SoraFS は SF-1 (`sorafs.sf1@1.0.0`) プロファイルを使用します。FastCDC 由来のローリング
ハッシュで、最小チャンクサイズは 64 KiB、ターゲットは 256 KiB、最大は 512 KiB、ブレーク
マスクは `0x0000ffff` です。このプロファイルは `sorafs_manifest::chunker_registry` に
登録されています。

### Rust ヘルパー

- `sorafs_car::CarBuildPlan::single_file` – CAR メタデータの準備中に、チャンクのオフセット、
  長さ、BLAKE3 ダイジェストを出力します。
- `sorafs_car::ChunkStore` – payload をストリームし、チャンクのメタデータを保持し、
  64 KiB / 4 KiB の Proof-of-Retrievability (PoR) サンプリングツリーを導出します。
- `sorafs_chunker::chunk_bytes_with_digests` – 2 つの CLI の背後にあるライブラリヘルパーです。

### CLI ツール

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON には順序付きのオフセット、長さ、チャンクダイジェストが含まれます。マニフェストや
オーケストレーターの取得仕様を構築する際は、このプランを保存してください。

### PoR の証跡

`ChunkStore` は `--por-proof=<chunk>:<segment>:<leaf>` と `--por-sample=<count>` を提供し、
監査人が決定的な証跡セットを要求できるようにします。これらのフラグを
`--por-proof-out` または `--por-sample-out` と組み合わせて JSON を記録してください。

## 2. マニフェストをラップする

`ManifestBuilder` はチャンクのメタデータとガバナンスの添付物を結合します。

- ルート CID (dag-cbor) と CAR コミットメント。
- エイリアス証明とプロバイダー能力の主張。
- 評議会の署名と任意のメタデータ（例: build ID）。

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

重要な出力:

- `payload.manifest` – Norito でエンコードされたマニフェストバイト。
- `payload.report.json` – 人間/自動化向けの要約で、`chunk_fetch_specs`、`payload_digest_hex`、
  CAR ダイジェスト、エイリアスメタデータを含みます。
- `payload.manifest_signatures.json` – マニフェストの BLAKE3 ダイジェスト、チャンクプランの
  SHA3 ダイジェスト、ソート済み Ed25519 署名を含む封筒。

`--manifest-signatures-in` を使って外部署名者からの封筒を検証してから再出力し、
`--chunker-profile-id` または `--chunker-profile=<handle>` でレジストリの選択を固定します。

## 3. 公開とピン留め

1. **ガバナンス提出** – マニフェストのダイジェストと署名封筒を評議会に渡し、pin が承認される
   ようにします。外部監査人はチャンクプランの SHA3 ダイジェストをマニフェストのダイジェストと
   一緒に保存してください。
2. **payload のピン留め** – マニフェストに参照された CAR アーカイブ（任意の CAR インデックスも含む）
   を Pin Registry にアップロードします。マニフェストと CAR が同じルート CID を共有していることを
   確認してください。
3. **テレメトリの記録** – JSON レポート、PoR 証跡、fetch のメトリクスをリリース成果物に保存します。
   これらの記録はオペレーターのダッシュボードに使われ、大きな payload をダウンロードせずに問題を
   再現するのに役立ちます。

## 4. マルチプロバイダ fetch シミュレーション

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` はプロバイダごとの並列度を増やします（上記の `#4`）。
- `@<weight>` はスケジューリングのバイアスを調整します。デフォルトは 1 です。
- `--max-peers=<n>` は、探索で候補が多すぎる場合に実行へ割り当てるプロバイダ数を制限します。
- `--expect-payload-digest` と `--expect-payload-len` はサイレントな破損を防ぎます。
- `--provider-advert=name=advert.to` はシミュレーション前にプロバイダの能力を検証します。
- `--retry-budget=<n>` はチャンクごとのリトライ数（デフォルト: 3）を上書きし、CI が障害シナリオの
  回帰をより早く検出できるようにします。

`fetch_report.json` は集約メトリクス（`chunk_retry_total`、`provider_failure_rate` など）を
提供し、CI のアサーションと可観測性に適しています。

## 5. レジストリ更新とガバナンス

新しい chunker プロファイルを提案する場合:

1. `sorafs_manifest::chunker_registry_data` に記述子を作成します。
2. `docs/source/sorafs/chunker_registry.md` と関連する charter を更新します。
3. fixtures（`export_vectors`）を再生成し、署名済みマニフェストを取得します。
4. governance 署名付きで charter 遵守レポートを提出します。

自動化は正規の handle（`namespace.name@semver`）を優先し、後方互換が必要な場合にのみ数値 ID
へフォールバックしてください。
