---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2025-12-29T18:16:35.927510+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK 安全目标 — ETSI EN 319 401 对齐

|领域|价值|
|--------|--------|
|文档版本 | 0.1 (2026-02-12) |
|范围 | Android SDK（`java/iroha_android/` 下的客户端库以及支持脚本/文档）|
|业主|合规与法律（索菲亚·马丁斯）|
|审稿人| Android 项目主管、发布工程、SRE 治理 |

## 1. TOE 描述

评估目标 (TOE) 包括 Android SDK 库代码 (`java/iroha_android/src/main/java`)、其配置界面（`ClientConfig` + Norito 摄取）以及 `roadmap.md` 中针对里程碑 AND2/AND6/AND7 引用的操作工具。

主要组成部分：

1. **配置摄取** — `ClientConfig` 线程 Torii 端点、TLS 策略、重试和来自生成的 `iroha_config` 清单的遥测挂钩，并强制执行初始化后的不变性 (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`)。
2. **密钥管理/StrongBox** — 硬件支持的签名通过 `SystemAndroidKeystoreBackend` 和 `AttestationVerifier` 实施，策略记录在 `docs/source/sdk/android/key_management.md` 中。证明捕获/验证使用 `scripts/android_keystore_attestation.sh` 和 CI 帮助程序 `scripts/android_strongbox_attestation_ci.sh`。
3. **遥测和编辑** - 通过 `docs/source/sdk/android/telemetry_redaction.md` 中描述的共享模式进行检测，导出散列权限、分桶设备配置文件，并覆盖支持手册强制执行的审核挂钩。
4. **操作手册** — `docs/source/android_runbook.md`（操作员响应）和 `docs/source/android_support_playbook.md`（SLA + 升级）通过确定性覆盖、混沌演练和证据捕获强化 TOE 的操作足迹。
5. **发布来源** — 基于 Gradle 的构建使用 CycloneDX 插件以及 `docs/source/sdk/android/developer_experience_plan.md` 和 AND6 合规性检查表中捕获的可重现构建标志。发布工件在 `docs/source/release/provenance/android/` 中进行签名和交叉引用。

## 2. 资产和假设

|资产|描述 |安全目标|
|--------|-------------|--------------------|
|配置清单 | Norito 派生的 `ClientConfig` 快照随应用程序分发。 |静态时的真实性、完整性和机密性。 |
|签名密钥 |通过 StrongBox/TEE 提供商生成或导入的密钥。 | StrongBox 首选项、证明日志记录、无密钥导出。 |
|遥测流 |从 SDK 工具导出的 OTLP 跟踪/日志/指标。 |假名化（哈希授权）、最小化 PII、覆盖审计。 |
|账本交互 | Norito 有效负载、准入元数据、Torii 网络流量。 |相互身份验证、抗重放请求、确定性重试。 |

假设：

- 移动操作系统提供标准沙箱+SELinux； StrongBox 设备实现了 Google 的 keymaster 界面。
- 运营商为 Torii 端点提供由理事会信任的 CA 签署的 TLS 证书。
- 在发布到 Maven 之前，构建基础设施遵循可重复构建的要求。

## 3. 威胁与控制|威胁|控制|证据|
|--------|---------|----------|
|被篡改的配置清单| `ClientConfig` 在应用之前验证清单（哈希+架构），并通过 `android.telemetry.config.reload` 记录拒绝的重新加载。 | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`； `docs/source/android_runbook.md` §1–2。 |
|签名密钥的泄露 | StrongBox 所需的策略、证明工具和设备矩阵审核可识别偏差；覆盖每个事件记录的内容。 | `docs/source/sdk/android/key_management.md`； `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`； `scripts/android_strongbox_attestation_ci.sh`。 |
|遥测中的 PII 泄露 | Blake2b 散列权限、分桶设备配置文件、运营商遗漏、覆盖日志记录。 | `docs/source/sdk/android/telemetry_redaction.md`；支持 Playbook §8。 |
| Torii RPC 上的重放或降级 | `/v1/pipeline` 请求构建器使用散列权限上下文强制执行 TLS 固定、噪声通道策略和重试预算。 | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java`； `docs/source/sdk/android/networking.md`（计划）。 |
|未签名或不可复制的版本 | CycloneDX SBOM + Sigstore 由 AND6 检查表门控的证明；发布 RFC 需要 `docs/source/release/provenance/android/` 中的证据。 | `docs/source/sdk/android/developer_experience_plan.md`； `docs/source/compliance/android/eu/sbom_attestation.md`。 |
|不完整的事件处理| Runbook + playbook 定义覆盖、混乱演练和升级树；遥测覆盖需要签名的 Norito 请求。 | `docs/source/android_runbook.md`； `docs/source/android_support_playbook.md`。 |

## 4. 评估活动

1. **设计审查** — 合规性 + SRE 验证配置、密钥管理、遥测和发布控制是否映射到 ETSI 安全目标。
2. **实施检查** — 自动化测试：
   - `scripts/android_strongbox_attestation_ci.sh` 验证矩阵中列出的每个 StrongBox 设备捕获的捆绑包。
   - `scripts/check_android_samples.sh` 和托管设备 CI 确保示例应用程序遵守 `ClientConfig`/遥测合同。
3. **操作验证** — 根据 `docs/source/sdk/android/telemetry_chaos_checklist.md` 每季度进行混乱演习（修订 + 覆盖练习）。
4. **证据保留** — 存储在 `docs/source/compliance/android/`（此文件夹）下并从 `status.md` 引用的文物。

## 5. ETSI EN 319 401 映射| EN 319 401 条款 | SDK控制|
|--------------------|-------------|
| 7.1 安全政策|记录在此安全目标 + 支持手册中。 |
| 7.2 组织安全|支持手册 §2 中的 RACI + 随叫随到所有权。 |
| 7.3 资产管理|上面第 §2 节中定义的配置、密钥和遥测资产目标。 |
| 7.4 访问控制| StrongBox 策略 + 覆盖需要签名 Norito 工件的工作流程。 |
| 7.5 加密控制 | AND2（密钥管理指南）的密钥生成、存储和证明要求。 |
| 7.6 操作安全 |遥测哈希、混乱排练、事件响应和发布证据门控。 |
| 7.7 通信安全| `/v1/pipeline` TLS 策略 + 哈希授权（遥测编辑文档）。 |
| 7.8 系统获取/开发| AND5/AND6 计划中可重现的 Gradle 构建、SBOM 和来源门。 |
| 7.9 供应商关系| Buildkite + Sigstore 证明与第三方依赖项 SBOM 一起记录。 |
| 7.10 事件管理| Runbook/Playbook 升级、覆盖日志记录、遥测失败计数器。 |

## 6. 维护

- 每当 SDK 引入新的加密算法、遥测类别或发布自动化更改时，请更新此文档。
- 将 `docs/source/compliance/android/evidence_log.csv` 中的签名副本与 SHA-256 摘要和审阅者签名链接起来。