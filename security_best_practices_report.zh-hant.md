<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: security_best_practices_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6055090fbfab93113392610b3eb78ef0a0e97d6fef4a94292c5609ec5562e925
source_last_modified: "2026-03-26T06:12:11.487656+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# 安全最佳實踐報告

日期：2026-03-25

## 執行摘要

我根據目前工作區刷新了先前的 Torii/Soracloud 報告
程式碼並將審查擴展到風險最高的伺服器、SDK 和
加密/序列化表面。這次審計初步確認了三項
入口/身份驗證問題：兩個高嚴重性和一個中等嚴重性。
這三項發現現已透過補救措施在當前樹中關閉
如下所述。後續運輸和內部調度審核已確認
另外九個中度嚴重性問題：一個出站 P2P 身分綁定問題
差距，一個出站 P2P TLS 降級預設值，兩個 Torii 信任邊界錯誤
webhook 傳遞和 MCP 內部調度，一種跨 SDK 敏感傳輸
Swift、Java/Android、Kotlin 和 JS 用戶端的差距，其中之一 SoraFS
可信任代理程式/客戶端 IP 策略差距，一個 SoraFS 本地代理
綁定/身份驗證間隙，一個對等遙測地理查找明文後備，
以及一個操作員身份驗證遠端 IP 故障開放鎖定/速率鍵控間隙。那些
後來的發現也已在目前樹中關閉。先前報導的四項 Soracloud 關於原始私鑰的調查結果
HTTP、僅限內部本機讀取代理執行、不計量公共執行時間
目前代碼中不再存在回退和遠端 IP 附件租賃。
這些在下面被標記為關閉/被取代，並帶有更新的程式碼參考。這仍然是以代碼為中心的審計，而不是一次詳盡的紅隊演習。
我優先考慮外部可存取的 Torii 入口和請求身份驗證路徑，然後
抽查 IVM、`iroha_crypto`、`norito`、Swift/Android/JS SDK
請求簽名助手、對等遙測地理路徑和 SoraFS
工作站代理程式幫助程式加上 SoraFS 接腳/閘道客戶端 IP 策略
表面。此入口/身份驗證、出口策略沒有即時確認的問題，
對等遙測地理、採樣的 P2P 傳輸預設值、MCP 調度、採樣的 SDK
傳輸、操作員驗證鎖定/速率鍵控、SoraFS 可信任代理程式/用戶端 IP
策略或本地代理切片在本報告中修復後仍然存在。
後續強化也擴展了故障關閉啟動事實集
取樣 IVM CUDA/Metal 加速器路徑；此工作並未確認新的
失敗打開問題。金屬樣品 Ed25519
修復多個 ref10 漂移後，簽章路徑現已在此主機上恢復
Metal/CUDA 連接埠中的點：驗證中的正基點處理，
`d2` 常數、精確的 `fe_sq2` 還原路徑、雜訊最終
`fe_mul` 提步，以及讓肢體缺失的術後場歸一化
邊界在標量階梯上漂移。現在聚焦金屬回歸報道
保持簽名管道啟用並驗證 `[true, false]`针对CPU参考路径的加速器。样本启动真相现已确定
还可以直接探测实时向量（`vadd64`、`vand`、`vxor`、`vor`）
Metal 和 CUDA 上的單輪 AES 批次核心位於這些後端之前
保持启用状态。后来的依赖性扫描添加了七个实时第三方发现
到积压，但当前的树已经删除了两个活动的 `tar`
通过删除 `xtask` Rust `tar` 依赖项并替换
`iroha_crypto` `libsodium-sys-stable` 使用 OpenSSL 支持的互操作测试
等價物。目前的樹也取代了直接的 PQ 依賴關係
在那次扫描中标记，从迁移 `soranet_pq`、`iroha_crypto` 和 `ivm`
`pqcrypto-dilithium` / `pqcrypto-kyber` 至
`pqcrypto-mldsa` / `pqcrypto-mlkem` 同時保留現有的 ML-DSA /
ML-KEM API 表面。隨後當天的依賴傳遞然後固定了工作區
`reqwest` / `rustls` 版本到已修补的补丁版本，这使
`rustls-webpki` 位於目前解析中的固定 `0.103.10` 線上。唯一的
剩下的依赖策略例外是两个传递性的未维护的
宏板条箱（`derivative`、`paste`），现已在
`deny.toml` 因为没有安全升级并且删除它们需要
替换或供应多个上游堆栈。這剩餘加速器功為
鏡像 CUDA 修復和擴展 CUDA 真值集的運行時驗證
具有即時 CUDA 驅動程式支援的主機，未確認正確性或失敗開啟
當前樹中的問題。

## 高嚴重性

### SEC-05：應用程式規格請求驗證繞過多重簽章閾值（已於 2026 年 3 月 24 日關閉）

影響：

- 多重簽章控制帳戶的任何單一成員金鑰都可以授權
  面向應用程式的請求，應該需要閾值或加權
  法定人數。
- 這會影響信任 `verify_canonical_request` 的每個端點，包括
  Soracloud 簽名突變入口、內容存取和簽名帳戶 ZK
  附件租賃。

證據：

- `verify_canonical_request` 將多重簽章控制器擴充為完整成員
  公鑰列表並接受驗證請求的第一個金鑰
  簽名，不評估閾值或累積權重：
  `crates/iroha_torii/src/app_auth.rs:198-210`。
- 實際的多重簽章策略模型同時帶有 `threshold` 和加權
  成員，並拒絕閾值超過總權重的策略：
  `crates/iroha_data_model/src/account/controller.rs:92-95`，
  `crates/iroha_data_model/src/account/controller.rs:163-178`，
  `crates/iroha_data_model/src/account/controller.rs:188-196`。
- 助手位於 Soracloud 突變入口的授權路徑上
  `crates/iroha_torii/src/lib.rs:2141-2157`，內容簽名帳戶訪問
  `crates/iroha_torii/src/content.rs:359-360`，以及附件租賃
  `crates/iroha_torii/src/lib.rs:7962-7968`。

為什麼這很重要：- 請求簽署者被視為 HTTP 准入的帳號權限，
  但實施過程會默默地將多重簽章帳號降級為“任何單一帳號”
  成員可以單獨行動。 」
- 將深度防禦 HTTP 簽名層轉變為授權
  繞過多重簽名保護的帳戶。

推薦：

- 在應用程式驗證層拒絕多重簽章控制的帳戶，直到
  存在正確的見證格式，或擴展協定以便 HTTP 請求
  攜帶並驗證滿足閾值的完整多重簽名見證集
  重量。
- 新增涵蓋 Soracloud 突變中間件、內容驗證和 ZK 的回歸
  低於閾值多重簽名的附件。

整治情況：

- 在目前程式碼中，由於無法關閉多重簽章控制的帳戶而被關閉
  `crates/iroha_torii/src/app_auth.rs`。
- 驗證者不再接受「任何單一成員都可以簽名」的語意
  多重簽章 HTTP 授權；多重簽章請求將被拒絕，直到
  存在滿足閾值的見證格式。
- 回歸覆蓋現在包括一個專用的多重簽名拒絕案例
  `crates/iroha_torii/src/app_auth.rs`。

