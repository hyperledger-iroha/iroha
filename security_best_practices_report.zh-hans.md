<!-- Auto-generated stub for Chinese (Simplified) (zh-hans) translation. Replace this content with the full translation. -->

---
lang: zh-hans
direction: ltr
source: security_best_practices_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6055090fbfab93113392610b3eb78ef0a0e97d6fef4a94292c5609ec5562e925
source_last_modified: "2026-03-26T06:12:11.487656+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# 安全最佳实践报告

日期：2026-03-25

## 执行摘要

我根据当前工作区刷新了之前的 Torii/Soracloud 报告
代码并将审查扩展到风险最高的服务器、SDK 和
加密/序列化表面。此次审计初步确认了三项
入口/身份验证问题：两个高严重性和一个中等严重性。
这三项发现现已通过补救措施在当前树中关闭
如下所述。后续运输和内部调度审核已确认
另外九个中等严重性问题：一个出站 P2P 身份绑定问题
差距，一个出站 P2P TLS 降级默认值，两个 Torii 信任边界错误
webhook 传递和 MCP 内部调度，一种跨 SDK 敏感传输
Swift、Java/Android、Kotlin 和 JS 客户端的差距，其中之一 SoraFS
可信代理/客户端 IP 策略差距，一个 SoraFS 本地代理
绑定/身份验证间隙，一个对等遥测地理查找明文后备，
以及一个操作员身份验证远程 IP 故障开放锁定/速率键控间隙。那些
后来的发现也已在当前树中关闭。之前报道的四项 Soracloud 关于原始私钥的调查结果
HTTP、仅限内部本地读取代理执行、不计量公共运行时
当前代码中不再存在回退和远程 IP 附件租赁。
这些在下面被标记为关闭/被取代，并带有更新的代码参考。这仍然是一次以代码为中心的审计，而不是一次详尽的红队演习。
我优先考虑外部可访问的 Torii 入口和请求身份验证路径，然后
抽查 IVM、`iroha_crypto`、`norito`、Swift/Android/JS SDK
请求签名助手、对等遥测地理路径和 SoraFS
工作站代理帮助程序加上 SoraFS 引脚/网关客户端 IP 策略
表面。该入口/身份验证、出口策略没有实时确认的问题，
对等遥测地理、采样的 P2P 传输默认值、MCP 调度、采样的 SDK
传输、操作员身份验证锁定/速率键控、SoraFS 可信代理/客户端 IP
策略或本地代理切片在本报告中修复后仍然存在。
后续强化还扩展了故障关闭启动事实集
采样 IVM CUDA/Metal 加速器路径；该工作并未确认新的
失败打开问题。金属样品 Ed25519
修复多个 ref10 漂移后，签名路径现已在此主机上恢复
Metal/CUDA 端口中的点：验证中的正基点处理，
`d2` 常数、精确的 `fe_sq2` 还原路径、杂散最终
`fe_mul` 提步，以及让肢体缺失的术后场归一化
边界在标量阶梯上漂移。现在聚焦金属回归报道
保持签名管道启用并验证 `[true, false]`针对CPU参考路径的加速器。样本启动真相现已确定
还可以直接探测实时向量（`vadd64`、`vand`、`vxor`、`vor`）
Metal 和 CUDA 上的单轮 AES 批处理内核位于这些后端之前
保持启用状态。后来的依赖性扫描添加了七个实时第三方发现
到积压，但当前的树已经删除了两个活动的 `tar`
通过删除 `xtask` Rust `tar` 依赖项并替换
`iroha_crypto` `libsodium-sys-stable` 使用 OpenSSL 支持的互操作测试
等价物。当前的树也取代了直接的 PQ 依赖关系
在那次扫描中标记，从迁移 `soranet_pq`、`iroha_crypto` 和 `ivm`
`pqcrypto-dilithium` / `pqcrypto-kyber` 至
`pqcrypto-mldsa` / `pqcrypto-mlkem` 同时保留现有的 ML-DSA /
ML-KEM API 表面。随后当天的依赖传递然后固定了工作区
`reqwest` / `rustls` 版本到已修补的补丁版本，这使
`rustls-webpki` 位于当前解析中的固定 `0.103.10` 线上。唯一的
剩下的依赖策略例外是两个传递性的未维护的
宏板条箱（`derivative`、`paste`），现已在
`deny.toml` 因为没有安全升级并且删除它们需要
替换或供应多个上游堆栈。这剩余加速器功为
镜像 CUDA 修复和扩展 CUDA 真值集的运行时验证
具有实时 CUDA 驱动程序支持的主机，未确认正确性或失败打开
当前树中的问题。

## 高严重性

### SEC-05：应用程序规范请求验证绕过多重签名阈值（已于 2026 年 3 月 24 日关闭）

影响：

- 多重签名控制帐户的任何单个成员密钥都可以授权
  面向应用程序的请求，应该需要阈值或加权
  法定人数。
- 这会影响信任 `verify_canonical_request` 的每个端点，包括
  Soracloud 签名突变入口、内容访问和签名帐户 ZK
  附件租赁。

证据：

- `verify_canonical_request` 将多重签名控制器扩展为完整成员
  公钥列表并接受验证请求的第一个密钥
  签名，不评估阈值或累积权重：
  `crates/iroha_torii/src/app_auth.rs:198-210`。
- 实际的多重签名策略模型同时带有 `threshold` 和加权
  成员，并拒绝阈值超过总权重的策略：
  `crates/iroha_data_model/src/account/controller.rs:92-95`，
  `crates/iroha_data_model/src/account/controller.rs:163-178`，
  `crates/iroha_data_model/src/account/controller.rs:188-196`。
- 助手位于 Soracloud 突变入口的授权路径上
  `crates/iroha_torii/src/lib.rs:2141-2157`，内容签名帐户访问
  `crates/iroha_torii/src/content.rs:359-360`，以及附件租赁
  `crates/iroha_torii/src/lib.rs:7962-7968`。

为什么这很重要：- 请求签名者被视为 HTTP 准入的帐户权限，
  但实施过程会默默地将多重签名帐户降级为“任何单一帐户”
  成员可以单独行动。”
- 将深度防御 HTTP 签名层转变为授权
  绕过多重签名保护的帐户。

推荐：

- 在应用程序身份验证层拒绝多重签名控制的帐户，直到
  存在正确的见证格式，或者扩展协议以便 HTTP 请求
  携带并验证满足阈值的完整多重签名见证集
  重量。
- 添加涵盖 Soracloud 突变中间件、内容身份验证和 ZK 的回归
  低于阈值多重签名的附件。

整治情况：

- 在当前代码中，由于无法关闭多重签名控制的帐户而被关闭
  `crates/iroha_torii/src/app_auth.rs`。
- 验证者不再接受“任何单个成员都可以签名”的语义
  多重签名 HTTP 授权；多重签名请求将被拒绝，直到
  存在满足阈值的见证格式。
- 回归覆盖现在包括一个专用的多重签名拒绝案例
  `crates/iroha_torii/src/app_auth.rs`。

## 高严重性

