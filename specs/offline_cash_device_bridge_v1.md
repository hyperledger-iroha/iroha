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
| 12 | 4 | exact required-feature mask `0x000003ff` |
| 16 | 4 | maximum command payload `65,536` |
| 20 | 4 | maximum response payload `65,536` |
| 24 | 32 | active non-zero `hardware_profile_id` |
| 56 | 32 | active non-zero `OfflineCashHardwareCredentialV1` digest |
| 88 | 8 | zero trailer |

The ten required bits, in order, are:

1. exact-next predecessor consumption;
2. one-use successor authorization;
3. rollback-resistant monetary counters;
4. rollback-resistant journal with sealed inputs and recovery seeds;
5. durable ticket-reserved inbox, exact deduplication, paging, and replay-root
   recovery;
6. pre-reserved authenticated durable retry outbox;
7. atomic candidate-bound commit and terminal-certificate recovery;
8. trusted time or secure monotonic authorization lease;
9. offline hardware-epoch rotation and rollback-safe counter rollover; and
10. no software fallback.

Missing or unknown bits fail closed. The IDs identify the enrolled result but
are not self-authenticating. Online enrollment verifies raw platform evidence,
resolves the governed `OfflineCashHardwareProfileV1`, and issues the compact
credential.
Core validates that credential, its profile membership and lifecycle status,
and every returned authenticator before granting authority. Active status is
required for enrollment, ticket issuance, and new offline commits; a status
change cannot block verification, staging, acknowledgement, or governed online
recovery/redemption of a terminally committed credit.

This ten-bit bridge mask groups related transport capabilities. It is not the
profile's circuit-visible capability mask: enrollment expands and verifies the
complete sixteen-bit `OfflineCashHardwareProfileV1` mask before issuing a
credential. Neither representation permits a missing or unknown capability.

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
| 2 | reserve exact inbox bytes and issue one `OfflineCashAcceptanceTicketV1` |
| 3 | recover an existing ticket and its reservation by exact ticket ID |
| 4 | consume a ticket reservation and stage one authenticated final payment |
| 5 | recover the byte-identical staged payment and durable inbox receipt |
| 6 | recover a bounded page of durable inbox credit IDs and digests |
| 7 | reserve all terminal bytes and prepare one exact-next state transition |
| 8 | recover the sealed prepared transition by exact operation ID |
| 9 | abandon a prepared transition that has no terminal commit |
| 10 | atomically commit one verified precommit-candidate envelope digest |
| 11 | recover the terminal hardware commit certificate and candidate binding |
| 12 | install the verified final commit-wrapper artifact in its reservation |
| 13 | recover/expose the byte-identical installed outgoing envelope |
| 14 | sign an acknowledgement for an already committed inbox receipt |
| 15 | release an outbox entry after matching ACK or finalized online submission |
| 16 | read trusted time or obtain/inspect the active monotonic lease |

Operation 2 accepts the exact signed `PaymentRequestV1` and a proof-bearing
`OfflineCashAcceptanceIntentAuthorizationV1` created before ticket issuance.
Its statement contains the compact intent plus exact release, suite,
verifying-key, and artifact-manifest bindings. Before invoking the reservation
operation, the audited native Core resolves that authenticated release and
cryptographically verifies both proof parities for hidden enabled-profile
membership, qualified sender hardware, sufficient private balance, and a
one-use predecessor authorization bound to the exact request and amount. The
service must not persist the compact intent, consume request budget, or reserve
capacity when that proof is missing or invalid; a canonical
`OfflineCashAcceptanceIntentV1` alone has no authority.

After verification, one atomic service operation records the exact intent
digest and decision, applies the request's private mode ledger, allocates
`reserved_inbox_bytes`, creates the recipient X25519 one-time key, and returns
an exact-amount ticket signed over the intent digest. `SingleExact` consumes its sole slot;
`PartialUntilTotal` checked-adds issued amount; `BoundedMultiPayment`
increments issued count; and `OpenReceive` has no cumulative count or amount
limit. Every mode prevents duplicate or conflicting intent issuance. Resolved
`OpenReceive` decisions may move into an authenticated compact accumulator, but
exact duplicate/conflict answers survive compaction and no historical count
limit is introduced. Neither request budget
nor inbox capacity is reclaimed on expiry; governed relocation preserves the
ledger decision and an equivalent durable delivery slot. A separate
authenticated no-commit closure may release an unresolved ticket only after it
proves that the exact authorized predecessor and sender intent never reached
terminal hardware commit;
consumed tickets remain counted. Operation 4 uses that
allocation, accepts a valid final wrapper regardless of later traffic or
delivery time, and is idempotent only for the same canonical bytes.

Operation 7 accepts a closed transition tag (`Bootstrap`, `MintFold`,
`SendSplit`, `ReceiveFoldBatch`, `RedeemSplit`, `Rotate`, or `SuiteUpgrade`). A
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

Every peer or mint encrypted credit is the canonical
`OfflineCashEncryptedCreditEnvelopeV1` defined in
[`offline_cash_v1.md`](offline_cash_v1.md). The recipient key is X25519; the
audited native/hardware boundary rejects low-order keys and a zero shared
secret, derives its key with the specified HKDF-SHA256 salt/info, and uses
XChaCha20-Poly1305 with the exact canonical AAD. Ephemeral secrets and 24-byte
nonces are fresh per envelope. Operation-specific proof relations constrain the
pre-ID commitments and exact public projection; the `k = 16` circuit does not
reimplement the KEM or AEAD.

Operation 10 is permitted only after Core has durably persisted the exact
candidate and cryptographically verified both parities under an immutable
`OfflineCashAuthenticatedReleaseV1`. Core can construct this command only from
its internal authenticated-candidate typestate; that capability is not
serialized into the frame and cannot be replaced by caller-supplied proof or
release metadata or a successful `validate_shape*` result. Repeating it with
the same operation and
`candidate_envelope_digest` recovers the same
`OfflineCashCommitCertificateV1`; another digest conflicts. Operation 9 is
forbidden once operation 10 has any terminal outcome. Operations 11--13 must
succeed from the reserved record after terminal commit and reproduce the same
`OfflineCashCommitWrapperProofV1` and final bytes. They cannot report capacity
exhaustion, select another successor, or change proof randomness. Only
operation 13 makes an outgoing envelope transport-visible.

At operation 10 the service first seals a self-free
`OfflineCashHardwareTerminalBodyV1` containing the candidate and lifecycle
digests, transition nullifier, reservation commitment, opaque commit evidence,
profile/policy, and private successor/journal/recovery commitments. It contains
no certificate, wrapper, or final-envelope identity. The service then commits
that body and derives the certificate ID, fixing the order terminal body →
terminal commitment → certificate ID → wrapper → envelope without a hash
cycle.

Operation 14 signs an opaque `OfflineCashInboxReceiptV1` only after the exact
payment is durable. Its public fields are the credit ID and receipt commitment;
the receiver lane, hardware epoch, exact-next inbox sequence, persistence time,
payment, and raw receipt are private authenticated inputs. Operation 16 may
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
ticket or recovering a terminally committed operation. After operation 10
succeeds, operations 11--13 may return only `0`, retryable `2`, or
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
