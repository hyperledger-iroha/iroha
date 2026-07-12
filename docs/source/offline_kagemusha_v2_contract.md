# Kagemusha recursive spend V2 contract

This document defines the first-release wire contract for exact fractional
transfers, one- or two-input joins, and independently redeemable sender change.
There is no deployed compatibility surface to preserve: implementations use
the single canonical layout below and reject retired pre-release shapes.

## Wire types

The canonical Norito names are:

- `KagemushaScaledAmountV2`
- `KagemushaSpendableNoteDescriptorV2`
- `KagemushaRecipientPaymentRequestV2`
- `KagemushaRequestAuthorizationV2`
- `KagemushaRecursiveSpendInitRequestV2`
- `KagemushaRecursiveSpendTopUpRequestV2`
- `KagemushaRecursiveSpendTopUpAnchorV2`
- `KagemushaRecursiveSpendTopUpAnchorRefV2`
- `KagemushaRecursiveSpendSplitIntentV2`
- `KagemushaRecursiveSpendAppendRequestV2`
- `KagemushaRecursiveSpendBranchV2`
- `KagemushaRecursiveSpendBundleV2`
- `KagemushaRecursiveSpendSplitResultV2`
- `KagemushaRecursiveSpendPeerPaymentV2`
- `KagemushaRecursiveSpendVerifyRequestV2`
- `KagemushaRecursiveSpendVerifyResultV2`
- `KagemushaRecursiveSpendLineageNodeV2`
- `KagemushaRecursiveSpendLineageWitnessV2`
- `KagemushaReceiverAcknowledgementPayloadV2`
- `KagemushaReceiverAcknowledgementV2`
- `KagemushaReceiverAcknowledgementVerifyResultV2`
- `KagemushaRecursiveSpendRedeemRequestV2`

`KagemushaScaledAmountV2` carries positive `u128` `atomic_units` and the
authoritative asset-definition `scale` (0 through 28). Public charging and
crediting use `Numeric(atomic_units, scale)`. Proof statements use
`atomic_units`. Conversion from a public `Numeric` is exact: implementations
reject excess precision and overflow and never round.

`KagemushaRecursiveSpendBundleV2` carries that authoritative scale through
every offline hop. Each split intent must use the parent bundle's scale, each
result branch preserves it, and redemption must match it. Relabeling the same
atomic note with a different scale is invalid. Top-up chain execution must also
compare the request scale to the live asset definition before debiting funds.

The chain-facing top-up and redeem requests bind a nonzero stable operation id
so a retry cannot create a second economic operation. Chain operation ids use
one global namespace across authorities and operation kinds, matching the
globally keyed top-up-anchor receipt; nonces, payload digests, and exact-request
replay markers remain authority-scoped. The local
`KagemushaRecursiveSpendInitRequestV2` is deliberately flat and anchor-derived.
It contains exactly the finalized `topup_anchor`, checked one-hop
`record_bundle`, `pallas_open_envelopes_archive`, `lineage_mode`, and optional
`lineage_artifact`. It does not nest the discarded pre-release init request or
duplicate amount, current note, operation id, inline keys, or an optional block
height. Amount,
note, operation identity, artifact generation, and verifier lifecycle height
come from the finalized anchor. Before native dispatch, the checked transfer
must match the anchor's chain, asset, initial/final roots, input nullifiers,
single output commitment, verifier id, verifier commitment, and finalization
height. The append request binds the receiver's nonce-bearing request digest,
the canonical parent bundle digests, recipient output, optional change output,
exact transfer amount, and operation id.

Before a sender reserves inputs or performs proof work, it validates the
receiver-device signature on `KagemushaRecipientPaymentRequestV2`. The signed
payload includes chain, asset, exact amount, recipient, output commitment and
opaque prover material, request nonce, issuance/expiry, registered device id,
device public key, and its domain-separated key reference. Its lifetime is also
bounded to five minutes. The later acknowledgement must use exactly the same
device id, public key, and key reference.

Top-up and redemption use a self-contained signed
`KagemushaRequestAuthorizationV2`. The authorization binds the complete
unsigned request digest, authority, device id, operation id, nonce, issuance
and expiry, and optional App-Attest evidence digest. Its lifetime is nonzero
and no longer than five minutes. Top-up requires the authority to equal the
charged asset account; redemption requires it to equal the credited recipient.
The chain transition must consume the operation id and nonce atomically with
the debit or credit. Torii must validate attached App-Attest evidence against
the registered device lineage; the model's evidence hash and account signature
checks are necessary but not a substitute for platform attestation validation.

