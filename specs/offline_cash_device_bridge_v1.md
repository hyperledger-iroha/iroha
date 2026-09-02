# Offline Cash V1 secure-device bridge

This note fixes the optional ABI between the audited Offline Cash native core
and a platform service that owns rollback-resistant wallet state. It does not
make an ordinary Android KeyMint, StrongBox, Secure Enclave, or App Attest key
offline-capable. When the service is absent, its profile or credential cannot
be authenticated, or any required capability is missing, the SDK is
online-only. There is no software fallback.

All integers are unsigned little-endian. Digests are SHA-256. Reserved bytes
must be zero. Every frame version is exactly `1`; every other value is rejected.
Command payloads are bounded canonical Norito values owned by Core. The service
must decode the exact operation-specific V1 type and reject an outer operation
code that does not match its canonical body.

Frame and storage bounds are per operation, not cumulative monetary limits.
The bridge may page or compact authenticated state, but it must not impose a
protocol limit based on hops, ancestry, origins, receipts, fan-in, proof depth,
provenance, or prior transitions. Capacity failure is legal only before a new
ticket or transition reservation; it cannot reject already committed money.
Every asset-scoped command carries the exact `AxtAssetIncarnationV1` token.
Online enrollment and top-up admission match it to authoritative
registered-asset state and the one `(network, asset, exact incarnation)`
liability pool. Offline bridge operations bind that token byte-for-byte; a
caller-supplied ordinal or bare integer is never incarnation authority.

## Capability frame

The capability frame is exactly 96 bytes.

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `IOCFJCP1` |
| 8 | 2 | version `1` |
| 10 | 1 | platform: Android `1`, iOS `2` |
| 11 | 1 | zero flags |
| 12 | 4 | exact required-feature mask `0x0000ffff` |
| 16 | 4 | maximum command payload `65,536` |
| 20 | 4 | maximum response payload `65,536` |
| 24 | 32 | active non-zero `hardware_profile_id` |
| 56 | 32 | active non-zero `OfflineCashHardwareCredentialV1` digest |
| 88 | 8 | zero trailer |

The sixteen required bits, in order, are:

1. exact-next predecessor consumption;
2. one-use successor authorization;
3. rollback-resistant counter and journal;
4. sealed transition inputs and deterministic recovery seeds;
5. one-use acceptance-ticket issuance;
6. durable inbox byte reservation;
7. authenticated inbound staging, exact deduplication, and paging;
8. authoritative replay-root and external sparse-tree recovery;
9. sender outbox byte reservation before predecessor lock;
10. authenticated durable retry outbox;
11. atomic commit bound to a Core-verified candidate digest;
12. recoverable terminal commit certificate;
13. trusted time or a secure monotonic authorization lease;
14. offline hardware-epoch rotation;
15. rollback-safe counter rollover; and
16. no software fallback.

Missing or unknown bits fail closed. The IDs identify the enrolled result but
are not self-authenticating. Online enrollment verifies raw platform evidence,
resolves the governed `OfflineCashHardwareProfileV1`, and issues the compact
credential.
Core validates that credential, its profile membership and lifecycle status,
and every returned authenticator before granting authority. Active status is
required for enrollment, ticket issuance, and new offline commits; a status
change cannot block verification, staging, acknowledgement, or governed online
recovery/redemption of a terminally committed credit.

The bridge mask is the `u32` framing of the profile's exact circuit-visible
lower-sixteen-bit capability mask. Enrollment and every bridge discovery must
therefore require `0x0000ffff`; a missing lower bit or any non-zero upper bit
fails closed.

## Command frame

The command frame has an 80-byte header followed by 1 to 65,536 payload bytes.

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `IOCFJCM1` |
| 8 | 2 | version `1` |
| 10 | 1 | operation code |
| 11 | 1 | zero flags |
| 12 | 32 | non-zero idempotency/request ID |
| 44 | 4 | payload length |
| 48 | 32 | SHA-256 of the payload |
| 80 | variable | canonical operation payload |

