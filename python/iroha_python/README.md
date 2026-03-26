# Iroha Python SDK (work in progress)

`iroha-python` packages the client-side utilities Python developers need to
interact with Hyperledger Iroha v2 nodes. It bundles Norito codecs, Torii client
helpers, and crypto primitives aligned with the Rust implementation—more high
level builders will follow as the SDK matures. See [`DESIGN.md`](DESIGN.md) for a
full architecture overview.

## Quickstart

```bash
pip install iroha-python
```

```python
from iroha_python import (
    ToriiClient,
    Instruction,
    build_signed_transaction,
    derive_ed25519_keypair_from_seed,
)

pair = derive_ed25519_keypair_from_seed(b"demo-seed")
authority = pair.default_account_id("wonderland")  # Canonical Katakana i105 account id
instruction = Instruction.register_domain("wonderland")

client = ToriiClient("http://127.0.0.1:8080", auth_token="dev-token")
envelope, status = client.build_and_submit_transaction(
    chain_id="local",
    authority=authority,
    private_key=pair.private_key,
    instructions=[instruction],
    wait=True,
)

print("Final status:", status)

# Config-aware client creation

import json
from iroha_python import create_torii_client, resolve_torii_client_config

with open("iroha_config.json", "r", encoding="utf-8") as handle:
    raw_config = json.load(handle)

resolved = resolve_torii_client_config(
    config=raw_config,
    overrides={"timeout_ms": 2_000, "max_retries": 5},
)
client = create_torii_client(
    raw_config.get("torii", {}).get("address", "http://127.0.0.1:8080"),
    resolved_config=resolved,
)
```

## Offline allowances

Issue offline wallet certificates and register them on-ledger with the offline
allowance endpoints. `top_up_offline_allowance` chains the certificate issue
and register steps, returning both responses.

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080", auth_token="dev-token")

draft = {
    "controller": "<canonical_i105_account_id>",
    "allowance": {"asset": "<canonical_base58_asset_definition_id>", "amount": "10", "commitment": [1, 2]},
    "spend_public_key": "ed0120deadbeef",
    "attestation_report": [3, 4],
    "issued_at_ms": 100,
    "expires_at_ms": 200,
    "policy": {"max_balance": "10", "max_tx_value": "5", "expires_at_ms": 200},
    "metadata": {},
}

top_up = client.top_up_offline_allowance(
    certificate=draft,
    authority="<canonical_i105_account_id>",
    private_key="operator-private-key",
)
print("certificate id", top_up.registration.certificate_id_hex)
```

For renewals, call `top_up_offline_allowance_renewal` with the current
`certificate_id_hex`:

```python
renewed = client.top_up_offline_allowance_renewal(
    certificate_id_hex=top_up.registration.certificate_id_hex,
    certificate=draft,
    authority="<canonical_i105_account_id>",
    private_key="operator-private-key",
)
print("renewed", renewed.registration.certificate_id_hex)
```

If you already have a signed certificate, call `register_offline_allowance` or
`renew_offline_allowance` directly instead of the top-up helpers.

## Account addresses

The `iroha_python.address` module mirrors the Rust codecs so applications can
round-trip canonical bytes and canonical Katakana i105 account literals without bespoke conversions:

```python
from iroha_python.address import AccountAddress

address = AccountAddress.from_account(domain="default", public_key=b"\x00" * 32)
print(address.canonical_hex())
print(address.to_i105(753))

formats = address.display_formats(753)
print(formats["i105"])
print(formats["chain_discriminant"])
print(formats["i105_warning"])
```

> ℹ️ Use i105 literals consistently across SDK samples and operator tooling.
> For Sora network discriminant `753`, literals should start with the `sora` sentinel.

## RWA instructions

`TransactionDraft` now mirrors the dedicated RWA lot family, including
register/merge, lifecycle controls, and per-lot metadata updates.

```python
from decimal import Decimal

from iroha_python import TransactionConfig, TransactionDraft

draft = TransactionDraft(
    TransactionConfig(chain_id="local", authority="<canonical_i105_account_id>")
)

draft.register_rwa(
    {
        "domain": "commodities",
        "quantity": "10.5",
        "spec": {"scale": 1},
        "primary_reference": "vault-cert-001",
        "status": None,
        "metadata": {"origin": "AE"},
        "parents": [],
        "controls": {
            "controller_accounts": [],
            "controller_roles": [],
            "freeze_enabled": False,
            "hold_enabled": False,
            "force_transfer_enabled": False,
            "redeem_enabled": False,
        },
    }
).set_rwa_key_value(
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities",
    "grade",
    {"bucket": "A", "score": Decimal("9")},
).hold_rwa(
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities",
    quantity=Decimal("2.500"),
)
```

`ToriiClient` also exposes the chain-state and explorer RWA read surfaces:

```python
from iroha_python import ToriiClient, rwa_query_envelope

client = ToriiClient("http://127.0.0.1:8080", auth_token="dev-token")

chain_page = client.list_rwas_typed(limit=20, offset=0)
detail_page = client.list_explorer_rwas_typed(domain="commodities", page=1, per_page=25)
detail = client.get_explorer_rwa_detail_typed("lot-001$commodities")
filtered = client.query_rwas_typed(
    filter={"eq": [{"name": "id"}, "lot-001$commodities"]},
    sort=[{"key": "id", "order": "asc"}],
)

envelope = rwa_query_envelope(limit=10, offset=0)
print(envelope["pagination"])
```

## CUDA helpers

The `iroha_python.gpu` module surfaces CUDA acceleration toggles and optional wrappers for
Poseidon permutations plus BN254 field arithmetic. These helpers return `None` when CUDA support is
unavailable or disabled so callers can fall back to the scalar implementations provided by the core
SDK.

## Subscriptions

Use the Torii subscription endpoints to publish plans, subscribe, and report usage. Set
`bill_for.period` to `previous_period` to bill in arrears (for example, charge on the first for the
previous month's usage). Fixed monthly plans use `pricing.kind = "fixed"` and
`bill_for.period = "next_period"`.

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080", auth_token="provider-token")

usage_plan = {
    "provider": "3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF",
    "billing": {
        "cadence": {
            "kind": "monthly_calendar",
            "detail": {"anchor_day": 1, "anchor_time_ms": 0},
        },
        "bill_for": {"period": "previous_period", "value": None},
        "retry_backoff_ms": 86_400_000,
        "max_failures": 3,
        "grace_ms": 604_800_000,
    },
    "pricing": {
        "kind": "usage",
        "detail": {
            "unit_price": "0.024",
            "unit_key": "compute_ms",
            "asset_definition": "usd#pay",
        },
    },
}

client.create_subscription_plan(
    authority="3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF",
    private_key="provider-private-key-hex",
    plan_id="aws_compute#commerce",
    plan=usage_plan,
)

subscription = client.create_subscription(
    authority="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
    private_key="subscriber-private-key-hex",
    subscription_id="sub-001",
    plan_id="aws_compute#commerce",
)

client.record_subscription_usage(
    "sub-001",
    authority="3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF",
    private_key="provider-private-key-hex",
    unit_key="compute_ms",
    delta="3600000",
)

client.charge_subscription_now(
    "sub-001",
    authority="3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF",
    private_key="provider-private-key-hex",
)
```

