---
slug: /sdks/recipes/python-ledger-flow
lang: my
direction: ltr
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Python ledger flow recipe
description: Reproduce the register → mint → transfer flow against the dev network using `iroha-python`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

'@site/src/components/SampleDownload' မှ SampleDownload ကို တင်သွင်းပါ။

ဤ Python အတိုအထွာသည် [CLI လယ်ဂျာ လမ်းညွှန်ချက်] (../../norito/ledger-walkthrough.md) ကို ထင်ဟပ်စေသည်
နှင့် [Rrust recipe](./rust-ledger-flow.md)။ ၎င်းသည် မူရင်း Docker ကို အသုံးပြုသည်။
ကွန်ရက်နှင့် `defaults/client.toml` တွင် စုစည်းထားသော ဒီမိုအထောက်အထားများကို ရေးဖွဲ့ပါ။

<နမူနာဒေါင်းလုဒ်လုပ်ပါ။
  href="/sdk-recipes/python/ledger_flow.py"
  filename="ledger_flow.py"
  description="ကုဒ်ကို လက်ဖြင့်ကူးယူခြင်းမပြုဘဲ ဤစာရွက်တွင် ပြသထားသည့် ဇာတ်ညွှန်းကို ဒေါင်းလုဒ်လုပ်ပါ။"
/>

## လိုအပ်ချက်များ

```bash
pip install iroha-python
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## ဥပမာ ဇာတ်ညွှန်း

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

`python ledger_flow.py` ဖြင့် လုပ်ဆောင်ပါ။ အထွက်သည် ငွေပေးငွေယူ hash ကို အစီရင်ခံသင့်သည်။
(ပြေစာပေးဆောင်မှုမှ) နောက်တွင် လက်ခံသူလက်ကျန်အသစ်ဖြင့်။ ပိုင်ဆိုင်မှု အဓိပ္ပါယ်ဖွင့်ဆိုချက် ရှိနေပြီဆိုလျှင်၊
mint / လွှဲပြောင်းမှုဆက်လက်အောင်မြင်နေချိန်တွင်စာရင်းသွင်းညွှန်ကြားချက်ကိုပယ်ချသည်။

## တူညီမှုကို အတည်ပြုပါ။

Cross-check hash များနှင့် Norito မှ တူညီသော CLI command များကို အသုံးပြုပါ။
ခွင်. JavaScript နှင့် Rust ချက်ပြုတ်နည်းများကို သင်လုပ်ဆောင်သောအခါ၊ SDK သုံးခုစလုံးသည် ဖြစ်သင့်သည်။
မျှဝေထားသောစီးဆင်းမှုအတွက် ငွေပေးငွေယူ hashes နှင့် Norito payloads များကို သဘောတူပါသည်။