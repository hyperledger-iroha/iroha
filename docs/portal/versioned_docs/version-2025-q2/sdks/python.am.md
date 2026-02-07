---
lang: am
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4d1af3021d94540c338c921ea8393a10dd918ee1549965cdc09fbc612c938444
source_last_modified: "2025-12-29T18:16:35.908874+00:00"
translation_last_reviewed: 2026-02-07
---

# Python SDK Quickstart

The Python SDK (`iroha-python`) mirrors the Rust client helpers so you can
interact with Torii from scripts, notebooks, or web backends. This quickstart
covers installation, transaction submission, and event streaming. For deeper
coverage see `python/iroha_python/README.md` in the repository.

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

## 5. Stream events

Torii SSE endpoints are exposed via generators. The SDK automatically resumes
when `resume=True` and you provide an `EventCursor`.

```python
from iroha_python import PipelineEventFilterBox, EventCursor

cursor = EventCursor()

for event in client.stream_pipeline_blocks(
    status="Committed",
    resume=True,
    cursor=cursor,
    with_metadata=True,
):
    print("Block height", event.data.block.height)
```

Other convenience methods include `stream_pipeline_transactions`,
`stream_events` (with typed filter builders), and `stream_verifying_key_events`.

## 6. Next steps

- Explore the examples under `python/iroha_python/src/iroha_python/examples/`
  for end-to-end flows covering governance, ISO bridge helpers, and Connect.
- Use `create_torii_client` / `resolve_torii_client_config` when you want to
  bootstrap the client from an `iroha_config` JSON file or environment.
- For Norito RPC or Connect-specific APIs, check the specialised modules such as
  `iroha_python.norito_rpc` and `iroha_python.connect`.

With these building blocks you can exercise Torii from Python without writing
your own HTTP glue or Norito codecs. As the SDK matures, additional high-level
builders will be added; consult the README in the `python/iroha_python`
directory for the latest status and migration notes.