## Streaming events

All streaming helpers decode JSON payloads by default. Pass `with_metadata=True` to receive full
`SseEvent` objects (event name, id, retry hint, raw payload) and pair the stream with an
`EventCursor` to persist the latest event id so long-running consumers can resume automatically
after reconnects. The optional `on_event` callback mirrors this behaviour: it receives a decoded
payload when metadata is disabled and the full `SseEvent` when metadata is requested.

```python
from iroha_python import create_torii_client, DataEventFilter

client = create_torii_client("http://127.0.0.1:8080", auth_token="admin-token")

# Stream verifying-key registry updates
for event in client.stream_verifying_key_events(updated=True):
    print("Verifying key event", event)

# Stream proof verification results for a specific proof id
proof_filter = DataEventFilter.proof(backend="halo2/ipa", proof_hash_hex="deadbeef" * 8)
for event in client.stream_events(filter=proof_filter, resume=True):
    print("Proof event", event)

# Stream pipeline activity with typed helpers
for event in client.stream_pipeline_transactions(status="Queued"):
    print("Queued tx event", event)

for block_event in client.stream_pipeline_blocks(status="Committed"):
    print("Committed block", block_event)

# Structured events and cursor-based resume support
from iroha_python import EventCursor

cursor = EventCursor()
for evt in client.stream_events(filter=proof_filter, cursor=cursor, resume=True, with_metadata=True):
    print(evt.id, evt.event, evt.data)

# The cursor tracks the last event id automatically, so a later run can resume where it left off.
for evt in client.stream_events(filter=proof_filter, cursor=cursor, resume=True):
    print("Replayed proof event", evt)

# Inspect Connect availability with typed helpers
status = client.get_connect_status_typed()
if status and status.enabled:
    for entry in status.per_ip_sessions:
        print(entry.ip, entry.sessions)

# Fetch consensus status with structured accessors
snapshot = client.get_sumeragi_status_typed()
print(snapshot.highest_qc.height, snapshot.tx_queue.saturated)
print("DA reschedules:", snapshot.da_reschedule_total)

# Inspect Nexus lane commitments and governance coverage from `/v1/status`
status_snapshot = client.get_status_snapshot_typed()
for commitment in status_snapshot.status.lane_commitments:
    print(
        "lane",
        commitment.lane_id,
        "TEU",
        commitment.teu_total,
        "tx_count",
        commitment.tx_count,
    )
for dataspace in status_snapshot.status.dataspace_commitments:
    print(
        "dataspace",
        dataspace.dataspace_id,
        "lane",
        dataspace.lane_id,
        "TEU",
        dataspace.teu_total,
    )
for lane in status_snapshot.status.lane_governance:
    state = "ready" if lane.manifest_ready else "missing"
    print(f"lane {lane.alias} manifest {state}; validators={', '.join(lane.validator_ids)}")
if status_snapshot.status.lane_governance_sealed_total:
    print("sealed lanes remaining:", status_snapshot.status.lane_governance_sealed_total)
    if status_snapshot.status.lane_governance_sealed_aliases:
        print("sealed aliases:", ", ".join(status_snapshot.status.lane_governance_sealed_aliases))
print("DA reschedules (delta):", status_snapshot.metrics.da_reschedule_delta)

# Inspect latest NEW_VIEW receipts
new_view = client.get_sumeragi_new_view_typed()
print(new_view.ts_ms, [entry.count for entry in new_view.items])

# Track RBC throughput and sessions
rbc_metrics = client.get_sumeragi_rbc_typed()
print(rbc_metrics.sessions_active, rbc_metrics.payload_bytes_delivered_total)
rbc_sessions = client.get_sumeragi_rbc_sessions_typed()
for session in rbc_sessions.items:
    print(session.block_hash, session.delivered)
candidate = client.find_sumeragi_rbc_sampling_candidate()
if candidate:
    print("sampling candidate height", candidate["height"], "view", candidate["view"])

# Inspect collector plan and consensus parameters
collectors = client.get_sumeragi_collectors_typed()
print(collectors.consensus_mode, len(collectors.collectors))
params = client.get_sumeragi_params_typed()
print(params.block_time_ms, params.next_mode)

# Manage triggers
trigger_payload = {
    "id": "notify-admins",
    "action": {"Mint": {"asset_id": "norito:<alert-asset-id-hex>", "value": 1}},
    "authority": "soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
    "filter": {"ByTime": {"schedule_ms": 60_000}},
}
client.register_trigger(trigger_payload)
for row in client.query_triggers(filter={"authority": "soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ"})["items"]:
    print("Trigger row", row)
client.delete_trigger("notify-admins")

# Capture SoraFS PoR telemetry
challenge_payload = b"...Norito PorChallengeV1 bytes..."
proof_payload = b"...PorProofV1 bytes..."
verdict_payload = b"...AuditVerdictV1 bytes..."

challenge = client.record_sorafs_por_challenge(challenge=challenge_payload)
proof = client.record_sorafs_por_proof(proof=proof_payload)
verdict = client.record_sorafs_por_verdict(verdict=verdict_payload)
observation = client.submit_sorafs_por_observation(success=True)
status_bytes = client.get_sorafs_por_status(manifest_hex="ab" * 32, status="verified")
weekly_report = client.get_sorafs_por_weekly_report("2026-W05")
ingestion = client.get_sorafs_por_ingestion_status(manifest_hex="ab" * 32)
for provider in ingestion.providers:
    print(provider.provider_id_hex, provider.pending_challenges, provider.failures_total)

# `status_bytes`, `weekly_report`, and the export helper all return Norito payloads.
# Decode them with the `norito` crate or via `norito.decode(...)` when the matching schema is available.

# Account listings
assets = client.list_account_assets(
    "soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
    limit=10,
    asset_id="norito:<asset-id-hex>",
)
txs = client.list_account_transactions(
    "soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
    limit=5,
    asset_id="norito:<asset-id-hex>",
)
query_txs = client.query_account_transactions(
    "soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
    filter={"status": {"Eq": "Committed"}},
    sort={"timestamp": "DESC"},
    limit=3,
)
print(assets, txs, query_txs)

```

```python
# Create a Connect session with type-safe response
from iroha_python import create_torii_client

client = create_torii_client("http://127.0.0.1:8080", auth_token="admin-token")
info = client.create_connect_session_info({"role": "app", "sid": "base64url-sid"})
print(info.app_uri)
print(info.wallet_token)
```
# Connect URI helpers
from iroha_python.connect import ConnectUri, build_connect_uri, parse_connect_uri

uri = build_connect_uri(ConnectUri(sid="base64url", chain_id="local-testnet", node="node.example:443"))
parsed = parse_connect_uri(uri)
assert parsed.sid == "base64url"


## Transaction helpers

Build transactions with ergonomic helpers that wrap the low-level `Instruction` APIs:

