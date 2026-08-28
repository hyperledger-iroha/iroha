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

[`SccpNetworkV1`] is a closed enum. The canonical production release-evidence
corridor profiles are:

| Profile | Domain | Canonical chain identity |
|---|---:|---|
| `ethereum-mainnet` | 1 | EIP-155 chain id 1 |
| `bsc-mainnet` | 2 | EIP-155 chain id 56 |
| `tron-mainnet` | 5 | mainnet network magic `0x2b6653dc` |
| `ton-mainnet` | 4 | global id `-239` and the canonical mainnet zero state |

Domain 3 is the exact implemented Solana testnet runtime and SDK profile, but it
is outside this production evidence corridor. Domain 4 is the mandatory TON
mainnet profile, with a native masterchain source proof and BLS12-381 outbound
proof path. TON testnet has an exact wire/runtime and SDK profile for schema and
conformance testing, but cannot satisfy a production evidence row. Release
validation therefore requires TON mainnet alongside Ethereum, BSC, and TRON,
and allows neither Solana testnet nor TON testnet to substitute for a mainnet
row.

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
of a canonical single-key Ed25519 account in the prime-order subgroup. Both
small-order and mixed-torsion points are rejected. A proof-authenticated
Taira-to-external sender may be a canonical single-key or multisig `AccountId`,
but every controller key must be Ed25519 or compressed secp256k1 in V1. Core
checks the typed controller and exact discriminant-`369` spelling before moving
assets into custody; the EVM/TVM and TON destination contracts independently
check the same AccountAddress tags, policy ordering, key admission, I105 round
trip, and checksum before proof dispatch. Other Rust-supported controller algorithms are
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
- `sccp_ton_destination_binding_hash_v1`;
- `sccp_exact_evm_xor_route_config_hash_v1`;
- `sccp_exact_tron_xor_route_config_hash_v1`; and
- `sccp_exact_ton_xor_route_config_hash_v1`.

For TON, the exact bidirectional bridge contract is both the native source
emitter and the destination route: its source-emitter address and code hash
must equal the destination route address and code hash. The Jetton master is a
separate contract role. The primitive TON destination-binding and route-config
preimages intentionally omit StateInit-derived addresses and initial-data
roots, because the data cells themselves store those commitments and including
them would require a cryptographic fixed point. The validated full governed
deployment stored in the registry and its signed destination readback bind both
addresses and both initial-data roots after StateInit derivation.

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
slot, BSC and TRON use the fully verified finalized native block height, and
TON uses the finalized masterchain sequence number.
An old anchor remains valid through its successor checkpoint inclusively (a
one-height overlap needed by native boundary proofs) and never beyond it; the
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

For TON, the governed deployment contains raw workchain/account addresses for
the Jetton master and bidirectional SCCP route; representation hashes for the
Jetton-master, Jetton-wallet, route, and linked verifier code cells; the exact
initial Jetton-master and route data-cell hashes; the complete BLS12-381
Groth16 key, circuit commitment, and proof-profile commitment; the mandatory
outbound proof policy; and the fixed amount multiplier. Both contracts are
first-release basechain contracts. The source emitter must select the same
route address, route code hash, and route-configuration hash as this deployment;
a distinct source-only contract is invalid.

TON deployment evidence carries complete, single-root ordinary code and data
BOCs. `ton_state_init_address_hash_v1` constructs the canonical StateInit with
absent `split_depth` and `special`, present code and data references, and an
empty library (`00110` in TL-B field order). The resulting representation hash
must equal the raw account id at workchain `0` for both the Jetton master and
the route. The pinned attestor's semantic readback must report Jetton storage
version 1, zero supply, and empty mint/burn replay dictionaries, plus route
storage version 1, zero refund sequence, and empty nonce, replay, and pending
dictionaries.

The verified TON destination call uses one exact typed 14-cell
`SccpFinalizeFromTaira` body BOC. Its root contains the opcode, query id, V1
schema, message id, and statement hash and references four linked public-signal
cells split `3/3/3/2`, one proof root with fixed compressed `A/B/C` cells of
`48/96/48` bytes, and one payload root with fixed
`50/100/100/remainder` cells. Canonical transfer payloads are therefore bounded
to 50 through 374 bytes, and generic snake-cell encodings are not accepted by
the contract boundary.

