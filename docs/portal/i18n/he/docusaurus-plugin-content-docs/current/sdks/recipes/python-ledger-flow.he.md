---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sdks/recipes/python-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aca850c94756d5af4a409bed69cd754337e967a32c82429800f93fb6cd41467f
source_last_modified: "2026-01-30T15:02:50+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/python-ledger-flow
title: מתכון זרימת לדג'ר ב-Python
description: שחזור זרימת רישום → הטבעה → העברה מול רשת הפיתוח באמצעות `iroha-python`.
---

import SampleDownload from '@site/src/components/SampleDownload';

קטע ה-Python הזה משקף את [סיור הלדג'ר ב-CLI](../../norito/ledger-walkthrough.md) ואת [מתכון Rust](./rust-ledger-flow.md). הוא משתמש ברשת Docker Compose המוגדרת כברירת מחדל ובפרטי הדמו שמצורפים ב-`defaults/client.toml`.

<SampleDownload
  href="/sdk-recipes/python/ledger_flow.py"
  filename="ledger_flow.py"
  description="הורד את הסקריפט שמוצג במתכון הזה כדי להריץ אותו בלי להעתיק קוד ידנית."
/>

## דרישות מקדימות

```bash
pip install iroha-python
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## סקריפט לדוגמה

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

הריצו `python ledger_flow.py`. הפלט אמור להציג את ההאש של העסקה (מתוך מטען הקבלה) ולאחריו יתרת המקבל החדשה. אם הגדרת הנכס כבר קיימת, הוראת הרישום תידחה בעוד ההטבעה וההעברה ימשיכו להצליח.

## אימות תאימות

השתמשו באותן פקודות CLI מתוך סיור Norito כדי להשוות האשים ויתרות. כשמריצים את מתכוני JavaScript ו-Rust, שלוש ה-SDK אמורות להסכים על האשים של העסקאות ועל payloads Norito עבור הזרימה המשותפת.