```python
from iroha_python import TransactionConfig, TransactionDraft, Ed25519KeyPair

config = TransactionConfig(chain_id="dev-chain", authority="soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ", ttl_ms=120_000)
draft = TransactionDraft(config)
draft.register_domain("wonderland") \
     .register_account("soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ", metadata={"role": "admin"}) \
     .register_asset_definition_numeric(
        "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
        owner="soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
        scale=2,
        mintable="Infinitely",
        metadata={"sym": "ROS"},
     ) \
     .mint_asset_numeric("norito:<asset-id-hex>", 10)

pair = Ed25519KeyPair.from_private_key(bytes([1] * 32))
envelope = draft.sign_with_keypair(pair)
receipt = client.submit_transaction_envelope(envelope)
if isinstance(receipt, dict):
    print("Submitted tx:", receipt.get("payload", {}).get("tx_hash"))
```

Apply metadata updates or transfer ownership without dropping to raw Norito:

```python
draft.set_account_key_value("nickname", "Queen Alice")
draft.transfer_domain("wonderland", destination="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ")
draft.transfer_asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM", destination="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ")
draft.transfer_nft("nft#dataspace", destination="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ")
draft.transfer_rwa(
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef$commodities",
    quantity="2.5",
    destination="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
)
```

### Repo settlement helpers

Create repo instructions without hand-crafting Norito payloads:

```python
from iroha_python import RepoCashLeg, RepoCollateralLeg, RepoGovernance

cash = RepoCashLeg(asset_definition_id="<cash_asset_definition_base58>", quantity="1000")
collateral = RepoCollateralLeg(
    asset_definition_id="<bond_asset_definition_base58>",
    quantity="1050",
    metadata={"isin": "ABC123"},
)
governance = RepoGovernance(haircut_bps=1500, margin_frequency_secs=86_400)

draft.repo_initiate(
    agreement_id="daily_repo",
    initiator="soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
    counterparty="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
    cash_leg=cash,
    collateral_leg=collateral,
    rate_bps=250,
    maturity_timestamp_ms=1_704_000_000_000,
    governance=governance,
)
draft.repo_unwind(
    agreement_id="daily_repo",
    initiator="soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
    counterparty="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
    cash_leg=cash,
    collateral_leg=collateral,
    settlement_timestamp_ms=1_704_086_400_000,
)
```

Load repo agreements from a Torii response and compute the next margin checkpoint:

```python
from iroha_python import RepoAgreementRecord

agreement = RepoAgreementRecord.from_payload(repo_payload)
next_margin = agreement.next_margin_check_after(at_timestamp_ms=now_ms)

# Discover agreements directly from Torii
from iroha_python import ToriiClient

client = ToriiClient("http://localhost:8080")
page = client.list_repo_agreements(limit=10)
for agreement in page.items:
    print(agreement.agreement_id, agreement.counterparty)
```

### DvP / PvP settlement helpers

Model bilateral settlements without hand-crafted Norito payloads:

```python
from iroha_python import (
    SettlementLeg,
    SettlementPlan,
    SettlementExecutionOrder,
    SettlementAtomicity,
)

delivery_leg = SettlementLeg(
    asset_definition_id="<bond_asset_definition_base58>",
    quantity="10",
    from_account="soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
    to_account="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
    metadata={"isin": "ABC123"},
)
payment_leg = SettlementLeg(
    asset_definition_id="<cash_asset_definition_base58>",
    quantity="1000",
    from_account="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
    to_account="soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
)
plan = SettlementPlan(
    order=SettlementExecutionOrder.PAYMENT_THEN_DELIVERY,
    atomicity=SettlementAtomicity.ALL_OR_NOTHING,
)

draft.settlement_dvp(
    settlement_id="trade_dvp",
    delivery_leg=delivery_leg,
    payment_leg=payment_leg,
    plan=plan,
    metadata={"desk": "rates"},
)

counter_leg = SettlementLeg(
    asset_definition_id="<counter_asset_definition_base58>",
    quantity="900",
    from_account="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
    to_account="soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
)
draft.settlement_pvp(
    settlement_id="trade_pvp",
    primary_leg=payment_leg,
    counter_leg=counter_leg,
)
```

Only the `all_or_nothing` atomicity policy is currently supported; the host will reject other modes until their semantics are implemented.

See `docs/source/finance/repo_runbook.md` for operator-facing CLI flows, determinism notes, and automation guidance covering both repo and settlement helpers.

For Connect automation, pass `--status-only` to the CLI helper when you only need status telemetry; the helper skips session creation and frame construction in that mode.

## Typed Connect session helper
`create_connect_session_info` returns a `ConnectSessionInfo` dataclass. When the node advertises a session TTL via `/v1/connect/status`, the helper populates `expires_at` with a UTC timestamp so callers know when to rotate tokens.

### CLI walkthrough
Run the end-to-end Connect CLI helper to stage a session, inspect policy limits, and emit an Open control frame:

```bash
python -m iroha_python.examples.connect_flow \
  --base-url http://127.0.0.1:8080 \
  --sid demo-session \
  --chain-id dev-chain \
  --auth-token admin-token \
  --app-name "Demo App" \
  --app-url https://demo.example \
  --app-icon-hash deadbeef \
  --frame-output connect-open.hex \
  --frame-json-output connect-open.json \
  --status-json-output connect-status.json \
  --send-open
```

The script prints the typed `ConnectSessionInfo`, shows the current `ConnectStatusSnapshot`, and encodes an `ConnectControlOpen` frame that can be relayed over WebSocket.

Pass `--app-name` (optionally with `--app-url` and `--app-icon-hash`) to embed display metadata in the control frame so wallets can render the requesting application context. Alternatively, provide `--app-metadata-file metadata.json` with a JSON object containing `name` (and optional `url`, `icon_hash`) to keep CLI flags tidy. A starter template lives at `python/iroha_python/src/iroha_python/examples/connect_app_metadata.json`. Use `--frame-output <path>` (with optional `--frame-output-format binary`) to persist the encoded frame, `--frame-json-output <path>` for a base64-friendly JSON blob, and `--status-json-output <path>` to dump the typed Connect status snapshot for later automation.

Run `python -m iroha_python.examples.connect_flow --write-app-metadata-template connect_app_metadata.json` to scaffold the sample metadata file without contacting a node. When you only need runtime telemetry, pass `--status-only` (optionally with `--status-json-output status.json`) to skip session creation entirely.

```python
info = client.create_connect_session_info({"role": "app", "sid": "base64url-sid"})
print(info.expires_at)
```

### Connect administration
Manage Connect registry state and admission controls directly from the SDK:

```python
registry = client.list_connect_apps(limit=25)
for app in registry.items:
    print(app.app_id, app.display_name)

policy = client.get_connect_app_policy_controls()
client.update_connect_app_policy_controls({"relay_enabled": False})

manifest = client.get_connect_admission_manifest()
client.set_connect_admission_manifest(manifest)
```

- A step-by-step automation walk-through lives at `python/iroha_python/notebooks/connect_automation.ipynb`; the accompanying notebook runs against stubbed endpoints and is executed in CI to ensure the flow stays up to date.

### Transaction manifests
For details on choosing bundles vs. images, see `docs/source/release_artifact_selection.md`.

Export manifests without hand-crafted Norito JSON:

```python
import json
from pathlib import Path

manifest = draft.to_manifest_dict()
print(json.dumps(manifest, indent=2))

manifest_json = draft.to_manifest_json(indent=2)
Path("transaction_manifest.json").write_text(manifest_json, encoding="utf-8")
```
# Connect frames

