---
lang: az
direction: ltr
source: docs/space-directory.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9494b87700653dc01e8e13d479c3e413d7291fcb60010dc1240bceaae3ee588e
source_last_modified: "2026-01-28T17:11:30.671427+00:00"
translation_last_reviewed: 2026-02-07
---

# Space Directory Operator Playbook

This playbook explains how to author, publish, audit, and rotate **Space
Directory** entries for Nexus dataspaces. It complements the architecture notes
in `docs/source/nexus.md` and the CBDC onboarding plan
(`docs/source/cbdc_lane_playbook.md`) by providing hands-on procedures,
fixtures, and governance templates.

> **Scope.** The Space Directory serves as the canonical registry for
> dataspace manifests, Universal Account ID (UAID) capability policies, and the
> audit trail that regulators rely on. While the backing contract is still
> under active development (NX-15), the fixtures and processes below are ready
> to wire into tooling and integration tests.

## 1. Core Concepts

| Term | Description | References |
|------|-------------|------------|
| Dataspace | Execution context/Lane that runs a governance-approved contract set. | `docs/source/nexus.md`, `crates/iroha_data_model/src/nexus/mod.rs` |
| UAID | `UniversalAccountId` (blake2b-32 hash) used to anchor cross-dataspace permissions. | `crates/iroha_data_model/src/nexus/manifest.rs` |
| Capability Manifest | `AssetPermissionManifest` describing deterministic allow/deny rules for a UAID/dataspace pair (deny wins). | Fixture `fixtures/space_directory/capability/*.manifest.json` |
| Dataspace Profile | Governance + DA metadata published alongside manifests so operators can reconstruct validator sets, composability whitelists, and audit hooks. | Fixture `fixtures/space_directory/profile/cbdc_lane_profile.json` |
| SpaceDirectoryEvent | Norito-encoded events emitted when manifests activate/expire/revoke. | `crates/iroha_data_model/src/events/data/space_directory.rs` |

## 2. Manifest Lifecycle

Space Directory enforces **epoch-based lifecycle management**. Every change
produces a signed manifest bundle plus an event:

| Event | Trigger | Required Actions |
|-------|---------|------------------|
| `ManifestActivated` | New manifest reaches `activation_epoch`. | Broadcast bundle, update caches, archive governance approval. |
| `ManifestExpired` | `expiry_epoch` passes without renewal. | Notify operators, sweep UAID handles, ready replacement manifest. |
| `ManifestRevoked` | Emergency deny-wins decision before expiry. | Revoke UAID immediately, emit incident report, schedule follow-up governance review. |

Subscribers should use `DataEventFilter::SpaceDirectory` to watch specific
dataspaces or UAIDs. Example filter (Rust):

```rust
use iroha_data_model::events::data::filters::SpaceDirectoryEventFilter;

let filter = SpaceDirectoryEventFilter::new()
    .for_dataspace(11u32.into())
    .for_uaid("uaid:0f4d…ab11".parse().unwrap());
```

## 3. Operator Workflow

| Phase | Owner(s) | Steps | Evidence |
|-------|----------|-------|----------|
| Draft | Dataspace owner | Clone fixture, edit allowances/governance, run `cargo test -p iroha_data_model nexus::manifest`. | Git diff, test log. |
| Review | Governance WG | Validate manifest JSON + Norito bytes, sign decision log. | Signed minutes, manifest hash (BLAKE3 + Norito `.to`). |
| Publish | Lane ops | Submit via CLI (`iroha app space-directory manifest publish`) using either a Norito `.to` payload or raw JSON **or** POST `/v1/space-directory/manifests` with the manifest JSON + optional reason, verify Torii response, capture `SpaceDirectoryEvent`. | CLI/Torii receipt, event log. |
| Expire | Lane ops / Governance | Run `iroha app space-directory manifest expire` (UAID, dataspace, epoch) when a manifest reaches its scheduled end-of-life, verify `SpaceDirectoryEvent::ManifestExpired`, archive binding cleanup evidence. | CLI output, event log. |
| Revoke | Governance + Lane ops | Run `iroha app space-directory manifest revoke` (UAID, dataspace, epoch, reason) **or** POST `/v1/space-directory/manifests/revoke` with the same payload to Torii, verify `SpaceDirectoryEvent::ManifestRevoked`, update evidence bundle. | CLI/Torii receipt, event log, ticket note. |
| Monitor | SRE/Compliance | Track telemetry + audit logs, set alerts for revocations/expiry. | Grafana screenshot, archived logs. |
| Rotate/Revoke | Lane ops + Governance | Stage replacement manifest (new epoch), run tabletop, file incident (if revoke). | Rotation ticket, incident postmortem. |

