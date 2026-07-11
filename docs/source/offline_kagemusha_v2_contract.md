# Kagemusha recursive spend V2 contract

This document fixes the additive wire contract for exact fractional transfers
and independently redeemable sender change. V1 remains unchanged. Implementations
MUST NOT reinterpret V1 integer note amounts as public scale-zero asset amounts.

## Wire types

The canonical Norito names are:

- `KagemushaScaledAmountV2`
- `KagemushaSpendableNoteDescriptorV2`
- `KagemushaRecipientPaymentRequestV2`
- `KagemushaRequestAuthorizationV2`
- `KagemushaRecursiveSpendInitRequestV2`
- `KagemushaRecursiveSpendTopUpRequestV2`
- `KagemushaRecursiveSpendSplitIntentV2`
- `KagemushaRecursiveSpendAppendRequestV2`
- `KagemushaRecursiveSpendBranchV2`
- `KagemushaRecursiveSpendBundleV2`
- `KagemushaRecursiveSpendSplitResultV2`
- `KagemushaRecursiveSpendVerifyRequestV2`
- `KagemushaRecursiveSpendVerifyResultV2`
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

The init, top-up, and redeem requests bind a nonzero stable operation id so a
retry cannot create a second economic operation. The append request binds the
receiver's nonce-bearing request digest, the parent lineage digest, recipient
output, optional change output, exact transfer amount, and operation id.

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
roots, top-up anchors, and hop count, while their current notes and recursive
proofs are distinct. This lets transport and durable wallet staging treat the
recipient and sender-change outputs as separate spendable states without
reconstructing opaque accumulator or proof bytes.

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

These properties require a V2 recursive accumulator/circuit and chain state
transition. V1 stores one current note and redemption consumes shared top-up
anchors, so constructing two V1 bundles from one 1-to-2 hop cannot provide
independent change: the first redemption invalidates the sibling.

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
the accepted bundle. Duplicate delivery returns those exact bytes; it must not
generate a fresh timestamp or signature.

## Fragmented balances and multi-input payments

The implemented V2 split/append contract has exactly one input note. It does
not recursively merge parents, and readiness therefore advertises
`supports_multi_input: false`. A wallet may display the sum of available notes,
but it must separately expose the maximum single-note spendable amount and must
not claim that the full displayed balance is spendable in one payment.

The recommended bounded follow-up is a typed aggregate over one or two
independent per-input split results, not a lineage-DAG merge. A receiver-created
aggregate request must bind one total exact amount and one or two recipient
output commitments. The sender proves each input independently, and a typed
payment aggregate binds the request digest, common operation id, ordered
recipient bundles, and the checked sum of their exact atomic values. The
receiver verifies and persists all recipient bundles atomically and signs one
aggregate acknowledgement; sender input reservations commit atomically only
after that acknowledgement. Replay/lost-ACK handling is at the aggregate
operation id. This aggregate wire/circuit orchestration is not yet implemented,
so `fragmented_balance_spendable` and `supports_multi_input` must remain false.

## Availability and rollout

`KAGEMUSHA_RECURSIVE_SPEND_V2_PROOF_BACKEND_AVAILABLE` is currently `false`.
The data model and Swift exact-amount API are available for integration and
fixture convergence, but V2 append/verify/redeem entrypoints MUST return
`RecursiveSpendV2ProofBackendUnavailable` before proving or state mutation.
Wallets MUST quarantine funded V1 state and disable fractional/split transfers;
they MUST NOT emulate V2 by cloning V1 bundles or trusting a host-only split.

The reserved native symbol names are
`connect_norito_kagemusha_recursive_spend_init_v2`,
`connect_norito_kagemusha_recursive_spend_topup_v2`,
`connect_norito_kagemusha_recursive_spend_append_v2`,
`connect_norito_kagemusha_recursive_spend_verify_v2`, and
`connect_norito_kagemusha_recursive_spend_redeem_v2`. Each is exported as an
ABI-17 fail-closed stub while the availability constant is false. Append
reserves a `KagemushaRecursiveSpendSplitResultV2` output archive. Standalone V1 lineage-witness verification
reserves `connect_norito_kagemusha_recursive_spend_lineage_witness_verify`;
current ABI 17 verifies that witness only inside redemption, so the Swift typed
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
6. The bounded multi-input aggregate contract above, or an explicitly reduced
   product contract that does not advertise fragmented balances as spendable.

