---
lang: zh-hans
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 676798a4cf7c3e7737a0f80640f3f268a2f625f92afdd359ac528881d2aeb046
source_last_modified: "2025-12-29T18:16:35.060950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 文档自动化基线 (AND5)

路线图项目 AND5 需要文档、本地化和发布
在 AND6（CI 与合规性）开始之前，自动化可进行审核。这个文件夹
记录 AND5/AND6 引用的命令、工件和证据布局，
反映捕获的计划
`docs/source/sdk/android/developer_experience_plan.md` 和
`docs/source/sdk/android/parity_dashboard_plan.md`。

## 管道和命令

|任务|命令 |预期的文物|笔记|
|------|------------|--------------------|--------|
|本地化存根同步 | `python3 scripts/sync_docs_i18n.py`（每次运行可选择传递 `--lang <code>`）|存储在 `docs/automation/android/i18n/<timestamp>-sync.log` 下的日志文件以及翻译的存根提交 |使 `docs/i18n/manifest.json` 与翻译的存根保持同步；日志记录触及的语言代码以及基线中捕获的 git 提交。 |
| Norito 夹具+奇偶校验| `ci/check_android_fixtures.sh`（包裹 `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`）|将生成的摘要 JSON 复制到 `docs/automation/android/parity/<stamp>-summary.json` |验证 `java/iroha_android/src/test/resources` 有效负载、清单哈希和签名的固定长度。将摘要附在 `artifacts/android/fixture_runs/` 下的节奏证据旁边。 |
|清单示例和发布证明 | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]`（运行测试 + SBOM + 来源）|出处捆绑元数据加上存储在 `docs/automation/android/samples/<version>/` 下的 `docs/source/sdk/android/samples/` 生成的 `sample_manifest.json` |将 AND5 示例应用程序和发布自动化联系在一起 - 捕获生成的清单、SBOM 哈希和出处日志以进行 Beta 审核。 |
| Parity 仪表板提要 | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` 后跟 `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` |将 `metrics.prom` 快照或 Grafana 导出 JSON 复制到 `docs/automation/android/parity/<stamp>-metrics.prom` |提供仪表板计划，以便 AND5/AND7 治理可以验证无效的提交计数器和遥测采用情况。 |

## 证据捕获

1. **为所有内容添加时间戳。** 使用 UTC 时间戳命名文件
   (`YYYYMMDDTHHMMSSZ`) 因此奇偶校验仪表板、治理会议记录并发布
   文档可以确定性地引用相同的运行。
2. **参考提交。** 每个日志应包含运行的 git commit 哈希值
   加上任何相关配置（例如，`ANDROID_PARITY_PIPELINE_METADATA`）。
   当隐私需要编辑时，请添加注释并链接到安全保管库。
3. **存档最小上下文。** 我们只检查结构化摘要（JSON、
   `.prom`、`.log`）。重要的文物（APK 包、屏幕截图）应保留在
   `artifacts/` 或在日志中记录有签名哈希的对象存储。
4. **更新状态条目。** 当 AND5 里程碑在 `status.md` 中取得进展时，引用
   相应的文件（例如，`docs/automation/android/parity/20260324T010203Z-summary.json`）
   因此审计人员可以跟踪基线而无需抓取 CI 日志。

遵循此布局满足“可用于的文档/自动化基线”
AND6 引用并保存 Android 文档程序的“审核”先决条件
与已公布的计划保持同步。