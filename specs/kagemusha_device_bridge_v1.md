# KAGEMUSHA V1 secure-device bridge

This document fixes the V1 ABI between the audited KAGEMUSHA native core and a
qualified non-forking hardware service. A missing service, incomplete profile,
invalid credential, malformed frame, or unsupported operation fails closed.
There is no software fallback.

All integers are unsigned little-endian, digests are SHA-256, reserved bytes
are zero, and every canonical Norito frame has version `1`. Frame bounds protect
parsers; they are not cumulative limits on hops, receipts, ancestry, fan-in, or
proof depth. Authenticated state and replay data may be paged, but valid staged
money cannot be rejected because a historical count was reached.

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
| 24 | 32 | non-zero active hardware profile ID |
| 56 | 32 | non-zero qualification-report SHA-256 of the active hardware profile |
| 88 | 8 | zero trailer |

The offset-56 field is named `attestationDigest` in the mobile SDKs. It equals
the active profile's `qualification_report_digest`, which the release validator
binds to the qualification-report file's SHA-256. It is not a device credential
digest or `credential_id`. The enrolled device credential remains a separate
authenticated object read through operation 1 and verified against its profile,
network, key, and epoch bindings. Capability framing alone grants no monetary
authority.

The required mask is exactly `0xffff`. Its sixteen bits attest exact-next
predecessor consumption, one-use successor authorization, rollback-resistant
counter/journal, sealed deterministic recovery inputs, one-use acceptance
tickets, durable inbox reservation, authenticated inbound staging/paging,
authoritative replay-root recovery, sender outbox reservation, authenticated
durable retry outbox, atomic commit of a Core-verified candidate, recoverable
terminal commit certificates, trusted time or lease, offline hardware-epoch
rotation, rollback-safe counter rollover, and no software fallback. Unknown or
missing required bits fail closed.

## Command and response frames

A command has an 80-byte header followed by 1 to 65,536 canonical payload
bytes:

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 8 | ASCII `IKGMJCM1` |
| 8 | 2 | version `1` |
| 10 | 1 | operation code |
| 11 | 1 | zero flags |
| 12 | 32 | non-zero idempotency or request ID |
| 44 | 4 | payload length |
| 48 | 32 | payload SHA-256 |
| 80 | variable | canonical operation payload |

The closed operation inventory is:

| Code | Operation |
| ---: | --- |
| 1 | `ReadActiveHardwareCredential` |
| 2 | `PrepareAcceptanceIntent` |
| 3 | `RecoverAcceptanceIntent` |
| 4 | `ValidateIntentReserveInboxAndIssueAcceptanceTicket` |
| 5 | `RecoverAcceptanceTicket` |
| 6 | `StageInboundPayment` |
| 7 | `RecoverStagedInboundPayment` |
| 8 | `RecoverInboundInboxPage` |
| 9 | `PrepareExactNextTransition` |
| 10 | `RecoverPreparedTransition` |
| 11 | `AbandonUncommittedPreparedTransition` |
| 12 | `CommitVerifiedCandidateAndSignTerminal` |
| 13 | `RecoverTerminalOutcome` |
| 14 | `InstallTerminalEnvelope` |
| 15 | `RecoverInstalledEnvelopeOrStateProof` |
| 16 | `SignReceiveAcknowledgement` |
| 17 | `ReleaseOutboxEntry` |
| 18 | `ReadTrustedTimeOrLease` |
| 19 | `PrepareMintAuthorization` |
| 20 | `RecoverMintAuthorization` |
| 21 | `VerifyAuthorizationAndStageMintCredit` |
| 22 | `FoldReceiveBatch` |
| 23 | `ReadPendingCreditWatermark` |
| 24 | `RotateHardwareEpoch` |
| 25 | `BootstrapAggregateState` |
| 26 | `RecoverWalletSnapshot` |
| 27 | `CreateSignedPaymentRequest` |

Codes `0`, `28`, and `255` are unknown, as is every code outside `1..=27`.
The inventory is closed; aliases or reused codes are invalid.

