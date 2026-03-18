---
slug: /sdks/recipes/python-ledger-flow
lang: mn
direction: ltr
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Python ledger flow recipe
description: Reproduce the register → mint → transfer flow against the dev network using `iroha-python`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SampleDownload-г '@site/src/components/SampleDownload'-аас импортлох;

Энэхүү Python хэсэг нь [CLI дэвтэрийн дэлгэрэнгүй танилцуулгыг](../../norito/ledger-walkthrough.md) толилуулж байна.
болон [Зэвний жор](./rust-ledger-flow.md). Энэ нь анхдагч Docker-г ашигладаг
сүлжээг үүсгэх, мөн `defaults/client.toml`-д багцлагдсан демо итгэмжлэлүүд.

<Жишээ татаж авах
  href="/sdk-recipes/python/ledger_flow.py"
  файлын нэр = "ledger_flow.py"
  description="Кодыг гараар хуулахгүйгээр ажиллуулахын тулд энэ жоронд үзүүлсэн скриптийг татаж авна уу."
/>

## Урьдчилсан нөхцөл

```bash
pip install iroha-python
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Жишээ скрипт

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

`python ledger_flow.py` ашиглан ажиллуулна уу. Гаралт нь гүйлгээний хэшийг мэдээлэх ёстой
(хүлээн авалтын ачааллаас) дараа нь хүлээн авагчийн шинэ үлдэгдэл. Хэрэв хөрөнгийн тодорхойлолт аль хэдийн байгаа бол,
гаа/шилжилт амжилттай үргэлжилж байхад бүртгэлийн заавар татгалзсан байна.

## Паритетийг баталгаажуулна уу

Norito гарын авлагын ижил CLI тушаалуудыг ашиглан хэш болон хөндлөн шалгах
тэнцэл. Та JavaScript болон Rust жоруудыг ажиллуулахад гурван SDK бүгд ажиллах ёстой
хуваалцсан урсгалын хувьд гүйлгээний хэш болон Norito ачааллын талаар тохиролцоно.