# SCCP V1 bridge proofs and release evidence

SCCP V1 is the first-release cross-chain protocol. Its security boundary is
the typed on-chain registry plus the canonical Rust admission code. Operator
JSON, RPC responses, deployment receipts, code hashes copied into a checklist,
and successful smoke tests are not proof by themselves.

This document describes the only supported first-release workflow. The former
per-chain browser collectors, live/source/destination evidence adapters,
material generators, mutable deployment helpers, and `allow-unready` release
paths have been removed. They must not be reintroduced as compatibility
layers.

## Security model

Every accepted transfer is bound to all of the following:

- one closed [`SccpNetworkV1`] profile and one directed [`SccpLaneIdV1`];
- one exact [`SccpGovernedRouteV1`] record in [`SccpRegistryV1`];
- one lane-level [`SccpNativeTrustAnchorV1`] checkpoint;
- one exact source emitter and destination deployment;
- one route id, asset key, amount scale, and SORA custody policy;
- one nonzero immutable route revision;
- one canonical native inbound proof or one authenticated outbound statement;
- one policy-approved semantic proof circuit and verifier key; and
- two independent release signatures over the complete evidence inventory.

No verifier guesses a network, codec flag, contract role, route revision, or
hash preimage. Unknown profiles, unknown fields, duplicate JSON keys,
non-canonical encodings, test networks, stale evidence, and unreferenced files
fail closed.

## Closed network and lane identities

[`SccpNetworkV1`] is a closed enum. The production external profiles are:

| Profile | Domain | Canonical chain identity |
|---|---:|---|
| `ethereum-mainnet` | 1 | EIP-155 chain id 1 |
| `bsc-mainnet` | 2 | EIP-155 chain id 56 |
| `tron-mainnet` | 5 | mainnet network magic `0x2b6653dc` |

Domains 3 and 4 are not part of the first-release wire/runtime surface.
Solana and TON lane records, proofs, deployments, policies, and release
evidence are rejected rather than reported as supported-but-unready.

The SORA endpoints are `sora-nexus` and `sora-taira`. A governed first-release
route is an external-to-SORA lane. The exact XOR destination deployments target
Taira. Network and lane commitments are derived only with
`sccp_network_identity_hash_v1` and `sccp_lane_id_hash_v1` from
`iroha_data_model::bridge`; SDKs and release scripts must not duplicate those
preimages.

## Atomic typed registry

`SccpRegistryV1` is lane anchored:

```text
SccpRegistryV1
└── SccpGovernedLaneV1
    ├── lane_id
    ├── native_trust_anchor
    └── routes[]
        └── SccpGovernedRouteV1
            ├── lane_id / route_id / asset_key
            ├── activation
            ├── source_identity
            ├── destination
            └── settlement
```

Call `SccpRegistryV1::validate` before registration or use. A route is valid
only when `SccpGovernedRouteV1::validate` succeeds and its source identity,
destination family, concrete route identity, and settlement scale agree.
`configuration_hash` commits the immutable economic route and its revision;
`destination_binding_hash` commits the family deployment. The family-specific
hash functions exported by `iroha_data_model::bridge` are the canonical
derivations:

- `sccp_evm_destination_binding_hash_v1`;
- `sccp_tron_destination_binding_hash_v1`;
- `sccp_exact_evm_xor_route_config_hash_v1`; and
- `sccp_exact_tron_xor_route_config_hash_v1`.

Registration starts in `Staged`. Directional activation is explicit:
`OutboundOnly` permits SORA-origin delivery and `Bidirectional` additionally
permits native inbound settlement. An activation that the selected native
backend cannot authenticate is invalid. Trust-anchor advancement is a
compare-and-swap operation over the exact lane and never rewrites route
identity.

## Destination deployment identity

The registry stores a closed `SccpDestinationDeploymentV1` variant.

For EVM and TRON, the deployment contains distinct token, verifier, and route
addresses; their exact runtime hashes; the full typed BN254 Groth16 verifying
key and its exact Solidity-compatible hash; and the fixed amount multiplier.
Every route has a nonzero immutable revision, preventing a successor deployment
whose nonce restarts from aliasing a predecessor message id. The route
constructor receives this revision as a nonzero `uint32`, stores it immutably,
and exposes it through `routeRevision()`. The canonical `Transfer` payload
encodes the revision immediately after the nonce. Its route-config asset tuple
is exactly `(keccak256(assetKey), keccak256(routeId), uint32 revision,
amountMultiplier)`.

The wrapped token receives its route bridge in its constructor. `bridge()` is
immutable, and a route constructor must reject a token whose `bridge()` is not
the route itself. There is no owner-set bridge, `setBridge`, `lockBridge`, or
`bridgeLocked` security state. EVM and BSC route hashing uses the EIP-152
BLAKE2F precompile. TRON uses the deterministic software implementation because
TVM assigns address `0x09` differently. The production smoke enforces the
24,576-byte runtime limit for every deployed contract role.

