---
lang: zh-hans
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-05T09:28:11.999717+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 发布清单 (AND6)

此清单捕获了 **AND6 — CI 和合规性强化** 门
`roadmap.md`（§优先级 5）。它将 Android SDK 版本与 Rust 保持一致
通过阐明 CI 作业、合规工件来发布 RFC 期望，
设备实验室证据以及在 GA 之前必须附加的出处包，
LTS（即修补程序）列车向前推进。

将本文档与以下内容一起使用：

- `docs/source/android_support_playbook.md` — 发布日历、SLA 和
  升级树。
- `docs/source/android_runbook.md` — 日常操作手册。
- `docs/source/compliance/android/and6_compliance_checklist.md` — 调节器
  文物库存。
- `docs/source/release_dual_track_runbook.md` — 双轨发布治理。

## 1. 舞台大门一览

|舞台|所需的大门|证据|
|--------|----------------|----------|
| **T−7 天（冻结前）** |每晚 `ci/run_android_tests.sh` 绿色，持续 14 天； `ci/check_android_fixtures.sh`、`ci/check_android_samples.sh` 和 `ci/check_android_docs_i18n.sh` 通过； lint/依赖项扫描已排队。 | Buildkite 仪表板、夹具差异报告、示例屏幕截图。 |
| **T−3 天（RC 促销）** |设备实验室预订已确认； StrongBox 认证 CI 运行 (`scripts/android_strongbox_attestation_ci.sh`)；在预定硬件上运行的机器人电动/仪表套件； `./gradlew lintRelease ktlintCheck detekt dependencyGuard` 干净。 |设备矩阵 CSV、证明捆绑清单、Gradle 报告存档在 `artifacts/android/lint/<version>/` 下。 |
| **T−1 天（去/不去）** |遥测编辑状态包已刷新 (`scripts/telemetry/check_redaction_status.py --write-cache`)；根据 `and6_compliance_checklist.md` 更新合规性工件；来源排练已完成 (`scripts/android_sbom_provenance.sh --dry-run`)。 | `docs/source/compliance/android/evidence_log.csv`，遥测状态 JSON，来源试运行日志。 |
| **T0（GA/LTS 切换）** | `scripts/publish_android_sdk.sh --dry-run` 已完成；出处+SBOM 签名；导出并附加到通过/不通过分钟的发布清单； `ci/sdk_sorafs_orchestrator.sh` 冒烟工作绿色。 |发布 RFC 附件、Sigstore 捆绑包、`artifacts/android/` 下的采用工件。 |
| **T+1 天（切换后）** |已验证修补程序准备情况 (`scripts/publish_android_sdk.sh --validate-bundle`)；审查了仪表板差异（`ci/check_android_dashboard_parity.sh`）；证据包已上传至`status.md`。 |仪表板差异导出，链接到 `status.md` 条目，存档的发布数据包。 |

