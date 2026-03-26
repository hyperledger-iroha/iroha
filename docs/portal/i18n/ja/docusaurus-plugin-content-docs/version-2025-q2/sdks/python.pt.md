---
lang: pt
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/version-2025-q2/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3dea23cc339fcad4a8062ba57d5563568c15600309983b5b2bbbaad1206c4e16
source_last_modified: "2026-01-30T17:50:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/python.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f2dd6b790ce0252c355db5218b64ca9a15f4200879fe874499df079ae168872
source_last_modified: "2026-01-30T12:29:51+00:00"
translation_last_reviewed: 2026-01-30
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

Account inventory helpers accept an optional `asset_id` filter when you only
care about a specific asset:

```python
asset_id = "norito:4e52543000000001"
assets = client.list_account_assets("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", asset_id=asset_id, limit=5)
txs = client.list_account_transactions("soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ", asset_id=asset_id, limit=5)
holders = client.list_asset_holders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", asset_id=asset_id, limit=5)
print(assets, txs, holders)
```

## 5. Offline allowances

Use the offline allowance endpoints to issue wallet certificates and register
them on-ledger. `top_up_offline_allowance` chains the issue + register steps
(there is no single top-up endpoint):

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080")

draft = {
    "controller": "i105:...",
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
    authority="soraゴヂアヌョシペギゥルゼプキュビルェッハガヌイタソタィニュチョヵボヮゾバュチョナボポビワグツニュノノツマヘサ",
    private_key="operator-private-key",
)
print("registered", top_up.registration.certificate_id_hex)
```

For renewals, call `top_up_offline_allowance_renewal` with the current certificate id:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="soraゴヂアヌョシペギゥルゼプキュビルェッハガヌイタソタィニュチョヵボヮゾバュチョナボポビワグツニュノノツマヘサ",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

If you need to split the flow, call `issue_offline_certificate` (or
`issue_offline_certificate_renewal`) followed by `register_offline_allowance`
or `renew_offline_allowance`.

## 6. Stream events

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

## 7. Next steps

- Explore the examples under `python/iroha_python/src/iroha_python/examples/`
  for end-to-end flows covering governance, ISO bridge helpers, and Connect.
- Use `create_torii_client` / `resolve_torii_client_config` when you want to
  bootstrap the client from an `iroha_config` JSON file or environment.
- For Norito RPC or Connect-specific APIs, check the specialised modules such as
  `iroha_python.norito_rpc` and `iroha_python.connect`.

## Related Norito examples

- [Hajimari entrypoint skeleton](../norito/examples/hajimari-entrypoint) — mirrors the compile/run
  workflow from this quickstart so you can deploy the same starter contract from Python.
- [Register domain and mint assets](../norito/examples/register-and-mint) — matches the domain +
  asset flows above and is useful when you want the ledger-side implementation instead of SDK builders.
- [Transfer asset between accounts](../norito/examples/transfer-asset) — showcases the `transfer_asset`
  syscall so you can compare contract-driven transfers with the Python helper methods.

With these building blocks you can exercise Torii from Python without writing
your own HTTP glue or Norito codecs. As the SDK matures, additional high-level
builders will be added; consult the README in the `python/iroha_python`
directory for the latest status and migration notes.