Append returns one `KagemushaRecursiveSpendSplitResultV2`, never a single
ambiguous bundle. The result carries the shared split statement and binding
digest, a branch-tagged recipient bundle, and a branch-tagged change bundle
exactly when `change_output` is present. Each bundle repeats the same split and
binding digest. Both branch accumulators must share chain, asset, initial/final
roots, compact top-up anchor references, and hop count, while their current
notes and recursive proofs are distinct. Full finalized anchors remain in
chain state and semantic init-node archives; spendable peer bundles carry only
the strictly ordered `(topup_operation_id, anchor_digest)` identity pair. This
lets transport and durable wallet staging treat recipient and sender-change
outputs as separate spendable states without growing every peer payload with
full anchor receipts.

`KagemushaRecursiveSpendPeerPaymentV2` is the recipient-only transport type and
contains exactly one field: `recipient_bundle`. Its stable operation id and
recipient-request digest are read from the bundle's proof-bound recipient
`PeerSplit` transition. They are not repeated as peer-payment fields, so there
is no second identity source that can disagree with the proof statement.

## Split and lineage invariants

For input value `I`, recipient value `R`, and optional change `C`:

- `R > 0`; no-change requires `R = I`; change requires `R + C = I` and
  `R < I`, `C > 0`, with checked `u128` arithmetic.
- Every note commitment and spend nullifier across input, recipient, and change
  is nonzero and pairwise distinct. The checked-hop statement contains exactly
  the input nullifier and the recipient/change commitments.
- The recursive proof binds the whole split intent and its parent accumulator.
  Host-side validation alone is not admission evidence.
- Recipient and change become separate branches with disjoint branch
  nullifiers. Redeeming either branch MUST NOT consume a shared ancestor in a
  way that invalidates the other branch.
- The parent may be split exactly once. Replaying the operation, spending the
  parent after a split, redeeming an ancestor after a descendant, redeeming a
  branch twice, and submitting overlapping sibling branches all fail closed.
- Each branch remains recursively spendable and independently redeemable while
  total value across all live/redeemed descendants never exceeds the top-up.

Branch coordinates use one lineage root and a 64-bit MSB-first path. A
recipient appends bit 0 and change appends bit 1. Equal and prefix-related
coordinates conflict; siblings do not. `proof_step_count` includes top-up and
redemption-change transitions, while `peer_hop_count` is zero at top-up and
increments only for offline peer transfers.

Each branch claim carries exactly `path.depth` transition tags, with no padded
slots. A tag is
`SHA-256("iroha:kagemusha:v2:transition-tag:sha256-192" || 0x00 || transition_digest)[0..24]`,
where `transition_digest` is the complete proof-bound 32-byte digest and the
result must be nonzero. On the wire, `transition_tags` is one `Vec<u8>` whose
length is exactly `path.depth * 24`; consecutive 24-byte slices are the tags.
It is not a vector of nested fixed-byte arrays. Claims and compact anchor
references are strictly ordered and unique. Every claim lineage root must equal
one referenced anchor digest; the ledger resolves each compact reference back
to the complete finalized anchor before crediting a redemption.

These properties require the recursive accumulator/circuit and chain state
transition. The discarded single-parent prototype stored one current note and
consumed shared top-up anchors at redemption, so cloning two bundles from one
1-to-2 hop could not provide independent change: the first redemption would
invalidate the sibling.

## Durable receiver acknowledgement

After verification and atomic persistence, the receiver signs a canonical
`KagemushaReceiverAcknowledgementPayloadV2`. It binds the sender operation id,
receiver-request digest, accepted recipient-bundle digest, recipient
commitment, one captured acceptance time, registered receiver device id, and a
domain-separated receiver public-key reference. The sender verifies the
request, bundle, commitment, key reference, and device signature before
committing its reserved inputs.

The receiver stores the final acknowledgement archive under
`(operation_id, recipient_request_digest)` in the same durable transaction as
the accepted bundle. Both values come from the accepted bundle's recipient
`PeerSplit` transition, not duplicated peer-envelope fields. Duplicate delivery
returns those exact bytes; it must not generate a fresh timestamp or signature.

## Fragmented balances and multi-input payments

