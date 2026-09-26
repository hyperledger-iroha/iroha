# F11: zero-slot decentralized-sum candidate rejection

This is a candidate-specific design audit on the existing `optimizations`
checkout, 2026-09-24. It selects no production protocol, parameter set,
verifying key, circuit, or compatibility route. The
[standalone election contract](../../../specs/standalone_election_protocol_contract.md),
[fault matrix](standalone-election-dropout-fault-matrix.md),
[functional-opening review](standalone-election-primary-source-functional-opening-review.md),
[NARAD subset audit](standalone-election-narad-subset-opening-review.md), and
[closed-corpus audit](standalone-election-final-corpus-opening-audit.md)
define the requirements being tested. This candidate fails a passive privacy
test before an implementation or resource benchmark would be justified.

## Concrete candidate and declared fault hypothesis

Try one independent instance per option of the **one-time decentralized sum**
(ODSUM) construction of [Nguyen, Phan and Pointcheval, Sections 3.2 and
6.2](https://eprint.iacr.org/2023/268.pdf). Its source explicitly describes
one encryption per sender with no label, an easy-discrete-log subgroup, and
public combination of a complete sender vector without a decryption key.
The proposed election adaptation adds an enrolled voter's encrypted zero
slot, then replaces that slot with a cast or latest conviction update. This
adaptation is **outside** the source construction's one-time security claim.

For a concrete stress shape, let the illustrative cohort contain at most
`N = 256` enrolled credential nullifiers, `K = 2..=64` choices, at most
`U = 1` conviction update per caster, and at most `A = 512` accepted
casts/updates. Allow up to `d = 256` voters to stop permanently after any
finalized message and up to `c = 85` adaptively corrupt credentials; only a
corrupted voter's own disclosed input is excluded from honest-ballot privacy.
Assume the existing four-validator finality/availability boundary with at
most one Byzantine validator, a permissionless worker with no secret state,
at least one such worker available after close, and a hypothetical one-hour
close-to-result deadline under a post-synchrony network delay of at most five
seconds. These are candidate hypotheses, not qualified deployment bounds.
Relayers only carry transactions and fees. No worker holds a decryption key,
committee share, master key, or voter secret.

The candidate's transcript would be:

1. Freeze network, election, eligibility root, asset incarnation and scale,
   conviction formula, option count, arithmetic bounds, and deadlines. An
   anonymous credential proof registers a stable election nullifier and one
   ODSUM public key per option. The key roster must be frozen before its
   pairwise masks can be computed.
2. Each enrolled slot posts a zero ciphertext for every option with a proof
   of correct masking. A setup-only noncaster remains zero. Cast acceptance
   replaces zero with a ciphertext encoding exactly the frozen smallest-unit
   conviction weight in one hidden option. A conviction update attaches a
   proof of the same choice, the exact prior accepted state, confidential
   bond conservation, nondecreasing bond and expiry, and the new exact weight.
   Every ciphertext and proof is retained in finalized history; acceptance
   cannot depend on a later message by its voter.
3. Close derives the unique ordered history `H`, its latest slot state
   `L(H)`, count/root, and finality certificate. A public worker multiplies
   exactly one latest ciphertext per enrolled slot and option, solves the
   resulting subgroup element, and submits only `T(H)` and a proof bound to
   the certified corpus. Duplicate finish is idempotent only for the same
   result and certificate.

The intended tally statement would verify the frozen context and finality
certificate, replay all accepted credential/bond/choice-preservation proofs
in consensus order, derive `L(H)` itself, enforce no field or `u128`
aggregate wrap, and for each option `k` prove
`product_{i in roster} C_latest(i,k) = f^(T(H)[k])` with
`T(H)[k] = sum_{i in cast latest slots} weight_i * 1[choice_i = k]`.
The worker's witness should be the durable public history and its accepted
proofs, never a dropped voter's opening. This would address exact-corpus
**soundness if implemented and proven**, but it does not restrict what a
public observer can compute from the same durable bytes.

Public transcript material includes the policy/eligibility commitment,
anonymous election nullifiers, per-option public keys and zero slots,
immutable cast/update ciphertexts and proofs, bond commitments and accepted
operation order, close certificate/root, and the final `Vec<u128>` totals.
The private credential, choice, bond opening, and per-option ODSUM secret
exponents remain with each voter. Privacy must compare all of this traffic
in equal-final-total worlds, not just the published final proof.

## Decisive same-mask distinguisher

Let `T_{i,k} = g^(t_{i,k})` and let the pairwise ODSUM mask for slot `i`,
option `k`, be
`M_{i,k} = (product_{j>i} T_{j,k} / product_{j<i} T_{j,k})^(t_{i,k})`.
The masks cancel when all enrolled slots are multiplied. Using independent
`t_{i,k}` across options gives this adaptation the more favorable case;
reusing one mask across options would leak even within one cast.

The zero slot is `Z_{i,k} = M_{i,k}`. A cast of positive weight `w` and
hidden choice `b` publishes
`C_{i,k} = M_{i,k} * f^(w * 1[b = k])`.
For every option, any observer of the immutable history computes

`C_{i,k} / Z_{i,k} = f^(w * 1[b = k])`.

The right side is the identity precisely for the unchosen options when
`0 < w < ord(f)`. Thus the observer learns `b` **without** a discrete
logarithm or a tally operation. If an accepted update raises the weight from
`w` to `w' > w` while preserving the choice, it also publishes
`C'_{i,k} / C_{i,k} = f^((w' - w) * 1[b = k])`; the update alone reveals the
same choice. A valid update whose integer-rounded weight does not change
does not save the candidate, because a positive initial cast already leaks.

For a direct equal-leakage comparison, give two honest slots A and B weight
one, with A choosing option 0 and B option 1 in world 0 and the choices
swapped in world 1. Both final totals are `(1, 1)`, with identical accepted
message shapes, timing and public policy. The quotient for A's option 0 is
nonidentity only in world 0. This violates aggregate-only privacy even with
no corrupted voter or worker. The same example can use A's one update
`1 -> 2` and B's weight 2; both final totals are `(2, 2)`, while A's update
quotient distinguishes its choice.

The paper's optional all-or-nothing encapsulation addresses **incomplete**
same-label ciphertext vectors, not two complete public versions of a slot or
a publicly retained zero baseline. It does not invalidate these ratios.
An on-chain rule that verifies only one final corpus likewise cannot prevent
an observer from doing group division on earlier public ciphertexts. This is
a rejection of the specified zero-slot adaptation, not a claim that ODSUM's
one-time theorem is false or that all closed-corpus functional openings are
impossible. It is a different failure from [NARAD's privileged subset-opening
pair](standalone-election-narad-subset-opening-review.md), derived from
[its protocol equations](https://arxiv.org/html/2607.07596v1): this attack
needs only a public transcript.

## Additional completion and resource gaps

The full public-key roster must precede computation of each pairwise zero
mask. A participant can publish its key and then disappear before publishing
the zero ciphertext; with its key still in the roster, the remaining masks
cannot form the full-vector sum. Removing that slot changes all other masks
and requires fresh messages from participants who may already have dropped.
The candidate therefore also lacks the stipulated after-each-message setup
dropout proof. Rotating a voter's mask at update would avoid this particular
ratio, but no compatible noninteractive compensation for all already-dropped
slots or proof against alternate complete-corpus openings is specified.

At `N = 256`, `A = 512`, `K = 64`, independent per-option keys, zero slots,
and accepted ciphertexts require `(N + N + A) * K = 65,536` group elements.
Even an optimistic **32 bytes per element** is 2,097,152 retained bytes
before proofs, bond/credential state, Norito framing, indexes, snapshots,
replication, and recovery. A single 64-option update has at least 2,048 bare
element bytes under that artificial encoding assumption. Final combination
does at least `N * K = 16,384` group contributions, plus subgroup solves and
verification of up to 256 setup and 512 cast/update proofs. Actual class-group
encodings, security parameters, total `u128` limits, proving/verifying cost,
resident/spool/I/O use, and one-hour completion were not measured. The
illustrative `N/A/U` limits are not current production configuration or an
approved restriction of the existing election contract.
With 256 possible positive `u128` weights, a `Vec<u128>` result additionally
needs an explicit checked aggregate cap (or a governed per-ballot cap) before
acceptance, and the subgroup order must exceed the maximum permitted total;
no audited class-group parameter or such policy is selected here.

The existing `FinalizeElection.tally` is `Vec<u128>`, but the current
[`CreateElection`/`SubmitBallot` shapes](../../../crates/iroha_data_model/src/isi/zk.rs)
do not carry this transcript or confidential bond relation. Core's
[`ensure_qualified_standalone_zk_relation_v1` guard](../../../crates/iroha_core/src/smartcontracts/isi/world.rs)
still rejects standalone private ballot and tally execution before proof
verification and accepted-state mutation. No code was changed and no Cargo
test was run for this research note.

**Decision:** reject this candidate. The next research obligation remains a
concrete, publicly verifiable **one-finalized-corpus-only** opening whose
entire accepted-voter recovery material survives immediate dropout, while
no public old/new ciphertext relation, worker key, or alternate history can
reveal an individual choice or second tally. It needs a formal equal-output
privacy game, exact fault thresholds, sound credential/bond/update and tally
relations, audited parameters, measured maximum resources, and independent
review before the production guard can change.
