---
lang: ja
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Norito Streaming

Norito Streaming は Torii と SoraNet を横断するライブメディアフロー向けのワイヤフォーマット、制御フレーム、参照コーデックを定義します。正規仕様はワークスペースルートの `norito_streaming.md` にあり、このページでは運用者と SDK 作者が必要とする要素と設定ポイントを抜粋しています。

## ワイヤフォーマットと制御プレーン

- **マニフェストとフレーム。** `ManifestV1` と `PrivacyRoute*` はセグメントのタイムライン、chunk 記述子、ルートヒントを表します。制御フレーム（`KeyUpdate`、`ContentKeyUpdate`、およびカデンスフィードバック）はマニフェストと並んで配置され、視聴者がデコード前にコミットメントを検証できます。
- **ベースラインコーデック。** `BaselineEncoder`/`BaselineDecoder` は単調な chunk id、タイムスタンプの算術、コミットメント検証を強制します。ホストは viewer または relay に提供する前に `EncodedSegment::verify_manifest` を呼び出す必要があります。
- **feature bits。** 機能ネゴシエーションは `streaming.feature_bits`（デフォルト `0b11` = baseline feedback + privacy route provider）を広告し、relay とクライアントが互換性のない peer を決定的に拒否できるようにします。

## 鍵・スイート・カデンス

- **アイデンティティ要件。** ストリーミング制御フレームは常に Ed25519 で署名されます。専用鍵は `streaming.identity_public_key`/`streaming.identity_private_key` で指定でき、指定しない場合はノードのアイデンティティを再利用します。
- **HPKE スイート。** `KeyUpdate` は共通の最小スイートを選択します。スイート #1 は必須（`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`）で、`Kyber1024` へのオプションのアップグレードパスがあります。スイート選択はセッションに保存され、各更新で検証されます。
- **ローテーション。** パブリッシャは 64 MiB または 5 分ごとに署名済み `KeyUpdate` を発行します。`key_counter` は厳密に増加する必要があり、後退は致命的なエラーです。`ContentKeyUpdate` は交代する Group Content Key を配布し、交渉済み HPKE スイートでラップして、ID + 有効期間でセグメント復号をゲートします。
- **スナップショット。** `StreamingSession::snapshot_state` と `restore_from_snapshot` は `{session_id, key_counter, suite, sts_root, cadence state}` を `streaming.session_store_dir`（デフォルト `./storage/streaming`）に保存します。復元時にトランスポートキーを再導出するため、クラッシュしてもセッションの秘密は漏れません。

## ランタイム設定

- **鍵素材。** `streaming.identity_public_key`/`streaming.identity_private_key`（Ed25519 multihash）で専用鍵を指定し、必要なら `streaming.kyber_public_key`/`streaming.kyber_secret_key` で Kyber 素材を指定します。既定値を上書きする場合は 4 つすべてが必要です。`streaming.kyber_suite` は `mlkem512|mlkem768|mlkem1024`（別名 `kyber512/768/1024`、デフォルト `mlkem768`）を受け付けます。
- **コーデックのガードレール。** CABAC はビルドで有効化されない限り無効のままです。バンドルされた rANS には `ENABLE_RANS_BUNDLES=1` が必要です。`streaming.codec.{entropy_mode,bundle_width,bundle_accel}` と、カスタムテーブルを使う場合の `streaming.codec.rans_tables_path` で強制します。バンドルの `bundle_width` は 2〜3（含む）でなければならず、幅 1 はレガシー専用です。
- **SoraNet ルート。** `streaming.soranet.*` は匿名トランスポートを制御します: `exit_multiaddr`（デフォルト `/dns/torii/udp/9443/quic`）、`padding_budget_ms`（デフォルト 25 ms）、`access_kind`（`authenticated` vs `read-only`）、任意の `channel_salt`、`provision_spool_dir`（デフォルト `./storage/streaming/soranet_routes`）、`provision_spool_max_bytes`（デフォルト 0、無制限）、`provision_window_segments`（デフォルト 4）、`provision_queue_capacity`（デフォルト 256）。
- **同期ゲート。** `streaming.sync` は AV ストリームのドリフト強制を切り替えます。`enabled`、`observe_only`、`ewma_threshold_ms`、`hard_cap_ms` が、タイミングのドリフトでセグメントを拒否する条件を決めます。

## 検証とフィクスチャ

- 正規の型定義とヘルパーは `crates/iroha_crypto/src/streaming.rs` にあります。
- 統合カバレッジは HPKE ハンドシェイク、content-key 配布、スナップショットのライフサイクルを検証します（`crates/iroha_crypto/tests/streaming_handshake.rs`）。ローカルでストリーミング面を検証するには `cargo test -p iroha_crypto streaming_handshake` を実行してください。
- レイアウト、エラーハンドリング、将来のアップグレードの詳細は、リポジトリルートの `norito_streaming.md` を参照してください。
