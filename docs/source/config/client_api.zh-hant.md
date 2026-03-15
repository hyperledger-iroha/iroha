---
lang: zh-hant
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2025-12-29T18:16:35.933651+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 客戶端API配置參考

本文檔跟踪 Torii 面向客戶端的配置旋鈕，這些旋鈕
通過 `iroha_config::parameters::user::Torii` 的表面。下面的部分
重點關注為 NRPC-1 引入的 Norito-RPC 傳輸控制；未來
客戶端 API 設置應擴展此文件。

### `torii.transport.norito_rpc`

|關鍵|類型 |默認 |描述 |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` |啟用二進制 Norito 解碼的主開關。當 `false` 時，Torii 使用 `403 norito_rpc_disabled` 拒絕每個 Norito-RPC 請求。 |
| `stage` | `string` | `"disabled"` |推出層：`disabled`、`canary` 或 `ga`。階段驅動准入決策和 `/rpc/capabilities` 輸出。 |
| `require_mtls` | `bool` | `false` |對 Norito-RPC 傳輸強制執行 mTLS：當 `true` 時，Torii 拒絕不攜帶 mTLS 標記標頭（例如 `X-Forwarded-Client-Cert`）的 Norito-RPC 請求。該標誌通過 `/rpc/capabilities` 出現，因此 SDK 可以對配置錯誤的環境發出警告。 |
| `allowed_clients` | `array<string>` | `[]` |金絲雀白名單。當 `stage = "canary"` 時，僅接受攜帶此列表中存在的 `X-API-Token` 標頭的請求。 |

配置示例：

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

階段語義：

- **禁用** — 即使 `enabled = true`，Norito-RPC 也不可用。客戶
  接收 `403 norito_rpc_disabled`。
- **canary** — 請求必須包含與一個匹配的 `X-API-Token` 標頭
  `allowed_clients` 的。所有其他請求都會收到`403
  Norito_rpc_canary_denied`。
- **ga** — Norito-RPC 可用於每個經過身份驗證的調用者（取決於
  通常的速率和預授權限制）。

操作員可以通過 `/v1/config` 動態更新這些值。每一次改變
立即反映在 `/rpc/capabilities` 中，允許 SDK 和可觀察性
儀表板顯示實時運輸姿態。