### SEC-06：应用程序规范请求签名可无限期重播（2026 年 3 月 24 日关闭）

影响：- 捕获的有效请求可以重放，因为签名消息没有
  时间戳、随机数、过期或重播缓存。
- 这可以重复状态改变的Soracloud突变请求并重新发出
  在原始客户端很久之后进行帐户绑定内容/附件操作
  有意为之。

证据：

- Torii 将应用程序规范请求定义为唯一
  `METHOD + path + sorted query + body hash` 中
  `crates/iroha_torii/src/app_auth.rs:1-17` 和
  `crates/iroha_torii/src/app_auth.rs:74-89`。
- 验证者仅接受 `X-Iroha-Account` 和 `X-Iroha-Signature` 并且不
  不强制新鲜度或维护重播缓存：
  `crates/iroha_torii/src/app_auth.rs:137-218`。
- JS、Swift 和 Android SDK 帮助程序生成相同的易于重放的标头
  没有随机数/时间戳字段的配对：
  `javascript/iroha_js/src/canonicalRequest.js:50-82`，
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift:41-68`，和
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:67-106`。
- Torii 的操作员签名路径已经使用了更强的模式
  缺少面向应用程序的路径：时间戳、随机数和重播缓存
  `crates/iroha_torii/src/operator_signatures.rs:1-21` 和
  `crates/iroha_torii/src/operator_signatures.rs:266-294`。

为什么这很重要：

- HTTPS 本身并不能阻止反向代理、调试记录器的重播，
  受感染的客户端主机或任何可以记录有效请求的中介。
- 由于所有主要客户端SDK都实现了相同的方案，因此重放
  弱点是系统性的，而不仅仅是服务器。

推荐：- 将签名的新鲜度材料添加到应用程序身份验证请求中，至少有一个时间戳
  和随机数，并使用有界重播缓存拒绝陈旧或重用的元组。
- 明确版本应用程序规范请求格式，以便 Torii 和 SDK 可以
  安全地弃用旧的双标头方案。
- 添加回归证明 Soracloud 突变、内容的重播拒绝
  访问和附件 CRUD。

整治情况：

- 在当前代码中关闭。 Torii 现在需要四标头方案
  （`X-Iroha-Account`、`X-Iroha-Signature`、`X-Iroha-Timestamp-Ms`、
  `X-Iroha-Nonce`) 并签名/验证
  `METHOD + path + sorted query + body hash + timestamp + nonce` 中
  `crates/iroha_torii/src/app_auth.rs`。
- 新鲜度验证现在强制执行有界时钟偏差窗口，验证
  随机数形状，并通过内存中的重播缓存拒绝重用的随机数，该缓存的
  旋钮通过 `crates/iroha_config/src/parameters/{defaults,actual,user}.rs` 呈现。
- JS、Swift 和 Android 帮助程序现在发出相同的四标头格式
  `javascript/iroha_js/src/canonicalRequest.js`，
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift`，和
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java`。
- 回归覆盖范围现在包括正签名验证和重播，
  过时时间戳和缺失新鲜度拒绝案例
  `crates/iroha_torii/src/app_auth.rs`。

## 中等严重程度

### SEC-07：mTLS 强制执行信任可欺骗的转发标头（2026 年 3 月 24 日关闭）

影响：- 如果直接使用Torii，则可以绕过依赖于`require_mtls`的部署
  可达或前端代理未剥离客户端提供的
  `x-forwarded-client-cert`。
- 该问题与配置相关，但触发后会变成已声明的问题
  将客户端证书要求转换为普通标头检查。

证据：

- Norito-RPC 门控通过调用强制执行 `require_mtls`
  `norito_rpc_mtls_present`，仅检查是否
  `x-forwarded-client-cert` 存在且非空：
  `crates/iroha_torii/src/lib.rs:1897-1926`。
- 操作员身份验证引导/登录流程调用 `check_common`，仅拒绝
  当 `mtls_present(headers)` 为假时：
  `crates/iroha_torii/src/operator_auth.rs:562-570`。
- `mtls_present` 也只是一个非空的 `x-forwarded-client-cert` 签入
  `crates/iroha_torii/src/operator_auth.rs:1212-1216`。
- 这些操作员身份验证处理程序仍然作为路由公开
  `crates/iroha_torii/src/lib.rs:16658-16672`。

为什么这很重要：

- 仅当 Torii 位于
  强化代理，剥离并重写标头。代码未验证
  该部署假设本身。
- 默默地依赖于反向代理卫生的安全控制很容易实现
  在分段、金丝雀或事件响应路由更改期间配置错误。

推荐：- 在可能的情况下更倾向于直接由运输国执行。如果必须有代理
  使用，信任经过身份验证的代理到 Torii 通道并需要允许列表
  或来自该代理的签名证明，而不是原始标头的存在。
- 记录 `require_mtls` 在直接暴露的 Torii 侦听器上不安全。
- 在 Norito-RPC 上添加伪造 `x-forwarded-client-cert` 输入的负面测试
  和操作员身份验证引导路由。

整治情况：

- 通过将转发标头信任绑定到配置的代理在当前代码中关闭
  CIDR 而不是单独存在原始标头。
- `crates/iroha_torii/src/limits.rs` 现在提供共享
  `has_trusted_forwarded_header(...)` 门，以及 Norito-RPC
  (`crates/iroha_torii/src/lib.rs`) 和操作员身份验证
  (`crates/iroha_torii/src/operator_auth.rs`) 与调用者 TCP 对等方一起使用
  地址。
- `iroha_config` 现在公开了 `mtls_trusted_proxy_cidrs`
  操作员身份验证和 Norito-RPC；默认值是仅环回的。
- 回归覆盖现在拒绝伪造的 `x-forwarded-client-cert` 输入
  操作员身份验证和共享限制帮助程序中的不受信任的远程。

## 中等严重程度

### SEC-08：出站 P2P 拨号未将经过身份验证的密钥绑定到预期对等点 ID（已于 2026 年 3 月 25 日关闭）

影响：- 到对等点 `X` 的出站拨号可以像其密钥的任何其他对等点 `Y` 一样完成
  应用层握手成功签名，因为握手
  验证了“此连接上的密钥”，但从未检查过该密钥是否是
  网络参与者想要到达的对等点 ID。
- 在许可的覆盖中，稍后的拓扑/允许列表检查仍然会删除
  错误的密钥，所以这主要是替换/可达性错误而不是
  而不是直接共识模拟错误。在公共覆盖中，它可以让
  受损的地址、DNS 响应或中继端点替换为不同的
  出站拨号上的观察者身份。

证据：

- 出站对等状态将预期的 `peer_id` 存储在
  `crates/iroha_p2p/src/peer.rs:5153-5179`，但是旧的握手流程
  在签名验证之前删除该值。
- `GetKey::read_their_public_key` 验证了签名的握手有效负载并
  然后立即从公布的远程公钥构造一个 `Peer`
  在 `crates/iroha_p2p/src/peer.rs:6266-6355` 中，没有与
  `peer_id` 最初供应给 `connecting(...)`。