```python
from iroha_python import (
    ConnectFrame,
    ConnectControlOpen,
    ConnectDirection,
    ConnectPermissions,
    encode_connect_frame,
    decode_connect_frame,
)

frame = ConnectFrame(
    sid=b"\x01" * 32,
    direction=ConnectDirection.APP_TO_WALLET,
    sequence=1,
    control=ConnectControlOpen(
        app_public_key=b"\x02" * 32,
        chain_id="local",
        permissions=ConnectPermissions(methods=["SIGN_REQUEST_TX"], events=[]),
    ),
)

payload = encode_connect_frame(frame)
restored = decode_connect_frame(payload)
assert restored == frame
```

Ciphertext frames can be represented via `ConnectCiphertext` when the encrypted
payload is already available:

```python
from iroha_python import ConnectCiphertext

ciphertext_frame = ConnectFrame(
    sid=b"\x03" * 32,
    direction=ConnectDirection.WALLET_TO_APP,
    sequence=10,
    ciphertext=ConnectCiphertext(
        direction=ConnectDirection.WALLET_TO_APP,
        aead=b"\xDE\xAD\xBE\xEF",
    ),
)

# Derive symmetric keys and approval preimage helpers
from iroha_python import (
    generate_connect_keypair,
    connect_public_key_from_private,
    derive_connect_direction_keys,
    build_connect_approve_preimage,
    ConnectPermissions,
    ConnectSignInProof,
)

pair = generate_connect_keypair()
assert connect_public_key_from_private(pair.private_key) == pair.public_key

app_key, wallet_key = derive_connect_direction_keys(
    local_private_key=b"\x11" * 32,
    peer_public_key=b"\x22" * 32,
    sid=b"\x33" * 32,
)
assert len(app_key) == len(wallet_key) == 32

preimage = build_connect_approve_preimage(
    sid=b"\xAA" * 32,
    app_public_key=b"\xBB" * 32,
    wallet_public_key=b"\xCC" * 32,
    account_id="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
    permissions=ConnectPermissions(methods=["SIGN_REQUEST_TX"], events=[]),
    proof=ConnectSignInProof(
        domain="example.org",
        uri="https://example.org/wallet",
        statement="Sign in",
        issued_at="2024-01-01T00:00:00Z",
        nonce="abcd",
    ),
)

# Post a control frame via the Torii client
from iroha_python import ConnectControlClose, create_torii_client

info = client.create_connect_session_info({"role": "app", "sid": pair.public_key.hex()})
print(info.sid, info.app_uri)
client = create_torii_client("http://localhost:8080", auth_token="admin-token")
client.send_connect_control_frame(
    "session-id",
    ConnectControlClose(role="App", code=4100, reason="finished", retryable=False),
)
# Binary fields (public keys, signatures) are base64-encoded automatically when the SDK
# serializes the control payload for Torii.

# Inspect Connect runtime status (returns `None` when Connect is disabled)
status = client.get_connect_status()
if status and status["enabled"]:
    print("Active sessions:", status["sessions_active"])
```
```

```python
# Encrypt and decrypt payloads post-approval
from iroha_python import (
    ConnectSignRequestRawPayload,
    ConnectEnvelope,
    seal_connect_payload,
    open_connect_payload,
)

raw_payload = ConnectSignRequestRawPayload(domain_tag="SIGN", payload=b"hash")
frame = seal_connect_payload(
    app_key,
    sid=b"\x01" * 32,
    direction=ConnectDirection.APP_TO_WALLET,
    sequence=42,
    payload=raw_payload,
)
restored = open_connect_payload(app_key, frame)
assert isinstance(restored, ConnectEnvelope)
assert restored.payload.payload == b"hash"
```

For repeated messaging, use `ConnectSessionKeys.derive(...)` and `ConnectSession` to manage per-direction keys, sequence counters, and anti-replay checks while calling the sealing helpers. `ConnectSession.snapshot_state()` returns a `ConnectSessionState` snapshot (including the monotonic counters and last-seen values) that can be serialised via `to_dict()` and restored with `ConnectSession.from_state(...)`. Persist the snapshot after every successful decrypt so wallets can resume after crashes without reusing sequence numbers, satisfying the PY6-P1 anti-replay requirement.

Use `ToriiClient.get_pipeline_recovery_typed` to inspect pipeline recovery sidecars with structured DAG and transaction summaries before streaming pipeline events.

Use the typed account helpers (`list_account_assets_typed`, `list_account_transactions_typed`, and their query counterparts) to receive structured paginated results instead of raw JSON blobs when working with account inventories. The list endpoints accept an optional `asset_id` for pre-filtering.



## Governance helpers

Protected-namespace admission flows can be orchestrated directly from the SDK. Setter
endpoints require the Torii API token if the node enforces one.

```python
from iroha_python import (
    create_torii_client,
    GovernanceReferendumResult,
    GovernanceTally,
)

client = create_torii_client(
    "http://127.0.0.1:8080",
    auth_token="admin-token",
    api_token="torii-token",
)

client.set_protected_namespaces(["apps", "system"])
protected = client.get_protected_namespaces()
instances_page = client.list_governance_instances_typed(
    "apps",
    contains="calc",
    hash_prefix="dead",
    order="hash_desc",
)
council = client.get_governance_council_current()
audit = client.get_governance_council_audit(epoch=42)
proposal = client.get_governance_proposal_typed("deadbeef")
referendum = client.get_governance_referendum_typed("ref-1")
tally = client.get_governance_tally_typed("ref-1")
assert proposal.found is False
assert referendum == GovernanceReferendumResult(found=False, referendum=None)
locks = client.get_governance_locks_typed("ref-1")
unlock_stats_typed = client.get_governance_unlock_stats_typed()
print("Referendum found:", referendum.found)
print("Aye votes:", tally.approve)
print("Lock owners:", list(locks.locks))
print("Expired locks:", unlock_stats_typed.expired_locks_now)
print("Governance instances:", [inst.contract_id for inst in instances_page.instances])
print("Protected namespaces:", protected)

# VRF helpers (Torii must be built with `gov_vrf`)
client.derive_governance_council_vrf(
    {"committee_size": 21, "candidates": [{"account_id": "3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF", "variant": "Normal"}]}
)
```

## Runtime upgrades and ABI helpers

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080", auth_token="admin-token")

# Inspect ABI policy advertised by the node
abi_state = client.get_runtime_abi_active()
abi_hash = client.get_runtime_abi_hash()
metrics = client.get_runtime_metrics()

# Draft a runtime upgrade manifest (ABI stays fixed to v1 in the first release)
manifest = {
    "name": "Refresh runtime provenance",
    "description": "Schedules a no-ABI-change runtime rollout.",
    "abi_version": 1,
    "abi_hash": "00" * 32,
    "added_syscalls": [],
    "added_pointer_types": [],
    "start_height": 1_500_000,
    "end_height": 1_500_256,
}
proposal = client.propose_runtime_upgrade(manifest)

# Later, coordinate activation/cancellation using the manifest id (hex string)
activation = client.activate_runtime_upgrade("deadbeef" * 4)
cancel = client.cancel_runtime_upgrade("0x" + "feedface" * 4)

## Peer inventory & Network Time Service

The roadmap’s admin-surface work (PY6-P0/PY6-P5) requires typed access to `/v1/peers`
and `/v1/time/{now,status}` so operators can capture evidence without falling back to
curl. The client exposes these helpers directly:

```python
peers = client.list_peers_typed()
for peer in peers:
    print(peer.address, peer.id.public_key, peer.metadata)