All artefacts for a rollout go under `artifacts/nexus/<dataspace>/<timestamp>/`
with a checksum manifest to satisfy regulator evidence requests.

### 3.1 Audit bundle automation

Use `iroha app space-directory manifest audit-bundle` to assemble the evidence pack for
each capability manifest. The helper accepts either the JSON or Norito payload,
plus the dataspace profile JSON, and emits a self-contained bundle:

```bash
iroha app space-directory manifest audit-bundle \
  --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
  --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
  --out-dir artifacts/nexus/cbdc/2026-02-01T00-00Z \
  --notes "CBDC -> wholesale rotation drill"
```

The command writes the canonical `manifest.json`, Norito bytes (`manifest.to`),
and hash (`manifest.hash`), copies the dataspace profile, and produces
`audit_bundle.json` summarising the UAID, dataspace id, activation/expiry
epochs, manifest hash, audit hooks, generation timestamp, and optional notes.
Profiles that omit `audit_hooks.events` or forget to subscribe to both
`SpaceDirectoryEvent.ManifestActivated` and `SpaceDirectoryEvent.ManifestRevoked`
are rejected up front so compliance can rely on the bundle without manual
linting. Drop the bundle directory inside `artifacts/nexus/<dataspace>/<stamp>/`
so the regulator packet includes the same bytes the CLI emitted.

### 3.2 Manifest & profile scaffolding

Roadmap item NX-16 calls for deterministic manifest/profile scaffolds so
operators and SDK automation can bootstrap UAID capability bundles without
hand-editing fixtures. Use `iroha app space-directory manifest scaffold` to emit a
pair of JSON templates (manifest + dataspace profile) that already match the
Space Directory schema and subscribe to the required audit hooks:

```bash
iroha app space-directory manifest scaffold \
  --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
  --dataspace 11 \
  --activation-epoch 4097 \
  --manifest-out artifacts/nexus/cbdc/scaffold/manifest.json \
  --profile-out artifacts/nexus/cbdc/scaffold/profile.json \
  --allow-program cbdc.transfer \
  --allow-method transfer \
  --allow-asset cbdc#centralbank \
  --allow-role initiator \
  --allow-max-amount 500000000 \
  --allow-window per-day \
  --deny-program cbdc.kit \
  --deny-method withdraw \
  --deny-reason "Withdrawals disabled for this UAID." \
  --profile-governance-issuer i105... \
  --profile-governance-ticket gov-2026-02-rotation \
  --profile-validator i105... \
  --profile-validator i105... \
  --profile-da-attester i105...
```

The command writes `manifest.json` + `profile.json` (defaults to
`artifacts/space_directory/scaffold/<dataspace>_<activation>/`) and prints the
paths so release playbooks can link the generated artefacts directly. Supply the
optional allow/deny flags to pre-populate rules; unspecified fields retain the
same placeholders as the curated fixtures, and both outputs include the mandated
`SpaceDirectoryEvent.ManifestActivated/Revoked` audit hook subscriptions so they
pass the same validation checks as production manifests. Edit the generated files
in-place, re-run `manifest encode/audit-bundle` for evidence, and commit them to
source control once governance approves the UAID dataspace binding.

## 4. Manifest Template & Fixtures

Use the curated fixtures as the canonical schema reference. The wholesale CBDC
sample (`fixtures/space_directory/capability/cbdc_wholesale.manifest.json`) has
both allow and deny entries:

```json
{
  "version": 1,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "issued_ms": 1762723200000,
  "activation_epoch": 4097,
  "expiry_epoch": 4600,
  "entries": [
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.transfer",
        "method": "transfer",
        "asset": "CBDC#centralbank",
        "role": "Initiator"
      },
      "effect": {
        "Allow": {
          "max_amount": "500000000",
          "window": "PerDay"
        }
      },
      "notes": "Wholesale transfer allowance (per UAID, per day)."
    },
    {
      "scope": {
        "dataspace": 11,
        "program": "cbdc.kit",
        "method": "withdraw"
      },
      "effect": {
        "Deny": {
          "reason": "Withdrawals disabled for this UAID."
        }
      },
      "notes": "Deny wins over any preceding allowance."
    }
  ]
}
```

Key rules:

- **Deny wins.** Place explicit denies after matching allow entries so operators
  can reason about precedence.
- **Deterministic amounts.** Keep `max_amount` as a decimal string to avoid
  float ambiguities when `Numeric` parses the value.
- **Scheduler-backed expiry.** The core runtime now expires manifests
  automatically once the block height reaches `expiry_epoch`, emitting
  `SpaceDirectoryEvent::ManifestExpired`, bumping
  `nexus_space_directory_revision_total`, and rebinding UAIDs before Torii/CLI
  surfaces update. Use the CLI only for manual overrides or evidence that
  requires a backfilled expiry record.
- **Epoch gating.** Use `activation_epoch` + `expiry_epoch` to model rotation
  cadences. Emergency revocations emit `ManifestRevoked`.

The retail dApp manifest is available under
`fixtures/space_directory/capability/retail_dapp_access.manifest.json` to
exercise composability scenarios.

Each capability manifest ships with its Norito-encoded twin in the same
directory (for example,
`fixtures/space_directory/capability/cbdc_wholesale.manifest.to` with
BLAKE3 digest `11a47182ab51e845d53f40f12387caef1e609585a824c0a4feab38f0922859fe`).
The `.to` files are generated with `cargo xtask space-directory encode` and the
`cbdc_capability_manifests_enforce_policy_semantics` integration test asserts
that the JSON and Norito payloads remain in lockstep.

### 3.2 CLI publish evidence checklist

Publishing manifests directly from the CLI must leave the same audit trail as
Torii-assisted flows. Before running `iroha app space-directory manifest publish`,
stage the following evidence:

1. **Encode and hash.** Produce the `.to` payload with
   `cargo xtask space-directory encode` (or the Norito helper embedded in
   `iroha app space-directory manifest encode`) and capture the emitted BLAKE3
   digest. Store both outputs under
   `artifacts/nexus/<dataspace>/<timestamp>/manifest/`.
2. **Command log.** Save the full CLI invocation (arguments, commit hash, and
   pipeline context) to `cli.log` in the same artefact directory. This lets
   reviewers replay the transaction with identical parameters.
3. **Event payload.** Subscribe to `SpaceDirectoryEvent` before publishing and
   archive the resulting `ManifestActivated` JSON (or the error payload if the
   transaction fails). Include the event ID and manifest hash in the evidence
   manifest.
4. **Telemetry snapshots.** Immediately after the block lands, snapshot
   `nexus_space_directory_revision_total{dataspace="<id>"}` and
   `nexus_space_directory_active_manifests{dataspace="<id>"}` via Prometheus
   plus a Grafana screenshot (panels 10–12 in
   `dashboards/grafana/nexus_lanes.json`). These snapshots prove the ledger
   applied the revision.
5. **Bundle + sign-off.** Extend the audit bundle (see §3.1) with the CLI log,
   hash, event payload, and telemetry snippets so regulators and incident
   responders can validate the rollout without fetching ad hoc artefacts.

## 5. Ledger bindings

The canonical UAID ↔ dataspace assignments live inside the new
`World::uaid_dataspaces` map maintained by the Space Directory host. Each entry
stores a UAID, the dataspaces it was granted, and the concrete account IDs that
belong to that slice. Downstream components use this ledger-backed view:

- Torii’s `GET /v1/accounts/{uaid}/portfolio` groups holdings per dataspace
  using `uaid_dataspaces`.
- Host-side allowance enforcement and telemetry can look up the binding without
  reloading manifest JSON.
- Torii now exposes `GET /v1/space-directory/uaids/{uaid}` for operators and
  SDKs that need to introspect bindings directly. Append
  `canonical i105 output` if you need the i105 literals for QR
  payloads; I105 strings remain the default.【docs/source/torii/portfolio_api.md】