- 相同的传输堆栈显式禁用 TLS / QUIC 证书
  `crates/iroha_p2p/src/transport.rs`中对P2P进行验证，因此绑定
  目标对等点 ID 的应用程序层身份验证密钥至关重要
  对出站连接进行身份检查。

为什么这很重要：- 该设计有意将对等身份验证置于传输之上
  层，这使得握手密钥检查唯一持久的身份绑定
  在出站拨号上。
- 如果没有这种检查，网络层可以默默地对待“成功”
  已验证某个对等点”相当于“已到达我们拨打的对等点”
  这是一个较弱的保证，并且可能会扭曲拓扑/声誉状态。

推荐：

- 通过签名握手阶段携带预期出站 `peer_id` 并
  如果验证的远程密钥与其不匹配，则关闭失败。
- 保持集中回归，证明握手的签名有效
  错误的密钥被拒绝，而正常的签名握手仍然成功。

整治情况：

- 在当前代码中关闭。 `ConnectedTo` 和下游握手状态现在
  携带预期出站 `PeerId`，并且
  `GetKey::read_their_public_key` 拒绝不匹配的经过身份验证的密钥
  `HandshakePeerMismatch` 中的 `crates/iroha_p2p/src/peer.rs`。
- 重点回归范围现在包括
  `outgoing_handshake_rejects_unexpected_peer_identity` 和现有的
  正 `handshake_v1_defaults_to_trust_gossip` 路径
  `crates/iroha_p2p/src/peer.rs`。

### SEC-09：HTTPS/WSS Webhook 交付在连接时重新解析经过审查的主机名（2026 年 3 月 25 日关闭）

影响：- 安全的 webhook 交付验证了针对 webhook 的目标 DNS 答案
  出口策略，但随后丢弃那些经过审查的地址并让客户端
  堆栈在实际 HTTPS 或 WSS 连接期间再次解析主机名。
- 可以在验证和连接时间之间影响 DNS 的攻击者可以
  可能将以前允许的主机名重新绑定到被阻止的私有或
  仅限运营商的目标并绕过基于 CIDR 的 Webhook 防护。

证据：

- 出口防护解析并过滤候选目标地址
  `crates/iroha_torii/src/webhook.rs:1746-1829`，以及安全的交付路径
  将这些经过审查的地址列表传递到 HTTPS / WSS 帮助程序中。
- 旧的 HTTPS 帮助程序随后针对原始 URL 构建了一个通用客户端
  主机在`crates/iroha_torii/src/webhook.rs`并且没有绑定连接
  到经过审查的地址集，这意味着内部再次发生了 DNS 解析
  HTTP 客户端。
- 旧的 WSS 助手同样名为 `tokio_tungstenite::connect_async(url)`
  针对原始主机名，这也重新解析了主机而不是
  重复使用已经批准的地址。

为什么这很重要：

- 目的地允许列表仅在所检查的地址是正确的情况下才起作用
  客户端实际连接到。
- 策略批准后重新解析会在 DNS 重新绑定/TOCTOU 间隙上创建
  运营商可能信任 SSRF 式遏制的路径。

推荐：- 将经过审查的 DNS 答案固定到实际的 HTTPS 连接路径中，同时保留
  SNI/证书验证的原始主机名。
- 对于 WSS，将 TCP 套接字直接连接到经过审查的地址并运行 TLS
  通过该流进行 websocket 握手，而不是调用基于主机名的
  方便连接器。

整治情况：

- 在当前代码中关闭。 `crates/iroha_torii/src/webhook.rs` 现在派生
  `https_delivery_dns_override(...)` 和
  `websocket_pinned_connect_addr(...)` 来自经过审查的地址集。
- HTTPS 传输现在使用 `reqwest::Client::builder().resolve_to_addrs(...)`
  因此，当 TCP 连接处于连接状态时，原始主机名对 TLS 保持可见
  固定到已经批准的地址。
- WSS 交付现在将原始 `TcpStream` 打开到经过审查的地址并执行
  `tokio_tungstenite::client_async_tls_with_config(...)` 通过该流，
  这避免了策略验证后的第二次 DNS 查找。
- 回归覆盖范围现在包括
  `https_delivery_dns_override_pins_vetted_domain_addresses`，
  `https_delivery_dns_override_skips_ip_literals`，和
  `websocket_pinned_connect_addr_pins_secure_delivery_when_guarded` 中
  `crates/iroha_torii/src/webhook.rs`。

### SEC-10：MCP 内部路由调度标记环回和继承的允许列表权限（已于 2026 年 3 月 25 日关闭）

影响：- 当启用 Torii MCP 时，内部工具调度将每个请求重写为
  无论实际调用者如何，都会进行环回。信任呼叫者 CIDR 的路由
  因此，特权或节流旁路可能会将 MCP 流量视为
  `127.0.0.1`。
- 该问题是配置门控的，因为默认情况下禁用 MCP，并且
  受影响的路由仍然依赖于允许列表或类似的环回信任
  政策，但一旦这些政策将 MCP 变成了特权升级的桥梁
  旋钮一起启用。

证据：

- 先前插入的 `crates/iroha_torii/src/mcp.rs` 中的 `dispatch_route(...)`
  `x-iroha-remote-addr: 127.0.0.1` 和合成环回 `ConnectInfo`
  每个内部发送的请求。
- `iroha.parameters.get` 以只读模式暴露在 MCP 表面上，并且
  `/v1/parameters` 当调用者 IP 属于
  在 `crates/iroha_torii/src/lib.rs:5879-5888` 中配置允许列表。
- `apply_extra_headers(...)` 还接受任意 `headers` 条目
  MCP 调用者，因此保留内部信任标头，例如
  `x-iroha-remote-addr` 和 `x-forwarded-client-cert` 未明确
  受保护。

为什么这很重要：- 内部桥接层必须保留原始信任边界。更换
  具有环回功能的真正调用者有效地将每个 MCP 调用者视为
  一旦请求通过网桥，内部客户端就会这样做。
- 该错误很微妙，因为外部可见的 MCP 配置文件仍然可以看到
  只读，而内部 HTTP 路由看到更特权的来源。

推荐：

- 保留外部 `/v1/mcp` 请求已收到的呼叫者 IP
  从 Torii 的远程地址中间件中合成 `ConnectInfo`
  该值而不是环回。
- 处理仅入口信任标头，例如 `x-iroha-remote-addr` 和
  `x-forwarded-client-cert` 作为保留的内部标头，因此 MCP 调用者无法
  通过 `headers` 参数走私或覆盖它们。

整治情况：

- 在当前代码中关闭。 `crates/iroha_torii/src/mcp.rs` 现在导出
  从外部请求注入的内部调度远程IP
  `x-iroha-remote-addr` 标头并从该实数合成 `ConnectInfo`
  呼叫者 IP 而不是环回。
- `apply_extra_headers(...)` 现在同时掉落 `x-iroha-remote-addr` 和
  `x-forwarded-client-cert` 作为保留的内部标头，因此 MCP 调用者
  无法通过工具参数欺骗环回/入口代理信任。
