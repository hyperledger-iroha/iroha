---
slug: /sdks/recipes/python-ledger-flow
lang: zh-hans
direction: ltr
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Python ledger flow recipe
description: Reproduce the register → mint → transfer flow against the dev network using `iroha-python`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

从“@site/src/components/SampleDownload”导入 SampleDownload；

此 Python 代码片段反映了 [CLI 账本演练](../../norito/ledger-walkthrough.md)
以及[生锈配方](./rust-ledger-flow.md)。它使用默认的 Docker
compose 网络以及 `defaults/client.toml` 中捆绑的演示凭证。

<样本下载
  href="/sdk-recipes/python/ledger_flow.py"
  文件名=“ledger_flow.py”
  描述=“下载本菜谱中展示的脚本来运行它，而无需手动复制代码。”
/>

## 先决条件

```bash
pip install iroha-python
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## 示例脚本

```python title="ledger_flow.py"
import os

from iroha_python import (
    Instruction,
    ToriiClient,
    build_transaction,
    submit_transaction,
    query,
)
from iroha_python.model import (
    AssetDefinitionId,
    AssetId,
    ChainId,
    NumericValue,
)

ADMIN_ACCOUNT = os.environ["ADMIN_ACCOUNT"]
RECEIVER_ACCOUNT = os.environ["RECEIVER_ACCOUNT"]
ADMIN_PRIVATE_KEY = os.environ["ADMIN_PRIVATE_KEY"]

client = ToriiClient(
    torii_url="http://127.0.0.1:8080",
)

# 1) Register coffee#wonderland if absent
asset_def = AssetDefinitionId.from_str("coffee#wonderland")
register_instruction = Instruction.register_asset_definition_numeric(asset_def)

# 2) Mint 250 units into admin
admin_asset = AssetId.new(asset_def, ADMIN_ACCOUNT)
mint_instruction = Instruction.mint_asset_numeric(
    value=NumericValue.from_int(250),
    asset_id=admin_asset,
)

# 3) Transfer 50 units to receiver
transfer_instruction = Instruction.transfer_asset_numeric(
    asset_id=admin_asset,
    quantity=NumericValue.from_int(50),
    destination=RECEIVER_ACCOUNT,
)

tx = build_transaction(
    chain=ChainId.from_str("00000000-0000-0000-0000-000000000000"),
    authority=ADMIN_ACCOUNT,
    instructions=[
        register_instruction,
        mint_instruction,
        transfer_instruction,
    ],
)

envelope = tx.sign(ADMIN_PRIVATE_KEY)
receipt = submit_transaction(client, envelope)
if isinstance(receipt, dict):
    hash_ = receipt.get("payload", {}).get("tx_hash")
else:
    hash_ = None
print("Submitted tx:", hash_ or "<pending>")

# 4) Verify receiver balance
result = query.find_account_assets(client, RECEIVER_ACCOUNT)
for asset in result.items:
    if asset.id.definition == asset_def:
        print("Receiver holds", asset.value, "units of", asset.id.definition)
```

使用 `python ledger_flow.py` 运行。输出应报告交易哈希值
（来自收据有效负载），然后是新的接收方余额。如果资产定义已经存在，
注册指令被拒绝，而铸币/传输继续成功。

## 验证奇偶校验

使用 Norito 演练中的相同 CLI 命令来交叉检查哈希值和
余额。当您运行 JavaScript 和 Rust 配方时，所有三个 SDK 都应该
就共享流的交易哈希和 Norito 有效负载达成一致。