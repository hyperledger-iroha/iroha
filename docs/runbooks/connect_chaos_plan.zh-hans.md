---
lang: zh-hans
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-12-29T18:16:35.913527+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 连接混乱和故障排练计划 (IOS3 / IOS7)

本剧本定义了满足 IOS3/IOS7 的可重复混沌练习
路线图行动_“计划联合混乱演练”_ (`roadmap.md:1527`)。与它配对
Connect 预览运行手册 (`docs/runbooks/connect_session_preview_runbook.md`)
进行跨 SDK 演示时。

## 目标和成功标准
- 执行共享的连接重试/退避策略、脱机队列限制以及
  遥测出口商在受控故障下，无需改变生产代码。
- 捕获确定性伪影（`iroha connect queue inspect` 输出，
  `connect.*` 指标快照、Swift/Android/JS SDK 日志），以便治理可以
  审核每一次演习。
- 证明钱包和 dApp 尊重配置更改（清单漂移、salt
  通过显示规范的 `ConnectError` 来旋转、证明失败）
  类别和编辑安全的遥测事件。

## 先决条件
1. **环境引导**
   - 启动演示 Torii 堆栈：`scripts/ios_demo/start.sh --telemetry-profile full`。
   - 启动至少一个 SDK 示例 (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`、Android `demo-connect`、JS `examples/connect`）。
2. **仪器仪表**
   - 启用 SDK 诊断（`ConnectQueueDiagnostics`、`ConnectQueueStateTracker`、
     Swift 中的 `ConnectSessionDiagnostics`； `ConnectQueueJournal` + `ConnectQueueJournalTests`
     Android/JS 中的等效项）。
   - 确保 CLI `iroha connect queue inspect --sid <sid> --metrics` 解析
     SDK 生成的队列路径（`~/.iroha/connect/<sid>/state.json` 和
     `metrics.ndjson`）。
   - 有线遥测导出器，因此以下时间序列在
     Grafana 和通过 `scripts/swift_status_export.py telemetry`: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`，`swift.connect.frame_latency`，
     `android.telemetry.redaction.salt_version`。
3. **证据文件夹** – 创建 `artifacts/connect-chaos/<date>/` 并存储：
   - 原始日志 (`*.log`)、指标快照 (`*.json`)、仪表板导出
     (`*.png`)、CLI 输出和 PagerDuty ID。

## 场景矩阵|身份证 |故障|注射步骤|预期信号 |证据|
|----|--------|-----------------|--------------------|----------|
| C1 | WebSocket 中断和重新连接 |将 `/v2/connect/ws` 包裹在代理后面（例如 `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`）或暂时阻止服务（`kubectl scale deploy/torii --replicas=0` ≤ 60 秒）。强制钱包继续发送帧，以便填满离线队列。 | `connect.reconnects_total` 递增，`connect.resume_latency_ms` 尖峰但保持 - 中断窗口的仪表板注释。- 包含重新连接 + 耗尽消息的示例日志摘录。 |
| C2 |离线队列溢出/TTL过期|修补示例以缩小队列限制（Swift：在 `ConnectSessionDiagnostics` 内实例化 `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`；Android/JS 使用相应的构造函数）。当 dApp 继续排队请求时，将钱包暂停 ≥2× `retentionInterval`。 | `connect.queue_dropped_total{reason="overflow"}` 和 `{reason="ttl"}` 增量，`connect.queue_depth` 在新限制处稳定，SDK 表面 `ConnectError.QueueOverflow(limit: 4)`（或 `.QueueExpired`）。 `iroha connect queue inspect` 显示 `state=Overflow`，其中 `warn/drop` 水印为 100%。 | - 指标计数器的屏幕截图。- CLI JSON 输出捕获溢出。- 包含 `ConnectError` 行的 Swift/Android 日志片段。 |
| C3 |明显漂移/录取拒绝|篡改提供给钱包的 Connect 清单（例如，修改 `docs/connect_swift_ios.md` 示例清单，或启动 Torii，其中 `--connect-manifest-path` 指向 `chain_id` 或 `permissions` 不同的副本）。让 dApp 请求批准并确保钱包通过策略拒绝。 | Torii 使用 `manifest_mismatch` 返回 `/v2/connect/session` 的 `HTTP 409`，SDK 发出 `ConnectError.Authorization.manifestMismatch(manifestVersion)`，遥测引发 `connect.manifest_mismatch_total`，并且队列保持为空 (`state=Idle`)。 | - Torii 日志摘录显示不匹配检测。- 出现错误的 SDK 屏幕截图。- 指标快照证明测试期间没有排队的帧。 |
| C4|按键旋转/盐版凹凸|在会话中轮换 Connect salt 或 AEAD 密钥。在开发堆栈中，使用 `CONNECT_SALT_VERSION=$((old+1))` 重新启动 Torii（镜像 `docs/source/sdk/android/telemetry_schema_diff.md` 中的 Android 编辑盐测试）。保持钱包离线，直到盐轮换完成，然后恢复。 |第一次恢复尝试失败，显示 `ConnectError.Authorization.invalidSalt`，队列刷新（dApp 因 `salt_version_mismatch` 原因丢弃缓存帧），遥测发出 `android.telemetry.redaction.salt_version` (Android) 和 `swift.connect.session_event{event="salt_rotation"}`。 SID 刷新成功后的第二个会话。 | - 带有盐纪元之前/之后的仪表板注释。- 包含无效盐错误和后续成功的日志。- `iroha connect queue inspect` 输出显示 `state=Stalled` 后跟新鲜的 `state=Active`。 || C5|证明/StrongBox 失败 |在 Android 钱包上，配置 `ConnectApproval` 以包含 `attachments[]` + StrongBox 证明。使用证明工具（`scripts/android_keystore_attestation.sh` 和 `--inject-failure strongbox-simulated`）或在提交给 dApp 之前篡改证明 JSON。 | DApp 以 `ConnectError.Authorization.invalidAttestation` 拒绝批准，Torii 记录失败原因，导出器碰撞 `connect.attestation_failed_total`，队列清除违规条目。 Swift/JS dApp 在保持会话活动的同时记录错误。 | - 带有注入故障 ID 的线束日志。- SDK 错误日志 + 遥测计数器捕获。- 队列删除了坏帧的证据 (`recordsRemoved > 0`)。 |

## 场景细节

### C1 — WebSocket 中断和重新连接
1. 将 Torii 包装在代理（toxiproxy、Envoy 或 `kubectl port-forward`）后面，以便
   您可以切换可用性而无需杀死整个节点。
2. 触发45秒中断：
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. 观察遥测仪表板和 `scripts/swift_status_export.py telemetry
   --json-out artifacts/connect-chaos//c1_metrics.json`。
4. 中断后立即转储队列状态：
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. 成功 = 单次重新连接尝试、有界队列增长和自动
   代理恢复后耗尽。

### C2 — 离线队列溢出/TTL 过期
1. 缩小本地构建中的队列阈值：
   - Swift：更新示例中的 `ConnectQueueJournal` 初始化程序
     （例如，`examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`）
     通过 `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`。
   - Android/JS：构造时传递等效的配置对象
     `ConnectQueueJournal`。
2. 暂停钱包（模拟器后台或设备飞行模式）≥60s
   而 dApp 发出 `ConnectClient.requestSignature(...)` 调用。
3. 使用 `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) 或 JS
   用于导出证据包的诊断助手（`state.json`、`journal/*.to`、
   `metrics.ndjson`）。
4. 成功 = 溢出计数器递增，SDK 表面 `ConnectError.QueueOverflow`
   一次，钱包恢复后队列恢复。

### C3 — 明显漂移/录取拒绝
1. 复印一份入场清单，例如：
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. 使用 `--connect-manifest-path /tmp/manifest_drift.json` 启动 Torii（或
   更新演习的 docker compose/k8s 配置）。
3. 尝试从钱包启动会话；预计 HTTP 409。
4. 捕获 Torii + SDK 日志以及 `connect.manifest_mismatch_total`
   遥测仪表板。
5. 成功=拒绝且队列不增长，并且钱包显示共享
   分类错误 (`ConnectError.Authorization.manifestMismatch`)。### C4 — 密钥旋转/盐凸块
1. 通过遥测记录当前的 salt 版本：
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. 使用新盐重新启动 Torii（`CONNECT_SALT_VERSION=$((OLD+1))` 或更新
   配置图）。保持钱包离线，直到重启完成。
3. 恢复钱包；第一个恢复应该失败并出现无效盐错误
   和 `connect.queue_dropped_total{reason="salt_version_mismatch"}` 增量。
4. 通过删除会话目录强制应用程序删除缓存的帧
   （`rm -rf ~/.iroha/connect/<sid>`或平台特定缓存清除），然后
   使用新令牌重新启动会话。
5. 成功 = 遥测显示盐块，记录无效的恢复事件
   一次，下一次会话无需人工干预即可成功。

### C5 — 证明/StrongBox 失败
1. 使用 `scripts/android_keystore_attestation.sh` 生成证明包
   （设置 `--inject-failure strongbox-simulated` 以翻转签名位）。
2. 让钱包通过其 `ConnectApproval` API 附加此捆绑包； dApp
   应验证并拒绝有效负载。
3.验证遥测（`connect.attestation_failed_total`，Swift/Android事件
   指标）并确保队列丢弃中毒的条目。
4. 成功 = 拒绝与不良批准隔离，队列保持健康，
   证明日志与演练证据一起存储。

## 证据清单
- `artifacts/connect-chaos/<date>/c*_metrics.json` 出口自
  `scripts/swift_status_export.py telemetry`。
- 来自 `iroha connect queue inspect` 的 CLI 输出 (`c*_queue.txt`)。
- SDK + Torii 日志带有时间戳和 SID 哈希值。
- 带有每个场景注释的仪表板屏幕截图。
- PagerDuty / 事件 ID（如果 Sev1/2 警报触发）。

每季度完成一次完整矩阵即可满足路线图门和
显示 Swift/Android/JS Connect 实现做出确定性响应
跨越最高风险的故障模式。