## 高嚴重性

### SEC-06：應用程式規格請求簽名可無限期重播（2026 年 3 月 24 日關閉）

影響：- 捕獲的有效請求可以重播，因為簽名訊息沒有
  時間戳記、隨機數、過期或重播快取。
- 這可以重複狀態改變的Soracloud突變請求並重新發出
  在原始客戶端很久之後進行帳戶綁定內容/附件操作
  有意為之。

證據：

- Torii 將應用程式規格請求定義為唯一
  `METHOD + path + sorted query + body hash` 中
  `crates/iroha_torii/src/app_auth.rs:1-17` 和
  `crates/iroha_torii/src/app_auth.rs:74-89`。
- 驗證者僅接受 `X-Iroha-Account` 和 `X-Iroha-Signature` 且不
  不強制新鮮度或維護重播快取：
  `crates/iroha_torii/src/app_auth.rs:137-218`。
- JS、Swift 和 Android SDK 幫助程式產生相同的易於重播的標頭
  沒有隨機數/時間戳字段的配對：
  `javascript/iroha_js/src/canonicalRequest.js:50-82`，
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift:41-68`，和
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:67-106`。
- Torii 的操作員簽章路徑已經使用了更強的模式
  缺少面向應用程式的路徑：時間戳記、隨機數和重播快取
  `crates/iroha_torii/src/operator_signatures.rs:1-21` 和
  `crates/iroha_torii/src/operator_signatures.rs:266-294`。

為什麼這很重要：

- HTTPS 本身並不能阻止反向代理、偵錯記錄器的重播，
  受感染的客戶端主機或任何可以記錄有效請求的中介。
- 由於所有主要客戶端SDK都實現了相同的方案，因此重播
  弱點是系統性的，而不僅僅是伺服器的。

推薦：- 将签名的新鲜度材料添加到应用程序身份验证请求中，至少有一个时间戳
  和随机数，并使用有界重播缓存拒绝陈旧或重用的元组。
- 明确版本应用程序规范请求格式，以便 Torii 和 SDK 可以
  安全地棄用舊的雙標頭方案。
- 添加回归证明 Soracloud 突变、内容的重播拒绝
  訪問和附件 CRUD。

整治情況：

- 在目前程式碼中關閉。 Torii 現在需要四個標頭方案
  （`X-Iroha-Account`、`X-Iroha-Signature`、`X-Iroha-Timestamp-Ms`、
  `X-Iroha-Nonce`) 並簽署/驗證
  `METHOD + path + sorted query + body hash + timestamp + nonce` 中
  `crates/iroha_torii/src/app_auth.rs`。
- 新鲜度验证现在强制执行有界时钟偏差窗口，验证
  随机数形状，并通过内存中的重播缓存拒绝重用的随机数，该缓存的
  旋鈕透過 `crates/iroha_config/src/parameters/{defaults,actual,user}.rs` 呈現。
- JS、Swift 和 Android 帮助程序现在发出相同的四标头格式
  `javascript/iroha_js/src/canonicalRequest.js`，
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift`，和
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java`。
- 回归覆盖范围现在包括正签名验证和重播，
  過時時間戳與缺失新鮮度拒絕案例
  `crates/iroha_torii/src/app_auth.rs`。

## 中等嚴重程度

### SEC-07：mTLS 强制执行信任可欺骗的转发标头（2026 年 3 月 24 日关闭）

影響：- 如果直接使用Torii，則可以繞過依賴`require_mtls`的部署
  可達或前端代理未剝離客戶端提供的
  `x-forwarded-client-cert`。
- 該問題與配置相關，但觸發後會變成已宣告的問題
  將客戶端憑證要求轉換為普通標頭檢查。

證據：

- Norito-RPC 閘控透過呼叫強制執行 `require_mtls`
  `norito_rpc_mtls_present`，僅檢查是否
  `x-forwarded-client-cert` 存在且非空：
  `crates/iroha_torii/src/lib.rs:1897-1926`。
- 操作員驗證引導/登入流程呼叫 `check_common`，僅拒絕
  當 `mtls_present(headers)` 為假時：
  `crates/iroha_torii/src/operator_auth.rs:562-570`。
- `mtls_present` 也只是一個非空的 `x-forwarded-client-cert` 簽入
  `crates/iroha_torii/src/operator_auth.rs:1212-1216`。
- 這些操作員身份驗證處理程序仍然作為路由公開
  `crates/iroha_torii/src/lib.rs:16658-16672`。

為什麼這很重要：

- 僅當 Torii 位於
  強化代理，剝離並重寫標頭。程式碼未驗證
  該部署假設本身。
- 默默地依賴反向代理衛生的安全控制很容易實現
  在分段、金絲雀或事件回應路由變更期間配置錯誤。

推薦：- 在可能的情況下更傾向於直接由運輸國執行。如果必須有代理
  使用，信任經過身份驗證的代理到 Torii 通道並需要允許列表
  或來自該代理人的簽名證明，而不是原始標頭的存在。
- 記錄 `require_mtls` 在直接暴露的 Torii 偵聽器上不安全。
- 在 Norito-RPC 上新增偽造 `x-forwarded-client-cert` 輸入的負面測試
  和操作員身份驗證引導路由。

整治情況：

- 透過將轉送標頭信任綁定到配置的代理程式在目前程式碼中關閉
  CIDR 而非單獨存在原始標頭。
- `crates/iroha_torii/src/limits.rs` 現在提供共享
  `has_trusted_forwarded_header(...)` 門，以及 Norito-RPC
  (`crates/iroha_torii/src/lib.rs`) 和操作員身份驗證
  (`crates/iroha_torii/src/operator_auth.rs`) 與呼叫者 TCP 對等方一起使用
  地址。
- `iroha_config` 現在公開了 `mtls_trusted_proxy_cidrs`
  操作員身份驗證和 Norito-RPC；預設值是僅環回的。
- 回歸覆蓋現在拒絕偽造的 `x-forwarded-client-cert` 輸入
  操作員身份驗證和共用限制幫助程式中的不受信任的遠端。

## 中等嚴重程度

### SEC-08：出站 P2P 撥號未將經過驗證的金鑰綁定到預期對等點 ID（已於 2026 年 3 月 25 日關閉）

影響：- 到對等點 `X` 的出站撥號可以像其密鑰的任何其他對等點 `Y` 一樣完成
  應用層握手成功簽名，因為握手
  驗證了“此連接上的密鑰”，但從未檢查過該密鑰是否為
  網路參與者想要到達的對等點 ID。
- 在許可證的覆蓋中，稍後的拓撲/允許清單檢查仍然會刪除
  錯誤的密鑰，所以這主要是替換/可達性錯誤而不是
  而不是直接共識模擬錯誤。在公共覆蓋中，它可以讓
  受損的位址、DNS 回應或中繼端點被替換為不同的
  出站撥號上的觀察者身分。

證據：

- 出站對等狀態將預期的 `peer_id` 儲存在
  `crates/iroha_p2p/src/peer.rs:5153-5179`，但是舊的握手流程
  在簽章驗證之前刪除該值。