- 回归覆盖范围现在包括
  `dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks`
  `dispatch_route_blocks_remote_addr_spoofing_from_extra_headers`，和
  `apply_extra_headers_blocks_reserved_internal_headers` 中
  `crates/iroha_torii/src/mcp.rs`。### SEC-11：SDK 客户端允许通过不安全或跨主机传输发送敏感请求材料（2026 年 3 月 25 日关闭）

影响：

- The sampled Swift, Java/Android, Kotlin, and JS clients did not
  始终将所有敏感请求形状视为传输敏感。
  根据助手的不同，调用者可以发送不记名/API 令牌标头、原始标头
  `private_key*` JSON 字段，或应用程序规范验证签名材料
  普通 `http` / `ws` 或通过跨主机绝对 URL 覆盖。
- 特别是在 JS 客户端中，在之后添加了 `canonicalAuth` 标头
  `_request(...)` 完成运输检查，仅车身 `private_key`
  JSON 根本不算敏感传输。

证据：- 斯威夫特现在将守卫集中在
  `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift` 并应用它
  `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift`，
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`，和
  `IrohaSwift/Sources/IrohaSwift/ConnectClient.swift`；在此之前
  这些帮助者不共享单一的交通政策大门。
- Java/Android 现在将相同的策略集中在
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java`
  并从 `NoritoRpcClient.java`、`ToriiRequestBuilder.java` 应用它，
  `OfflineToriiClient.java`, `SubscriptionToriiClient.java`,
  `stream/ToriiEventStreamClient.java`，和
  `websocket/ToriiWebSocketClient.java`。
- Kotlin 现在反映了该政策
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt`
  并从匹配的 JVM client/request-builder/event-stream / 应用它
  websocket 表面。
- JS `ToriiClient._request(...)` 现在处理 `canonicalAuth` 加上 JSON 主体
  包含 `private_key*` 字段作为敏感传输材料
  `javascript/iroha_js/src/toriiClient.js`，以及遥测事件形状
  `javascript/iroha_js/index.d.ts` 现在记录 `hasSensitiveBody` /
  当使用 `allowInsecure` 时，为 `hasCanonicalAuth`。

为什么这很重要：

- 移动设备、浏览器和本地开发助手通常指向可变的开发/登台
  基本 URL。如果客户端没有将敏感请求固定到配置的
  方案/主机，方便的绝对 URL 覆盖或纯 HTTP 基础可以
  将 SDK 变成秘密渗透或签名请求降级路径。
- 风险比不记名代币更广泛。原始 `private_key` JSON 和新鲜
  规范认证签名在网络上也是安全敏感的，并且
  不应默默地绕过传输策略。

推荐：- 在每个 SDK 中集中传输验证并在网络 I/O 之前应用它
  对于所有敏感请求形状：身份验证标头、应用程序规范身份验证签名、
  和原始 `private_key*` JSON。
- 仅将 `allowInsecure` 保留为显式本地/开发逃生舱口，并发出
  当呼叫者选择加入时进行遥测。
- 在共享请求构建器上添加集中回归，而不仅仅是在
  更高级别的便利方法，因此未来的助手继承相同的守卫。

整治情况：- 在当前代码中关闭。采样的 Swift、Java/Android、Kotlin 和 JS
  客户端现在拒绝敏感数据的不安全或跨主机传输
  请求上面的形状，除非调用者选择仅记录开发人员
  不安全模式。
- Swift 重点回归现在涵盖不安全的 Norito-RPC 身份验证标头，
  不安全的 Connect websocket 传输和 raw-`private_key` Torii 请求
  尸体。
- 以 Kotlin 为中心的回归现在涵盖不安全的 Norito-RPC 身份验证标头，
  离线/订阅 `private_key` 主体、SSE 身份验证标头和 websocket
  身份验证标头。
- 以 Java/Android 为中心的回归现在涵盖了不安全的 Norito-RPC 身份验证
  标头、离线/订阅 `private_key` 主体、SSE 身份验证标头以及
  通过共享 Gradle 线束的 websocket auth 标头。
- 以 JS 为中心的回归现在涵盖了不安全和跨主机
  `private_key`-body 请求加上不安全的 `canonicalAuth` 请求
  `javascript/iroha_js/test/transportSecurity.test.js`，同时
  `javascript/iroha_js/test/toriiCanonicalAuth.test.js` 现在运行正
  安全基本 URL 上的规范验证路径。

### SEC-12：SoraFS 本地 QUIC 代理接受非环回绑定，无需客户端身份验证（2026 年 3 月 25 日关闭）

影响：- `LocalQuicProxyConfig.bind_addr` 以前可以设置为 `0.0.0.0`，
  LAN IP，或任何其他非环回地址，暴露了“本地”
  工作站代理作为远程可访问的 QUIC 侦听器。
- 该侦听器未对客户端进行身份验证。任何可以到达的对等点
  完成 QUIC/TLS 会话并发送版本匹配的握手可以
  然后打开 `tcp`、`norito`、`car` 或 `kaigi` 流，具体取决于哪个
  桥接模式已配置。
- 在 `bridge` 模式下，将操作员错误配置转变为远程 TCP
  操作员工作站上的中继和本地文件流表面。

证据：

- `LocalQuicProxyConfig::parsed_bind_addr(...)` 在
  `crates/sorafs_orchestrator/src/proxy.rs` 之前只解析了套接字
  地址并且不拒绝非环回接口。
-同一文件中的 `spawn_local_quic_proxy(...)` 启动 QUIC 服务器
  自签名证书和 `.with_no_client_auth()`。
- `handle_connection(...)` 接受任何 `ProxyHandshakeV1` 的客户
  version 匹配单个支持的协议版本，然后输入
  应用程序流循环。
- `handle_tcp_stream(...)` 通过拨打任意 `authority` 值
  `TcpStream::connect(...)`，而 `handle_norito_stream(...)`，
  `handle_car_stream(...)`和`handle_kaigi_stream(...)`流本地文件
  从配置的假脱机/缓存目录。

为什么这很重要：- 仅当客户端同意时，自签名证书才能保护服务器身份
  选择验证它。它不对客户端进行身份验证。一旦代理
  可以在环回之外到达，握手路径相当于仅版本
  入场。
- API 和文档将此帮助程序描述为本地工作站代理
  浏览器/SDK 集成，因此允许远程访问绑定地址
  信任边界不匹配，而不是预期的远程服务模式。

推荐：

- 在任何非环回 `bind_addr` 上失败关闭，因此当前助手无法
  暴露在本地工作站之外。
- 如果远程代理暴露成为产品要求，请介绍
  首先显式客户端身份验证/能力许可，而不是
  放松绑定防护装置。

整治情况：

- 在当前代码中关闭。现在 `crates/sorafs_orchestrator/src/proxy.rs`
  使用 `ProxyError::BindAddressNotLoopback` 拒绝非环回绑定地址
  在 QUIC 侦听器启动之前。
- 配置字段文档
  `docs/source/sorafs/developer/orchestrator.md` 和
  `docs/portal/docs/sorafs/orchestrator-config.md` 现在文档
  `bind_addr` 仅作为环回。
- 回归覆盖范围现在包括
  `spawn_local_quic_proxy_rejects_non_loopback_bind_addr` 和现有的
  本地桥接测试阳性
  `proxy::tests::tcp_stream_bridge_transfers_payload` 中
  `crates/sorafs_orchestrator/src/proxy.rs`。

### SEC-13：出站 P2P TLS-over-TCP 默认情况下静默降级为明文（2026 年 3 月 25 日关闭）

影响：- 启用 `network.tls_enabled=true` 实际上并未强制仅使用 TLS
  出境运输，除非运营商也发现并设置
  `tls_fallback_to_plain=false`。
- 因此出站路径上的任何 TLS 握手失败或超时
  默认情况下将拨号降级为纯文本 TCP，从而删除了传输
  针对途中攻击者或不当行为的机密性和完整性
  中间盒。
- 签名的应用程序握手仍然验证了对等方身份，因此
  这是传输策略的降级，而不是对等欺骗的绕过。

证据：

- `tls_fallback_to_plain` 默认为 `true`
  `crates/iroha_config/src/parameters/user.rs`，因此回退已激活
  除非操作员在配置中明确覆盖它。
- `crates/iroha_p2p/src/peer.rs` 中的 `Connecting::connect_tcp(...)` 尝试
  每当设置 `tls_enabled` 时都会进行 TLS 拨号，但在 TLS 错误或超时时
  记录警告并回退到明文 TCP
  `tls_fallback_to_plain` 已启用。
- `crates/iroha_kagami/src/wizard.rs` 中面向操作员的示例配置和
  `docs/source/p2p*.md` 中的公共 P2P 传输文档也做了广告
  明文后备作为默认行为。

为什么这很重要：- 一旦运营商开启 TLS，更安全的期望是失败关闭：如果 TLS
  无法建立会话，拨号应该失败而不是静静地
  脱落运输保护。
- 默认情况下保留降级会使部署对以下因素敏感
  网络路径怪癖、代理干扰和主动握手中断
  在推出过程中很容易错过的方式。

推荐：

- 保留纯文本后备作为显式兼容性旋钮，但默认为
  `false` 因此 `network.tls_enabled=true` 表示仅 TLS，除非操作员选择
  进入降级行为。

整治情况：

- 在当前代码中关闭。现在 `crates/iroha_config/src/parameters/user.rs`
  默认 `tls_fallback_to_plain` 至 `false`。
- 默认配置夹具快照、Kagami 示例配置和类似默认配置
  P2P/Torii 测试助手现在镜像强化的运行时默认值。
- 重复的 `docs/source/p2p*.md` 文档现在将纯文本回退描述为
  明确的选择加入而不是默认的。

### SEC-14：对等遥测地理查找默默地退回到纯文本第三方 HTTP（已于 2026 年 3 月 25 日关闭）

影响：- 启用 `torii.peer_geo.enabled=true` 而没有导致显式端点
  Torii 将对等主机名发送到内置明文
  `http://ip-api.com/json/...` 服务。
