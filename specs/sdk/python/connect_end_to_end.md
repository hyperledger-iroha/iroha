<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Python SDK — Torii & Connect End-to-End Guide

Roadmap item **PY6-P2 (end-to-end docs & notebooks)** requires a single place
that ties the Python SDK’s Torii helpers, Connect CLI, and notebook automation
together. Use this guide when building parity tests or operator demos ahead of
the PY6 gate.

## Prerequisites

- Python ≥ 3.11 with `pip`.
- Access to a Torii endpoint (default `http://127.0.0.1:8080`) plus an auth
  token (`IROHA_TORII_AUTH_TOKEN`) that can create Connect sessions and submit
  transactions.
- Git checkout of this repository (for editable installs, examples, and tests).
- Optional: virtualenv tooling (`python3 -m venv .venv && source .venv/bin/activate`)
  to keep dependencies isolated.

> Set `IROHA_NETWORK_ID`, `IROHA_TORII_URL`, `IROHA_TORII_AUTH_TOKEN`, and
> `IROHA_TORII_API_TOKEN` in your shell before running the snippets below.
> `IROHA_NETWORK_ID` must be the deployment's exact canonical lowercase
> 64-hex genesis-derived `NetworkId`, with the final marker bit set—not an
> operator-selected chain label or tagged Norito-JSON hash.

## 1. Install and configure the SDK

Install the SDK directly from the workspace so you can run examples and tests:

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -e python/iroha_python

# include dev tooling (pytest/mypy/ruff) when preparing CI parity runs
pip install -e python/iroha_python[dev]
```

Verify the CLI wiring:

```bash
python -m iroha_python.examples.connect_flow --help
```

If you prefer to consume a published wheel, install `pip install iroha-python`
and keep the repo checkout handy for notebooks/fixtures.

## 2. Submit transactions through `/v1/pipeline`

Use the high-level `ToriiClient` helpers to build transactions with Norito
builders and post them to the pipeline endpoint. The example below mirrors the
quickstart in `python/iroha_python/README.md`.

```python
import os

from iroha_python import (
    Instruction,
    NetworkId,
    build_signed_transaction,
    create_torii_client,
    derive_ed25519_keypair_from_seed,
)

client = create_torii_client("http://127.0.0.1:8080", auth_token="admin-token")
pair = derive_ed25519_keypair_from_seed(b"demo-seed")
authority = pair.default_account_id("wonderland")

instruction = Instruction.register_domain("wonderland")
tx = build_signed_transaction(
    network_id=NetworkId.parse(os.environ["IROHA_NETWORK_ID"]),
    authority=authority,
    private_key=pair.private_key,
    fee_payment={"payer": "authority", "value": {"charge_limits": [], "gas_limit": None}},
    instructions=[instruction],
)

