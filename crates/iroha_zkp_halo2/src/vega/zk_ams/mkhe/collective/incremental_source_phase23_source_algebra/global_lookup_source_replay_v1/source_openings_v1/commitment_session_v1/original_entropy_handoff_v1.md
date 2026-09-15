# Original materializer entropy handoff

The source materializer retains its exact concrete `R` after the 43 encrypted
source records. The original key, source snapshots, manifests, materialized
accumulators and correspondence capability remain in the same move-only owner.
Source algebra and authenticated replay preserve that owner. Only the private
replay transition can consume `original_random` into its commitment session.
It validates the source and reserves the existing complete inventory before
moving the RNG; no bytes are sampled during this handoff.

The session context is the existing canonical materializer `bundle_digest`.
That owner already binds the source receipt, original key and authority, fixed
profile, roster, batch, input and materialized roots, and all 43 manifest digests.
The handoff accepts neither a replacement RNG nor a caller-selected context.
The source receipt/context validation remains mandatory after the RNG has moved;
immutable source authentication does not require the RNG to remain in two owners.
The opaque completed source prefix and exact D/S stage cursors are unchanged.

Every commitment blinding uses canonical big-endian T256 rejection sampling:
a nonzero scalar strictly below the modulus, at most 128 independent 32-byte
requests. The new request counter is derived from the actual inventory:
72,386 × 128 × 32 = 296,493,056 bytes. It charges before the actual fallible fill,
including failures. This counts commitment requests only. Original source
encryption accounting remains separately owned; this is not an OS entropy I/O
or complete proof resource claim. The original RNG error becomes
`RandomUnavailable`; allocation/count overflow remains `ResourceCeilingExceeded`.

The enclosing session removes its live state before sampling, so an error or
unwind drops the same RNG and all owned blinding scratch and prevents retry.
The canonical crypto chunk owns sampled-byte erasure, and the scalar/MSM owners
retain their existing erasure. Ordinary fill errors and bad metadata also clear
the supplied destination. A borrowed RNG remains borrowed for the entire owner
lifetime; dropping its adapter releases that borrow and does not independently
erase the caller's underlying RNG. The health wrapper retains its own original
reservoir/poison rules. This module adds no second health implementation and does
not claim complete secret cleanup, memory-locking or side-channel qualification.

TODO: supply the actual Core-to-MKHE correspondence/materializer callsite. Core's
`HealthCheckedCryptoRngV1` currently serves the separate credential-admission
route. Its canonical health checks must remain at that owner when an actual
MKHE producer is wired; the private source/session factory does not authenticate
an arbitrary RNG or mint the still-unavailable correspondence/source-algebra
and replay-sink authorities. The normal production module is registered, but no
full-source positive fixture, comparator/delta/storage continuation, final proof
or release admission is established here.

Unit fixtures exercise the real Production entropy branch with an isolated
borrowed byte source: continuation, exact nonzero sampling, duplicate poisoning,
partial error, unwind, overflow, invalid shape, full rejection budget and drop.
They grant no authenticated source/proof authority. Existing test-only KAT
entropy remains unchanged; its u16 ordinal framing covers source/D/S fixtures,
not the entire later 72,386-point inventory. Native execution is required before
these authored tests become validation evidence.
