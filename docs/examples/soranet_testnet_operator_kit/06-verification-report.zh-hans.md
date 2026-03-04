---
lang: zh-hans
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/06-verification-report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf489427d0eba2beebfdefc44092730c3963cbd77e83669853f4e9681ac9fd2d
source_last_modified: "2025-12-29T18:16:35.092552+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 操作员验证报告（T0 阶段）

- 操作员姓名：______________________
- 中继描述符 ID：______________________
- 提交日期（UTC）：___________________
- 联系电子邮件/矩阵：___________________

### 清单摘要

|项目 |已完成（是/否）|笔记|
|------|-----------------|--------|
|硬件和网络验证| | |
|应用合规块 | | |
|录取信封已验证 | | |
|防护轮旋转烟雾测试| | |
|遥测数据抓取和仪表板实时显示 | | |
|执行限电演习 | | |
| PoW 票证在目标范围内成功 | | |

### 指标快照

- PQ 比率 (`sorafs_orchestrator_pq_ratio`)：________
- 过去 24 小时降级次数：________
- 平均电路 RTT (p95): ________ ms
- PoW 中值解决时间：________ 毫秒

### 附件

请附上：

1. 中继支持捆绑散列 (`sha256`)： __________________________
2. 仪表板屏幕截图（PQ 比率、电路成功、PoW 直方图）。
3. 签名的钻取包（`drills-signed.json` + 签名者公钥十六进制和附件）。
4. SNNet-10 指标报告 (`cargo xtask soranet-testnet-metrics --input <snapshot> --out metrics-report.json`)。

### 操作员签名

我保证上述信息准确无误，并且所有必需的步骤均已完成
完成。

签名：_________________________ 日期：_________________