The first-release public HTTP lifecycle is deliberately small:

- `GET /v1/offline/readiness?asset_definition_id=...`
- `POST /v1/offline/top-up`
- `POST /v1/offline/redeem`
- `GET /v1/offline/operations/{operation_id}`

The POST body is the typed `OfflineTopUpRequest` or `OfflineRedeemRequest`
itself. With `Content-Type: application/json`, clients send its structured JSON
representation. With `Content-Type: application/x-norito`, clients send the
canonical typed Norito value directly. There is no object whose purpose is to
wrap the entire request in a `*_norito_base64` field.

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
names; tagged enums use the documented `tag` and `content` fields. Fixed byte
arrays and ordinary byte vectors are arrays of JSON integers from 0 through
255 unless a field explicitly declares another representation. Hashes, keys,
signatures, account identifiers, and numeric values use the textual or numeric
mapping declared by their data-model type. In particular, wide integers are
lossless JSON integers; clients must not round them through an IEEE-754
`double`. An `Option` is emitted as its typed value or `null` unless its field
explicitly omits `None`; decoders also treat an omitted optional field as
`None`. Duplicate declared fields, unknown enum discriminator values,
non-finite numbers, and out-of-range integers are invalid. Decoders ignore
unknown object members. Unit enum content is emitted as explicit `null`, while
decoders also accept an omitted content member as the same unit value.
Signatures and payload digests cover canonical typed Norito bytes, never the
lexical spelling, member order, or ignored members of input JSON.

The `/v1` Norito layouts and their JSON mappings are frozen once this first
release ships. Adding or reordering a field, changing a field mapping, removing
a field, or adding an enum variant to a closed public DTO requires `/v2` unless
the type already defines an explicit extensibility mechanism. Adding a new
route or a new stable error-code string is additive; clients must treat unknown
error codes as non-exhaustive while preserving the HTTP status. Required
request fields cannot be added within `/v1`. Internal consensus/proof type
names may retain implementation version suffixes, but those suffixes never
appear as nested HTTP path versions or response-level negotiation.

Top-up and redemption are asynchronous. Acceptance returns `202 Accepted`, a
typed `OfflineOperationReference`, and a `Location` header pointing to the
operation status resource. Polling returns a tagged `pending`,
`applied { result }`, or `rejected { error }` state; a successful top-up result
contains its typed finalized anchor, while redemption does not carry an
irrelevant nullable anchor. Identical retries use the signed operation id and
idempotency key to resolve to the same operation.

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
Torii derives the submitted transaction's creation time and lifetime from the
signed authorization, so an identical replay produces the same transaction
hash. Pending operations are recovered from the transaction queue and committed
applied or rejected operations from retained blocks. The auxiliary admission
registry is a process-local optimization, is not restart-persistent, and is
pruned opportunistically during reservation after the signed authorization's
expiry plus 24 hours. Committed results remain discoverable while the
corresponding block is retained.

Accepted operation references and pending status responses include
`Retry-After: 1`. Accepted operation references and successful status responses
use `Cache-Control: no-store`. The status resource is subject to the same Torii
access policy as submission and is not projected as an MCP tool.

Readiness is a domain-state read. The selector may be a canonical
asset-definition address literal or a currently live asset alias; the response
always contains the resolved canonical id. When Torii evaluates the requested
asset but offline payments are not ready, it returns `200 OK` with
`ready: false`, typed blockers, the evaluated block height, and
representation-specific `ETag`, the header
`Cache-Control: private, max-age=0, must-revalidate`, and `Vary: Accept`.
`If-None-Match` supports a matching strong or weak validator and `*`;
a match returns `304 Not Modified` without a body. A
`503 readiness_unavailable` error means Torii could not perform the evaluation.
No alternate `/offline/v2`, note-issuer, audit, or wrapper-body HTTP surface is
mounted in this first release.
