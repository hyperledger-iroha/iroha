# Final promotion native authority V1

This is the implementation contract for the deployment-scoped authority used by
[final promotion receipts](final_promotion_receipt_v1.md). It is one V1 contract.
Obsolete development state must be reseeded; there are no migration or alias paths.

## Owners and scope

Manifest owns `signer::custody_control::{SignerCustodyPolicyV1,
SignerCustodyControlStateV1}`, the shared `configure_signer_custody_policy_v1`
transition, and the existing typed operation intent, custody, reservation, audit
and response commitments. DataModel owns native execution
records and `MutateSorafsFinalPromotionAuthority`. Core owns transactional
enforcement and same-State reads. The daemon owns private receipt durability,
normal signed transaction submission and finalized observation before release.

The stable native scope is the exact deployment identifier for
`FinalPromotionProvenance` (role 14). Both the chain/network and the role/purpose
binding must match native state. The scope does not change with key, policy,
service handle or custody-record rotation. Role 11 remains provider-scoped
StreamToken and role 13 remains ReleaseManifest; neither supplies deployment
promotion authority. The distinct role15 transaction-account custody is specified
in [the account-custody contract](final_promotion_account_custody_v1.md).

`CanManageSorafsFinalPromotionCustody { deployment_id }` authorizes custody
configuration, enrollment and revocation. `CanOperateSorafsFinalPromotion {
deployment_id }` authorizes reservation, completion and expiry.
`CanCheckSorafsFinalPromotion { deployment_id }` authorizes a separately pinned
observer to submit no-write eligibility checks. The registered observer must differ
from the required `expected_operator`, which must remain registered with the exact
deployment Operate permission. Direct and assigned-role grants are checked for both
accounts. Check rights grant no custody-management or operation authority.
Native execution checks permission independently of an installed Executor,
including during genesis. Genesis may bootstrap permission grants, but does not
bypass mutation authorization. A provider registration, alias, domain or asset is
not a substitute for this deployment permission.

## Mutation contract

The only instruction wire identity is
`iroha.instruction.v1::sorafs::MutateSorafsFinalPromotionAuthority`. Its fields are
`deployment_id`, `expected_control_revision`, `expected_control_digest` and
`action`. All seven actions compare the exact current custody predecessor; only
first configuration uses revision zero and a zero digest. JSON action tags are
`configure`, `enroll`, `revoke`, `reserve`, `complete`, `expire` and `check`. Policy and
enrollment payloads are bounded complete canonical Manifest frames. Operation
payloads reuse Manifest's typed commitments, not a parallel signing schema.

| Action | Required transition |
| --- | --- |
| Configure | Validate independently governed signer/attester identities and policy. Enforce monotonic generations and retired-key non-reuse. Clear active eligibility while retaining enrollment lineage, operation tombstones, audit head and fence. |
| Enroll | Verify the complete independent attestation against the exact already committed predecessor. Reject a pending same-block control transition as its approval anchor. Preserve the signed record for later validity checks. |
| Revoke | Strictly set signer and/or attester revocation flags. Require no signature or cooperation from the revoked device. |
| Reserve | Require eligible enrolled custody, a `Sign` intent, exact audit predecessor, unused operation ID and no exclusive pending slot. Derive a nonzero reservation ID and next monotonic fence from native state. Persist the slot and replay identity before success. |
| Complete | Compare original intent, custody, reservation, fence, original account and audit predecessor. Execute in a later block than reservation and strictly before expiry, advance audit by exactly one and retain immutable response/signatures commitments. |
| Expire | Compare the exact original operation and slot, require execution at or after expiry, terminalize without completion or audit advancement, and preserve replay rejection. |
| Check | Evaluate current custody and an exact request/phase under a fresh challenge and committed minimum block. Change no custody, history, operation ID, fence or audit state. |

Every genuine custody change atomically invalidates any outstanding reservation.
It records an `Invalidated` terminal outcome instead of a forged completion.
Governed emergency revocation does not claim the generic daemon's separate
old-key-audited terminal-transition contract.

An exact existing reservation or completion may be observed for an ambiguous
submission under unchanged control. Such observation never authorizes another
signer call. A reused ID with different coordinates, a terminal failure, or
changed custody is rejected. Stale control mutations fail their current CAS;
callers can inspect retained immutable history to reconcile an ambiguous result.

