# Iroha VM / Kotodama ドキュメント索引（日本語版）

このファイルは IVM、Kotodama、および IVM ファーストなトランザクションパイプラインに関する主要ドキュメントへのリンクを日本語でまとめたものです。英語版の索引は [`README.md`](./README.md) を参照してください。

- IVM アーキテクチャと言語対応: `../../ivm.md`
- IVM システムコール ABI: `ivm_syscalls.md`（生成版: `ivm_syscalls_generated.md`。更新する場合は `make docs-syscalls`）
- IVM バイトコードヘッダ: `ivm_header.md`
- Kotodama 文法と意味論: `kotodama_grammar.md`
- Kotodama のサンプルと syscall 対応表: `kotodama_examples.md`
- IVM ファーストのトランザクションパイプライン: `../../new_pipeline.md`
- Torii コントラクト API（マニフェスト）: `torii_contracts_api.md`
- JSON クエリ封筒（CLI／ツール向け）: `query_json.md`
- Norito ストリーミングモジュール: `norito_streaming.md`
- ランタイム ABI サンプル: `samples/runtime_abi_active.md`, `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- ZK アプリ API（添付ファイル／プローバ／投票集計）: `zk_app_api.md`
- Torii ZK 添付／プローバ運用手順: `zk/prover_runbook.md`
- Torii ZK アプリ API オペレータガイド: `../../crates/iroha_torii/docs/zk_app_api.md`
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- VK／プルーフのライフサイクル: `zk/lifecycle.md`
- Torii オペレータ向け補助資料: `references/operator_aids.md`
- MOCHI クイックスタート／アーキテクチャ: `mochi/index.md`
- Swift SDK パリティ／CI ダッシュボード: `references/ios_metrics.md`
- ガバナンス: `../../gov.md`
- ロードマップ: `../../roadmap.md`
- Docker ビルダーイメージの利用方法: `docker_build.md`

## 使い方メモ

- `examples/` 以下のサンプルは外部ツール（`koto_compile`, `ivm_run` など）を利用して実行できます。
  - `make examples-run`（`ivm_tool` が利用可能なら `make examples-inspect` も実行可能）
- サンプルやヘッダ検証に関する追加の統合テストは `integration_tests/tests/` にあり、デフォルトではスキップされます。

## パイプライン設定

- ランタイムの挙動は `iroha_config` で集中管理します。運用時に環境変数を利用することはありません。
- 既定値は実運用に適したバランスに設定されています。大半のデプロイでは変更不要です。
- `[pipeline]` セクションの主なキー:
  - `dynamic_prepass`: IVM のリードオンリープリパスを有効化し、アクセスセットを導出します（既定: true）。
  - `access_set_cache_enabled`: `(code_hash, entrypoint)` 単位で導出済みアクセスセットをキャッシュします。ヒントのデバッグ時は無効化できます（既定: true）。
  - `parallel_overlay`: オーバーレイ構築を並列実行します。コミットの決定論性は維持されます（既定: true）。
  - `gpu_key_bucket`: スケジューラのプリパス用に `(key, tx_idx, rw_flag)` の安定ラジックスキャンを GPU で行います。常に決定論的な CPU フォールバックがあります（既定: false）。
  - `cache_size`: IVM プリデコード（ストリーム保持）キャッシュの容量（既定: 128）。同一コードの再実行時にデコード時間を短縮できます。

## ドキュメント同期チェック

- システムコール定数: `docs/source/ivm_syscalls_generated.md`
  - 再生成: `make docs-syscalls`
  - 差分チェックのみ: `bash scripts/check_syscalls_doc.sh`
- システムコール ABI 表: `crates/ivm/docs/syscalls.md`
  - チェックのみ: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - 生成セクションの更新: `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- ポインタ ABI テーブル: `crates/ivm/docs/pointer_abi.md` と `ivm.md`
  - チェックのみ: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - セクション更新: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM ヘッダポリシーと ABI ハッシュ: `docs/source/ivm_header.md`
  - チェックのみ: `cargo run -p ivm --bin gen_header_doc -- --check` および `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - セクション更新: `cargo run -p ivm --bin gen_header_doc -- --write` および `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

## CI

`.github/workflows/check-docs.yml` はこれらのチェックをすべての push／PR で実行し、生成ドキュメントと実装の差異を検出します。
