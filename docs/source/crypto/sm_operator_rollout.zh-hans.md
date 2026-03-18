---
lang: zh-hans
direction: ltr
source: docs/source/crypto/sm_operator_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dffc2cf6c6e59f54d1fc22136ba93f75466509c699a4361a381bf7e0ce0d1dda
source_last_modified: "2025-12-29T18:16:35.943754+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SM 功能推出和遥测清单

此清单可帮助 SRE 和运营商团队启用 SM (SM2/SM3/SM4) 功能
一旦审核和合规门被清除，就可以安全设置。遵循本文档
以及 `docs/source/crypto/sm_program.md` 中的配置简介和
`docs/source/crypto/sm_compliance_brief.md` 中的法律/出口指南。

## 1. 飞行前准备
- [ ] 确认工作区发行说明将 `sm` 显示为仅验证或签名，
      取决于推出阶段。
- [ ] 验证队列正在运行从包含以下内容的提交构建的二进制文件
      SM 遥测计数器和配置旋钮。 （目标发布待定；跟踪
      在推广票中。）
- [ ] 在暂存节点上运行 `scripts/sm_perf.sh --tolerance 0.25`（每个目标
      架构）并存档摘要输出。该脚本现在自动选择
      标量基线作为加速模式的比较目标
      （SM3 NEON工作落地时，`--compare-tolerance`默认为5.25）；
      如果是主要的或比较的，则调查或阻止推出
      守卫失败。在 Linux/aarch64 Neoverse 硬件上捕获时，通过
      `--baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_<mode>.json --write-baseline`
      用主机的捕获覆盖导出的 `m3-pro-native` 中位数
      发货前。
- [ ] 确保 `status.md` 和部署票记录了合规性备案
      在需要它们的司法管辖区运行的任何节点（请参阅合规简介）。
- [ ] 如果验证器将 SM 签名密钥存储在中，则准备 KMS/HSM 更新
      硬件模块。

## 2. 配置更改
1. 运行 xtask 帮助程序以生成 SM2 密钥清单和准备粘贴的代码片段：
   ```bash
   cargo xtask sm-operator-snippet \
     --distid CN12345678901234 \
     --json-out sm2-key.json \
     --snippet-out client-sm2.toml
   ```
   当您只需要检查输出时，使用 `--snippet-out -`（以及可选的 `--json-out -`）将输出流式传输到标准输出。
   如果您更喜欢手动执行较低级别的 CLI 命令，则等效流程为：
   ```bash
   cargo run -p iroha_cli --features sm -- \
     crypto sm2 keygen \
     --distid CN12345678901234 \
     --output sm2-key.json

   cargo run -p iroha_cli --features sm -- \
     crypto sm2 export \
     --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
     --distid CN12345678901234 \
     --snippet-output client-sm2.toml \
     --emit-json --quiet
   ```
   如果 `jq` 不可用，请打开 `sm2-key.json`，复制 `private_key_hex` 值，并将其直接传递给导出命令。
2. 将生成的代码片段添加到每个节点的配置中（显示的值
   仅验证阶段；根据环境进行调整并保持键排序，如图所示）：
```toml
[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]   # remove "sm2" to stay in verify-only mode
sm2_distid_default = "1234567812345678"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```
3. 重新启动节点并按预期确认 `crypto.sm_helpers_available` 和（如果启用了预览后端）`crypto.sm_openssl_preview_enabled` 表面：
   - `/status` JSON (`"crypto":{"sm_helpers_available":true,"sm_openssl_preview_enabled":true,...}`)。
   - 每个节点的渲染 `config.toml`。
4. 更新清单/创世条目以将 SM 算法添加到允许列表，如果
   签名将在稍后推出。使用 `--genesis-manifest-json` 时
   没有预先签名的创世块，`irohad` 现在播种运行时加密
   直接从清单的 `crypto` 块获取快照 - 确保清单是
   在向前推进之前检查您的变革计划。## 3. 遥测与监控
- 抓取 Prometheus 端点并确保出现以下计数器/仪表：
  - `iroha_sm_syscall_total{kind="verify"}`
  - `iroha_sm_syscall_total{kind="hash"}`
  - `iroha_sm_syscall_total{kind="seal|open",mode="gcm|ccm"}`
  - `iroha_sm_openssl_preview`（0/1 仪表报告预览切换状态）
  - `iroha_sm_syscall_failures_total{kind="verify|hash|seal|open",reason="..."}`
- 启用 SM2 签名后挂钩签名路径；添加计数器
  `iroha_sm_sign_total` 和 `iroha_sm_sign_failures_total`。
- 创建 Grafana 仪表板/警报：
  - 故障计数器中的尖峰（窗口 5m）。
  - SM 系统调用吞吐量突然下降。
  - 节点之间的差异（例如，启用不匹配）。

