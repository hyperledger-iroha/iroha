---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55e3fee1f354fa023ec258958b25c01ebda75a35bafdaa049a82f3ca73dab017
source_last_modified: "2026-01-22T16:26:46.513338+00:00"
translation_last_reviewed: 2026-02-07
title: Python ledger flow recipe
description: Reproduce the register → mint → transfer flow against the dev network using `iroha-python`.
slug: /sdks/recipes/python-ledger-flow
translator: machine-google-reviewed
---

從“@site/src/components/SampleDownload”導入 SampleDownload；

此 Python 代碼片段反映了 [CLI 賬本演練](../../norito/ledger-walkthrough.md)
以及[生鏽配方](./rust-ledger-flow.md)。它使用默認的 Docker
compose 網絡以及 `defaults/client.toml` 中捆綁的演示憑證。

<樣本下載
  href="/sdk-recipes/python/ledger_flow.py"
  文件名=“ledger_flow.py”
  描述=“下載本菜譜中展示的腳本來運行它，而無需手動複製代碼。”
/>

## 先決條件

```bash
pip install iroha-python
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## 示例腳本

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

# 1) Register 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ if absent
asset_def = AssetDefinitionId.from_str("7Sp2j6zDvJFnMoscAiMaWbWHRDBZ")
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

使用 `python ledger_flow.py` 運行。輸出應報告交易哈希值
（來自收據有效負載），然後是新的接收方餘額。如果資產定義已經存在，
註冊指令被拒絕，而鑄幣/傳輸繼續成功。

## 驗證奇偶校驗

使用 Norito 演練中的相同 CLI 命令來交叉檢查哈希值和
餘額。當您運行 JavaScript 和 Rust 配方時，所有三個 SDK 都應該
就共享流的交易哈希和 Norito 有效負載達成一致。