### 5.1 CLI manifest & binding inspectors

`iroha` now mirrors the Torii read surfaces so operators can capture signed
evidence without hand-rolling HTTP helpers. Use the new helpers after each
manifest change (or during audits) to capture deterministic JSON snapshots:

```bash
# Fetch the manifest inventory for a UAID, filtering to active entries
iroha app space-directory manifest fetch \
  --uaid uaid:0f4d…ab11 \
  --status active \
  --json-out artifacts/nexus/cbdc/2026-05-01T00Z/uaid_manifests.json

# Inspect the dataspace bindings and persist the response for auditors
iroha app space-directory bindings fetch \
  --uaid uaid:0f4d…ab11 \
  --json-out artifacts/nexus/cbdc/2026-05-01T00Z/uaid_bindings.json
```

Both commands honour the same query parameters as the Torii endpoints:
`--dataspace`, `--status`, `--limit`, `--offset`, and `--address-format`. The CLI
prints the JSON payload to stdout and, when `--json-out` is supplied, stores the
pretty-printed response so audit bundles, tabletop drills, and governance
reviews can reference the exact payload that Torii served. This mirrors roadmap
item **NX-16**’s requirement for automated enforcement and evidence capture
across all dataspaces.

Whenever a manifest activates, add the `(uaid, dataspace, account)` tuple to
the map. When the manifest expires or is revoked, remove those tuples so the
ledger immediately reflects the current policy.

**Automation (NX‑16 update).** The core node now keeps the ledger map in sync:

- `SpaceDirectoryEvent::ManifestActivated` binds every ledger account tagged
  with the UAID to the dataspace; `Expired`/`Revoked` events clear the binding
  and mark the lifecycle fields for Torii introspection.
- When a new account registers with a UAID, the runtime immediately binds it to
  every currently-active dataspace entry (no extra CLI step required). Removing
  the account maintains the map as part of the unregister flow.

Operators still seed the manifest itself, but from this release onward Torii
and telemetry always see the fresh UAID↔dataspace view as soon as manifests
change state or new UAID accounts appear.

> **Upgrade note:** `iroha_core` v2 now runs a one-time ledger migration at
> startup that repopulates `World::uaid_dataspaces` from the canonical manifest
> registry whenever an older snapshot lacks the derived bindings. No manual
> CLI steps are required after upgrading nodes; the migration logs how many
> UAIDs were recovered and is guarded by a regression test so future schema
> tweaks keep the behavior deterministic.【crates/iroha_core/src/state.rs:5990】【crates/iroha_core/src/state.rs:2555】

**Admission enforcement.** Transactions signed by UAID-backed accounts are
routed using global UAID lookup. If a Space Directory manifest exists for the
target dataspace it must be active; missing target-dataspace manifests do not
trigger a binding-specific rejection. Inactive manifests are rejected through
the standard `LaneComplianceDenied` path with a deterministic reason string.

### Manifest introspection API

Torii exposes a read-only manifest inventory so operators can audit the exact
capability payload bound to a UAID:

```
GET /v1/space-directory/uaids/{uaid}/manifests?dataspace={id}
```

Use the optional `dataspace` query parameter (u64) to filter the response to a
single dataspace. `canonical i105 output` rewrites the embedded `accounts`
arrays to the Sora-specific `sora` encoding for wallet/UI parity. Each entry
includes the manifest hash, lifecycle metadata, ledger bindings, and the
canonical `AssetPermissionManifest` JSON:

```jsonc
{
  "uaid": "uaid:0f4d…ab11",
  "manifests": [
    {
      "dataspace_id": 11,
      "dataspace_alias": "cbdc",
      "manifest_hash": "12d486d6d3620a…f285a",
      "status": "Active",
      "lifecycle": {
        "activated_epoch": 4097,
        "expired_epoch": null,
        "revocation": null
      },
      "accounts": ["i105..."],
      "manifest": {
        "version": 1,
        "uaid": "uaid:0f4d…ab11",
        "dataspace": 11,
        "issued_ms": 1762723200000,
        "activation_epoch": 4097,
        "expiry_epoch": 4600,
        "entries": [
          {
            "scope": { "program": "cbdc.transfer", "method": "transfer" },
            "effect": { "Allow": { "max_amount": "500000000", "window": "PerDay" } }
          }
        ]
      }
    }
  ]
}
```

