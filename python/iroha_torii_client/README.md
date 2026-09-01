# Iroha Torii client

Use the public Taira profile instead of copying its origin, address
discriminant, Digital Shekel, and XOR metadata. The deployment's exact
genesis-derived `NetworkId` remains caller-supplied because public resets can
change it:

```python
from iroha_torii_client import (
    TAIRA_TESTNET_PROFILE,
    ToriiClient,
    taira_local_signing_context,
)

client = ToriiClient(
    TAIRA_TESTNET_PROFILE.torii_base_url,
    local_signing_context=taira_local_signing_context(configured_network_id),
    orderbook_chain_discriminant=TAIRA_TESTNET_PROFILE.i105_discriminant,
)
```

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

Use `get_status_snapshot()` for `/status`. That route remains a distinct
operational-health surface; its queue and historical lane telemetry must not be
treated as consensus-authoritative state.

Typed public pipeline metadata keeps every lifecycle kind and both exact read
scopes visible. `PipelineTransactionStatusResponse.is_authoritatively_applied`
is the sole finality-success helper: it is true only for `Applied` with
`scope == "global"` and `resolved_from == "state"`. `Committed`, local results,
and queue/cache observations are not successful finality.
Signed transaction hashes in this status surface use exact
`[0-9a-f]{63}[13579bdf]` text; the final odd nibble is the Iroha `HashOf`
marker, not a normalization option. Contract `tx_hash_hex` receipt fields use
the same exact spelling, as do contract entrypoint hashes, multisig transaction
hashes, and offline-operation status transaction hashes.

`KagemushaTopUpRequestV4` and `KagemushaRedeemRequestV4` decode the exact
embedded authorization archive and derive one immutable six-field operation
identity: operation id, authority digest, canonical request digest, kind,
issuance time, and expiry time. Submission and every status response must match
that complete identity; an exact retry may advance only the active transaction
hash. Callers supply only the canonical Norito request archive and cannot
override any identity field.

## Kagemusha native validation

Applied Kagemusha top-up status validation requires the ABI-23
`connect_norito_bridge` shared library, Kagemusha native contract revision 1,
and its `connect_norito_kagemusha_offline_operation_status_json_validate_v2`
export.
Install the platform bridge artifact where the operating-system dynamic loader
can discover it before starting the Python process. A missing library, a
different ABI, a missing validator symbol, or a non-zero validation result
fails closed; the client has no Python or older-ABI compatibility fallback.

## Caller-trusted unsigned drafts

Contract-call and multisig bytes returned for local signing fail closed unless
the client has the exact genesis-derived `local_signing_context` and the caller
supplies an off-wire `ContractCallDraftIntent` or `MultisigDraftIntent`. Each
contract-call intent contains the exact Norito executable and final merged
metadata archives plus the trusted resolved address, code hash, and request
payload digest:

```python
from iroha_torii_client import ContractCallDraftIntent, contract_payload_digest_hex

call_payload = {"amount": 1}

draft = client.prepare_contract_call(
    authority=authority,
    contract_alias="router::universal",
    entrypoint="increment",
    payload=call_payload,
    metadata={"caller_note": "trusted"},
    creation_time_ms=created_at_ms,
    transaction_ttl_ms=100_000,
    fee_payment=quoted_fee_payment,
    draft_intent=ContractCallDraftIntent(
        executable_b64=trusted_executable_norito_b64,
        metadata_b64=trusted_final_metadata_norito_b64,
        contract_address=trusted_resolved_contract_address,
        code_hash_hex=trusted_contract_code_hash_hex,
        payload_digest_hex=contract_payload_digest_hex(call_payload),
    ),
)
```