Operations 2 through 5 implement the compact pre-ticket exchange. Operation 2
prepares only `KagemushaAcceptanceIntentV1`, with no proof, terminal key,
recipient key, or encrypted transfer. Operation 3 recovers the identical intent.
After Core authenticates the receiver profile and signed request/intent
context, operation 4 atomically applies the private request-mode allowance,
reserves inbox bytes, records the intent decision, and issues a signed one-use
ticket containing a fresh X25519 recipient key. Operation 5 recovers that exact
ticket and key. Neither request nor intent carries the recipient encryption key.
Retries cannot create another decision, and new tickets cannot reuse unresolved
or previously accepted keys. Expiry alone never releases allowance or capacity.

Operations 6 through 8 accept the final `KagemushaPaymentV1` only against its
existing ticket allocation. Before hardware mutation, Core verifies the exact
request, compact intent, ticket, actual post-commit payment proof and certificate
under the authenticated release/profile, including decrypted opening and
receiver binding. Hardware atomically stores canonical bytes and returns a
durable opaque receipt. Exact duplicates return the same receipt; reuse of a
ticket or credit ID with different bytes conflicts. A validly committed payment
must remain stageable after delivery delay, ticket expiry, ordinary suite or
credential rotation, and later traffic; old verification authority and the
reserved delivery slot must remain available.

The receiver command bodies are distinct canonical Norito V1 archives and
repeat the outer version and operation binding. Operation 4 carries the exact
signed request and compact intent (2 KiB maximum); operation 5 carries only the
non-zero ticket ID (512 bytes); operation 6 carries the request, intent, signed
ticket, and payment (12 KiB); operation 7 carries the non-zero credit ID and
exact envelope digest (512 bytes); and operation 8 carries a full-width `u128`
journal revision, optional non-zero credit cursor, and requested page count
1--4 (512 bytes). Replies are capped at 16 KiB for ticket issue/recovery, 24 KiB
for staging/recovery, and 64 KiB for a page. The page-count bound limits one
response only; repeated revision-consistent pages support an arbitrary backlog.

Ticket replies preserve the exact signed ticket, request and intent plus Core's
reservation certificate. Staged replies preserve the public exchange, Core's
stage certificate, and the exact durable acknowledgement bytes; they never
return the private credit opening. Recovery replies must match both the selected
identity and its exact digest. Page entries must be ordered, unique, at or below
the authenticated revision and consistent with the returned cursor. Canonical
shape and public binding checks do not authenticate `GuardBundle`, a recursive
proof, Core state, trusted time, receiver key custody, or journal durability.

The [sender recovery body contract](kagemusha_device_sender_v1.md) specifies
operations 9–15/17, caller-known operation IDs, immutable creation context,
authenticated historical recovery, monotonic tombstones and bounded native
index paging. Stock dispatch validates these public schemas but remains unavailable.

Receiver archives use stable schema names rather than Rust private type names:

| Body | Canonical schema name |
| --- | --- |
| operation 4 command | `iroha.kagemusha.device.v1.reserve-acceptance-ticket-command` |
| operation 5 command | `iroha.kagemusha.device.v1.recover-acceptance-ticket-command` |
| operation 6 command | `iroha.kagemusha.device.v1.stage-inbound-payment-command` |
| operation 7 command | `iroha.kagemusha.device.v1.recover-staged-inbound-payment-command` |
| operation 8 command | `iroha.kagemusha.device.v1.recover-inbound-inbox-page-command` |
| operation 4/5 reply | `iroha.kagemusha.device.v1.acceptance-ticket-reply` |
| staged receipt nested in operations 6–8 | `iroha.kagemusha.device.v1.public-staged-inbound-receipt` |
| operation 6/7 reply | `iroha.kagemusha.device.v1.staged-inbound-payment-reply` |
| operation 8 reply | `iroha.kagemusha.device.v1.inbound-inbox-page-reply` |

For peer sends, operations 9 through 15 implement the outgoing lifecycle only
after the receiver ticket/key is fixed. Redemption uses the same lifecycle
without a peer ticket. Preparation seals exact inputs, randomness,
ticket/key, encrypted output, and deterministic recovery material, and reserves
the complete sender budget before locking the predecessor. Core generates,
durably persists, and verifies the actual recursive aggregate-state candidate
before constructing operation 12 from an authenticated-candidate capability.

Operation 12 consumes the predecessor exactly once, installs the sole
successor, and returns the recoverable hardware terminal certificate for that
exact candidate and full lifecycle. Its fixed ABI name
`CommitVerifiedCandidateAndSignTerminal` describes internal hardware outcome
authentication; it does not authorize a transported signature-only payment or
cancellation. Core subsequently proves the actual `TerminalAuthorization`
relation and narrow `CommitWrapper`, verifies the final proof/certificate, and
uses operation 14 to install the canonical terminal envelope. Operations 10,
13, and 15 recover the exact prepared, committed, or installed record after
restart or power loss. They cannot select new inputs, seeds, a different
ticket/key, another successor, or a different canonical envelope. Only the
installed result recovered by operation 15 may be exposed.

The state candidate and final proof bind the same proof-independent payment
body digest; the final proof separately binds the candidate and certificate.
The full envelope digest exists only after the final proof and is the retry/ACK
identity. Neither proof/certificate bytes nor the full envelope digest enter
the precommit body, and actual ciphertext never enters its own AEAD AAD.
Operation 11 can abandon only a provably uncommitted local preparation; it
cannot roll back a committed successor or independently release the receiver's
ticket. Genuine proof-authenticated irreversible cancellation remains
unfinished, so the current cancellation input always fails closed.

`Bootstrap`, `MintFold`, `SendSplit`, `ReceiveFoldBatch`, `RedeemSplit`,
`SuiteUpgrade`, and `Rotate` use the exact-next aggregate-state relation.
Operation 22 folds a padded fixed-shape batch of 1--16 staged credits with one
replay nonmembership/update per active slot. Repeating it drains an arbitrary
backlog without a protocol count maximum. `SuiteUpgrade` requires a separately
authenticated old-to-new verifier bridge; operation 24 is hardware rotation
only and cannot silently change suite or verifier authority.

### Control and recovery public bodies

Operations 1–3, 16, 18–20 and 22–27 use distinct canonical Norito archives.
Every archive starts with `version: u16 = 1` and its exact `operation: u8`.
The remaining command fields are in this wire order:

| Operation | Schema suffix after `iroha.kagemusha.device.v1.` | Remaining command fields | Bound |
| ---: | --- | --- | ---: |
| 1 | `read-active-hardware-credential-command` | none | 256 B |
| 2 | `prepare-acceptance-intent-command` | `intent_id: [u8;32]`, `canonical_request: Vec<u8>`, `exact_amount: u128` | 2 KiB |
| 3 | `recover-acceptance-intent-command` | `intent_id: [u8;32]` | 256 B |
| 16 | `sign-receive-acknowledgement-command` | canonical request, intent, ticket and payment byte vectors, then `KagemushaInboxReceiptV1` | 12 KiB |
| 18 | `read-trusted-time-or-lease-command` | none | 256 B |
| 19 | `prepare-mint-authorization-command` | `operation_id: [u8;32]`, `amount: u128`, canonical `payer: AccountId`, canonical `recipient: AccountId` | 2 KiB |
| 20 | `recover-mint-authorization-command` | `operation_id: [u8;32]` | 256 B |
| 22 | `fold-receive-batch-command` | `operation_id: [u8;32]`, `inbox_sequence_inclusive: u128` | 256 B |
| 23 | `read-pending-credit-watermark-command` | none | 256 B |
| 24 | `rotate-hardware-epoch-command` | `operation_id: [u8;32]` | 256 B |
| 25 | `bootstrap-aggregate-state-command` | `operation_id: [u8;32]` | 256 B |
| 26 | `recover-wallet-snapshot-command` | none | 256 B |
| 27 | `create-signed-payment-request-command` | `request_id: [u8;32]`, canonical `recipient: AccountId`, `request_mode: KagemushaPaymentRequestModeV1`, `validity_window_ms: u64` | 2 KiB |

Nested limits are request 928 bytes, intent 192, ticket 256, payment 7,552,
acknowledgement 256, aggregate state 768, profile 512, credential 768, and mint
authorization 7,936 bytes. Before operation 2 the caller generates and durably
stores a random nonzero intent ID. It is both the outer request ID and body
`intent_id`; operations 2 and 3 reject a difference, and success returns that
same intent ID. Operation 16 uses the payment credit ID as its outer request ID
and receipt credit ID. Operations 19/20 repeat their operation ID. Before each
operation 22 batch and operation 24 rotation, the caller durably stores an
independent nonzero operation ID repeated in the outer frame and body. Operation
25 likewise persists its operation ID before bootstrap, and operation 27 uses
its request ID as both the outer ID and signed request nonce. Read-only operations
1, 18, 23 and 26 use a fresh nonzero correlation ID. Operation 27 accepts only
`1..=300,000` milliseconds; the device selects trusted `issued_at_ms` and uses
checked addition to derive `expires_at_ms`.

Successful replies repeat version and operation, followed by:

| Operation | Schema suffix | Remaining reply fields | Bound |
| ---: | --- | --- | ---: |
| 1 | `active-hardware-credential-reply` | nonzero release ID, nonzero hardware-policy digest, full profile, full credential | 2 KiB |
| 2/3 | `acceptance-intent-reply` | canonical intent bytes | 2 KiB |
| 16 | `receive-acknowledgement-reply` | canonical acknowledgement bytes | 2 KiB |
| 18 | `trusted-time-or-lease-reply` | `KagemushaCommitEvidenceV1` | 512 B |
| 19/20 | `mint-authorization-reply` | canonical authorization bytes | 12 KiB |
| 22 | `fold-receive-batch-reply` | echoed inclusive sequence, active count `1..=16`, canonical aggregate state | 2 KiB |
| 23 | `pending-credit-watermark-reply` | inclusive sequence as `u128` | 256 B |
| 24 | `rotate-hardware-epoch-reply` | canonical sequence-zero aggregate state | 2 KiB |
| 25 | `bootstrap-aggregate-state-reply` | canonical sequence-zero aggregate state | 2 KiB |
| 26 | `wallet-recovery-snapshot-reply` | optional canonical aggregate state, journal revision `u128`, pending-credit count `u128`, retry-outbox count `u128` | 2 KiB |
| 27 | `signed-payment-request-reply` | exact canonical signed payment request | 2 KiB |

An absent operation 3 or 20 record is outer status `Missing` with an empty
body. Operation 1 checks profile shape and the credential governance signature,
but the release ID and hardware-policy digest still need response authentication
and Core release-catalog membership. Operation 18 exposes only a hiding evidence
commitment, not wall-clock time or payment-request signing authority. Operation
24 becomes authoritative only after response authentication, Core old-to-new
verification and atomic hardware persistence; the caller then repeats operation
1 to read the new qualification.

Operation 25 takes no host-selected state, release, proof, lane, nonce, time,
credential or capacity. The qualified native service owns those values and may
return success only after it atomically persists the unique sequence-zero state.
Operation 26 reads the optional aggregate state and all three full-width counters
from one atomic journal snapshot. The counters remain independent full-width
observations even when the optional state is absent. Pending-credit watermark
remains the separate operation 23 read. Operation 27 takes only the
host-selected recipient, closed request mode, request nonce and bounded lifetime;
its reply must match all four values and contain a valid signature under its
embedded active credential. These contracts define bodies and bindings; stock
dispatch remains unavailable because it owns none of the required authority.

### Operation 21 public mint-stage bodies

`KagemushaDeviceMintStageCommandV1` is one canonical Norito archive with
`version: u16 = 1`, `canonical_authorization: Vec<u8>`, and
`canonical_mint_credit: Vec<u8>`, in that order. Its schema is
`iroha_data_model::kagemusha::kagemusha_device_v1::KagemushaDeviceMintStageCommandV1`
with root alignment 8. The outer body is bounded at 65,536 bytes; each exact
nested archive independently retains its 7,936-byte limit. Decoders validate
both nested shapes and their complete public authorization/credit binding.

`KagemushaDeviceMintStageResultV1` has `version: u16 = 1`, `disposition: u8`,
and nonzero `credit_id: [u8; 32]`, in that order, with root alignment 2 and the
same schema namespace. Its cap is 128 bytes. Disposition 0 denotes newly staged
credit and 1 an exact already-pending or consumed credit. The result must name
the command's exact credit. Replaying the same outer operation ID must recover
the byte-identical original response, including its original disposition;
using a different operation ID may report an exact duplicate. Reusing an
operation or credit ID with different canonical bytes conflicts.

Private reservation openings, key handles, hardware snapshots, and complete
Guard certificates never enter these public bodies. The outer request ID must
equal the authorization context's operation ID. A qualified native service
must verify the authenticated release, recursive proofs, finalized mint,
reserved recipient binding and hardware journal, then atomically stage the
credit before returning an authenticated result. The public codec and C
validation exports check shape only. Stock C/JNI validates operation 21's body
but still returns unavailable; the SDK-to-OEM staging adapter remains
unfinished. The [Rust-owned structural fixture](../fixtures/offline/kagemusha_device_mint_stage_v1.json)
is shared by the maintained SDKs and is not monetary proof or hardware evidence.

The response has this 116-byte header, followed by payload and authenticator:

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

A successful response has 1--65,536 payload bytes and exactly 64 authenticator
bytes. The authenticator is a canonical low-S P-256 ECDSA-SHA256 signature in
fixed `r || s` form under the enrolled `credential.device_public_key`. A
non-success response has empty payload and authenticator and both header digests
equal SHA-256 of the empty string.

The exact signature message is the concatenation below. Integer fields remain
little-endian. No field has an implicit length or encoding beyond the listed
bytes.

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

The header's authenticator SHA-256 is transport integrity for the signature and
is excluded from its own signing message. Both capability digests must be
nonzero and distinct. The operation-1 payload must first validate its profile,
governance credential and equality of `hardware_policy_digest`, profile ID and
the capability policy ID; its profile qualification-report digest must equal
the capability qualification digest. Only then may its embedded device key
verify that response. Core must additionally establish signed release-catalog
membership for the returned release before the session enables monetary use.
Every later successful response verifies under that accepted key and the same
capability bindings.

Before commit, unavailable capacity may reject a new preparation without
changing the predecessor. After operation 12 commits, only success, retryable
concurrency, or governed recovery are legal; the same command must eventually
recover the committed result.

Canonical decoding or a host-side signature check is never monetary authority.
The native core verifies response authentication, credential/profile,
normalized GuardBundle, actual persisted state candidate, post-commit terminal
relation and final wrapper, exact state projection, certificate, and release
before advancing its internal typestates. Signature-only terminal material,
compact intents, and shape-valid proof bytes cannot grant monetary or
capacity-release authority. The generic bridge now performs exact outer-frame
validation and operation-specific canonical body checks for every code 1–27.
Valid commands still return unavailable because the stock service has no
monetary engine. Codec or frame tests do not establish real-proof or hardware
qualification.

Operation 8 pages staged inbox records at a pinned journal revision. Operation
15 pages the authenticated sender index, including installed retry entries and
terminal tombstones, four at a time. These are the exact enumeration mechanisms
for recovery after operation 26 atomically establishes the aggregate state and
counts. Operation 23 separately reads the pending-credit watermark when the
provider needs that selector.

## Platform entry points

Swift and Android discover equivalent optional native entry points:

```c
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

For operation 1, the last pointer is null and its length is zero; the verifier
extracts the key only after checking the qualification payload bindings above.
For operations 2--27, it is the accepted 65-byte uncompressed SEC1 key from
operation 1. This entry point authenticates the response and its capability
bindings. It does not substitute for Core's release membership, recursive-proof,
state-transition or journal checks.

The stock bridge intentionally provides no qualifying hardware service. An
OEM or secure-element implementation must pass physical airplane-mode,
restart, power-loss, clock-rollback, backup/restore, counter-rollover, thermal,
latency, memory, throughput, and byte-identical recovery qualification before
its profile may be enabled. Swift, Kotlin, mirrored Java, JavaScript, Python,
C#, JNI, QR, and NFC use one audited native cryptographic core; SDK layers own
only framing, storage, transport, and orchestration.

JNI forwards bounded byte arrays through the same C entry points. Signed Java
array lengths are converted without narrowing before the 80..=65,616-byte
command bound is applied; malformed C results become `IllegalArgumentException`,
unavailable remains a null optional service result, and every other invalid
status or response length becomes `IllegalStateException`. Copied command and
response buffers are zeroized. This wiring does not install an Android applet,
Apple credential, OEM service, hardware profile, or qualification report.
