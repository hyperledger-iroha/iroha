---
lang: hy
direction: ltr
source: docs/source/torii/portfolio_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b818ccb129e5ca3d501f0ce969e05fff8945e039f741aa276c5eaf41661cb89
source_last_modified: "2026-01-30T18:06:03.653701+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# UAID Portfolio API

The Nexus program exposes a read-only Torii endpoint for inspecting the
aggregated holdings associated with a Universal Account ID (UAID). The surface
is backed by `iroha_core::nexus::portfolio::collect_portfolio`, which consults
the Space Directory bindings managed by the main Sora Nexus dataspace to learn
which accounts/dataspaces a UAID is active in. The response is grouped first by
dataspace and then by account for deterministic diffing.

```
GET /v1/accounts/{uaid}/portfolio
```

## Path parameters

| Name | Description |
|------|-------------|
| `uaid` | UAID literal. Accepts either the `uaid:<hex>` form or a raw 64-character hex digest (LSB=1). |

## Query parameters

| Name | Description |
|------|-------------|
| `asset_id` | Optional filter that limits the response to positions for the specified asset identifier. |

## Response schema

```jsonc
{
  "uaid": "uaid:ab7c…",
  "totals": {
    "accounts": 2,
    "positions": 3
  },
  "dataspaces": [
    {
      "dataspace_id": 0,
      "dataspace_alias": "universal",
      "accounts": [
        {
          "account_id": "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
          "label": null,
          "assets": [
            {
              "asset_id": "cash#portfolio::6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw",
              "asset_definition_id": "cash#portfolio",
              "quantity": "500"
            }
          ]
        }
      ]
    },
    {
      "dataspace_id": 11,
      "dataspace_alias": "cbdc",
      "accounts": [
        {
          "account_id": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
          "label": "primary-cbdc",
          "assets": [
            {
              "asset_id": "wholesale#cbdc::34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
              "asset_definition_id": "wholesale#cbdc",
              "quantity": "250"
            },
            {
              "asset_id": "fx#cbdc::34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r",
              "asset_definition_id": "fx#cbdc",
              "quantity": "25"
            }
          ]
        }
      ]
    }
  ]
}
```

* `totals.accounts` counts how many ledger accounts reference the UAID.
* `totals.positions` counts the non-zero asset positions aggregated across
  those accounts.
* `dataspaces` enumerate holdings per dataspace. Each slice corresponds to the
  UAID↔dataspace bindings stored in the Space Directory; if a UAID has not been
  bound yet it will appear under the fallback `default_dataspace`
  (`DataSpaceId::GLOBAL`).
* Each account entry includes the optional stable label plus the sorted list of
  asset positions with their canonical identifiers and Norito numeric balances.

## Notes

- The response is sorted deterministically by dataspace, then by account ID,
  and finally by asset ID to simplify caching and SDK diffs.
- Dataspace aliases come from the configured `DataSpaceCatalog`. When the Space
  Directory has not yet bound an account, the entry falls back to the routing
  policy’s `default_dataspace`.
- The aggregator ignores zero-valued placeholders to keep the output concise.
- When `asset_id` is supplied, the response payload (including totals) only
  includes positions that match the requested asset identifier.
- UAID literals are validated before execution; malformed inputs return
  `400 Bad Request` with `QueryExecutionFail::InvalidSingularParameters`.
- This endpoint is gated by the standard Torii access controls and inherits the
  same CIDR/API-token policies as other account surfaces.
- The Space Directory is authoritative for UAID bindings. Keep the registry in
  sync when capability manifests activate/revoke so portfolio responses stay
  accurate across dataspaces. From NX‑16 onward the node updates bindings
  automatically whenever manifests fire events or UAID-backed accounts
  register/unregister.

### Golden fixtures

Roadmap item **NX-16** calls for auditable fixtures covering the UAID portfolio
pipeline. The repository now ships `fixtures/nexus/uaid_portfolio/global_default_portfolio.json`
plus a README explaining how to refresh it. The deterministic integration test
`crates/iroha_torii/tests/accounts_portfolio.rs::accounts_portfolio_snapshot_matches_fixture`
seeds a predictable dataspace/domain and asserts `/v1/accounts/{uaid}/portfolio`
returns exactly the JSON stored in the fixture. SDKs and ops runbooks can use
the same file to diff client implementations, and updating the fixture is as
simple as re-running the test with `--nocapture` and copying the emitted JSON as
described in `fixtures/nexus/uaid_portfolio/README.md`.

## UAID Bindings API

To troubleshoot capability manifests or simply query where a UAID has active
permissions, Torii exposes a lightweight bindings snapshot:

```
GET /v1/space-directory/uaids/{uaid}
```

| Query | Description |
|-------|-------------|
| Address output | Canonical I105 only. |

Sample response:

```jsonc
{
  "uaid": "uaid:ab7c…",
  "dataspaces": [
    {
      "dataspace_id": 0,
      "dataspace_alias": "universal",
      "accounts": ["6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw"]
    },
    {
      "dataspace_id": 11,
      "dataspace_alias": "cbdc",
      "accounts": ["34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r"]
    }
  ]
}
```

The bindings snapshot is derived from the `uaid_dataspaces` ledger map (managed
by the Space Directory). If no bindings exist yet, the `dataspaces` array is
empty. Access controls mirror the portfolio endpoint: CIDR/API-token policies
apply, and malformed UAID literals produce `400` errors. Use this endpoint to
confirm manifest rollouts before attaching allowances or enabling AMX flows.
The underlying ledger map now updates automatically: when a capability manifest
activates/expirs/revokes or when a UAID-backed account is registered,
`World::uaid_dataspaces` is refreshed so Torii always reports the current
bindings with no manual seeding.

