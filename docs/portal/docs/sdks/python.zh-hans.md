---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f2dd6b790ce0252c355db5218b64ca9a15f4200879fe874499df079ae168872
source_last_modified: "2026-01-30T18:06:01.646084+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Python SDK 快速入门

Python SDK (`iroha-python`) 镜像 Rust 客户端帮助程序，因此您可以
从脚本、笔记本或 Web 后端与 Torii 交互。本快速入门
涵盖安装、事务提交和事件流。为了更深入
覆盖范围请参阅存储库中的 `python/iroha_python/README.md`。

## 1.安装

```bash
pip install iroha-python
```

可选附加功能：

- `pip install aiohttp` 如果您计划运行异步变体
  流媒体助手。
- 当您需要 SDK 之外的 Ed25519 密钥派生时，`pip install pynacl`。

## 2. 创建客户端和签名者

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

`ToriiClient` 接受其他关键字参数，例如 `timeout_ms`，
`max_retries` 和 `tls_config`。助手 `resolve_torii_client_config`
如果您想与 Rust CLI 进行奇偶校验，则解析 JSON 配置有效负载。

## 3.提交交易

SDK 附带了指令构建器和事务帮助器，因此您很少构建
手动 Norito 有效负载：

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

`build_and_submit_transaction` 返回已签名的信封和最后一个
观察到的状态（例如，`Committed`、`Rejected`）。如果您已经有签名
交易信封使用 `client.submit_transaction_envelope(envelope)` 或
以 JSON 为中心的 `submit_transaction_json`。

## 4.查询状态

所有 REST 端点都有 JSON 帮助器，并且许多公开类型化数据类。对于
例如，列出域：

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

分页感知助手（例如 `list_accounts_typed`）返回一个对象
包含 `items` 和 `next_cursor`。

当您仅
关心特定资产：

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("rose#wonderland", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. 离线津贴

使用离线配额端点颁发钱包证书并注册
他们在账本上。 `top_up_offline_allowance` 链接问题 + 注册步骤
（没有单一充值端点）：

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "i105:...",
    "allowance": {"asset": "usd#wonderland", "amount": "10", "commitment": [1, 2]},
    "spend_public_key": "ed0120deadbeef",
    "attestation_report": [3, 4],
    "issued_at_ms": 100,
    "expires_at_ms": 200,
    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
    "metadata": {},
}

top_up = client.top_up_offline_allowance(
    certificate=draft,
    authority="6cmzPVPX96RC3GJu43xurPoaAiQUx89nVpPgB63M62fpMZ2WibN7DuZ",
    private_key="operator-private-key",
)
print("registered", top_up.registration.certificate_id_hex)
```

如需续订，请使用当前证书 ID 调用 `top_up_offline_allowance_renewal`：

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="6cmzPVPX96RC3GJu43xurPoaAiQUx89nVpPgB63M62fpMZ2WibN7DuZ",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

如果需要分流，请致电 `issue_offline_certificate`（或
`issue_offline_certificate_renewal`) 后跟 `register_offline_allowance`
或 `renew_offline_allowance`。

## 6. 流事件

Torii SSE 端点通过生成器公开。 SDK自动恢复
当 `resume=True` 并且您提供 `EventCursor` 时。

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
`stream_events`（带有类型化过滤器构建器）和 `stream_verifying_key_events`。

## 7. 后续步骤

- 探索 `python/iroha_python/src/iroha_python/examples/` 下的示例
  用于涵盖治理、ISO 桥助手和 Connect 的端到端流程。
- 当您想要时使用 `create_torii_client` / `resolve_torii_client_config`
  从 `iroha_config` JSON 文件或环境引导客户端。
- 对于 Norito RPC 或 Connect 特定的 API，请检查专用模块，例如
  `iroha_python.norito_rpc` 和 `iroha_python.connect`。

## 相关 Norito 示例

- [Hajimari 入口点骨架](../norito/examples/hajimari-entrypoint) — 镜像编译/运行
  此快速入门中的工作流程，以便您可以从 Python 部署相同的入门合约。
- [注册域名和铸造资产](../norito/examples/register-and-mint) — 匹配域名 +
  资产在上面流动，当您想要账本端实现而不是 SDK 构建器时非常有用。
- [在账户之间转移资产](../norito/examples/transfer-asset) — 展示 `transfer_asset`
  syscall，以便您可以将合约驱动的传输与 Python 帮助器方法进行比较。

使用这些构建块，您可以从 Python 中练习 Torii，而无需编写
您自己的 HTTP 胶水或 Norito 编解码器。随着 SDK 的成熟，额外的高层
将添加建造者；请参阅 `python/iroha_python` 中的自述文件
最新状态和迁移说明的目录。