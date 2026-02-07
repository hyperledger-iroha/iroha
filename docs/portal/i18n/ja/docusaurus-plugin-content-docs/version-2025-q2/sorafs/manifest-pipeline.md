---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS チャンク化 → マニフェスト パイプライン

このクイックスタートのコンパニオンでは、生に変換されるエンドツーエンドのパイプラインをトレースします。
SoraFS ピン レジストリに適した Norito マニフェストにバイトを追加します。内容は
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md) から改変。
正規の仕様と変更ログについては、そのドキュメントを参照してください。

## 1. 決定的にチャンク化する

SoraFS は SF-1 (`sorafs.sf1@1.0.0`) プロファイルを使用します: FastCDC からインスピレーションを得たローリング
最小チャンク サイズが 64KiB、ターゲットが 256KiB、最大が 512KiB、および
`0x0000ffff` ブレークマスク。プロフィールが登録されているのは、
`sorafs_manifest::chunker_registry`。

### 錆びのヘルパー

- `sorafs_car::CarBuildPlan::single_file` – チャンクのオフセット、長さ、および
  BLAKE3 は CAR メタデータを準備しながらダイジェストします。
- `sorafs_car::ChunkStore` – ペイロードをストリーミングし、チャンク メタデータを永続化し、
  64KiB / 4KiB Proof-of-Retrievability (PoR) サンプリング ツリーを派生します。
- `sorafs_chunker::chunk_bytes_with_digests` – 両方の CLI の背後にあるライブラリ ヘルパー。

### CLI ツール

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON には、順序付けられたオフセット、長さ、およびチャンク ダイジェストが含まれています。を永続化します。
マニフェストまたはオーケストレーターのフェッチ仕様を構築するときに計画します。

### PoR 証人

`ChunkStore` は `--por-proof=<chunk>:<segment>:<leaf>` を公開し、
`--por-sample=<count>` なので、監査人は確定的な証人セットを要求できます。ペア
これらのフラグを `--por-proof-out` または `--por-sample-out` に設定して JSON を記録します。

## 2. マニフェストをラップする

`ManifestBuilder` は、チャンク メタデータとガバナンス添付ファイルを組み合わせます。

- ルート CID (dag-cbor) および CAR コミットメント。
- エイリアスの証明とプロバイダー機能の主張。
- 評議会の署名とオプションのメタデータ (ビルド ID など)。

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

重要な出力:

- `payload.manifest` – Norito でエンコードされたマニフェスト バイト。
- `payload.report.json` – 人間/自動化が読み取れる概要を含む
  `chunk_fetch_specs`、`payload_digest_hex`、CAR ダイジェスト、およびエイリアス メタデータ。
- `payload.manifest_signatures.json` – マニフェスト BLAKE3 を含むエンベロープ
  ダイジェスト、チャンクプラン SHA3 ダイジェスト、およびソートされた Ed25519 署名。

`--manifest-signatures-in` を使用して、外部から提供されたエンベロープを検証します。
署名者を書き戻す前に、`--chunker-profile-id` または
`--chunker-profile=<handle>` レジストリの選択をロックします。

## 3. 公開して固定する

1. **ガバナンスへの提出** – マニフェストのダイジェストと署名を提供します
   ピンが認められるように評議会に封筒を送ります。外部監査人は次のことを行うべきです。
   チャンクプラン SHA3 ダイジェストをマニフェスト ダイジェストと一緒に保存します。
2. **ペイロードのピン** – 参照される CAR アーカイブ (およびオプションの CAR インデックス) をアップロードします。
   マニフェストで Pin レジストリに追加します。マニフェストと CAR が確実に共有するようにします。
   同じルート CID。
3. **テレメトリの記録** – JSON レポート、PoR 証人、およびフェッチを永続化します。
   リリースアーティファクトのメトリクス。これらの記録はオペレーターのダッシュボードにフィードされ、
   大きなペイロードをダウンロードせずに問題を再現するのに役立ちます。

## 4. マルチプロバイダーフェッチシミュレーション

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`

- `#<concurrency>` は、プロバイダーごとの並列処理を増加します (上記の `#4`)。
- `@<weight>` はスケジュールのバイアスを調整します。デフォルトは 1 です。
- `--max-peers=<n>` は、実行がスケジュールされているプロバイダーの数を制限します。
  発見により、期待よりも多くの候補が得られます。
- `--expect-payload-digest` および `--expect-payload-len` はサイレントから保護します
  腐敗。
- `--provider-advert=name=advert.to` は、前にプロバイダーの機能を検証します。
  シミュレーションでそれらを使用します。
- `--retry-budget=<n>` はチャンクごとの再試行回数 (デフォルト: 3) をオーバーライドするため、CI
  障害シナリオをテストするときに、回帰をより早く表面化できます。

`fetch_report.json` は、集約されたメトリクスを表示します (`chunk_retry_total`、
`provider_failure_rate` など) CI アサーションと可観測性に適しています。

## 5. レジストリの更新とガバナンス

新しいチャンカー プロファイルを提案する場合:

1. `sorafs_manifest::chunker_registry_data` で記述子を作成します。
2. `docs/source/sorafs/chunker_registry.md` および関連憲章を更新します。
3. フィクスチャ (`export_vectors`) を再生成し、署名されたマニフェストをキャプチャします。
4. ガバナンス署名付きの憲章遵守報告書を提出します。

オートメーションは正規ハンドル (`namespace.name@semver`) を優先し、フォールする必要があります。