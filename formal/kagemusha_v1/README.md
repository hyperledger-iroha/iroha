# KAGEMUSHA V1 formal model

`KagemushaV1.tla` is the finite-state safety oracle for the clean-slate
KAGEMUSHA aggregate-balance protocol. It models one asset, one pooled reserve,
and exactly three peer messages: `PaymentRequestV1`, `PaymentV1`, and
`AcknowledgementV1`.

The protocol has no hop, note, origin, ancestry, fan-in, receipt-count,
transition-count, or proof-depth field. Each device lane holds one hidden
aggregate balance, one replay accumulator root, a hardware epoch, a logical
sequence, a hardware counter, a nonce, and a public state commitment. Public
payments and proofs have a fixed shape independent of the length of the state
history.

## Modelled protocol

- `Bootstrap` creates the hardware-bound zero state.
- `TopUp` atomically moves online value into the pooled reserve and produces a
  unique, finalized, device-bound mint credit. `MintFold` absorbs that credit
  into the aggregate state.
- `SendSplit` consumes the exact current hardware state, subtracts one positive
  request amount, and always installs a sender successor. A full spend therefore
  leaves a valid zero-balance successor. The committed amount immediately
  becomes an irrevocable receiver-bound credit. Proof generation, canonical
  envelope persistence, and exposure follow the hardware commit; crash recovery
  retains the committed state and exact outbox bytes.
- Receiver staging atomically stores the exact canonical payment and its
  rollback-resistant acknowledgement. Redelivery of the same bytes returns the
  same acknowledgement. Different bytes under the same credit ID enter only the
  rejection evidence set.
- `ReceiveFold` adds one staged credit to the aggregate balance after replay
  nonmembership, then updates the replay root. Repeating this fixed-shape step
  drains an arbitrarily large backlog. No amount or count is charged to a
  request, and no request state is consumed. The sample data deliberately maps
  two distinct credits to the same request and accepts both.
- `RedeemSplit` creates a full or partial terminal voucher and a successor
  state. Application consumes that unique voucher, debits the reserve, and
  credits the online account atomically.
- `Rotate` consumes the exact current head and carries the entire balance and
  replay root into the next hardware epoch without an online checkpoint. The
  logical sequence and nonce advance while the epoch-local hardware counter
  restarts.

Every monetary device transition is prepared against one exact head. Commit
consumes that head once and records exactly one successor. A stale preparation
cannot commit after another transition advances the lane. Monetary authority
requires a hardware transition certificate and a recursively verified
`GuardBundle`; a host-side certificate alone cannot reach a transportable
payment.

The request binds network, asset, exact positive amount, recipient account and
lane, recipient encryption key, hardware policy, request ID, and validity
window. It deliberately contains no receiver balance head or state commitment.
The validity window is checked only at the sender's trusted hardware commit.
After that commit, proof recovery, exposure, receiver staging, duplicate
delivery, folding, and acknowledgement remain valid after expiry.

## Conservation and availability checks

The model checks:

- `reserve = total top-ups - total redemptions`;
- the reserve equals device balances plus finalized mint credits, in-flight peer
  credits, and unapplied redemption vouchers;
- online value plus the reserve remains constant;
- exact-next, one-successor hardware transitions and preparation binding;
- exact request/receiver binding and commit-time validity;
- recursively verified hardware authority before exposure;
- durable staging before acknowledgement, byte-identical duplicate replies,
  and rejection of conflicting credit bytes;
- replay nonmembership and monotonic replay-root updates;
- fold eligibility without request-use or receipt-count admission state;
- immediate usability of the sender successor regardless of an outstanding
  retry outbox;
- finalized mint admission, full and partial redemption, unique terminal
  vouchers, and reserve-underflow prevention;
- complete balance/replay preservation across offline hardware rotation; and
- append-only transition, replay, conflict, duplicate, and acknowledgement
  evidence plus stable post-commit envelopes.

The finite replay-root value used by TLC is the mathematical set represented by
the sparse Merkle root. Production public state carries only the constant-size
root; the exact local replay index may grow with received credits.

## Proof-gate traces

`KagemushaV1ProofGates.tla` provides deterministic positive payment and
redemption traces. They cover two distinct payments against one request,
delivery after request expiry, crash/recovery, exact duplicate delivery,
conflicting delivery rejection, two receive folds, partial redemption, offline
rotation, a final full redemption, and a zero-balance successor.

The following mutations must produce counterexamples to the named safety
boundary:

1. `nonrecursive-payment` — transport authority without recursive guard
   verification.
2. `fork` — a second successor from a consumed hardware head.
3. `ack-before-stage` — acknowledgement before durable staging.
4. `conflict-accepted` — accepting different bytes under an existing credit ID.
5. `replay` — folding a credit already present in the replay root.
6. `expired-commit` — hardware commit outside the request window.
7. `reserve-accounting` — online redemption credit without reserve debit.
8. `rotation-loss` — hardware epoch rotation that drops aggregate state.

The repository test runner supplies `Mutation`, replaces `Spec` with
`HarnessSpec`, retains the complete invariant/property list, and adds
`HarnessCompletion`. If `TLA2TOOLS_JAR` is not configured, checker-dependent
tests skip explicitly; source-level hard-cut checks still run.

## Exploration bounds are not protocol limits

Every constant in `KagemushaV1.cfg` exists solely to keep TLC's state graph
finite: devices, identifiers, amounts, time, epochs, logical sequences, and
hardware counters. None may be copied into production admission logic. Larger
alternate configurations extend exploration coverage; they do not extend or
change the protocol.

This model is a safety abstraction. It does not prove circuit soundness,
attestation security, cryptographic privacy, sparse-tree implementation,
canonical Norito bytes, proof/envelope size, physical durability, liveness,
thermal behavior, or throughput. Those remain independent release gates.

With the checksum-pinned TLA+ jar installed, run:

```sh
cd formal/kagemusha_v1
java -Xmx768m -XX:+UseParallelGC -cp /absolute/path/to/tla2tools.jar \
  tlc2.TLC -simulate num=500 -depth 80 -seed 923 -workers 1 \
  -config KagemushaV1.cfg KagemushaV1.tla
```

For release evidence, retain the checker version, configuration, source
digests, search strategy, seed, generated/distinct state counts, depth, and
every invariant/property result. A bounded or partial run is never evidence of
unbounded execution; history independence comes from the protocol state and
proof shape, not the size of a TLC configuration.
