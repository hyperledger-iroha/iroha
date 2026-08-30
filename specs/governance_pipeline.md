% Governance Pipeline (SORA Parliament V1)

The broader target doctrine, economic model, and mechanism maturity matrix are
defined in [`sora_adversarial_constitution.md`](./sora_adversarial_constitution.md).
This file describes the attempt reducer and native execution boundary that are
present in the source tree. It does not declare the current checkout or a binary
release qualified.

# Canonical proposal attempt lifecycle

1. A typed `ProposalKind` is admitted into governance storage. Its canonical
   fingerprint becomes the immutable `ProposalContentId`; a retry derives a
   new `GovernanceAttemptId` from that content id and the exact next sequence.
2. Core derives the risk tier, required-body order, policy version, exact effect
   preimage hash, and compare-and-set head. A caller cannot supply those fields.
   First-release proposal semantics require both the Monetary Policy Committee
   and Financial Markets Authority for `ValidationFeePolicy` and
   `ValidationFeePayoutLifecycle`: their complete order is Rules, Agenda,
   Interest, Review, Coordination, MPC, FMA, Oversight, Policy Jury. This
   reflects their network-wide fee-schedule and governed treasury-payout
   effects. SCCP route governance retains the same order without MPC; all other
   proposal mappings are unchanged.
3. For each body election, Core freezes the complete canonically ordered
   eligible-citizen snapshot in the containing block. The corresponding
   `SortitionRequestV1` commits that snapshot before the one exact finalized
   threshold-beacon pulse at `request_height +
   parliament_sortition_pulse_delay_blocks` (default 4) for the network's
   stable logical beacon identifier. Core uses checked addition, freezes the
   nonzero delay in the attempt, and rejects both nearer and arbitrarily distant
   pulse heights during live admission and persistence validation.
   The first pulse consumption covers every initially required body in one
   simultaneous batch; a later no-roster retry or a newly required Confirmation
   Jury consumes a fresh pulse slot. `ConsumeSortitionPulseBatch` is a
   permissionless progress trigger, but its relayer cannot choose entropy or
   split the pending set: Core verifies the exact finalized pulse and requires
   the complete strictly ordered request family before deriving assignments.
   `RegisterSortitionRequest` remains manager-gated request intent. If an
   ordinary initial generation includes a hidden-ballot body, or an ordinary
   hidden-body retry is requested, and the live body-specific electorate has
   zero or one member, Core records typed pre-request capacity evidence instead
   of admitting an invalid `SortitionRequestV1`. The exact 0/1-member snapshot,
   request slot, target, and sequence are frozen; no beacon pulse is reserved or
   consumed. A later block may request only the exact next bounded generation,
   and the final failed generation rejects the governance attempt as
   `SortitionRetriesExhausted`. The special atomic Confirmation-capacity result
   described below remains separate.
   Threshold key rotation is independent of that logical request identifier.
   Its exact-roster certificate compare-and-sets the expected active predecessor,
   and a global key change in block `H` takes effect at `H + 1`. Consequently an
   optional Parliament pulse or mandatory NPoS pulse produced from the parent
   state is verified against the key session active at its own height, not the
   successor pointer visible after the block's transactions execute. The
   retired consensus VRF commit/reveal protocol and independent epoch-council
   records are neither entropy sources nor fallbacks.
4. The future pulse deterministically ranks primaries and alternates. Candidates
   accept or decline their own invitations under their transaction authority;
   `BeginInvitationAcceptance` is permissionless and carries only the election
   id; containing-block height and the consensus configuration determine its
   window. After that fixed response window, either permissionless
   `SealBodyRoster` derives the nonempty accepted assignments, roster root, and
   body id, or permissionless `FailBodyElectionNoRoster` proves from the reducer
   and finalized-pulse store that the pulse expired unavailable or the accepted
   roster is empty. Neither trigger accepts a caller-selected window, failure
   reason, assignment list, root, or body id. The
   `RecordAttemptAbsence` lets the same authority declare only its exact seated
   assignment absent. Absence is attempt-local and immutable, does not slash or
   change the original-seat quorum denominator, and must precede that body's
   endorsements or ballot. Once Reflection opens, the same inclusive frozen
   public-finding deadline also gates new absence declarations. If the
   authenticated absence makes the immutable original-seat public-finding
   quorum mathematically unreachable, Core sets that body to `NoResult` and
   rejects the governance attempt.
   The independent epoch-council read is not a roster source or fallback.
   Every member of a frozen candidate snapshot retains its citizenship bond
   while its election is `AwaitingPulse`, `Drawing`, or
   `AcceptingInvitations`. `NoRoster` and superseded elections release unseated
   candidates; sealed body assignments retain their members through the active
   attempt. An active retryable singleton pre-request capacity failure likewise
   retains its one candidate until a later generation supersedes it or final
   exhaustion rejects the attempt. Eligibility therefore cannot be withdrawn
   after request intent but before retry, draw, or roster sealing.
