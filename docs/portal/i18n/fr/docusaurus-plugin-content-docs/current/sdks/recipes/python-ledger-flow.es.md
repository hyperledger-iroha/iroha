---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/recipes/python-ledger-flow.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: es
direction: ltr
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 886c9870bde32342438108521dd2ae09d29f264c4eb36a9744e6036975aeb2f6
source_last_modified: "2025-11-11T10:23:10.384372+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Receta de flujo del libro mayor en Python
description: Reproduce el flujo registrar → acuñar → transferir contra la red de desarrollo usando `iroha-python`.
slug: /sdks/recipes/python-ledger-flow
---

import SampleDownload from '@site/src/components/SampleDownload';

Este fragmento de Python refleja el [recorrido del libro mayor de la CLI](../../norito/ledger-walkthrough.md) y la [receta de Rust](./rust-ledger-flow.md). Usa la red por defecto de Docker Compose junto con las credenciales de demo incluidas en `defaults/client.toml`.

<SampleDownload
  href="/sdk-recipes/python/ledger_flow.py"
  filename="ledger_flow.py"
  description="Descarga el script mostrado en esta receta para ejecutarlo sin copiar el código a mano."
/>

## Prerequisitos

```bash
pip install iroha-python
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Script de ejemplo

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

Ejecuta `python ledger_flow.py`. La salida debe informar el hash de la transacción (del payload del recibo) seguido del nuevo saldo del receptor. Si la definición del activo ya existe, la instrucción de registro se rechaza mientras la acuñación y la transferencia siguen teniendo éxito.

## Verifica la paridad

Usa los mismos comandos de la CLI del recorrido de Norito para contrastar hashes y saldos. Cuando ejecutes las recetas de JavaScript y Rust, los tres SDK deben coincidir en los hashes de transacción y los payloads Norito para el flujo compartido.
