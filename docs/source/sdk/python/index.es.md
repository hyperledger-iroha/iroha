---
lang: es
direction: ltr
source: docs/source/sdk/python/index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 92a224896d82d8c970b83aa867d1dac752b7db3fff8f04dc9ebc8779b59db70d
source_last_modified: "2026-01-30T12:29:45.472189+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Iroha Python SDK

The Python SDK (`iroha-python`) exposes typed Torii helpers, Norito codecs, and
Connect utilities that mirror the Rust data model. These guides focus on the
roadmap work tracked under PY6 (Full Torii & Connect Coverage) so language
owners can ship repeatable workflows, notebooks, and release automation.

```{toctree}
:maxdepth: 1

connect_end_to_end
privacy_admin
release_automation
support_playbook
```

## Explorer helpers

The Torii client now exposes the `/v1/explorer/accounts/{account_id}/qr` route
so wallets and explorers can render canonical account QR codes without re‑implementing
the encoder. Call
`ToriiClient.get_explorer_account_qr_typed(account_id)`
to receive an `ExplorerAccountQrSnapshot`, which includes the canonical Katakana i105 account id,
the Norito literal used for the QR payload, the network prefix, error‑correction
setting, module count, QR version, and the inline SVG rendering emitted by Torii.
described in the ADDR‑6b roadmap item; omit it to keep the preferred i105 output
while still matching the QR payloads used by the JS and Swift SDKs.

## ISO 20022 bridge helpers

`iroha-python` now exposes typed wrappers around Torii’s ISO 20022 bridge. Call
`ToriiClient.submit_iso_pacs008_typed` or `ToriiClient.submit_iso_pacs009_typed` with a raw XML
payload to enqueue payments, then reuse `IsoSubmissionRecord` with
`wait_for_iso_message_status` to block until the message reaches a terminal
state:

```python
from iroha_python import ToriiClient

client = ToriiClient("https://torii.sora.example")
submission = client.submit_iso_pacs008_typed("<Document>...</Document>")
if submission is None:
    raise RuntimeError("bridge returned an empty payload")
status = client.wait_for_iso_message_status(
    submission.message_id,
    resolve_on_accepted=True,
    poll_interval=1.0,
)
print(status.status, status.pacs002_code, status.transaction_hash)
```

`resolve_on_accepted=True` mirrors the CLI/JS behaviour for operators who want to
treat `Accepted` without a transaction hash as success (useful when ledger commits
are delayed but the pacs.002 status must be reported). Bridge responses include
the derived pacs.002 code, optional transaction hash, ledger/asset identifiers,
and reason codes so automation can generate the same evidence bundles described
in :doc:`../../finance/settlement_iso_mapping`.

## Account inventory filters

Use `asset_id` filters on the account inventory helpers to pre-filter holdings,
transactions, and asset holder listings without building a full query envelope:

```python
from iroha_python import ToriiClient

client = ToriiClient("https://torii.sora.example")
asset_id = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"

assets = client.list_account_assets("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=10)
txs = client.list_account_transactions("sorauロ1PaQスGh1エ6pAワnqクfJuソMムVqマvQミレシセヒaネウハc1コハ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## Offline allowances

Use the offline allowance helpers to issue wallet certificates and register them
on-ledger. `top_up_offline_allowance` chains the certificate issue + register
steps and returns both responses. There is no single top-up endpoint; the helper
simply chains the issue + register calls.

```python
from iroha_python import ToriiClient

client = ToriiClient("https://torii.sora.example")

draft = {
    "controller": "<katakana-i105-account-id>",
    "allowance": {"asset": "7EAD8EFYUx1aVKZPUU1fyKvr8dF1", "amount": "10", "commitment": [1, 2]},
    "spend_public_key": "ed0120deadbeef",
    "attestation_report": [3, 4],
    "issued_at_ms": 100,
    "expires_at_ms": 200,
    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
    "metadata": {},
}

top_up = client.top_up_offline_allowance(
    certificate=draft,
    authority="sorauロ1PクCカrムhyワエトhウヤSqP2GFGラヱミケヌマzヘオミMヌヨトksJヱRRJXVB",
    private_key="operator-private-key",
)
print(top_up.registration.certificate_id_hex)
```

For renewals, use `top_up_offline_allowance_renewal` with the existing
`certificate_id_hex`:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="sorauロ1PクCカrムhyワエトhウヤSqP2GFGラヱミケヌマzヘオミMヌヨトksJヱRRJXVB",
    private_key="operator-private-key",
)
print(renewed.registration.certificate_id_hex)
```

If you need to split the flow, call `issue_offline_certificate` (or
`issue_offline_certificate_renewal`) followed by `register_offline_allowance`
or `renew_offline_allowance`.
