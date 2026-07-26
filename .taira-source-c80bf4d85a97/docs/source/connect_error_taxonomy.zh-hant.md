---
lang: zh-hant
direction: ltr
source: docs/source/connect_error_taxonomy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bd6ab18725af866ffd2589a3fa7bd7130bab1d5efef2920408baa8e338d5d068
source_last_modified: "2025-12-29T18:16:35.935471+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 連接錯誤分類法（Swift 基線）

本說明跟踪 IOS-CONNECT-010 並記錄了共享錯誤分類法
Nexus 連接 SDK。 Swift 現在導出規範的 `ConnectError` 包裝器，
它將所有與連接相關的故障映射到六個類別之一，因此遙測，
儀表板和用戶體驗副本在各個平台上保持一致。

> 最後更新: 2026-01-15  
> 所有者：Swift SDK Lead（分類管理員）  
> 狀態：Swift + Android + JavaScript 實現**落地**；隊列整合待定。

## 類別

|類別 |目的|典型來源|
|----------|---------|-----------------|
| `transport` | WebSocket/HTTP 傳輸失敗會終止會話。 | `URLError(.cannotConnectToHost)`，`ConnectClient.ClientError.closed` |
| `codec` |編碼/解碼幀時序列化/橋接失敗。 | `ConnectEnvelopeError.invalidPayload`、`DecodingError` |
| `authorization` |需要用戶或操作員修復的 TLS/證明/策略失敗。 | `URLError(.secureConnectionFailed)`、Torii 4xx 響應 |
| `timeout` |空閒/離線過期和看門狗（隊列 TTL、請求超時）。 | `URLError(.timedOut)`，`ConnectQueueError.expired` |
| `queueOverflow` | FIFO 背壓信號使應用程序可以優雅地卸載負載。 | `ConnectQueueError.overflow(limit:)` |
| `internal` |其他一切：SDK 濫用、缺少 Norito 橋、損壞的日誌。 | `ConnectSessionError.missingDecryptionKeys`、`ConnectCryptoError.*` |

每個 SDK 都會發布一個符合分類法的錯誤類型並公開
結構化遙測屬性：`category`、`code`、`fatal` 和可選
元數據（`http_status`、`underlying`）。

## 快速映射

Swift 導出 `ConnectError`、`ConnectErrorCategory` 和輔助協議
`IrohaSwift/Sources/IrohaSwift/ConnectError.swift`。所有公共連接錯誤
類型符合 `ConnectErrorConvertible`，因此應用程序可以調用 `error.asConnectError()`
並將結果轉發到遙測/記錄層。|快速錯誤 |類別 |代碼|筆記|
|------------|----------|------|--------|
| `ConnectClient.ClientError.alreadyStarted` | `internal` | `client.already_started` |表示雙`start()`；開發者錯誤。 |
| `ConnectClient.ClientError.closed` | `transport` | `client.closed` |關閉後發送/接收時引發。 |
| `ConnectClient.ClientError.unknownPayload` | `codec` | `client.unknown_payload` | WebSocket 傳遞文本有效負載，同時期望二進制。 |
| `ConnectSessionError.streamEnded` | `transport` | `session.stream_ended` | Counterparty 意外關閉了流。 |
| `ConnectSessionError.missingDecryptionKeys` | `internal` | `session.missing_direction_keys` |應用程序忘記配置對稱密鑰。 |
| `ConnectEnvelopeError.invalidPayload` | `codec` | `envelope.invalid_payload` | Norito 有效負載缺少必填字段。 |
| `ConnectEnvelopeError.unknownPayloadKind` | `codec` | `envelope.unknown_payload_kind` |舊 SDK 看到的未來有效負載。 |
| `ConnectCodecError.*` | `codec` | `codec.bridge_unavailable` / `codec.encode_failed` / `codec.decode_failed` | Norito 橋丟失或無法編碼/解碼幀字節。 |
| `ConnectCryptoError.*` | `internal` | `crypto.*` |橋接不可用或密鑰長度不匹配。 |
| `ConnectQueueError.overflow(limit:)` | `queueOverflow` | `queue.overflow` |離線隊列長度超出配置的限制。 |
| `URLError(.timedOut)` | `timeout` | `network.timeout` |由 `URLSessionWebSocketTask` 浮現。 |
| `URLError` TLS 案例 | `authorization` | `network.tls_failure` | ATS/TLS 協商失敗。 |
| `DecodingError` / `EncodingError` | `codec` | `codec.*` | SDK 中其他地方的 JSON 解碼/編碼失敗；消息使用 Swift 解碼器上下文。 |
|任何其他 `Error` | `internal` | `unknown_error` |保證包羅萬象；消息鏡像 `LocalizedError`。 |

單元測試 (`IrohaSwift/Tests/IrohaSwiftTests/ConnectErrorTests.swift`) 鎖定
映射，以便將來的重構不能默默地更改類別或代碼。

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

## 遙測和儀表板

Swift SDK提供`ConnectError.telemetryAttributes(fatal:httpStatus:)`
它返回規範屬性映射。出口商應轉發這些
屬性到 `connect.error` OTEL 事件中，並帶有可選附加功能：

```
attributes = {
  "category": "transport",
  "code": "client.closed",
  "fatal": "true",
  "http_status": "101" // optional
  "underlying": "URLError(Code=-1005 ...)" // optional
}
```

儀表板將 `connect.error` 計數器與隊列深度相關聯 (`connect.queue_depth`)
並重新連接直方圖以檢測回歸，而無需探索日誌。

## Android 映射Android SDK 導出 `ConnectError`、`ConnectErrorCategory`、`ConnectErrorTelemetryOptions`、
`ConnectErrorOptions`，以及下面的幫助實用程序
`org.hyperledger.iroha.android.connect.error`。構建器風格的幫助程序轉換任何 `Throwable`
到符合分類法的有效負載中，從傳輸/TLS/編解碼器異常中推斷類別，
並公開確定性遙測屬性，以便 OpenTelemetry/採樣堆棧可以消耗
沒有定制適配器的結果。 【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectError.java:8】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectErrors.java:25】
`ConnectQueueError` 已經實現了 `ConnectErrorConvertible`，發出隊列溢出/超時
溢出/過期條件的類別，以便離線隊列檢測可以連接到同一流程。 【java/iroha_android/src/main/java/org/hyperledger/iroha/android/connect/error/ConnectQueueError.java:5】
Android SDK 自述文件現在引用了分類法並展示瞭如何包裝傳輸異常
在發出遙測數據之前，保持 dApp 指導與 Swift 基線保持一致。 【java/iroha_android/README.md:167】

## JavaScript 映射

Node.js/瀏覽器客戶端導入 `ConnectError`、`ConnectQueueError`、`ConnectErrorCategory` 和
`connectErrorFrom()` 來自 `@iroha/iroha-js`。共享助手檢查 HTTP 狀態代碼，
節點錯誤代碼（套接字、TLS、超時）、`DOMException` 名稱以及編解碼器無法發出
本說明中記錄的相同類別/代碼，而 TypeScript 定義對遙測進行建模
屬性覆蓋，因此工具無需手動轉換即可發出 OTEL 事件。 【javascript/iroha_js/src/connectError.js:1】【javascript/iroha_js/index.d.ts:840】
SDK 自述文件記錄了工作流程並鏈接回該分類，以便應用程序團隊可以
逐字複製儀器片段。 【javascript/iroha_js/README.md:1387】

## 後續步驟（跨 SDK）

- **隊列集成：**一旦離線隊列發貨，確保出隊/丟棄邏輯
  表面 `ConnectQueueError` 值，因此溢出遙測仍然值得信賴。