The operation codes are closed:

| Code | Operation |
| ---: | --- |
| 1 | read the active compact hardware credential and profile reference |
| 2 | prepare one proof-bearing `OfflineCashAcceptanceIntentAuthorizationV1` |
| 3 | recover that byte-identical prepared acceptance authorization |
| 4 | verify the authorization, reserve exact inbox bytes, and issue one `OfflineCashAcceptanceTicketV1` |
| 5 | recover an existing ticket and its reservation by exact ticket ID |
| 6 | consume a ticket reservation and stage one authenticated final payment |
| 7 | recover the byte-identical staged payment and durable inbox receipt |
| 8 | recover a bounded page of durable inbox credit IDs and digests |
| 9 | reserve all terminal bytes and prepare one exact-next state transition |
| 10 | recover the sealed prepared transition by exact operation ID |
| 11 | abandon a prepared transition that has no terminal commit |
| 12 | atomically commit one verified precommit-candidate envelope digest |
| 13 | recover the terminal hardware commit certificate and candidate binding |
| 14 | install the verified final commit-wrapper artifact in its reservation |
| 15 | recover/expose the byte-identical installed outgoing envelope or state proof |
| 16 | sign an acknowledgement for an already committed inbox receipt |
| 17 | release an outbox entry after matching ACK or finalized online submission |
| 18 | read trusted time or obtain/inspect the active monotonic lease |
| 19 | prepare one proof-bearing `OfflineCashMintAuthorizationV1` before reserve debit |
| 20 | recover that byte-identical prepared mint authorization |
| 21 | verify the authorization and stage its matching finalized mint credit |
| 22 | fold one staged credit into the aggregate balance |
| 23 | read the stable pending-credit high-water mark |
| 24 | rotate aggregate state into the next qualified hardware epoch |

Operations 2 and 3 prepare and recover the proof-bearing
`OfflineCashAcceptanceIntentAuthorizationV1` for one exact signed
`PaymentRequestV1` and amount before ticket issuance. Operation 4 accepts the
exact request and that authorization.
Its statement contains the compact intent plus exact release, suite,
verifying-key, and artifact-manifest bindings. Before invoking the reservation
operation 4, the audited native Core resolves that authenticated release and
cryptographically verifies both proof parities for hidden enabled-profile
membership, qualified sender hardware, sufficient private balance, and a
one-use predecessor authorization bound to the exact request and amount. The
service must not persist the compact intent or reserve physical inbox capacity
when that proof is missing or invalid; a canonical
`OfflineCashAcceptanceIntentV1` alone has no authority.

After verification, one atomic service operation records the exact intent
digest and decision in the replay ledger, allocates
`reserved_inbox_bytes`, creates the recipient X25519 one-time key, and returns
an exact-amount ticket signed over the intent digest. Every distinct valid
intent against the same request is independently acceptable and receives its
own one-use ticket; only exact intent replay or conflicting reuse fails. Resolved
decisions may move into an authenticated compact accumulator, but exact
duplicate/conflict answers survive compaction and history never becomes an
admission condition. Inbox capacity is not reclaimed on expiry; governed
relocation preserves the ledger decision and an equivalent durable delivery
slot. A separate
authenticated no-commit closure may release an unresolved ticket only after it
proves that the exact authorized predecessor and sender intent never reached
terminal hardware commit;
consumed ticket decisions remain authenticated for replay detection. Operation 6 uses that
allocation, accepts a valid final wrapper regardless of later traffic or
delivery time, and is idempotent only for the same canonical bytes.

Operation 9 accepts a closed transition tag (`Bootstrap`, `MintFold`,
`SendSplit`, `ReceiveFold`, `RedeemSplit`, `Rotate`, or `SuiteUpgrade`). A
batch payload names 1--16 already-staged credit IDs; it never embeds sixteen
payment envelopes. For an outgoing transition, the reserved byte count covers
the sealed record, precommit candidate, terminal certificate, final wrapper,
maximum canonical envelope, and retry metadata before the predecessor is
locked.