5. For a nonbinding body, `EndorsePublicFinding` lets each nonexcluded seated
   authority endorse exactly one root of the public evidence, deliberation, and
   dissent record. Core automatically finalizes only when one identical root
   reaches `ceil(2 * original_seats / 3)`, then binds the canonical
   strictly ordered `endorsing_assignments` list, its recomputed endorsement
   root, exact endorsement count, and immutable quorum into the body
   certificate. Context-free certificate validation requires the list to be
   nonempty and strictly increasing and requires
   `list.len == endorsements == quorum`. No manager can choose or finalize a
   finding. The Policy Jury, and a fresh disjoint Confirmation Jury when a
   narrowly approved result requires one, must use the mandatory private
   zero-knowledge timed-OVN ballot. A public finding is not a formal ballot and
   cannot replace a required private jury result. Hidden-ballot bodies and their
   eligible candidate snapshots require at least two members. Before a narrow
   Policy result is committed, Core removes every sealed Policy Jury member from
   the current eligible-citizen snapshot. Fewer than two remaining candidates
   terminalize the verified opening as
   `ConfirmationJuryCapacityUnavailable`; the Policy binding and unfillable
   Confirmation requirement are not committed. At the proposal-wide redraw
   ceiling, at least two candidates instead terminalize the same verified
   opening as `RandomnessRedrawBudgetExhausted` before committing either the
   Policy binding or a Confirmation draw. Otherwise, that same finalization
   transaction freezes and registers the exact disjoint snapshot, configured
   target, current request height, and deterministic future pulse slot. The sequence-zero request height must equal the Policy result height,
   and restore rejects a missing or differently timed initial request.
   Eligibility cannot race a separate initial Confirmation request. If
   later invitation responses leave only one accepted hidden-ballot seat, Core
   records an objective insufficient-roster election failure and follows the
   bounded fresh-sortition retry path rather than sealing a cryptographically
   unusable body.
   After each endorsement, Core derives `eligible = original roster -
   authenticated absences` and `remaining = eligible - immutable
   endorsements`. If the strongest existing root plus every remaining seat is
   below quorum, the body becomes `NoResult` and the governance attempt becomes
   `Rejected`; a manager cannot choose a root to break the split. Entry into
   Reflection freezes an inclusive endorsement deadline from the consensus
   `parliament_public_finding_phase_blocks` value (default 3,600). Endorsements
   after it are invalid; once `current_height > deadline`, the payload-minimal
   permissionless `FailPublicFindingNoResult` trigger derives
   `DeadlineExpired`, marks the body `NoResult`, and rejects the attempt.
6. A private ballot attempt freezes one configuration-derived schedule:
   registration close, survivor freeze, masked-ballot commitment close, and
   earliest release height, plus an inclusive opening deadline equal to the
   release height plus `opening_phase_blocks` (default 600). The first three
   transitions are accepted only at their exact heights; release consumption
   and aggregate finalization are rejected after the opening deadline.
   Registration, survivor, and ballot corpora are bounded by the frozen
   per-attempt limit; the default and hard ceiling are 1,000.
