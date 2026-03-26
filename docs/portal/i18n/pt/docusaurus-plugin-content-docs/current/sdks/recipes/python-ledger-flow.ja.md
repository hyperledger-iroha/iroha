---
lang: ja
direction: ltr
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sdks/recipes/python-ledger-flow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf2a5a1118bb21e2e5be6130c71e7038022161075690c77e40e8aeb1f7d5d69b
source_last_modified: "2026-01-30T15:02:50+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: pt
direction: ltr
source: docs/portal/docs/sdks/recipes/python-ledger-flow.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /sdks/recipes/python-ledger-flow
title: Receita de fluxo do ledger em Python
description: Reproduza o fluxo registrar → cunhar → transferir na rede de desenvolvimento com `iroha-python`.
---

import SampleDownload from '@site/src/components/SampleDownload';

Este trecho em Python espelha o [passo a passo do ledger na CLI](../../norito/ledger-walkthrough.md) e a [receita em Rust](./rust-ledger-flow.md). Ele usa a rede padrão do Docker Compose e as credenciais de demo incluídas em `defaults/client.toml`.

<SampleDownload
  href="/sdk-recipes/python/ledger_flow.py"
  filename="ledger_flow.py"
  description="Baixe o script apresentado nesta receita para executá-lo sem copiar o código manualmente."
/>

## Pré-requisitos

```bash
pip install iroha-python
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
export ADMIN_PRIVATE_KEY="802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
```

## Script de exemplo

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

Execute `python ledger_flow.py`. A saída deve mostrar o hash da transação (do payload do recibo) seguido do novo saldo do destinatário. Se a definição do ativo já existir, a instrução de registro é rejeitada enquanto a cunhagem e a transferência continuam a funcionar.

## Verifique a paridade

Use os mesmos comandos da CLI do walkthrough Norito para cruzar hashes e saldos. Ao executar as receitas de JavaScript e Rust, os três SDKs devem concordar nos hashes de transação e nos payloads Norito para o fluxo compartilhado.