The TRON source route uses the exact
`transferToTaira(bytes,uint256,uint64 expectedNonce)` ABI. Successful execution
requires `expectedNonce == transferNonces(caller)`, writes that same value into
the canonical payload, and increments only the caller's counter. Different
senders may safely use the same nonce because the canonical payload and message
id also commit the sender address. Native admission reconstructs the complete
ABI call from the payload recipient, scaled amount, sender, and nonce, so the
retired two-argument selector, a stale or future caller nonce, and an exhausted
per-caller `uint64` nonce all fail closed.

Every retained TRON revision in one exact lane must use a distinct source route
address. Native transaction inclusion authenticates the call address and
arguments but not the emitted route-revision/configuration fields, while the
route contract stores those fields immutably. A legitimate successor therefore
requires a fresh deployment address; registry validation rejects address reuse
across staged, active, draining, paused, and retired revisions so one finalized
transaction cannot be relabeled under another revision. A registered TRON
revision remains permanently retained even while staged, preventing route
removal from forgetting the address replay boundary before a successor is
registered.

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
`82315c500d019c3dbcd730b73a5b9812ec0fb842b6ad012311e8103ec8a4c6ea`
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
| TON mainnet | native masterchain and shard proof verifier available |

Solana testnet and TON testnet are excluded from this four-profile production
evidence table. Neither can replace a required mainnet row through observer
assertions, environment variables, or release flags. This production boundary
does not remove their separately implemented runtime/schema surfaces.

The TON verifier starts from the governed zero state, checkpoint, post-state
root, and validator configuration. It authenticates a consecutive bounded
masterchain continuation using the exact ordinary-catchain or Simplex Ed25519
transcript, follows the finalized masterchain `ShardHashes` commitment to the
selected shard, proves the transaction's pre-state account code and route
configuration, and opens the exact outbound message. A shard post-state alone
is insufficient because another transaction could restore governed state.

Cryptographic finality is mandatory in every SCCP build. The crate has no
consensus-changing BLS feature switch: it always compiles the same
proof-of-possession, public-key, aggregate-signature, and quorum verification
paths for Taira and BSC finality. Structural parsers and proof-controlled
self-consistency helpers are diagnostics, not trust anchors. A validator binary
whose build metadata enables test-fixture code is not a production release
validator.

TON native admission has the same deterministic reservation boundary. Its
length-only work estimator charges the bounded BOCs, at most 64 masterchain
continuations, supplied Ed25519 signatures, and the conservative validator-key
upper bound before parsing cells, hashing, validating keys, or verifying
signatures.

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
For `H` BSC continuation headers, the complete contribution bound is
`64 × (H + 5 + ceil((H + 1) / 1000))`: `H` possible attestations, five active/
recent/pending anchor-context rosters, and every epoch roster that can occur in
the contiguous anchor-plus-continuation interval. At the 1,004-header maximum
this is 64,704 contributions.

## Deterministic verifier-work limits

Closed SCCP proofs use the dedicated, mandatory `[zk.sccp]` consensus limits;
they do not consume or inherit the unrelated confidential-proof counters. The
default first-release transaction/block limits are:

| Work dimension | Transaction | Block |
|---|---:|---:|
| proofs | 1 | 4 |
| canonical proof bytes | 8 MiB | 32 MiB |
| native continuation headers/blocks | 1,004 | 4,016 |
| Ethereum light-client updates | 128 | 512 |
| framed native-finality bytes | 8 MiB | 32 MiB |
| secp256k1 recoveries | 1,005 | 4,020 |
| BLS aggregate checks | 1,004 | 4,016 |
| BLS key/contribution work items | 131,713 | 526,852 |
| Ed25519 signature checks | 65,536 | 262,144 |
| TON Ed25519 validator-key checks | 198,656 | 794,624 |
| BN254 pairing-product checks | 1 | 4 |
| BLS12-381 pairing-product checks | 1 | 4 |

One proof may contain at most 8 MiB of canonical bytes. All limits are nonzero,
transaction limits cannot exceed block limits, and they are included in the ZK
consensus-policy hash. They have no environment-variable aliases: validators
must obtain identical values from configuration files.

