---
lang: zh-hant
direction: ltr
source: docs/source/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7548d481edd33d7e325d22559a5f53f261fa302ffd8710a1626acc4a5705e428
source_last_modified: "2025-12-29T18:16:35.915400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 虛擬機 + Kotodama 文檔索引

該索引鏈接了 IVM、Kotodama 和 IVM 第一管道的主要設計和參考文檔。 日本語訳は [`README.ja.md`](./README.ja.md) を脅してください。

- IVM 架構和語言映射：`../../ivm.md`
- IVM 系統調用 ABI：`ivm_syscalls.md`
- 生成的系統調用常量：`ivm_syscalls_generated.md`（運行 `make docs-syscalls` 進行刷新）
- IVM 字節碼頭：`ivm_header.md`
- Kotodama 語法和語義：`kotodama_grammar.md`
- Kotodama 示例和系統調用映射：`kotodama_examples.md`
- 交易管道（IVM-第一個）：`../../new_pipeline.md`
- Torii 合約 API（清單）：`torii_contracts_api.md`
- 通用賬號/UAID操作指南：`universal_accounts_guide.md`
- JSON 查詢信封（CLI/工具）：`query_json.md`
- Norito 流媒體模塊參考：`norito_streaming.md`
- 運行時 ABI 示例：`samples/runtime_abi_active.md`、`samples/runtime_abi_hash.md`、`samples/find_active_abi_versions.md`
- ZK App API（附件、證明者、投票統計）：`zk_app_api.md`
- Torii ZK 附件/驗證者操作手冊：`zk/prover_runbook.md`
- Torii ZK App API 操作指南（附件/證明器；板條箱文檔）：`../../crates/iroha_torii/docs/zk_app_api.md`
- VK/證明生命週期（註冊、驗證、遙測）：`zk/lifecycle.md`
- Torii 操作員輔助（可見性端點）：`references/operator_aids.md`
- Nexus 默認通道快速入門：`quickstart/default_lane.md`
- MOCHI Supervisor 快速入門和架構：`mochi/index.md`
- JavaScript SDK 指南（快速入門、配置、發布）：`sdk/js/index.md`
- Swift SDK 奇偶校驗/CI 儀表板：`references/ios_metrics.md`
- 治理：`../../gov.md`
- 域名認可（委員會、政策、驗證）：`domain_endorsements.md`
- JDG 證明（離線驗證工具）：`jdg_attestations.md`
- 澄清協調提示：`coordination_llm_prompts.md`
- 路線圖：`../../roadmap.md`
- Docker 構建器映像使用：`docker_build.md`

使用技巧
- 使用外部工具（`koto_compile`、`ivm_run`）在 `examples/` 中構建並運行示例：
  - `make examples-run`（如果 `ivm_tool` 可用，則為 `make examples-inspect`）
- `integration_tests/tests/` 中的示例和標頭檢查的可選集成測試（默認情況下忽略）。管道配置
- 所有運行時行為均通過 `iroha_config` 文件進行配置。環境變量不用於操作員。
- 提供合理的默認值；大多數部署不需要更改。
- `[pipeline]` 下的相關密鑰：
  - `dynamic_prepass`：啟用 IVM 只讀預通道以派生訪問集（默認值：true）。
  - `access_set_cache_enabled`：根據 `(code_hash, entrypoint)` 緩存派生訪問集；禁用調試提示（默認值：true）。
  - `parallel_overlay`：並行構建覆蓋；提交保持確定性（默認值：true）。
  - `gpu_key_bucket`：在 `(key, tx_idx, rw_flag)` 上使用穩定基數的調度程序預傳遞的可選密鑰存儲；確定性 CPU 回退始終處於活動狀態（默認值： false）。
  - `cache_size`：全局 IVM 預解碼緩存的容量（保留解碼流）。默認值：128。增加可以減少重複執行的解碼時間。

文檔同步檢查
- 系統調用常量 (docs/source/ivm_syscalls_ generated.md)
  - 再生：`make docs-syscalls`
  - 僅檢查：`bash scripts/check_syscalls_doc.sh`
- Syscall ABI 表 (crates/ivm/docs/syscalls.md)
  - 僅檢查：`cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - 更新生成的部分（和代碼文檔表）：`cargo run -p ivm --bin gen_syscalls_doc -- --write`
- 指針 ABI 表（crates/ivm/docs/pointer_abi.md 和 ivm.md）
  - 僅檢查：`cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - 更新部分：`cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM 標頭策略和 ABI 哈希 (docs/source/ivm_header.md)
  - 僅檢查：`cargo run -p ivm --bin gen_header_doc -- --check` 和 `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - 更新部分：`cargo run -p ivm --bin gen_header_doc -- --write` 和 `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI
- GitHub Actions 工作流程 `.github/workflows/check-docs.yml` 在每次推送/PR 上運行這些檢查，如果生成的文檔偏離實現，則會失敗。
- [治理手冊](governance_playbook.md)