The canonical split consumes one or two parent bundles. Parent bundles are
strictly ordered by bundle digest; their compact top-up references are merged,
deduplicated, and strictly ordered. Checked `u128` addition conserves the sum
of both inputs into one recipient output and optional sender change. Mixing
parents from another chain, asset, scale, lineage mode, artifact generation,
or an alternative transition choice is invalid.

Semantic mode carries a bounded canonical lineage DAG rather than a linear
archive list. Each node identifies its result bundle, zero to two sorted parent
digests, proof-step count, verifier-check height, and opaque typed transition
archive. Nodes are uniquely ordered by `(proof_step_count,
result_bundle_digest)`, contain the complete ancestor closure, and have one
final sink. Limits are 64 nodes, 64 KiB per transition archive, 2 MiB total,
and eight semantic peer hops. Root nodes retain the full finalized anchor and
the proof-bearing init bundle; peer nodes retain the full split preimage and
selected result bundle. This makes cross-top-up joins verifiable without
putting full ancestry into every spendable peer bundle.

## Availability and rollout

`KAGEMUSHA_RECURSIVE_SPEND_V2_PROOF_BACKEND_AVAILABLE` is currently `false`.
The data model, Swift exact-amount API, and proof-independent native top-up
bridge builder are available for integration and fixture convergence. Core
execution of V2 top-up instructions and all proof-gated init, append,
redeem-change, verify, and redeem entrypoints MUST return
`RecursiveSpendV2ProofBackendUnavailable` before proving or state mutation. No
funded state from a discarded pre-release shape is accepted or migrated.
Wallets MUST disable fractional/split transfers and MUST NOT emulate them by
cloning pre-release bundles or trusting a host-only split.

The proof-gated native symbol names are
`connect_norito_kagemusha_recursive_spend_init_v2`,
`connect_norito_kagemusha_recursive_spend_append_v2`,
`connect_norito_kagemusha_recursive_spend_redeem_change_v2`,
`connect_norito_kagemusha_recursive_spend_verify_v2`, and
`connect_norito_kagemusha_recursive_spend_redeem_v2`. Each is exported as an
ABI-18 fail-closed stub while the availability constant is false. ABI 18 also
exports `connect_norito_kagemusha_recursive_spend_capabilities_v1`; callers
must require its `proof_backend_available` field and must not infer readiness
from symbols. It selects artifact manifest
`kagemusha.offline.recursive_spend.artifact_manifest.v3`, mode
`recursive_spend_v2`, proof backend `halo2/ipa-pasta-cycle-v1`, and transcript
profile `kagemusha-pasta-cycle-poseidon-v1`. V3 artifact files are framed with
`KRV3KEY\0`; the retired V2 artifact spool is not exported by the first-release
bridge.
Use the maintained `kagemusha_recursive_spend_v3_bundle` binary documented in
`offline_kagemusha_recursion_adapter.md` to frame an externally generated,
reviewed 2×3 artifact set and its canonical top-up finality roster; ABI-7
material and generators are not compatible. This packager is an unsigned
staging step. Its canonical `manifest.norito` is the runtime input and
`manifest.norito.sha256` is only a content identifier until authenticated by
the separate release envelope; `manifest.json` is an operator view. Wallets
and native verification must use the authenticated manifest SHA and exact
roster/artifact digests, never generation labels alone.
Until that signer/policy-bound release-envelope verifier exists,
`authenticated_release_envelope` remains a canonical missing capability gate
and the proof backend remains unavailable. Recursive init also does not yet
consume a verified top-up-finality capability, so
`topup_finality_bound_init` is a separate missing gate. The public
`connect_norito_kagemusha_topup_finality_verify_v2` symbol fails closed until
both gates are wired; a caller-supplied manifest plus its self-hash cannot
select the BLS trust root.
The
proof-independent `connect_norito_kagemusha_recursive_spend_topup_v2` path is
part of the protocol-symbol inventory and validates the finalized transfer and
debit contract without pretending that recursive proving is available. Append
reserves a `KagemushaRecursiveSpendSplitResultV2` output archive. Standalone V1
lineage-witness verification reserves
`connect_norito_kagemusha_recursive_spend_lineage_witness_verify`;
the current bridge verifies that witness only inside redemption, so the Swift typed
verifier also reports unavailable rather than substituting structural parsing.

Production availability requires all of the following in one release:

1. A recursive circuit exposing the split-binding digest and independent branch
   lineage/nullifiers.
2. Native bridge append/verify/redeem entrypoints for the V2 Norito types.
3. Scale-aware V2 chain instructions that charge/mint
   `Numeric(atomic_units, scale)` and verify the live asset definition has the
   same scale.
4. Replay/ancestor/sibling conflict tracking plus conservation and independent
   sibling redemption tests.
5. Signed Reserved-lineage init/append artifacts and the advertised production
   performance gates.
6. One- and two-input circuit tests covering compact-anchor resolution,
   semantic DAG closure, conservation, and alternative-branch rejection.

The first-release public HTTP lifecycle is deliberately small:

- `GET /v1/offline/readiness?asset_definition_id=...`
- `POST /v1/offline/top-up`
- `POST /v1/offline/redeem`
- `GET /v1/offline/operations/{operation_id}`

Readiness returns `evaluated_block_height` and an exact 64-character lowercase
`evaluated_block_hash` from the same committed state view. Wallets use that
pair as the recent-block anchor for device-attestation registration; they must
not combine a height and hash from independent reads. The same snapshot carries
required nullable `active_transfer_verifier` and
`active_topup_shield_verifier` projections. Each non-null verifier must be
active at the evaluated height. A null value is valid exactly with its matching
`transfer_verifier_unavailable` or `topup_shield_verifier_unavailable` blocker,
and `ready: true` requires both verifier roles.

The POST body is the typed `OfflineTopUpRequest` or `OfflineRedeemRequest`
itself. With `Content-Type: application/json` (optionally one
`charset=utf-8` parameter), clients send its structured JSON representation.
With the parameter-free `Content-Type: application/x-norito`, clients send the
canonical typed Norito value directly. Other request media types, suffix JSON
types, and unrecognized parameters are rejected rather than interpreted by
fallback. There is no object whose purpose is to wrap the entire request in a
`*_norito_base64` field.

When API-token authentication is enabled, Torii validates the exact singleton
token after the bounded connection pre-authentication gate admits a request and
before `Content-Type`, `Idempotency-Key`, or body decoding. Missing, invalid,
and duplicate token fields therefore fail with the typed authentication
response without reading a malformed command body. With a valid token,
route-level access/rate policy runs before exact media and command-header
validation; typed body decoding is the final admission-boundary step.
Malformed or unacceptable `Accept` input likewise cannot mask a pre-auth or
token rejection: Torii emits the primary rejection as deterministic JSON when
the preference itself cannot be used. Strict negotiation resumes immediately
after successful authentication and returns `406 response_not_acceptable`
before command-body admission.

The public Norito headers use stable schema names rather than Rust module or
implementation-type names:

- `OfflineTopUpRequest`: `iroha.torii.v1.offline.top_up.request`
- `OfflineRedeemRequest`: `iroha.torii.v1.offline.redeem.request`

Each header carries the first 16 bytes of the domain-separated Norito schema
hash for that name. Moving or renaming the private Rust implementation does not
change the public request schema.

### Representation and evolution rules

The JSON form is the Norito JSON mapping of the same DTO, not an independently
shaped compatibility payload. Struct fields use their declared snake-case
names; tagged enums use the documented `tag` and `content` fields. An
unannotated fixed `[u8; N]` is a JSON string of exactly `2 * N` hexadecimal
digits; canonical output is uppercase and the decoder accepts either case. A
field using the `fixed_bytes` helper and an ordinary `Vec<u8>` instead use
integer-byte arrays unless that field declares another textual helper. Hashes,
keys, signatures, account identifiers, and numeric values use the mapping
declared by their data-model type. In particular, wide integers are lossless
JSON integers; clients must not round them through an IEEE-754 `double`. An
`Option` is emitted as its typed value or `null` unless its field explicitly
omits `None`; decoders also treat an omitted optional field as `None`. Duplicate
declared fields, unknown enum discriminator values, non-finite numbers, and
out-of-range integers are invalid. Decoders ignore unknown object members. Unit
enum content is emitted as explicit `null`, while decoders also accept an
omitted content member as the same unit value. Signatures and payload digests
cover canonical typed Norito bytes, never the lexical spelling, member order,
or ignored members of input JSON.