- 泄露的对等遥测目标指向未经身份验证的第三方 HTTP
  依赖性，并让任何路径上的攻击者或受损的端点源被伪造
  位置元数据返回到 Torii。
- 该功能是选择加入的，但公共遥测文档和示例配置
  公布了内置默认值，这使得部署模式不安全
  一旦运营商启用了对等地理查找，就可能会发生这种情况。

证据：

- `crates/iroha_torii/src/telemetry/peers/monitor.rs` 先前定义
  `DEFAULT_GEO_ENDPOINT = "http://ip-api.com/json"` 和
  `construct_geo_query(...)` 每当使用该默认值
  `GeoLookupConfig.endpoint` 是 `None`。
- 每当对等遥测监视器生成 `collect_geo(...)`
  `geo_config.enabled` 为真
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`，所以明文
  回退可以在已发布的运行时代码中实现，而不是在仅测试代码中实现。
- `crates/iroha_config/src/parameters/defaults.rs` 中的默认配置和
  `crates/iroha_config/src/parameters/user.rs` 保留 `endpoint` 未设置，并且
  `docs/source/telemetry*.md` plus 中重复的遥测文档
  `docs/source/references/peer.template.toml` 明确记录了
  当操作员启用该功能时内置后备。

为什么这很重要：- 对等遥测不应通过纯文本 HTTP 默默地传出对等主机名
  一旦运营商打开便利标志，就可以向第三方服务提供服务。
- 隐藏的不安全默认设置也会破坏变更审核：运营商可以
  启用地理查找，而无需意识到他们已经引入了外部
  第三方元数据泄露和未经身份验证的响应处理。

推荐：

- 删除默认的内置地理端点。
- 启用对等地理查找时需要显式 HTTPS 端点
  否则跳过查找。
- 集中回归证明缺失或非 HTTPS 端点失败关闭。

整治情况：

- 在当前代码中关闭。 `crates/iroha_torii/src/telemetry/peers/monitor.rs`
  现在使用 `MissingEndpoint` 拒绝丢失的端点，拒绝非 HTTPS
  具有 `InsecureEndpoint` 的端点，并跳过对等地理查找而不是
  默默地退回到明文内置服务。
- `crates/iroha_config/src/parameters/user.rs` 不再注入隐式
  解析时的端点，因此未设置的配置状态在所有
  运行时验证的方式。
- 重复的遥测文档和规范示例配置
  `docs/source/references/peer.template.toml` 现在声明
  当 `torii.peer_geo.endpoint` 必须显式配置 HTTPS
  功能已启用。
- 回归覆盖范围现在包括
  `construct_geo_query_requires_explicit_endpoint`，
  `construct_geo_query_rejects_non_https_endpoint`，
  `collect_geo_requires_explicit_endpoint_when_enabled`，和
  `collect_geo_rejects_non_https_endpoint` 中
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`。### SEC-15：SoraFS pin 和网关策略缺乏可信代理感知客户端 IP 解析（已于 2026 年 3 月 25 日关闭）

影响：

- 反向代理 Torii 部署之前可能会崩溃不同的 SoraFS
  评估存储引脚 CIDR 时调用者访问代理套接字地址
  允许列表、每个客户端的存储 pin 限制和网关客户端
  指纹。
- 削弱了对 `/v1/sorafs/storage/pin` 和 SoraFS 的滥用控制
  通过以下方式在常见的反向代理拓扑中网关下载表面
  多个客户端共享一个存储桶或一个白名单身份。
- 默认路由器仍然注入内部远程元数据，因此这不是一个
  新的未经身份验证的入口绕过，但这是一个真正的信任边界差距
  对于代理感知部署和处理程序边界过度信任
  内部远程 IP 标头。

证据：- `crates/iroha_config/src/parameters/user.rs` 和
  `crates/iroha_config/src/parameters/actual.rs` 以前没有通用
  `torii.transport.trusted_proxy_cidrs` 旋钮，因此代理感知规范客户端
  IP 分辨率在一般 Torii 入口边界处不可配置。
- `inject_remote_addr_header(...)` 中的 `crates/iroha_torii/src/lib.rs`
  之前覆盖了内部 `x-iroha-remote-addr` 标头
  仅 `ConnectInfo`，它从中删除了可信转发的客户端 IP 元数据
  真正的反向代理。
