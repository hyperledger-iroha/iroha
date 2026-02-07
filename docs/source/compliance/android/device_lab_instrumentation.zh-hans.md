---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2025-12-29T18:16:35.924058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android 设备实验室仪器挂钩 (AND6)

该参考文献结束了路线图行动“暂存剩余的设备实验室/
AND6 启动前仪表挂钩”。它解释了如何每个保留
device-lab 插槽必须捕获遥测、队列和证明工件，以便
AND6 合规性检查表、证据日志和治理包共享相同的内容
确定性工作流程。将此注释与预订程序配对
(`device_lab_reservation.md`) 和规划排练时的故障转移操作手册。

## 目标和范围

- **确定性证据** – 所有仪器输出均位于
  `artifacts/android/device_lab/<slot-id>/` 具有 SHA-256 清单，因此审核员
  可以在不重新运行探针的情况下比较包。
- **脚本优先工作流程** – 重用现有的助手
  （`ci/run_android_telemetry_chaos_prep.sh`，
  `scripts/android_keystore_attestation.sh`、`scripts/android_override_tool.sh`)
  而不是定制的 adb 命令。
- **检查表保持同步** – 每次运行都会引用此文档
  AND6 合规性检查表并将工件附加到
  `docs/source/compliance/android/evidence_log.csv`。

## 工件布局

1. 选择与预订票相匹配的唯一插槽标识符，例如
   `2026-05-12-slot-a`。
2. 播种标准目录：

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. 将每个命令日志保存在匹配的文件夹中（例如
   `telemetry/status.ndjson`、`attestation/pixel8pro.log`）。
4. 槽关闭后捕获 SHA-256 清单：

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## 仪表矩阵

|流量|命令 |输出位置 |笔记|
|------|------------|-----------------|--------|
|遥测编辑 + 状态包 | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`、`telemetry/status.log` |在槽的开始和结束处运行；将 CLI 标准输出附加到 `status.log`。 |
|待处理队列+混乱准备| `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`、`queue/*.json`、`queue/*.sha256` |镜像来自 `readiness/labs/telemetry_lab_01.md` 的 ScenarioD；扩展插槽中每个设备的环境变量。 |
|覆盖分类账摘要 | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` |即使没有活动覆盖也需要；证明零状态。 |
| StrongBox/TEE认证| `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` |对每个保留设备重复此操作（匹配 `android_strongbox_device_matrix.md` 中的名称）。 |
| CI 利用证明回归 | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` |捕获 CI 上传的相同证据；包括在手动运行中以实现对称。 |
| Lint / 依赖基线 | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`、`logs/lint.log` |每个冻结窗口运行一次；引用合规包中的摘要。 |

## 标准槽程序1. **飞行前（T-24h）** – 确认预订机票引用此
   文档，更新设备矩阵条目，并播种工件根。
2. **时段期间**
   - 首先运行遥测捆绑+队列导出命令。通行证
     `--note <ticket>` 到 `ci/run_android_telemetry_chaos_prep.sh` 所以日志
     引用事件 ID。
   - 触发每个设备的证明脚本。当线束产生
     `.zip`，将其复制到 artefact 根目录并记录打印的 Git SHA
     脚本的结尾。
   - 使用覆盖的摘要路径执行 `make android-lint`，即使 CI
     已经跑了；审计员期望每个槽的日志。
3. **运行后**
   - 在插槽内生成 `sha256sum.txt` 和 `README.md`（自由格式注释）
     总结已执行命令的文件夹。
   - 将一行添加到 `docs/source/compliance/android/evidence_log.csv`
     插槽 ID、哈希清单路径、Buildkite 引用（如果有）以及最新版本
     预留日历导出中的设备实验室容量百分比。
   - 链接 `_android-device-lab` 票证中的插槽文件夹，AND6
     清单和 `docs/source/android_support_playbook.md` 发布报告。

## 故障处理和升级

- 如果任何命令失败，请捕获 `logs/` 下的 stderr 输出并按照
  `device_lab_reservation.md` §6 中的升级阶梯。
- 队列或遥测不足应立即记录覆盖状态
  `docs/source/sdk/android/telemetry_override_log.md` 并引用插槽 ID
  这样治理就可以追踪演练。
- 证明回归必须记录在
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  以及上面记录的失败设备序列号和捆绑包路径。

## 报告清单

在将插槽标记为完成之前，请验证以下参考是否已更新：

- `docs/source/compliance/android/and6_compliance_checklist.md` — 标记
  完成仪表行并记下插槽 ID。
- `docs/source/compliance/android/evidence_log.csv` — 添加/更新条目
  插槽哈希和容量读取。
- `_android-device-lab` 票证 — 附加工件链接和 Buildkite 作业 ID。
- `status.md` — 在下一个 Android 准备摘要中包含一个简短的注释，以便
  路线图读者知道哪个插槽产生了最新证据。

遵循此流程可保留 AND6 的“设备实验室 + 仪器挂钩”
里程碑可审核，并防止预订、执行、
和报告。