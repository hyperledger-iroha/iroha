# Offline sender recovery device contract

`connect_norito_bridge/src/kagemusha_device_bridge_v1/sender_payload.rs` defines
canonical public bodies for ABI-23 sender operations 9–15 and 17. Stock C/JNI
validates these bodies and returns unavailable. The contract does not implement
a monetary service, authenticate hardware, persist an operation index, or grant
Core proof authority. The source's native integration TODO remains open.

## Operation identity and authority

Before its first native call, the caller generates and durably retains an
independent, nonzero 32-byte operation ID. It must not derive that ID from the
amount or receiver request: the same reusable request can legitimately be paid
more than once. Every single-operation body repeats this ID and must match the
outer command's request ID and operation code exactly. A page has an independent
query ID; each returned record retains its original operation ID.

The public-input digest is SHA-256 of the ASCII domain
`iroha:kagemusha:device:v1:sender-public-inputs`, a zero byte, the canonical
preimage byte count as `u64` little-endian, and the canonical Norito preimage.
The preimage binds version 1, operation ID, complete creation wallet context,
and either exact receiver request/intent/ticket bytes or the positive `u128`
redemption amount and canonical beneficiary. The native service must atomically
retain this digest, operation ID, exact Core preparation/reservation/output
identities, private recovery material and phase before reporting success.
Reusing an ID with another context or input digest is a conflict. Retrying an
existing ID observes or resumes that exact operation and never starts a new one.

The wallet context includes the stable network/device lane/asset/scale,
protocol/suite/key-set/release/asset incarnation, hardware profile/policy epoch,
credential ID, full `u128` hardware generation and epoch ID, and device policy
binding. These are selectors, not caller authority. The qualified native
session authenticates both its current context and retained historical records.

A reply carries the **current** authenticated context. Each record carries its
immutable **creation** context. Ordinary hardware, credential and suite rotation
must not strand installed outboxes: historical recovery is valid only within
the same stable lane/network/asset/scale and asset incarnation, with creation
generation no greater than current generation. Equal generations require the
same epoch ID. A single-operation descriptor must exactly match its record's
creation context and digest. New Prepare commands and page queries use the
current context; historical authority cannot prepare new work. Retained proof
release, credentials, sealed material and native authentication are still
required and cannot be established by these shape comparisons.

## Canonical bodies and observations

All bodies are canonical, resource-bounded Norito archives with distinct schema
names. Commands are at most 16 KiB and replies at most 64 KiB. Integers retain
their full width. Appended bytes, wrong schemas, substituted outer IDs, unknown
operation/body pairings and noncanonical archives reject before dispatch.

| Operation | Public body |
| --- | --- |
| 9 Prepare | Exact public inputs; input digest is derived from the canonical preimage. |
| 10 Recover prepared | Original input digest. |
| 11 Abandon uncommitted | Original input digest and preparation ID. |
| 12 Commit | Original input digest, preparation ID and persisted candidate digest. |
| 13 Recover terminal | Original input digest. |
| 14 Install | Original input digest, preparation ID, candidate digest, public inputs and exact terminal envelope. |
| 15 Recover installed | Single original input digest, or pinned index revision, exclusive operation-ID cursor and page count. |
| 17 Release peer outbox | Original input digest, exact envelope digest, public inputs, envelope and matching acknowledgement. |

Record phases are Prepared, CandidatePersisted, Committed, Installed, Released
and Abandoned. Missing exists only as an authenticated, tombstone-aware native
lookup result; empty bytes or transport errors never become Missing. Phase
observations can skip intermediate phases after lost returns, but they cannot
regress, change immutable selectors, replace previously known digests or reuse a
terminal operation. Any change requires a greater native index revision.
Released and Abandoned tombstones retain immutable replay anchors and discard
public input bytes. Committed work cannot be abandoned. This is an observation
check; operation 11 still requires independent proof that commitment did not
occur and does not release a receiver ticket.

Only operation 15's Installed item carries exact terminal bytes. The returned
bytes must match the retained envelope, candidate, commit-certificate and output
identities and the original exchange context. Released entries expose no bytes
and cannot be resurrected by installation retry. Peer acknowledgement release
binds the exact payment and acknowledgement digests. A peer ACK cannot release
a redemption; an authenticated settlement-receipt contract remains required.

Pages are ordered by operation ID and contain at most four entries per response,
with no lifetime backlog cap. They pin a stable-wallet `u128` index revision;
the revision does not reset at hardware rotation. Noninitial cursors require a
revision. The native service selects the exact bounded prefix atomically, and
the response must match that selection, revision and end marker. Self-consistent
host pages do not prove completeness. Native lookup-selection and monotonic
observation checks also reject omitted operations and disappearing tombstones.

Core binding helpers compare these public selectors against actual retained
`PreparedOutgoingCandidateV1` and `DurableOutgoingEnvelopeV1` objects and borrow
Core's immutable public recovery views. They preserve creation hardware and
lifecycle scope across rotation and bind the exact ciphertext to the lifecycle.
They never serialize private state, replace authenticated restore, verify a
recursive proof, create a candidate capability or approve a hardware mutation.

## Validation scope

The adjacent tests cover canonical framing, exact operation/input bindings,
lost Prepare/Commit/Install returns, immutable terminal digests, tombstones,
full-width revisions, historical installed lookup/page/release, cross-wallet
and epoch-rebinding rejection, exact page/lookup selection and Core public
projection substitution. These are software contract tests, not real monetary
proofs, rebuilt SDK packages, OEM service integration or physical qualification.