- `manifest_hash` is the Norito/Space Directory digest operators already record
  in governance proposals.
- `status` reflects the most recent lifecycle event (`Pending`, `Active`,
  `Expired`, `Revoked`), while `lifecycle.revocation` carries the epoch/reason
  for deny-wins drills.
- `accounts` reuse the `uaid_dataspaces` map so auditors can see exactly which
  ledger accounts the manifest currently reaches.
- `manifest` is the full `AssetPermissionManifest` object, allowing SDKs and
  dashboards to render the entries without bespoke schemas.

Access controls mirror the bindings and portfolio APIs (CIDR/API-token/fee
policy gates). Use this surface to verify manifest rotations, ensure revocation
evidence is visible to regulators, and hydrate SDK caches without scraping the
Space Directory contract directly.

### Manifest publish API

Operators can now publish manifests directly through Torii instead of relying on
the CLI. The HTTP surface mirrors the `PublishSpaceDirectoryManifest` ISI and
accepts a full `AssetPermissionManifest` payload expressed as JSON; an optional
`reason` string can be supplied to backfill `notes` on entries that omit one,
matching the CLI convenience flag.

```
POST /v1/space-directory/manifests
```

| Field | Type | Description |
|-------|------|-------------|
| `authority` | `AccountId` | Account that signs the publication transaction (must hold `CanPublishSpaceDirectoryManifest{dataspace}`). |
| `private_key` | `ExposedPrivateKey` | Wrapped private key matching `authority`. |
| `manifest` | `AssetPermissionManifest` | Canonical manifest payload (UAID, dataspace, lifecycle schedule, entries). |
| `reason` (optional) | `String` | Convenience string applied to `entries[*].notes` when missing. |

Sample request body:

```jsonc
{
  "authority": "i105...",
  "private_key": "ed25519:CiC7…",
  "manifest": {
    "version": 1,
    "uaid": "uaid:0f4d…ab11",
    "dataspace": 11,
    "issued_ms": 1762723200000,
    "activation_epoch": 4097,
    "expiry_epoch": 4600,
    "entries": [
      {
        "scope": {
          "dataspace": 11,
          "program": "cbdc.transfer",
          "method": "transfer",
          "asset": "CBDC#centralbank"
        },
        "effect": {
          "Allow": { "max_amount": "500000000", "window": "PerDay" }
        }
      }
    ]
  },
  "reason": "CBDC onboarding wave 4"
}
```

Torii responds with `202 Accepted` as soon as the transaction is queued. When
the block executes, `SpaceDirectoryEvent::ManifestActivated` fires (subject to
`activation_epoch`), bindings are rebuilt automatically, and the manifest
inventory endpoint reflects the new payload. Access controls mirror the other
Space Directory write APIs (CIDR/API-token/fee-policy gating).

### Manifest revocation API

Emergency revocations no longer require shelling out to the CLI: operators can
POST directly to Torii to enqueue the canonical `RevokeSpaceDirectoryManifest`
instruction. The submitting account must hold
`CanPublishSpaceDirectoryManifest { dataspace }`, matching the CLI workflow.

```
POST /v1/space-directory/manifests/revoke
```

| Field | Type | Description |
|-------|------|-------------|
| `authority` | `AccountId` | Account that signs the revocation transaction. |
| `private_key` | `ExposedPrivateKey` | Base64-wrapped private key used by Torii to sign on behalf of `authority`. |
| `uaid` | `String` | UAID literal (`uaid:<hex>` or raw 64-char hex digest, LSB=1). |
| `dataspace` | `u64` | Dataspace identifier that hosts the manifest. |
| `revoked_epoch` | `u64` | Epoch (inclusive) when the revocation should take effect. |
| `reason` | `Option<String>` | Optional audit trail message stored alongside the lifecycle data. |

Sample JSON body:

```jsonc
{
  "authority": "i105...",
  "private_key": "ed25519:CiC7…",
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "dataspace": 11,
  "revoked_epoch": 9216,
  "reason": "Fraud investigation #NX-16-R05"
}
```