Outbound SCCP messages have two additional fixed V1 limits that are not
configuration knobs: at most 512 successful messages per block and at most
4,096 canonical payload bytes per message. The consensus-critical `[zk.sccp]`
pending-outbox defaults are 65,536 payload-bearing messages and 268,435,456
canonical payload bytes. Operators may lower or raise the pending limits in the
shared validator configuration, but every validator must use the same values.

Core preflights proof count and bytes before any proof-controlled canonical
decode. It then derives hardware-independent work from bounded framing and
atomically registers the complete transaction/block delta before signature
recovery, BLS verification, BN254 pairing, or BLS12-381 pairing. An abandoned or rejected
transaction does not leak staged work into the block. Destination admission
conservatively reserves two passes over the maximum 31-validator Taira
roster, covering both key validation and all-signer PoP/aggregation. Ethereum
reserves one 513-key bootstrap plus up to 128 updates, each with 513 next-
committee key validations and 512 possible signer contributions. BSC reserves
all framed headers, the anchor seal and continuation recoveries, every possible
attestation, and all active/pending/epoch roster validation passes. These are
upper bounds, so different peer hardware cannot change admission results.
TON source admission reserves its Ed25519 signature and key-validation work
before BOC parsing. TON destination admission reserves one BLS12-381 Groth16
pairing-product check, while EVM, BSC, TRON, and the non-production Solana
profile reserve one BN254 check. The backend is a closed enum, so a caller
cannot select an unmetered curve.

## Outbound commitment, retention, and discovery

Every successful outbound message receives a `commitment_index` in block
execution order. Indices are dense and zero-based (`0..=511`); rejected or
rolled-back execution cannot consume an index. The block header's SCCP Merkle
root commits leaves in exactly that order. Lane, route, message-id, or map-key
ordering never substitutes for the commitment index, and a gap, duplicate,
swap, omission, extra message, or out-of-range index fails validation.

Before finality is published or a block body can be evicted, Kura writes one
immutable retained-block record containing the exact canonical header and the
canonical SCCP payload archive in commitment-index order. It validates every
payload, reconstructs the header commitment root, and rejects a conflicting
rewrite. The same version-3 record may contain the compact merge reference
extracted from the live canonical body for local historical sidecar service;
that field is not part of SCCP proof authority. Finality-proof,
message-bundle, proof-request, and recent-message reconstruction use this
root-authenticated archive. They never require the
historical block body and never treat a mutable WSV payload copy as proof
material. Restart performs the same header, canonical-hash, archive, and root
checks before serving history. Canonical version-2 records remain readable;
because they predate the optional merge field, Kura upgrades one only from the
exact still-present body before eviction and otherwise preserves its absent
witness without weakening SCCP proof authority.

`GET /v1/sccp/proof-requests/{message_id}` returns the concrete governed
request, not an opaque generic proof job: an EVM/BSC/TRON/Solana route returns
the canonical BN254 request and a TON route returns the canonical BLS12-381
request. `SccpDestinationProofRequestV1` is the internal closed classifier; the
wire response is the selected concrete request with its own exact schema.

Every V1 proof request exposes the governed policy as four required fields:
`semantic_proof_profile`, `semantic_proof_profile_hash`,
`sora_finality_anchor`, and `sora_finality_anchor_hash`. Core copies the two
typed values from the destination deployment's `outbound_proof_policy`,
recomputes both domain-separated hashes, and exposes all four on the request.
The destination binding, route-configuration hash, and statement commit both
hashes; the ordered public signals carry those governed commitments, with
signal 10 equal to the Taira finality-anchor hash. Admission reconstructs the
canonical request from the retained bundle and governed route and requires
exact equality. An omitted, zero, aliased, or drifting value or hash, a hash
that does not recompute from its typed value, or a request that no longer
matches the live governed deployment is rejected; there is no client-selected
fallback policy or legacy request decoder.

WSV separates bulky pending data from permanent replay metadata. A newly
committed message stores its canonical payload in the pending map and charges
both `[zk.sccp]` pending counters. When the exact destination proof is accepted,
the payload and its counter charge are removed atomically and replaced with a
fixed-size terminal descriptor. The global message locator and ordered history
index persist across that transition. Consequently the pending payload state is
hard-bounded by both configured maxima, while terminal replay descriptors,
their locator/index entries, and Kura's immutable retained history intentionally
grow to preserve permanent replay protection and historical proof service.
Operators must capacity-plan and monitor that permanent history; the limits do
not claim that total chain state is bounded.

