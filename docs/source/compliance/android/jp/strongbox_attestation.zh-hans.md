---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2025-12-29T18:16:35.929201+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox 证明证据 — 日本部署

|领域|价值|
|--------|--------|
|评估窗口| 2026-02-10 – 2026-02-12 |
|文物地点 | `artifacts/android/attestation/<device-tag>/<date>/`（每个 `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` 的捆绑格式）|
|捕获工具| `scripts/android_keystore_attestation.sh`、`scripts/android_strongbox_attestation_ci.sh`、`scripts/android_strongbox_attestation_report.py` |
|审稿人|合规与法律硬件实验室主管（日本）|

## 1. 捕获过程

1. 在 StrongBox 矩阵中列出的每个设备上，生成质询并捕获证明包：
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. 将捆绑元数据（`result.json`、`chain.pem`、`challenge.hex`、`alias.txt`）提交到证据树。
3. 运行 CI 帮助程序以重新离线验证所有捆绑包：
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. 设备概要 (2026-02-12)

|设备标签 |模型/保险箱|捆绑路径 |结果 |笔记|
|------------|--------------------|-------------|--------|--------|
| `pixel6-strongbox-a` | Pixel 6 / 张量 G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ 通过（硬件支持）|挑战限制，操作系统补丁 2025 年 3 月 5 日。 |
| `pixel7-strongbox-a` | Pixel 7 / 张量 G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ 通过 |主要 CI 泳道候选者；温度在规格范围内|
| `pixel8pro-strongbox-a` | Pixel 8 Pro / 张量 G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ 通过（复试）|更换 USB-C 集线器； Buildkite `android-strongbox-attestation#221` 捕获了传递的包。 |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ 通过 | Knox 认证配置文件于 2026 年 2 月 9 日导入。 |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ 通过 |导入 Knox 认证配置文件； CI 车道现已绿色。 |

设备标签映射到 `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`。

## 3. 审稿人清单

- [x] 验证 `result.json` 显示 `strongbox_attestation: true` 和到受信任根的证书链。
- [x] 确认挑战字节匹配 Buildkite 运行 `android-strongbox-attestation#219`（初始扫描）和 `#221`（Pixel 8 Pro 重新测试 + S24 捕获）。
- [x] 硬件修复后重新运行 Pixel 8 Pro 捕获（所有者：硬件实验室负责人，于 2026 年 2 月 13 日完成）。
- [x] 一旦 Knox 配置文件批准到达，即可完成 Galaxy S24 捕获（所有者：Device Lab Ops，于 2026 年 2 月 13 日完成）。

## 4. 分配

- 将此摘要以及最新报告文本文件附加到合作伙伴合规性数据包中（FISC 清单§数据驻留）。
- 响应监管机构审核时参考捆绑路径；不要在加密通道之外传输原始证书。

## 5. 变更日志

|日期 |改变 |作者 |
|------|--------|--------|
| 2026-02-12 |初始 JP 捆绑捕获 + 报告。 |设备实验室运营|