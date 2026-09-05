# KAGEMUSHA V1 secure-device bridge

This document fixes the only V1 ABI between the audited KAGEMUSHA native core
and a qualified non-forking hardware service. A missing service, incomplete
profile, invalid credential, malformed frame, unsupported operation, or failed
binding check fails closed. There is no software fallback.

All integers are unsigned little-endian, digests are SHA-256, reserved bytes
are zero, and every canonical Norito archive has version `1`. Frame and page
bounds protect parsers and transports. They are not limits on payment history,
hops, received credits, aggregate balance ancestry, or recursive proof depth.

## Peer protocol

The only peer exchange is:

1. `KagemushaPaymentRequestV1` (`IPM1` tag `1`)
2. `KagemushaPaymentV1` (`IPM1` tag `2`)
3. `KagemushaAcknowledgementV1` (`IPM1` tag `3`)

Raw and `kgm1:` transport validators expose only those three shapes. A payment
request directly binds the network, release, asset incarnation and scale,
liability pool, amount, recipient account, recipient encryption key, hardware
credential, request ID, issue time, expiry, and receiver signature. It does not
bind the receiver's current aggregate-state head.

A payment directly binds the request digest, amount, sender before/after
commitments, transition nullifier, unique credit ID, ciphertext commitment,
trusted commit evidence, commit time, encrypted credit, terminal commit
certificate, and constant-size paired proof. The sender's hardware commit time
must be inside the request window. A payment committed in that window remains
stageable and foldable indefinitely.

The acknowledgement binds the request digest, payment digest, credit ID,
rollback-resistant inbox receipt, and receiver signature. It is created only
after the exact request/payment bytes and receipt have been irreversibly staged.
An exact duplicate recovers the same durable result; reusing an operation ID or
credit ID with different bytes is a conflict.

Distinct valid payments made against the same request are independently valid.
Invoice deduplication is outside the monetary protocol.

## Native contract vector

Every linked native bridge exports one canonical Norito
`KagemushaNativeContractVectorV1`. Its typed body contains exact counts and
ordered `{code, name}` entries for the three peer messages, 50 proof-artifact
roles, eight qualified relations, six helper circuits, sixteen mandatory
hardware capabilities, and 22 secure-device operations. The helper inventory
ends with `mint_hash_shard` and `mint_hash_claim`; the relation inventory is the
six monetary operations followed by `terminal_authorization` and
`commit_wrapper`.

The body digest transcript is:

```text
ASCII "iroha:kagemusha:native-contract-vector:v1"
|| canonical_body_length:u64_le
|| canonical_norito(KagemushaNativeContractVectorBodyV1)
```

Its pinned SHA-256 is
`13b51124f0329fc47b0aa3bf551f83f1806920c9898e7c07cd7f0730eb57fbb9`.
The complete archive is bounded at 4,096 bytes. Rust reconstructs every entry
from the authoritative V1 constants/enums and rejects noncanonical encoding,
count/order/name drift, or digest mismatch. Swift, Kotlin, and mirrored Java
expose the raw canonical archive as an optional native probe.

This digest is an ABI/tamper pin only. It is not a signature, hardware
attestation, consensus proof, settlement receipt, or source of monetary
authority.

## Capability frame

The capability frame is 96 bytes:

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `IKGMJCP1` |
| 8 | 2 | version `1` |
| 10 | 1 | platform: Android `1`, iOS `2` |
| 11 | 1 | zero flags |
| 12 | 4 | exact required-feature mask |
| 16 | 4 | maximum command payload, at least `65,536` |
| 20 | 4 | maximum response payload, at least `65,536` |
| 24 | 32 | nonzero active hardware-policy ID |
| 56 | 32 | nonzero qualification-report SHA-256 |
| 88 | 8 | zero trailer |

The offset-56 field is `qualificationReportDigest` in the mobile SDKs. It is the
active profile's qualification-report digest, not a device credential identity. The
device credential is read through operation 1 and separately checked against
its release, profile, network, key, policy, and epoch bindings.

The required mask is exactly `0xffff`. Its sixteen bits attest:

- exact-next predecessor consumption;
- one-use successor authorization;
- rollback-resistant counter and journal;
- sealed transition recovery;
- receiver-bound credit commit;
- rollback-resistant accepted-credit inbox;
- authenticated inbound staging;
- authoritative replay-root recovery;
- sender outbox reservation;
- authenticated durable retry outbox;
- atomic commit of a Core-verified candidate;
- recoverable terminal commit certificates;
- trusted time or monotonic lease;
- offline hardware-epoch rotation;
- rollback-safe counter rollover; and
- absence of a software fallback.

Unknown or missing required bits fail closed. Capability framing alone grants
no monetary authority.

## Command frame and operation inventory

A command has an 80-byte header followed by 1 to 65,536 canonical payload
bytes:

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `IKGMJCM1` |
| 8 | 2 | version `1` |
| 10 | 1 | operation code |
| 11 | 1 | zero flags |
| 12 | 32 | nonzero idempotency/request ID |
| 44 | 4 | payload length |
| 48 | 32 | payload SHA-256 |
| 80 | variable | canonical operation payload |

The V1 operation inventory is closed and contiguous:

| Code | Operation |
| ---: | --- |
| 1 | `ReadActiveHardwareCredential` |
| 2 | `StageInboundPayment` |
| 3 | `RecoverStagedInboundPayment` |
| 4 | `RecoverInboundInboxPage` |
| 5 | `PrepareExactNextTransition` |
| 6 | `RecoverPreparedTransition` |
| 7 | `CommitVerifiedCandidateAndSignTerminal` |
| 8 | `RecoverTerminalOutcome` |
| 9 | `InstallTerminalEnvelope` |
| 10 | `RecoverInstalledEnvelopeOrStateProof` |
| 11 | `SignReceiveAcknowledgement` |
| 12 | `ReleaseOutboxEntry` |
| 13 | `ReadTrustedTimeOrLease` |
| 14 | `PrepareMintAuthorization` |
| 15 | `RecoverMintAuthorization` |
| 16 | `VerifyAuthorizationAndStageMintCredit` |
| 17 | `FoldReceiveCredit` |
| 18 | `ReadPendingCreditWatermark` |
| 19 | `RotateHardwareEpoch` |
| 20 | `BootstrapAggregateState` |
| 21 | `RecoverWalletSnapshot` |
| 22 | `CreateSignedPaymentRequest` |

Codes `0`, `23` through `255`, aliases, and reused codes are invalid.

### Direct inbound staging: operations 2--4 and 11

Operation 2 accepts exactly one canonical request, its canonical payment, and
bounded staging metadata. The outer request ID is the payment's credit ID. Core
must authenticate the release and profile and verify the actual recursive proof,
terminal certificate, decrypted opening, request binding, and hardware guard
before asking hardware to mutate its inbox. Hardware then atomically stores the
exact public bytes and a rollback-resistant receipt.

Operation 3 recovers the byte-identical staged record selected by credit ID.
Operation 4 recovers an ordered page at one stable inbox revision. Its
`maximum_entries` range of 1 through 4 limits one response only; repeated
revision-consistent pages support an arbitrary backlog. Page entries are unique,
ordered, at or below the returned revision, and cursor-consistent.

Operation 11 signs the acknowledgement for one already durable inbox receipt.
Its body contains only the canonical request, canonical payment, and receipt.
The outer request ID, receipt credit ID, and payment credit ID must match.

Receiver schemas are:

| Operation | Canonical schema suffix after `iroha.kagemusha.device.v1.` |
| ---: | --- |
| 2 | `stage-inbound-payment-command` |
| 3 | `recover-staged-inbound-payment-command` |
| 4 | `recover-inbound-inbox-page-command` |
| 2/3 reply | `staged-inbound-payment-reply` |
| 4 reply | `inbound-inbox-page-reply` |
| nested staged record | `staged-inbound-payment-record` |
| 11 | `sign-receive-acknowledgement-command` |
| 11 reply | `receive-acknowledgement-reply` |

### Outgoing lifecycle: operations 5--10 and 12

Operations 5--10 implement the sole crash-recoverable outgoing lifecycle for a
peer `SendSplit` or chain-facing `RedeemSplit`. Operation 5 seals the immutable
public input preimage, reserves durable outbox capacity, and prepares one
exact-next transition. Operation 6 recovers that preparation. Core generates,
persists, and verifies the actual recursive candidate before operation 7.

Operation 7 consumes the predecessor exactly once, installs the sole successor,
and returns the recoverable hardware terminal certificate for the exact
candidate and lifecycle. The hardware commit makes a peer credit irrevocable;
there is no path that recreates the predecessor or cancels an exposed credit.
The sender remainder is immediately usable.

Operation 8 recovers the terminal outcome. Operation 9 installs the verified
canonical terminal envelope. Operation 10 recovers either one installed result
or a stable page of sender records. Only installed bytes may be exposed to a
peer or chain submitter. A missing acknowledgement retains the byte-identical
retry outbox but does not freeze the successor balance. Operation 12 releases
an outbox entry only after checking the exact installed envelope and a closed
terminal receipt: either the matching durable peer acknowledgement for
`SendSplit`, or a compact projection selected by a Core-verified finalized
redemption capability for `RedeemSplit`. Redemption receipt bytes are selectors
only: the qualified in-process service must already hold and consume the
non-constructible, operation-indexed Core capability, so a host call or matching
public digest cannot authorize release. Immutable replay anchors remain after
either terminal path.

Every sender command uses canonical schema
`iroha.kagemusha.device.v1.sender-command`; every reply uses
`iroha.kagemusha.device.v1.sender-reply`. Commands are bounded at 16 KiB and
replies at 64 KiB. A sender page contains at most four records per response,
which is a transport bound rather than a wallet-history bound.

### Mint and aggregate-state control: operations 13--21

Operation 13 reads a hiding trusted-time or monotonic-lease evidence object.
Operations 14 and 15 prepare and recover one proof-bearing mint authorization
under a caller-generated, durably stored operation ID.

Operation 16 validates one canonical `KagemushaDeviceMintStageCommandV1`
containing the exact mint authorization and finalized mint credit. A qualified
service verifies release authority, recursive proof, finalized reserve debit,
recipient binding, and journal state, then atomically stages the credit. Its
result is `KagemushaDeviceMintStageResultV1` with disposition `0` for newly
staged or `1` for an exact already-pending/consumed duplicate. It can never bind
the same operation or credit ID to different bytes.

Operation 17 folds exactly one staged credit selected by `credit_id` into the
aggregate balance, proves replay nonmembership, updates the replay root, and
returns the successor aggregate state. It is intentionally singular. Clients
repeat it to drain any backlog; there is no batch width, fan-in maximum, or
count-based rejection of valid money.

Operation 18 reads the pending-credit inclusive high-water mark. Operation 19
carries the complete balance and replay root into the next qualified hardware
epoch without an online checkpoint. Operation 20 creates the unique
hardware-owned sequence-zero aggregate state. Operation 21 returns one atomic
snapshot of the optional aggregate state, journal revision, pending-credit
count, and retry-outbox count, all as full-width values.

Control schemas and fields are:

| Operation | Schema suffix | Fields after `version`, `operation` |
| ---: | --- | --- |
| 1 | `read-active-hardware-credential-command` | none |
| 11 | `sign-receive-acknowledgement-command` | canonical request, canonical payment, inbox receipt |
| 13 | `read-trusted-time-or-lease-command` | none |
| 14 | `prepare-mint-authorization-command` | operation ID, positive amount, payer, recipient |
| 15 | `recover-mint-authorization-command` | operation ID |
| 17 | `fold-receive-credit-command` | operation ID, credit ID |
| 18 | `read-pending-credit-watermark-command` | none |
| 19 | `rotate-hardware-epoch-command` | operation ID |
| 20 | `bootstrap-aggregate-state-command` | operation ID |
| 21 | `recover-wallet-snapshot-command` | none |
| 22 | `create-signed-payment-request-command` | request ID, recipient, positive amount, validity window |

Operations 14, 15, 17, 19, and 20 repeat the outer request ID as their inner
operation ID. Operation 22 repeats it as the request ID. Read-only operations
1, 13, 18, and 21 use a fresh nonzero correlation ID. Operation 17 additionally
requires a distinct nonzero credit ID.

### Native request creation: operation 22

Operation 22 constructs and signs the complete `KagemushaPaymentRequestV1`
under the active qualified receiver key. The caller supplies only the recipient,
positive amount, independent request ID, and validity window. Hardware supplies
the authenticated network/lane/asset/release/policy context, recipient
encryption key, credential, and trusted issue time. The validity window is
between 1 and `KAGEMUSHA_REQUEST_MAX_TTL_MS_V1` milliseconds, inclusive, and
expiry is derived with checked arithmetic.

Any number of such requests may be outstanding simultaneously. Request creation
does not reserve aggregate balance or inbox capacity.

## Response frame and authentication

The response has a 116-byte header followed by payload and authenticator:

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `IKGMJRS1` |
| 8 | 2 | version `1` |
| 10 | 1 | exact operation code |
| 11 | 1 | closed status code |
| 12 | 32 | exact command request ID |
| 44 | 4 | payload length |
| 48 | 4 | authenticator length |
| 52 | 32 | payload SHA-256 |
| 84 | 32 | authenticator SHA-256 |
| 116 | variable | payload, then authenticator |

A successful response has 1 through 65,536 payload bytes and exactly 64 bytes
of canonical low-S P-256 ECDSA-SHA256 `r || s` authentication. A non-success
response has empty payload and authenticator and both header digests equal the
SHA-256 of empty bytes.

The exact success-authenticator message is:

```text
ASCII "iroha:kagemusha:device:v1:response-authenticator"
|| 0x00
|| ASCII "IKGMJRS1"
|| version:u16
|| operation:u8
|| status:u8 = 0
|| request_id:[u8;32]
|| payload_length:u32
|| authenticator_length:u32 = 64
|| payload_sha256:[u8;32]
|| capability_hardware_policy_id:[u8;32]
|| capability_qualification_report_digest:[u8;32]
```

The authenticator digest in the response header is excluded from its own
signature message. Both capability digests must be nonzero and distinct.

Operation 1 bootstraps response authentication only after its profile,
credential, policy ID, qualification digest, credential governance signature,
and embedded device key validate. Core must additionally establish release
catalog membership. Operations 2--22 verify under that accepted 65-byte
uncompressed SEC1 key and the same capability bindings.

Canonical decoding or a host-side signature check is never monetary authority.
Core recursively verifies the normalized hardware guard, aggregate transition,
terminal relation, state projection, certificate, proof release, and replay-root
update before advancing state.

## Persistence and recovery requirements

A qualified provider must implement exact-next transitions or one-use successor
keys, a rollback-resistant journal and inbox, trusted commit time, atomic and
recoverable transition certificates, authenticated durable state, a durable
payment outbox, and offline epoch rotation.

Outgoing work stages its exact immutable inputs, commits hardware state exactly
once, then generates and persists the proof and canonical envelope before
exposure. Recovery resumes that same transition and can never recreate a
consumed predecessor. Incoming work stores the exact request/payment and durable
receipt before acknowledgement. Credits may be folded continuously in the
background; before a send or redemption, the wallet synchronously folds whatever
pending credits are required. Backlog may add latency but cannot make valid value
unspendable.

Secure monetary state is intentionally non-cloneable. Backup/restore must never
permit two devices to spend the same predecessor; unrecoverable device loss has
the same consequence as lost physical cash.

## Platform entry points

Swift and Android discover the same optional native entry points:

```c
int32_t connect_norito_kagemusha_contract_vector_v1(
    uint8_t *output,
    size_t output_capacity,
    size_t *output_length
);

int32_t connect_norito_kagemusha_core_coordinator_contract_v1(
    uint32_t *output_words,
    size_t output_capacity_words
);

int32_t connect_norito_kagemusha_core_coordinator_open_v1(
    const uint8_t *storage_path_utf8,
    size_t storage_path_length,
    uint64_t *output_handle
);

int32_t connect_norito_kagemusha_core_coordinator_invoke_v1(
    uint64_t handle,
    uint8_t method,
    const uint8_t *request_frame,
    size_t request_frame_length,
    uint8_t **output_frame,
    size_t *output_frame_length
);

int32_t connect_norito_kagemusha_device_capabilities_v1(
    uint8_t *output,
    size_t output_capacity
);

int32_t connect_norito_kagemusha_device_execute_v1(
    const uint8_t *command,
    size_t command_length,
    uint8_t *output,
    size_t output_capacity,
    size_t *output_length
);

int32_t connect_norito_kagemusha_device_response_authenticator_v1_verify(
    const uint8_t *response,
    size_t response_length,
    uint8_t expected_operation,
    const uint8_t *expected_request_id,
    size_t expected_request_id_length,
    const uint8_t *hardware_policy_id,
    size_t hardware_policy_id_length,
    const uint8_t *qualification_report_digest,
    size_t qualification_report_digest_length,
    const uint8_t *device_public_key,
    size_t device_public_key_length
);
```

For the contract-vector function, `output = NULL` and `output_capacity = 0` is
the length probe: `output_length` receives the required length and the function
returns the bridge's buffer-too-small status. The second call must provide
exactly that much or more storage and returns the same canonical bytes. This
probe remains available even when the stock monetary provider correctly reports
device-unavailable.