- `PinSubmissionPolicy::enforce(...)` 在
  `crates/iroha_torii/src/sorafs/pin.rs` 和
  `gateway_client_fingerprint(...)` 在 `crates/iroha_torii/src/sorafs/api.rs`
  没有共享可信代理感知的规范 IP 解析步骤
  处理程序边界。
- `crates/iroha_torii/src/sorafs/pin.rs` 中的存储引脚节流也已锁定
  每当存在令牌时，仅在不记名令牌上，这意味着多个
  共享一个有效 pin 令牌的代理客户端被迫采用相同的费率
  即使在区分了客户端 IP 后。

为什么这很重要：

- 反向代理是 Torii 的正常部署模式。如果运行时
  无法一致地区分受信任的代理和不受信任的调用者、IP
  允许列表和每个客户端的限制不再意味着运营商认为的那样
  意思是。
- SoraFS 引脚和网关路径显然是滥用敏感表面，因此
  将呼叫者折叠到代理 IP 或过度信任过时的转发
  即使基本路线仍然存在，元数据在操作上也很重要
  需要其他入场。

推荐：- 添加通用 Torii `trusted_proxy_cidrs` 配置表面并解决
  来自 `ConnectInfo` 的规范客户端 IP 加上任何预先存在的转发
  仅当套接字对等方位于该允许列表中时才使用标头。
- 在 SoraFS 处理程序路径中重用规范 IP 解析，而不是
  盲目信任内部标头。
- 通过令牌加上规范客户端 IP 来限制共享令牌存储 pin 的范围
  当两者都在场时。

整治情况：

- 在当前代码中关闭。 `crates/iroha_config/src/parameters/defaults.rs`，
  `crates/iroha_config/src/parameters/user.rs`，和
  `crates/iroha_config/src/parameters/actual.rs`现已曝光
  `torii.transport.trusted_proxy_cidrs`，默认为空列表。
- `crates/iroha_torii/src/lib.rs` 现在解析规范客户端 IP
  入口中间件内部的 `limits::ingress_remote_ip(...)` 并重写
  仅来自受信任代理的内部 `x-iroha-remote-addr` 标头。
- `crates/iroha_torii/src/sorafs/pin.rs` 和
  `crates/iroha_torii/src/sorafs/api.rs` 现在解析规范客户端 IP
  针对存储引脚处理程序边界处的 `state.trusted_proxy_nets`
  策略和网关客户端指纹识别，因此直接处理程序路径无法
  过度信任陈旧的转发 IP 元数据。