## 4. 推出步骤
|相|行动|笔记|
|--------|---------|--------|
|仅验证 |将 `crypto.default_hash` 更新为 `sm3-256`，保留 `allowed_signing` 不带 `sm2`，监视验证计数器。 |目标：在不冒共识分歧风险的情况下执行 SM 验证路径。 |
|混合签名试点|允许有限的 SM 签名（验证器的子集）；监控签名计数器和延迟。 |确保回退到 Ed25519 仍然可用；如果遥测显示不匹配，则停止。 |
| GA 签名 |扩展 `allowed_signing` 以包含 `sm2`、更新清单/SDK 并发布最终 Runbook。 |需要封闭的审计结果、更新的合规文件和稳定的遥测。 |

### 准备情况审核
- **仅验证准备情况 (SM-RR1)。** 召集发布工程师、加密工作组、运营和法律。要求：
  - `status.md` 注明合规性备案状态 + OpenSSL 出处。
  - `docs/source/crypto/sm_program.md` / `sm_compliance_brief.md` / 此清单在上一个版本窗口内更新。
  - `defaults/genesis` 或特定于环境的清单显示 `crypto.allowed_signing = ["ed25519","sm2"]` 和 `crypto.default_hash = "sm3-256"`（如果仍处于第一阶段，则为不带 `sm2` 的仅验证变体）。
  - `scripts/sm_openssl_smoke.sh` + `scripts/sm_interop_matrix.sh` 日志附加到部署票证上。
  - 遥测仪表板 (`iroha_sm_*`) 已审核稳态行为。
- **签署飞行员准备就绪 (SM-RR2)。** 附加登机口：
  - RustCrypto SM 堆栈关闭的审计报告或由安全部门签署的用于补偿控制的 RFC。
  - 操作员运行手册（特定于设施）更新了签名回退/回滚步骤。
  - 试点队列的 Genesis 清单包括 `allowed_signing = ["ed25519","sm2"]`，并且允许列表镜像在每个节点配置中。
  - 记录退出/回滚计划（将 `allowed_signing` 切换回 Ed25519、恢复清单、重置仪表板）。
- **GA 准备就绪 (SM-RR3)。** 需要积极的试点报告、所有验证者管辖区的更新合规性文件、签署的遥测基线以及来自 Release Eng + Crypto WG + Ops/Legal triad 的发布票证批准。## 5. 包装和合规检查表
- **捆绑 OpenSSL/Tongsuo 工件。** 将 OpenSSL/Tongsuo 3.0+ 共享库 (`libcrypto`/`libssl`) 与每个验证程序包一起发送，或记录确切的系统依赖项。在发布清单中记录版本、构建标志和 SHA256 校验和，以便审核员可以跟踪供应商构建。
- **在 CI 期间进行验证。** 添加一个 CI 步骤，针对每个目标平台上的打包工件执行 `scripts/sm_openssl_smoke.sh`。如果启用了预览标志但提供程序无法初始化（缺少标头、不支持的算法等），则作业必定会失败。
- **发布合规性说明。** 使用捆绑的提供商版本、出口管制参考（GM/T、GB/T）以及 SM 算法所需的任何司法管辖区特定文件更新发行说明/`status.md`。
- **操作员运行手册更新。** 记录升级流程：暂存新的共享对象，使用 `crypto.enable_sm_openssl_preview = true` 重新启动对等点，确认 `/status` 字段和 `iroha_sm_openssl_preview` 仪表翻转到 `true`，并在预览遥测数据在机群中出现偏差时保留回滚计划（翻转配置标志或恢复包）。
- **证据保留。** 将 OpenSSL/Tongsuo 包的构建日志和签名证明与验证器发布工件一起存档，以便将来的审计可以重现来源链。

## 6. 事件响应
- **验证失败峰值：** 回滚到没有 SM 支持的版本或删除 `sm2`
  从 `allowed_signing`（根据需要恢复 `default_hash`）并故障转移到上一个
  调查期间释放。捕获失败的有效负载、比较哈希值和节点日志。
- **性能回归：** 将 SM 指标与 Ed25519/SHA2 基线进行比较。
  如果ARM内部路径导致发散，则设置`crypto.sm_intrinsics = "force-disable"`
  （功能切换待实施）并报告调查结果。
- **遥测差距：** 如果计数器丢失或未更新，请提出问题
  反对发布工程；在出现间隙之前不要继续进行更广泛的推广
  已解决。

## 7.清单模板
- [ ] 配置已暂存且对等点已重新启动。
- [ ] 遥测计数器可见并已配置仪表板。
- [ ] 记录合规/法律步骤。
- [ ] 推出阶段由 Crypto WG/Release TL 批准。
- [ ] 已完成推出后审查并记录结果。

在部署票证中维护此清单，并在以下情况下更新 `status.md`：
机队在阶段之间进行转换。