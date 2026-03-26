---
lang: az
direction: ltr
source: docs/source/sdk/python/connect_end_to_end.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afb94acbf158206f0d63ff7720518315df339204655d472d6d7f42a3da9268f5
source_last_modified: "2026-01-28T17:11:30.748393+00:00"
translation_last_reviewed: 2026-02-07
---

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

> Set `IROHA_TORII_URL`, `IROHA_TORII_AUTH_TOKEN`, and `IROHA_TORII_API_TOKEN`
> in your shell before running the snippets below. The helpers fall back to the
> same environment variables that `create_torii_client` and the CLI use in
> `python/iroha_python/README.md`.

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
from iroha_python import (
    Instruction,
    build_signed_transaction,
    create_torii_client,
    derive_ed25519_keypair_from_seed,
)

client = create_torii_client("http://127.0.0.1:8080", auth_token="admin-token")
pair = derive_ed25519_keypair_from_seed(b"demo-seed")
authority = pair.default_account_id("wonderland")

instruction = Instruction.register_domain("wonderland")
tx = build_signed_transaction(
    chain_id="dev-chain",
    authority=authority,
    private_key=pair.private_key,
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
  --sid demo-session \
  --chain-id dev-chain \
  --auth-token "${IROHA_TORII_AUTH_TOKEN}" \
  --app-name "Demo dApp" \
  --app-url https://demo.example \
  --app-icon-hash deadbeef \
  --frame-output ./connect-open.hex \
  --frame-json-output ./connect-open.json \
  --status-json-output ./connect-status.json \
  --send-open
```

The helper prints the typed `ConnectSessionInfo`, current policy limits, and
optionally posts the encoded Open control frame back to Torii when `--send-open`
is set. Outputs:

- `connect-open.hex` — hex-encoded envelope suitable for regressions or manual
  relay.
- `connect-open.json` — base64 representation for dashboards/telemetry.
- `connect-status.json` — typed snapshot of `/v1/connect/status` (mirrors the
  dataclasses in `iroha_python.connect`).

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
from iroha_python import (
    ConnectKeyPair,
    bootstrap_connect_preview_session,
    create_connect_session_preview,
    generate_connect_sid,
    create_torii_client,
)

client = create_torii_client("http://127.0.0.1:8080", auth_token="admin-token")

# Deterministically derive a SID from the app's public key (optional).
sid_material = generate_connect_sid(
    chain_id="dev-chain",
    app_public_key=b"\x01" * 32,
    nonce=b"\x02" * 16,
)
print("SID (base64url):", sid_material.sid_base64url)

# Build deeplinks + key pair without registering the session.
preview = create_connect_session_preview(
    chain_id="dev-chain",
    node="https://torii.dev.example",
)
print("Wallet URI:", preview.wallet_uri)
print("App URI:", preview.app_uri)

# Register the session with Torii and capture the issued tokens.
result = bootstrap_connect_preview_session(
    client,
    chain_id="dev-chain",
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
  --chain-id dev-chain \
  --preview-node https://torii.dev.example \
  --preview-register \
  --preview-output artifacts/connect-preview/dev-chain.json \
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

The test asserts that `/v1/connect/session`, `/v1/connect/status`, and the
control endpoints are exercised, and it inspects the decoded frame contents to
catch drift before fixtures update. Attach the generated artefacts to roadmap
evidence when closing PY6-P2 items.

## 5. Suggested CI hooks & troubleshooting

- Run `python/iroha_python/scripts/run_integration.py` (or the `run_integration.sh`
  wrapper) to stand up the docker-compose Torii topology and execute the
  integration marker before publishing notebooks or docs.
- Use `python/iroha_python/scripts/run_norito_rpc_smoke.sh` to validate the
  Norito RPC helpers that power `/v1/pipeline` submissions.
- `python3 scripts/python_fixture_regen.sh` keeps the SDK’s copy of the Norito
  fixtures in sync with Android; `python3 scripts/check_python_fixtures.py`
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
(`/v1/sumeragi/{qc,pacemaker,phases,leader,collectors,params,bls_keys,evidence/count}`);
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
qc = client.get_sumeragi_qc()
print("Highest QC height:", qc.highest_qc.height, "view", qc.highest_qc.view)

collectors = client.get_sumeragi_collectors()
for slot in collectors.collectors:
    print("Collector slot", slot.index, "peer", slot.peer_id)

phases = client.get_sumeragi_phases()
print("Pipeline latency (ms):", phases.pipeline_total_ms)

params = client.get_sumeragi_params()
print("DA enabled:", params.da_enabled, "chain height:", params.chain_height)

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
confidential gas schedule), while `set_confidential_gas_schedule()` reuses the
current logger section and posts the new limits through `/v1/configuration`.

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
canonical Katakana i105 literal plus the ready-to-embed SVG payload:

```python
from iroha_python import create_torii_client

client = create_torii_client("http://127.0.0.1:8080", auth_token="admin-token")
snapshot = client.get_explorer_account_qr_typed(
    "soraカタカナ...",
)

print("Literal:", snapshot.literal)
print("QR SVG:", snapshot.svg[:80], "…")
```

The helper accepts canonical Katakana i105 literals only and normalizes payload casing selected by Torii. Use the
typed helper when generating wallet/explorer share buttons so the resulting QR
carries the canonical Katakana i105 account id, network prefix, and error-correction metadata
expected by ADDR-6 dashboards.

## 9. Submit ISO 20022 bridge messages

Roadmap deliverables **JS-06/PY6** call for parity across SDKs when exercising
Torii’s ISO bridge. The Python client now mirrors the JavaScript helpers:
`submit_iso_pacs008[_typed]`, `submit_iso_pacs009[_typed]`, and
`wait_for_iso_message_status` expose the same flows used in
`docs/source/finance/settlement_iso_mapping.md`. Pair them with the field
mapping guide to keep XML payloads valid before handing them to Torii.

```python
from pathlib import Path
from iroha_python import create_torii_client

client = create_torii_client("http://127.0.0.1:8080", auth_token="iso-bridge")
iso_xml = Path("artifacts/iso/pacs008.xml").read_bytes()

submission = client.submit_iso_pacs008_typed(iso_xml)
if submission is None:
    raise RuntimeError("bridge did not return a message id")

# Poll `/v1/iso20022/status/{id}` until the message leaves Pending/Accepted.
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