- 存储引脚节流现在通过“令牌+规范”来键共享不记名令牌
  客户端 IP`（当两者都存在时），保留每个客户端存储桶以供共享
  pin 令牌。
- 回归覆盖范围现在包括
  `limits::tests::ingress_remote_ip_preserves_trusted_forwarded_header`，
  `limits::tests::ingress_remote_ip_ignores_forwarded_header_from_untrusted_peer`，
  `sorafs::pin::tests::rate_key_scopes_shared_tokens_by_ip`，
  `storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`，
  `car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`，以及
  配置夹具
  `torii_transport_trusted_proxy_cidrs_default_to_empty`。### SEC-16：当注入的远程 IP 标头丢失时，操作员身份验证锁定和速率限制键控回退到共享匿名存储桶（已于 2026 年 3 月 25 日关闭）

影响：

- 操作员授权准入已经收到接受的套接字 IP，但是
  锁定/速率限制键忽略它并将请求折叠到共享中
  每当内部 `x-iroha-remote-addr` 标头出现时，`"anon"` 存储桶
  缺席。
- 这不是默认路由器上的新公共入口旁路，因为
  入口中间件在这些处理程序运行之前重写内部标头。
  对于较窄的内部机构来说，这仍然是一个真正的失败开放式信任边界差距。
  处理程序路径、直接测试以及到达的任何未来路由
  注入中间件之前的`OperatorAuth`。
- 在这些情况下，一个呼叫者可能会消耗速率限制预算或锁定其他呼叫者
  本应通过源 IP 隔离的呼叫者。

证据：- `OperatorAuth::check_common(...)` 在
  `crates/iroha_torii/src/operator_auth.rs` 已收到
  `remote_ip: Option<IpAddr>`，但以前称为 `auth_key(headers)`
  并完全放弃了传输 IP。
- 之前的 `crates/iroha_torii/src/operator_auth.rs` 中的 `auth_key(...)`
  仅解析 `limits::REMOTE_ADDR_HEADER`，否则返回 `"anon"`。
- `crates/iroha_torii/src/limits.rs` 中的通用 Torii 助手已经有
  ` effective_remote_ip(headers,
  Remote)`，更喜欢注入的规范标头，但又回到
  当直接处理程序调用绕过中间件时接受套接字 IP。

为什么这很重要：

- 锁定和速率限制状态必须以相同的有效呼叫者身份为关键
  Torii 的其余部分用于策略决策。回到共享状态
  匿名存储桶将缺失的内部元数据跃点转变为跨客户端
  干扰，而不是将影响局限于真正的呼叫者。
- 操作员身份验证是滥用敏感边界，因此即使是中等严重程度
  桶碰撞问题值得明确关闭。

推荐：

- 从 `limits:: effective_remote_ip(headers,
  remote_ip)` 因此注入的标头在存在时仍然会获胜，但是是直接的
  处理程序调用回退到传输地址而不是 `"anon"`。
- 仅当内部标头和
  传输 IP 不可用。

整治情况：- 在当前代码中关闭。 `crates/iroha_torii/src/operator_auth.rs` 现在调用
  `auth_key(headers, remote_ip)` 来自 `check_common(...)` 和 `auth_key(...)`
  现在从以下位置导出锁定/速率限制密钥
  `limits::effective_remote_ip(headers, remote_ip)`。
- 回归覆盖范围现在包括
  `operator_auth_key_uses_remote_ip_when_internal_header_missing` 和
  `operator_auth_key_prefers_injected_header_over_transport_remote_ip` 中
  `crates/iroha_torii/src/operator_auth.rs`。

## 先前报告中的已结束或被取代的调查结果

- 早期原始私钥 Soracloud 发现：已关闭。当前突变入口
  拒绝内联 `authority` / `private_key` 字段
  `crates/iroha_torii/src/soracloud.rs:5305-5308`，将 HTTP 签名者绑定到
  `crates/iroha_torii/src/soracloud.rs:5310-5315` 中的突变来源，以及
  返回草稿交易指令，而不是服务器提交签名的交易指令
  交易在 `crates/iroha_torii/src/soracloud.rs:5556-5565` 中。
- 早期仅内部本地读取代理执行发现：已关闭。公共
  路由解析现在会跳过非公开和更新/私有更新处理程序
  `crates/iroha_torii/src/soracloud.rs:8445-8463`，运行时拒绝
  非公共本地读取路由
  `crates/irohad/src/soracloud_runtime.rs:5906-5923`。
- 早期公共运行时未计量的后备发现：按书面关闭。公共
  运行时入口现在强制执行速率限制和飞行上限
  `crates/iroha_torii/src/lib.rs:8837-8852` 在解析公共路由之前
  `crates/iroha_torii/src/lib.rs:8858-8860`。
- 早期的远程 IP 附件租赁发现：已关闭。现已出租附件
  需要经过验证的登录帐户
  `crates/iroha_torii/src/lib.rs:7962-7968`。
  附属租赁先前继承了 SEC-05 和 SEC-06；那个继承
  已被上述当前应用程序身份验证补救措施关闭。## 依赖性发现- `cargo deny check advisories bans sources --hide-inclusion-graph` 现在运行
  直接针对跟踪的 `deny.toml`，现在报告三个实时数据
  从生成的工作区锁定文件中发现依赖性。
- `tar` 建议不再出现在活动依赖关系图中：
  `xtask/src/mochi.rs` 现在使用具有固定参数的 `Command::new("tar")`
  向量，并且 `iroha_crypto` 不再拉动 `libsodium-sys-stable`
  将这些检查交换到 OpenSSL 后进行 Ed25519 互操作测试。
- 目前的发现：
  - `RUSTSEC-2024-0388`：`derivative` 未维护。
  - `RUSTSEC-2024-0436`：`paste` 未维护。
- 影响分类：
  - 先前报告的 `tar` 公告已关闭
    依赖图。 `cargo tree -p xtask -e normal -i tar`，
    `cargo tree -p iroha_crypto -e all -i tar`，和
    `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` 现在全部失败
    “包 ID 规范...与任何包都不匹配”，并且
    `cargo deny` 不再报告 `RUSTSEC-2026-0067` 或
    `RUSTSEC-2026-0068`。
  - 之前报告的直接 PQ 更换咨询现已结束
    当前的树。 `crates/soranet_pq/Cargo.toml`，
    `crates/iroha_crypto/Cargo.toml` 和 `crates/ivm/Cargo.toml` 现在取决于
    在 `pqcrypto-mldsa` / `pqcrypto-mlkem` 上，以及触摸的 ML-DSA / ML-KEM
    迁移后运行时测试仍然通过。
  - 之前报告的 `rustls-webpki` 通报在以下国家不再有效
    目前的决心。工作区现在将 `reqwest` / `rustls` 固定到修补补丁版本，使 `rustls-webpki` 保留在 `0.103.10` 上，这是
    超出咨询范围。
  - `derivative` 和 `paste` 不直接在工作区源中使用。
    他们通过 BLS / arkworks 堆栈传递进入
    `w3f-bls` 和其他几个上游板条箱，因此删除它们需要
    上游或依赖堆栈更改而不是本地宏清理。
    当前的树现在明确接受这两个建议
    `deny.toml` 并记录原因。

## 覆盖范围注释- 服务器/运行时/配置/网络：SEC-05、SEC-06、SEC-07、SEC-08、SEC-09、
  SEC-10、SEC-12、SEC-13、SEC-14、SEC-15 和 SEC-16 在期间得到确认
  审核现已在当前树中关闭。额外硬化
  当前的树现在也使 Connect websocket/会话准入失败
  当内部注入的远程 IP 标头丢失时关闭，而不是
  将该条件默认为环回。
- IVM/加密/序列化：本次审计没有额外确认的发现
  切片。积极的证据包括机密关键材料归零
  `crates/iroha_crypto/src/confidential.rs:53-60` 和重放感知 Soranet PoW
  `crates/iroha_crypto/src/soranet/pow.rs:823-879` 中的签名票证验证。
  后续强化现在还可以拒绝两个畸形的加速器输出
  采样的 Norito 路径：`crates/norito/src/lib.rs` 验证加速 JSON
  `TapeWalker` 取消引用偏移量之前的 Stage-1 磁带现在还需要
  动态加载的 Metal/CUDA Stage-1 帮助程序以证明与
  激活前的标量结构索引构建器，以及
  `crates/norito/src/core/gpu_zstd.rs` 验证 GPU 报告的输出长度
  在截断编码/解码缓冲区之前。 `crates/norito/src/core/simd_crc64.rs`
  现在还可以针对动态加载的 GPU CRC64 帮助程序进行自测试
  `hardware_crc64` 之前的规范回退会信任它们，因此格式错误
  帮助程序库失败关闭，而不是默默更改 Norito 校验和行为。无效的助手结果现在会回退，而不是恐慌释放
  建立或漂移校验和奇偶校验。在IVM侧，采样加速器
  启动门现在还涵盖 CUDA Ed25519 `signature_kernel`、CUDA BN254
  add/sub/mul 内核，CUDA `sha256_leaves` / `sha256_pairs_reduce`，实时
  CUDA 矢量/AES 批处理内核（`vadd64`、`vand`、`vxor`、`vor`、
  `aesenc_batch`、`aesdec_batch`) 以及配套的金属
  `sha256_leaves`/向量/AES 批处理内核在这些路径受信任之前。的
  采样金属 Ed25519 签名路径现在也回来了
  在该主机上设置的实时加速器内：较早的奇偶校验失败是
  通过恢复标量阶梯上的 ref10 肢体绑定标准化来修复，
  重点金属回归现在验证 `[s]B`、`[h](-A)`、
  二的幂基点梯形图，以及完整的 `[true, false]` 批量验证
  在 Metal 上相对于 CPU 参考路径。镜像的CUDA源发生变化
  在`--features cuda --tests`下编译，现在CUDA启动真值集
  如果实时 Merkle 叶/对内核偏离 CPU，则无法关闭
  参考路径。运行时 CUDA 验证在此仍受主机限制
  环境。
- SDK/示例：SEC-11 在以传输为重点的采样过程中得到确认
  跨越 Swift、Java/Android、Kotlin 和 JS 客户端，并且查找现已在当前树中关闭。 JS、Swift 和 Android
  规范请求助手也已更新为新的
  新鲜度感知的四标头方案。
  采样的 QUIC 流媒体传输评论也没有产生实时结果
  在当前树中运行时查找：`StreamingClient::connect(...)`，
  `StreamingServer::bind(...)`，能力协商助手是
  目前仅通过 `crates/iroha_p2p` 中的测试代码进行测试，
  `crates/iroha_core`，因此该帮助程序中的许可自签名验证程序
  路径目前仅用于测试/帮助程序，而不是已发货的入口表面。
- 示例和移动示例应用程序仅在抽查级别进行审查，并且
  不应被视为经过彻底审核。

## 验证和覆盖范围差距- `cargo deny check advisories bans sources --hide-inclusion-graph` 现在运行
  直接使用跟踪的 `deny.toml`。在当前模式运行下，
  `bans` 和 `sources` 是干净的，而 `advisories` 则失败并显示五个
  上面列出的依赖性发现。
- 已关闭 `tar` 结果的依赖关系图清理验证已通过：
  `cargo tree -p xtask -e normal -i tar`，
  `cargo tree -p iroha_crypto -e all -i tar`，和
  `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` 现在全部报告
  “包 ID 规范...与任何包都不匹配，”而
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p xtask create_archive_packages_bundle_directory -- --nocapture`，
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p xtask`，
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_verify -- --nocapture`，
  和
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_sign -- --nocapture`
  一切都过去了。
- `bash scripts/fuzz_smoke.sh` 现在通过运行真正的 libFuzzer 会话
  `cargo +nightly fuzz`，但 IVM 的一半脚本未在
  这次通过是因为 `tlv_validate` 的第一个夜间构建仍在
  交接时的进展。该构建已经完成足以执行
  直接生成 libFuzzer 二进制文件：
  `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
  现在到达 libFuzzer 运行循环，并在运行 200 次后干净退出
  空语料库。修复后，Norito 一半成功完成
  harness/manifest 漂移和 `json_from_json_equiv` 模糊目标编译
  打破。
