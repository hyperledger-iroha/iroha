---
lang: hy
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

ներմուծել SampleDownload-ից '@site/src/components/SampleDownload';

Python-ի այս հատվածը արտացոլում է [CLI մատյանների անցումը] (../../norito/ledger-walkthrough.md)
և [Rust բաղադրատոմսը] (./rust-ledger-flow.md): Այն օգտագործում է լռելյայն Docker
կազմել ցանց, գումարած `defaults/client.toml`-ում ներառված ցուցադրական հավատարմագրերը:

<Նմուշի ներբեռնում
  href="/sdk-recipes/python/ledger_flow.py"
  filename = "ledger_flow.py"
  description="Ներբեռնեք այս բաղադրատոմսում ցուցադրված սցենարը՝ այն գործարկելու համար՝ առանց ձեռքով կոդը պատճենելու։"
/>

## Նախադրյալներ

```bash
pip install iroha-python
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Օրինակ սցենար

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

Գործարկել `python ledger_flow.py`-ով: Արդյունքը պետք է զեկուցի գործարքի հեշը
(անդորրագրի օգտակար բեռից), որին հաջորդում է նոր ստացողի մնացորդը: Եթե ակտիվի սահմանումն արդեն գոյություն ունի,
գրանցման հրահանգը մերժվում է, մինչդեռ դրամահատարանը/փոխանցումը շարունակվում է հաջողությամբ:

## Ստուգեք հավասարությունը

Օգտագործեք նույն CLI հրամանները Norito միջանցքից՝ խաչաձև ստուգելու հեշերը և
մնացորդներ. Երբ գործարկում եք JavaScript և Rust բաղադրատոմսերը, պետք է բոլոր երեք SDK-ները
պայմանավորվել գործարքի հեշերի և Norito բեռի մասին ընդհանուր հոսքի համար: