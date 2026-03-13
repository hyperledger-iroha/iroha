---
lang: zh-hans
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 客户端API配置参考

本文档跟踪 Torii 面向客户端的配置旋钮，这些旋钮
通过 `iroha_config::parameters::user::Torii` 的表面。下面的部分
重点关注为 NRPC-1 引入的 Norito-RPC 传输控制；未来
客户端 API 设置应扩展此文件。

### `torii.transport.norito_rpc`

|关键|类型 |默认 |描述 |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` |启用二进制 Norito 解码的主开关。当 `false` 时，Torii 使用 `403 norito_rpc_disabled` 拒绝每个 Norito-RPC 请求。 |
| `stage` | `string` | `"disabled"` |推出层：`disabled`、`canary` 或 `ga`。阶段驱动准入决策和 `/rpc/capabilities` 输出。 |
| `require_mtls` | `bool` | `false` |对 Norito-RPC 传输强制执行 mTLS：当 `true` 时，Torii 拒绝不携带 mTLS 标记标头（例如 `X-Forwarded-Client-Cert`）的 Norito-RPC 请求。该标志通过 `/rpc/capabilities` 出现，因此 SDK 可以对配置错误的环境发出警告。 |
| `allowed_clients` | `array<string>` | `[]` |金丝雀白名单。当 `stage = "canary"` 时，仅接受携带此列表中存在的 `X-API-Token` 标头的请求。 |

配置示例：

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

阶段语义：

- **禁用** — 即使 `enabled = true`，Norito-RPC 也不可用。客户
  接收 `403 norito_rpc_disabled`。
- **canary** — 请求必须包含与一个匹配的 `X-API-Token` 标头
  `allowed_clients` 的。所有其他请求都会收到`403
  Norito_rpc_canary_denied`。
- **ga** — Norito-RPC 可用于每个经过身份验证的调用者（取决于
  通常的速率和预授权限制）。

操作员可以通过 `/v2/config` 动态更新这些值。每一次改变
立即反映在 `/rpc/capabilities` 中，允许 SDK 和可观察性
仪表板显示实时运输姿态。