now = client.get_time_now_typed()
print("cluster time:", now.now_ms, "offset", now.offset_ms)

status = client.get_time_status_typed()
for sample in status.samples:
    print(sample.peer, sample.last_offset_ms, sample.last_rtt_ms, sample.count)
print("RTT buckets:", status.rtt.buckets)
```

Use these outputs when filing telemetry readiness notes or running the Connect automation
notebook—the typed DTOs keep parity with the Rust roadmap references.

### Capture a full node-admin snapshot

Roadmap milestone **PY6-P5** often requires a single evidence bundle covering
`/v1/configuration`, `/v1/peers`, `/v1/time/{now,status}`, `/v1/telemetry/peers-info`,
and `/v1/node/capabilities`. The `capture_node_admin_snapshot()` helper records all
of those surfaces with one call so runbooks and tests can store a deterministic
payload:

```python
from iroha_python import create_torii_client

client = create_torii_client("http://127.0.0.1:8080", auth_token="admin-token")

snapshot = client.capture_node_admin_snapshot()
print("queue capacity:", snapshot.configuration.queue.capacity)
print("time offset:", snapshot.time_now.offset_ms)
print("capabilities:", snapshot.node_capabilities.abi_version)
if snapshot.telemetry_peers:
    print("telemetry peers:", len(snapshot.telemetry_peers))
```

Pass ``include_peer_telemetry=False`` when the deployment does not expose
`/v1/telemetry/peers-info`; the helper still records the remaining endpoints for
the roadmap audit trails.

## Kaigi relay inventory

The Kaigi rollout (SNNet-12) requires operators to audit registered relays, inspect per-domain
metrics, and capture health snapshots. The Torii client now exposes typed helpers so scripts can
collect the evidence directly:

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080", auth_token="admin-token")

relays = client.list_kaigi_relays_typed()
for entry in relays.items:
    print(entry.relay_id, entry.domain, entry.status, entry.hpke_fingerprint_hex)

relay_id = relays.items[0].relay_id if relays.items else None
detail = client.get_kaigi_relay_typed(relay_id) if relay_id else None
if detail and detail.metrics:
    print("reported by:", detail.reported_by, "registrations:", detail.metrics.registrations_total)

health = client.get_kaigi_relays_health_typed()
print(
    "healthy relays:", health.healthy_total,
    "failovers:", health.failovers_total,
)
for domain in health.domains:
    print(domain.domain, "registrations", domain.registrations_total)
```

`KaigiRelaySummary`, `KaigiRelayDetail`, and `KaigiRelayHealthSnapshot` mirror the Rust payloads,
so dashboards and readiness scripts can assert against the same DTOs tracked in the roadmap.

For configuration changes, the client now mirrors the `/v1/configuration` contract so
admin scripts can stage updates without hand-editing JSON blobs. For example:

```python
# Update gossip fan-out/intervals while preserving the existing logger/queue/gas sections.
client.set_network_gossip_config(
    block_gossip_size=8,
    block_gossip_period_ms=200,
    transaction_gossip_size=32,
    transaction_gossip_period_ms=75,
)

# Resize the transaction queue deterministically.
client.set_queue_capacity(capacity=512)
```

Both helpers fetch the latest configuration, reuse unchanged sections for parity evidence,
and raise `ValueError` when invalid parameters are supplied, keeping the PY6 admin-surface
roadmap gate reproducible.

Configuration snapshots also expose transport defaults so automation can pick up the
streaming/SoraNet knobs without parsing raw JSON:

```python
snapshot = client.get_configuration_typed()
transport = snapshot.transport
if transport and transport.streaming and transport.streaming.soranet:
    soranet = transport.streaming.soranet
    print("SoraNet exit:", soranet.exit_multiaddr)
    print("Provision queue cap:", soranet.provision_queue_capacity)
```

## UAID portfolio & manifests (NX-16)

The `NX-16` roadmap introduces UAID-level portfolio, bindings, and Space Directory manifest APIs.
The Torii client exposes typed helpers so wallets and automation scripts can hit the endpoints
without bespoke parsing, including optional `asset_id` filtering for portfolio reads:

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080", auth_token="admin-token")
uaid_literal = "aabb" * 16  # raw hex (LSB=1) accepted; helper normalises to `uaid:<hex>`

portfolio = client.get_uaid_portfolio_typed(uaid_literal, asset_id="norito:<portfolio-asset-id-hex>")
print("UAID", portfolio.uaid, "positions", portfolio.total_positions)
for dataspace in portfolio.dataspaces:
    for account in dataspace.accounts:
        for asset in account.assets:
            print(dataspace.dataspace_alias, account.account_id, asset.asset_id, asset.quantity)

bindings = client.get_uaid_bindings_typed(uaid_literal)
for slice in bindings.dataspaces:
    print(slice.dataspace_alias, slice.accounts)

manifests = client.list_space_directory_manifests_typed(
    uaid_literal,
    dataspace=11,
    status="active",
)
for record in manifests.manifests:
    print(record.dataspace_alias, record.status, record.manifest_hash)

# Publish or revoke capability manifests directly from Python.
client.publish_space_directory_manifest(
    {
        "authority": "soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
        "privateKeyHex": "ed0123...",
        "manifest": manifest_payload,  # matches AssetPermissionManifest JSON
        "reason": "CBDC onboarding wave",
    }
)
client.revoke_space_directory_manifest(
    {
        "authority": "soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
        "privateKeyBytes": bytes.fromhex("11" * 32),
        "uaid": uaid_literal,
        "dataspace": 11,
        "revokedEpoch": 9216,
        "reason": "deny-wins drill",
    }
)
```

All helpers accept raw hex (LSB=1) or `uaid:<hex>` literals, normalise query parameters, and
return rich dataclasses (`UaidPortfolioSnapshot`, `UaidBindingsSnapshot`,
`SpaceDirectoryManifestList`) so callers can render dashboards or build evidence bundles for the
NX-16 rollout with deterministic parsing.

## Trigger lifecycle walkthrough

```python
from iroha_python import Instruction, ToriiClient

client = ToriiClient("http://127.0.0.1:8080", auth_token="admin-token")
trigger_id = "hourly-reward"

# 1) Build the instruction with the high-level helper.
register = Instruction.register_time_trigger(
    trigger_id=trigger_id,
    authority="soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
    action=Instruction.mint_asset(
        asset_id="norito:<reward-asset-id-hex>",
        account_id="soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
        value=1,
    ),
    interval_ms=3_600_000,
    repeats=None,
)

# 2) Submit the transaction and wait for confirmation.
envelope, status = client.build_and_submit_transaction(
    chain_id="local",
    authority="soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ",
    private_key="ed25519:...",
    instructions=[register],
    wait=True,
)
assert status["kind"] == "Committed"

