---
lang: zh-hans
direction: ltr
source: docs/source/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7548d481edd33d7e325d22559a5f53f261fa302ffd8710a1626acc4a5705e428
source_last_modified: "2025-12-29T18:16:35.915400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 虚拟机 + Kotodama 文档索引

该索引链接了 IVM、Kotodama 和 IVM 第一管道的主要设计和参考文档。 日本语訳は [`README.ja.md`](./README.ja.md) を胁してください。

- IVM 架构和语言映射：`../../ivm.md`
- IVM 系统调用 ABI：`ivm_syscalls.md`
- 生成的系统调用常量：`ivm_syscalls_generated.md`（运行 `make docs-syscalls` 进行刷新）
- IVM 字节码头：`ivm_header.md`
- Kotodama 语法和语义：`kotodama_grammar.md`
- Kotodama 示例和系统调用映射：`kotodama_examples.md`
- 交易管道（IVM-第一个）：`../../new_pipeline.md`
- Torii 合约 API（清单）：`torii_contracts_api.md`
- 通用账号/UAID操作指南：`universal_accounts_guide.md`
- JSON 查询信封（CLI/工具）：`query_json.md`
- Norito 流媒体模块参考：`norito_streaming.md`
- 运行时 ABI 示例：`samples/runtime_abi_active.md`、`samples/runtime_abi_hash.md`、`samples/find_active_abi_versions.md`
- ZK App API（附件、证明者、投票统计）：`zk_app_api.md`
- Torii ZK 附件/验证者操作手册：`zk/prover_runbook.md`
- Torii ZK App API 操作指南（附件/证明器；板条箱文档）：`../../crates/iroha_torii/docs/zk_app_api.md`
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- VK/证明生命周期（注册、验证、遥测）：`zk/lifecycle.md`
- Torii 操作员辅助（可见性端点）：`references/operator_aids.md`
- Nexus 默认通道快速入门：`quickstart/default_lane.md`
- MOCHI Supervisor 快速入门和架构：`mochi/index.md`
- JavaScript SDK 指南（快速入门、配置、发布）：`sdk/js/index.md`
- Swift SDK 奇偶校验/CI 仪表板：`references/ios_metrics.md`
- 治理：`../../gov.md`
- 域名认可（委员会、政策、验证）：`domain_endorsements.md`
- JDG 证明（离线验证工具）：`jdg_attestations.md`
- 澄清协调提示：`coordination_llm_prompts.md`
- 路线图：`../../roadmap.md`
- Docker 构建器映像使用：`docker_build.md`

使用技巧
- 使用外部工具（`koto_compile`、`ivm_run`）在 `examples/` 中构建并运行示例：
  - `make examples-run`（如果 `ivm_tool` 可用，则为 `make examples-inspect`）
- `integration_tests/tests/` 中的示例和标头检查的可选集成测试（默认情况下忽略）。管道配置
- 所有运行时行为均通过 `iroha_config` 文件进行配置。环境变量不用于操作员。
- 提供合理的默认值；大多数部署不需要更改。
- `[pipeline]` 下的相关密钥：
  - `dynamic_prepass`：启用 IVM 只读预通道以派生访问集（默认值：true）。
  - `access_set_cache_enabled`：根据 `(code_hash, entrypoint)` 缓存派生访问集；禁用调试提示（默认值：true）。
  - `parallel_overlay`：并行构建覆盖；提交保持确定性（默认值：true）。
  - `gpu_key_bucket`：在 `(key, tx_idx, rw_flag)` 上使用稳定基数的调度程序预传递的可选密钥存储；确定性 CPU 回退始终处于活动状态（默认值： false）。
  - `cache_size`：全局 IVM 预解码缓存的容量（保留解码流）。默认值：128。增加可以减少重复执行的解码时间。

文档同步检查
- 系统调用常量 (docs/source/ivm_syscalls_ generated.md)
  - 再生：`make docs-syscalls`
  - 仅检查：`bash scripts/check_syscalls_doc.sh`
- Syscall ABI 表 (crates/ivm/docs/syscalls.md)
  - 仅检查：`cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - 更新生成的部分（和代码文档表）：`cargo run -p ivm --bin gen_syscalls_doc -- --write`
- 指针 ABI 表（crates/ivm/docs/pointer_abi.md 和 ivm.md）
  - 仅检查：`cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - 更新部分：`cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM 标头策略和 ABI 哈希 (docs/source/ivm_header.md)
  - 仅检查：`cargo run -p ivm --bin gen_header_doc -- --check` 和 `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - 更新部分：`cargo run -p ivm --bin gen_header_doc -- --write` 和 `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI
- GitHub Actions 工作流程 `.github/workflows/check-docs.yml` 在每次推送/PR 上运行这些检查，如果生成的文档偏离实现，则会失败。
- [治理手册](governance_playbook.md)
