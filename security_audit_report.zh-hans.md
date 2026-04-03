<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# 安全审计报告

日期：2026-03-26

## 执行摘要

此审核重点关注当前树中风险最高的表面：Torii HTTP/API/auth 流、P2P 传输、秘密处理 API、SDK 传输防护和附件清理程序路径。

我发现了 6 个可操作的问题：

- 2 个严重程度较高的发现
- 4 项中等严重程度的发现

最重要的问题是：

1. Torii 当前记录每个 HTTP 请求的入站请求标头，这可以将承载令牌、API 令牌、操作员会话/引导令牌以及转发的 mTLS 标记公开到日志中。
2. 多个公共 Torii 路由和 SDK 仍然支持将原始 `private_key` 值发送到服务器，以便 Torii 可以代表调用者进行签名。
3. 一些“秘密”路径被视为普通请求体，包括某些 SDK 中的机密种子派生和规范请求身份验证。

## 方法

- Torii、P2P、加密/虚拟机和 SDK 秘密处理路径的静态审查
- 有针对性的验证命令：
  - `cargo check -p iroha_torii --lib --message-format short` -> 通过
  - `cargo check -p iroha_p2p --message-format short` -> 通过
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> 通过
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> 通过，仅重复版本警告
- 本次未完成：
  - 完整工作区构建/测试/clippy
  - Swift/Gradle 测试套件
  - CUDA/Metal 运行时验证

## 调查结果

### SA-001 高：Torii 全局记录敏感请求标头影响：任何提供请求跟踪的部署都可能将承载/API/操作员令牌和相关身份验证材料泄漏到应用程序日志中。

证据：

- `crates/iroha_torii/src/lib.rs:20752` 启用 `TraceLayer::new_for_http()`
- `crates/iroha_torii/src/lib.rs:20753` 启用 `DefaultMakeSpan::default().include_headers(true)`
- 敏感标头名称在同一服务的其他地方被积极使用：
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

为什么这很重要：

- `include_headers(true)` 将完整的入站标头值记录到跟踪范围中。
- Torii 接受标头中的身份验证材料，例如 `Authorization`、`x-api-token`、`x-iroha-operator-session`、`x-iroha-operator-token` 和 `x-forwarded-client-cert`。
- 因此，日志接收器泄露、调试日志收集或支持包可能成为凭证泄露事件。

建议的补救措施：

- 停止在生产范围中包含完整的请求标头。
- 如果调试仍需要标头日志记录，则为安全敏感标头添加显式编辑。
- 默认情况下，将请求/响应日志记录视为包含秘密，除非数据已明确列入允许列表。

### SA-002 高：公共 Torii API 仍然接受服务器端签名的原始私钥

影响：鼓励客户端通过网络传输原始私钥，以便服务器可以代表他们进行签名，从而在 API、SDK、代理和服务器内存层创建不必要的秘密暴露通道。

证据：- 治理路由文档明确宣传服务器端签名：
  - `crates/iroha_torii/src/gov.rs:495`
- 路由实现解析提供的私钥并在服务器端签名：
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDK 主动将 `private_key` 序列化为 JSON 主体：
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

注意事项：

- 此模式并不孤立于一个路由族。当前的树包含跨治理、离线现金、订阅和其他面向应用程序的 DTO 的相同便利模型。
- 仅 HTTPS 传输检查可减少意外的明文传输，但不能解决服务器端秘密处理或日志记录/内存暴露风险。

建议的补救措施：

- 弃用所有携带原始 `private_key` 数据的请求 DTO。
- 要求客户在本地签名并提交签名或完全签名的交易/信封。
- 在兼容性窗口后从 OpenAPI/SDK 中删除 `private_key` 示例。

### SA-003 中：机密密钥派生将秘密种子材料发送到 Torii 并将其回显

影响：机密密钥派生 API 将种子材料转换为正常的请求/响应有效负载数据，增加了通过代理、中间件、日志、跟踪、崩溃报告或客户端滥用而泄露种子的机会。

证据：- 请求直接接受种子材料：
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- 响应模式以十六进制和 Base64 形式回显种子：
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- 处理程序显式地重新编码并返回种子：
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK 将其公开为常规网络方法，并将回显的种子保留在响应模型中：
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