Before the first release is published, pre-release DTOs and wire layouts are
replaced directly when correctness or clarity requires it. This is a sharp
cutover, not a migration: the schema names and field sets documented here are
the sole supported release target, with no compatibility period or alternate
decoder. Implementations must not retain fallback decoders for the retired
nested-init, full-anchor peer bundle, duplicated peer-payment identity, fixed
padded or nested branch-history, or linear semantic-witness shapes. At
publication, these public DTOs follow the `/v1` evolution policy in
`torii/api_contract.md`.
Internal consensus/proof type names may retain implementation version suffixes,
but those suffixes never appear as nested HTTP path versions or response-level
negotiation.

Top-up and redemption are asynchronous. Acceptance returns `202 Accepted`, a
typed `OfflineOperationReference`, and a `Location` header pointing to the
operation status resource. Polling returns a tagged `pending`,
`applied { result }`, or `rejected { error }` state; a successful top-up result
contains its typed finalized anchor and the mandatory typed Sumeragi finality
proof for that exact anchor, while redemption does not carry irrelevant
nullable top-up fields. Neither field is a base64 archive wrapper. Identical
retries use the signed operation id and idempotency key to resolve to the same
operation.
Every applied result has a non-zero `finalized_block_height` and
`server_time_ms` recovered from the exact canonical carrier block. Torii never
substitutes zero for unavailable finality metadata; a missing or inconsistent
index, carrier, timestamp, or finalized top-up anchor returns typed
`503 offline_operation_index_inconsistent` (or the more specific documented
index/history-unavailable code) instead of an applied status or client error.
If the operation and anchor are finalized but the local immutable finality
artifact or top-up path sidecar is not available, Torii returns
`503 offline_topup_finality_proof_unavailable`; clients retry the same status
resource and must not construct a spendable initial bundle.

`Idempotency-Key` is required on both commands and is exactly the lowercase
hexadecimal form of the request authorization's 32-byte `operation_id`. The
operation id is globally unique across the offline command namespace and binds
the command route plus the complete signed authorization and canonical typed
request. Reusing the key with the identical signed request returns `202` with
the original operation URI and transaction hash, including after the operation
has reached a terminal state. A header that does not match the embedded signed
identifier returns `409 idempotency_key_conflict`; reusing an operation id for
another route, authorization, or payload returns `409 operation_id_conflict`
while the original queue, admission-cache, or committed-block record is
retained. Clients must treat the identifier as globally single-use and must not
recycle it after a status lookup returns `404`.
Torii resolves an identical replay against its admission registry, transaction
queue, and retained committed blocks before applying current snapshot or issuer
policy checks. It returns the original transaction hash and therefore does not
depend on a signing implementation reproducing identical signature bytes.
Concurrent identical submissions on one Torii node share an in-flight admission
coordinator: duplicate callers wait for the leader's queue result and receive
`202` only after that transaction has actually been admitted. If the leader
fails or is cancelled, waiters retry recovery instead of receiving a provisional
operation reference.
Pending operations are recovered from the transaction queue and committed
applied or rejected operations through Kura's operation-id index and one exact
retained carrier-block read. When the operation executed through a lane merge,
Kura also resolves that carrier's full merge entry by its sparse exact-height
index and revalidates the canonical block hash, compact reference, and durable
merge-log entry. A status miss never scans retained chain or merge history.
While either index is being reconstructed, or when the indexed block body or
merge entry is unavailable, Torii returns `503` with
`offline_operation_index_unavailable` or `offline_operation_history_unavailable`;
an index/body/result disagreement returns
`offline_operation_index_inconsistent`. These states never guess a `404`. The
admission registry may bridge the short interval between queue admission and
authoritative pipeline visibility, but it cannot manufacture an unbounded
pending state. Once the signed expiry is behind the latest chain snapshot, an
accepted binding with no queue, pipeline, or canonical terminal provenance
returns `503 offline_operation_index_inconsistent` instead of `pending`. The
auxiliary admission registry is a process-local optimization and is not
restart-persistent. It retains only a fixed-size binding: command kind,
operation id, the hash of the canonical Norito request bytes, transaction hash,
and signed submission/expiry timestamps. It never retains the proof-bearing
request DTO; status and replay checks recover that complete request from the
transaction queue or the indexed Kura carrier block. Accepted bindings and
in-flight reservations share one atomic registry and the same capacity. It is
bounded by both `torii.offline_issuer.operation_registry_max_entries` (default
4,096) and `torii.offline_issuer.operation_registry_max_bytes` (default 524,288
canonical accounted bytes). Entry count and byte budget must be positive, and
the byte budget must fit at least one binding. Canonical accounting reserves
113 bytes for every accepted binding or in-flight identity independently of
allocator or host architecture; the entry limit separately bounds map and
coordination-object overhead. Expired accepted bindings are pruned
opportunistically only after signed expiry plus 24 hours. Active and retained
bindings are never evicted for capacity: a new unique operation instead gets
`503 offline_operation_capacity_exhausted`, while identical accepted replays
and in-flight followers bypass the capacity check. Dropping a failed or
cancelled leader atomically releases its reservation, and admission atomically
replaces a successful reservation with its accepted binding. Torii also repeats
authoritative queue/Kura recovery after electing a leader, so expiry pruning
cannot create a replacement-submission interval. If an already queued command
cannot complete that reservation-to-binding transition, its leader fails
closed with `503 offline_operation_admission_inconsistent` and followers retry
authoritative recovery; Torii never publishes a cache-only accepted result.
Committed results remain discoverable while the corresponding block is
retained.