- `GetKey::read_their_public_key` 驗證了簽署的握手有效負載並
  然後立即從公佈的遠端公鑰建構一個 `Peer`
  在 `crates/iroha_p2p/src/peer.rs:6266-6355` 中，沒有與
  `peer_id` 最初供應給 `connecting(...)`。
- 相同的傳輸堆疊明確停用 TLS / QUIC 憑證
  `crates/iroha_p2p/src/transport.rs`中對P2P進行驗證，因此綁定
  目標對等點 ID 的應用程式層身份驗證金鑰至關重要
  對出站連線進行身份檢查。

為什麼這很重要：- 該設計有意將對等身份驗證置於傳輸之上
  層，這使得握手金鑰檢查唯一持久的身份綁定
  在出站撥號上。
- 如果沒有這種檢查，網路層可以默默地對待“成功”
  已驗證某個對等點”相當於“已到達我們撥打的對等點”
  這是一個較弱的保證，並且可能會扭曲拓撲/聲譽狀態。

推薦：

- 透過簽名握手階段攜帶預期出站 `peer_id` 並
  如果驗證的遠端金鑰與其不匹配，則關閉失敗。
- 保持集中回歸，證明握手的簽名有效
  錯誤的金鑰被拒絕，而正常的簽名握手仍然成功。

整治情況：

- 在目前程式碼中關閉。 `ConnectedTo` 和下游握手狀態現在
  攜帶預期出站 `PeerId`，並且
  `GetKey::read_their_public_key` 拒絕不符的經過驗證的金鑰
  `HandshakePeerMismatch` 中的 `crates/iroha_p2p/src/peer.rs`。
- 重點迴歸範圍現在包括
  `outgoing_handshake_rejects_unexpected_peer_identity` 和現有的
  正 `handshake_v1_defaults_to_trust_gossip` 路徑
  `crates/iroha_p2p/src/peer.rs`。

### SEC-09：HTTPS/WSS Webhook 交付在連接時重新解析經過審查的主機名稱（2026 年 3 月 25 日關閉）

影響：- 安全的 webhook 交付驗證了針對 webhook 的目標 DNS 答案
  出口策略，但隨後丟棄那些經過審查的地址並讓客戶端
  堆疊在實際 HTTPS 或 WSS 連線期間再次解析主機名稱。
- 可以在驗證和連線時間之間影響 DNS 的攻擊者可以
  可能將先前允許的主機名稱重新綁定到被封鎖的私有或
  僅限營運商的目標並繞過基於 CIDR 的 Webhook 防護。

證據：

- 出口防護解析並過濾候選目標位址
  `crates/iroha_torii/src/webhook.rs:1746-1829`，以及安全的交付路徑
  將這些經過審查的地址清單傳遞到 HTTPS / WSS 幫助程式中。
- 舊的 HTTPS 幫助程式隨後針對原始 URL 建立了一個通用客戶端
  主機在`crates/iroha_torii/src/webhook.rs`且沒有綁定連接
  到經過審查的地址集，這意味著內部再次發生了 DNS 解析
  HTTP 客戶端。
- 舊的 WSS 助手同樣名為 `tokio_tungstenite::connect_async(url)`
  針對原始主機名，這也重新解析了主機而不是
  重複使用已經批准的地址。

為什麼這很重要：

- 目的地允許清單僅在所檢查的地址是正確的情況下才起作用
  客戶端實際連接到。
- 政策批准後重新解析會在 DNS 重新綁定/TOCTOU 間隙上建立
  營運商可能信任 SSRF 式遏制的路徑。

推薦：- 將經過審查的 DNS 答案固定到實際的 HTTPS 連線路徑中，同時保留
  SNI/憑證驗證的原始主機名稱。
- 對於 WSS，將 TCP 套接字直接連接到經過審查的位址並執行 TLS
  透過此流進行 websocket 握手，而不是呼叫基於主機名的
  方便連接器。

整治情況：

- 在目前程式碼中關閉。 `crates/iroha_torii/src/webhook.rs` 現在派生
  `https_delivery_dns_override(...)` 和
  `websocket_pinned_connect_addr(...)` 來自經過審查的地址集。
- HTTPS 傳輸現在使用 `reqwest::Client::builder().resolve_to_addrs(...)`
  因此，當 TCP 連線處於連線狀態時，原始主機名稱對 TLS 保持可見
  固定到已經批准的地址。
- WSS 交付現在將原始 `TcpStream` 開啟到經過審查的地址並執行
  `tokio_tungstenite::client_async_tls_with_config(...)` 通過該流，
  這避免了策略驗證後的第二次 DNS 查找。
- 回歸覆蓋範圍現在包括
  `https_delivery_dns_override_pins_vetted_domain_addresses`，
  `https_delivery_dns_override_skips_ip_literals`，和
  `websocket_pinned_connect_addr_pins_secure_delivery_when_guarded` 中
  `crates/iroha_torii/src/webhook.rs`。

### SEC-10：MCP 內部路由調度標記環回和繼承的允許清單權限（已於 2026 年 3 月 25 日關閉）

影響：- 當啟用 Torii MCP 時，內部工具調度會將每個請求重寫為
  無論實際呼叫者如何，都會進行環回。信任呼叫者 CIDR 的路由
  因此，特權或節流旁路可能會將 MCP 流量視為
  `127.0.0.1`。
- 此問題是配置門控的，因為預設情況下禁用 MCP，並且
  受影響的路由仍然依賴於允許列表或類似的環回信任
  政策，但一旦這些政策將 MCP 變成了特權升級的橋樑
  旋鈕一起啟用。

證據：

- 先前插入的 `crates/iroha_torii/src/mcp.rs` 中的 `dispatch_route(...)`
  `x-iroha-remote-addr: 127.0.0.1` 和合成環回 `ConnectInfo`
  每個內部發送的請求。
- `iroha.parameters.get` 以唯讀模式暴露在 MCP 表面上，並且
  `/v1/parameters` 當呼叫者 IP 屬於
  在 `crates/iroha_torii/src/lib.rs:5879-5888` 中配置允許清單。
- `apply_extra_headers(...)` 也接受任意 `headers` 條目
  MCP 呼叫者，因此保留內部信任標頭，例如
  `x-iroha-remote-addr` 和 `x-forwarded-client-cert` 未明確
  受保護。

為什麼這很重要：- 內部橋接層必須保留原始信任邊界。更換
  具有環回功能的真正呼叫者有效地將每個 MCP 呼叫者視為
  一旦請求通過網橋，內部客戶端就會這樣做。
- 該錯誤很微妙，因為外部可見的 MCP 設定檔仍然可以看到
  只讀，而內部 HTTP 路由看到更特權的來源。

推薦：

- 保留外部 `/v1/mcp` 請求已收到的呼叫者 IP
  從 Torii 的遠端位址中間件合成 `ConnectInfo`
  該值而不是環回。
- 處理僅入口信任標頭，例如 `x-iroha-remote-addr` 和
  `x-forwarded-client-cert` 作為保留的內部標頭，因此 MCP 呼叫者無法
  透過 `headers` 參數走私或覆蓋它們。

整治情況：