# 3) Inspect the registered trigger via REST.
details = client.get_trigger(trigger_id)
print(details["status"])

# 4) Stream trigger execution events with the typed filter.
for event in client.stream_trigger_events(trigger_id=trigger_id, resume=True):
    print("Trigger event:", event)
    break  # demonstration

# 5) Query triggers with pagination helpers.
page = client.query_triggers(filter={"authority": "soraゴヂアニヤナサヰイユヶサヲワニュスゥァヨワコモペバプボチョナソヒョニュニョムベイゴエホタフナナハカウセミカ"}, limit=10)
for item in page["items"]:
    print(item["id"])

# 6) Unregister the trigger when no longer needed.
client.delete_trigger(trigger_id)
```

## Pipeline monitoring & SSE playbook

```python
from iroha_python import DataEventFilter, ToriiClient

client = ToriiClient("http://127.0.0.1:8080", auth_token="admin-token")

# Batched history: inspect the latest committed blocks.
recent_blocks = client.list_blocks(limit=5)
print([row["height"] for row in recent_blocks.get("items", [])])

# Detailed recovery snapshot for a specific height.
sidecar = client.get_pipeline_recovery(height=42)
print(sidecar.get("transactions", []))

# Subscribe to pipeline transaction events (Queued → Executed → Committed).
filter_obj = DataEventFilter.pipeline_transaction(status="Queued")
for event in client.stream_pipeline_transactions(filter_obj, resume=True):
    tx = event["payload"]
    print("Queued tx:", tx["hash_hex"], tx["status"]["kind"])
    break

# Watch committed blocks and snapshot metadata.
for block_event in client.stream_pipeline_blocks(status="Committed"):
    block = block_event["payload"]
    print("Committed block", block["height"], block["hash_hex"])
    break

# Resume SSE consumption using the last event id exposed by the client helper.
resume_filter = DataEventFilter.pipeline_witness(epoch=0)
client.stream_pipeline_witnesses(
    filter=resume_filter,
    last_event_id="opaque-event-id",
    resume=True,
    on_event=lambda payload, eid: print("Witness", payload["id"], eid),
)
```

## Consensus telemetry snapshot

```python
from iroha_python import ToriiClient

client = ToriiClient("http://127.0.0.1:8080", auth_token="admin-token")
telemetry = client.get_sumeragi_telemetry_typed()

print("Votes ingested:", telemetry.availability.total_votes_ingested)
for collector in telemetry.availability.collectors:
    print("Collector", collector.collector_idx, collector.peer_id, collector.votes_ingested)

for entry in telemetry.qc_latency_ms:
    print(entry.kind, "EMA (ms):", entry.last_ms)

print("RBC backlog sessions:", telemetry.rbc_backlog.pending_sessions)
if telemetry.vrf.found:
    print("Active VRF epoch:", telemetry.vrf.epoch, "seed:", telemetry.vrf.seed_hex)
```

> Developer tip: set `IROHA_PYTHON_CONNECT_CODEC=stub` when running the unit-test
> suite locally to bypass the native Connect codec. The stub keeps the helpers
> testable without compiling the `iroha_python._crypto` extension; production
> binaries must run without this flag.

## Integration tests

The SDK ships an opt-in integration harness that exercises runtime and metadata
endpoints against a live Torii node. The Python helper spins up the single-node
docker-compose topology, waits for the API to become available, and runs
`pytest` with the `integration` marker:

```bash
python python/iroha_python/scripts/run_integration.py
```

Use the shell wrapper for convenience:

```bash
python/iroha_python/scripts/run_integration.sh
```

Harness options are available as CLI flags (see `--help`). Common environment
variables:

| Variable | Purpose |
|----------|---------|
| `START_TORII` | Set to `0` to reuse an existing node instead of starting docker compose. |
| `COMPOSE_FILE` | Override the compose file (defaults to `defaults/docker-compose.single.yml`). |
| `COMPOSE_SERVICE` | Service name to start (defaults to `irohad0`). |
| `IROHA_TORII_URL` | Torii URL used by the tests (defaults to `http://127.0.0.1:8080`). |

When running against an external environment, set `--no-start`,
`--torii-url` (or `IROHA_TORII_URL`), and optional auth tokens
(`IROHA_TORII_AUTH_TOKEN`, `IROHA_TORII_API_TOKEN`) before invoking `pytest -m
integration`.

## Norito RPC helper

Use `NoritoRpcClient` to call Torii endpoints that speak the Norito RPC surface.
The helper wraps `requests.Session`, automatically attaches Norito headers, and
shares retry/auth configuration with the HTTP client.

```python
from iroha_python.norito_rpc import NoritoRpcClient, NoritoRpcConfig

config = NoritoRpcConfig(base_url="http://127.0.0.1:8080")
with NoritoRpcClient(config) as rpc:
    response_bytes = rpc.call("/v1/pipeline/transactions", payload_bytes)
```

Override headers, query parameters, or target URLs per call via keyword
arguments. The pytest suite (`tests/test_norito_rpc.py`) provides additional
usage examples.

### Norito RPC smoke tests

Release automation and local workflows can run the targeted Norito RPC parity
suite (header/method coverage) with:

```bash
python/iroha_python/scripts/run_norito_rpc_smoke.sh
```

Set `PYTHON_BIN=/path/to/python` to exercise a specific interpreter (for
example, the virtualenv created by `scripts/release_smoke.sh`). The script
requires `pytest` and exits non-zero on any regression so Norito RPC helpers
stay aligned with Torii.

## Norito fixtures

Android remains the canonical source of Norito fixtures. To keep the Python copy
in sync, mirror the artifacts with:

```bash
./scripts/python_fixture_regen.sh
```

This script copies `.norito` payloads and supporting JSON manifests from
`java/iroha_android/src/test/resources` into
`python/iroha_python/tests/fixtures`. CI (and local workflows) can verify parity
via:

```bash
python3 scripts/check_python_fixtures.py --quiet
```

Both scripts accept `PYTHON_FIXTURE_SOURCE`/`PYTHON_FIXTURE_OUT` (regen) and
`--source`/`--target` (check) to point at alternate directories during local
iterations.
A `make python-fixtures` target runs the regeneration and parity check in one step.

The regeneration script also writes cadence metadata to
`artifacts/python_fixture_regen_state.json`, mirroring the Swift/Android fixture
rotation. Override `PYTHON_FIXTURE_STATE_FILE`, `PYTHON_FIXTURE_ROTATION_OWNER`,
and `PYTHON_FIXTURE_CADENCE` to point at alternate paths or update the rotation
labels when wiring Cron jobs. A weekly automation run can chain the regen and
parity check:

```bash
PYTHON_FIXTURE_ROTATION_OWNER=sdk-python \
PYTHON_FIXTURE_CADENCE=weekly-wed-1700utc \
  ./scripts/python_fixture_regen.sh
python3 scripts/check_python_fixtures.py --quiet
```

**Fixture regen SLA (<48h)**