`Check` carries a 256-bit challenge, network ID, minimum height/hash, required
`expected_operator: AccountId`, the canonical Manifest
`SignerFinalPromotionRequestV1` and one typed subject. The observer signs the exact
Check payload. Every row subject retains the original reservation and execution
operator; changing the observer never transfers operation ownership. `Current` pins the
current audit head. `BeforeProvider`, `AfterProvider` and `BeforeCommit` pin the
complete original Reserved row and require the live exclusive slot before expiry.
`AfterCommit` and `BeforeRelease` pin the complete retained Completed row; newer
operations and audit heads do not relabel that old completion. Every phase checks
current custody again. Every phase rejects custody time before the retained native
control execution, even when the attestation was issued earlier. Runtime operation
checks additionally reject time before the original operation execution. The floor must be an exact committed ancestor before Check
execution. The instruction retains the 32-KiB canonical bound even though Check
allocates no mutation digest or execution record. Re-execution always evaluates
eligibility; it does not enter the Complete retry fast path.

## Native histories and bounds

Custody records and operation records have separate domain-separated digests.
Reservation, audit and completion progress never changes the custody digest.
Both histories store actual execution height, zero-based per-history block
ordinal, deterministic block timestamp and submitting account. No mutation
accepts caller-selected execution provenance or its own current block hash.
Finalized readers attach the actual block hash after execution.

Operation records retain the original intent, custody, reservation and reservation
execution/account through `Reserved`, `Completed`, `Expired` and `Invalidated`.
The completed outcome carries audit/response commitments and the digest of the
ordered signatures. It does not contain the unreleased signatures themselves.
Immutable revision records and bounded indexed lookups preserve history; the
latest operation-ID index never permits an ID to become fresh again. Every
selected active reservation must name the selected custody digest and enrollment,
with neither authority revoked. Independent canonical history prefixes cannot
be paired to restore an old live reservation under newer custody; legitimate
historical observations remain readable at their original block.

The native limits are 8,192 normal custody revisions, 8,194 total custody
revisions, 65,536 permanently retained operation IDs and 32 KiB per complete
native record. Admission must reserve capacity for the eventual terminal
operation record. Emergency revocation keeps its separate custody capacity.
Counter overflow and capacity exhaustion fail closed; deletion or rotation does
not reset replay protection.

Reservations last at most 60,000 milliseconds from native execution time and
are capped by signed custody expiry and independent attester eligibility.
Eligibility uses inclusive starts and exclusive ends. Completion must execute
in a strictly later block than reservation and strictly before reservation expiry. A timely completed result may become
finalized and be observed after that expiry, while its current custody must
still be fresh, eligible and unchanged. Expiry after completion never permits
reserving or signing the request again.

## Finality and private receipt boundary

The production observer must capture custody, audit and operation state from the
same native State view, compare its committed block hash with durable Kura state,
authenticate retained finality artifacts, and require their genesis-derived network
identity to equal the same State network. A caller-supplied block hash, local
receipt or self-signed observation cannot establish native finality. Never keep a
State view open while awaiting consensus.

The source verifies a purpose-bound, durably staged private receipt before it
submits completion commitments. Core authenticates execution and the committed
coordinates; it cannot attest to a daemon's filesystem durability. Submitting
the receipt's signatures in the transaction would release them before completion
finality and is excluded from this schema. Before returning any signature, the
daemon must verify exact finalized completion and freshly observe current custody.

## Current-authority prerequisite

Historical finality is insufficient for signing or release. V2 validation in
[`block.rs`](../../crates/iroha_core/src/block.rs) deliberately ignores local wall
clocks and requires `max(parent time + cadence, transaction creation times + 1 ms)`.
[`tx.rs`](../../crates/iroha_core/src/tx.rs) bounds ordinary ingress time against
NTS, but consensus revalidation uses block time (or a retained QueuePlan admission
time). The ingress bound is not a wall-clock guarantee from the finalizing quorum.
A formerly future-dated QC may pass a header-age check much later, including after
fresh startup. Header age can be a conservative admission limit, never proof of
current revocation state; changing observation time cannot refresh that authority.

The current implementation adds a bounded purpose-native `Check` ordered through the
same ordinary consensus transaction path as revocation. It binds a fresh
unpredictable one-use challenge, exact phase/subject and custody expectations,
network and an independently retained minimum height/hash. Authenticate committee
continuity from that floor; a candidate-selected historical roster is insufficient.
A challenged node-tip signature cannot replace this executed check.