- 在目前程式碼中關閉。 `crates/iroha_torii/src/mcp.rs` 現在導出
  從外部請求注入的內部調度遠端IP
  `x-iroha-remote-addr` 標頭並由此實數合成 `ConnectInfo`
  呼叫者 IP 而不是環回。
- `apply_extra_headers(...)` 現在同時掉落 `x-iroha-remote-addr` 和
  `x-forwarded-client-cert` 作為保留的內部標頭，因此 MCP 呼叫者
  無法透過工具參數欺騙環回/入口代理信任。
- 回歸覆蓋範圍現在包括
  `dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks`
  `dispatch_route_blocks_remote_addr_spoofing_from_extra_headers`，和
  `apply_extra_headers_blocks_reserved_internal_headers` 中
  `crates/iroha_torii/src/mcp.rs`。### SEC-11：SDK 用戶端允許透過不安全或跨主機傳輸發送敏感請求資料（2026 年 3 月 25 日關閉）

影響：

- 採樣的 Swift、Java/Android、Kotlin 和 JS 用戶端沒有
  始終將所有敏感請求形狀視為傳輸敏感。
  根據助手的不同，呼叫者可以發送不記名/API 令牌標頭、原始標頭
  `private_key*` JSON 字段，或應用程式規範驗證簽名材料
  普通 `http` / `ws` 或透過跨主機絕對 URL 覆寫。
- 特別是在 JS 用戶端中，在之後添加了 `canonicalAuth` 標頭
  `_request(...)` 完成運輸檢查，僅車身 `private_key`
  JSON 根本不算敏感傳輸。

證據：- 斯威夫特現在將守衛集中在
  `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift` 並應用它
  `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift`，
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`，和
  `IrohaSwift/Sources/IrohaSwift/ConnectClient.swift`；在此之前
  這些幫助者不共享單一的交通政策大門。
- Java/Android 現在將相同的策略集中在
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java`
  並從 `NoritoRpcClient.java`、`ToriiRequestBuilder.java` 應用它，
  `OfflineToriiClient.java`, `SubscriptionToriiClient.java`,
  `stream/ToriiEventStreamClient.java`，和
  `websocket/ToriiWebSocketClient.java`。
- Kotlin 現在反映了該政策
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt`
  並從匹配的 JVM client/request-builder/event-stream / 應用它
  websocket 表面。
- JS `ToriiClient._request(...)` 現在處理 `canonicalAuth` 加上 JSON 主體
  包含 `private_key*` 字段作為敏感傳輸材料
  `javascript/iroha_js/src/toriiClient.js`，以及遙測事件形狀
  `javascript/iroha_js/index.d.ts` 現在記錄 `hasSensitiveBody` /
  使用 `allowInsecure` 時，為 `hasCanonicalAuth`。

為什麼這很重要：

- 行動裝置、瀏覽器和本機開發助理通常指向可變的開發/登台
  基本 URL。如果客戶端沒有將敏感請求固定到配置的
  方案/主機，方便的絕對 URL 覆蓋或純 HTTP 基礎可以
  將 SDK 變成秘密滲透或簽章請求降級路徑。
- 風險比不記名代幣更廣泛。原始 `private_key` JSON 和新鮮
  規範認證簽名在網路上也是安全敏感的，並且
  不應默默地繞過傳輸策略。

推薦：- 在每個 SDK 中集中傳輸驗證並在網路 I/O 之前應用它
  對於所有敏感請求形狀：身份驗證標頭、應用程式規範身份驗證簽名、
  和原始 `private_key*` JSON。
- 僅將 `allowInsecure` 保留為明確本地/開發逃生艙口，並發出
  當呼叫者選擇加入時進行遙測。
- 在共用請求建構器上新增集中回歸，而不僅僅是在
  更高等級的便利方法，因此未來的助手繼承相同的守衛。

整治情況：- 在目前程式碼中關閉。採樣的 Swift、Java/Android、Kotlin 和 JS
  客戶端現在拒絕敏感資料的不安全或跨主機傳輸
  請求上面的形狀，除非呼叫者選擇僅記錄開發人員
  不安全模式。
- Swift 重點回歸現在涵蓋不安全的 Norito-RPC 身份驗證標頭，
  不安全的 Connect websocket 傳輸和 raw-`private_key` Torii 請求
  屍體。
- 以 Kotlin 為中心的回歸現在涵蓋不安全的 Norito-RPC 身份驗證標頭，
  離線/訂閱 `private_key` 主體、SSE 驗證標頭和 websocket
  身份驗證標頭。
- 以 Java/Android 為中心的回歸現在涵蓋了不安全的 Norito-RPC 身份驗證
  標頭、離線/訂閱 `private_key` 主體、SSE 身份驗證標頭以及
  透過共享 Gradle 線束的 websocket auth 標頭。
- 以 JS 為中心的回歸現在涵蓋了不安全和跨主機
  `private_key`-body 請求加上不安全的 `canonicalAuth` 請求
  `javascript/iroha_js/test/transportSecurity.test.js`，同時
  `javascript/iroha_js/test/toriiCanonicalAuth.test.js` 現在運行正
  安全基本 URL 上的規範驗證路徑。

### SEC-12：SoraFS 本機 QUIC 代理程式接受非環回綁定，無需客戶端身份驗證（2026 年 3 月 25 日關閉）

影響：- `LocalQuicProxyConfig.bind_addr` 以前可以設定為 `0.0.0.0`，
  LAN IP，或任何其他非環回位址，暴露了“本地”
  工作站代理程式作為遠端可存取的 QUIC 偵聽器。
- 該偵聽器未對用戶端進行身份驗證。任何可以到達的對等點
  完成 QUIC/TLS 會話並傳送版本匹配的握手可以
  然後開啟 `tcp`、`norito`、`car` 或 `kaigi` 串流，取決於哪一個
  橋接模式已配置。
- 在 `bridge` 模式下，將操作員錯誤配置轉換為遠端 TCP
  操作員工作站上的中繼和本地文件流表面。

證據：

- `LocalQuicProxyConfig::parsed_bind_addr(...)` 在
  `crates/sorafs_orchestrator/src/proxy.rs` 之前只解析了套接字
  位址並且不拒絕非環回介面。
-同一檔案中的 `spawn_local_quic_proxy(...)` 啟動 QUIC 伺服器
  自簽名證書和 `.with_no_client_auth()`。
- `handle_connection(...)` 接受任何 `ProxyHandshakeV1` 的客戶
  version 匹配單一支援的協定版本，然後輸入
  應用程式流程循環。
- `handle_tcp_stream(...)` 透過撥打任意 `authority` 值
  `TcpStream::connect(...)`，而 `handle_norito_stream(...)`，
  `handle_car_stream(...)`和`handle_kaigi_stream(...)`流本地文件
  從配置的假脫機/快取目錄。

為什麼這很重要：- 僅當客戶端同意時，自簽名憑證才能保護伺服器身份
  選擇驗證它。它不對客戶端進行身份驗證。一旦代理
  可以在環回之外到達，握手路徑相當於僅版本
  入場。
- API 和文件將此幫助程序描述為本機工作站代理
  瀏覽器/SDK 集成，因此允許遠端存取綁定位址
  信任邊界不匹配，而不是預期的遠端服務模式。

推薦：

- 在任何非環回 `bind_addr` 上失敗關閉，因此目前助手無法
  暴露在本地工作站之外。
- 如果遠端代理暴露成為產品要求，請介紹
  首先顯式客戶端身份驗證/能力許可，而不是
  放鬆綁定防護裝置。

整治情況：

- 在目前程式碼中關閉。現在 `crates/sorafs_orchestrator/src/proxy.rs`
  使用 `ProxyError::BindAddressNotLoopback` 拒絕非環回綁定位址
  在 QUIC 偵聽器啟動之前。
- 配置字段文檔
  `docs/source/sorafs/developer/orchestrator.md` 和
  `docs/portal/docs/sorafs/orchestrator-config.md` 現在文檔
  `bind_addr` 僅作為環回。
- 回歸覆蓋範圍現在包括
  `spawn_local_quic_proxy_rejects_non_loopback_bind_addr` 和現有的
  本地橋接測試陽性
  `proxy::tests::tcp_stream_bridge_transfers_payload` 中
  `crates/sorafs_orchestrator/src/proxy.rs`。

### SEC-13：出站 P2P TLS-over-TCP 預設靜默降級為明文（2026 年 3 月 25 日關閉）

影響：- 啟用 `network.tls_enabled=true` 實際上並未強制僅使用 TLS
  出境運輸，除非運營商也發現並設置
  `tls_fallback_to_plain=false`。
- 因此出站路徑上的任何 TLS 握手失敗或逾時
  預設將撥號降級為純文字 TCP，從而刪除了傳輸
  針對途中攻擊者或不當行為的機密性和完整性
  中間盒。
- 簽署的應用程式握手仍然驗證了對等方身份，因此
  這是傳輸策略的降級，而不是對等欺騙的繞過。

證據：

- `tls_fallback_to_plain` 預設為 `true`
  `crates/iroha_config/src/parameters/user.rs`，因此回退已激活
  除非操作員在配置中明確覆蓋它。
- `crates/iroha_p2p/src/peer.rs` 中的 `Connecting::connect_tcp(...)` 嘗試
  每當設定 `tls_enabled` 時都會進行 TLS 撥號，但在 TLS 錯誤或逾時時
  記錄警告並回退到明文 TCP
  `tls_fallback_to_plain` 已啟用。
- `crates/iroha_kagami/src/wizard.rs` 中操作員為導向的範例設定和
  `docs/source/p2p*.md` 中的公共 P2P 傳輸文件也做了廣告
  明文後備作為預設行為。

為什麼這很重要：- 一旦營運商開啟 TLS，更安全的期望是失敗關閉：如果 TLS
  無法建立會話，撥號應該失敗而不是靜靜地
  脫落運輸保護。
- 預設保留降級會使部署對以下因素敏感
  網路路徑怪癖、代理幹擾和主動握手中斷
  在推出過程中很容易錯過的方式。

推薦：

- 保留純文字後備作為顯式相容性旋鈕，但預設為
  `false` 因此 `network.tls_enabled=true` 表示僅 TLS，除非操作員選擇
  進入降級行為。

整治情況：

- 在目前程式碼中關閉。現在 `crates/iroha_config/src/parameters/user.rs`
  預設 `tls_fallback_to_plain` 至 `false`。
- 預設配置夾具快照、Kagami 範例配置和類似預設配置
  P2P/Torii 測試助手現在鏡像強化的運行時預設值。
- 重複的 `docs/source/p2p*.md` 文件現在將純文字回退描述為
  明確的選擇加入而不是預設的。

### SEC-14：對等遙測地理尋找默默地退回純文字第三方 HTTP（已於 2026 年 3 月 25 日關閉）

影響：- 啟用 `torii.peer_geo.enabled=true` 而不會導致明確端點
  Torii 將對等主機名稱傳送至內建明文
  `http://ip-api.com/json/...` 服務。
