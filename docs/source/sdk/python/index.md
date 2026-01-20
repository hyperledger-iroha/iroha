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
`ToriiClient.get_explorer_account_qr_typed(account_id, address_format="ih58")`
to receive an `ExplorerAccountQrSnapshot`, which includes the canonical account id,
the Norito literal used for the QR payload, the network prefix, error‑correction
setting, module count, QR version, and the inline SVG rendering emitted by Torii.
Passing `address_format="compressed"` mirrors the second-best Sora-only toggle
described in the ADDR‑6b roadmap item; omit it to keep the preferred IH58 output
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
