# Python SDK Quickstart

The Python SDK (`iroha-python`) mirrors the Rust client helpers so you can
interact with Torii from scripts, notebooks, or web backends. This quickstart
covers installation, transaction submission, and event streaming. For deeper
coverage see `python/iroha_python/README.md` in the repository.

For SORA Nexus wallet-approved app transfers, import
`NexusAppClient` from `iroha_python.nexus_app`; see
[Nexus App Facade](./nexus-app-facade).

## 1. Install

```bash
pip install iroha-python
```

Optional extras:

- `pip install aiohttp` if you plan to run the asynchronous variants of the
  streaming helpers.
- `pip install pynacl` when you need Ed25519 key derivation outside of the SDK.

## 2. Create a client and signers

```python
from iroha_python import (
    ToriiClient,
    derive_ed25519_keypair_from_seed,
)

pair = derive_ed25519_keypair_from_seed(b"demo-seed")  # replace with secure storage
authority = pair.default_account_id("wonderland")

client = ToriiClient(
    torii_url="http://127.0.0.1:8080",
    auth_token="dev-token",  # optional: omit if Torii does not require a token
    telemetry_url="http://127.0.0.1:8080",  # optional
)
```

`ToriiClient` accepts additional keyword arguments such as `timeout_ms`,
`max_retries`, and `tls_config`. The helper `resolve_torii_client_config`
parses a JSON configuration payload if you want parity with the Rust CLI.

## 3. Submit a transaction

The SDK ships instruction builders and transaction helpers so you rarely build
Norito payloads by hand:

```python
from iroha_python import Instruction

instruction = Instruction.register_domain("research")

envelope, status = client.build_and_submit_transaction(
    chain_id="local",
    authority=authority,
    private_key=pair.private_key,
    instructions=[instruction],
    wait=True,          # poll until the transaction reaches a terminal status
    fetch_events=True,  # include intermediate pipeline events
)

print("Final status:", status)
```

`build_and_submit_transaction` returns both the signed envelope and the last
observed status (e.g., `Committed`, `Rejected`). If you already have a signed
transaction envelope use `client.submit_transaction_envelope(envelope)` or the
JSON-centric `submit_transaction_json`.

## 4. Query state

All REST endpoints have JSON helpers and many expose typed dataclasses. For
example, listing domains:

```python
domains = client.list_domains_typed()
for domain in domains.items:
    print(domain.name)
```

Pagination-aware helpers (e.g., `list_accounts_typed`) return an object that
contains both `items` and `next_cursor`.

Account inventory helpers accept an optional `asset_id` filter when you only
care about a specific asset:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Offline readiness

Readiness is evaluated for one exact asset definition. Pass its canonical ID to
`get_kagemusha_readiness`; for example, the call below sends
`GET /v1/offline/readiness?asset_definition_id=xor%23wonderland`. A successful
response sets `ready` to `True` exactly when its typed `blockers` tuple is empty.
An unmet requirement is a normal `ready == False` result; HTTP `503` is reserved
for an evaluation failure.

The same transport-only client submits canonical Norito archives through the
unchanged `POST /v1/offline/top-up` and `POST /v1/offline/redeem` routes and
polls `GET /v1/offline/operations/{operation_id}`. It does not install recursive
artifacts or claim a native prover. Top-up requests are limited to 512 KiB and
redemption requests to 48 MiB; JSON and whole-request base64 wrappers are not
accepted.

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")
readiness = client.get_kagemusha_readiness(asset_definition_id="xor#wonderland")
print(
    "offline ready",
    readiness.ready,
    readiness.required_bridge_abi_version,
    readiness.active_transfer_verifier,
    readiness.active_topup_shield_verifier,
    readiness.active_unshield_verifier,
    readiness.active_recursive_step_eq_verifier,
    readiness.active_recursive_step_ep_verifier,
    readiness.artifact_set,
    readiness.proof_backend_available,
    readiness.recursive_lineage_supported,
    readiness.blockers,
)
```

All five verifier fields are required nullable snapshots for distinct roles.
Each is null exactly with its matching unavailable blocker. Readiness requires
exact bridge ABI 21. The recursive records use the exact logical V4 roles
`kagemusha_recursive_step_eq_v4_verifier_record` and
`kagemusha_recursive_step_ep_v4_verifier_record`, with circuits
`kagemusha-recursive-spend-step-eq-compact-layout-v5` and
`kagemusha-recursive-spend-step-ep-compact-lineage-v5` respectively, and
backend `halo2/ipa`.

`artifact_set` is required but nullable. When present, it binds the
authenticated V4 generation, manifest, release-policy and release-attestation
digests, issuance window, proof-pair bound, and asset scale to both recursive
verifier records. A null value requires both recursive records and backend
construction to be unavailable with exactly one
`recursive_v4_registry_unavailable` or `recursive_v4_registry_malformed`
blocker; a non-null value forbids both. It may coexist with
`proof_backend_available=True`, which means that the exact authenticated
backend was constructed. `recursive_lineage_supported` additionally requires
the authenticated artifact set and distinct active Eq/Ep records;
`recursive_lineage_unavailable` is present exactly when that conjunction is
false. `ready` is true only when the complete blocker set is empty, so unrelated
blockers do not erase valid backend or lineage facts. The client rejects
inconsistent combinations. Kagemusha is the only offline protocol; readiness has no
selectable or negotiated product-mode field.

## 6. Stream events

Torii SSE helpers return live-only generators. They may reconnect within the
configured retry budget, but Torii retains no replay log: reconnects can leave
a gap. Replay cursors and resume arguments are intentionally unsupported; query
committed ledger state when complete history is required.

```python
for event in client.stream_pipeline_blocks(
    status="Committed",
    with_metadata=True,
):
    print("Block height", event.data.block.height)
```

Other convenience methods include `stream_pipeline_transactions`,
`stream_events` (with typed filter builders), and `stream_verifying_key_events`.

## 7. Next steps

- Explore the examples under `python/iroha_python/src/iroha_python/examples/`
  for end-to-end flows covering governance, ISO bridge helpers, and Connect.
- Use `create_torii_client` / `resolve_torii_client_config` when you want to
  bootstrap the client from an `iroha_config` JSON file or environment.
- For Norito RPC or Connect-specific APIs, check the specialised modules such as
  `iroha_python.norito_rpc` and `iroha_python.connect`.

## Related Norito examples

- [Initializer skeleton](../norito/examples/hajimari-entrypoint) — mirrors the check/build
  workflow from this quickstart so you can deploy the same starter contract from Python.
- [Register domain and mint assets](../norito/examples/register-and-mint) — matches the domain +
  asset flows above and is useful when you want the ledger-side implementation instead of SDK builders.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — showcases the `transfer_asset`
  syscall so you can compare contract-driven transfers with the Python helper methods.

With these building blocks you can exercise Torii from Python without writing
your own HTTP glue or Norito codecs. As the SDK matures, additional high-level
builders will be added; consult the README in the `python/iroha_python`
directory for the latest status and migration notes.