- 洩漏的對等遙測目標指向未經身份驗證的第三方 HTTP
  依賴性，並讓任何路徑上的攻擊者或受損的端點源被偽造
  位置元資料返回 Torii。
- 此功能是選擇加入的，但公共遙測文件和範例配置
  公佈了內建預設值，這使得部署模式不安全
  一旦運營商啟用了對等地理查找，就可能會發生這種情況。

證據：

- `crates/iroha_torii/src/telemetry/peers/monitor.rs` 先前定義
  `DEFAULT_GEO_ENDPOINT = "http://ip-api.com/json"` 和
  `construct_geo_query(...)` 每當使用該預設值
  `GeoLookupConfig.endpoint` 是 `None`。
- 每當對等遙測監視器產生 `collect_geo(...)`
  `geo_config.enabled` 為真
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`，所以明文
  回退可以在已發布的運行時程式碼中實現，而不是在僅測試程式碼中實現。
- `crates/iroha_config/src/parameters/defaults.rs` 中的預設配置和
  `crates/iroha_config/src/parameters/user.rs` 保留 `endpoint` 未設置，且
  `docs/source/telemetry*.md` plus 中重複的遙測文檔
  `docs/source/references/peer.template.toml` 明確記錄了
  當操作員啟用此功能時內建後備。

為什麼這很重要：- 對等遙測不應透過純文字 HTTP 默默地傳出對等主機名
  一旦運營商打開便利標誌，就可以向第三方服務提供服務。
- 隱藏的不安全預設設定也會破壞變更審核：操作員可以
  啟用地理查找，而無需意識到他們已經引入了外部
  第三方元資料外洩和未經身份驗證的回應處理。

推薦：

- 刪除預設的內建地理端點。
- 啟用對等地理搜尋時需要明確 HTTPS 端點
  否則跳過查找。
- 集中迴歸證明缺失或非 HTTPS 端點失敗關閉。

整治情況：

- 在目前程式碼中關閉。 `crates/iroha_torii/src/telemetry/peers/monitor.rs`
  現在使用 `MissingEndpoint` 拒絕遺失的端點，拒絕非 HTTPS
  具有 `InsecureEndpoint` 的端點，並跳過對等地理查找而不是
  默默地退回明文內建服務。
- `crates/iroha_config/src/parameters/user.rs` 不再注入隱式
  解析時的端點，因此未設定的配置狀態在所有
  運行時驗證的方式。
- 重複的遙測文件和規格範例配置
  `docs/source/references/peer.template.toml` 現在聲明
  當 `torii.peer_geo.endpoint` 必須明確配置 HTTPS
  功能已啟用。
- 回歸覆蓋範圍現在包括
  `construct_geo_query_requires_explicit_endpoint`，
  `construct_geo_query_rejects_non_https_endpoint`，
  `collect_geo_requires_explicit_endpoint_when_enabled`，和
  `collect_geo_rejects_non_https_endpoint` 中
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`。### SEC-15：SoraFS pin 和網關策略缺乏可信任代理程式感知客戶端 IP 解析（已於 2026 年 3 月 25 日關閉）

影響：

- 反向代理 Torii 部署之前可能會崩潰不同的 SoraFS
  評估儲存引腳 CIDR 時呼叫者存取代理套接字位址
  允許清單、每個客戶端的儲存 pin 限制和網關客戶端
  指紋。
