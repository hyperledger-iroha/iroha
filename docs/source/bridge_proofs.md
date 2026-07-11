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
non-canonical encodings, test-network production evidence, stale evidence, and
unreferenced files fail closed.

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

The closed network enum represents `sora-taira` as its sole SORA endpoint;
`sora-nexus` has no SCCP V1 representation. Every value-moving governed V1
route targets Taira. The exact local identity is the public Sumeragi-v2 chain id
`fc56984b-2be7-431d-840e-21514d1883f0` with I105 discriminant `369`
(`0x0171`, the canonical `test...` sentinel family). Discriminant `753`, a
generic/default discriminant, or any custom numeric discriminant does not name
Taira. The archived pre-v2 chain id `809574f5-fee7-5e69-bfcf-52451e42d50f`
is read-only history and is not an SCCP V1 settlement target. A governed route
is stored in its external-to-Taira direction and also
binds the exact reverse destination deployment. Network and lane commitments
are derived only with
`sccp_network_identity_hash_v1` and `sccp_lane_id_hash_v1` from
`iroha_data_model::bridge`; SDKs and release scripts must not duplicate those
preimages.

Account roles on a value-moving route are not interchangeable. An irreversible
external-to-Taira burn recipient is limited to the exact `test...` I105 spelling
of a canonical, non-weak single-key Ed25519 account. A proof-authenticated
Taira-to-external sender may be a canonical single-key or multisig `AccountId`,
but every controller key must be Ed25519 or compressed secp256k1 in V1. Core
checks the typed controller and exact discriminant-`369` spelling before moving
assets into custody; the EVM/TVM contracts independently check the same
AccountAddress tags, policy ordering, key admission, I105 round trip, and
checksum before proof dispatch. Other Rust-supported controller algorithms are
fail-closed for SCCP V1 so they cannot create an outbound lock that the
destination route cannot finalize.

## Atomic typed registry

`SccpRegistryV1` is lane anchored:

