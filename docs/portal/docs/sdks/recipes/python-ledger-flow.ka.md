---
lang: ka
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

SampleDownload-ის იმპორტი '@site/src/components/SampleDownload'-დან;

პითონის ეს ფრაგმენტი ასახავს [CLI ledger walkthrough] (../../norito/ledger-walkthrough.md)
და [Rust რეცეპტი] (./rust-ledger-flow.md). ის იყენებს ნაგულისხმევ Docker-ს
შეადგინეთ ქსელი, პლუს დემო სერთიფიკატები, რომლებიც შეფუთულია `defaults/client.toml`-ში.

<SampleDownload
  href="/sdk-recipes/python/ledger_flow.py"
  ფაილის სახელი = "ledger_flow.py"
  description="ჩამოტვირთეთ ამ რეცეპტში ნაჩვენები სკრიპტი, რომ გაუშვათ კოდი ხელით კოპირების გარეშე."
/>

## წინაპირობები

```bash
pip install iroha-python
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## მაგალითი სკრიპტი

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

იმოძრავეთ `python ledger_flow.py`-ით. გამომავალმა უნდა აცნობოს ტრანზაქციის ჰეშის შესახებ
(ქვითრის დატვირთვიდან) მოჰყვება ახალი მიმღების ბალანსი. თუ აქტივის განმარტება უკვე არსებობს,
რეესტრის ინსტრუქცია უარყოფილია, სანამ ზარაფხანა/გადაცემა გრძელდება.

## დაადასტურეთ პარიტეტი

გამოიყენეთ იგივე CLI ბრძანებები Norito-დან ჰეშების გადასამოწმებლად და
ნაშთები. JavaScript-ისა და Rust-ის რეცეპტების გაშვებისას, სამივე SDK უნდა
შეთანხმდნენ ტრანზაქციის ჰეშებზე და Norito დატვირთვაზე საერთო ნაკადისთვის.