`GET /v1/sccp/messages/recent` returns height-descending history and preserves
execution order within one height. Each item carries `commitment_index`. When
more entries exist, `next` contains the compound cursor `{ from, after_index }`
for the last returned item; the next request must send both values. This avoids
skipping or repeating entries when a page ends inside a 512-message block.
`after_index` without `from`, indices outside `0..=511`, non-canonical decimal
spellings, duplicate or unknown query fields, and limits outside `1..=50` are
rejected.

The retained-block and finality records are safety evidence, not evictable body
cache. Their bytes are included in Kura's total/operator disk-usage accounting,
but excluded from the evictable-storage budget so a body cap cannot deadlock the
write of the evidence required to make that body evictable. This is not a total
disk bound: immutable retained records grow with chain history.

## Proof-local typed admission

`BridgeProof` has no free-standing manifest field that can be reinterpreted by
the caller. Its payload owns a role-preserving `BridgeProofBinding`:

- `NativeProtocol(BridgeNativeProtocolProofV1)` carries a closed native backend,
  the exact historical SCCP route-configuration hash, and one canonical typed
  native envelope;
- `SccpDestination(BridgeSccpDestinationProofV1)` is the only SCCP destination
  submit envelope. It carries a closed EVM/TVM/Solana BN254 or TON BLS12-381
  backend, the exact historical route-configuration hash, and one canonical
  curve-specific destination artifact; and
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

1. **Prepare:** send canonical Taira `authority`, one explicit typed
   `fee_payment` payer selection, and the endpoint artifact, optionally with a
   positive `creation_time_ms`. Omit both `signature_b64` and
   `transaction_payload_b64`. The response has `submitted: false`, no
   `tx_hash_hex`. Torii runs the same authoritative fee-quote engine as
   `POST /v1/fees/quote` and returns the exact canonical quoted transaction
   payload plus its 32-byte signing prehash. Sign that returned payload without
   rebuilding or editing it. The payload carries the standard 100-second
   signature-bound transaction TTL and no nonce.
2. **Direct submit:** resend the same artifact and payer selection with both
   `signature_b64` and the quoted `transaction_payload_b64`, plus the exact
   positive `creation_time_ms`. Torii decodes and re-encodes the bounded
   payload, byte-compares its chain, authority, fee payer and exact sponsor
   revision, proof instruction, metadata, creation time, default TTL, and absent
   nonce, verifies the detached signature, and queues exactly that transaction.
   The response has
   `submitted: true` and `tx_hash_hex`, with no signing scaffold.

Detached signatures are canonical padded base64 of one nonempty, nonzero
generic signature payload, bounded to 16 KiB; they are not restricted to raw
Ed25519 length. The authority must be a single-key canonical Taira I105 account.
Multisig authorities must prepare the payload and use the normal multisig
propose/approve flow. Transaction payloads are bounded to 16 MiB. Mixed signing
states, an omitted direct creation time, a payload from another chain,
authority, route, proof, or time, and a non-verifying signature fail before
queue submission.

`fee_payment` is required in both states. Authority payment carries an empty
program selection; sponsored payment carries one exact immutable program ID
and revision. Legacy fee metadata and implicit sender or sponsor fallback are
rejected. The direct submission's selection, gas bound, and quoted maxima must
match the signed transaction payload exactly.

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
the closed `iroha_data_model::bridge::BridgeSccpDestinationProofV1` envelope;
its backend selects either `iroha_sccp::SccpGroth16Bn254ProofArtifactV1` or
`iroha_sccp::SccpTonGroth16Bls12381ProofArtifactV1`. Native submissions are bound to
`iroha_sccp::native_admission::SccpNativeInboundMessageProofV1`. Typed registry,
message-bundle, and proof-request reads likewise bind their own exact schemas.
Cross-type replay, one-, eight-, and 64-byte padded alternatives, nonzero
padding, compression, unknown flags, bad lengths, checksum drift, and trailing
bytes fail before a request is sent or opaque response bytes are returned. This
framing check does not claim to decode or bind the embedded SCCP message id.

