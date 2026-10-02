# KAGEMUSHA V1 normalized GuardBundle

The `GuardBundle` is the circuit-normalized contract between KAGEMUSHA's
recursive monetary relations and an approved non-forking hardware provider. It
is the only offline hardware-authority path. A platform signature, raw
attestation chain, host validation result, or software fallback is not a
substitute.

## Profile and credential

Governance publishes one authenticated `KagemushaHardwareProfileV1` per
qualified provider/product/firmware class. The profile binds:

- release and network;
- provider, product class, firmware measurement policy, and policy epoch;
- credential issuer and transition-certificate verifier;
- the complete required capability set;
- proof-suite and normalized-guard protocol identities;
- qualification-report digest and validity window; and
- suspension state for new issuance and redemption.

Online enrollment verifies the raw OEM/platform evidence and issues a compact
`KagemushaHardwareCredentialV1`. The credential binds the exact network, device
lane commitment, transition public key, profile, firmware policy, policy epoch,
hardware epoch/generation, and credential validity. Recursive proofs consume
the credential and authenticated profile membership; raw attestation bytes are
not transported in peer payments.

The required capability set is indivisible:

1. exact-next state transition or one-use successor authorization;
2. rollback-resistant journal and logical sequence;
3. rollback-resistant accepted-credit inbox with exact canonical-byte
   deduplication and conflicting credit-ID rejection;
4. trusted commit time or a securely bounded monotonic authorization lease;
5. sealed transition inputs and deterministic recovery material;
6. atomic, recoverable transition certificates;
7. authenticated durable monetary state and payment outbox;
8. authoritative replay-root custody and authenticated sparse-tree recovery;
9. offline hardware-epoch rotation and rollback-safe counter rollover; and
10. no software fallback.

A profile missing any required capability is online-only. Stock KeyMint,
StrongBox, Secure Enclave, App Attest, or an equivalent signing API does not
qualify unless an OEM or secure-element service implements and passes the full
contract.

## Complete credential carrier authentication

The internal PlatformCredential proof uses exactly three instance columns
`[56, 8162, 8162]` at `k = 16`. Rows `0..42` of its semantic column retain the
credential digest, app-policy binding, Eq/Ep audits, and each parity's exact
544-byte transported SHA history. Rows `42..56` contain the four original IPA
carrier commitments (two compressed `u128` limbs each), Eq/Ep challenges, and
the four carrier evaluations. Both fields carry the same fourteen values.
There is no scalar-only accepting credential shape.

Each complete carrier retains its original ordered sources: compressed source
point limbs, both canonical aggregate coefficient limbs, then every original
audit-bound value. Its capacity is `4 * 2016 + 98 = 8162`; the shorter active
prefix is zero-padded through constrained constant cells. The coefficient
challenge still hashes the complete original equation source namespace, tags,
selectors and bound-value count. The new fourteen values are separately
proof-bound rather than recursively inserted into that original audit hash.
The reciprocal dense MSM retains every native aggregate equation. A carrier
alone confers no authentication.

Both full carrier columns are opened by actual proof-supplied IPA commitments.
The hybrid proof reader recomputes the semantic-column commitment, checks the
corresponding two compressed carrier points against rows `42/44` for Eq and
`46/48` for Ep, reads the complete ordinary transcript and openings, and emits
its genuine accumulator. Native acceptance requires the retained terminal IPA
decision. Recursive Guard and Mint consumers perform the same full hybrid
reading before the original credential-history folds and merges. They compare
the complete fourteen-value tails across parities; history remains exactly
rows `8..42`. Guard binds all four complete semantic columns in its reciprocal
audit. Mint retains its original Claim14 bound prefix and appends Credential14
once, so its complete native audit and reciprocal carrier bind all 28 values.

Carrier equality across the two fields uses the injective common-prime encoding
`x = r + q * (2^127 - 1)` for every `u128`, with `r < 2^127 - 1` and
`q in {0,1,2}`. Eighty ordered quotients form one base-three pack; all 103 packs,
range bounds and padding are constrained by the streaming machine. Challenges
are nonzero 125-bit values derived after all four original commitments, with
credential domain `kgpcrlc1`, version 1, actual `k` and capacity. Both full
carriers are evaluated at both challenges. Each carrier has an independent
14-advice-column lane and 49,392 physical rows, sharing only an actually
configured identical 15-bit range table. The complete configured Base advice,
lookup, row, key and resident-memory measurements remain authoritative; the
hard 1,024-advice ceiling is unchanged.

The probabilistic equality argument is conditional on the authenticated IPA
commitments and transcript/random-oracle assumptions. Layout or native MSM
parity alone is not proof acceptance. Qualification must execute a genuine
paired credential proof, the complete Guard and Mint consumers, original
source/coefficient/padding and all fourteen tail substitutions, history and
policy substitutions, malformed topology, and the unchanged terminal decision
refusals. Credential, Guard and descendant protocols, keys and golden outputs
must be regenerated through their actual producer before qualification; no
signed or proof bytes are edited to match this representation.

## Private state and public commitment

Each `(network, device lane, asset, asset incarnation)` has exactly one private
monetary state:

```text
balance: u128
device/lane/profile/policy binding
hardware epoch and logical sequence
consumed-credit sparse-Merkle root
state nonce
```

The public state commitment deterministically binds every field while hiding
its openings. The recursive relation proves one exact predecessor-to-successor
transition. No state or guard field carries hop count, ancestry, origin set,
note inventory, fan-in count, accepted-credit count, or proof depth.

