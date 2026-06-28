---
lang: mn
direction: ltr
source: docs/source/sorafs_proto_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e5d81629f284ea39ef1503f7671171e95987f5b7c7dc84a2c884c58777aba890
source_last_modified: "2026-06-25T16:58:37+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Wire Format & Schema Reference
summary: SF-10 implementation status for canonical Norito payloads, fixtures, validators, and remaining release-evidence work.
---

# SoraFS Wire Format & Schema Reference

## Current Status

SF-10 has local schema, fixture, and reference-validation foundations. Canonical
SoraFS payloads live in `crates/sorafs_manifest`, committed fixtures live under
`fixtures/sorafs_manifest/`, and `sorafs-validate` plus the reference FFI expose
stable validation outcomes for SDK and release smoke tests. Remaining work is
live release evidence and SDK distribution hygiene, not defining a separate
`sora-proto` codec outside Norito.

## Canonical Modules & Payloads

| Domain | Rust module | Primary payloads |
|--------|-------------|------------------|
| Provider adverts | `provider_advert.rs` | `ProviderAdvertV1`, `ProviderAdvertBodyV1`, `AdvertSignature` |
| Provider admission | `provider_admission.rs` | `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, renewal and revocation payloads |
| Capacity and replication | `capacity.rs` | `ReplicationOrderV1`, `SignedReplicationOrderV1`, capacity declarations, pricing, disputes, telemetry |
| Orderbook and streaming settlement | `orderbook.rs` | `OrderRequestV1`, `OrderCancelV1`, `TradeEventV1`, `SettlementChannelV1`, `SettlementReceiptV1` |
| PoR / audit | `por.rs` | `PorChallengeV1`, `PorProofV1`, `AuditVerdictV1` |
| PDP | `pdp.rs` | PDP commitment/challenge/proof descriptors and validators |
| PoTR | `potr.rs` | `PotrProbeV1`, `PotrReceiptV1`, `PotrVerdictV1` |
| Repair | `repair.rs` | repair tasks, evidence, reports, policies, approvals, auditor requests, worker signatures, events |
| Reputation | `reputation.rs` | reputation weights, provider inputs, snapshots, events, Merkle proofs |
| Governance DAG | `governance.rs` | `GovernanceLogNodeV1`, payload variants, publisher signatures |
| Reference validation | `reference.rs`, `reference_ffi.rs` | `ValidationOutcomeV1`, byte validators, C ABI facade |

Every payload that crosses a SoraFS boundary must be encoded with Norito. JSON
views are for operator readability and fixtures; they are not alternate wire
formats.

## Implemented Validators

`crates/sorafs_manifest::reference` exposes byte-level validators for:

- provider adverts;
- provider admission envelopes, renewals, and revocations;
- replication orders and signed replication orders;
- orderbook and streaming-settlement payloads;
- PoR challenge/proof pairs;
- PoTR receipts;
- repair payloads;
- fixture-directory bundles, including committed orderbook payload fixtures;
- governance log nodes.

The `reference_ffi` facade returns `ValidationOutcomeV1` Norito JSON buffers for
SDK bindings that need the Rust reference validators without duplicating schema
logic.

## CLI Surface

The `sorafs-validate` binary provides the release-facing validator:

```sh
cargo run --locked -p sorafs_manifest --bin sorafs-validate -- \
  advert --input fixtures/sorafs_manifest/provider_admission/advert_v1.to --format json

cargo run --locked -p sorafs_manifest --bin sorafs-validate -- \
  admission --input fixtures/sorafs_manifest/provider_admission/envelope_v1.to --format json

cargo run --locked -p sorafs_manifest --bin sorafs-validate -- \
  order --order fixtures/sorafs_manifest/replication_order/order_v1.to --format json

cargo run --locked -p sorafs_manifest --bin sorafs-validate -- \
  orderbook --receipt fixtures/sorafs_manifest/orderbook/settlement_receipt_v1.to --format json

