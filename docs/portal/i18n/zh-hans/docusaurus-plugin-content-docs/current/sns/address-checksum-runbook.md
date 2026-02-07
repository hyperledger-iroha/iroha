---
id: address-checksum-runbook
lang: zh-hans
direction: ltr
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Account Address Checksum Incident Runbook
sidebar_label: Checksum incidents
description: Operational response for IH58 (preferred) / compressed (`sora`, second-best) checksum failures (ADDR-7).
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
此页面镜像 `docs/source/sns/address_checksum_failure_runbook.md`。更新
首先源文件，然后同步此副本。
:::

校验和失败表现为 `ERR_CHECKSUM_MISMATCH` (`ChecksumMismatch`)
Torii、SDK 和钱包/浏览器客户端。现在的 ADDR-6/ADDR-7 路线图项目
每当出现校验和警报或支持时，要求操作员遵循此操作手册
票火了。

## 何时运行该剧

- **警报：** `AddressInvalidRatioSlo`（定义于
  `dashboards/alerts/address_ingest_rules.yml`) 行程及注释列表
  `reason="ERR_CHECKSUM_MISMATCH"`。
- **夹具漂移：** `account_address_fixture_status` Prometheus 文本文件或
  Grafana 仪表板报告任何 SDK 副本的校验和不匹配。
- **支持升级：** 钱包/浏览器/SDK 团队引用校验和错误、IME
  损坏，或剪贴板扫描不再解码。
- **手动观察：** Torii 日志显示重复的 `address_parse_error=checksum_mismatch`
  对于生产端点。

如果事件专门与 Local-8/Local-12 冲突有关，请遵循
`AddressLocal8Resurgence` 或 `AddressLocal12Collision` 剧本。

## 证据清单

|证据|命令/位置|笔记|
|----------|--------------------|--------|
| Grafana 快照 | `dashboards/grafana/address_ingest.json` |捕获无效原因故障和受影响的端点。 |
|警报有效负载 | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` |包括上下文标签和时间戳。 |
|夹具健康| `artifacts/account_fixture/address_fixture.prom` + Grafana |证明 SDK 副本是否偏离 `fixtures/account/address_vectors.json`。 |
| PromQL 查询 | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` |导出事件文档的 CSV。 |
|日志 | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'`（或日志聚合）|共享前清理 PII。 |
|夹具验证| `cargo xtask address-vectors --verify` |确认规范生成器和提交的 JSON 一致。 |
| SDK奇偶校验| `python3 scripts/account_fixture_helper.py check --target <path> --metrics-out artifacts/account_fixture/<label>.prom --metrics-label <label>` |针对警报/票证中报告的每个 SDK 运行。 |
|剪贴板/输入法理智 | `iroha tools address inspect <literal>` |检测隐藏字符或 IME 重写；引用 `address_display_guidelines.md`。 |

## 立即响应

1. 确认警报，链接事件中的 Grafana 快照 + PromQL 输出
   线程，并注意受影响的 Torii 上下文。
2. 冻结涉及地址解析的清单促销/SDK 发布。
3. 将仪表板快照和生成的 Prometheus 文本文件工件保存在
   事件文件夹 (`docs/source/sns/incidents/YYYY-MM/<ticket>/`)。
4. 提取显示 `checksum_mismatch` 有效负载的日志样本。
5. 通知 SDK 所有者 (`#sdk-parity`) 示例有效负载，以便他们进行分类。

## 根本原因隔离

### 夹具或发电机漂移

- 重新运行`cargo xtask address-vectors --verify`；如果失败则重新生成。
- 执行`ci/account_fixture_metrics.sh`（或个别
  `scripts/account_fixture_helper.py check`) 每个 SDK 确认捆绑
  固定装置与规范的 JSON 相匹配。

### 客户端编码器/IME 回归

- 通过 `iroha tools address inspect` 检查用户提供的文字以查找零宽度
  连接、假名转换或截断的有效负载。
- 交叉检查钱包/浏览器流量
  `docs/source/sns/address_display_guidelines.md`（双复制目标、警告、
  QR 助手）以确保他们遵循批准的用户体验。

### 清单或注册表问题

- 按照 `address_manifest_ops.md` 重新验证最新的清单包并
  确保没有 Local-8 选择器重新出现。
  出现在有效负载中。

### 恶意或格式错误的流量

- 通过 Torii 日志和 `torii_http_requests_total` 分解违规 IP/应用程序 ID。
- 保留至少 24 小时的日志以供安全/治理跟进。

## 缓解和恢复

|场景|行动|
|----------|---------|
|夹具漂移|重新生成 `fixtures/account/address_vectors.json`、重新运行 `cargo xtask address-vectors --verify`、更新 SDK 捆绑包并将 `address_fixture.prom` 快照附加到票证。 |
| SDK/客户端回归 |引用规范夹具 + `iroha tools address inspect` 输出的文件问题，以及 SDK 奇偶校验 CI 后面的门释放（例如，`ci/check_address_normalize.sh`）。 |
|恶意提交 |如果需要逻辑删除选择器，则对违规主体进行速率限制或阻止，升级至治理。 |

缓解措施落实后，重新运行上面的 PromQL 查询以确认
`ERR_CHECKSUM_MISMATCH` 保持为零（不包括 `/tests/*`）至少
事件降级前30分钟。

## 关闭

1. 存档 Grafana 快照、PromQL CSV、日志摘录和 `address_fixture.prom`。
2. 更新 `status.md`（ADDR 部分）以及路线图行（如果工具/文档）
   改变了。
3. 新课程时，将事件后记录归档在 `docs/source/sns/incidents/` 下
   出现。
4. 确保 SDK 发行说明提及适用的校验和修复。
5. 确认警报保持绿色 24 小时，并且灯具检查之前保持绿色
   解决。