Torii returns `202 Accepted` once the transaction enters the queue. When the
block executes you will receive `SpaceDirectoryEvent::ManifestRevoked`,
`uaid_dataspaces` is rebuilt automatically, and both `/portfolio` and the
manifest inventory start reporting the revoked state immediately. CIDR and
fee-policy gates match the read endpoints.

## 6. Dataspace Profile Template

Profiles capture everything a new validator needs before connecting. The
`profile/cbdc_lane_profile.json` fixture documents:

- Governance issuer/quorum (`i105...` + evidence ticket ID).
- Validator set + quorum and protected namespaces (`cbdc`, `gov`).
- DA profile (class A, attester roster, rotation cadence).
- Composability group ID and whitelist linking UAIDs to capability manifests.
- Audit hooks (event list, log schema, PagerDuty service).

Reuse the JSON as a starting point for new dataspaces and update the whitelist
paths to point at the relevant capability manifests.

## 7. Publishing & Rotation

1. **Encode UAID.** Derive the blake2b-32 digest and prefix with `uaid:`.

   ```bash
   python3 - <<'PY'
   import hashlib, binascii
   seed = bytes.fromhex("0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11")
   print("uaid:" + hashlib.blake2b(seed, digest_size=32).hexdigest())
   PY
   ```

2. **Encode Norito payload.**

   ```bash
   cargo xtask space-directory encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

   Or run the CLI helper (writes both the `.to` file and a `.hash` with the
   BLAKE3-256 digest):

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/cbdc_wholesale.manifest.json \
     --out artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to
   ```

3. **Publish through Torii.**

   ```bash
   # If you already encoded the Norito payload
   iroha app space-directory manifest publish \
     --manifest artifacts/nexus/cbdc/manifest/cbdc_wholesale.manifest.to

   # Or publish directly from the JSON manifest
   iroha app space-directory manifest publish \
     --manifest-json fixtures/space_directory/capability/cbdc_wholesale.manifest.json
   ```

   The submitting account (from `client.toml`) must hold
   `CanPublishSpaceDirectoryManifest { dataspace: 11 }`. The CLI appends the
   instruction to your transaction pipeline, so `--input/--output` still work
   for batching. Publishing immediately activates the manifest (emitting
   `SpaceDirectoryEvent::ManifestActivated`) and rebinds every UAID account to
   the dataspace using the deny-win manifest rules.

4. **Monitor events + telemetry.**

   Subscribe to `SpaceDirectoryEvent` and ensure the Grafana dashboard
   (`dashboards/grafana/nexus_lanes.json`) records the activation.

5. **Rotation checklist.**

   - Stage manifest with `activation_epoch = current_epoch + 2`.
   - Run programmable-money tabletop with both manifests.
   - Capture the `nexus_space_directory_revision_total` delta for the evidence
     bundle. Snapshot the Prometheus counter before and after the rotation
     (e.g., `curl -s "$PROM_URL/api/v1/query?query=nexus_space_directory_revision_total{dataspace=\"cbdc\"}"`)
     and store the two values plus their difference in
     `artifacts/nexus/<dataspace>/<timestamp>/telemetry.json`.

# Revoking a Manifest

When governance requires an emergency deny or scheduled rotation, revoke the
existing manifest before publishing the replacement. Gather the UAID, dataspace,
and revocation epoch from the manifest catalog, then run:

```bash
iroha app space-directory manifest revoke \
  --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
  --dataspace 11 \
  --revoked-epoch 4610 \
  --reason "Emergency deny request #INC-2026-42"
```

This emits `SpaceDirectoryEvent::ManifestRevoked`, clears UAID bindings, and
records the audit reason in the lifecycle metadata. Capture the CLI output and
event log in the evidence bundle before publishing the successor manifest.

# Expiring a Manifest

Scheduled expiries now run automatically: when the current block height reaches
`expiry_epoch`, the runtime emits `SpaceDirectoryEvent::ManifestExpired`,
rebuilds UAID bindings, and increments the
`nexus_space_directory_revision_total{dataspace="<id>"}` counter before Torii
surfaces update their lifecycle metadata. Operators should:

1. Subscribe to the event stream and archive the emitted
   `SpaceDirectoryEvent::ManifestExpired` payload.