cargo run --locked -p sorafs_manifest --bin sorafs-validate -- \
  por --challenge fixtures/sorafs_manifest/por/challenge_v1.to \
      --proof fixtures/sorafs_manifest/por/proof_v1.to \
      --format json

cargo run --locked -p sorafs_manifest --bin sorafs-validate -- \
  potr --receipt fixtures/sorafs_manifest/potr/receipt_v1.to --format json

cargo run --locked -p sorafs_manifest --bin sorafs-validate -- \
  repair --task fixtures/sorafs_manifest/repair/task_v1.to --format json

cargo run --locked -p sorafs_manifest --bin sorafs-validate -- \
  governance --node fixtures/sorafs_manifest/governance/node_v1.to --format json

cargo run --locked -p sorafs_manifest --bin sorafs-validate -- \
  bundle --bundle fixtures/sorafs_manifest --format json
```

The bundle validator discovers `fixtures/sorafs_manifest/orderbook/*.to`
payloads in addition to the manifest-linked advert, admission, replication,
PoR, PoTR, repair, and governance fixture artifacts.

Signing helpers are available for adverts, replication orders, and governance
nodes:

```sh
cargo run --locked -p sorafs_manifest --bin sorafs-validate -- \
  sign --kind governance \
  --input fixtures/sorafs_manifest/governance/node_v1.to \
  --out artifacts/sorafs/governance/signed_node_v1.to \
  --key-hex <ed25519-seed-hex> \
  --format json
```

## Fixtures & Active Generators

Committed fixtures live under:

- `fixtures/sorafs_manifest/provider_admission/`
- `fixtures/sorafs_manifest/replication_order/`
- `fixtures/sorafs_manifest/orderbook/`
- `fixtures/sorafs_manifest/por/`
- `fixtures/sorafs_manifest/potr/`
- `fixtures/sorafs_manifest/repair/`
- `fixtures/sorafs_manifest/governance/`
- `fixtures/sorafs_manifest/ci_sample/`

Use the active generators and stubs:

```sh
cargo run --locked -p sorafs_car --features cli --bin provider_admission_fixtures
cargo run --locked -p sorafs_car --bin sorafs_manifest_stub -- \
  capacity replication-order --spec fixtures/sorafs_manifest/replication_order/order_v1.json
cargo run --locked -p sorafs_manifest --bin generate_orderbook_fixtures
cargo run --locked -p sorafs_manifest --bin generate_por_fixtures
```

Do not document retired generator names as required workflow. When a schema
changes, refresh the relevant fixture directory and run the matching
`sorafs-validate` command plus focused crate tests.

## Cross-Language Contract

- Rust code consumes `sorafs_manifest` payloads directly with
  `norito::{to_bytes, decode_from_bytes}`.
- SDKs should call the reference validator or C ABI facade for schema checks
  rather than reimplementing Norito layout rules.
- Transport should preserve raw Norito bytes and may include decoded JSON only
  as commentary.
- Unknown versions, missing required fields, invalid signatures, and broken
  fixture cross-links must fail closed. Bundle validation also fails closed if
  any discovered orderbook fixture is malformed or violates settlement policy.

## Remaining Production Gates

- Publish release bundles that include the refreshed `.to` fixtures,
  human-readable JSON commentary, validation outcomes, and digest manifests.
- Keep portal error-catalog links synchronized with `ValidationOutcomeV1` codes.
- Capture SDK smoke evidence that JavaScript/TypeScript, Python, Swift, Kotlin,
  Java, and C# consumers validate the same committed fixtures through shared
  validators or FFI bindings.
- Re-run fixture and reference-validator smoke tests whenever Norito payload
  layouts, signing domains, or governance payload variants change.

## Validation

Focused checks for this reference are:

```sh
cargo test -p sorafs_manifest
cargo test -p sorafs_manifest --test sorafs_validate_cli
```

Run SDK parity tests when changing the reference FFI or generated language
bindings.