A runtime hash is the executable identity. An ABI is an operator interface,
not a consensus identity. Release policy may pin an audited ABI digest for
reproducibility, but admission never trusts a Boolean assertion that an ABI has
no mutator.

## Native inbound admission

The release validator and Core both call
`verify_sccp_native_inbound_message_proof_v1`. The input is a complete
`SccpNativeInboundMessageProofV1`, the governed source identity, and the exact
governed trust anchor. Successful validation returns normalized lane, source,
anchor, message, payload, source-event, and finality commitments.

The native verifier authenticates the chain-specific finality and inclusion
material before it accepts the event. A release script must never recompute a
lane hash or accept fields such as `proof_valid: true` as a substitute.

First-release availability is explicit:

| Inbound source | V1 release status |
|---|---|
| Ethereum mainnet | native proof verifier available |
| BSC mainnet | native proof verifier available |
| TRON mainnet | native proof verifier available |

Solana and TON inputs fail profile admission before proof verification. They
cannot be converted to success by observer assertions, environment variables,
or release flags.

## Outbound destination authentication

An outbound lane is `verified` only when the canonical Rust release validator
authenticates a complete destination-state statement. For EVM, BSC, and TRON,
the statement is signed by the per-profile Ed25519 destination attestor pinned
in the external production trust policy. The signature covers the canonical
Norito JSON statement under the
`iroha:sccp:destination-state-attestation:v1` domain.

The statement contains the typed governed route, finalized chain identity and
block, exact raw token/verifier/route runtime bytecode, immutable contract
readbacks, verifier-key hash, destination binding, concrete route configuration,
and governed-route configuration. The Rust validator:

1. verifies the pinned attestor signature;
2. validates the governed route with the data-model implementation;
3. checks the exact production chain id or TRON network magic;
4. Keccak-hashes each raw runtime byte sequence and compares it with both the
   registry and production build policy;
5. requires token `bridge()` to equal the typed route address;
6. requires route token and verifier readbacks to equal their typed addresses,
   and `routeRevision()` to equal the nonzero typed registry revision;
7. requires `verifyingKeyHash()` to equal both the registry value and the
   policy-approved semantic verifier key, after full key equality, canonical
   curve/subgroup validation, and canonical key hashing;
8. derives and compares destination-binding, concrete route-config, and
   governed-route configuration hashes with canonical Rust APIs.

The release timestamp must be no more than 24 hours after the authenticated
readback and must not precede it. Missing authenticated destination state is
`unavailable`, never `verified`.

The readiness report remains `ready: false` while any required ETH, BSC, or
TRON capability is unavailable.

## Semantic circuit policy

Pairing-valid algebra is not sufficient. The production trust policy pins, per
profile:

- a canonical semantic circuit id;
- the semantics `nexus-finality-v1` and `sccp-exact-statement-v1`;
- the exact circuit artifact SHA-256;
- the exact verifier-key hash;
- the nonzero route revision and SHA-256 of the canonical full typed key;
- the prover build and toolchain-lock SHA-256 digests;
- a reproducible source bundle and compiler build digest;
- token, verifier, and route build-artifact and operator-interface digests; and
- the expected token, verifier, and route runtime hashes.

Two distinct policy-pinned auditors sign this complete record: a semantic
security auditor and a prover reproducibility auditor. Each signature also
binds the auditor's report digest. The algebraic smoke-test verifier key
`9ef8067d260532f88e60cfa4b458fe678fc46b9c242de18fc91ba646e0857fc4`
is explicitly forbidden. Circuit ids containing `smoke` or `test` are also
rejected.

The policy establishes that the deployed key proves Nexus finality and the
exact SCCP statement, rather than merely being a syntactically valid Groth16
key. The bundle commits the complete policy hash.

## External release trust policy

Production tooling accepts exactly `sccp-release-trust-policy-v1` with
`environment: production`. It contains:

- the `release-engineering` and `release-security` signer identities/keys;
- one distinct destination-state attestor identity/key per production profile;
- two distinct circuit-auditor identities/keys; and
- the three audited semantic proof-system records.

Every identity and Ed25519 key must be distinct and valid in the prime-order
subgroup. The evidence document cannot introduce or replace a trusted key.
Release signatures are checked against the external policy.

The committed fixture uses the incompatible
`sccp-release-test-trust-policy-v1` schema with `environment: test-fixture`.
Only `scripts/sccp_release_fixture.py` accepts it, and that runner pins the
fixture policy id, release id, evidence path, and artifact root. Production
entrypoints have no option that enables fixture keys.

Signing keys are runtime-only. Keep private keys, bearer tokens, RPC
credentials, and authorization headers out of evidence, transcripts, bundles,
repository files, and command logs.

## Canonical release evidence

Build the read-only validator with production features:

```bash
cargo build -p iroha_sccp --bin sccp_release_evidence
```

Do not enable `test-fixtures` in production workflows or Make targets.

One canonical `sccp-release-evidence-v1` document contains:

- release/hub identity and creation time;
- the external trust-policy id;
- the validator protocol, crate version, source hash, and build identity;
- three lanes in canonical profile order;
- one typed lane-evidence artifact for every lane;
- one transcript for every required corridor phase;
- a sorted, hash- and size-bound artifact inventory; and
- two detached Ed25519 release signatures.

The signing payload is the domain
`iroha:sccp:release-evidence:v1\0` followed by canonical sorted compact JSON of
the evidence without `provenance`. Sign outside the repository and insert only
the public key and detached signature.

Validate standalone production evidence:

```bash
python3 scripts/sccp_all_lanes_evidence.py path/to/evidence.json \
  --artifact-root path/to/artifacts \
  --trust-policy /secure/public/sccp-production-trust-policy.json \
  --rust-validator target/release/sccp_release_evidence
```

The Python layer checks canonical files, external signatures, policy audits,
inventory, bounds, path safety, hard/symbolic links, and credential leakage.
It invokes the Rust validator independently for all three lane artifacts. Each
invocation receives the lane bytes plus the complete signed policy and evidence;
Rust re-verifies the signatures, selects the profile-pinned attestor and circuit,
and binds the result to the signed artifact digest and status. There is no CLI
that accepts a Python-projected attestor key, verifier hash, revision, or runtime
hash. Python does not implement SCCP hash formulas.

## Deterministic release bundle

Create a new bundle directory; existing outputs are never overwritten:

```bash
python3 scripts/sccp_release_bundle.py path/to/evidence.json \
  --artifact-root path/to/artifacts \
  --trust-policy /secure/public/sccp-production-trust-policy.json \
  --rust-validator target/release/sccp_release_evidence \
  --output-dir dist/sccp-release-v1
```

`bundle.json` commits:

- the exact evidence and artifact inventory;
- the external trust-policy id and SHA-256;
- the logical Rust validator source/build identity;
- the actual validator executable SHA-256; and
- a framed bundle root over all of those values.

Verify after transfer or publication with a separately supplied trusted policy
and validator executable:

```bash
python3 scripts/sccp_verify_release_bundle.py dist/sccp-release-v1 \
  --trust-policy /secure/public/sccp-production-trust-policy.json \
  --rust-validator target/release/sccp_release_evidence
```

The verifier enumerates the directory and rejects extra files, links, reused
digests, inventory drift, policy substitution, validator substitution,
signature drift, artifact tampering, and any Rust semantic mismatch. It invokes
the Rust lane validator again instead of trusting the builder's result.

Render machine or operator readiness only from verified inputs:

```bash
python3 scripts/sccp_release_readiness_report.py dist/sccp-release-v1 \
  --trust-policy /secure/public/sccp-production-trust-policy.json \
  --rust-validator target/release/sccp_release_evidence \
  --format markdown
```

`ready` is derived from the exact required capability matrix. It is never an
input flag. Every ETH, BSC, and TRON inbound and outbound must be verified for
`ready: true`; domains 3 and 4 are outside the schema.

## Production corridor

The focused corridor is:

```bash
bash scripts/check_sccp_production_corridor.sh
```

It runs Rust SCCP verification, adversarial release-tooling tests, SDK proof
packaging tests, direct contract smoke tests, and Core admission tests. With
`--log-dir`, every phase is captured as a separate signed-evidence transcript.
The evidence phase builds the production validator without fixture features,
validates the pinned non-production fixture, constructs a deterministic bundle,
and independently verifies that bundle.

Use `--phase NAME` only for focused diagnosis. A release requires every phase.
The aggregate CI job fails when any phase fails, is cancelled, or is skipped.

## Required negative coverage

Changes to SCCP proofs, registry state, contracts, or release tooling must keep
negative tests for at least:

- wrong profile/domain/chain identity and test-network substitution;
- malformed, duplicate-key, non-canonical, oversized, or deeply nested JSON;
- path traversal, symbolic links, hard links, file swaps, and extra files;
- stale/future destination attestations and rogue attestor keys;
- invalid Ed25519 encodings, role swaps, key reuse, and signature replay;
- unapproved circuit ids, audit/report drift, smoke VK, and policy substitution;
- mutated native proof, source identity, anchor, event, payload, or finality;
- mutated raw runtime bytecode, immutable bridge, verifier key, or role address;
- route activation, settlement, destination binding, and configuration drift;
- validator source or executable substitution; and
- credentials in plain, percent-encoded, HTML-escaped, or base64 form.

Positive smoke tests never replace these adversarial admission tests.

[`SccpNetworkV1`]: ../../crates/iroha_data_model/src/bridge/sccp.rs
[`SccpLaneIdV1`]: ../../crates/iroha_data_model/src/bridge/sccp.rs
[`SccpRegistryV1`]: ../../crates/iroha_data_model/src/bridge/sccp_registry.rs
[`SccpGovernedRouteV1`]: ../../crates/iroha_data_model/src/bridge/sccp_registry.rs
[`SccpNativeTrustAnchorV1`]: ../../crates/iroha_data_model/src/bridge/sccp.rs