## 2. CI 和质量门矩阵|门 |命令/脚本 |笔记|
|------|--------------------|--------|
|单元+集成测试| `ci/run_android_tests.sh`（包裹 `ci/run_android_tests.sh`）|发出 `artifacts/android/tests/test-summary.json` + 测试日志。包括 Norito 编解码器、队列、StrongBox 回退和 Torii 客户端利用测试。每晚和标记之前需要。 |
|灯具奇偶校验| `ci/check_android_fixtures.sh`（包裹 `scripts/check_android_fixtures.py`）|确保重新生成的 Norito 装置与 Rust 规范集相匹配；当门失败时附加 JSON diff。 |
|示例应用程序 | `ci/check_android_samples.sh` |构建 `examples/android/{operator-console,retail-wallet}` 并通过 `scripts/android_sample_localization.py` 验证本地化屏幕截图。 |
|文档/I18N | `ci/check_android_docs_i18n.sh` | Guards README + 本地化快速入门。在 doc 编辑到发布分支后再次运行。 |
|仪表板奇偶校验 | `ci/check_android_dashboard_parity.sh` |确认 CI/导出的指标与 Rust 的对应指标一致； T+1验证时需要。 |
| SDK采用烟雾| `ci/sdk_sorafs_orchestrator.sh` |使用当前 SDK 练习多源 Sorafs Orchestrator 绑定。上传舞台制品之前需要。 |
|鉴证验证 | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` |聚合 `artifacts/android/attestation/**` 下的 StrongBox/TEE 证明包；将摘要附加到 GA 数据包中。 |
|设备实验室插槽验证 | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` |在将证据附加到发布包之前验证仪器包； CI 针对 `fixtures/android/device_lab/slot-sample` 中的示例槽运行（遥测/证明/队列/日志 + `sha256sum.txt`）。 |

> **提示：** 将这些作业添加到 `android-release` Buildkite 管道中，以便
> 冻结周会使用释放分支提示自动重新运行每个门。

整合的 `.github/workflows/android-and6.yml` 作业运行 lint，
每个 PR/推送的测试套件、证明摘要和设备实验室插槽检查
触及 Android 源，在 `artifacts/android/{lint,tests,attestation,device_lab}/` 下上传证据。

## 3. Lint 和依赖项扫描

从存储库根目录运行 `scripts/android_lint_checks.sh --version <semver>`。的
脚本执行：

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- 报告和依赖性保护输出存档于
  用于发布的 `artifacts/android/lint/<label>/` 和 `latest/` 符号链接
  管道。
- 失败的 lint 发现需要修复或在版本中记录
  RFC 记录可接受的风险（由发布工程 + 计划批准）
  铅）。
- `dependencyGuardBaseline` 重新生成依赖锁；附上差异
  到通过/不通过数据包。

## 4. 设备实验室和 StrongBox 覆盖范围

1. 使用中引用的容量跟踪器保留 Pixel + Galaxy 设备
   `docs/source/compliance/android/device_lab_contingency.md`。阻止发布
   如果可用性 ` 刷新证明报告。
3. 运行仪器矩阵（记录设备中的套件/ABI 列表）
   跟踪器）。即使重试成功，也会在事件日志中捕获失败。
4. 如果需要回退到 Firebase 测试实验室，请提交票证；链接票证
   在下面的清单中。

## 5. 合规性和遥测工件- 欧盟遵循 `docs/source/compliance/android/and6_compliance_checklist.md`
  和 JP 提交的材料。更新 `docs/source/compliance/android/evidence_log.csv`
  带有哈希值 + Buildkite 作业 URL。
- 通过以下方式刷新遥测编辑证据
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`。
  将生成的 JSON 存储在
  `artifacts/android/telemetry/<version>/status.json`。
- 记录架构差异输出
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  证明与 Rust 出口商的平等。

## 6. 出处、SBOM 和出版

1. 试运行发布管道：

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. 生成 SBOM + Sigstore 出处：

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3.附上`artifacts/android/provenance/<semver>/manifest.json`并签名
   `checksums.sha256` 到发布 RFC。
4.升级到真正的Maven仓库时，重新运行
   `scripts/publish_android_sdk.sh` 没有 `--dry-run`，捕获控制台
   记录，并将生成的工件上传到 `artifacts/android/maven/<semver>`。

## 7. 提交包模板

每个 GA/LTS/修补程序版本应包括：

1. **已完成的清单** — 复制此文件的表格，勾选每个项目，然后链接
   支持工件（Buildkite 运行、日志、文档差异）。
2. **设备实验室证据** — 认证报告摘要、预订日志和
   任何应急激活。
3. **遥测数据包** — 编辑状态 JSON、架构差异、链接
   `docs/source/sdk/android/telemetry_redaction.md` 更新（如果有）。
4. **合规性工件** — 在合规性文件夹中添加/更新的条目
   加上刷新的证据日志 CSV。
5. **出处捆绑** — SBOM、Sigstore 签名和 `checksums.sha256`。
6. **发布摘要** — `status.md` 总结中附有一页概述
   以上（日期、版本、任何被放弃的门的亮点）。

将数据包存储在 `artifacts/android/releases/<version>/` 下并引用它
在 `status.md` 和发布 RFC 中。

- 自动`scripts/run_release_pipeline.py --publish-android-sdk ...`
  复制最新的 lint 存档 (`artifacts/android/lint/latest`) 和
  合规证据登录 `artifacts/android/releases/<version>/`，以便
  提交数据包始终具有规范位置。

---

**提醒：**每当有新的 CI 作业、合规工件、
或添加遥测要求。路线图项目 AND6 保持开放状态，直到
检查表和相关自动化在连续两个版本中证明是稳定的
火车。