`GET /v1/sccp/capabilities` requires two closed limit objects in every V1
response. `registry_limits` advertises the five fixed lane/live/retained
capacities. `resource_limits` has exactly 29 fields: the two fixed outbound
limits, the two configured pending-outbox limits, and all 25 verifier-work
limits from `[zk.sccp]`. Rust, Swift, Kotlin, Java, JavaScript/TypeScript, Python,
and .NET clients reject missing, unknown, zero, reversed transaction/block, or
drifted fixed limits before accepting the capability snapshot. All eight
`u64`-valued fields (`max_outbound_message_payload_bytes`, both
`max_pending_outbound_*` fields, the three `max_proof_bytes_*` fields, and both
`max_native_header_bytes_*` fields) are restricted to canonical unsigned JSON
integer tokens no greater than `9,007,199,254,740,991` (`2^53 - 1`). Fractional,
exponent, signed, leading-zero, and larger spellings are rejected before a
runtime with binary floating-point numbers can round them into a different
consensus limit.

## Outbound destination authentication

An outbound lane is `verified` only when the canonical Rust release validator
authenticates a complete destination-state statement. For all four production
profiles, the statement is signed by the per-profile Ed25519 destination
attestor pinned in the external production trust policy. The signature covers
the canonical Norito JSON statement under the
`iroha:sccp:destination-state-attestation:v1` domain.

For EVM, BSC, and TRON, the statement contains the typed governed route,
finalized chain identity and block, exact raw token/verifier/route runtime
bytecode, immutable contract readbacks, verifier-key hash, destination binding,
concrete route configuration, and governed-route configuration. The Rust
validator:

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

For TON, the signed statement binds mainnet global id `-239`, the canonical
zero state and authenticated masterchain position, the typed governed route,
and complete single-root ordinary BOCs for Jetton-master code and data,
Jetton-wallet code, route code and data, and linked verifier code. Rust checks
each BOC representation hash, requires the signed semantic zero-state fields,
derives both canonical StateInit account ids, and requires them to equal the
governed master and route addresses.
It then compares the full BLS12-381 key, circuit/profile commitments, policy
hashes, destination binding, concrete route configuration, and governed route
configuration. Signed attestation is the evidence authority; an unauthenticated
TON RPC self-report cannot make the lane verified. Rust does not execute the
Tolk storage decoder over the opaque initial-data BOCs, so production assurance
also depends on the policy-pinned attestor's TON decoder and the independent
audit of the checked-in Tolk storage layout. That residual boundary is why live
audited deployment evidence remains mandatory.

The release timestamp must be no more than 24 hours after the authenticated
readback and must not precede it. Missing authenticated destination state is
`unavailable`, never `verified`.

The readiness report remains `ready: false` while any required Ethereum, BSC,
TRON, or TON-mainnet capability is unavailable.

## Semantic circuit policy

Pairing-valid algebra is not sufficient. The production trust policy pins, per
profile:

- the exact profile-specific semantic circuit id;
- the complete ordered semantics
  `sccp-canonical-transfer-v1`, `sccp-message-leaf-v1`,
  `sccp-merkle-inclusion-v1`, `sora-taira-block-commitment-v1`,
  `sora-taira-v2-finality-artifact-v1`,
  `sora-taira-v2-dual-quorum-v1`, and
  `sora-taira-anchor-continuity-v1`;
- the exact compiled circuit/proving-key artifact SHA-256;
- the exact reproducible witness-generator SHA-256;
- the fixed ordered eleven-public-signal schema hash;
- the domain-separated semantic-profile hash derived from those three roles;
- a typed wire-revision-4 Taira checkpoint containing the chain id, height, block
  hash, context id, and finality-artifact hash, plus the independently derived
  finality-anchor hash;
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

Every curve consumes exactly eleven public signals, with signal 10 equal to the
governed Taira finality-anchor hash. A BN254 key has twelve IC points including
the constant and an exact 38-ABI-word canonical preimage; ten-signal,
eleven-IC-point, and 36-word representations are invalid. A TON BLS12-381 key
has the fixed compressed `alpha_g1`, `beta_g2`, `gamma_g2`, `delta_g2`, and
twelve-IC-point shape, and every point is canonically decoded and subgroup
checked. The curve-tagged policy therefore establishes that the deployed key
proves canonical transfer semantics, message inclusion, the complete
Sumeragi-v2 finality artifact and dual quorum, and continuity from the governed
checkpoint rather than merely checking a syntactically valid Groth16 equation.
The bundle commits the complete policy hash.

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
- the four audited semantic proof-system records.