The normalized bundle contains fixed-width commitments to:

- authenticated release, proof suite, profile, and credential;
- operation kind and exact network/asset/pool identity;
- predecessor and successor public state commitments;
- hardware epoch and exact-next logical sequence relation;
- old and new replay roots;
- operation-specific request, credit, mint, redemption, or rotation context;
- trusted commit-time evidence;
- sealed transition intent and candidate commitment;
- durable journal and outbox/inbox commitment;
- atomic transition certificate and hardware terminal commitment; and
- the exact public statement consumed by the paired recursive wrapper.

All unused fixed-shape operation fields are canonical zero. Operation selection
is constrained in-circuit; a host-selected branch or unproved certificate cannot
change monetary semantics.

## Operation relations

`Bootstrap` proves a qualified credential and establishes zero balance, the
canonical empty replay root, initial hardware epoch/sequence, and one usable
successor state.

`MintFold` verifies one finalized reserve-backed mint credit and its recursive
authorization/finality chain, proves that its recipient credential opens to the
active lane, proves replay nonmembership for its credit ID, checked-adds the
amount, and updates the replay root.

`SendSplit` verifies a positive amount no greater than the private balance,
checked-subtracts it, and binds one unique transition nullifier and receiver
credit to the signed request, recipient account/lane/key, sender before/after
commitments, encrypted opening, trusted commit instant, and terminal hardware
commitment. A full spend still creates a zero-balance successor.

`ReceiveFold` verifies one complete request-bound payment and its recursively
verified normalized sender guard, decrypts/authenticates the opening for the
request key, proves credit-ID nonmembership, checked-adds the amount, and
updates the replay root. It has one credit input and no active-count field.

`RedeemSplit` verifies a positive full or partial amount, checked-subtracts it,
and binds one terminal voucher/nullifier and beneficiary. The voucher's
recursive state and hardware commitment are the only reserve-debit authority.

`Rotate` carries the complete private balance, replay root, asset/pool binding,
and sequence continuity into one exact successor hardware epoch. The prior
epoch is irreversibly invalidated. Rotation does not require an online
checkpoint and cannot alter value or replay history.

Every addition/subtraction and counter conversion is checked. Overflow,
underflow, stale predecessor, skipped/reused sequence, wrong replay path, proof
substitution, or a second successor fails without partial mutation.

## Sender transition and recovery

Before hardware mutation the host persists a sealed transition intent with the
exact candidate inputs, randomness, proof plan, receiver/request binding, and
canonical-envelope recovery material. It reserves durable outbox capacity and
locally verifies the actual candidate relation. Hardware then consumes the
predecessor once and returns an atomic recoverable certificate bound to that
candidate and exact-next successor.

After commit, Core recovers or generates the terminal proof, verifies it, and
persists the complete canonical payment before any exposure. Recovery may only
resume this transition. It cannot recreate the predecessor, select another
receiver or successor, replace proof inputs, or emit different bytes.

At commit the amount becomes an irrevocable receiver-bound credit. The sender
remainder is immediately usable. Missing acknowledgements retain only the
byte-identical retry outbox and never freeze value. There is no cancellation,
non-commit closure, acceptance reservation, or ticket-release authority.

## Receiver inbox and folding

Receiver hardware validates the signed request binding, payment proof,
certificate, encrypted opening, credit ID, and canonical bytes before atomic
staging. Only then may it emit a signed acknowledgement binding request,
payment, credit ID, and durable inbox receipt.

Exact duplicate bytes for one credit ID recover the same receipt and
acknowledgement. Different bytes for an existing credit ID are a conflict. Any
number of live requests and any number of distinct credits for one request are
permitted; invoice deduplication is not a hardware monetary rule.

The inbox can stage many credits while one lane serializes state transitions.
Background work folds one credit at a time. Before a send or redemption, the
wallet synchronously folds the pending credits required for the amount. Finite
inbox space may apply backpressure before receipt, but no accepted credit may be
evicted, cancelled, rejected by count, or stranded by a backlog.

Request expiry is checked only against trusted sender hardware commit time. A
payment committed in-window remains verifiable and stageable indefinitely;
delivery delay, device restart, ordinary verifier rotation, profile refresh, or
request expiry cannot revoke it.

## Replay storage and backup

Qualified hardware owns the authoritative sparse-Merkle root. External replay
nodes are authenticated against that root and may be paged or rebuilt only by a
profile-approved recovery procedure. The local exact index grows with received
credit count while public state and proofs remain constant-size.

Cloneable secure-state backup is forbidden because it would create two valid
predecessors. Restore cannot roll the hardware epoch, sequence, replay root, or
monetary state backward. Permanent secure-state loss behaves like lost physical
cash unless an independent online loss policy compensates it outside the
offline balance.

## Qualification

Every profile must pass physical airplane-mode, restart, sudden power loss,
clock rollback, backup/restore, thermal, latency, memory, throughput, offline
rotation, and counter-rollover tests. Fault injection covers every journal,
candidate, hardware commit, proof, state persistence, inbox, outbox, transport,
and acknowledgement boundary.

Adversarial cases include stale predecessors, forked successors, rollback,
counter reuse/skip, forged rotation, overflow, request/payment/proof/certificate
substitution, delayed delivery, duplicate/conflicting credit IDs, and full or
partial redemption. Qualification must demonstrate value conservation,
byte-identical recovery of every exposed payment, and history-independent proof
size and verification work through at least 1,024 real recursive handoffs.
