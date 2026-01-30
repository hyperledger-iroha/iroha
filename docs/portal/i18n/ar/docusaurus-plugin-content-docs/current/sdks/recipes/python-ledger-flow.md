---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/python-ledger-flow
title: وصفة تدفق دفتر الأستاذ في Python
description: أعد إنتاج تدفق التسجيل → السك → التحويل على شبكة التطوير باستخدام `iroha-python`.
---

import SampleDownload from '@site/src/components/SampleDownload';

يعكس هذا المقتطف بلغة Python [جولة دفتر الأستاذ عبر CLI](../../norito/ledger-walkthrough.md) و[وصفة Rust](./rust-ledger-flow.md). يستخدم شبكة Docker Compose الافتراضية مع بيانات الاعتماد التجريبية المضمنة في `defaults/client.toml`.

<SampleDownload
  href="/sdk-recipes/python/ledger_flow.py"
  filename="ledger_flow.py"
  description="نزّل السكربت المعروض في هذه الوصفة لتشغيله دون نسخ الكود يدويًا."
/>

## المتطلبات المسبقة

```bash
pip install iroha-python
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## مثال على النص البرمجي

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

شغّل `python ledger_flow.py`. يجب أن تعرض المخرجات تجزئة المعاملة (من حمولة الإيصال) تليها قيمة رصيد المستلم الجديدة. إذا كان تعريف الأصل موجودًا مسبقًا، فسيُرفض أمر التسجيل بينما تستمر عمليتا السك والتحويل في النجاح.

## تحقّق من التكافؤ

استخدم أوامر CLI نفسها من جولة Norito لمطابقة التجزئات والأرصدة. عند تشغيل وصفات JavaScript وRust، ينبغي أن تتفق حزم SDK الثلاث على تجزئات المعاملات وحمولات Norito للتدفق المشترك.