The consumer must retain the exact signed entrypoint and authenticate its
successful aligned execution result through the existing
[`TrustedBlockProofAnchor` and `BlockProofs`](../../crates/iroha_data_model/src/block/proofs.rs)
owners. Their executed-wire commitment matters: a block-header hash alone does
not authenticate execution results. Authenticate Check at height H, then join it
to the same State's current applied cut J >= H and durable Kura/network evidence.
Verify committee continuity from the independent floor through H to J. Recheck
custody/audit/operation state, the distinct observer's current Check permission and
the expected operator's current Operate permission at J, including direct and role
grants and account registration; permissions have no historical-H reader. Never combine historical native
rows at H with account permissions at J. A
per-history transition ordinal is not a transaction index. Rejection, incomplete
application or revocation later in the same block must fail eligibility.

The check need not add an authority database or consume signing-operation IDs;
its body/result are retained in canonical executed block history. Missing proof
material fails closed. Historical Reserve/Complete retries are reconciliation,
not new eligibility: an old row or idempotent success cannot refresh an observation.
The separate transaction account key must not recurse through the protected
application key. Retire each challenge on every outcome, preserve independent
floors across restart, and bound each phase using monotonic runtime time. Measure
the full phase latency against reservation and custody limits without caching
another phase's proof. A linearizable read does not cancel a key call after a
concurrent revocation; subsequent checks must fence output release.

The Core account and receipt wrappers reuse one crate-private closed-purpose Check
proof owner. It retains a move-only challenge, exact signed External bytes, the
original bounded monotonic deadline and actual `Arc<State>`, authenticating aligned
successful execution and canonical committee successors before each wrapper samples
its purpose-owned finite UTC interval and rechecks the current applied cut.
Both wrappers require a single Ed25519 observer and reject another account-key
algorithm before issuing the round or performing signing, State or proof I/O.
A verified result preserves `original_floor()` with its exact height, hash and
`HeightContextId`, separately from the authenticated descendant `applied_floor()`.
It also retains `canonical_external()`, `check_height()` and `check_block_hash()`
for the exact successful Check at H. The later cut J does not replace those
execution coordinates. These historical getters neither renew eligibility nor
qualify how the original floor was provisioned.
The runtime interval requires `0 < earliest_unix_ms <= latest_unix_ms < u64::MAX`.
Both endpoints must satisfy custody and phase eligibility against the same native
snapshot; the consumer walks history once. Initial validation uses the earliest
endpoint as the observation time for both endpoints, so interval width consumes
`max_anchor_age_ms`. Each verified wrapper's `recheck_use_interval` retains that
original earliest time, snapshot and monotonic deadline for both new endpoints.
It rejects intervals beginning before the original earliest time or exceeding the
original age bound; it neither samples a clock nor observes later revocations.
Every new phase still requires a fresh Check. Reserved phases require the upper
endpoint strictly before expiry; completed phases retain their original execution
time and may remain eligible after reservation expiry. No scalar-clock overload or
wire-serializable clock qualification is provided. Its floor/context, account,
reviewed request and clock are explicit independent trust inputs. The production
factory must qualify those inputs and enforce immediate use for the exact phase.
After persisting the authenticated floor, preserve the original earliest observation
bound and resample a qualified eligibility interval; storage delays must not renew or extend a
reservation or custody window. Reject backwards/unhealthy time. The floor requires
independent rollback authority across restore; local file durability alone does
not provide it. A production account signer must authorize the exact unsigned
network/deployment/action payload before key I/O and must use a separate key from
the role-14 receipt provider. Role15 signs only Reserve and Complete. Both native
Check kinds use the separately pinned observer and its finite approved fee budget.
The production factory must reject an observer key equal to either protected key.
Recovery and release require fresh observer-signed receipt Checks, current role14
custody and the original operator's current Operate permission. They require no
role14 or role15 key operation, Reserve or Complete mutation. Current role15 key
custody is not an additional condition for releasing an already completed receipt.