envelope, status = client.submit_pipeline_transaction(tx, wait=True)
print("Pipeline status:", status)
```

- `submit_pipeline_transaction` posts directly to `/v1/pipeline/transactions`
  and returns the canonical envelope plus the latest status.
- A `404` from `/v1/pipeline/transactions/status` means Torii has no cached status yet
  (for example after a restart), so the client treats it as pending and continues polling.
- Stream the resulting events with `client.stream_pipeline_transactions` or
  fetch the recovery sidecar via `client.get_pipeline_recovery(height)` to
  validate retries, as required by PY6-P0.
- When driving governance/Connect fixtures, emit Norito JSON with
  `Instruction.to_json()` and submit it through the same helper to keep parity
  with the Rust reference tests.

## 3. Stage a Connect session end-to-end

The scripted workflow in
`python/iroha_python/src/iroha_python/examples/connect_flow.py` walks through
session creation, policy inspection, and frame encoding. Run it against your
Torii target:

```bash
python -m iroha_python.examples.connect_flow \
  --base-url ${IROHA_TORII_URL:-http://127.0.0.1:8080} \
  --sid "${IROHA_CONNECT_SID}" \
  --network-id "${IROHA_NETWORK_ID}" \
  --app-public-key "${IROHA_CONNECT_APP_PUBLIC_KEY_HEX}" \
  --nonce "${IROHA_CONNECT_NONCE_HEX}" \
  --auth-token "${IROHA_TORII_AUTH_TOKEN}" \
  --app-name "Demo dApp" \
  --app-url https://demo.example \
  --app-icon-hash deadbeef \
  --frame-output ./connect-open.hex \
  --frame-json-output ./connect-open.json \
  --send-open
```

The app-focused helper prints the typed `ConnectSessionInfo` and optionally
posts the encoded Open control frame back to Torii when `--send-open` is set.
It does not read node aggregate status, which requires a runtime-only operator
signing context. Outputs:

- `connect-open.hex` — hex-encoded envelope suitable for regressions or manual
  relay.
- `connect-open.json` — base64 representation for dashboards/telemetry.
Use `--write-app-metadata-template <path>` to generate the JSON metadata stub
shipped alongside the script, or pass `--app-metadata-file metadata.json` to
reuse existing metadata blobs. The same dataclasses power the Connect
administration helpers (`client.list_connect_apps()`, `client.update_connect_app_policy_controls()`)
described in the README, so automation can pivot between CLI, notebook, and API
calls without re-encoding frames manually.

### Bootstrap previews directly from Python

Roadmap deliverable **PY6-P1** also calls for SDK helpers so wallets and
dashboards can mint Connect previews without shelling out to the CLI. The
`iroha_python.connect` module now exposes `generate_connect_sid`,
`create_connect_session_preview`, and `bootstrap_connect_preview_session` to
cover this workflow:

```python
import os

from iroha_python import (
    NetworkId,
    bootstrap_connect_preview_session,
    create_connect_session_preview,
    generate_connect_sid,
    create_torii_client,
)

client = create_torii_client("http://127.0.0.1:8080", auth_token="admin-token")
network_id = NetworkId.parse(os.environ["IROHA_NETWORK_ID"])

# Derive the only valid SID for this exact NetworkId, app key, and nonce.
sid_material = generate_connect_sid(
    network_id=network_id,
    app_public_key=b"\x01" * 32,
    nonce=b"\x02" * 16,
)
print("SID (base64url):", sid_material.sid_base64url)

# Build deeplinks + key pair without registering the session.
preview = create_connect_session_preview(
    network_id=network_id,
    node="https://torii.dev.example",
)
print("Wallet URI:", preview.wallet_uri)
print("App URI:", preview.app_uri)

# Register the session with Torii and capture the issued tokens.
result = bootstrap_connect_preview_session(
    client,
    network_id=network_id,
    node="https://torii.dev.example",
)
print("Torii SID:", result.preview.sid_base64url)
print("Wallet token:", result.tokens.wallet if result.tokens else None)
```

Pass `register=False` when you only need deterministic URIs (for example when
rendering QR codes) or supply `session_options={"node": "https://override"}` to
force the node stored in Torii. The helper returns the typed
`ConnectSessionInfo`, so dashboards/tests can persist or assert on the exact
tokens that Torii produced.

### Generate previews with the CLI

The CLI now mirrors the preview helpers so roadmap milestone **PY6-P1** can be
exercised without writing a bespoke script. Switch the example to preview mode
and capture the JSON summary along with the issued tokens:

```bash
python -m iroha_python.examples.connect_flow \
  --mode preview \
  --network-id "${IROHA_NETWORK_ID}" \
  --preview-node https://torii.dev.example \
  --preview-register \
  --preview-output artifacts/connect-preview/exact-network.json \
  --auth-token "${IROHA_TORII_AUTH_TOKEN}"
```

- `--preview-register` hits Torii’s `/v1/connect/session` API and prints the
  issued wallet/app tokens so QA and dashboards can store the evidence bundle.
- `--preview-output` writes the preview metadata, keypair, URIs, tokens, and
  raw Torii session payload into a deterministic JSON file.
- `--preview-nonce` and `--preview-app-private-key` accept hex-encoded blobs
  (16 B and 32 B respectively) so deterministic SIDs/keypairs can be rehearsed
  alongside the Android/JS flows during the shared preview drills.
- `--preview-session-node` threads an override into the Torii session so the
  stored node differs from the deeplink node when the rollout plan requires it.

Use preview mode when staging demos, smoke testing the Connect runbooks, or
exporting evidence to `status.md`—the CLI writes everything required by the
PY6 dashboards without requiring manual JSON edits.

## 4. Execute the Connect automation notebook

The reproducible notebook lives at
`python/iroha_python/notebooks/connect_automation.ipynb` and walks through:

1. Creating a `ToriiClient`.
2. Opening a session and capturing the `ConnectSessionInfo`.
3. Building and encoding the `ConnectControlOpen` frame.
4. Decoding the frame for verification.

CI executes the notebook via `pytest -m nb`, and you can run the same check
locally:

```bash
pytest -m nb python/iroha_python/tests/test_connect_notebook.py
```

The test asserts that `/v1/connect/session` and the control endpoints are
exercised, and it inspects the decoded frame contents to
catch drift before fixtures update. Attach the generated artefacts to roadmap
evidence when closing PY6-P2 items.

## 5. Suggested CI hooks & troubleshooting

- Run `python/iroha_python/scripts/run_integration.py` (or the `run_integration.sh`
  wrapper) to stand up the docker-compose Torii topology and execute the
  integration marker before publishing notebooks or docs.
- Use `python/iroha_python/scripts/run_norito_rpc_smoke.sh` to validate the
  Norito RPC helpers that power `/v1/pipeline` submissions.
- `cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-fixtures --output-root /path/to/first-new-norito-rpc-publication`
  renders the complete canonical owner publication. Repeat at a second absent
  root and require identical exact path sets, entry types, modes, completion
  manifests, and every file byte before applying the reviewed identity-relative
  tracked patch; Python and Android are
  generated consumers of that same result. `python3 scripts/check_python_fixtures.py`
  enforces the parity gate referenced in `roadmap.md`.

When filing readiness reports, include:

- The exact CLI command (with sanitized tokens) used to generate Connect
  frames.
- Hashes of the `connect-open` artefacts or notebook outputs.
- Links to the CI job that executed the `pytest -m nb` notebook run.

Following the steps above gives SDK maintainers the reproducible Torii +
Connect workflow expected by the PY6 roadmap milestones.

## 6. Inspect node-admin & telemetry surfaces

Roadmap item **PY6-P5** also calls for parity across the core admin endpoints so SDK
operators can triage peers, network time, and Sumeragi health without switching to curl
or bespoke scripts. The Python client now exposes typed wrappers for `/v1/peers`,
`/v1/time/{now,status}`, and the Sumeragi inspection endpoints
(`/v1/sumeragi/{qc,pacemaker,phases,leader,telemetry,params,bls_keys,evidence/count}`);
include them in integration tests and runbooks so the resulting artefacts satisfy the
telemetry-readiness portion of PY6.

```python
from iroha_python import create_torii_client

client = create_torii_client("http://127.0.0.1:8080", auth_token="admin-token")

# Peer inventory (matches `ToriiClient.list_peers_typed`)
peers = client.list_peers_typed()
for peer in peers:
    print(peer.address, peer.id.public_key, peer.last_seen_at)

# Network Time Service snapshots (typed DTOs)
snapshot = client.get_time_now_typed()
print("Cluster time:", snapshot.now_ms, "offset", snapshot.offset_ms)

status = client.get_time_status_typed()
for sample in status.samples:
    print(
        sample.peer,
        "offset", sample.last_offset_ms,
        "rtt", sample.last_rtt_ms,
        "count", sample.count,
    )
print("RTT buckets:", status.rtt.buckets)

# Sumeragi admin surfaces (new in PY6-P5)
qc = client.get_sumeragi_qc_typed()
if qc.highest_prepare_qc is not None:
    print(
        "Highest PrepareQC height:", qc.highest_prepare_qc.round.height,
        "view", qc.highest_prepare_qc.round.view,
    )

params = client.get_sumeragi_params()
print(
    "Signed block cadence:", params["block_cadence_ms"],
    "chain height:", params["chain_height"],
)
```

Consensus mode and DA geometry come from signed chain context, not from a local
Sumeragi switch or an observability field.

### Capture admin evidence with one helper

For readiness reports that need a single JSON blob, call
`client.capture_node_admin_snapshot()` to fetch the `/v1/configuration`,
`/v1/peers`, `/v1/time/{now,status}`, `/v1/telemetry/peers-info`, and
`/v1/node/capabilities` payloads in one go. This is especially useful when
filling the PY6-P5 telemetry templates:

```python
snapshot = client.capture_node_admin_snapshot()
print("Queue capacity:", snapshot.configuration.queue.capacity)
print("Peers:", [peer.address for peer in snapshot.peers])
print("Time offset (ms):", snapshot.time_now.offset_ms)
print("ABI version:", snapshot.node_capabilities.abi_version)
print("Data model version:", snapshot.node_capabilities.data_model_version)

if snapshot.telemetry_peers is None:
    print("Telemetry endpoint skipped")
else:
    print("Telemetry peers:", len(snapshot.telemetry_peers))
```

Pass ``include_peer_telemetry=False`` when the fleet omits
`/v1/telemetry/peers-info`; the helper still records the remaining endpoints so
dashboards and auditors can rely on one deterministic structure.
The SDK refuses to submit transactions when `data_model_version` differs from
its built-in value, so mismatched nodes are rejected before submission.

## 7. Persist Connect session counters

Roadmap item **PY6-P1** mandates anti-replay enforcement and recoverable
Connect sessions. The SDK now exposes `ConnectSessionState` snapshots so wallets
or dApps can persist the per-direction counters after every successful decrypt
and restore them after a crash without dropping the session.

```python
from iroha_python import ConnectSession, ConnectSessionKeys, ConnectSessionState

# after Connect approval
keys = ConnectSessionKeys.derive(
    local_private_key=local_private_key_bytes,
    peer_public_key=peer_public_key_bytes,
    sid=session_sid_bytes,
)
session = ConnectSession(sid=session_sid_bytes, keys=keys)

# ... encrypt/decrypt frames ...

# persist snapshot (e.g., to disk after decrypting a frame)
snapshot = session.snapshot_state()
write_snapshot_to_disk(snapshot.to_dict())

# later, resume the session
snapshot_dict = read_snapshot_from_disk()
restored = ConnectSession.from_state(
    keys=keys,
    state=ConnectSessionState.from_dict(snapshot_dict),
)
```

Snapshots include the `sid`, the next sequence number for each direction, and
the last decrypted sequence so replayed frames remain rejected even after a
restart. Serialising with `to_dict()` yields a JSON-friendly payload that can be
encrypted and stored alongside other wallet secrets. Use the same helper in CI
fixtures when demonstrating PY6-P1 compliance; the restored session should
reject previously decrypted frames and continue incrementing counters without
gaps.
```

- `list_peers_typed()` returns a list of `PeerInfo` records (address, peer ID, metadata)
  so dashboards or operator tooling can diff the online set deterministically.
- `get_time_now_typed()` mirrors `/v1/time/now` and raises if the endpoint is disabled,
  matching the deterministic behaviour expected by the runbooks.
- `get_time_status_typed()` parses the per-peer samples and RTT histogram produced by
  `/v1/time/status`; use it to confirm clock skew alerts before filing readiness reports.

Configuration surfaces now follow the same pattern: `get_configuration_typed()`
returns a `ConfigurationSnapshot` (logger, gossip windows, queue capacity, and
the confidential gas schedule). The gas schedule is read-only at runtime because
it is consensus-relevant and committed into the ZK policy hash. Operators change
it through a coordinated startup configuration rollout, not through
`/v1/configuration`.

```python
config = client.get_configuration_typed()
print("Logger:", config.logger.level, "Queue cap:", config.queue.capacity if config.queue else "n/a")

schedule = client.get_confidential_gas_schedule_typed()
if schedule:
    print("Proof base cost:", schedule.proof_base)
```

Capture the JSON output (or include it in notebook artefacts) when closing PY6-P5
deliverables—governance reviewers often request hashes of the peer list and network
time samples alongside the Connect evidence.

## 8. Render Explorer account QR payloads

Roadmap item **ADDR-6b** calls for parity across SDKs when surfacing the new
`/v1/explorer/accounts/{account_id}/qr` endpoint. Python now ships the typed
`ExplorerAccountQrSnapshot` DTO so wallets/tools can fetch the preferred i105 or
canonical I105 literal plus the ready-to-embed SVG payload:

```python
from iroha_python import create_torii_client

client = create_torii_client("http://127.0.0.1:8080", auth_token="admin-token")
snapshot = client.get_explorer_account_qr_typed(
    "<i105-account-id>",
)

print("Literal:", snapshot.literal)
print("QR SVG:", snapshot.svg[:80], "…")
```

The helper accepts canonical I105 literals only and normalizes payload casing selected by Torii. Use the
typed helper when generating wallet/explorer share buttons so the resulting QR
carries the canonical I105 account id, network prefix, and error-correction metadata
expected by ADDR-6 dashboards.

## 9. Submit ISO 20022 bridge messages

Roadmap deliverables **JS-06/PY6** call for parity across SDKs when exercising
Torii’s ISO bridge. The Python client now mirrors the JavaScript helpers:
`submit_iso_pacs008[_typed]`, `submit_iso_pacs009[_typed]`, and
`wait_for_iso_message_status` expose the same flows used in
`specs/finance/settlement_iso_mapping.md`. Pair them with the field
mapping guide to keep XML payloads valid before handing them to Torii.

```python
from pathlib import Path
from iroha_python import create_torii_client

client = create_torii_client("http://127.0.0.1:8080", auth_token="iso-bridge")
iso_xml = Path("artifacts/iso/pacs008.xml").read_bytes()

submission = client.submit_iso_pacs008_typed(iso_xml)
if submission is None:
    raise RuntimeError("bridge did not return a message id")

# Poll `/v1/iso20022/messages/{id}` until the message leaves Pending/Accepted.
status = client.wait_for_iso_message_status(
    submission.message_id,
    poll_interval=0.5,
    max_attempts=30,
    resolve_on_accepted=True,
)

print("ISO status:", status.status, "pacs.002 code:", status.pacs002_code)
print("Ledger record:", status.ledger_id, status.transaction_hash)
```

- `submit_iso_pacs009[_typed]` mirrors the same signature for PvP cash legs; pass
  `content_type="application/pacs009+xml"` when Torii enforces strict MIME values.
- `submit_iso_pacs008_and_wait`/`submit_iso_pacs009_and_wait` accept a `wait={}`
  dictionary (keys: `poll_interval`, `max_attempts`, `resolve_on_accepted`,
  `timeout`, `on_poll`) when you prefer a single call that submits and polls.
- `IsoSubmissionRecord` captures the bridge metadata (message id, ledger ids,
  PACS.002 reason codes, account identifiers) so notebooks/tests can archive the
  evidence that governance expects for ISO rehearsals.

Keep the XML fixtures, Torii responses, and the `IsoSubmissionRecord` JSON
attachments in your roadmap evidence bundles. Reviewers typically expect the raw
ISO payload, the bridge response (with PACS.002 codes), and the final ledger
transaction hash when closing out ISO readiness items.
