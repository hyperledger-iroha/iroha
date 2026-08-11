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

## Node-local core and pipeline reads

Peer addresses, detailed clock state, and pipeline preflight load/policy are
operator-only. Configure a separate lightweight client with an immutable signer
bound to the deployment's exact genesis `NetworkId`:

```python
from iroha_torii_client import ToriiClient, ToriiOperatorSigningContext

operator_context = ToriiOperatorSigningContext(
    network_id=exact_genesis_network_id,
    public_key=operator_public_key_multihash,
    signer=operator_signer.sign,
)
operator_client = ToriiClient(
    "https://torii.example",
    operator_signing_context=operator_context,
)

peers = operator_client.list_peers()
clock = operator_client.get_time_status()
preflight = operator_client.get_pipeline_preflight()

relays = operator_client.list_kaigi_relays()
relay = operator_client.get_kaigi_relay(relays.items[0].relay_id) if relays.items else None
health = operator_client.get_kaigi_relays_health()
```

Each helper generates a fresh signature over the exact `GET`, path, query, and
empty body and dispatches once with redirects and retries disabled. Bearer/API
tokens, canonical-account or witness headers, and precomputed operator headers
are rejected rather than used as fallbacks. The lightweight client has no
pipeline-recovery, policy, or proof-retention method; no replacement API is
invented for those absent surfaces. Kaigi list and health also fail closed at
Torii's hard relay diagnostic cap rather than materializing an unbounded
registry; the relay SSE handshake remains a separate streaming protocol.

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

Space Directory publish/revoke drafts follow the same contract and additionally
require the exact canonical I105 payload authority to equal `auth.account_id`:

```python
from iroha_torii_client import ToriiClient, ToriiLocalSigningContext

client = ToriiClient(
    torii_url,
    local_signing_context=ToriiLocalSigningContext(exact_network_id),
)
draft = client.publish_space_directory_manifest(
    authority=authority,
    manifest=manifest,
    canonical_auth=auth,
)
client.revoke_space_directory_manifest(
    authority=authority,
    uaid=uaid,
    dataspace=11,
    revoked_epoch=42,
    canonical_auth=auth,
)
```

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
