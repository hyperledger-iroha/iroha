---
lang: zh-hans
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2025-12-29T18:16:35.962003+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 来自值得信赖的同行的 Genesis Bootstrap

没有本地 `genesis.file` 的 Iroha 对等点可以从受信任的对等点获取签名的创世块
使用 Norito 编码的引导协议。

- **协议：**对等方交换 `GenesisRequest`（`Preflight` 用于元数据，`Fetch` 用于有效负载）和
  由 `request_id` 键控的 `GenesisResponse` 帧。响应者包括链 ID、签名者公钥、
  哈希值和可选的大小提示；仅在 `Fetch` 上返回有效负载，并且重复的请求 ID
  接收 `DuplicateRequest`。
- **守卫：** 响应者强制执行白名单（`genesis.bootstrap_allowlist` 或受信任的对等方
  设置）、链 ID/公钥/哈希匹配、速率限制 (`genesis.bootstrap_response_throttle`) 和
  尺寸上限 (`genesis.bootstrap_max_bytes`)。允许列表之外的请求收到 `NotAllowed`，并且
  由错误密钥签名的有效负载收到 `MismatchedPubkey`。
- **请求者流程：** 当存储为空并且 `genesis.file` 未设置时（并且
  `genesis.bootstrap_enabled=true`)，节点使用可选的选项预检受信任的对等点
  `genesis.expected_hash`，然后获取有效负载，通过`validate_genesis_block`验证签名，
  并在应用该块之前将 `genesis.bootstrap.nrt` 与 Kura 一起保留。引导重试
  荣誉 `genesis.bootstrap_request_timeout`、`genesis.bootstrap_retry_interval` 和
  `genesis.bootstrap_max_attempts`。
- **失败模式：** 由于白名单未命中、链/公钥/哈希不匹配、大小而拒绝请求
  违反上限、速率限制、缺少本地起源或重复的请求 ID。冲突的哈希值
  跨对等方中止获取；没有响应者/超时回退到本地配置。
- **操作员步骤：** 确保至少有一个受信任的对等点可以通过有效的创世到达，配置
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` 和重试旋钮，以及
  可选择引脚 `expected_hash` 以避免接受不匹配的有效负载。持久有效负载可以是
  通过将 `genesis.file` 指向 `genesis.bootstrap.nrt` 在后续引导中重用。