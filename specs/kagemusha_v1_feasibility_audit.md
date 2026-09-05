# KAGEMUSHA V1 feasibility audit

Source audit, 2026-09-04. This records implementation truth; it is not physical
hardware qualification or independent cryptographic review.

## Fixed first-release design

KAGEMUSHA V1 has one recursively proven aggregate balance per device lane and
asset. Its only monetary state transitions are `Bootstrap`, `MintFold`,
`SendSplit`, `ReceiveFold`, `RedeemSplit`, and `Rotate`. `ReceiveFold` consumes
exactly one staged credit. A wallet may repeat it for any backlog length; there
is no protocol batch width or cumulative receipt limit.

The peer protocol has exactly three messages:

1. `KagemushaPaymentRequestV1` (tag 1), binding one positive `u128` amount,
   recipient account, lane, encryption key, policy, request ID, and time window.
2. `KagemushaPaymentV1` (tag 2), binding the request, unique credit ID, sender
   before/after commitments, encrypted credit, trusted sender commit time,
   hardware transition commitment, and paired recursive proof.
3. `KagemushaAcknowledgementV1` (tag 3), binding the request, payment, credit
   ID, and rollback-resistant inbox receipt.

There is no receiver balance head in a request and no reservation handshake,
request mode, cancellation message, hop counter, note inventory, origin array,
ancestry, fan-in, input count, or proof-depth admission field. Distinct valid
payments against one request are independently stageable. Request expiry gates
the sender hardware commit time only; an in-window committed credit remains
acceptable indefinitely.

The hard transport requirements remain:

- paired proof bytes at or below 6,528;
- complete raw three-message exchange at or below 9,211 bytes; and
- complete `kgm1:` text exchange at or below 12,288 bytes.

These are physical transport ceilings for one constant-size operation, not
history limits. The hidden balance and logical sequence use `u128` arithmetic.

## Aggregate-state implementation

Core stores a private state containing the stable lane/asset scope, hidden
`u128` balance, release and device-policy binding, hardware epoch, full-width
logical sequence, state nonce commitment, consumed-credit sparse-Merkle root,
and public state commitment. Exact replay records remain local and grow with
the number of accepted credits; public state, proofs, and payments do not.

The transition relation enforces checked balance arithmetic, exact-next
sequence or epoch rotation, replay-root preservation or one-credit insertion,
and a zero-balance successor for a full send or redemption. Hardware guard
inputs are normalized and recursively verified; host-side signature success is
not monetary authority.

Current structural tests exercise 1,000 singular receives followed by one
1,000-unit send and downstream partial/full spending, plus 1,024 handoffs with
unchanged public and proof-envelope shape at depths 8, 64, and 1,024. Those are
model/shape tests, not 1,024 independently generated and verified recursive
proofs.

## MintFold resource status

The current Table8 SHA-256 circuit implementation is deterministic across both
Pasta fields. The original monolithic mint-authority circuit schedules 176 SHA
compression blocks across 36 jobs. Under its unchanged five-lane `k = 16`
profile it requires an estimated 85,248 rows per lane, above the 65,527 usable
rows, so generation correctly fails closed instead of raising a protocol or
transport limit.

The source contains two bounded prerequisites for replacing that monolith:

- a `k = 12` one-block mint-hash shard that binds exact plan position, block
  bytes, chaining state, release, and output state; and
- a `k = 16` ordered paired claim fold that recursively consumes shard proofs
  into one constant-size history accumulator.

Neither a shard nor a host-computed digest may authorize value. Release
completion requires integrating the claim fold into `MintFold`, authenticating
its Eq/Ep protocols and artifact roles, and equality-binding the complete
canonical mint authorization, finalized reserve credit, recipient lane,
amount, replay entry, envelope commitment, and normalized hardware guard. That
integration is not complete in the current source. The release gate must remain
closed until real key generation, proving, verification, mutation, memory, and
proof-size evidence passes without changing the public limits above.

## Hardware and persistence status

The native ABI exposes the exact 22-operation non-forking lifecycle defined in
`kagemusha_device_bridge_v1.md`. Its generic bridge validates canonical frames
and returns unavailable; it deliberately has no monetary software fallback.
An OEM provider must supply exact-next predecessor consumption, rollback-safe
journal and inbox, trusted commit time, atomic recoverable certificates,
authenticated state/outbox storage, byte-identical recovery, offline epoch
rotation, and counter rollover.

The host codecs and state-machine tests cover many binding and recovery
invariants, but they are not a qualified secure implementation. No stock
KeyMint, StrongBox, Secure Enclave, App Attest, Keychain, application database,
or file-backed signer satisfies the contract merely by producing signatures.

## Settlement status

The chain model uses one consensus-accounted liability reserve per asset.
Top-up debit and reserve credit are atomic and idempotent; peer transfers leave
the reserve unchanged; redemption consumes a unique terminal nullifier and
atomically debits reserve while crediting the beneficiary. Tests cover
underflow, duplicate/concurrent operations, partial/full redemption,
zero-balance continuation, and 1,000 independently funded liabilities.

These tests do not replace a four-validator end-to-end corridor with real
recursive proofs and a qualified device provider.

## Release conclusion

The aggregate design removes arbitrary history, hop, and fan-in limits without
making physical resources infinite. Release enablement remains blocked on the
integrated real `MintFold` recursion, at least 1,024 real handoffs, complete
crash-injection and adversarial recovery evidence, regenerated cross-SDK
fixtures and artifacts, workspace validation, independent cryptographic review,
and physical airplane-mode/restart/power-loss/clock-rollback/backup/thermal/
latency/memory/throughput qualification for each hardware profile.
