---
lang: ja
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 415a4317a6ce46a8272c6474f9cbf670a9c6100af8f9ec74e2be299dc5b0be1c
source_last_modified: "2025-12-18T14:07:05.705507+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/agents/env_var_migration.md -->

# Env → Config 移行トラッカー

このトラッカーは `docs/source/agents/env_var_inventory.{json,md}` に示される
本番向けの環境変数トグルと、`iroha_config` への移行（または dev/test 専用への
明示的スコープ）計画を要約する。

注記: `ci/check_env_config_surface.sh` は、`AGENTS_BASE_REF` と比較して新しい
**本番** 環境 shim が現れた場合に失敗する。`ENV_CONFIG_GUARD_ALLOW=1` が
設定されていない限り、意図的な追加はここに記載してから override を使用すること。

## 完了した移行

- **IVM ABI opt-out** — `IVM_ALLOW_NON_V1_ABI` を削除。コンパイラは非 v1 ABI を
  無条件に拒否し、ユニットテストでエラーパスを保護する。
- **IVM debug banner env shim** — `IVM_SUPPRESS_BANNER` の env opt-out を廃止。
  バナー抑制はプログラム的 setter で引き続き利用可能。
- **IVM cache/sizing** — cache/prover/GPU sizing を `iroha_config` に移行
  (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`,
  `accel.max_gpus`) し、ランタイム env shim を削除。ホストは
  `ivm::ivm_cache::configure_limits` と `ivm::zk::set_prover_threads` を呼び、
  テストは env override の代わりに `CacheLimitsGuard` を使う。
- **Connect queue root** — クライアント設定に `connect.queue.root`
  (デフォルト: `~/.iroha/connect`) を追加し、CLI と JS 診断へ配線。
  JS ヘルパーは設定（または明示的 `rootDir`）を解決し、
  `allowEnvOverride` を通じて dev/test でのみ `IROHA_CONNECT_QUEUE_ROOT` を尊重する。
  テンプレートにノブを記載し、運用者は env override を不要にする。
- **Izanami network opt-in** — Izanami chaos ツールに明示的な `allow_net`
  CLI/config フラグを追加。実行は `allow_net=true`/`--allow-net` が必須となり、
- **IVM banner beep** — `IROHA_BEEP` env shim を `ivm.banner.{show,beep}`
  （デフォルト: true/true）に置換。バナー/ビープの配線は本番で
  設定のみを読む。dev/test ビルドでは手動トグルのため env override を尊重する。
- **DA spool override (tests only)** — `IROHA_DA_SPOOL_DIR` は `cfg(test)` の
  ヘルパー内に限定。本番コードは常に設定から spool パスを取得する。
- **Crypto intrinsics** — `IROHA_DISABLE_SM_INTRINSICS` / `IROHA_ENABLE_SM_INTRINSICS`
  を `crypto.sm_intrinsics` ポリシー（`auto`/`force-enable`/`force-disable`）に置換し、
  `IROHA_SM_OPENSSL_PREVIEW` ガードを削除。ホストは起動時にポリシーを適用し、
  bench/test は `CRYPTO_SM_INTRINSICS` で opt-in 可能。OpenSSL preview は
  設定フラグのみを尊重する。Izanami は既に `--allow-net`/永続設定を要求し、
  テストは env トグルではなくそのノブに移行済み。
- **FastPQ GPU tuning** — `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`
  を追加（デフォルト: `None`/`None`/`false`/`false`/`false`）し CLI 解析へ配線。
  `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` shim は dev/test の fallback として動作し、
  設定読み込み後は（未設定でも）無視される。ドキュメント/在庫は移行を示すため更新済み。
  【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`,
  `IVM_DEBUG_REGALLOC`, `IVM_DEBUG_METAL_ENUM`, `IVM_DEBUG_METAL_SELFTEST`,
  `IVM_FORCE_METAL_ENUM`, `IVM_FORCE_METAL_SELFTEST_FAIL`, `IVM_FORCE_CUDA_SELFTEST_FAIL`,
  `IVM_DISABLE_METAL`, `IVM_DISABLE_CUDA`) は共有ヘルパーで debug/test ビルドにのみ
  ゲートされ、本番バイナリは無視する一方、ローカル診断用ノブは維持される。
  env 在庫は dev/test 専用スコープを反映するよう再生成された。
- **FASTPQ fixture updates** — `FASTPQ_UPDATE_FIXTURES` は FASTPQ 統合テストにのみ存在。
  本番ソースは env トグルを読まなくなり、在庫は test-only を反映する。
- **Inventory refresh + scope detection** — env 在庫ツールは `build.rs` を build スコープとして
  タグ付けし、`#[cfg(test)]`/integration harness を追跡する。これにより test-only トグル
  （例: `IROHA_TEST_*`, `IROHA_RUN_IGNORED`）や CUDA build フラグが production カウント外に
  反映される。2025-12-07 に在庫を再生成（518 refs / 144 vars）。
- **P2P topology env shim release guard** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` は release ビルドで
  決定論的な起動エラーを発生（debug/test では warn のみ）し、本番ノードは
  `network.peer_gossip_period_ms` のみに依存する。env 在庫はガードと分類器更新を反映する。

## 高優先度移行（本番パス）

- _なし（cfg!/debug 検出を含む在庫更新済み、P2P shim 強化後も env-config guard は green）。_

## dev/test 専用トグルのフェンス

- 現行スイープ（2025-12-07）: build-only CUDA フラグ（`IVM_CUDA_*`）は `build` として
  スコープされ、harness トグル（`IROHA_TEST_*`, `IROHA_RUN_IGNORED`, `IROHA_SKIP_BIND_CHECKS`）は
  在庫で `test`/`debug` として登録される（`cfg!` ガード shim を含む）。追加のフェンスは不要。
  将来の追加は `cfg(test)`/bench-only ヘルパーの背後に置き、暫定 shim には TODO を付ける。

## ビルド時 env（そのまま保持）

- Cargo/feature env（`CARGO_*`, `OUT_DIR`, `DOCS_RS`, `PROFILE`, `CUDA_HOME`,
  `CUDA_PATH`, `JSONSTAGE1_CUDA_ARCH`, `FASTPQ_SKIP_GPU_BUILD` など）は
  build-script の範囲であり、ランタイム設定移行の対象外。

## 次のアクション

1) config-surface 更新後に `make check-env-config-surface` を実行し、
   新しい本番 env shim を早期に検知してサブシステム担当/ETA を割り当てる。  
2) 各スイープ後に `make check-env-config-surface` で在庫を更新し、
   トラッカーを新しい guardrails と同期させ、env-config guard の diff をノイズレスに保つ。
