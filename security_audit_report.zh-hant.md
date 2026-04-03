<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# 安全審計報告

日期：2026-03-26

## 執行摘要

此審核重點在於目前樹中風險最高的表面：Torii HTTP/API/auth 流、P2P 傳輸、秘密處理 API、SDK 傳輸防護和附件清理程式路徑。

我發現了 6 個可操作的問題：

- 2 個嚴重程度較高的發現
- 4 項嚴重嚴重程度的發現

最重要的問題是：

1. Torii 目前記錄每個 HTTP 請求的入站請求標頭，這可以將承載令牌、API 令牌、操作員會話/引導令牌以及轉送的 mTLS 標記公開到日誌中。
2. 多個公共 Torii 路由和 SDK 仍然支援將原始 `private_key` 值傳送到伺服器，以便 Torii 可以代表呼叫者進行簽署。
3. 一些「秘密」路徑被視為普通請求體，包括某些 SDK 中的機密種子派生和規範請求身份驗證。

## 方法

- Torii、P2P、加密/虛擬機器和 SDK 秘密處理路徑的靜態審查
- 針對性的驗證指令：
  - `cargo check -p iroha_torii --lib --message-format short` -> 透過
  - `cargo check -p iroha_p2p --message-format short` -> 透過
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> 透過
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> 通過，僅重複版本警告
- 本次未完成：
  - 完整工作區建置/測試/clippy
  - Swift/Gradle 測試套件
  - CUDA/Metal 運行時驗證

## 調查結果

### SA-001 高：Torii 全域記錄敏感請求標頭影響：任何提供請求追蹤的部署都可能將承載/API/操作員令牌和相關身份驗證材料洩漏到應用程式日誌中。

證據：

- `crates/iroha_torii/src/lib.rs:20752` 啟用 `TraceLayer::new_for_http()`
- `crates/iroha_torii/src/lib.rs:20753` 啟用 `DefaultMakeSpan::default().include_headers(true)`
- 敏感標頭名稱在同一服務的其他地方被積極使用：
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

為什麼這很重要：

- `include_headers(true)` 將完整的入站標頭值記錄到追蹤範圍中。
- Torii 接受標頭中的驗證資料，例如 `Authorization`、`x-api-token`、`x-iroha-operator-session`、`x-iroha-operator-token` 和 `x-forwarded-client-cert`。
- 因此，日誌接收器洩漏、調試日誌收集或支援包可能成為憑證洩露事件。

建議的補救措施：

- 停止在生產範圍中包含完整的請求標頭。
- 如果偵錯仍需要標頭日誌記錄，則為安全敏感標頭新增明確編輯。
- 預設情況下，將請求/回應日誌記錄視為包含秘密，除非資料已明確列入允許清單。

### SA-002 高：公用 Torii API 仍然接受伺服器端簽署的原始私鑰

影響：鼓勵客戶端透過網路傳輸原始私鑰，以便伺服器可以代表他們進行簽名，從而在 API、SDK、代理和伺服器記憶體層創建不必要的秘密暴露通道。

證據：- 治理路由文件明確宣傳伺服器端簽章：
  - `crates/iroha_torii/src/gov.rs:495`
- 路由實作解析提供的私鑰並在伺服器端簽署：
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDK 主動將 `private_key` 序列化為 JSON 主體：
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

注意事項：

- 此模式並非孤立於一個路由族。目前的樹包含跨治理、離線現金、訂閱和其他以應用程式為導向的 DTO 的相同便利模型。
- 僅 HTTPS 傳輸檢查可減少意外的明文傳輸，但無法解決伺服器端秘密處理或日誌記錄/記憶體暴露風險。

建議的補救措施：

- 棄用所有攜帶原始 `private_key` 資料的請求 DTO。
- 要求客戶在本地簽名並提交簽名或完全簽名的交易/信封。
- 在相容性視窗後從 OpenAPI/SDK 中刪除 `private_key` 範例。

### SA-003 中：機密金鑰衍生將秘密種子資料傳送至 Torii 並將其回顯

影響：機密金鑰派生 API 將種子材料轉換為正常的請求/回應有效負載數據，增加了透過代理、中間件、日誌、追蹤、崩潰報告或客戶端濫用而洩露種子的機會。

證據：- 請求直接接受種子材料：
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- 反應模式以十六進位和 Base64 形式回顯種子：
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- 處理程序明確地重新編碼並返回種子：
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK 將其公開為常規網路方法，並將回顯的種子保留在回應模型中：
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