```text
SccpRegistryV1
└── SccpGovernedLaneV1
    ├── lane_id
    ├── native_trust_anchors[0..=4096]
    ├── current_native_trust_anchor_hash
    └── routes[1..=64]
        └── SccpGovernedRouteV1
            ├── lane_id / route_id / asset_key
            ├── activation
            ├── inbound_finality_cutoff
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

Registration starts in `Staged`. `Bidirectional` is the only state that permits
new SORA-origin locks. `InboundOnly` drains already-issued external claims
without admitting new locks, `Paused` is resumable emergency state, and
`Retired` is terminal historical state. Every lifecycle update, revision
switch, and trust-anchor advance is an exact compare-and-swap operation; a
stale expected value or illegal transition fails without rewriting registry
state. Only a never-used `Staged` revision can be removed.

Native trust-anchor rotation is append-only. The current hash must select the
last, highest checkpoint, while every earlier checkpoint remains available for
proofs that were finalized before governance advanced the lane. Anchor hashes
are unique, heights increase strictly, and all entries use the lane's one
native verifier family. Admission resolves the proof-carried anchor hash
through the validated registry index and then compares the complete governed
anchor before performing chain-specific verification; an unknown hash or a
known hash paired with forged height/backend material fails closed.

Anchor intervals use an authenticated consensus-progress coordinate, not a
generic event block number: Ethereum uses the fully verified finalized beacon
slot, while BSC and TRON use the fully verified finalized native block height.
An old anchor remains valid through its successor checkpoint inclusively (a
one-height overlap needed by BSC/TRON boundary proofs) and never beyond it; the
last current anchor is open-ended. A terminal route carries a mandatory
`inbound_finality_cutoff` whose `max_anchor_interval_height` must equal the
referenced historical anchor's successor checkpoint. Governance must therefore
advance to checkpoint B before atomically retiring the old revision at B. This
preserves every delayed claim authenticated in the complete A-to-B interval
while rejecting later events from the retired emitter; mid-interval,
open-ended-current, unknown-anchor, and zero cutoffs are invalid.

The durable inbound replay record persists both coordinates: the event/source
finality height used by the proof range and the separately verified
`anchor_interval_height` used by checkpoint-interval and retirement-cutoff
admission. This keeps an accepted Ethereum proof auditable as a beacon-slot
decision even after the original proof bytes are pruned.

Admission also max-raises a fixed-size high-water index keyed by exact lane and
trust-anchor hash. A trust-anchor advance may select successor checkpoint B
only when B is at or above every coordinate already admitted under current
anchor A; equality is valid because the A/B boundary is inclusive. Snapshot
hydration recomputes this index from durable inbound records and requires exact
equality, rejecting missing, stale, malformed, or unbacked entries. This keeps
rotation from retroactively invalidating accepted evidence without adding a
lifetime replay-map scan to governance execution.

Capacity limits distinguish mutable state from retained history. A lane may
have at most eight nonterminal (`Staged`, active, draining, or `Paused`) routes
and the registry at most 64 live routes. Separately, each lane retains at most
64 total route revisions and 4,096 native anchors. `Retired` revisions do not
consume the live budget, but retained entries are never evicted implicitly;
the next append rejects atomically at the history bound so authenticated
evidence is not silently invalidated. On a single-lineage lane, 64 monthly
deployments cover more than five years; 4,096 daily anchor rotations cover more
than eleven years. Multiple lineages share the route horizon. Conservative 4
KiB-per-route and 64-byte-per-anchor canonical envelopes bound retained entry
payloads across 16 lanes to 8 MiB before small framing overhead. Each
governance action still appends at most one record, and operators must plan an
explicit migration before exhausting either horizon.

## Destination deployment identity

The registry stores a closed `SccpDestinationDeploymentV1` variant.

For EVM and TRON, the deployment contains distinct token, verifier, and route
addresses; their exact runtime hashes; the full typed BN254 Groth16 verifying
key and its exact Solidity-compatible hash; the mandatory typed outbound proof
policy; and the fixed amount multiplier. The proof policy pins both the audited
semantic circuit profile and the governed Taira finality anchor. Their
domain-separated hashes are included in the destination binding and concrete
route configuration, and are exposed by immutable verifier readbacks.
Every route has a nonzero immutable revision, preventing a successor deployment
whose nonce restarts from aliasing a predecessor message id. The route
constructor receives this revision as a nonzero `uint32`, stores it immutably,
and exposes it through `routeRevision()`. The canonical `Transfer` payload
encodes the revision immediately after the nonce. Its route-config asset tuple
is exactly `(keccak256(assetKey), keccak256(routeId), uint32 revision,
amountMultiplier)`.

Deployment tooling precomputes the route address, deploys the wrapped token
with that exact immutable bridge, then deploys the route at the precomputed
address with the exact external `tokenAddress`. The route constructor requires
`token.bridge() == address(this)`, nonempty token code, the governed token code
hash, distinct token/verifier roles, and the complete typed outbound proof
policy before construction succeeds. There is no owner-set bridge,
`setBridge`, `lockBridge`, or `bridgeLocked` security state.
EVM and BSC route hashing uses the EIP-152 BLAKE2F precompile. TRON uses the
deterministic software implementation because TVM assigns address `0x09`
differently. The production smoke enforces the 24,576-byte runtime limit for
every deployed contract role.

The checked-in artifact lock pins corridor manifest digest
`d2d595bc62e39eabc604a94c0c0b91660c80a8bfdb7d6d67ce04fcd3069303b0`
and canonical compiler-lock digest
`4e47b010e2c6475a3711d79f22ba58355534a5b519c99609094bf9b97d968c99`.
Any compiler identity, setting, source input, creation bytecode, deployed
runtime, ABI, immutable reference, or size drift invalidates that lock before a
provider is created.

The release TVM corridor runs only the authenticated TRON artifact on the
digest-pinned official TRE image. Before execution, it copies the manifest and
Rust-generated native-transfer vectors once through bounded no-follow file
descriptors into a runner-owned private, read-only snapshot. It verifies that
copy against the artifact lock and recomputes the current EVM/TVM standard-JSON
source-input hashes, so a stale or replaced manifest cannot pass static smoke
and then feed different bytes to TRE. Live negative cases require an included,
transaction-ID-matched TRE receipt whose TVM result is not `SUCCESS`;
transport, ABI, broadcast, and timeout failures are infrastructure failures,
not proof that the contract rejected an adversarial transaction. The same run
checks all checked-in Rust native-transfer hash vectors on TVM, boundary-length
BLAKE2b inputs, independently derived deployment bindings, and the complete
emitted source-event payload and topics.

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

Cryptographic finality is mandatory in every SCCP build. The crate has no
consensus-changing BLS feature switch: it always compiles the same
proof-of-possession, public-key, aggregate-signature, and quorum verification
paths for Taira and BSC finality. Structural parsers and proof-controlled
self-consistency helpers are diagnostics, not trust anchors. A validator binary
whose build metadata enables test-fixture code is not a production release
validator.

Native continuations are canonical shortest prefixes. A BSC proof may select a
target only within the first full 1,000-block Parlia epoch after its governed
anchor and may carry at most four later headers: the three retained
fast-finality ancestor contexts plus the one contiguous vote that makes the
target a finalized source. A TRON proof may select a target only within one
27-witness scheduling round and may use at most one further 27-witness round to
reach the native 19-of-27 solid-height threshold. Thus the absolute framing
limits are 1,004 BSC headers and 54 TRON headers, replacing generic large-array
caps. Governance must advance a stale checkpoint instead of asking every peer
to replay an older chain segment.

The target must become finalized or solid for the first time on the final
supplied header. Once that condition is reached, an appended header is rejected
without parsing it or recovering its signature; a continuation outside the
protocol window is rejected before anchor or signature verification. The
cryptography-free `bsc_native_finality_work_estimate` and
`tron_native_finality_work_estimate` functions expose framed header bytes,
header counts, secp256k1 recovery counts, and conservative BSC aggregate/signing
contribution bounds so consensus admission can reserve per-transaction and
per-block native-verifier work before dispatching cryptography.

## Deterministic verifier-work limits

Closed SCCP proofs use the dedicated, mandatory `[zk.sccp]` consensus limits;
they do not consume or inherit the unrelated confidential-proof counters. The
default first-release transaction/block limits are:

| Work dimension | Transaction | Block |
|---|---:|---:|
| proofs | 1 | 4 |
| canonical proof bytes | 8 MiB | 32 MiB |
| BSC/TRON continuation headers | 1,004 | 4,016 |
| Ethereum light-client updates | 128 | 512 |
| framed native-finality bytes | 8 MiB | 32 MiB |
| secp256k1 recoveries | 1,005 | 4,020 |
| BLS aggregate checks | 1,004 | 4,016 |
| BLS key/contribution work items | 131,713 | 526,852 |
| BN254 pairing-product checks | 1 | 4 |

One proof may contain at most 8 MiB of canonical bytes. All limits are nonzero,
transaction limits cannot exceed block limits, and they are included in the ZK
consensus-policy hash. They have no environment-variable aliases: validators
must obtain identical values from configuration files.

Core preflights proof count and bytes before any proof-controlled canonical
decode. It then derives hardware-independent work from bounded framing and
atomically registers the complete transaction/block delta before signature
recovery, BLS verification, or BN254 pairing. An abandoned or rejected
transaction does not leak staged work into the block. Destination admission
conservatively reserves two passes over the maximum 4,096-validator Taira
roster, covering both key validation and all-signer PoP/aggregation. Ethereum
reserves one 513-key bootstrap plus up to 128 updates, each with 513 next-
committee key validations and 512 possible signer contributions. BSC reserves
all framed headers, the anchor seal and continuation recoveries, every possible
attestation, and all active/pending/epoch roster validation passes. These are
upper bounds, so different peer hardware cannot change admission results.

## Proof-local typed admission

`BridgeProof` has no free-standing manifest field that can be reinterpreted by
the caller. Its payload owns a role-preserving `BridgeProofBinding`:

- `NativeProtocol(BridgeNativeProtocolProofV1)` carries a closed native backend,
  the exact historical SCCP route-configuration hash, and one canonical typed
  native envelope;
- `SccpDestination(BridgeSccpDestinationProofV1)` carries a closed EVM/TVM
  Groth16 backend, the exact historical route-configuration hash, and one
  canonical destination artifact; and
- generic `Ics` and `TransparentZk` payloads carry a distinct
  `VerifierManifest` binding only.

Core validates the closed SCCP variants against the retained governed route,
typed source identity or destination policy, exact proof range, durable message
record, and local committed finality state. `SubmitBridgeProof` rejects generic
ICS and transparent-ZK payloads until an authoritative on-chain verifier is
implemented. A generic `ProofBox` backend label, manifest hash, or pairing-valid
but route-unbound artifact cannot enter SCCP settlement.

## Detached Torii transaction flow

`POST /v1/bridge/proofs/submit` accepts one canonical
`destination_proof_b64`; `POST /v1/bridge/messages` accepts one canonical
`native_proof_b64`. Both endpoints are available only on the exact Taira chain
and use the same two states:

1. **Prepare:** send canonical Taira `authority` and the endpoint artifact,
   optionally with a positive `creation_time_ms`. Omit both `signature_b64` and
   `transaction_payload_b64`. The response has `submitted: false`, no
   `tx_hash_hex`, and returns the exact canonical transaction payload plus its
   32-byte signing prehash.
2. **Direct submit:** resend the same artifact with both `signature_b64` and
   the returned `transaction_payload_b64`, plus the exact positive
   `creation_time_ms`. Torii decodes and re-encodes the bounded payload,
   byte-compares its chain, authority, proof instruction, metadata, and time,
   verifies the detached signature, and queues exactly that transaction. The
   response has `submitted: true` and `tx_hash_hex`, with no signing scaffold.

Detached signatures are canonical padded base64 of one nonempty, nonzero
generic signature payload, bounded to 16 KiB; they are not restricted to raw
Ed25519 length. The authority must be a single-key canonical Taira I105 account.
Multisig authorities must prepare the payload and use the normal multisig
propose/approve flow. Transaction payloads are bounded to 16 MiB. Mixed signing
states, an omitted direct creation time, a payload from another chain,
authority, route, proof, or time, and a non-verifying signature fail before
queue submission.

Requests and responses use closed field sets and canonical JSON/base64/Norito
encodings. The unified response field is
`route_configuration_hash_hex`; `manifest_hash_hex` and other retired aliases
are unknown fields, not compatibility spellings. Prepared responses must carry
the exact transaction-payload prehash, and submitted responses must not retain
the preparation scaffold.

Torii authenticates and rate-limits both submit endpoints before polling their
bodies. It rejects malformed, duplicate, conflicting, overflowing, or lying
`Content-Length` framing with `400`, and rejects declared or streamed bodies
above the endpoint/operator cap with `413`. Chunked bodies remain bounded by
actual decoded bytes, and accepted bytes are restored exactly once for the
strict downstream JSON extractor.

Every supported SDK applies the same fail-closed response contract. Only exact
HTTP `200` is success. JSON responses require `application/json` (parameters
such as `charset=utf-8` are allowed, structured-suffix aliases are not), while
binary responses require `application/x-norito`. The capabilities endpoint is
bounded to 64 KiB, recent messages to 8 MiB, and other SCCP JSON responses to
64 MiB. Native-proof Norito responses are bounded to 16 MiB; destination-proof
Norito responses allow the same 16 MiB proof budget plus 64 KiB of envelope
overhead. Non-success bodies use the same endpoint-specific bound.

Declared lengths are canonical, unambiguous preflights, never the authority for
allocation or completion. Transports stream and count the actual post-decoding
body, cancel/close it on the first over-limit chunk, and enforce the bound when
`Content-Length` is missing, understated, or describes compressed bytes. An
empty body, invalid UTF-8 JSON, ambiguous framing, the wrong media type, or a
trailing/oversized Norito envelope fails before model parsing.

Every SDK preflights SCCP binary values as canonical, uncompressed NRT0 frames
with a nonempty payload, valid CRC64, the exact endpoint-specific schema hash,
and exactly zero header-padding bytes. Destination submissions are bound to
`iroha_sccp::SccpGroth16Bn254ProofArtifactV1`; native submissions are bound to
`iroha_sccp::native_admission::SccpNativeInboundMessageProofV1`. Typed registry,
message-bundle, and proof-request reads likewise bind their own exact schemas.
Cross-type replay, one-, eight-, and 64-byte padded alternatives, nonzero
padding, compression, unknown flags, bad lengths, checksum drift, and trailing
bytes fail before a request is sent or opaque response bytes are returned. This
framing check does not claim to decode or bind the embedded SCCP message id.

`GET /v1/sccp/capabilities` requires two closed limit objects in every V1
response. `registry_limits` advertises the fixed lane/live/retained capacities,
while `resource_limits` advertises every consensus-critical `[zk.sccp]` value.
Rust, Swift, Kotlin, Java, JavaScript/TypeScript, Python, and .NET clients reject
missing, unknown, zero, reversed transaction/block, or drifted fixed-registry
limits before accepting the capability snapshot. The five byte-budget fields
are also restricted to canonical unsigned JSON integer tokens no greater than
`9,007,199,254,740,991` (`2^53 - 1`). Fractional, exponent, signed, leading-zero,
and larger spellings are rejected before a runtime with binary floating-point
numbers can round them into a different consensus limit.

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
8. requires `semanticProofProfileHash()` and `soraFinalityAnchorHash()` to
   equal the independently derived hashes of the governed typed policy; and
9. derives and compares destination-binding, concrete route-config, and
   governed-route configuration hashes with canonical Rust APIs.

The release timestamp must be no more than 24 hours after the authenticated
readback and must not precede it. Missing authenticated destination state is
`unavailable`, never `verified`.

The readiness report remains `ready: false` while any required ETH, BSC, or
TRON capability is unavailable.

## Semantic circuit policy

Pairing-valid algebra is not sufficient. The production trust policy pins, per
profile:

- the exact profile-specific semantic circuit id;
- the complete ordered semantics
  `sccp-canonical-transfer-v1`, `sccp-message-leaf-v1`,
  `sccp-merkle-inclusion-v1`, `sora-taira-block-commitment-v1`,
  `sora-taira-commit-qc-v1`, and
  `sora-taira-anchor-continuity-v1`;
- the exact compiled circuit/proving-key artifact SHA-256;
- the exact reproducible witness-generator SHA-256;
- the fixed ordered eleven-public-signal schema hash;
- the domain-separated semantic-profile hash derived from those three roles;
- a typed Taira checkpoint, validator-set epoch/hash, and hash version, plus
  the independently derived finality-anchor hash;
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
is explicitly forbidden. Circuit ids containing `smoke`, `test`,
`signal-binding`, or `labeled-signal` are also rejected, and the exact SHA-256
of the checked-in labeled-signal-only circuit is forbidden independently of
its name.

The verifier consumes exactly eleven public signals. Signal 10 is the governed
Taira finality-anchor hash; there are twelve IC points including the constant
point, and the canonical verifying-key preimage is exactly 38 ABI words.
Ten-signal, eleven-IC-point, and 36-word representations are invalid. The policy
therefore establishes that the deployed key proves canonical transfer
semantics, message inclusion, Taira commit-QC finality, and continuity from the
governed checkpoint rather than merely checking a syntactically valid Groth16
equation. The bundle commits the complete policy hash.

Production evidence must also carry the actual content-addressed bytes, not
only their policy digests. For every profile, both auditors independently sign
the same closed seven-role inventory: semantic circuit artifact, witness
generator, verifying key, prover build, toolchain lock, honest witness, and
canonical honest-proof Norito artifact. Their canonical reports must contain
the identical artifact metadata and identical honest-proof claim. The claim
binds the governed route, message, payload, commitment, finality, destination,
request/result, verifier key, semantic profile, finality anchor, and the exact
ordered eleven 32-byte public-signal words.

The honest proof is bounded by the consensus proof-artifact ceiling of 16 MiB
plus 64 KiB. Each other semantic artifact is bounded to 64 MiB, each audit
report to 2 MiB, the complete inventory to 64 files and 256 MiB, and every
content digest is role-separated. Empty, all-zero, fixture-only, smoke-test,
unlisted, substituted, shared-across-role, or report-disagreeing material fails
closed.

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
- the closed semantic artifact set and two signed canonical audit reports per
  profile, including one distinct honest witness/proof pair per profile;
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
inventory, bounds, path safety, hard/symbolic links, credential leakage, exact
semantic roles, and agreement between both auditors. It invokes the Rust
validator independently for all three lane artifacts and all three honest
proofs. Each invocation receives the bytes plus the complete signed policy and
evidence; Rust re-verifies the signatures, selects the profile-pinned attestor
and circuit, and binds the result to the signed artifact digest and status.

For an honest proof, Rust additionally canonical-decodes the Norito artifact,
validates target/backend, derives the exact eleven signals from the governed
statement and full Taira finality anchor, verifies the BN254 pairing, checks the
circuit/witness/key/build/toolchain metadata and route revision, and emits a
canonical receipt. Python requires that receipt to equal the claim signed by
both auditors exactly. There is no CLI that accepts a Python-projected attestor
key, verifier hash, revision, signal vector, or runtime hash. Python
independently recomputes only the two small, fixed policy hashes (semantic
profile and Taira anchor) and pins their Rust/Solidity golden vectors; lane,
message, route, destination, public-signal, and verifying-key preimages remain
solely in the canonical Rust/data-model implementation.

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
- mixed detached-signing states, payload/authority/time/proof substitution,
  wrong Taira discriminants, and preparation/direct response contradictions;
- generic ICS/transparent-ZK bridge submission and no-BLS finality fallback;
- unapproved circuit ids, incomplete semantics, signal-only circuit replay,
  witness/schema/profile substitution, finality-anchor drift, audit/report
  drift, smoke VK, and policy substitution;
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
