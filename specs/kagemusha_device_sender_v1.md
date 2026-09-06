# KAGEMUSHA V1 sender recovery device contract

`connect_norito_bridge/src/kagemusha_device_bridge_v1/sender_payload.rs` defines
the canonical public bodies for secure-device operations 5–10 and 12. These are
shape and binding codecs, not a monetary software implementation. Stock C/JNI
dispatch validates every body and returns unavailable until a qualified,
attested, non-forking hardware provider is installed.

## Operation identity and authority

Before its first native call, the wallet generates and durably retains an
independent nonzero 32-byte operation ID. It must not derive that ID from an
amount or payment request because the same reusable request can receive more
than one distinct valid payment. Every single-operation command repeats the ID
and must match the outer command frame exactly. Reuse with different bytes or
context is a conflict.

The public-input digest is SHA-256 over the domain
`iroha:kagemusha:device:v1:sender-public-inputs`, a zero byte, the canonical
preimage length as little-endian `u64`, and the canonical Norito preimage. That
preimage binds version 1, operation ID, complete creation wallet context, and
either the exact signed `KagemushaPaymentRequestV1` or a positive redemption
amount and canonical beneficiary. There is no receiver reservation, handshake
sub-protocol, cancellation path, or alternate request policy.

The creation context includes the network, device lane, asset, scale, proof
release, asset incarnation, hardware profile and policy epoch, authenticated
credential ID, full-width hardware generation and epoch ID, and device-policy
binding. These are selectors, not authority. The qualified native session must
authenticate current state and every retained historical record.

Ordinary credential, proof-suite, and hardware-epoch rotation must not strand a
committed outbox. Historical recovery is valid only for the same stable lane,
network, asset, scale, and asset incarnation. Its creation generation cannot be
greater than the current generation; equal generations require the same epoch
ID. Historical state can recover existing work but cannot prepare new work.

## Canonical operations

All command and reply bodies are bounded canonical Norito archives. Command
bodies are at most 16 KiB and replies at most 64 KiB. Appended bytes, unknown
variants, substituted outer IDs, noncanonical archives, and mismatched native
contexts reject before dispatch.

| ABI operation | Sender body |
| --- | --- |
| 5 `PrepareExactNextTransition` | Original public inputs; reserves outbox capacity and fixes the exact predecessor/successor transition. |
| 6 `RecoverPreparedTransition` | Original input digest; returns the byte-identical retained preparation or later phase. |
| 7 `CommitVerifiedCandidateAndSignTerminal` | Original input digest, preparation ID, and persisted Core-verified candidate digest. |
| 8 `RecoverTerminalOutcome` | Original input digest; returns the immutable committed outcome or later phase. |
| 9 `InstallTerminalEnvelope` | Original binding plus exact canonical payment or redemption envelope. |
| 10 `RecoverInstalledEnvelopeOrStateProof` | Single-operation lookup or a revision-pinned bounded index page. |
| 12 `ReleaseOutboxEntry` | Exact installed envelope plus either its durable payment acknowledgement or the compact selector bound to a Core-verified redemption-settlement capability. |

The durable phases are `Prepared`, `CandidatePersisted`, `Committed`,
`Installed`, and `Released`. Observations may skip intermediate phases after a
lost return, but they cannot regress, change immutable selectors, reuse a
consumed predecessor, or replace retained bytes. Missing is an authenticated,
tombstone-aware lookup result; an empty response or transport error is never
interpreted as missing. A committed credit cannot be cancelled.

Only an installed result may carry terminal envelope bytes. Those bytes must
match the retained public inputs, candidate, commit certificate, outcome, and
envelope digest. A peer acknowledgement releases only its byte-identical payment
outbox entry. A redemption selector is not authority: the qualified in-process
service must bind it to and consume the non-serializable
`VerifiedKagemushaRedemptionReleaseV1` that Core created from the complete
finalized operation status and caller-pinned trust anchor. Raw operation-12
bytes or a host-computed digest can never release a redemption outbox entry.

Recovery pages are ordered by operation ID, contain at most four entries per
response, and pin a stable full-width index revision. This is a transport page
size, not a backlog or history limit: callers continue with the returned cursor
until the authenticated index is exhausted.

Core projection helpers compare the native selectors with actual
`PreparedOutgoingCandidateV1` and `DurableOutgoingEnvelopeV1` values. They do
not expose private state, substitute for recursive verification, manufacture a
candidate capability, or authorize a hardware transition.

## Validation scope

Adjacent tests cover canonical framing, operation/input binding, lost-return
recovery, immutable terminal digests, tombstones, full-width revisions,
historical recovery across rotation, exact page selection, and cross-wallet or
epoch substitution. These software tests do not qualify an OEM secure service
or physical device profile.