7. A registration or dropout is accepted only from the exact seated authority
   named by its canonical timed-OVN record. At close and freeze, Core derives
   the ordered registration corpus, survivor subset, and roots from those
   accepted records; a manager cannot submit replacement registration corpora
   or survivor subsets. The survivor set is immutable before ballots are
   accepted. `FreezeTimedOvnCorpus` is a permissionless exact-next append. Core
   derives the committed survivor offset and checks the active ballot, exact
   phase and containing-height window, body and predecessor bindings, nonempty
   chunk width, canonical record widths, capacity, and every one-hot proof
   before advancing the replay-checkable prefix. A relayer therefore cannot
   forge, omit, overlap, reorder, or alter one member's ballot, and only the
   terminal prefix seals the complete survivor-ordered corpus. Payload-minimal
   close, survivor freeze, release, failure, and finalization triggers remain
   permissionless.
   Before registration close, survivor freeze, or a corpus append, Core checks
   the reducer-owned active ballot, exact phase, body binding, predecessor
   checkpoint, and containing height using only bounded scalar state. Wrong-
   height and replayed checkpoint traffic therefore fails before proof work;
   an exact-height append still verifies every new record. Aggregate
   finalization first verifies the fixed-size public TLE/session/release binding
   and final threshold signature, then verifies the committed public aggregate
   transcript before mutation. Snapshot restore replays the complete raw
   evidence instead of trusting the cache. Core persists no secret shares or
   individual openings. A finalized release pulse and verified threshold-BLS
   signature open only the aggregate Aye/Nay/Abstain tally.
8. If a phase deadline is missed, Core derives the eligible `NoResult` reason
   and evidence commitment from persisted state and the containing block
   height. `ReleasePulseUnavailable` is available only after the committed
   release height, no later than the opening deadline, and when the exact
   network/session/height pulse is absent from authoritative finalized-pulse
   history. `OpeningDeadlineExpired` is available after the immutable opening
   deadline whether the ballot is still awaiting release or is opening. A
   finalized pulse therefore cannot be falsely
   classified as unavailable, and neither release consumption nor a result can
   arrive after the deadline. The transition carries only the ballot id, not a
   failure reason or root. An invalid threshold release or aggregate opening is
   rejected without mutation and is not itself terminal evidence. A retry uses
   the exact next sequence, a fresh ballot id, and a fresh TLE session.
   `NoResult` on the final permitted sequence rejects the governance attempt
   instead of leaving it active without a legal retry. There is no plaintext,
   manual-opening, public-ballot, or post-freeze recovery fallback.
   Committed audit events classify sortition retry exhaustion, both
   public-finding outcomes, the five phase/release private-ballot failures, and
   insufficient fresh Confirmation capacity or proposal-wide redraw exhaustion
   with the closed ten-variant
   `ParliamentNoResultKindV1`; callers cannot supply that
   classification. `SortitionRetriesExhausted` is emitted when the final
   permitted body-election sequence fails before a body instance exists.
9. Core automatically constructs one `GovernanceCertificateV1` when the final
   required result is accepted, from the exact
   persisted body, sortition, roster, authority-endorsed public-finding,
   private-ballot, TLE, release, policy, effect, and expected-head bindings. The
   native boundary requires
   `enact_at_height = certified_at_height + gov.min_enactment_delay`.
10. At that exact due height Core's automatic block-start step re-derives the
    governed subject head. A mismatch atomically records `Superseded` without
    applying the effect. A match applies the typed proposal effect in a
    rollback-isolated state transaction and records `Enacted`. On an effect
    error Core drops that transaction, then uses a fresh transaction to record
    `ExecutionFailed` and the deterministic failure root derived from the exact
    retained certificate and due height. None of certificate construction,
    enactment, supersession, or execution failure is a public lifecycle
    transition. Core emits the terminal result as a separate canonical
    `ParliamentAutomaticExecutionOutcomeV1` audit payload with a
    domain-separated digest; that payload is not submit-able.

An emergency contract hold is deliberately sticky after its exclusive expiry:
expiry restores execution but does not erase the incident record or authorize a
second hold. The append-only `ContractLifecycleGovernanceActionV1` variant
`CompleteEmergencyHoldRetrospective` (Norito index 5) is the sole clear path.
Its proposal binds the retained hold's proposal-content id, governance-attempt
id, and incident digest, plus a non-zero retrospective finding root. A bonded
citizen may submit that proposal only once the exclusive expiry height has been
reached. Automatic certificate enactment repeats every binding and expiry
check against the compare-and-set lifecycle head, clears only the matching
hold, advances the lifecycle revision, and emits the prior hold, finding root,
revision, and complete post-state. A zero finding, an early request, any
substituted hold coordinate, a missing hold, or a replay fails closed. No direct
instruction, timer sweep, owner shortcut, or expired-record fallback can clear
the hold; a later independent emergency hold becomes possible only after the
certified retrospective is committed.