建議的補救措施：

- 偏好 CLI/SDK 程式碼中的本機金鑰派生，並完全刪除遠端派生路由。
- 如果必須保留路線，則切勿在回應中返回種子，並將種子承載者在所有傳輸防護和遙測/記錄路徑中標記為敏感。

### SA-004 中：SDK 傳輸敏感度偵測對非 `private_key` 秘密資料存在盲點

影響：某些 SDK 將對原始 `private_key` 請求強制使用 HTTPS，但仍允許其他安全敏感請求資料透過不安全的 HTTP 傳輸或傳輸到不匹配的主機。

證據：- Swift 將規範請求身份驗證標頭視為敏感：
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- 但 Swift 仍然只在 `"private_key"` 上進行身體匹配：
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Kotlin 僅辨識 `authorization` 和 `x-api-token` 標頭，然後回退到相同的 `"private_key"` 主體啟發式：
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android 也有同樣的限制：
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Kotlin/Java 規格要求簽署者會產生額外的身份驗證標頭，這些標頭不會被自己的傳輸防護歸類為敏感標頭：
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

建議的補救措施：

- 用明確的請求分類取代啟發式身體掃描。
- 根據合同，而不是透過子字串匹配，將規範身份驗證標頭、種子/密碼字段、簽名突變標頭以及任何未來的秘密承載字段視為敏感字段。
- 保持 Swift、Kotlin 和 Java 之間的敏感度規則一致。

### SA-005 中：附件「沙箱」只是一個子程序加上 `setrlimit`影響：附件清理程式被描述和報告為“沙盒”，但其實作只是目前二進位檔案的 fork/exec，具有資源限制。解析器或歸檔漏洞仍將使用與 Torii 相同的使用者、檔案系統視圖和環境網路/進程權限來執行。

證據：

- 產生子項後，外部路徑將結果標記為沙箱：
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- 子項預設為目前可執行檔：
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- 子進程明確切換回 `AttachmentSanitizerMode::InProcess`：
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- 唯一應用的強化是 CPU/位址空間 `setrlimit`：
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

建議的補救措施：

- 要麼實現真正的作業系統沙箱（例如命名空間/seccomp/landlock/監獄式隔離、權限下降、無網路、受限檔案系統），要麼停止將結果標記為 `sandboxed`。
- 將當前設計視為“子流程隔離”，而不是 API、遙測和文件中的“沙箱”，直到存在真正的隔離。

### SA-006 中：可選 P2P TLS/QUIC 傳輸停用憑證驗證影響：啟用 `quic` 或 `p2p_tls` 時，頻道提供加密，但不驗證遠端端點。主動的路徑攻擊者仍然可以中繼或終止通道，從而破壞操作員與 TLS/QUIC 相關的正常安全期望。

證據：

- QUIC 明确记录许可证书验证：
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- QUIC验证者无条件接受服务器证书：
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- TLS-over-TCP 传输的作用相同：
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

建議的補救措施：

- 驗證對等憑證或在高層簽章握手和傳輸會話之間新增明確通道綁定。
- 如果當前行為是故意的，請將該功能重新命名/記錄為未經身份驗證的加密傳輸，以便操作員不會將其誤認為是完整的 TLS 對等身份驗證。

## 建議的補救措施1. 透過編輯或停用標頭日誌記錄立即修復 SA-001。
2. 設計並發布 SA-002 的遷移計劃，以便原始私鑰停止跨越 API 邊界。
3. 刪除或縮小遠端機密金鑰匯出路徑，並將種子承載體分類為敏感。
4. 跨 Swift/Kotlin/Java 協調 SDK 傳輸敏感度規則。
5. 決定附件清理是否需要真正的沙箱或誠實的重命名/重新界定範圍。
6. 在營運商啟用那些需要經過驗證的 TLS 的傳輸之前，澄清並強化 P2P TLS/QUIC 威脅模型。

## 驗證說明

- `cargo check -p iroha_torii --lib --message-format short` 通過。
- `cargo check -p iroha_p2p --message-format short` 通過。
- `cargo deny check advisories bans sources --hide-inclusion-graph`在沙箱外運行後通過；它發出重複版本警告，但報告 `advisories ok, bans ok, sources ok`。
- 在此審計期間啟動了針對機密派生金鑰集路由的重點 Torii 測試，但在編寫報告之前尚未完成；無論如何，這一發現都得到了直接來源檢驗的支持。