For `MintFold`, the authenticated online top-up carries the complete compact
hardware credential and `OfflineCashMintAuthorizationV1`; the private
transition input carries the openings of its per-mint credential and credit
commitments. Core verifies both authorization parities through the exact active
release/profile/credential before payer debit or pooled-reserve mutation. The
public mint statement carries only the complete lifecycle, randomized
credential commitment, authorization context/complete-authorization digests,
amount, issuance and credit commitments, recipient, and committed-ledger time.
The finalized mint helper recursively verifies the same authorization digest.
Stable credential, lane, epoch, and device-key identities must not escape
through the bridge's public result.

Operations 19 and 20 give that pre-debit mint authorization the same sealed,
byte-identical recovery property as the acceptance authorization. Operation 21
verifies it before staging the matching finalized mint credit. Operations 22
and 23 expose the fixed 1--16 folding and stable-snapshot watermark required by
the aggregate receiver; neither introduces a cumulative inbox or receipt
limit. Operations 22 and 24 must preserve the exact-next preparation,
candidate-bound commit, and recovery invariants established for operations
9--15; neither is an alternate commit path. Operation 24 performs only a
qualified exact-next hardware-epoch and counter rollover and must preserve the
aggregate balance and replay root.

Every peer or mint encrypted credit is the canonical
`OfflineCashEncryptedCreditEnvelopeV1` defined in
[`offline_cash_v1.md`](offline_cash_v1.md). The recipient key is X25519; the
audited native/hardware boundary rejects low-order keys and a zero shared
secret, derives its key with the specified HKDF-SHA256 salt/info, and uses
XChaCha20-Poly1305 with the exact canonical AAD. Ephemeral secrets and 24-byte
nonces are fresh per envelope. Operation-specific proof relations constrain the
pre-ID commitments and exact public projection; the `k = 16` circuit does not
reimplement the KEM or AEAD.

Operation 12 is permitted only after Core has durably persisted the exact
candidate and cryptographically verified both parities under an immutable
`OfflineCashAuthenticatedReleaseV1`. Core can construct this command only from
its internal authenticated-candidate typestate; that capability is not
serialized into the frame and cannot be replaced by caller-supplied proof or
release metadata or a successful `validate_shape*` result. Repeating it with
the same operation and
`candidate_envelope_digest` recovers the same
`OfflineCashCommitCertificateV1`; another digest conflicts. Operation 11 is
forbidden once operation 12 has any terminal outcome. Operations 13--15 must
succeed from the reserved record after terminal commit and reproduce the same
`OfflineCashCommitWrapperProofV1` and final bytes. They cannot report capacity
exhaustion, select another successor, or change proof randomness. Only
operation 15 makes an outgoing envelope transport-visible.

At operation 12 the service first seals a self-free
`OfflineCashHardwareTerminalBodyV1` containing the candidate and lifecycle
digests, transition nullifier, reservation commitment, opaque commit evidence,
profile/policy, and private successor/journal/recovery commitments. It contains
no certificate, wrapper, or final-envelope identity. The service then commits
that body and derives the certificate ID, fixing the order terminal body →
terminal commitment → certificate ID → wrapper → envelope without a hash
cycle.

Operation 16 signs an opaque `OfflineCashInboxReceiptV1` only after the exact
payment is durable. Its public fields are the credit ID and receipt commitment;
the receiver lane, hardware epoch, exact-next inbox sequence, persistence time,
payment, and raw receipt are private authenticated inputs. Operation 18 may
return only the opaque public `time_evidence_commitment` or
`lease_evidence_commitment` used by the terminal proof. The exact checked
deadline, actual commit time, clock authority, lease identity/window, and
authorization counter remain private sealed witnesses.

## Response frame