1. When the Android SDK updates the canonical fixtures, pull the changes (or rebuild via `scripts/export_norito_fixtures`) and run `./scripts/python_fixture_regen.sh`.
2. Verify parity with `python3 scripts/check_python_fixtures.py --quiet` (also covered by `make python-fixtures`).
3. Commit the regenerated files under `python/iroha_python/tests/fixtures/` alongside the updated manifest.
4. Before opening a PR, execute `./python/iroha_python/scripts/run_checks.sh` so lint/type/tests and the fixture parity guard all pass.

### Release smoke test

Before publishing, exercise the release smoke pipeline:

```bash
bash python/iroha_python/scripts/release_smoke.sh
```

(`make python-release-smoke` dispatches to the same script.)

The workflow now:

1. Builds the wheel with `python -m build` and installs it into a fresh virtualenv for an import smoke test plus the Norito RPC parity suite.
2. Runs `twine check` followed by a `twine upload --dry-run` call so PyPI metadata and credentials are validated ahead of time.
3. Emits `dist/CHANGELOG_PREVIEW.md`, summarising commits since the latest tag, alongside a deterministic `SHA256SUMS` file.
4. Generates an ephemeral RSA key to produce detached signatures (`*.sig`/`*.pub`) for the wheel so downstream automation can rehearse the provenance flow without the production key.
5. Writes `dist/release_artifacts.json` capturing the wheel, hashes, change-log preview, and signature metadata so release automation can archive provenance data in a single artefact.

Set `PYTHON_RELEASE_SIGNING_KEY=/path/to/production.pem` (or pass `--signing-key`) to reuse the real signing key instead of the ephemeral default. Use `--manifest-out <path>` if you need the manifest in a different location.

When running locally, set `PYTHON_RELEASE_SMOKE_KEEP_DIST=1` to preserve the artefacts under `dist/` after the smoke completes.

For details on choosing between binary bundles and container images, consult `docs/source/release_artifact_selection.md`.

The artefacts live under `python/iroha_python/dist/` for the duration of the run; capture them before the script exits if you need to share the preview or signatures with reviewers.

The smoke bundle also signs the generated changelog preview with the same key used for the wheel. Verify it locally after the smoke run with:

```bash
PUB_PATH=$(ls python/iroha_python/dist/*.pub)
openssl dgst -sha256 \
  -verify "${PUB_PATH}" \
  -signature python/iroha_python/dist/CHANGELOG_PREVIEW.md.sig \
  python/iroha_python/dist/CHANGELOG_PREVIEW.md
```

`release_artifacts.json` records the changelog checksum/signature alongside the wheel so release managers can archive the evidence without extra scripting.

### Support policy & cadence (PY6-P3)

- Critical security fixes ship within **48 hours** of confirmation; high-severity defects ship within **5 business days**; maintenance issues land in the next scheduled release (≤30 days).
- Maintain at least **18 months** of overlapping LTS branches and regenerate SBOM/provenance bundles for every supported train.
- Release packets must include the signed changelog preview (`CHANGELOG_PREVIEW.md` + `.sig`), `SHA256SUMS`, and `release_artifacts.json` produced by `scripts/release_smoke.sh` so publishing/rollback drills have reproducible evidence.
- Publish changelog entries and release notes alongside the roadmap pointers to keep parity with Rust/Android/JS/Swift cadence expectations.


## Testing

Run the Python lint/type-check/test suite with:

```bash
./python/iroha_python/scripts/run_checks.sh
```
`make python-checks` is a convenience wrapper around the same script.

This command runs `ruff`, `mypy`, `pytest`, and the fixture parity check so the SDK stays in sync with the canonical Norito artifacts.
extras first (`pip install -e python/iroha_python[dev]`) to pull in the required

This command runs `ruff`, `mypy`, `pytest`, the fixture parity check, and (optionally) the release smoke test so the SDK stays in sync with the canonical Norito artifacts.
Install the dev extras first (`pip install -e python/iroha_python[dev]`) to pull in the required tools. Set `SKIP_LINT=1`, `SKIP_TESTS=1`, or `SKIP_FIXTURES=1` to skip individual phases while iterating locally.
iterating locally.

Run the Rust unit tests for the bindings with:

```bash
./python/iroha_python/scripts/test_rs.sh
```

The helper script wraps `cargo test -p iroha_python_rs`, automatically loading a
local CPython runtime when needed.

The script first looks for an explicit path in
`python/iroha_python/iroha_python_rs/python-runtime-path`. If that file is
absent, it tries to auto-detect the shared library by querying `${PYTHON_BIN:-python3}`
via `sysconfig`. Set `PYTHON_BIN` to point at a specific interpreter (for
example, a virtualenv) before running the script if you need to override the
default.

### macOS runtime configuration

When running tests on macOS the binary embeds CPython directly. If your system
Python does not expose the shared library globally (for example, the
Xcode-provided interpreter), create a `python-runtime-path` file alongside
`python/iroha_python/iroha_python_rs/Cargo.toml` containing the absolute path to
the CPython dynamic library. The `test_rs.sh` wrapper reads this file and sets
the necessary dynamic loader environment variables for you. Lines starting with
`#` are treated as comments, so the file can also include short notes.

Example `python-runtime-path`:

```
# Path to Python3 shared library used by tests
/Applications/Xcode.app/Contents/Developer/Library/Frameworks/Python3.framework/Versions/3.9/Python3
```

You can auto-populate the file with the discovered shared library by running:

```bash
python python/iroha_python/scripts/update_python_runtime_path.py
```

The helper mirrors the discovery logic used in `build.rs`, consulting
`IROHA_PYTHON_RUNTIME_PATH` first and falling back to `sysconfig`.

## Configuration & overrides

`resolve_torii_client_config` keeps Python clients aligned with the operational
policy embedded in `iroha_config`. It merges (1) the parsed config file, (2)
developer overrides supplied via environment variables, and (3) inline overrides
passed directly to the resolver/`create_torii_client`. The following environment
variables are available for local tweaking:

| Variable | Purpose |
|----------|---------|
| `IROHA_TORII_TIMEOUT_MS` | Request timeout in milliseconds |
| `IROHA_TORII_MAX_RETRIES` | Maximum retry attempts |
| `IROHA_TORII_BACKOFF_INITIAL_MS` | Initial retry backoff delay |
| `IROHA_TORII_BACKOFF_MULTIPLIER` | Exponential backoff multiplier |
| `IROHA_TORII_MAX_BACKOFF_MS` | Maximum backoff delay |
| `IROHA_TORII_RETRY_STATUSES` | Comma separated HTTP status codes that should be retried |
| `IROHA_TORII_RETRY_METHODS` | Comma separated HTTP methods eligible for retries |
| `IROHA_TORII_AUTH_TOKEN` | Default `Authorization: Bearer …` header |
| `IROHA_TORII_API_TOKEN` | Default `X-API-Token` header |

Environment overrides are meant for development convenience; production nodes
should rely on the canonical `iroha_config`.

The test harness automatically loads this library when the file is present, so
no environment variables need to be exported.

## Current scope

- Re-export the maintained Norito codec (`iroha-norito`) so payload encoding and
  decoding stay consistent with Rust fixtures.
- Provide a convenient constructor for the Torii HTTP client used to manage
  attachments and prover reports.
- Expose Ed25519 key generation (random/seeded), private-key import (bytes/hex),
  multihash/account-id helpers, signing, verification, and Blake2b-256 hashing
  through a PyO3 bridge to `iroha_crypto`.
