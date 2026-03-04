---
lang: zh-hans
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d04af2ad3ae5cc6a9254236f5627850aab7c6517308e9f3a09650cbc1490168
source_last_modified: "2026-01-05T18:22:23.401292+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 连接会话预览运行手册 (IOS7 / JS4)

本操作手册记录了暂存、验证和部署的端到端过程
根据路线图里程碑的要求拆除 Connect 预览会话 **IOS7**
和 **JS4**（`roadmap.md:1340`、`roadmap.md:1656`）。每当
您演示了 Connect Strawman (`docs/source/connect_architecture_strawman.md`)，
执行 SDK 路线图中承诺的队列/遥测挂钩，或收集
`status.md` 的证据。

## 1. 飞行前检查表

|项目 |详情 |参考文献 |
|------|---------|------------|
| Torii 端点 + 连接策略 |确认 Torii 基本 URL、`chain_id` 和连接策略 (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`)。捕获 Runbook 票证中的 JSON 快照。 | `javascript/iroha_js/src/toriiClient.js`、`docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
|夹具+桥版本|请注意您将使用的 Norito 夹具哈希和桥接构建（Swift 需要 `NoritoBridge.xcframework`，JS 需要 `@iroha/iroha-js` ≥ 附带 `bootstrapConnectPreviewSession` 的版本）。 | `docs/source/sdk/swift/reproducibility_checklist.md`，`javascript/iroha_js/CHANGELOG.md` |
|遥测仪表板 |确保绘制 `connect.queue_depth`、`connect.queue_overflow_total`、`connect.resume_latency_ms`、`swift.connect.session_event` 等的仪表板可访问（Grafana `Android/Swift Connect` 板 + 导出的 Prometheus 快照）。 | `docs/source/connect_architecture_strawman.md`、`docs/source/sdk/swift/telemetry_redaction.md`、`docs/source/sdk/js/quickstart.md` |
|证据文件夹|选择一个目的地，例如 `docs/source/status/swift_weekly_digest.md`（每周摘要）和 `docs/source/sdk/swift/connect_risk_tracker.md`（风险跟踪器）。将日志、指标屏幕截图和确认存储在 `docs/source/sdk/swift/readiness/archive/<date>/connect/` 下。 | `docs/source/status/swift_weekly_digest.md`，`docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. 引导预览会话

1. **验证政策+配额。** 致电：
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   如果 `queue_max` 或 TTL 与您计划的配置不同，则运行失败
   测试。
2. **生成确定性 SID/URI。** `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` 帮助程序将 SID/URI 生成与 Torii 联系起来
   会议注册；即使 Swift 将驱动 WebSocket 层也可以使用它。
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - 将 `register: false` 设置为空运行 QR/深层链接场景。
   - 将返回的 `sidBase64Url`、深层链接 URL 和 `tokens` blob 保留在
     证据文件夹；治理审查预计会出现这些人工制品。
3. **分发秘密。** 与钱包运营商共享深层链接 URI
   （swift dApp 示例、Android 钱包或 QA 工具）。切勿粘贴原始令牌
   进入聊天；使用启用数据包中记录的加密保管库。

## 3. 推动会议1. **打开 WebSocket。** Swift 客户端通常使用：
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   参考 `docs/connect_swift_integration.md` 用于其他设置（桥
   导入、并发适配器）。
2. **批准 + 签署流程。** DApp 调用 `ConnectSession.requestSignature(...)`，
   而钱包则通过 `approveSession` / `reject` 进行响应。每次批准都必须记录
   哈希别名 + 权限以匹配 Connect 治理章程。
3. **练习队列 + 恢复路径。** 切换网络连接或暂停
   钱包确保有界队列和重放挂钩日志条目。 JS/安卓
   SDK 发出 `ConnectQueueError.overflow(limit)` /
   `.expired(ttlMs)` 当它们丢帧时；斯威夫特应该观察同样的一次
   IOS7 队列脚手架登陆 (`docs/source/connect_architecture_strawman.md`)。
   记录至少一次重新连接后，运行
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   （或传递 `ConnectSessionDiagnostics` 返回的导出目录）和
   将渲染的表/JSON 附加到 Runbook 票证。 CLI 读取相同
   `ConnectQueueStateTracker`产生的`state.json` / `metrics.ndjson`对，
   因此，治理审核人员无需定制工具即可追踪演练证据。

## 4. 遥测和可观测性

- **要捕获的指标：**
  - `connect.queue_depth{direction}` 表（应保持在保单上限以下）。
  - `connect.queue_dropped_total{reason="overflow|ttl"}` 计数器（仅非零
    在故障注入期间）。
  - `connect.resume_latency_ms`直方图（强制后记录p95
    重新连接）。
  - `connect.replay_success_total` / `connect.replay_error_total`。
  - Swift 专用 `swift.connect.session_event` 和
    `swift.connect.frame_latency` 导出 (`docs/source/sdk/swift/telemetry_redaction.md`)。
- **仪表板：** 使用注释标记更新连接板书签。
  将屏幕截图（或 JSON 导出）与原始数据一起附加到证据文件夹中
  通过遥测导出器 CLI 拉取的 OTLP/Prometheus 快照。
- **警报：** 如果触发任何 Sev1/2 阈值（根据 `docs/source/android_support_playbook.md` §5），
  寻呼 SDK 项目负责人并在运行手册中记录 PagerDuty 事件 ID
  继续之前先买票。

## 5. 清理和回滚

1. **删除暂存会话。** 始终删除预览会话，以便队列深度
   警报仍然有意义：
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   对于仅 Swift 的测试运行，请通过 Rust/CLI 帮助程序调用相同的端点。
2. **清除日志。** 删除任何保留的队列日志
   （`ApplicationSupport/ConnectQueue/<sid>.to`，IndexedDB 存储等）所以
   下次运行开始干净。如果需要，请在删除前记录文件哈希
   调试重播问题。
3. **归档事件记录。** 总结运行情况：
   - `docs/source/status/swift_weekly_digest.md`（增量块），
   - `docs/source/sdk/swift/connect_risk_tracker.md`（清除或降级CR-2
     一旦遥测到位），
   - JS SDK 变更日志或配方（如果新行为已验证）。
4. **升级故障：**
   - 没有注入错误的队列溢出 ⇒ 针对 SDK 提交错误
     政策与 Torii 不同。
   - 恢复错误⇒附加 `connect.queue_depth` + `connect.resume_latency_ms`
     事件报告的快照。
   - 治理不匹配（代币重复使用、超出 TTL）⇒ 通过 SDK 筹集资金
     在下一次修订期间负责程序并注释 `roadmap.md`。

## 6. 证据清单|文物 |地点 |
|----------|----------|
| SID/深层链接/令牌 JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
|仪表板导出（`connect.queue_depth`等）| `.../metrics/` 子文件夹 |
| PagerDuty / 事件 ID | `.../notes.md` |
|清理确认（Torii 删除，日志擦除）| `.../cleanup.log` |

完成此清单即可满足“文档/运行手册已更新”退出标准
适用于 IOS7/JS4，并为治理审查者提供每个问题的确定性线索
连接预览会话。