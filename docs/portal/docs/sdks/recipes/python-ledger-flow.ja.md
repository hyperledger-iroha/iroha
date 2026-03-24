---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 886c9870bde32342438108521dd2ae09d29f264c4eb36a9744e6036975aeb2f6
source_last_modified: "2025-11-11T10:23:10.384372+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Python 台帳フローレシピ
description: `iroha-python` で登録 → ミント → 転送のフローを開発ネットワーク上で再現する。
slug: /sdks/recipes/python-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

この Python スニペットは [CLI の台帳ウォークスルー](../../norito/ledger-walkthrough.md) と [Rust レシピ](./rust-ledger-flow.md) を反映しています。既定の Docker Compose ネットワークと、`defaults/client.toml` に同梱されたデモ資格情報を利用します。

<SampleDownload
  href="/sdk-recipes/python/ledger_flow.py"
  filename="ledger_flow.py"
  description="このレシピで紹介しているスクリプトをダウンロードし、手動でコードを写さずに実行できます。"
/>

## 前提条件

```bash
pip install iroha-python
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## サンプルスクリプト

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

`python ledger_flow.py` で実行します。出力にはトランザクションハッシュ（レシートペイロード由来）が表示され、その後に受信者の新しい残高が続きます。アセット定義が既に存在する場合、登録命令は拒否されますが、ミントと転送は引き続き成功します。

## パリティの確認

Norito のウォークスルーと同じ CLI コマンドを使ってハッシュと残高を突き合わせます。JavaScript と Rust のレシピを実行すると、3 つの SDK は共有フローのトランザクションハッシュと Norito ペイロードで一致するはずです。
