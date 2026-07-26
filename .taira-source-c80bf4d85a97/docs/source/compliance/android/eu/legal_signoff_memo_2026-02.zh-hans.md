---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2025-12-29T18:16:35.926476+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 欧盟法律签署备忘录 — 2026.1 GA (Android SDK)

## 总结

- **发布/训练：** 2026.1 GA (Android SDK)
- **审核日期：** 2026-04-15
- **顾问/审阅者：** Sofia Martins — 合规与法律
- **范围：** ETSI EN 319 401 安全目标、GDPR DPIA 摘要、SBOM 证明、AND6 设备实验室应急证据
- **相关票证：** `_android-device-lab` / AND6-DR-202602，AND6 治理跟踪器 (`GOV-AND6-2026Q1`)

## 工件清单

|文物| SHA-256 |位置/链接|笔记|
|----------|---------|-----------------|--------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` |匹配 2026.1 GA 版本标识符和威胁模型增量（Torii NRPC 添加）。 |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` |参考 AND7 遥测政策 (`docs/source/sdk/android/telemetry_redaction.md`)。 |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore 捆绑包 (`android-sdk-release#4821`)。 | CycloneDX + 出处已审查；与 Buildkite 作业 `android-sdk-release#4821` 匹配。 |
|证据日志| `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv`（行 `android-device-lab-failover-20260220`）|确认日志捕获的捆绑散列+容量快照+备忘录条目。 |
|设备实验室应急包| `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` |取自 `bundle-manifest.json` 的哈希值；票 AND6-DR-202602 记录了向法律/合规部门的移交。 |

## 调查结果和例外情况

- 未发现阻塞问题。工件符合 ETSI/GDPR 要求； AND7 遥测奇偶校验已在 DPIA 摘要中注明，无需额外缓解措施。
- 建议：监控计划的 DR-2026-05-Q2 演练（票证 AND6-DR-202605），并在下一个治理检查点之前将生成的包附加到证据日志中。

## 批准

- **决定：** 批准
- **签名/时间戳：** _Sofia Martins（通过治理门户进行数字签名，2026-04-15 14:32 UTC）_
- **后续所有者：** 设备实验室运营（在 2026 年 5 月 31 日之前交付 DR-2026-05-Q2 证据包）