2. Snapshot `nexus_space_directory_revision_total` (or the Grafana panel) before
   and after the expiry and store the delta in the evidence bundle.
3. Use `GET /v1/space-directory/uaids/{uaid}` to verify the manifest moved to
   `status = "Expired"` and that UAID bindings no longer list the dataspace.

Manual CLI expiries are still available when governance needs an override or a
backfilled lifecycle record:

```bash
iroha app space-directory manifest expire \
  --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
  --dataspace 11 \
  --expired-epoch 4700
```

The command emits the same event and telemetry as the automatic path, so
evidence remains consistent even when an operator has to memorialize an expiry
outside of the scheduler window.

## SDK Sample (Rust)

The Rust client can submit the same lifecycle instructions programmatically.
For a runnable helper check `docs/examples/space_directory_lifecycle.rs`, or
copy the snippet below to publish a manifest and later expire it without going
through the CLI:

```rust
use iroha::client::Client;
use iroha::config::Config;
use iroha::data_model::{
    isi::{
        InstructionBox,
        space_directory::{
            ExpireSpaceDirectoryManifest, PublishSpaceDirectoryManifest,
        },
    },
    nexus::{AssetPermissionManifest, DataSpaceId, ManifestVersion, UniversalAccountId},
};

fn publish_and_expire(client: &Client) -> eyre::Result<()> {
    let manifest = AssetPermissionManifest {
        version: ManifestVersion::V1,
        uaid: "uaid:0f4d…ab11".parse::<UniversalAccountId>()?,
        dataspace: DataSpaceId::new(11),
        issued_ms: 1_762_723_200_000,
        activation_epoch: 4_097,
        expiry_epoch: Some(4_700),
        entries: vec![],
    };
    client.submit_all([InstructionBox::from(PublishSpaceDirectoryManifest {
        manifest: manifest.clone(),
    })])?;

    client.submit_all([InstructionBox::from(ExpireSpaceDirectoryManifest {
        uaid: manifest.uaid,
        dataspace: manifest.dataspace,
        expired_epoch: manifest.expiry_epoch.unwrap(),
    })])?;
    Ok(())
}

fn main() -> eyre::Result<()> {
    let cfg = Config::from_path("client.toml")?;
    let client = Client::new(cfg)?;
    publish_and_expire(&client)
}
```

Swap the manifest contents for your dataspace before running the helper.

## 8. Audit Logging & Telemetry

Emit structured logs whenever a manifest changes state. Recommended fields:

```json
{
  "ts": "2026-01-15T12:05:33Z",
  "dataspace": 11,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "event": "SpaceDirectory.ManifestActivated",
  "manifest_hash": "12d4…9fa",
  "activation_epoch": 4097,
  "issued_ms": 1762723200000,
  "governance_ticket": "gov-2026-01-15-cbdc-wholesale",
  "approval_quorum": 5
}
```

Telemetry recommendations (align with NX-18 SLO work):

- `nexus_space_directory_active_manifests{dataspace,profile}` — gauge tracking
  active entries.
- `nexus_space_directory_revision_total{dataspace}` — monotonic counter of
  manifest publications (snapshot the delta before/after each activation).
- `nexus_space_directory_revocations_total{reason}` — counter for deny-wins
  actions.
- Alert: revocation without follow-up governance ticket within 24 h.

These feeds now ship directly from the core node telemetry path
(`crates/iroha_core/src/telemetry.rs`) and surface on the Nexus lanes Grafana
board (`dashboards/grafana/nexus_lanes.json`) as the **Space Directory Active
Manifests** (panel 10), **Manifest Revision Counter** (panel 11), and
**Manifest Revocations (rate)** (panel 12). Use those panels when collecting
evidence for governance tickets or verifying that revocations triggered the
expected deny-wins alerts.

Example expiry log entry (suitable for NDJSON shipping to the bundle):

```json
{
  "ts": "2026-02-12T08:00:00Z",
  "dataspace": 11,
  "uaid": "uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11",
  "event": "SpaceDirectory.ManifestExpired",
  "expired_epoch": 4700,
  "manifest_hash": "12d4…9fa",
  "reason": null
}
```