## Space Directory Manifest API

Space Directory manifests can be inspected through Torii to confirm which UAID
policies are active per dataspace. The endpoint returns the canonical manifest
payload (`AssetPermissionManifest`), ledger bindings, and lifecycle metadata in
one response:

```
GET /v1/space-directory/uaids/{uaid}/manifests?dataspace={id}
```

| Query | Description |
|-------|-------------|
| `dataspace` (optional) | Filter results to a specific dataspace ID (u64). |
| `status` (optional) | `active`, `inactive`, or `all` (default). Inactive captures pending, expired, and revoked manifests. |
| `limit` (optional) | Maximum number of manifests to return (default unlimited). |
| `offset` (optional) | Number of manifests to skip before collecting results (default `0`). |
| Address output | Canonical I105 only. |

Sample response:

```jsonc
{
  "uaid": "uaid:0f4d…ab11",
  "total": 1,
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
      "accounts": ["34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r"],
      "manifest": {
        "version": 1,
        "uaid": "uaid:0f4d…ab11",
        "dataspace": 11,
        "issued_ms": 1762723200000,
        "activation_epoch": 4097,
        "expiry_epoch": 4600,
        "entries": [
            { "scope": { "program": "cbdc.transfer" }, "effect": { "Allow": { "max_amount": "500000000", "window": "PerDay" } } }
        ]
      }
    }
  ]
}
```

- `manifest_hash` is the BLAKE3 digest of the Norito-encoded manifest payload.
- `status` reports `Pending`, `Active`, `Expired`, or `Revoked` based on the
  recorded lifecycle events (`Activated` beats schedule, `Revoked` wins over
  expiry).
- `lifecycle.revocation` includes the epoch/reason for emergency denies when
  present.
- `accounts` reuse the `uaid_dataspaces` ledger map so operators can see which
  concrete account IDs are tied to the manifest’s dataspace. Set
  canonical I105 output for
  offline or QR workflows.
- The `manifest` object is the exact `AssetPermissionManifest` structure
  published to the Space Directory, making it easy for SDKs to replay the
  entries without bespoke JSON schemas.
- `total` reports how many manifests matched before pagination; combine it with
  `limit`/`offset` to page through large UAID histories.
- `status` filters help operators focus on active manifests. `inactive` returns

The endpoint shares the same access controls as `/v1/accounts/{uaid}/portfolio`
and `/v1/space-directory/uaids/{uaid}`. See `docs/space-directory.md` for the
operational playbooks tied to these responses.

## Manifest Publish Endpoint

Capability manifests can now be published over HTTP:

```
POST /v1/space-directory/manifests
```

Body schema:

| Field | Description |
|-------|-------------|
| `authority` | Account that signs the publication transaction (must own `CanPublishSpaceDirectoryManifest{dataspace}`). |
| `private_key` | `ExposedPrivateKey` for `authority`. |
| `manifest` | Full `AssetPermissionManifest` JSON payload (UAID, dataspace, lifecycle schedule, entries). |
| `reason` (optional) | Convenience string applied to entries lacking `notes`, mirroring the CLI `--reason` helper. |

Example payload:

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
        "scope": { "dataspace": 11, "program": "cbdc.transfer", "method": "transfer" },
        "effect": { "Allow": { "max_amount": "500000000", "window": "PerDay" } }
      }
    ]
  },
  "reason": "CBDC onboarding wave 4"
}
```

The endpoint returns `202 Accepted` once the transaction is queued. The usual
CIDR/API-token/fee-policy gates apply. Upon execution the node emits
`SpaceDirectoryEvent::ManifestActivated`, rebuilds UAID bindings, and the read
surfaces immediately expose the updated manifest.
Python SDKs can invoke this workflow through
`ToriiClient.publish_space_directory_manifest`, which normalises UAID literals,
dataspace identifiers, and credential payloads before issuing the HTTP request.

## Manifest Revocation Endpoint

Torii also exposes a write surface so operators can trigger emergency
revocations without switching tools:

```
POST /v1/space-directory/manifests/revoke
```

Body fields mirror the `RevokeSpaceDirectoryManifest` ISI:

| Field | Description |
|-------|-------------|
| `authority` | Account ID that signs the transaction (must hold `CanPublishSpaceDirectoryManifest{dataspace}` or an equivalent grant). |
| `private_key` | `ExposedPrivateKey` belonging to `authority`. |
| `uaid` | UAID literal (`uaid:<hex>` or raw 64-hex digest, LSB=1). |
| `dataspace` | Dataspace identifier hosting the manifest. |
| `revoked_epoch` | Epoch (inclusive) when the revocation takes effect. |
| `reason` (optional) | Human-readable justification archived with the lifecycle record. |

Example payload:

```jsonc
{
  "authority": "i105...",
  "private_key": "ed25519:CiC7…",
  "uaid": "uaid:0f4d…ab11",
  "dataspace": 11,
  "revoked_epoch": 9216,
  "reason": "Fraud investigation #NX-16-R05"
}
```

Torii responds with `202 Accepted` once the transaction is queued; no body is
returned. The same CIDR/API-token/fee-policy gates applied to the read
endpoints protect this route as well. When the transaction executes the node
emits `SpaceDirectoryEvent::ManifestRevoked`, rebuilds UAID bindings
automatically, and the read APIs immediately reflect the revoked status. See
`docs/space-directory.md` for the operator runbook and evidence expectations.
Python automation can call `ToriiClient.revoke_space_directory_manifest` to
mirror the Norito payload without hand-rolling JSON serialisation or key
formatting.