These values must come from a trusted local builder and verified artifact/schema
path, never from the Torii response being checked. Payload hashing uses compact,
key-sorted UTF-8 JSON and rejects floats and integers outside the cross-SDK safe
range; encode decimal and wider numeric schema values as canonical strings.
Validation runs before network dispatch where possible, then binds the network,
authority, executable, metadata, response-enriched fee, creation time, TTL,
ordinary admission mode, absent nonce and attachments, and the closed operation
receipt (selector resolution, code/ABI, entrypoint, gas state, and payload
digest) before any bytes are exposed for signing. Generic multisig proposals
apply their archive-binding rule whenever `signature_b64` is absent.

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

The preflight DTO exposes both current IVM cycle limits and validates every fee
account field as an exact canonical I105 account id. Alias-shaped
`name@domain` values are rejected instead of interpreted as account identity.

Each helper generates a fresh signature over the exact `GET`, path, query, and
empty body and dispatches once with redirects and retries disabled. Bearer/API
tokens, canonical-account or witness headers, and precomputed operator headers
are rejected rather than used as fallbacks; session authentication and cookies
are rejected as ambient authority too. The lightweight client has no pipeline
recovery, policy, or proof-retention method; no replacement API is invented for
those absent surfaces. Typed Kaigi responses require Torii's exact fields and
integer spellings, and relay details bind the decoded HPKE key to its advertised
marked fingerprint. Each relay snapshot is streamed through a 64 MiB
post-transfer byte bound, decoded as strict UTF-8 JSON with unique object keys,
and closed on every outcome. Kaigi list and health also require Torii's
canonical ordering and fail closed at the hard relay diagnostic cap rather than
materializing an unbounded registry; the relay SSE handshake remains a separate
streaming protocol.

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
Methods must be ASCII HTTP tokens and signed paths must be the exact
root-relative ASCII wire spelling. `build_canonical_request_headers` first
prepares that target with Requests and signs its `PreparedRequest.path_url`;
the client sends that same prepared request. The pure canonical-message helpers
continue to consume an already exact wire spelling. Operator header builders
and authenticated operator reads use the same prepared-target ownership.
Signer callbacks return 1--3,309 non-zero
signature bytes. The complete `0x` account-header prefix is reserved for
canonical address hex and is never emitted for an alias. Alias headers receive
only a bounded lowercase-ASCII structural preflight; Torii remains authoritative
for UTS-46, active-catalog resolution, and controller verification. The public Python client is signer-only: it neither forwards an
externally constructed `X-Iroha-Witness` nor constructs a typed multisignature
witness end to end.

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

## Atomic private settlement transport

The Python SDK exposes the complete V1 Torii route set without accepting proof
witnesses or audit plaintext. A native wallet or coordinator first produces a
bounded JSON object for one closed operation. Python validates its exact
top-level shape, signs the final route, sends it once with redirects and retries
disabled, and returns an opaque response that can be handed back to native code:

```python
from iroha_torii_client import (
    AtomicPrivateSettlementOperationV1,
    AtomicPrivateSettlementPreparedRequestV1,
)

prepared = AtomicPrivateSettlementPreparedRequestV1.from_native_prepared_json(
    AtomicPrivateSettlementOperationV1.LEG_UPLOAD,
    native_coordinator.prepared_leg_upload_json(),
)
try:
    response = client.upload_private_settlement_leg_v1(
        prepared,
        canonical_auth=sponsor_auth,
    )
    try:
        native_coordinator.accept_torii_response(response.bytes())
    finally:
        response.close()
finally:
    prepared.close()
```

Availability, Prepare, Commit, certificate persistence, leg upload, and global
carrier submission use the sponsor's canonical account signature. Committee
proof reads require the exact validator operator identity; capsule reads and
approval submission require the exact governed auditor identity. Bundle status
and receipt reads are public and expose only the protocol allowlist.

Prepared requests are operation-bound and retained in erasable buffers. Their
representations and all transport errors redact bodies. Restricted responses
remain opaque, bounded, strict UTF-8 JSON; unexpected fields, identifier
substitution, redirects, compressed responses, and noncanonical hash literals
fail closed. The network must still have the governed feature activated and
the audited proof profile available. This SDK surface is not evidence that a
deployment has passed the independent audit or production qualification gates.