Store the NDJSON next to the generated `audit_bundle.json` (for example,
`cp /var/log/irohad/space_directory.ndjson artifacts/nexus/cbdc/2026-02-01T00-00Z/`)
so regulators can replay the same evidence without scraping logs from running
nodes.

## 9. Governance Proposal Template

Use the following Markdown front-matter for council submissions. Reference the
manifest hash (BLAKE3 of the Norito bytes) and activation epoch.

```markdown
---
proposal_id: GOV-2026-01-15-CBDC-01
dataspace: 11
uaid: uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11
manifest_hash: b0d40353396d828a674dc4b3d3df0e053d819f402bd8b6518420dfb4cc9c5c32
activation_epoch: 4097
expiry_epoch: 4600
change_type: activation
owners:
  - i105...
  - i105...
approvals_required: 5
---

- Capability summary: Wholesale transfer allowance, withdrawals denied.
- Evidence: `artifacts/nexus/cbdc/20260115T120000Z/`.
- Rollback plan: revert to previous manifest hash if telemetry breaches the
  error budget or if programmable-money replay fails.
```

Store the proposal text next to the manifest bundle so auditors see the full
chain of custody.

## 10. Incident & Revocation Playbook

1. Trigger emergency revoke (`ManifestRevoked`) when fraud or policy violation
   is detected. Use the audit hooks defined in the profile to alert PagerDuty.
2. Capture the denial reason in both the manifest entry and the event payload.
3. File an incident ticket linking to log excerpts and attach it to the next
   governance review.
4. Keep the revoked UAID in the profile whitelist history for traceability.

## 11. Regulator & Regional Examples

Regulator-facing manifests now ship alongside the CBDC and retail samples so
operators can rehearse MiCA/JFSA workflows without inventing bespoke payloads.
They live under `fixtures/space_directory/capability/` and mirror the evidence
bundle expectations described earlier in this guide.

| Fixture | Region | Highlights |
|---------|--------|------------|
| `eu_regulator_audit.manifest.json` (`53193a5e9ddb1c0634de1c14fae1f828dcc42118c3c0d1615777f025623bc42f`) | EU / ESMA + ESRB | Read-only allowances scoped to `compliance.audit.stream_reports` and `request_snapshot`, deny-wins guard on `retail.payments.transfer` to keep UAIDs mutation-free. |
| `jp_regulator_supervision.manifest.json` (`d797a1683f5c8771d69528425018cd72427be732d4251da9b2b9ca54b2e845a4`) | Japan / JFSA | Combines audit streams with a capped `cbdc.supervision.issue_stop_order` allowance (`100000000` micro-XOR per day) and explicitly denies `force_liquidation` to require dual controls. |

Usage notes:

- Update the `uaid` literals before publishing (the fixtures default to sample
  UAIDs with the least-significant bit already set). `iroha app space-directory
  manifest encode` and `publish` accept either the raw JSON or the `.to`
  payloads emitted by `cargo xtask space-directory encode`.
- The fixtures include inline `notes` that spell out policy intent (“Regulator
  UAIDs remain read-only”, “Requires dual controls”), so telemetry dashboards
  and governance packets surface the same descriptions regulators expect.
- Use them when rehearsing region-specific workflows: EU manifests drive MiCA
  data tap drills, while the JP example feeds the stop-order tabletop and
  demonstrates how to scope AMX roles for regulator-issued overrides.

When adapting the manifests to a new dataspace, change `dataspace`, `asset`, and
`activation/expiry` epochs but keep the deny-wins structure intact so existing
telemetry and audit scripts continue to match.

## 12. Checklist

- [ ] Capability manifest stored under `fixtures/space_directory/capability/`.
- [ ] Dataspace profile updated with validator/DA/composability metadata.
- [ ] Governance proposal uses the template above and cites manifest hash.
- [ ] Torii publish receipt archived with evidence pack.
- [ ] `SpaceDirectoryEvent` logs attached to compliance bundle.
- [ ] Dashboard annotation added for activation/revocation.

---

**Status (NX-15): Closed**

- Storage/fixtures, CLI + Torii helpers, and dashboards are live; the audit
  bundle + governance templates above are the canonical references.
- Scheduler expiry/revocation paths now have regression coverage, and telemetry
  panels are linked directly from this guide for operators and auditors.