- 削弱了對 `/v1/sorafs/storage/pin` 和 SoraFS 的濫用控制
  透過以下方式在常見的反向代理拓撲中網關下載表面
  多個客戶端共用一個儲存桶或一個白名單身分。
- 預設路由器仍然注入內部遠端元數據，因此這不是一個
  新的未經身份驗證的入口繞過，但這是一個真正的信任邊界差距
  對於代理感知部署和處理程序邊界過度信任
  內部遠端 IP 標頭。

證據：- `crates/iroha_config/src/parameters/user.rs` 和
  `crates/iroha_config/src/parameters/actual.rs` 以前沒有通用
  `torii.transport.trusted_proxy_cidrs` 旋鈕，因此代理感知規範客戶端
  IP 解析度在一般 Torii 入口邊界處無法設定。
- `inject_remote_addr_header(...)` 中的 `crates/iroha_torii/src/lib.rs`
  之前覆蓋了內部 `x-iroha-remote-addr` 標頭
  僅 `ConnectInfo`，它從中刪除了可信任轉發的客戶端 IP 元數據
  真正的反向代理。
- `PinSubmissionPolicy::enforce(...)` 在
  `crates/iroha_torii/src/sorafs/pin.rs` 和
  `gateway_client_fingerprint(...)` 在 `crates/iroha_torii/src/sorafs/api.rs`
  沒有共享可信任代理程式感知的規範 IP 解析步驟
  處理程序邊界。
- `crates/iroha_torii/src/sorafs/pin.rs` 中的儲存引腳節流也已鎖定
  每當存在令牌時，僅在不記名令牌上，這意味著多個
  共享一個有效 pin 令牌的代理客戶端被迫採用相同的費率
  即使在區分了客戶端 IP 後。

為什麼這很重要：

- 反向代理是 Torii 的正常部署模式。如果運行時
  無法一致地區分受信任的代理和不受信任的呼叫者、IP
  允許清單和每個客戶端的限制不再意味著運營商認為的那樣
  意思是。
- SoraFS 接腳和閘道路徑顯然是濫用敏感表面，因此
  將呼叫者折疊到代理 IP 或過度信任過時的轉發
  即使基本路線仍然存在，元資料在操作上也很重要
  需要其他入場。

推薦：- 新增通用 Torii `trusted_proxy_cidrs` 配置表面並解決
  來自 `ConnectInfo` 的規範用戶端 IP 加上任何預先存在的轉發
  僅當套接字對等方位於該允許清單中時才使用標頭。
- 在 SoraFS 處理程序路徑中重複使用規範 IP 解析，而非
  盲目信任內部標頭。
- 透過令牌加上規範客戶端 IP 來限制共享令牌儲存 pin 的範圍
  當兩者都在場時。

整治情況：

- 在目前程式碼中關閉。 `crates/iroha_config/src/parameters/defaults.rs`，
  `crates/iroha_config/src/parameters/user.rs`，和
  `crates/iroha_config/src/parameters/actual.rs`現已曝光
  `torii.transport.trusted_proxy_cidrs`，預設為空列表。
- `crates/iroha_torii/src/lib.rs` 現在解析規範客戶端 IP
  入口中間件內部的 `limits::ingress_remote_ip(...)` 並重寫
  僅來自受信任代理的內部 `x-iroha-remote-addr` 標頭。
- `crates/iroha_torii/src/sorafs/pin.rs` 和
  `crates/iroha_torii/src/sorafs/api.rs` 現在解析規範客戶端 IP
  針對儲存引腳處理程序邊界處的 `state.trusted_proxy_nets`
  策略和網關客戶端指紋識別，因此直接處理程序路徑無法
  過度信任陳舊的轉送 IP 元資料。