- Expose SM2 helpers (`generate_sm2_keypair`, `derive_sm2_keypair_from_seed`,
  `load_sm2_keypair`, `sign_sm2`, `verify_sm2`, `sm2_public_key_multihash`) and surface
  the canonical deterministic fixture via `sm2_fixture_from_seed` so SDK parity
  tests can assert the shared seed/distid/ZA/signature bytes even when the native
  module is unavailable (falls back to the bundled vector).
- Provide confidential key-derivation helpers (`derive_confidential_keyset`,
  hex variants, and a `ConfidentialKeyset` wrapper) so wallets can obtain
  `nk`/`ivk`/`ovk`/`fvk` alongside the spend key locally.
- Offer a `TransactionBuilder` wrapper for constructing and signing canonical
  transactions (bare + versioned Norito bytes) with signature/hash inspection
  helpers plus dict/JSON export/import helpers for envelopes.
- Provide Python-friendly instruction constructors (register domain/account,
  mint/transfer numeric assets) to assemble manifests without raw JSON, plus
  `Instruction.from_json`/`Instruction.to_json` helpers for full Norito
  coverage when bespoke wrappers are unnecessary.
- Expose typed wrappers for `DomainId`, `AccountId`, `AssetDefinitionId`, and
  `AssetId` so Python code can compose Norito payloads with on-chain
  identifiers while preserving Rust validation semantics.
- Extend the Torii client with configurable retries/auth headers, JSON helper
  methods for `/v1/status`, `/v1/health`, `/v1/configuration`, `/v1/metrics`,
  and block queries, laying the groundwork for optional gRPC parity.
- Extend the Torii client with governance helpers (proposal deployment, ballot
  submission, referendum status) so clients can orchestrate governance flows
  without hand-crafted HTTP requests.
- Include configuration helpers (`get_confidential_gas_schedule`,
  `set_confidential_gas_schedule`) so operators can inspect or update the
  confidential verification gas schedule without manually building DTO payloads.
- Expose administrative helpers for configuration updates, peer discovery
  (typed `PeerInfo` via `list_peers_typed`), network time introspection
  (`NetworkTimeSnapshot`/`NetworkTimeStatus`), and runtime metadata
  (`NodeCapabilities`, `RuntimeMetrics`, `RuntimeAbiActive`, `RuntimeAbiHash`) so
  operators can script parity checks without massaging raw JSON from
  `/v1/configuration`, `/v1/peers`, `/v1/time/{now,status}`, `/v1/node/capabilities`,
  and `/v1/runtime/*`.
- Add typed runtime upgrade helpers (`list_runtime_upgrades_typed`, the
  `RuntimeUpgrade*` dataclasses, and typed wrappers around `propose_runtime_upgrade`,
  `activate_runtime_upgrade`, `cancel_runtime_upgrade`) to inspect manifests and
  instruction bundles without hand-decoding JSON payloads.
- Provide trigger helpers for registering time/pre-commit actions, executing triggers, and
  minting/burning/unregistering repetitions so automation flows avoid manual Norito. Typed trigger
  listings (`TriggerRecord`, `TriggerListPage`) and mutation responses
  (`TriggerMutationResponse` via `register_trigger_typed`/`delete_trigger_typed`) surface structured results and
  governance drafts for `/v1/triggers`, `/v1/triggers/query`, and the lifecycle endpoints.
- Add Torii trigger lifecycle wrappers (`register_trigger`, `register_trigger_typed`, `query_triggers`,
  `delete_trigger`, `delete_trigger_typed`, `get_trigger`, `list_triggers`)
  so automation flows can manage schedules directly from Python while validating governance payloads when desired.
- Add typed account asset/transaction listings (`list_account_assets`, `list_account_transactions`) and JSON query helpers
  (`query_account_assets`, `query_account_transactions`) to cover the remaining Torii account endpoints.
- Provide typed query and list wrappers for accounts/domains/asset definitions/holders/permissions
  (`query_accounts_typed`, `list_accounts_typed`, `query_domains_typed`, `list_domains_typed`,
  `query_asset_definitions_typed`, `list_asset_definitions_typed`, `query_asset_holders_typed`,
  `list_asset_holders_typed`, `list_account_permissions_typed`) so pagination metadata and core
  fields (ids, ownership, balances, permission payloads) are validated before reaching downstream automation.
- Offer event filter builders (verifying key, proof, trigger) plus streaming helpers so Torii SSE integrations avoid hand-crafted JSON payloads.
- Extend the Torii client with consensus telemetry helpers covering `/v1/sumeragi/telemetry`,
  RBC status/delivery snapshots, and authenticated sampling (`/v1/sumeragi/rbc/sample` with
  `X-API-Token`) for operator tooling.
- Surface pipeline recovery sidecars (`/v1/pipeline/recovery/{height}`), Sumeragi evidence listing/counting,
  and pipeline/witness event filters with streaming helpers so Python operators can monitor ledger history
  without reimplementing the Rust toolchain.
- Extend the Torii client with transaction submission/status helpers so signed
  envelopes can be delivered directly to `/v1/pipeline/transactions`.
- Provide a `submit_transaction_envelope_and_wait` helper that submits a signed
  envelope and polls Torii until the transaction reaches a terminal status.
- Add `build_signed_transaction` so callers can assemble + sign a transaction
  in one step using high-level instruction helpers.
- Provide `ToriiClient.build_and_submit_transaction` to construct, submit, and
  optionally await transaction finalization with a single call, returning the
  envelope as an object, dict, or JSON string; `submit_transaction_json`
  accepts an envelope JSON payload directly.
- Ship a lightweight CLI helper (`python/iroha_python/bin/submit_envelope_json.py`)
  for replaying JSON envelopes from stdin or a file via the Torii client.
- Include structured account query envelopes plus Torii helpers for
  `/v1/accounts/query`, `/v1/accounts/{id}/assets`, and
  `/v1/accounts/{id}/transactions`, alongside a basic SSE consumer for
  `/v1/events/sse` streams with Last-Event-ID resume. A lightweight filter DSL
  (`Eq`, `Between`, `metadata_eq`, `metadata_exists`, `field_in`, …) keeps
  payloads deterministic without hand-crafted JSON.
- Offer a `wait_for_transaction_status` helper that polls pipeline status until
  success or failure with configurable intervals, terminal-state handling, and
  callbacks for UI progress indicators.
- Contracts API wrappers (`/v1/contracts/code`, `/v1/contracts/deploy`,
  `/v1/contracts/code-bytes/{hash}`, `/v1/contracts/instances/{ns}`) round out
  the Torii surface for manifest management.
- Ship optional Norito RPC helpers (`iroha_python.norito_rpc`) so callers can
  invoke Norito-encoded RPC endpoints without vendor-specific transports.

## Next steps

- Expand the transaction builder with ergonomic instruction builders and account
  helpers so Python code can compose manifests without raw JSON payloads.
- Offer high-level APIs for transaction submission, queries, and event
  streaming.
- Extend the Norito RPC helper once the Nexus RPC services stabilise and add
  end-to-end fixtures for the Connect flows.

Contributions and feedback are welcome while the package is taking shape.