The response has a 116-byte header, at most 65,536 payload bytes, and at most
8,192 authenticator bytes.

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `IOCFJRS1` |
| 8 | 2 | version `1` |
| 10 | 1 | exact echoed operation |
| 11 | 1 | status |
| 12 | 32 | exact echoed request ID |
| 44 | 4 | payload length |
| 48 | 4 | authenticator length |
| 52 | 32 | SHA-256 of the payload |
| 84 | 32 | SHA-256 of the authenticator |
| 116 | variable | payload, then authenticator |

Statuses are `0` success, `1` unavailable/capacity-before-reservation,
`2` retryable/concurrent, `3` binding mismatch, `4` trusted-time/lease rejection,
`5` policy rejection, `6` missing, `7` conflict, `8` corrupt, and `9` malformed
request; `10` means governed recovery is required while the committed record
remains authoritative. Success requires a non-empty payload and non-zero
authenticator; failure carries neither. Status `1` is allowed for new ticket
issuance and new transition preparation, never for staging against a valid
ticket or recovering a terminally committed operation. After operation 12
succeeds, operations 13--15 may return only `0`, retryable `2`, or
recovery-required `10`; callers retry the identical command or enter governed
recovery. Every other status is an implementation breach, not a legal rejection
of the committed value.

The authenticator is profile-specific. The trusted native adapter verifies it
before returning success, and Core independently verifies the authenticated
release, exact credential/profile, reservation, candidate, certificate, state,
and wrapper proof before advancing a non-forgeable internal typestate. Canonical
decode and shape validation alone are never monetary authority. Swift and
Kotlin wipe temporary framed buffers after each call; callers remain
responsible for the original canonical secret payload.

## Platform entry points

Swift discovers two optional C symbols in the already authenticated
`NoritoBridge` image:

```c
int32_t connect_norito_offline_cash_device_capabilities_v1(
    uint8_t *output,
    size_t output_capacity
);

int32_t connect_norito_offline_cash_device_execute_v1(
    const uint8_t *command,
    size_t command_length,
    uint8_t *output,
    size_t output_capacity,
    size_t *output_length
);
```

Android exposes equivalent optional JNI methods on the Kotlin bridge. A
reviewed build may bind them with `RegisterNatives` or the generated names.
Java delegates to Kotlin so Android has one codec and one production decision.
A missing symbol, linkage error, malformed frame, platform mismatch, wrong
profile or credential, partial feature mask, or non-zero native status fails
closed during capability discovery; none triggers a TEE or software downgrade.
An authenticated operation status such as pre-reservation capacity exhaustion
fails only that operation and does not change an otherwise valid discovery.

The generic `connect_norito_bridge` entry points publish this closed numeric
inventory but intentionally provide no secure-device implementation: both
stock C and JNI execution paths remain unavailable. Exact per-operation
canonical command/result bodies and authenticators must be implemented and
qualified by the OEM/secure-element provider; the stock bridge must not
simulate them in software.

The stock-platform build intentionally exposes no qualifying service. Closing
this gate requires an audited OEM/secure-element implementation and physical
evidence for reservation exhaustion, exact-next concurrency, every
prepare/prove/commit/recovery crash edge, rollback and backup/restore attacks,
clock/lease failure, epoch and counter rollover, and byte-identical recovery in
airplane mode.

The authenticated release must also contain exact measurements and artifacts
for the release-wide `AcceptanceIntentAuthorization` relation and the
`MintAuthorization` helper in addition to all aggregate-state, mint,
credential, GuardBundle, and commit-wrapper circuits. A bridge cannot infer
those authorities from a generic state or wrapper measurement.

All SDKs must use the same versioned audited native core for hashing,
encryption, proof creation and verification, release/profile/credential
admission, certificate verification, and canonical fixture generation. SDK
layers own only framing, codecs, storage, transport, and orchestration; an
independent Swift, Kotlin, Java, JavaScript, Python, or C# cryptographic path is
not a qualifying bridge. The stock bridge and current evidence tooling do not
claim that real production circuits or this physical-device gate have passed.
