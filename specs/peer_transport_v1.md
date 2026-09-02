# Offline Cash V1 peer transport

Offline Cash V1 has one transport-neutral five-message exchange:

1. the receiver emits `PaymentRequestV1`;
2. the sender emits a release-bound proof-bearing
   `OfflineCashAcceptanceIntentAuthorizationV1`;
3. after verifying that proof and reserving capacity, receiver hardware emits
   one exact-amount `OfflineCashAcceptanceTicketV1`;
4. the sender commits an irreversible receiver-bound credit and emits
   `PaymentV1`; and
5. the receiver durably stages the exact payment and emits
   `AcknowledgementV1`.

An authenticated `OfflineCashNoCommitClosureV1` is a separate recovery
envelope, not a sixth message in the normal payment exchange. It embeds the
exact request, the original sender authorization, the exact issued ticket, and
the proof that closes that intent without committing a payment. Its bindings
prevent a closure from being replayed across a different request,
authorization, ticket, release, suite, verifying-key set, or artifact manifest.

All binary values are canonical Norito. Text transport is exactly `oc1:` followed
by unpadded base64url of one canonical binary value. There are no alternate
profiles, compatibility discriminators, legacy prefixes, or heuristic decoders.

## Semantics

The signed request identifies the authenticated release, network, exact asset
incarnation and scale, pooled reserve, recipient, complete compact receiver
credential, policy epoch, request ID/window, and one positive exact amount. It
never carries a mutable balance head. Any number of requests may coexist, and
every distinct valid payment against the same request remains acceptable.

The sender authorization carries a compact one-use intent and paired proof. The
proof hides sender credential, lane, epoch, predecessor, successor, and balance
while proving qualified enabled-profile hardware, sufficient private balance,
and an exact one-use predecessor authorization. Receiver intent-decision state
and physical inbox capacity cannot change until the authenticated native verifier
accepts it. The ticket then binds the exact request and intent digests, amount,
asset incarnation, reserved inbox bytes, recipient X25519 key, receiver profile
and policy epoch, and exclusive sender-commit deadline.

The payment carries one unlinkable transition nullifier/credit ID, the compact
intent and complete ticket, exact request/ticket bindings, recipient key,
amount and ciphertext commitments, typed encrypted-credit envelope, hardware
profile/policy, opaque time-or-lease evidence, recoverable terminal certificate,
and constant-size paired wrapper proof. It exposes no sender predecessor or
successor commitment, stable credential audit, lane, or hardware epoch. Only
the sender's hardware commit must occur before ticket expiry. Delivery,
staging, folding, spending, and redemption remain valid indefinitely afterward.

The receiver acknowledges only after rollback-resistant staging of the exact
payment bytes and credit ID. Exact duplicate delivery returns the byte-identical
durable acknowledgement. Reusing a credit ID with different bytes is corruption
and fails closed. A missing acknowledgement never cancels the credit or freezes
the sender successor.

## Bounds

Bounds protect parsers and physical transports; none depends on balance history,
receipt count, fan-in, origins, hops, ancestry, or proof depth.

| Value | Canonical binary maximum | `oc1:` text maximum |
| --- | ---: | ---: |
| `PaymentRequestV1` | 1,024 bytes | 1,370 bytes |
| `OfflineCashAcceptanceIntentAuthorizationV1` | 7,936 bytes | 10,586 bytes |
| `OfflineCashAcceptanceTicketV1` | 1,024 bytes | 1,370 bytes |
| `PaymentV1` | 7,936 bytes | 10,586 bytes |
| `AcknowledgementV1` | 512 bytes | 687 bytes |
| `OfflineCashNoCommitClosureV1` recovery envelope | 16,384 bytes | 21,850 bytes |
| terminal request/payment/ack trio | 9,211 bytes | 12,288 bytes |
| pre-ticket request/authorization/ticket absolute cap | 9,984 bytes | 13,326 bytes |
| complete five-message absolute cap | 18,171 bytes | 24,244 bytes |
| paired recursive proof | 6,528 bytes | not separately transported |

The pre-ticket request/authorization/ticket exchange has an 8,960-byte raw
qualification target, distinct from its 9,984-byte decoder ceiling. The
complete five-message exchange has a 16,384-byte raw qualification target,
distinct from its 18,171-byte raw and 24,244-byte text decoder ceilings. The
complete raw cap is the 9,211-byte terminal-trio cap plus the independently
bounded authorization and ticket. The 9,211/12,288 limits apply only to the
terminal three-message subset and cannot be presented as the complete
proof-bearing exchange. The independently framed no-commit recovery envelope
has its own 16,384-byte raw and 21,850-byte text ceilings and is excluded from
all normal-exchange totals.

Decoders enforce the applicable byte ceiling before allocation, require the
canonical frame and exact V1 type, reject trailing bytes, and then run semantic
validation. A lower implementation transport limit is not an Offline Cash
profile and cannot be advertised as offline-capable.

## QR

A QR implementation carries each independently framed `oc1:` value directly
when it fits one symbol.
For animated QR, frames contain a session ID, value kind, total encoded length,
offset, frame payload, and checksum. The receiver accepts frames in any order,
deduplicates byte-identical overlap, rejects conflicting overlap, and decodes only
after the complete bounded byte range is present. Frame count is derived from the
bounded value length and selected symbol capacity; it is not a cash-history field.

## NFC and local radio

NFC, Nearby, Multipeer, and equivalent links exchange the canonical binary value
with an explicit value kind and total length. Fragmentation and retry are transport
concerns. Reassembly uses the same overlap/conflict rules as animated QR and must
not acknowledge a payment before the device's irreversible inbox stage completes.

Transport encryption does not replace the recipient-only canonical
`OfflineCashEncryptedCreditEnvelopeV1`, recursive proof, or hardware
authorization. Plaintext transport metadata must not contain balance openings,
replay-index contents, stable proof audits, or journal material.

## Conformance

Rust, Swift, Kotlin, mirrored Java, JavaScript, Python, C#, JNI, QR, and NFC must
decode and re-encode identical canonical fixtures. Conformance covers shuffled
frames, missing/forged sender authorization, duplicate/conflicting intent
decisions, delayed delivery after expiry, exact duplicate delivery, credit-ID
conflict, low-order X25519 keys, malformed AEAD envelopes, truncation,
over-limit input, noncanonical base64url, trailing bytes, proof/output
substitution, no-commit request/authorization/ticket substitution, restart
between staging and acknowledgement, and byte-identical retry.