## SORA Parliament V1

The account-authenticated Parliament surface is available through strict V1
methods for readiness, attempt drafting and reading, timed-OVN casting context
and proof pages, TLE release context and local partial release, and lifecycle
transition drafting. Draft callers supply the independently derived IDs or
transition digest that the response must match before an instruction is exposed
for signing:

```python
capabilities = client.get_governance_capabilities_v1(canonical_auth=auth)
draft = client.draft_parliament_attempt_v1(
    proposal,
    attempt_sequence=0,
    expected_proposal_content_id=proposal_content_id,
    expected_governance_attempt_id=governance_attempt_id,
    canonical_auth=auth,
)
attempt = client.get_parliament_attempt_v1(
    governance_attempt_id, canonical_auth=auth
)
```

`get_parliament_timed_ovn_casting_proof_page_v1(...)` accepts an independently
trusted nonzero checkpoint height, builds the sole canonical Norito request
frame internally, and returns an opaque, schema-bound Norito response frame.
The lightweight Python package validates media type, schema, flags, checksum,
and the 8 MiB response bound only. Before any ballot seed is used,
pass the response and the independently pinned network ID, checkpoint height,
checkpoint context ID, and ballot-attempt ID to the ABI-23 native verifier.
Python does not claim to verify finality, the ordinary-write witness,
application membership, or the embedded Core archive. Local partial-release
requests are deliberately bodyless and their public response is rebound to a
previously validated release context.

`FreezeTimedOvnCorpus` transition drafts accept one contiguous batch of at
most 32 canonical 2,858-byte records per call; the complete frozen corpus may
still contain up to 1,000 records across calls. Parliament requests reject
ambient session auth headers, cookies, and `Session.auth`, and suppress
Requests' environment/netrc credential fallback during preparation.

The separate `get_parliament_timed_ovn_casting_context_v1(...)` response is a
node-local diagnostic projection, not a finality proof or authorization
capability. Its archive must not reach a secret-local operation unless the
casting-proof response has been verified by the ABI-23 native verifier.

## Signed SoraFS orderbook submission

The lightweight client exposes the three signed orderbook submit routes only
with an explicitly injected native verifier:

```python
from iroha_torii_client import SorafsOrderbookSubmissionAmbiguousError, ToriiClient

client = ToriiClient(
    "https://torii.example",
    orderbook_native_verifier=trusted_native_provider,
    orderbook_chain_discriminant=369,
)
receipt = client.submit_sorafs_orderbook_order(
    signed_transaction_bytes,
    expected_network_id=network_id,
    expected_receipt_signer=torii_receipt_public_key,
)
```

The provider must implement
`inspect_sorafs_orderbook_submission_for_discriminant_v1(...)` and
`verify_sorafs_orderbook_submission_receipt_v1(...)`. Without both, or without
the exact expected network, deployment I105 chain discriminant, and receipt
signer, submission fails before HTTP.
The strict route requires a canonical HTTPS base URL, snapshots an exact stock
`requests.Session`, and constructs its own zero-retry adapter. It sends only
qualified explicit headers/proxies/`verify`/`cert`, ignores `trust_env`, netrc,
environment proxy/CA discovery, hooks, cookie persistence, and Requests elapsed
timing, and rejects custom sessions, ambient cookies, or mutable transport
configuration. Its positive timeout (30 seconds by default) is a Requests
connect/read inactivity timeout, not an absolute deadline: a slow drip may
exceed that wall time.
After dispatch, catch `SorafsOrderbookSubmissionAmbiguousError`, reconcile its
payload-free `expected_identity` against finalized state, and never resubmit
automatically. The full `iroha_python.ToriiClient` supplies this native provider
and derives the expected network from its local signing context.
