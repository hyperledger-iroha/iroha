---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Python SDK 快速入門

Python SDK (`iroha-python`) 鏡像 Rust 客戶端幫助程序，因此您可以
從腳本、筆記本或 Web 後端與 Torii 交互。本快速入門
涵蓋安裝、事務提交和事件流。為了更深入
覆蓋範圍請參閱存儲庫中的 `python/iroha_python/README.md`。

## 1.安裝

```bash
pip install iroha-python
```

可選附加功能：

- `pip install aiohttp` 如果您計劃運行異步變體
  流媒體助手。
- 當您需要 SDK 之外的 Ed25519 密鑰派生時，`pip install pynacl`。

## 2. 創建客戶端和簽名者

```python
from iroha_python import (
    ToriiClient,
    derive_ed25519_keypair_from_seed,
)

pair = derive_ed25519_keypair_from_seed(b"demo-seed")  # replace with secure storage
authority = pair.default_account_id("wonderland")

client = ToriiClient(
    torii_url="http://127.0.0.1:8080",
    auth_token="dev-token",  # optional: omit if Torii does not require a token
    telemetry_url="http://127.0.0.1:8080",  # optional
)
```

`ToriiClient` 接受其他關鍵字參數，例如 `timeout_ms`，
`max_retries` 和 `tls_config`。助手 `resolve_torii_client_config`
如果您想與 Rust CLI 進行奇偶校驗，則解析 JSON 配置有效負載。

## 3.提交交易

SDK 附帶了指令構建器和事務幫助器，因此您很少構建
手動 Norito 有效負載：

```python
from iroha_python import Instruction

instruction = Instruction.register_domain("research")

envelope, status = client.build_and_submit_transaction(
    chain_id="local",
    authority=authority,
    private_key=pair.private_key,
    instructions=[instruction],
    wait=True,          # poll until the transaction reaches a terminal status
    fetch_events=True,  # include intermediate pipeline events
)

print("Final status:", status)
```

`build_and_submit_transaction` 返回已簽名的信封和最後一個
觀察到的狀態（例如，`Committed`、`Rejected`）。如果您已經有簽名
交易信封使用 `client.submit_transaction_envelope(envelope)` 或
以 JSON 為中心的 `submit_transaction_json`。

## 4.查詢狀態

所有 REST 端點都有 JSON 幫助器，並且許多公開類型化數據類。對於
例如，列出域：

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

分頁感知助手（例如 `list_accounts_typed`）返回一個對象
包含 `items` 和 `next_cursor`。

當您僅
關心特定資產：

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. 離線津貼

使用離線配額端點頒發錢包證書並註冊
他們在賬本上。 `top_up_offline_allowance` 鏈接問題 + 註冊步驟
（沒有單一充值端點）：

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "i105:...",
    "allowance": {"asset": "7EAD8EFYUx1aVKZPUU1fyKvr8dF1", "amount": "10", "commitment": [1, 2]},
    "spend_public_key": "ed0120deadbeef",
    "attestation_report": [3, 4],
    "issued_at_ms": 100,
    "expires_at_ms": 200,
    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
    "metadata": {},
}

top_up = client.top_up_offline_allowance(
    certificate=draft,
    authority="soraゴヂアヌョシペギゥルゼプキュビルェッハガヌイタソタィニュチョヵボヮゾバュチョナボポビワグツニュノノツマヘサ",
    private_key="operator-private-key",
)
print("registered", top_up.registration.certificate_id_hex)
```

如需續訂，請使用當前證書 ID 調用 `top_up_offline_allowance_renewal`：

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="soraゴヂアヌョシペギゥルゼプキュビルェッハガヌイタソタィニュチョヵボヮゾバュチョナボポビワグツニュノノツマヘサ",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

如果需要分流，請致電 `issue_offline_certificate`（或
`issue_offline_certificate_renewal`) 後跟 `register_offline_allowance`
或 `renew_offline_allowance`。

## 6. 流事件

Torii SSE 端點通過生成器公開。 SDK自動恢復
當 `resume=True` 並且您提供 `EventCursor` 時。

```python
from iroha_python import PipelineEventFilterBox, EventCursor

cursor = EventCursor()

for event in client.stream_pipeline_blocks(
    status="Committed",
    resume=True,
    cursor=cursor,
    with_metadata=True,
):
    print("Block height", event.data.block.height)
```

其他便捷方法包括 `stream_pipeline_transactions`、
`stream_events`（帶有類型化過濾器構建器）和 `stream_verifying_key_events`。

## 7. 後續步驟

- 探索 `python/iroha_python/src/iroha_python/examples/` 下的示例
  用於涵蓋治理、ISO 橋助手和 Connect 的端到端流程。
- 當您想要時使用 `create_torii_client` / `resolve_torii_client_config`
  從 `iroha_config` JSON 文件或環境引導客戶端。
- 對於 Norito RPC 或 Connect 特定的 API，請檢查專用模塊，例如
  `iroha_python.norito_rpc` 和 `iroha_python.connect`。

## 相關 Norito 示例

- [Hajimari 入口點骨架](../norito/examples/hajimari-entrypoint) — 鏡像編譯/運行
  此快速入門中的工作流程，以便您可以從 Python 部署相同的入門合約。
- [註冊域名和鑄造資產](../norito/examples/register-and-mint) — 匹配域名 +
  資產在上面流動，當您想要賬本端實現而不是 SDK 構建器時非常有用。
- [在賬戶之間轉移資產](../norito/examples/transfer-asset) — 展示 `transfer_asset`
  syscall，以便您可以將合約驅動的傳輸與 Python 幫助器方法進行比較。

使用這些構建塊，您可以從 Python 中練習 Torii，而無需編寫
您自己的 HTTP 膠水或 Norito 編解碼器。隨著 SDK 的成熟，額外的高層
將添加建造者；請參閱 `python/iroha_python` 中的自述文件
最新狀態和遷移說明的目錄。