- 儲存引腳流現在透過「令牌+規範」來鍵共享不記名令牌
  客戶端 IP`（當兩者都存在時），保留每個客戶端儲存桶以供共享
  pin 令牌。
- 回歸覆蓋範圍現在包括
  `limits::tests::ingress_remote_ip_preserves_trusted_forwarded_header`，
  `limits::tests::ingress_remote_ip_ignores_forwarded_header_from_untrusted_peer`，
  `sorafs::pin::tests::rate_key_scopes_shared_tokens_by_ip`，
  `storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`，
  `car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`，以及
  配置夾具
  `torii_transport_trusted_proxy_cidrs_default_to_empty`。### SEC-16：當注入的遠端 IP 標頭遺失時，操作員驗證鎖定和速率限制鍵控回退到共用匿名儲存桶（已於 2026 年 3 月 25 日關閉）

影響：

- 操作員授權准入已收到接受的套接字 IP，但是
  鎖定/速率限制鍵忽略它並將請求折疊到共享中
  每當內部 `x-iroha-remote-addr` 標頭出現時，`"anon"` 儲存桶
  缺席。
- 這不是預設路由器上的新公共入口旁路，因為
  入口中間件在這些處理程序運作之前重寫內部標頭。
  對於較窄的內部機構來說，這仍然是一個真正的失敗開放式信任邊界差距。
  處理程序路徑、直接測試以及到達的任何未來路由
  注入中間件之前的`OperatorAuth`。
- 在這些情況下，一個呼叫者可能會消耗速率限制預算或鎖定其他呼叫者
  本應透過來源 IP 隔離的呼叫者。

證據：- `OperatorAuth::check_common(...)` 在
  `crates/iroha_torii/src/operator_auth.rs` 已收到
  `remote_ip: Option<IpAddr>`，但以前稱為 `auth_key(headers)`
  並完全放棄了傳輸 IP。
- 之前的 `crates/iroha_torii/src/operator_auth.rs` 中的 `auth_key(...)`
  僅解析 `limits::REMOTE_ADDR_HEADER`，否則回傳 `"anon"`。
- `crates/iroha_torii/src/limits.rs` 中的通用 Torii 助手已經有
  ` effective_remote_ip(headers,
  Remote)`，偏好注入的規範標頭，但又回到
  當直接處理程序呼叫繞過中間件時接受套接字 IP。

為什麼這很重要：

- 鎖定和速率限制狀態必須以相同的有效呼叫者身分為關鍵
  Torii 的其餘部分用於策略決策。回到共享狀態
  匿名儲存桶將缺少的內部元資料躍點轉變為跨客戶端
  幹擾，而不是將影響局限於真正的呼叫者。
- 操作員身份驗證是濫用敏感邊界，因此即使是中等嚴重程度
  桶碰撞問題值得明確關閉。

推薦：

- 從 `limits:: effective_remote_ip(headers,
  remote_ip)` 因此註入的標頭在存在時仍然會獲勝，但是是直接的
  處理程序呼叫回退到傳輸位址而不是 `"anon"`。
- 僅當內部標頭和
  傳輸 IP 不可用。

整治情況：- 在目前程式碼中關閉。 `crates/iroha_torii/src/operator_auth.rs` 現在調用
  `auth_key(headers, remote_ip)` 來自 `check_common(...)` 和 `auth_key(...)`
  現在從以下位置匯出鎖定/速率限制金鑰
  `limits::effective_remote_ip(headers, remote_ip)`。
- 回歸覆蓋範圍現在包括
  `operator_auth_key_uses_remote_ip_when_internal_header_missing` 和
  `operator_auth_key_prefers_injected_header_over_transport_remote_ip` 中
  `crates/iroha_torii/src/operator_auth.rs`。

## 先前報告中的已結束或被取代的調查結果

- 早期原始私鑰 Soracloud 發現：已關閉。目前突變入口
  拒絕內聯 `authority` / `private_key` 字段
  `crates/iroha_torii/src/soracloud.rs:5305-5308`，將 HTTP 簽署者綁定到
  `crates/iroha_torii/src/soracloud.rs:5310-5315` 中的突變來源，以及
  返回草稿交易指令，而不是伺服器提交簽署的交易指令
  交易在 `crates/iroha_torii/src/soracloud.rs:5556-5565` 中。
- 早期僅內部本機讀取代理執行發現：已關閉。公有
  路由解析現在會跳過非公開和更新/私人更新處理程序
  `crates/iroha_torii/src/soracloud.rs:8445-8463`，運行時拒絕
  非公共本地讀取路由
  `crates/irohad/src/soracloud_runtime.rs:5906-5923`。
- 早期公共運行時未計量的後備發現：按書面關閉。公有
  運行時入口現在強制執行速率限制和飛行上限
  `crates/iroha_torii/src/lib.rs:8837-8852` 在解析公共路由之前
  `crates/iroha_torii/src/lib.rs:8858-8860`。
- 早期的遠端 IP 附件租賃發現：已關閉。現已出租附件
  需要經過驗證的登入帳戶
  `crates/iroha_torii/src/lib.rs:7962-7968`。
  附屬租賃先前繼承了 SEC-05 和 SEC-06；那個繼承
  已被上述當前應用程式身份驗證補救措施關閉。## 依賴性發現- `cargo deny check advisories bans sources --hide-inclusion-graph` 現在運行
  直接針對追蹤的 `deny.toml`，現在報告三個即時數據
  從生成的工作區鎖定檔案中發現依賴性。
- `tar` 建議不再出現在活動依賴關係圖中：
  `xtask/src/mochi.rs` 現在使用具有固定參數的 `Command::new("tar")`
  向量，並且 `iroha_crypto` 不再拉動 `libsodium-sys-stable`
  將這些檢查交換到 OpenSSL 後進行 Ed25519 互通測試。
- 目前的發現：
  - `RUSTSEC-2024-0388`：`derivative` 未維護。
  - `RUSTSEC-2024-0436`：`paste` 未維修。
- 影響分類：
  - 先前報告的 `tar` 公告已關閉
    依賴圖。 `cargo tree -p xtask -e normal -i tar`，
    `cargo tree -p iroha_crypto -e all -i tar`，和
    `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` 現在全部失敗
    “套件 ID 規格...與任何套件都不符”，並且
    `cargo deny` 不再報告 `RUSTSEC-2026-0067` 或
    `RUSTSEC-2026-0068`。
  - 先前報告的直接 PQ 更換諮詢現已結束
    目前的樹。 `crates/soranet_pq/Cargo.toml`，
    `crates/iroha_crypto/Cargo.toml` 和 `crates/ivm/Cargo.toml` 現在取決於
    在 `pqcrypto-mldsa` / `pqcrypto-mlkem` 上，以及觸控的 ML-DSA / ML-KEM
    遷移後運行時測試仍然通過。
  - 先前報告的 `rustls-webpki` 通報在以下國家不再有效
    目前的決心。工作區現在將 `reqwest` / `rustls` 固定到修補補丁版本，將 `rustls-webpki` 保留在 `0.103.10` 上，這是
    超出諮詢範圍。
  - `derivative` 和 `paste` 不直接在工作區來源中使用。
    他們透過 BLS / arkworks 堆疊傳遞進入
    `w3f-bls` 和其他幾個上游板條箱，因此刪除它們需要
    上游或依賴堆疊更改而不是本地巨集清理。
    目前的樹現在明確接受這兩個建議
    `deny.toml` 並記錄原因。

## 覆蓋範圍註釋- 服务器/运行时/配置/网络：SEC-05、SEC-06、SEC-07、SEC-08、SEC-09、
  SEC-10、SEC-12、SEC-13、SEC-14、SEC-15 和 SEC-16 在期间得到确认
  審核現已在目前樹中關閉。額外硬化
  当前的树现在也使 Connect websocket/会话准入失败
  当内部注入的远程 IP 标头丢失时关闭，而不是
  將該條件預設為環回。
- IVM/加密/序列化：本次审计没有额外确认的发现
  切片。積極的證據包括機密關鍵材料歸零
  `crates/iroha_crypto/src/confidential.rs:53-60` 和重播感知 Soranet PoW
  `crates/iroha_crypto/src/soranet/pow.rs:823-879` 中的簽名票證驗證。
  后续强化现在还可以拒绝两个畸形的加速器输出
  采样的 Norito 路径：`crates/norito/src/lib.rs` 验证加速 JSON
  `TapeWalker` 取消引用偏移量之前的 Stage-1 磁带现在还需要
  动态加载的 Metal/CUDA Stage-1 帮助程序以证明与
  啟動前的標量結構索引建構器，以及
  `crates/norito/src/core/gpu_zstd.rs` 驗證 GPU 報告的輸出長度
  在截斷編碼/解碼緩衝區之前。 `crates/norito/src/core/simd_crc64.rs`
  現在也可以針對動態載入的 GPU CRC64 幫助程式進行自我測試
  `hardware_crc64` 之前的規範回退會信任它們，因此格式錯誤
  帮助程序库失败关闭，而不是默默更改 Norito 校验和行為。無效的助手結果現在會回退，而不是恐慌釋放
  建立或漂移校驗和奇偶校驗。在IVM側，取樣加速器
  啟動門現在還涵蓋 CUDA Ed25519 `signature_kernel`、CUDA BN254
  add/sub/mul 內核，CUDA `sha256_leaves` / `sha256_pairs_reduce`，即時
  CUDA 向量/AES 批次內核（`vadd64`、`vand`、`vxor`、`vor`、
  `aesenc_batch`、`aesdec_batch`) 以及配套的金屬
  `sha256_leaves`/向量/AES 批次核心在這些路徑受信任之前。的
  採樣金屬 Ed25519 簽名路徑現在也回來了
  在該主機上設定的即時加速器內：較早的奇偶校驗失敗是
  透過恢復標量階梯上的 ref10 肢體綁定標準化來修復，
  重點金屬回歸現在驗證 `[s]B`、`[h](-A)`、
  二的冪基點梯形圖，以及完整的 `[true, false]` 批次驗證
  在 Metal 上相對於 CPU 參考路徑。鏡像的CUDA源發生變化
  在`--features cuda --tests`下編譯，現在CUDA啟動真值集
  如果即時 Merkle 葉/對核心偏離 CPU，則無法關閉
  參考路徑。運行時 CUDA 驗證在此仍受主機限制
  環境。
- SDK/範例：SEC-11 在以傳輸為重點的取樣過程中得到確認
  跨越 Swift、Java/Android、Kotlin 和 JS 用戶端，並且尋找現已在目前樹中關閉。 JS、Swift 和 Android
  規範請求助手也已更新為新的
  新鮮度感知的四標頭方案。
  採樣的 QUIC 串流傳輸評論也沒有產生即時結果
  在目前樹中運行時尋找：`StreamingClient::connect(...)`，
  `StreamingServer::bind(...)`，能力協商助手是
  目前僅透過 `crates/iroha_p2p` 中的測試程式碼進行測試，
  `crates/iroha_core`，因此該幫助程序中的許可自簽名驗證程序
  路徑目前僅用於測試/幫助程序，而不是已發貨的入口表面。
- 範例和移動範例應用程式僅在抽查層級進行審查，並且
  不應被視為經過徹底審核。

## 驗證和覆蓋範圍差距- `cargo deny check advisories bans sources --hide-inclusion-graph` 現在運行
  直接使用追蹤的 `deny.toml`。在當前模式運作下，
  `bans` 和 `sources` 是乾淨的，而 `advisories` 則失敗並顯示五個
  上面列出的依賴性發現。
- 已關閉 `tar` 結果的依賴關係圖清理驗證已通過：
  `cargo tree -p xtask -e normal -i tar`，
  `cargo tree -p iroha_crypto -e all -i tar`，和
  `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` 現在全部報告
  「包 ID 規格...與任何套件都不匹配，」而
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p xtask create_archive_packages_bundle_directory -- --nocapture`，
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p xtask`，
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_verify -- --nocapture`，
  和
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_sign -- --nocapture`
  一切都過去了。
- `bash scripts/fuzz_smoke.sh` 現在透過執行真正的 libFuzzer 會話
  `cargo +nightly fuzz`，但 IVM 的一半腳本未在
  這次通過是因為 `tlv_validate` 的第一個夜間建造仍在
  交接時的進展。該構建已經完成足以執行
  直接產生 libFuzzer 二進位檔案：
  `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
  現在到達 libFuzzer 運行循環，並在運行 200 次後乾淨退出
  空語料庫。修復後，Norito 一半成功完成
  harness/manifest 漂移和 `json_from_json_equiv` 模糊目標編譯
  打破。
