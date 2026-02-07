---
lang: zh-hans
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 连接错误分类法（Swift 基线）

本说明跟踪 IOS-CONNECT-010 并记录了共享错误分类法
Nexus 连接 SDK。 Swift 现在导出规范的 `ConnectError` 包装器，
它将所有与连接相关的故障映射到六个类别之一，因此遥测，
仪表板和用户体验副本在各个平台上保持一致。

> 最后更新: 2026-01-15  
> 所有者：Swift SDK Lead（分类管理员）  
> 状态：Swift + Android + JavaScript 实现**落地**；队列整合待定。

## 类别

|类别 |目的|典型来源|
|----------|---------|-----------------|
| `transport` | WebSocket/HTTP 传输失败会终止会话。 | `URLError(.cannotConnectToHost)`，`ConnectClient.ClientError.closed` |
| `codec` |编码/解码帧时序列化/桥接失败。 | `ConnectEnvelopeError.invalidPayload`、`DecodingError` |
| `authorization` |需要用户或操作员修复的 TLS/证明/策略失败。 | `URLError(.secureConnectionFailed)`、Torii 4xx 响应 |
| `timeout` |空闲/离线过期和看门狗（队列 TTL、请求超时）。 | `URLError(.timedOut)`，`ConnectQueueError.expired` |
| `queueOverflow` | FIFO 背压信号使应用程序可以优雅地卸载负载。 | `ConnectQueueError.overflow(limit:)` |
| `internal` |其他一切：SDK 滥用、缺少 Norito 桥、损坏的日志。 | `ConnectSessionError.missingDecryptionKeys`、`ConnectCryptoError.*` |

每个 SDK 都会发布一个符合分类法的错误类型并公开
结构化遥测属性：`category`、`code`、`fatal` 和可选
元数据（`http_status`、`underlying`）。

## 快速映射

Swift 导出 `ConnectError`、`ConnectErrorCategory` 和辅助协议
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`。所有公共连接错误
类型符合 `ConnectErrorConvertible`，因此应用程序可以调用 `error.asConnectError()`
并将结果转发到遥测/记录层。|快速错误 |类别 |代码|笔记|
|------------|----------|------|--------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` |表示双`start()`；开发者错误。 |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` |关闭后发送/接收时引发。 |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket 传递文本有效负载，同时期望二进制。 |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | Counterparty 意外关闭了流。 |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` |应用程序忘记配置对称密钥。 |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito 有效负载缺少必填字段。 |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` |旧 SDK 看到的未来有效负载。 |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito 桥丢失或无法编码/解码帧字节。 |
| `ConnectCryptoError.*` | `internal` | `crypto.*` |桥接不可用或密钥长度不匹配。 |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` |离线队列长度超出配置的限制。 |
| `URLError(.timedOut)` | `timeout` | `network.timeout` |由 `URLSessionWebSocketTask` 浮现。 |
| `URLError` TLS 案例 | `authorization` | `network.tls_failure` | ATS/TLS 协商失败。 |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | SDK 中其他地方的 JSON 解码/编码失败；消息使用 Swift 解码器上下文。 |
|任何其他 `Error` | `internal` | `unknown_error` |保证包罗万象；消息镜像 `LocalizedError`。 |

单元测试 (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) 锁定
映射，以便将来的重构不能默默地更改类别或代码。

### 用法示例

```swift
do {
    try await session.nextEnvelope()
} catch {
    let connectError = error.asConnectError()
    telemetry.emit(event: "connect.error",
                   attributes: connectError.telemetryAttributes(fatal: true))
    logger.error("Connect failure: \(connectError.code) – \(connectError.message)")
}
```

## 遥测和仪表板

Swift SDK提供`ConnectError.telemetryAttributes(fatal:httpStatus:)`
它返回规范属性映射。出口商应转发这些
属性到 `connect.error` OTEL 事件中，并带有可选附加功能：

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

仪表板将 `connect.error` 计数器与队列深度相关联 (`connect.queue_depth`)
并重新连接直方图以检测回归，而无需探索日志。

## Android 映射Android SDK 导出 `ConnectError`、`ConnectErrorCategory`、`ConnectErrorTelemetryOptions`、
`ConnectErrorOptions`，以及下面的帮助实用程序
`org.hyperledger.iroha.android.connect.error`。构建器风格的帮助程序转换任何 `Throwable`
到符合分类法的有效负载中，从传输/TLS/编解码器异常中推断类别，
并公开确定性遥测属性，以便 OpenTelemetry/采样堆栈可以消耗
没有定制适配器的结果。【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.java:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
`ConnectQueueError` 已经实现了 `ConnectErrorConvertible`，发出队列溢出/超时
溢出/过期条件的类别，以便离线队列检测可以连接到同一流程。【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
Android SDK 自述文件现在引用了分类法并展示了如何包装传输异常
在发出遥测数据之前，保持 dApp 指导与 Swift 基线保持一致。【java/iroha_android/README.md:167】

## JavaScript 映射

Node.js/浏览器客户端导入 `ConnectError`、`ConnectQueueError`、`ConnectErrorCategory` 和
`connectErrorFrom()` 来自 `@iroha/iroha-js`。共享助手检查 HTTP 状态代码，
节点错误代码（套接字、TLS、超时）、`DOMException` 名称以及编解码器无法发出
本说明中记录的相同类别/代码，而 TypeScript 定义对遥测进行建模
属性覆盖，因此工具无需手动转换即可发出 OTEL 事件。【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
SDK 自述文件记录了工作流程并链接回该分类，以便应用程序团队可以
逐字复制仪器片段。【javascript/iroha_js/README.md:1387】

## 后续步骤（跨 SDK）

- **队列集成：**一旦离线队列发货，确保出队/丢弃逻辑
  表面 `ConnectQueueError` 值，因此溢出遥测仍然值得信赖。