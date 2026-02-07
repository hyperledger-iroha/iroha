---
lang: ja
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-04T17:06:14.405886+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS クイックスタート

この実践ガイドでは、決定論的な SF-1 チャンカー プロファイルについて説明します。
SoraFS を支えるマニフェスト署名とマルチプロバイダーフェッチフロー
ストレージパイプライン。 [マニフェスト パイプラインの詳細](manifest-pipeline.md) と組み合わせます。
設計ノートおよび CLI フラグの参考資料については、こちらをご覧ください。

## 前提条件

- Rust ツールチェーン (`rustup update`)、ローカルにクローンされたワークスペース。
- オプション: [OpenSSL で生成された Ed25519 キーペア](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  マニフェストに署名するため。
- オプション: Docusaurus ポータルをプレビューする場合は、Node.js ≥ 18。

役立つ CLI メッセージを表示するために実験しながら、`export RUST_LOG=info` を設定します。

## 1. 決定論的フィクスチャを更新します

正規の SF-1 チャンキング ベクトルを再生成します。このコマンドは署名付きのメッセージも出力します
`--signing-key` が指定された場合はマニフェスト エンベロープ。 `--allow-unsigned`を使用
ローカル開発中のみ。

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

出力:

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (署名されている場合)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. ペイロードをチャンク化して計画を検査する

`sorafs_chunker` を使用して、任意のファイルまたはアーカイブをチャンクします。

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

主要なフィールド:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` パラメータを確認します。
- `chunks[]` – 順序付けられたオフセット、長さ、およびチャンク BLAKE3 ダイジェスト。

大規模なフィクスチャの場合は、proptest に基づく回帰を実行して、ストリーミングと
バッチチャンクは同期を保ちます:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. マニフェストを構築して署名する

次を使用して、チャンク プラン、エイリアス、およびガバナンス署名をマニフェストにラップします。
`sorafs-manifest-stub`。以下のコマンドは、単一ファイルのペイロードを示しています。パスする
ツリーをパッケージ化するためのディレクトリ パス (CLI は辞書順にたどります)。

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` をレビューしてください:

- `chunking.chunk_digest_sha3_256` – オフセット/長さの SHA3 ダイジェスト。
  チャンカーフィクスチャ。
- `manifest.manifest_blake3` – マニフェスト エンベロープで署名された BLAKE3 ダイジェスト。
- `chunk_fetch_specs[]` – オーケストレーター用の順序付けされたフェッチ命令。

実際の署名を提供する準備ができたら、`--signing-key` および `--signer` を追加します。
引数。このコマンドは、Ed25519 の署名をすべて検証してから、
封筒。

## 4. マルチプロバイダーの取得をシミュレートする

開発者フェッチ CLI を使用して、1 つ以上のチャンク プランを再生します。
プロバイダー。これは、CI スモーク テストやオーケストレーターのプロトタイピングに最適です。

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

アサーション:

- `payload_digest_hex` はマニフェスト レポートと一致する必要があります。
- `provider_reports[]` は、プロバイダーごとの成功/失敗数を示します。
- ゼロ以外の `chunk_retry_total` は、背圧調整を強調表示します。
- `--max-peers=<n>` を渡して、実行がスケジュールされているプロバイダーの数を制限します
  CI シミュレーションは主な候補に焦点を当て続けます。
- `--retry-budget=<n>` は、デフォルトのチャンクごとの再試行回数 (3) をオーバーライドします。
  失敗を挿入するときに、オーケストレーターの回帰をより早く表面化できます。

`--expect-payload-digest=<hex>` および `--expect-payload-len=<bytes>` を追加すると失敗します
再構築されたペイロードがマニフェストから逸脱すると高速になります。

## 5. 次のステップ

- **ガバナンス統合** – マニフェスト ダイジェストをパイプし、
  `manifest_signatures.json` を評議会のワークフローに追加すると、ピン レジストリが
  空き状況を宣伝します。
- **レジストリ ネゴシエーション** – [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md) を参照してください。
  新しいプロファイルを登録する前に。オートメーションでは正規ハンドルを優先する必要があります
  (`namespace.name@semver`) 数値 ID を使用します。
- **CI 自動化** – パイプラインをリリースするために上記のコマンドを追加します。
  フィクスチャとアーティファクトは、署名付きマニフェストとともに決定論的なマニフェストを公開します
  メタデータ。