- Torii 修復驗證現在包括：
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
  - 較窄的 `--no-default-features --features app_api,app_api_https` Torii
    測試矩陣在 DA / 中仍存在不相關的現有編譯失敗
    Soracloud 門控的 lib-test 程式碼，因此此頻道驗證了已發佈的
    預設功能 MCP 路徑和 `app_api_https` webhook 路徑而不是
    聲稱完全覆蓋最小功能。
- Trusted-proxy/SoraFS 修復驗證現在包括：
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib limits::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib sorafs::pin::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_requires_token_and_respects_allowlist_and_rate_limit -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limits_repeated_clients -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_config --test fixtures torii_transport_trusted_proxy_cidrs_default_to_empty -- --nocapture`
- P2P 修復驗證現在包括：
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib --no-run`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib outgoing_handshake_rejects_unexpected_peer_identity -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib handshake_v1_defaults_to_trust_gossip -- --nocapture`
- SoraFS 本機代理修復驗證現在包括：
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-proxy cargo test -p sorafs_orchestrator spawn_local_quic_proxy_rejects_non_loopback_bind_addr -- --nocapture`
  - `/tmp/iroha-codex-target-proxy/debug/deps/sorafs_orchestrator-b3be10a343598c7b --exact proxy::tests::tcp_stream_bridge_transfers_payload --nocapture`
- P2P TLS 預設修復驗證現在包括：
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-config-tls2 cargo test -p iroha_config tls_fallback_defaults_to_tls_only -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib start_rejects_tls_without_feature_when_tls_only_outbound -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib tls_only_dial_requires_p2p_tls_feature_when_no_fallback -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-connect-gating cargo test -p iroha_torii --test connect_gating --no-run`
- SDK 端修復驗證現在包括：
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
- Norito 後續驗證現在包括：
  - `python3 scripts/check_norito_bindings_sync.py`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture`
  - `bash scripts/fuzz_smoke.sh`（Norito 目標 `json_parse_string`，
    `json_parse_string_ref`、`json_skip_value` 和 `json_from_json_equiv`線束/目標修復後通過）
- IVM 模糊後續驗證現在包括：
  - `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
- IVM 加速器後續驗證現在包括：
  - `xcrun -sdk macosx metal -c crates/ivm/src/metal_ed25519.metal -o /tmp/metal_ed25519.air`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo check -p ivm --features cuda --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_bitwise_single_vector_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_batch_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture`
- 重點 CUDA lib-test 執行在此主機上仍受到環境限制：
  `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo test -p ivm --features cuda --lib selftest_covers_ -- --nocapture`
  仍然無法鏈接，因為 CUDA 驅動程式符號 (`cu*`) 不可用。
- 重點金屬運行時驗證現在完全在加速器上運行
  主機：採樣的 Ed25519 簽章管道在啟動時保持啟用狀態
  自檢，`metal_ed25519_batch_matches_cpu` 驗證 `[true, false]`
  直接在 Metal 上針對 CPU 參考路徑。
- 我沒有重新運行完整的工作區 Rust 測試掃描、完整的 `npm test` 或
  在此修復過程中完整的 Swift/Android 套件。

## 優先修復待辦事項列表

### 下一批- 監控明確接受的傳遞性的上游替換
  `derivative` / `paste` 巨集債務並刪除 `deny.toml` 異常時
  可以在不破壞 BLS / Halo2 / PQ / 穩定性的情況下進行安全升級
  UI 依賴堆疊。
- 在熱緩存上重新運行完整的夜間 IVM fuzz-smoke 腳本，以便
  `tlv_validate` / `kotodama_lower` 旁邊有穩定的記錄結果
  現在綠色的 Norito 目標。直接 `tlv_validate` 二進位運行現已完成，
  但完整的夜間煙霧仍然很出色。
- 在具有 CUDA 驅動程式的主機上重新執行聚焦的 CUDA lib-test 自測試片
  安裝了庫，因此驗證了擴充的 CUDA 啟動真值集
  超越 `cargo check` 和鏡像 Ed25519 標準化修復加上
  新的向量/AES 啟動探針在執行時執行。
- 一旦不相關的套件等級重新運行更廣泛的 JS/Swift/Android/Kotlin 套件
  該分支上的阻止程序已被清除，因此新的規範請求和
  除了上述重點幫助測試之外，還涵蓋了運輸保安人員。
- 決定是否應保留長期應用程式身份驗證多重簽名故事
  失敗關閉或發展一流的 HTTP 多重簽名見證格式。

＃＃＃ 監視器- 繼續重點檢視 `ivm` 硬體加速/不安全路徑
  以及剩餘的 `norito` 流/加密邊界。 JSON 第一階段
  GPU zstd 幫助程式切換現在已強化，可以在以下情況下失敗關閉
  發布版本，採樣的 IVM 加速器啟動真值集現已發布
  更廣泛，但更廣泛的不安全/決定論審查仍然開放。