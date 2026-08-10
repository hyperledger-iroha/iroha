# Iroha Torii client

The lightweight client exposes authoritative Sumeragi v2 status separately
from the general operational-health endpoint:

```python
from iroha_torii_client import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")
status = client.get_sumeragi_status()

print(status.height, status.view, status.phase)
print(status.height_context.mode, status.height_context.validator_count)
if status.last_commit_qc is not None:
    qc = status.last_commit_qc
    print(qc.signer_count, qc.validator_count, qc.signed_power, qc.total_power)

diagnostics = client.get_sumeragi_diagnostics()
for block in diagnostics.committed_lane_blocks:
    print(block["lane_id"], block["lane_block_height"], block["execution_status"])
```

`get_sumeragi_status()` validates the authoritative JSON projection from
`GET /v1/sumeragi/status`. It rejects non-v2 protocol values, malformed frozen
height contexts, out-of-range leaders, inconsistent or under-quorum CommitQCs,
and malformed liveness state. Its typed `liveness` section exposes exact
partial quorums, durable outbound intents, local-work stages, queue service
debt, the last tracked reducer transition, and any classified delay.

`get_sumeragi_diagnostics()` separately validates
`GET /v1/sumeragi/diagnostics`, including bounded lane evidence, queue
pressure, governance readiness, and Native AMX participant-application
records. Diagnostics are operational evidence and are not consensus authority.

Use `get_status_snapshot()` for `/v1/status`. That route remains a distinct
operational-health surface; its queue and historical lane telemetry must not be
treated as consensus-authoritative state.

## Tenant-scoped ZK attachments

Every attachment upload, list, fetch, and delete is account-authenticated. The
client signs the exact genesis-derived NetworkId, method, percent-encoded path,
query, and body and disables redirects and retries for the one-shot request:

```python
import os

from iroha_torii_client import ToriiCanonicalRequestAuth, ToriiClient

client = ToriiClient("https://torii.example")
auth = ToriiCanonicalRequestAuth(
    network_id=os.environ["IROHA_NETWORK_ID"],
    account_id=authority,
    signer=wallet.sign,
)
meta = client.upload_attachment(
    b"{}", content_type="application/json", canonical_auth=auth
)
items = client.list_attachments(canonical_auth=auth)
payload, content_type = client.get_attachment(meta["id"], canonical_auth=auth)
client.delete_attachment(meta["id"], canonical_auth=auth)
```

Use a fresh nonce per call (the default). A human chain label, foreign genesis
hash, unsigned call, redirect replay, or missing canonical auth is rejected.

## Fee quotes and sponsor programs

Transaction signing is quote-first. Build one complete unsigned payload with a
required typed `fee_payment`, then account-sign the quote request with the same
authority:

```python
import os

from iroha_torii_client import ToriiCanonicalRequestAuth

auth = ToriiCanonicalRequestAuth(
    network_id=os.environ["IROHA_NETWORK_ID"],
    account_id=authority,
    signer=wallet.sign,
)
program = client.get_fee_sponsor_program(
    f"{sponsor_account}/wallet_payments",
    canonical_auth=auth,
)
quote = client.quote_fees(unsigned_payload, canonical_auth=auth)
```

For sponsorship, `unsigned_payload["fee_payment"]` must name the exact program
and non-zero immutable revision. Verify that `quote["intent"]` preserves the
payer, program/revision, and gas bound, replace only that field, then sign and
submit the unchanged payload. The client does not infer a sponsor, reserve a
quote, or fall back to the authority. Legacy transaction metadata keys
`fee_sponsor`, `gas_asset_id`, and `gas_limit` are rejected.
`IROHA_NETWORK_ID` must be the canonical checksummed hash literal generated
from the deployment genesis; a display chain label is never accepted as a
signing domain.