建议的补救措施：

- 更喜欢 CLI/SDK 代码中的本地密钥派生，并完全删除远程派生路由。
- 如果必须保留路线，则切勿在响应中返回种子，并将种子承载体在所有传输防护和遥测/记录路径中标记为敏感。

### SA-004 中：SDK 传输敏感度检测对非 `private_key` 秘密材料存在盲点

影响：某些 SDK 将对原始 `private_key` 请求强制使用 HTTPS，但仍允许其他安全敏感请求材料通过不安全的 HTTP 传输或传输到不匹配的主机。

证据：- Swift 将规范请求身份验证标头视为敏感：
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- 但 Swift 仍然只在 `"private_key"` 上进行身体匹配：
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Kotlin 仅识别 `authorization` 和 `x-api-token` 标头，然后回退到相同的 `"private_key"` 主体启发式：
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android 也有同样的限制：
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Kotlin/Java 规范请求签名者会生成额外的身份验证标头，这些标头不会被自己的传输防护归类为敏感标头：
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

建议的补救措施：

- 用明确的请求分类替换启发式身体扫描。
- 根据合同，而不是通过子字符串匹配，将规范身份验证标头、种子/密码字段、签名突变标头以及任何未来的秘密承载字段视为敏感字段。
- 保持 Swift、Kotlin 和 Java 之间的敏感度规则保持一致。

### SA-005 中：附件“沙箱”只是一个子进程加上 `setrlimit`影响：附件清理程序被描述和报告为“沙盒”，但其实现只是当前二进制文件的 fork/exec，具有资源限制。解析器或归档漏洞仍将使用与 Torii 相同的用户、文件系统视图和环境网络/进程权限来执行。

证据：

- 生成子项后，外部路径将结果标记为沙箱：
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- 子项默认为当前可执行文件：
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- 子进程显式切换回 `AttachmentSanitizerMode::InProcess`：
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- 唯一应用的强化是 CPU/地址空间 `setrlimit`：
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

建议的补救措施：

- 要么实现真正的操作系统沙箱（例如命名空间/seccomp/landlock/监狱式隔离、权限下降、无网络、受限文件系统），要么停止将结果标记为 `sandboxed`。
- 将当前设计视为“子流程隔离”，而不是 API、遥测和文档中的“沙箱”，直到存在真正的隔离。

### SA-006 中：可选 P2P TLS/QUIC 传输禁用证书验证影响：启用 `quic` 或 `p2p_tls` 时，通道提供加密，但不验证远程端点。主动的路径攻击者仍然可以中继或终止通道，从而破坏操作员与 TLS/QUIC 相关的正常安全期望。

证据：

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

建议的补救措施：

- 验证对等证书或在高层签名握手和传输会话之间添加显式通道绑定。
- 如果当前行为是故意的，请将该功能重命名/记录为未经身份验证的加密传输，以便操作员不会将其误认为是完整的 TLS 对等身份验证。

## 建议的补救措施1. 通过编辑或禁用标头日志记录立即修复 SA-001。
2. 设计并发布 SA-002 的迁移计划，以便原始私钥停止跨越 API 边界。
3. 删除或缩小远程机密密钥导出路径，并将种子承载体分类为敏感。
4. 跨 Swift/Kotlin/Java 协调 SDK 传输敏感度规则。
5. 决定附件清理是否需要真正的沙箱或诚实的重命名/重新界定范围。
6. 在运营商启用那些需要经过身份验证的 TLS 的传输之前，澄清并强化 P2P TLS/QUIC 威胁模型。

## 验证说明

- `cargo check -p iroha_torii --lib --message-format short` 通过。
- `cargo check -p iroha_p2p --message-format short` 通过。
- `cargo deny check advisories bans sources --hide-inclusion-graph`在沙箱外运行后通过；它发出重复版本警告，但报告 `advisories ok, bans ok, sources ok`。
- 在此审计期间启动了针对机密派生密钥集路由的重点 Torii 测试，但在编写报告之前尚未完成；无论如何，这一发现都得到了直接来源检验的支持。