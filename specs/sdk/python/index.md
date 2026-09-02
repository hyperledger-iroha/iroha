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
to receive an `ExplorerAccountQrSnapshot`, which includes the canonical I105 account id,
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

assets = client.list_account_assets("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=10)
txs = client.list_account_transactions("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## Offline lifecycle

The first-release HTTP lifecycle consists of exactly four canonical routes:

- `GET /v1/kagemusha/readiness`
- `POST /v1/kagemusha/top-up`
- `POST /v1/kagemusha/redeem`
- `GET /v1/kagemusha/operations/{operation_id}`

POSTs send the typed request directly as canonical Norito and return `202
Accepted` with a typed operation reference and `Location`. They do not accept
JSON or whole-request base64 wrapper objects. The Python surface is
transport-only: it does not install recursive artifacts or claim a native
prover. Top-up requests are limited to 512 KiB and redemption requests to 48
MiB. The capability route is query-free and reports the universal,
asset-neutral application capability. It is not backend readiness and does not
evaluate an asset or dataspace.

```python
from iroha_python import ToriiClient

client = ToriiClient("https://torii.sora.example")
capability = client.get_kagemusha_readiness()
print(
    "offline UI capability",
    capability.ready,
    capability.kagemusha_handoff_capability,
    capability.wire_version,
    capability.device_lifecycle_version,
)
```

The closed response contains exactly
`kagemusha_handoff_capability="kagemusha_handoff_v1"`, wire version `1`, secure-device
lifecycle version `1`, and `ready=True`. It deliberately contains no hop or
history bound. Wallet/device peer handoff must not depend on network
discovery. Missing proof material for a particular online top-up or redemption
rejects that command only; it cannot make a node, asset, or dataspace “not
offline ready.”
