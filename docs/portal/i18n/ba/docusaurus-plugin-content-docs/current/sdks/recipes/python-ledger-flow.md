---
slug: /sdks/recipes/python-ledger-flow
lang: ba
direction: ltr
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Python ledger flow recipe
description: Reproduce the register → mint → transfer flow against the dev network using `iroha-python`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

импорт SapleDownload '@site/src/компоненттар/SampleDownload';

Был Python өҙөк көҙгө [CLI леджер проходка] (../../norito/ledger-walkthrough.md)
һәм [Рат рецепты] (./rust-ledger-flow.md). Ул Docker ғәҙәттәгесә ҡулланыла
селтәрен төҙөү плюс демо-раҫмалар йыйылмаһы I18NI000000007X.

<СэмплДау-лог
  href="/sdk-рецепттар/питон/леджер_ағым.п".
  файл исеме="аркалы_ағым.п".
  был рецептта күрһәтелгән сценарийҙы скачать итеп, уны ҡул менән күсермәйенсә эшләтмәйенсә эшләй.
/>

## Алдан шарттар

```bash
pip install iroha-python
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Миҫал сценарийы

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

`python ledger_flow.py` менән йүгерә. Сығыш тураһында хәбәр итергә тейеш транзакция хеш
(квитанциянан файҙалы йөктән) яңы приемник балансы килә. Әгәр актив билдәләмәһе инде бар,
регистр инструкцияһы кире ҡағыла, ә мәтрүшкә/тапшырыу уңышҡа өлгәшеүен дауам итә.

## Паритетты раҫлау

Ҡулланыу өсөн шул уҡ CLI командалары I18NT0000000000000Х үткәреү өсөн кросс-тикшерергә хеш һәм
баланс. Ҡасан һеҙ йүгерә JavaScript һәм Rust рецептары, өс SDK тейеш
транзакция хештары һәм I18NT0000000001X файҙалы йөктәр тураһында уртаҡ ағым өсөн килешә.