The 15 native Check and 23 Core consumer tests pass in the
[local native checkpoint](v1_closure_ledger.md#native-deployment-authority-and-schema-checkpoint).
TODO: Qualify their production trust inputs and wire the actual daemon source. Torii role 11 requires its own
purpose-owned authority; this role-14 action cannot supply missing per-token
operation state. Bounded four-validator adversarial models
support this design but do not verify a production protocol. Reuse existing
transaction ordering and finality; no new quorum-tip message protocol is selected.
Keep historical anchors under `query::signer_finality` and deterministic consensus
time unchanged. Qualify replay, wrong network/floor/committee, rejected or mismatched
entry/result, unapplied decisions, partition, timeout/restart and concurrent revoke,
including an old future-dated QC entering an apparent age window after startup.

## Distinct account custody

Role15 `FinalPromotionAccountTransaction` has a deployment purpose and its own native
control history, permanent generation indexes and Configure/Enroll/Revoke/Current
Check contract. It reuses canonical Manifest custody frames and one native history
engine; it does not reuse role14 reservation/operation/receipt authority. The Current
Check binds a fresh nonce, independent network/floor, exact control CAS, derived
Ed25519 target and independently selected complete-payload digest. Observer must
differ from target and retain Check permission; target must exist and retain Operate
permission at the authenticated current cut. Both UTC endpoints must satisfy custody
and be no earlier than native enrollment execution. The shared proof owner verifies
only exact Check execution and finality; each wrapper owns its current predicate.

Core's `validate_final_promotion_account_transaction_envelope_v1` preflight checks
ordinary single-Ed25519 payload structure and the complete signed External size;
its private sizing signature and envelope never escape. Both native purposes use
`FINAL_PROMOTION_NATIVE_TRANSACTION_MAX_BYTES_V1`, the sole 64-KiB bound.
`final_promotion_native_signed_entry_frame_v1` checks the actual External entry,
rejects attachments and multisig authorization sidecars, verifies its signature,
and returns the exact canonical frame. Check binding uses this same owner.
Retained-byte consumers must bounded-decode and compare the returned frame with
the original bytes. These checks approve no fees, action, route, custody or
currentness; callers must still compare the full independently reviewed payload.

The daemon's `signer_operation::final_promotion::account_transaction` owner now
derives Reserve only from an executed Current receipt Check and Complete only
from an executed BeforeCommit Check plus the actual pinned four-signature receipt.
It retains the entire current transaction payload, compares an independently
approved fee intent, and computes the account contract's SHA-256 commitment.
The continuation retains the original receipt Check and its round through account
approval and key I/O. It generates an unpredictable account challenge after key
I/O, followed by a fresh receipt-phase challenge; only the exact executed results
can release the signed transaction. Complete's private receipt and all original
observations remain owned through submission and read-only reconciliation.
No replacement challenge, recomputed payload or new deadline substitutes for them.

The adjacent `observer_transaction` owner now signs only the exact original
prepared receipt/account Check. It pins distinct protected/observer identities
and the independently approved observer fee intent, bounds the entire envelope,
verifies the returned observer signature, and checks the original lifetime before
and after key use. Its result is the original Core pending Check, not a verified
observation. The prepared account workflow constructs this owner from its retained
bindings; the same native tests use it for both observation phases.

The remaining configured assembly must connect that exact-Check request to a
purpose-restricted observer signing service and ordinary Queue submission. Before
signing, durable spending approval must bind this full fee intent and operation;
ambiguous submission must reconcile the exact signed bytes rather than generate a
replacement payload. Core verification then consumes the original pending Check,
and the source must persist its authenticated descendant floor through independent
rollback authority and resample qualified UTC before use. Restart retires in-flight
challenges; it does not reconstruct monotonic deadlines from serialized claims.
Role15 Reserve/Complete and role14 receipt signing need their separate configured
service handlers and actual native state-source assembly. The generic service's
four unsupported purposes remain rejected until those contracts are connected.

TODO: Qualify the added native/adversarial tests and wire the configured authenticated
software provider, independent observer, approved spending journal, UTC/floor and
configuration/submission source. Optional hardware uses the same contract; HSM,
non-exportability and hardware-origin evidence are not prerequisites.
A supplied digest or decoded record does not establish reviewed payload, signer
custody, qualified time or rollback resistance. The local results remain scoped in
the [account checkpoint](v1_closure_ledger.md#native-account-custody-and-shared-check-checkpoint).

## Qualification

From the repository root, `bash ci/check_sorafs_native_authority_runtime.sh` builds
one Core/Torii/daemon/SCCP test graph, requires each critical test to be collected
exactly once, and executes the same selection including ignored cases. The release
gate invokes it unconditionally inside its Cargo.lock guard. This focused command
does not replace full workspace, SDK or deployed qualification.

TODO: Complete full native/deployed qualification, the actual daemon submission and
finalized-state adapter, configured software signer, independent observer and
real four-validator integration. Native unit fixtures do not qualify signer
custody or a deployed authority. The complete remaining scope stays in the
[implementation goals](v1_implementation_goals.md) and
[closure ledger](v1_closure_ledger.md).