- Torii 修复验证现在包括：
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_torii --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib --no-run`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_accepts_valid_signature -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_rejects_replayed_nonce -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_key_ -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib trusted_forwarded_header_requires_proxy_membership -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo check -p iroha_torii --lib --features app_api_https`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo test -p iroha_torii --lib https_delivery_dns_override_ --features app_api_https -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook2 cargo test -p iroha_torii --lib websocket_pinned_connect_addr_pins_secure_delivery_when_guarded -- --nocapture`- `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_blocks_remote_addr_spoofing_from_extra_headers -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib apply_extra_headers_blocks_reserved_internal_headers -- --nocapture`
  - 较窄的 `--no-default-features --features app_api,app_api_https` Torii
    测试矩阵在 DA / 中仍然存在不相关的现有编译失败
    Soracloud 门控的 lib-test 代码，因此此通道验证了已发布的
    默认功能 MCP 路径和 `app_api_https` webhook 路径而不是
    声称完全覆盖最小功能。
- Trusted-proxy/SoraFS 修复验证现在包括：
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib limits::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib sorafs::pin::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_requires_token_and_respects_allowlist_and_rate_limit -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limits_repeated_clients -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_config --test fixtures torii_transport_trusted_proxy_cidrs_default_to_empty -- --nocapture`
- P2P 修复验证现在包括：
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib --no-run`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib outgoing_handshake_rejects_unexpected_peer_identity -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib handshake_v1_defaults_to_trust_gossip -- --nocapture`
- SoraFS 本地代理修复验证现在包括：
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-proxy cargo test -p sorafs_orchestrator spawn_local_quic_proxy_rejects_non_loopback_bind_addr -- --nocapture`
  - `/tmp/iroha-codex-target-proxy/debug/deps/sorafs_orchestrator-b3be10a343598c7b --exact proxy::tests::tcp_stream_bridge_transfers_payload --nocapture`
- P2P TLS 默认修复验证现在包括：
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-config-tls2 cargo test -p iroha_config tls_fallback_defaults_to_tls_only -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib start_rejects_tls_without_feature_when_tls_only_outbound -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib tls_only_dial_requires_p2p_tls_feature_when_no_fallback -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-connect-gating cargo test -p iroha_torii --test connect_gating --no-run`
- SDK 端修复验证现在包括：
  - `node --test javascript/iroha_js/test/canonicalRequest.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `cd IrohaSwift && swift test --filter CanonicalRequestTests`
  - `cd IrohaSwift && swift test --filter 'NoritoRpcClientTests/testCallRejectsInsecureAuthorizationHeader'`
  - `cd IrohaSwift && swift test --filter 'ConnectClientTests/testBuildsConnectWebSocketRequestRejectsInsecureTransport'`
  - `cd IrohaSwift && swift test --filter 'ToriiClientTests/testCreateSubscriptionPlanRejectsInsecureTransportForPrivateKeyBody'`
  - `cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.TransportSecurityClientTest --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.NoritoRpcClientTests,org.hyperledger.iroha.android.client.OfflineToriiClientTests,org.hyperledger.iroha.android.client.SubscriptionToriiClientTests,org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests,org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClientTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain`
  - `node --test javascript/iroha_js/test/transportSecurity.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `node --test javascript/iroha_js/test/toriiSubscriptions.test.js`
- Norito 后续验证现在包括：
  - `python3 scripts/check_norito_bindings_sync.py`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture`
  - `bash scripts/fuzz_smoke.sh`（Norito 目标 `json_parse_string`，
    `json_parse_string_ref`、`json_skip_value` 和 `json_from_json_equiv`线束/目标修复后通过）
- IVM 模糊后续验证现在包括：
  - `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
- IVM 加速器后续验证现在包括：
  - `xcrun -sdk macosx metal -c crates/ivm/src/metal_ed25519.metal -o /tmp/metal_ed25519.air`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo check -p ivm --features cuda --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_bitwise_single_vector_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_batch_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture`
- 重点 CUDA lib-test 执行在此主机上仍然受到环境限制：
  `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo test -p ivm --features cuda --lib selftest_covers_ -- --nocapture`
  仍然无法链接，因为 CUDA 驱动程序符号 (`cu*`) 不可用。
- 重点金属运行时验证现在完全在加速器上运行
  主机：采样的 Ed25519 签名管道在启动时保持启用状态
  自检，`metal_ed25519_batch_matches_cpu` 验证 `[true, false]`
  直接在 Metal 上针对 CPU 参考路径。
- 我没有重新运行完整的工作区 Rust 测试扫描、完整的 `npm test` 或
  在此修复过程中完整的 Swift/Android 套件。

## 优先修复待办事项列表

### 下一批- 监控明确接受的传递性的上游替换
  `derivative` / `paste` 宏债务并删除 `deny.toml` 异常时
  可以在不破坏 BLS / Halo2 / PQ / 稳定性的情况下进行安全升级
  UI 依赖堆栈。
- 在热缓存上重新运行完整的夜间 IVM fuzz-smoke 脚本，以便
  `tlv_validate` / `kotodama_lower` 旁边有稳定的记录结果
  现在绿色的 Norito 目标。直接 `tlv_validate` 二进制运行现已完成，
  但完整的夜间烟雾仍然很出色。
- 在带有 CUDA 驱动程序的主机上重新运行聚焦的 CUDA lib-test 自测试片
  安装了库，因此验证了扩展的 CUDA 启动真值集
  超越 `cargo check` 和镜像 Ed25519 标准化修复加上
  新的向量/AES 启动探针在运行时执行。
- 一旦不相关的套件级别重新运行更广泛的 JS/Swift/Android/Kotlin 套件
  该分支上的阻止程序已被清除，因此新的规范请求和
  除了上述重点帮助测试之外，还涵盖了运输保安人员。
- 决定是否应保留长期应用程序身份验证多重签名故事
  失败关闭或发展一流的 HTTP 多重签名见证格式。

＃＃＃ 监视器- 继续重点审查 `ivm` 硬件加速/不安全路径
  以及剩余的 `norito` 流/加密边界。 JSON 第一阶段
  GPU zstd 帮助程序切换现在已得到强化，可以在以下情况下失败关闭
  发布版本，采样的 IVM 加速器启动真值集现已发布
  更广泛，但更广泛的不安全/决定论审查仍然开放。