Every identity and Ed25519 key must be distinct and valid in the prime-order
subgroup. The evidence document cannot introduce or replace a trusted key.
Release signatures are checked against the external policy.

The committed `sccp-release-test-trust-policy-v1` fixture is retained only as
negative evidence. Its Sumeragi protocol-v3 anchors are invalid under the sole
first-release protocol revision, v4. No runner validates, bundles, verifies, or
reseals that snapshot; `python3 scripts/sccp_release_fixture.py reject` asserts
that the current policy loader rejects it at the protocol boundary.

A future positive test fixture must be created from a fresh v4 policy and must
receive fresh external circuit-auditor and release-role signatures. The retired
v3 signatures cannot be reused or transformed, and no private signing material
may enter repository tooling, session directories, runtime files, transcripts,
bundles, or logs. Production entrypoints expose no fixture-key switch.

## Canonical release evidence

Build the read-only validator without the optional fixture feature:

```bash
CARGO_TARGET_DIR=/absolute/operator/path/sccp-validator-target \
  cargo build --release --locked --offline --no-default-features \
    --features dev-tools \
    -p iroha_sccp --bin sccp_release_evidence
```

Do not enable `test-fixtures` in production workflows or Make targets.
`dev-tools` is the required target-selection feature and the only accepted
production validator feature. It remains in the build identity; an empty,
duplicated, additional, historical, or unknown feature set is invalid.

One canonical `sccp-release-evidence-v1` document contains:

- release/hub identity and creation time;
- the external trust-policy id;
- the validator protocol, crate version, source hash, and build identity;
- exactly four lanes in canonical Ethereum/BSC/TRON/TON-mainnet order;
- four distinct typed lane-evidence artifacts, one for every lane;
- one transcript for every required corridor phase;
- the closed semantic artifact set and two signed canonical audit reports per
  profile (eight reports total), including one distinct honest witness/proof
  pair per profile;
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
validator independently for all four lane artifacts and all four honest
proofs. Successful validation yields four lane receipts and four
semantic-proof receipts, in the same canonical profile order. Each invocation
receives the bytes plus the complete signed policy and evidence; Rust
re-verifies the signatures, selects the profile-pinned attestor and circuit,
and binds the result to the signed artifact digest and status.

For an honest proof, Rust additionally canonical-decodes the Norito artifact,
validates target/backend, derives the exact eleven signals from the governed
statement and full Taira finality anchor, verifies the profile-selected BN254
or BLS12-381 pairing, checks the circuit/witness/key/build/toolchain metadata
and route revision, and emits a canonical receipt. Python requires that receipt
to equal the claim signed by both auditors exactly. There is no CLI that accepts
a Python-projected attestor key, verifier hash, revision, signal vector, or
runtime hash. Python independently recomputes only the two small, fixed policy
hashes (semantic profile and Taira anchor) and pins their Rust/Solidity golden vectors; lane,
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
input flag. Every Ethereum, BSC, TRON, and TON-mainnet inbound and outbound must
be verified for `ready: true`. Solana testnet and TON testnet remain outside the
production inventory and cannot satisfy or replace those four rows.

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
and independently verifies that bundle. The TON contract phase formats, checks,
builds, and runs the pinned Acton 1.1.0/Tolk 1.4.1 suite; its current local
conformance result is 20/20.

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

[`SccpNetworkV1`]: ../crates/iroha_data_model/src/bridge/sccp.rs
[`SccpLaneIdV1`]: ../crates/iroha_data_model/src/bridge/sccp.rs
[`SccpRegistryV1`]: ../crates/iroha_data_model/src/bridge/sccp_registry.rs
[`SccpGovernedRouteV1`]: ../crates/iroha_data_model/src/bridge/sccp_registry.rs
[`SccpNativeTrustAnchorV1`]: ../crates/iroha_data_model/src/bridge/sccp.rs
