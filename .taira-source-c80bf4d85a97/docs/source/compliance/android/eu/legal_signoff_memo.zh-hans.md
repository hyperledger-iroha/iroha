---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2025-12-29T18:16:35.926037+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 欧盟法律签署备忘录模板

本备忘录记录了路线图项目 **AND6** 之前要求的法律审查
欧盟 (ETSI/GDPR) 工件数据包已提交给监管机构。律师应该克隆
每个版本的此模板，填充下面的字段，并存储签名的副本
以及备忘录中提到的不可改变的文物。

## 总结

- **发布/训练：** `<e.g., 2026.1 GA>`
- **审核日期：** `<YYYY-MM-DD>`
- **顾问/审阅者：** `<name + organisation>`
- **范围：** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **相关门票：** `<governance or legal issue IDs>`

## 工件清单

|文物 | SHA-256 |位置/链接|笔记|
|----------|---------|-----------------|--------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + 治理档案 |确认发布标识符和威胁模型调整。 |
| `gdpr_dpia_summary.md` | `<hash>` |相同目录/本地化镜像 |确保编辑策略引用与 `sdk/android/telemetry_redaction.md` 匹配。 |
| `sbom_attestation.md` | `<hash>` |证据桶中的同一目录+共同签名捆绑包|验证 CycloneDX + 来源签名。 |
|证据日志行| `<hash>` | `docs/source/compliance/android/evidence_log.csv` |行号 `<n>` |
|设备实验室应急包| `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` |确认与此版本相关的故障转移演练。 |

> 如果数据包包含更多文件（例如隐私文件），请附加附加行
> 附录或 DPIA 翻译）。每个人工制品都必须引用其不可变的
> 上传目标和生成它的 Buildkite 作业。

## 调查结果和例外情况

- `None.` *（替换为涵盖剩余风险的项目符号列表，补偿
  控制措施或所需的后续行动。）*

## 批准

- **决定：** `<Approved / Approved with conditions / Blocked>`
- **签名/时间戳：** `<digital signature or email reference>`
- **后续所有者：** `<team + due date for any conditions>`

将最终备忘录上传到治理证据桶，将 SHA-256 复制到
`docs/source/compliance/android/evidence_log.csv`，并链接上传路径
`status.md`。如果决策为“阻止”，则升级至 AND6 转向
委员会并记录路线图热门列表和
设备实验室应急日志。