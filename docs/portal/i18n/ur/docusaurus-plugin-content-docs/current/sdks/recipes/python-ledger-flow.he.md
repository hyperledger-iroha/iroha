---
lang: he
direction: rtl
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/sdks/recipes/python-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f74a8d8c67d5b9d9bec84256196747fbd0ba9aa87f1781d1312ab280de399a6
source_last_modified: "2026-01-30T15:02:50+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/python-ledger-flow
title: پائتھون لیجر فلو ترکیب
description: `iroha-python` کے ذریعے ڈیولپمنٹ نیٹ ورک پر رجسٹر → منٹ → ٹرانسفر فلو دوبارہ چلائیں۔
---

import SampleDownload from '@site/src/components/SampleDownload';

یہ Python اسنیپٹ [CLI لیجر واک تھرو](../../norito/ledger-walkthrough.md) اور [Rust ترکیب](./rust-ledger-flow.md) کی عکاسی کرتا ہے۔ یہ ڈیفالٹ Docker Compose نیٹ ورک اور `defaults/client.toml` میں موجود ڈیمو کریڈینشلز استعمال کرتا ہے۔

<SampleDownload
  href="/sdk-recipes/python/ledger_flow.py"
  filename="ledger_flow.py"
  description="اس ترکیب میں دکھایا گیا اسکرپٹ ڈاؤن لوڈ کریں تاکہ کوڈ ہاتھ سے کاپی کیے بغیر اسے چلا سکیں۔"
/>

## پیشگی تقاضے

```bash
pip install iroha-python
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## مثالی اسکرپٹ

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

`python ledger_flow.py` چلائیں۔ آؤٹ پٹ میں ٹرانزیکشن ہیش (رسید پے لوڈ سے) اور اس کے بعد وصول کنندہ کا نیا بیلنس دکھنا چاہیے۔ اگر اثاثے کی تعریف پہلے سے موجود ہو تو رجسٹر ہدایت رد ہو جائے گی جبکہ منٹنگ اور ٹرانسفر کامیاب رہیں گے۔

## برابری کی تصدیق

Norito walkthrough کے وہی CLI کمانڈز استعمال کریں تاکہ ہیشز اور بیلنسز کا موازنہ ہو سکے۔ جب آپ JavaScript اور Rust کی ترکیبیں چلاتے ہیں تو تینوں SDKs کو مشترکہ فلو کے لیے ٹرانزیکشن ہیشز اور Norito payloads پر متفق ہونا چاہیے۔
