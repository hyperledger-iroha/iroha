---
lang: ja
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T17:57:58.226177+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% コンテンツ ホスティング レーン
% Iroha コア

# コンテンツ ホスティング レーン

コンテンツ レーンは、小さな静的バンドル (tar アーカイブ) をオンチェーンに保存し、サービスを提供します。
Torii から直接個々のファイルを取得します。

- **公開**: tar アーカイブ、オプションの有効期限を指定して `PublishContentBundle` を送信します。
  高さ、およびオプションのマニフェスト。バンドル ID は、
  タールボール。 Tar エントリは通常のファイルである必要があります。名前は正規化された UTF-8 パスです。
  サイズ/パス/ファイル数の上限は、`content` 構成 (`max_bundle_bytes`、
  `max_files`、`max_path_len`、`max_retention_blocks`、`chunk_size_bytes`)。
  マニフェストには、Norito-index ハッシュ、データスペース/レーン、キャッシュ ポリシーが含まれます
  (`max_age_seconds`、`immutable`)、認証モード (`public` / `role:<role>` /
  `sponsor:<uaid>`)、保持ポリシー プレースホルダー、および MIME オーバーライド。
- **重複排除**: tar ペイロードはチャンク化され (デフォルトは 64KiB)、1 回につき 1 回保存されます。
  参照カウントを含むハッシュ。バンドルをリタイアすると、チャンクがデクリメントされ、プルーニングされます。
- **サーブ**: Torii は `GET /v2/content/{bundle}/{path}` を公開します。応答ストリーム
  `ETag` = ファイル ハッシュ、`Accept-Ranges: bytes`、を使用してチャンク ストアから直接
  範囲のサポート、およびマニフェストから派生したキャッシュ制御。敬意を表して読みます
  マニフェスト認証モード: ロールゲートおよびスポンサーゲートの応答には正規のものが必要です
  署名されたリクエストヘッダー (`X-Iroha-Account`、`X-Iroha-Signature`)
  アカウント;見つからない/期限切れのバンドルは 404 を返します。
- **CLI**: `iroha content publish --bundle <path.tar>` (または `--root <dir>`)
  マニフェストを自動生成し、オプションの `--manifest-out/--bundle-out` を発行し、
  `--auth`、`--cache-max-age-secs`、`--dataspace`、`--lane`、`--immutable`、
  `--expires-at-height` はオーバーライドされます。 `iroha content pack --root <dir>` ビルド
  何も送信せずに決定的な tarball + マニフェストを作成します。
- **構成**: キャッシュ/認証ノブは `content.*` の `iroha_config` の下に存在します。
  (`default_cache_max_age_secs`、`max_cache_max_age_secs`、`immutable_bundles`、
  `default_auth_mode`) であり、公開時に適用されます。
- **SLO + 制限**: `content.max_requests_per_second` / `request_burst` および
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` キャップ読み取り側
  スループット。 Torii は、バイトの処理とエクスポートの前に両方を強制します
  `torii_content_requests_total`、`torii_content_request_duration_seconds`、および
  結果ラベル付きの `torii_content_response_bytes_total` メトリクス。レイテンシ
  ターゲットは `content.target_p50_latency_ms` / の下に存在します。
  `content.target_p99_latency_ms` / `content.target_availability_bps`。
- **不正行為の制御**: レート バケットは UAID/API トークン/リモート IP によってキー化され、
  オプションの PoW ガード (`content.pow_difficulty_bits`、`content.pow_header`)
  読み取りの前に必要です。 DA ストライプ レイアウトのデフォルトは次のとおりです。
  `content.stripe_layout` は、レシート/マニフェスト ハッシュにエコーされます。
- **領収書と DA 証拠**: 成功した応答が添付されます
  `sora-content-receipt` (base64 Norito フレームの `ContentDaReceipt` バイト) を運ぶ
  `bundle_id`、`path`、`file_hash`、`served_bytes`、提供されるバイト範囲、
  `chunk_root` / `stripe_layout`、オプションの PDP コミットメント、およびタイムスタンプ
  クライアントは、本体を再度読み取ることなく、取得した内容をピン留めできます。

主な参考文献:- データモデル: `crates/iroha_data_model/src/content.rs`
- 実行: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii ハンドラー: `crates/iroha_torii/src/content.rs`
- CLI ヘルパー: `crates/iroha_cli/src/content.rs`