Both the pending lookup and Kura index bind the signed operation id to the
configured outer Offline issuer authority. A different transaction authority
cannot front-run an observed signed request, commit a rejected transaction, and
shadow the later issuer submission under the same operation id. Deployments
must retain the issuer identity for as long as they promise operation-status
recovery; changing it creates a distinct recovery namespace.

The in-flight coordinator, admission registry, and transaction queue are local
to one Torii process. A load balancer must keep a command submitter and its
pending status polls on the accepting instance until commit. Before commit, a
status lookup routed to another instance can return `404`; that miss never
permits reuse or replacement of the globally single-use operation id. Every
replica that accepts Offline commands for one deployment must use the same
issuer identity and behaviorally identical issuer policy so an identical
signed request constructs the same canonical transaction. Independent replicas
can nevertheless race to admit that candidate, so consensus and the on-chain
operation-id uniqueness rule—not a distributed idempotency cache—provide the
final at-most-one-economic-effect guarantee. After commit, synchronized
replicas resolve the terminal state through Kura's operation-id index while the
indexed block is retained. A deployment that cannot provide pre-commit
instance affinity must not expose these command routes until it provides shared
admission coordination.

Accepted operation references and pending status responses include
`Retry-After: 1`. Every response from the top-up, redeem, and operation-status
resources—including `202`, `400`, `401`, `404`, `429`, and `503`—uses
`Cache-Control: no-store`, so neither a command outcome nor a pre-submission or
cross-instance miss can be reused after the operation becomes visible.
Readiness failures also use `no-store`; successful readiness evaluations use
the explicit private revalidation policy below. Economic command authorization
is carried by the signed request body. Separately, when Torii's API-token policy
is enabled, all
four lifecycle routes require the configured `x-api-token`; a missing or invalid
token returns `401 api_token_required` with
`WWW-Authenticate: IrohaApiToken realm="torii"`. Operation status remains
non-secret chain-status data keyed by the non-zero operation id: possession of
the id is not itself an authorization credential and does not bypass the
transport token policy. Neither command nor status routes are projected as MCP
tools. Only top-up and redeem document typed `403` rejection for the
authenticated signed-body/header-policy check; readiness and operation status
do not expose that command-only response.

Readiness is a domain-state read. The selector may be a canonical
asset-definition address literal or a currently live asset alias; the response
always contains the resolved canonical id. When Torii evaluates the requested
asset but offline payments are not ready, it returns `200 OK` with
`ready: false`, typed blockers, the evaluated block height, and
the authoritative nullable `u32` asset scale. The response keeps an
out-of-policy scale above 28 intact together with `asset_scale_unsupported` so
clients can decode the expected unavailable state; only `ready: true` implies a
scale in the supported 0-through-28 range and non-null active transfer and
top-up shield verifiers. It also carries
representation-specific strong `ETag` computed over the exact selected JSON
or Norito response octets, the header
`Cache-Control: private, max-age=0, must-revalidate`, and `Vary: Accept`.
`If-None-Match` supports a matching strong or weak validator and `*`;
a match returns `304 Not Modified` without a body. A
`503 readiness_unavailable` error means Torii could not perform the evaluation.
No alternate `/offline/v2`, note-issuer, audit, or wrapper-body HTTP surface is
mounted in this first release.