Validation-fee policy and payout-lifecycle proposals additionally bind their
canonical `proposal_operator` into the proposal fingerprint. Their protected
registry authorization retains the complete Parliament certificate and its
canonical `GovernanceCertificateId`; a standalone referendum tally is not a
validation-fee authorization. Both proposal kinds follow the same attempt,
certification, exact-due-height enactment, and rollback-isolated terminal
lifecycle above. The verified protected-registry projection binds the
certification and enactment heights and requires
`effective_from_height = enacted_at_height + 120,960`; it is not activated by a
client finalization call or a public ballot.

# Cryptographic claim boundary

Timed OVN provides aggregate-only opening under the implemented transcript and
threshold-release checks. It does not, by itself, establish voter anonymity
against network metadata, receipt freeness, coercion resistance, endpoint
security, or side-channel resistance. In particular, modern coercion analyses
show that blockchains, delay encryption, privacy-preserving contracts, and
trusted hardware can strengthen coercers or vote sellers under threat models
that older definitions omit. No documentation or UI may label this protocol
receipt-free or coercion-resistant without a separate construction and proof.
Michalas's July 2026 SACMAT construction obtains coercion resistance through a
specific anamorphic-encryption voting design. Timed OVN neither implements nor
analyzes that construction, so its publication does not support a coercion-
resistance claim for Parliament.
The August 2026 `somewhat deniable voting` construction instead assumes a
trusted teller and deliberately trades away part of individual verifiability
to obtain its stated deniability boundary. The 2026 journal version of Yin et
al.'s scalable blockchain construction likewise proves its claims for a
different dummy-voting and liquid-democracy protocol. Timed OVN implements
neither construction nor threat model, so those publications strengthen the
requirement for a protocol-specific proof rather than extending their claims to
Parliament.

“Aggregate-only” is not “winner-only” and does not make participation
unlinkable. V1 publishes the exact Aye/Nay/Abstain counts and the accepted
corpus size, while the per-ballot participant hash is deterministically derived
from the public account and ballot attempt. Small panels and auxiliary knowledge
can therefore reveal individual choices. Until a separately reviewed proof
reveals only quorum, outcome, and the narrow-result predicate, release material
must describe V1 as ballot-value confidentiality with an exact public tally and
linkable participation, not as anonymous voting. Winner-only and cast-or-audit
constructions published in 2026 are research inputs rather than compatible
replacements for the current certificate and proof statement.

The threshold-release profile implements the three-polynomial Das--Ren design
with a proof on every non-key-unique partial. V1 fixes `n = 3f + 1`, threshold
`f + 1`, and at most `f` distinct signing-share exposures over an unrefreshed
key session. It has no proactive refresh; a cumulative exposure beyond that
budget requires a fresh DKG and purpose-distinct session. Zeroizing Rust buffers
are defense in depth, not a compiler, OS, HSM, or hardware erasure guarantee.
The cited Das--Ren result is in the random-oracle model under DDH and co-CDH;
code conformance and replay tests are not a proof that an implementation meets
that theorem.
The ePrint 2025/943 key-uniqueness impossibility result does not directly cover
this non-key-unique profile, but it makes the per-partial representation proof
and a precise corruption model mandatory. “Adaptive” in a type name is not a
generic standard-assumption security claim.

The chain cannot observe a share compromise. V1 therefore persists separate
consensus lifecycle metadata beside (and outside) the cryptographic transcript:
an activation height, immutable expiry height, rotation-shortened inclusive
selection deadline, committed fresh-ballot counter, and immutable use ceiling.
An install or rotation committed at `H` leaves the predecessor selectable
through `H` and makes the successor selectable at `H + 1`. Fresh ballot
registration fails closed before activation, after expiry/cutover, and at the
use ceiling; restart recounts committed ballot bindings and rejects mismatched
counters or session/roster/lifecycle bindings. Already committed ballots retain
their historical public session and custody requirement through their own
inclusive opening deadline. These bounds limit exposure but cannot detect a
compromise; proactive or silent refresh remains a separately specified protocol
revision with explicit secure-erasure assumptions.