The coordinator contract call returns the written word count and pins exactly
`[2, 23, 3, 6, 50, 8, 6, 22, 16, 0xffff]`: frame version, native ABI, peer
messages, complete wire payloads, artifact roles, relations, helper circuits,
device operations, hardware capabilities, and the required capability mask.
Its digest/inventory role is compatibility and tamper detection only.

Coordinator request and response frames start with ASCII `IKGMCOR1`, followed
by little-endian `version:u16 = 2`, `field_count:u16`, reserved zero `u32`, and
then `field_count` repetitions of `length:u32 || bytes`. A frame has at most 16
fields, each field at most 64 KiB, a request at most 256 KiB, and a response at
most 128 KiB. Methods 1 through 10 are, in order: reserve operation ID, accept
qualification, accept authenticated reply, begin sender transition, prove the
prepared sender transition, build the terminal envelope, accept the installed
terminal, recover sender, recover the byte-identical terminal envelope, and
release the outbox after a closed terminal receipt. The generic Kotlin Android
SDK uses `org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorJniV1`
`nativeContractV1`, `nativeOpenV1`, and `nativeInvokeV1`; the signed-app
`KagemushaNativeCoreJniV1` exports delegate through the same native implementation.
The Java Android facade uses the Kotlin transport. Swift invokes the corresponding
C exports through the validated native loader. The SDK checks the complete
ten-word ABI inventory, retains unsigned handle bits, serializes calls, and
correlates all returned identities/envelopes before exposing bounded fields.

`KagemushaCoreCoordinatorFrameV1` and `KagemushaCoreCoordinatorBridgeV1` are
transport layers, not implementations of `KagemushaNativeCoreCoordinatorV1`.
The shared `fixtures/offline/kagemusha_core_coordinator_frame_v1.tsv` corpus
covers every method, both sender kinds, both recovery selectors, and missing
recovery; its opaque archive strings do not represent valid proofs or credentials.
TODO: connect the typed SDK coordinator to the native-owned, canonical Norito
preparation (operation ID, wallet context, inputs digest), candidate (preparation,
selector, candidate digest, commit authorization), and recovery (operation ID,
terminal ID, wallet context, inputs digest) archives once their exact native
schemas and fixtures are available. No SDK decoder may infer those schemas
from nonempty frame fields or treat structural validation as journal authority.

Schema 2 is the sole coordinator frame schema. Reservation method 1 takes
`operation:u32`, the caller-persisted nonzero 32-byte operation ID, and the exact
public binding. It acknowledges that same operation ID after durable admission;
it never creates a replacement ID. Device operation 5 uses the complete canonical
Norito `iroha.kagemusha.device.v1.sender-public-inputs` enum as its public binding:
`SendSplit { request: Vec<u8> }` or `RedeemSplit { amount: u128, beneficiary: AccountId }`.
Untagged request bytes or concatenated amount/account bytes are invalid. The Core
operation journal checks canonical nested payment-request shape before reserving
capacity. A reservation is retry material, not authenticated monetary authority.

Release method 10 returns the retained operation ID, preparation, envelope digest,
exact installed envelope, and hardware release authorization. The frame boundary
requires the returned envelope bytes to equal the supplied envelope for both send
and redemption. Qualified Core still verifies the terminal identity, public-input
binding, envelope digest, receipt and hardware authorization before release.

The generic bridge installs no qualified durable coordinator. It validates
storage paths, method codes, and complete frames, clears outputs, and returns
device-unavailable. It never fabricates a handle, monetary response, terminal
receipt, or hardware authority. A product build may enable these operations
only by explicitly installing the process-global Rust
`KagemushaCoreCoordinatorBackendV1` once. That backend cannot be overwritten or
uninstalled and must bind an authenticated durable coordinator to qualified
non-forking hardware. There is no C/JNI installer and the stock fail-closed path
is not a coordinator implementation. The bridge also validates and bounds the
backend's complete response frame before it crosses C or JNI.

The generic bridge validates the complete outer frame and each canonical body,
but returns unavailable because it contains no qualifying monetary service. An
OEM or secure-element provider must pass physical airplane-mode, restart,
power-loss, clock-rollback, backup/restore, counter-rollover, thermal, latency,
memory, throughput, and byte-identical recovery qualification before its profile
is enabled.