RFC 9380 standardizes the hash-to-curve building block used by the fixed
domain separators. It does not standardize the Das--Ren threshold composition,
its corruption model, or the complete timed-OVN release protocol; the BLS suite
label in source remains explicitly draft-derived. The July 2026 CFRG BLS
document is still an Internet-Draft and specifies base BLS signatures and
aggregation, not this threshold protocol or its lifecycle.

NIST IR 8214C's January 2026 Threshold Call asks submitters for a technical
specification, reference implementation, and experimental report, followed by
public analysis and a possible characterization report. It is an evidence-
gathering process, not a standardization or approval of Parliament's Das--Ren
profile. The January 2026 BBDL tBLS item is explicitly a version-0.1 preview of a
planned later package, whose team and technical scope may still change; it is
not a completed NIST submission, standard, validation, or approval of this
different threshold-release profile. The MPTS 2026 workshop likewise records
previews and current research on BLS security, adaptive and proactive
corruption, post-quantum threshold schemes, and threshold ZK; a workshop
preview is not a conformance or security certificate.

The 13 August 2026 Berkeley report on practical witness encryption says that
general-NP constructions remain prohibitively expensive and rest on strong,
comparatively lightly scrutinized assumptions, while its practical results are
special-purpose pairing constructions. Its silent and batched threshold-
encryption designs are research alternatives, not drop-in replacements for the
implemented timed-OVN transcript, release identity, corruption model, or
consensus lifecycle. Adopting one would require a separately specified,
reviewed, and enacted protocol change.

Recent beacon work also prevents a broader claim than the implementation makes.
A VDF establishes construction-specific sequential work, not a fixed amount of
civil time or economic resistance to specialized hardware. A VRF proves one
key's evaluation, not resistance to key grinding, selective withholding,
proposer choice, forks, or bias accumulated when one epoch feeds another.
Parliament instead freezes one network/session/height pulse slot before drawing,
rejects an unavailable classification once the authoritative slot exists, and
bounds retries. Release qualification must still exercise selective withholding
and repeated-retry bias; single-round uniformity is not sufficient evidence.
Independent per-stage retry caps likewise do not bound the conditional advantage
of nested governance, roster, and fresh-ballot redraws. A release candidate must
account for fresh entropy consumption with one proposal-level budget, keep
idempotent transport retries outside that budget, and quantify the resulting
capture bound under selective aborts.

Ballot presentation is a separate governance-security boundary. A July 2026
observational DAO study reports associations between voting-power share and an
author's selected choice, approval-oriented wording, and first-list position;
the authors explicitly do not claim that those associations establish
causation. Parliament removes caller-selected body order, derives all initially
required bodies in one canonical simultaneous batch, and uses a fixed binary
body ballot, so a proposal author cannot choose the body sequence or reorder
ballot options. Those protocol rules do not eliminate interface framing,
author cues, vote-visibility effects, or client rendering defects. Release
qualification must therefore verify canonical rendering across clients and
must not describe deterministic ordering or private opening as proof that
human presentation bias is absent.

Likewise, a replicated ledger is only the bulletin board. It does not by itself
prove cast-as-intended, recorded-as-cast, tallied-as-recorded, client integrity,
or ballot privacy. The V1 transcript therefore binds the exact proposal,
attempt, body, participant, survivor corpus, release identity, option, and proof
domain; every partial and aggregate is independently verified before use. This
does not claim an ElectionGuard-compatible voter-verification ceremony or close
the endpoint and coercion boundaries above.

Nor does an `AccountId` establish one-human-one-vote. Unless a separately
governed uniqueness-assurance profile is bound into the eligibility snapshot
and certificate, the accurate claim is equal weight per eligible account or
pseudonym. The current formal model treats cryptographic verification as a
trusted input and is not a composed proof of eligibility, beacon bias,
adaptive corruption, abort/retry behavior, ballot secrecy, finality, and
enactment.

The BLS12-381 threshold release, pairing-based timed-OVN ballot, and classical
beacon are not post-quantum. Versioned sessions and domain-separated algorithm
identities provide a migration boundary, but using ML-DSA elsewhere in Iroha
does not make Parliament post-quantum. Replacing these primitives requires a
separately specified, reviewed, consensus-enacted protocol revision and new
fixtures; current lattice DKG/beacon proposals are research inputs, not
standards or drop-in implementations.

Research boundary reviewed through 2026-08-30:

- Das and Ren, [*Adaptively Secure BLS Threshold Signatures from DDH and
  co-CDH*](https://eprint.iacr.org/2023/1553).
- Ciampi, Crites, Komlo, and Maller, [*On the Adaptive Security of Key-Unique
  Threshold Signatures*](https://eprint.iacr.org/2025/943).
- Finogina, Herranz, and Rønne, [*Expanding the Toolbox: Coercion and
  Vote-Selling at Vote-Casting Revisited*](https://eprint.iacr.org/2024/1167).
- Michalas, [*Coercion-Resistant Voting via Anamorphic
  Encryption*](https://doi.org/10.1145/3750555.3811888), ACM SACMAT 2026,
  published 8 July 2026.
- Jia, Shi, Ye, Huang, and Peng, [*Somewhat Deniable Voting:
  Coercion-Resistant Electronic Voting Scheme with Privacy Preservation
  Property*](https://doi.org/10.32604/cmc.2026.084123), *Computers, Materials
  & Continua* 89(1), published 13 August 2026. Its trusted-teller and reduced
  individual-verifiability boundary is not the Timed OVN threat model.
- Yin, Zhang, Nastenko, Oliynykov, and Ren, [*A Scalable Coercion-Resistant
  Voting Scheme for Blockchain Decision-Making*](https://doi.org/10.1109/TDSC.2026.3651473),
  *IEEE Transactions on Dependable and Secure Computing*, 2026. Its
  construction and proof do not apply to Timed OVN without implementing and
  analyzing that protocol.
- IRTF, [RFC 9380: Hashing to Elliptic
  Curves](https://www.rfc-editor.org/rfc/rfc9380).
- CFRG, [*BLS Signatures*, draft-irtf-cfrg-bls-signature-07
  (work in progress, 6 July
  2026)](https://datatracker.ietf.org/doc/draft-irtf-cfrg-bls-signature/07/).
- NIST, [*NIST First Call for Multi-Party Threshold Schemes*, NIST IR
  8214C](https://doi.org/10.6028/NIST.IR.8214C), January 2026.
- Bacho, Boldyreva, Das, and Loss, [*tBLS: Threshold BLS Signature Scheme,
  Preview Writeup version 0.1*](https://csrc.nist.gov/csrc/media/Projects/threshold-cryptography/documents/TCall-1/BBDL-tBLS-PW01.pdf),
  19 January 2026.
- NIST, [*MPTS 2026: NIST Workshop on Multi-Party Threshold Schemes
  2026*](https://csrc.nist.gov/Events/2026/mpts2026), January 2026.
- Policharla, [*Practical Witness Encryption Schemes and
  Applications*](https://www2.eecs.berkeley.edu/Pubs/TechRpts/2026/EECS-2026-243.html),
  UCB/EECS-2026-243, 13 August 2026.
- Glaeser, Seres, Zhu, and Bonneau,
  [*Cicada: A Framework for Private Non-Interactive On-Chain Auctions and
  Voting*](https://eprint.iacr.org/2023/1473).
- Shang and Chen, [*Economic Security of VDF-Based Randomness Beacons: Models,
  Thresholds, and Design Guidelines*](https://arxiv.org/abs/2604.04744), 6
  April 2026.
- Gaži, Quader, and Russell, [*Taming Iterative Grinding Attacks on
  Blockchain Beacons*](https://eprint.iacr.org/2025/1974), ASIACRYPT 2025.
- [*SoK: Distributed Randomness Beacons*](https://eprint.iacr.org/2023/728),
  IEEE Symposium on Security and Privacy 2023.
- [*Enforcing Winner-Only Disclosure: Verifiable Tally Hiding for Weighted DAO
  Governance*](https://eprint.iacr.org/2026/1773),
  revised 26 August 2026. Its honest-trustee assumptions do not establish the
  malicious sub-threshold privacy required here.
- [*PQKryvos: Post-Quantum Secure E-Voting With Flexible Ballot Formats and
  Public Tally-Hiding*](https://eprint.iacr.org/2026/1004), PoPETs 2026.
- [*Audit-or-Cast: Enforcing Honest Elections with Privacy-Preserving Public
  Verification*](https://arxiv.org/abs/2604.18163), revised 21 April 2026.
- [*Threshold Receipt-Free Voting with Server-Side Vote
  Validation*](https://eprint.iacr.org/2025/1321),
  E-Vote-ID 2025.
- [*FiltrumVote: Scalable, Verifiable, and Coercion-Resistant Internet
  Voting*](https://eprint.iacr.org/2026/1435), July 2026.
- [*On the Necessity of Pre-agreed Secrets for Thwarting Last-minute Coercion:
  Vulnerabilities and Lessons From the Loki E-voting
  Protocol*](https://arxiv.org/abs/2604.00188),
  CSF 2026 extended version.
- [*Proactive Refresh for Accountable Threshold Signatures*](https://eprint.iacr.org/2022/1656).
- [*Quadratic Asynchronous DKG from Plain Setup*](https://eprint.iacr.org/2026/1159),
  June 2026.
- [*Anchor-DKG: Distributed Key Generation with Repeating
  Parties*](https://eprint.iacr.org/2026/1570), CCS 2026.
- [*Practical Silent Threshold Signatures and Silent Threshold Encryption for
  Dynamic Committees*](https://eprint.iacr.org/2026/1820),
  CCS 2026.
- [*Beyond Blockchain Ballots: UC-Secure Layer-2 Voting and
  Governance*](https://eprint.iacr.org/2026/1521), CSF 2026.
- [*Proof-of-Uniqueness: Sybil-Resistant Privacy-Preserving Decentralized
  Identity through Threshold-OPRF and zk-SNARK
  Registry*](https://eprint.iacr.org/2026/1725), August 2026.
- Balietti, Saggese, and Strohmaier, [*Voting Biases in Decentralized
  Autonomous Organization (DAO) Governance*](https://arxiv.org/abs/2607.09435),
  10 July 2026.
- Microsoft Research, [*ElectionGuard Specification
  2.1*](https://electionguard.vote/spec/).
- Cortier, Debant, and Gaudry, [*Breaking Verifiability and Vote
  Privacy in CHVote*](https://eprint.iacr.org/2025/080), ESORICS 2025.
- NIST, [*Considerations for Achieving Crypto Agility: Strategies and
  Practices*, CSWP
  39upd1](https://doi.org/10.6028/NIST.CSWP.39-upd1), 29 June 2026.

As of 30 August 2026, the NIST Threshold Call remains in its three-round
preview phase; package submissions are expected in November 2026. The BBDL
tBLS document above is a preview writeup, not a completed package, NIST
standard, or approval of Parliament's construction.

# Standalone referendum boundary

The repository still contains a standalone referendum subsystem with public
PLAIN and proof-backed ZK ballot routes, conviction locks, and tally reads.
Those routes are not Parliament body ballots, cannot stand in for a timed-OVN
jury result, and are not inputs to `GovernanceCertificateV1`. Independent epoch
council records likewise remain readable but are not consulted by the attempt
reducer. The first-release public contract contains no proposal approval
snapshot, equal Parliament stage ballot, caller-selected referendum window or
mode, client finalization, or client enactment path.

# Outstanding release gates

- Re-run the already-green focused data-model/Core/Torii and source/model gates
  from one clean immutable candidate, then pass workspace tests, strict Clippy,
  formatting, and the remaining release matrix with archived provenance.
- Qualify the implemented live threshold-beacon partial-share transport,
  per-session runtime custody, threshold aggregation, candidate-effect
  assembly, and authoritative finalized-pulse persistence on at least four
  peers, including missing/invalid shares, restart, idempotent retransmission,
  mandatory NPoS boundary slots, optional Parliament demand slots, and key
  rotation.
- Qualify canonical carrier publication and retirement on every nonproducer
  follower. Cover an `author = false` live follower retiring a losing carrier
  from the exact FIFO-only/no-Queue-owner state, plus strict cold-start replay
  at the all-`ReleasePending` and partial-`Released` cuts. Require unchanged
  Queue/FIFO journal bytes, no fabricated Queue owner, complete Kura/Queue
  terminal cleanup, a still-live follower runner, and fail-before-mutation
  rejection of missing or misordered FIFO evidence.
- Qualify the implemented authenticated release-context read, bodyless local
  partial request, independently verifying multi-session custody/coordinator,
  canonical combine, ordinary `FinalizeOpenedBallot` submission tooling, and
  the bounded public broker projection/projected-signer validation boundary
  against a genuine authenticated broker/HSM provider. The public projection is
  not evidence of committed-state origin. Qualify the implemented consensus
  active-session cutover, immutable per-session ordered-roster persistence, and
  the custody rule that forbids retirement while a session remains selectable
  or any committed ballot deadline references it. Startup now scans the active
  session and every deadline-retained historical session, derives the local seat
  from that session's frozen roster, and requires the same runtime signer to
  return an exact non-signing key-session/transcript/seat capability
  attestation. The external broker path requalifies around that lookup and
  poisons substituted results. This is point-in-time readiness evidence, not a
  proof of future availability, HSM provenance, or erasure. Demonstrate the
  complete behavior with daemon-scoped broker admission,
  old-share retention/zeroization,
  restart recovery, peer authentication/rate limits, and threshold collection
  on at least four peers. The source seam is not yet an operationally automatic
  release service and does not prove secure erasure or HSM provisioning.
- Qualify the Core-authorized pre-seal timed-OVN casting context and its
  four-mebibyte, header-framed canonical Norito archive. The archive validator
  replays the public TLE transcript, exact timed-OVN session, registration
  proofs, and (after survivor freeze) the prepared survivor/release statement.
  Core admits the read only inside the exact half-open phase window and rejects
  malformed/nonmonotone reducer schedules before proof replay. The V1 archive
  deliberately omits those deadlines: it proves a point-in-time snapshot, can
  age after retrieval, and is public data rather than a ledger authorization.
  Qualify the source-implemented native secret-local registration/ballot C ABI,
  complete platform-keystore wrappers and archive refresh, and prove that no
  seed, registration secret, dropout set, masked ballot, share, or opening is
  returned by the read surface.
- The feature-isolated four-validator target contains a corridor for two
  independently validated global-beacon DKG transcripts. It installs the
  predecessor, applies
  a `2f + 1` compare-and-set rotation in an epoch-boundary block, verifies that
  the same block's pre-boundary pulse still uses the parent session, and verifies
  that the next pre-boundary pulse and epoch seed use the activated successor.
  The same corridor covers proof-valid timed release, exact-height enactment,
  and normal restart/restore. The target also contains stale-head supersession
  and rollback-isolated execution-failure corridors; all require fresh
  same-source four-validator evidence before promotion.
- The four-validator public-finding target contains authority-bound self-absence,
  early impossible-quorum `NoResult`, a fresh governance retry, immutable
  competing roots, post-deadline endorsement rejection, permissionless
  `PublicFindingDeadlineExpired`, a second retry, four-peer state equality, and
  normal validator restore. Progress still assumes an eligible transaction
  eventually submits the deterministic deadline trigger.
- Candidate-qualify the implemented Torii, MCP, CLI, OpenAPI,
  Rust/JavaScript/Kotlin/Java/Swift SDK, and shared-fixture coverage for the
  typed attempt and certificate surface. Regenerate signed OpenAPI provenance,
  execute candidate-native SDK artifacts, verify canonical ballot rendering
  across clients, and keep the already-retired equal Parliament ballot and
  proposal-backed finalize/enact surfaces absent.
- Candidate-qualify the implemented aggregate-only transition/failure counters,
  committed status/stage gauges, and reviewed stuck-attempt/deadline alarms
  across restart and four-peer execution. Keep identifiers, roots, registrations,
  ballots, shares, individual openings, and account labels out of metrics.
- Candidate-qualify the existing focused automatic-enactment, stale-head
  supersession, and rollback-isolated `ExecutionFailed` coverage on four peers,
  including restart validation and rejection of every signed terminal-outcome
  draft.
- Obtain an independent review of the exact timed-OVN arithmetic,
  Fiat--Shamir statements, constant-time/side-channel boundary, threshold-BLS
  corruption assumptions, implementation, build artifacts, and target matrix.
  The official publication manifest validator exists, but no external audit
  report or evidence archive is embedded or claimed by this repository.
- Run the bounded model as counterexample search, exhaustively check the
  configured state space with pinned TLC 2.19, and archive both same-source
  outputs. These are complementary evidence, not replacements for proof review,
  cryptographic test vectors, implementation tests, or multi-peer execution.
- Complete the candidate-native ABI-23 Swift and Android replay, capacity/rekey/
  validation-fee restore scenarios, same-source benchmark archive, strict TLAPS
  and pinned-Verus gates, chaos/soak qualification, and external release signing.
