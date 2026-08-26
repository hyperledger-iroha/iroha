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
   Jury consumes a fresh pulse slot. Threshold key rotation is independent of
   that logical request identifier. Its exact-roster certificate
   compare-and-sets the expected active predecessor, and a global key change in
   block `H` takes effect at `H + 1`. Consequently an optional Parliament pulse
   or mandatory NPoS pulse produced from the parent state is verified against
   the key session active at its own height, not the successor pointer visible
   after the block's transactions execute.
4. The future pulse deterministically ranks primaries and alternates. Candidates
   accept or decline their own invitations under their transaction authority;
   after the fixed response window, Core derives and seals the roster. The
   `RecordAttemptAbsence` lets the same authority declare only its exact seated
   assignment absent. Absence is attempt-local and immutable, does not slash or
   change the original-seat quorum denominator, and must precede that body's
   endorsements or ballot. Once Reflection opens, the same inclusive frozen
   public-finding deadline also gates new absence declarations. If the
   authenticated absence makes the immutable original-seat public-finding
   quorum mathematically unreachable, Core sets that body to `NoResult` and
   rejects the governance attempt.
   The independent epoch-council read is not a roster source or fallback.
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
   narrowly approved result requires one, use the private timed-OVN ballot. A
   public finding is not a formal ballot and cannot replace a required private
   jury result.
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
   accepted. The complete survivor-ordered masked-ballot batch remains a
   manager-authorized freeze payload because verifying as many as 1,000
   attacker-controlled one-hot proofs is not a permissionless checkpoint. Core
   performs that authorization before cryptographic work, then requires one
   proof-valid record for every frozen survivor, so the manager cannot forge,
   omit, reorder, or alter one member's ballot. Payload-minimal close, survivor
   freeze, release, failure, and finalization triggers remain permissionless.
   Before close or freeze replays a registration or ballot corpus, Core checks
   the reducer-owned active ballot, exact phase, body binding, predecessor
   checkpoint, and containing height using only bounded scalar state. Wrong-
   height and replayed checkpoint traffic therefore fails before proof work;
   an exact-height transition still performs the complete replay. Aggregate
   finalization similarly verifies the fixed-size public TLE/session/release
   binding and final threshold signature before either corpus is replayed, then
   retains the full replay and second signature verification before mutation.
   Core replays the public aggregate transcript and persists no secret shares or
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
   public-finding outcomes, and the five private-ballot failures with the closed
   eight-variant `ParliamentNoResultKindV1`; callers cannot supply that
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

Validation-fee policy and payout-lifecycle proposals additionally bind their
canonical `proposal_operator` into the proposal fingerprint. Their protected
registry authorization retains the complete Parliament certificate and its
canonical `GovernanceCertificateId`; a standalone referendum tally is not a
validation-fee authorization.

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
profile. The MPTS 2026 workshop likewise records previews and current research
on BLS security, adaptive and proactive corruption, post-quantum threshold
schemes, and threshold ZK; a workshop preview is not a conformance or security
certificate.

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

Likewise, a replicated ledger is only the bulletin board. It does not by itself
prove cast-as-intended, recorded-as-cast, tallied-as-recorded, client integrity,
or ballot privacy. The V1 transcript therefore binds the exact proposal,
attempt, body, participant, survivor corpus, release identity, option, and proof
domain; every partial and aggregate is independently verified before use. This
does not claim an ElectionGuard-compatible voter-verification ceremony or close
the endpoint and coercion boundaries above.

The BLS12-381 threshold release, pairing-based timed-OVN ballot, and classical
beacon are not post-quantum. Versioned sessions and domain-separated algorithm
identities provide a migration boundary, but using ML-DSA elsewhere in Iroha
does not make Parliament post-quantum. Replacing these primitives requires a
separately specified, reviewed, consensus-enacted protocol revision and new
fixtures; current lattice DKG/beacon proposals are research inputs, not
standards or drop-in implementations.

Research boundary reviewed through 2026-08-27:

- Das and Ren, [*Adaptively Secure BLS Threshold Signatures from DDH and
  co-CDH*](https://eprint.iacr.org/2023/1553).
- Ciampi, Crites, Komlo, and Maller, [*On the Adaptive Security of Threshold
  Signatures*](https://eprint.iacr.org/2025/943).
- Rønne, Finogina, and Herranz, [*Expanding the Toolbox: Coercion and
  Vote-Selling at Vote-Casting Revisited*](https://eprint.iacr.org/2024/1167).
- Michalas, [*Coercion-Resistant Voting via Anamorphic
  Encryption*](https://doi.org/10.1145/3750555.3811888), ACM SACMAT 2026,
  published 8 July 2026.
- IRTF, [RFC 9380: Hashing to Elliptic
  Curves](https://www.rfc-editor.org/rfc/rfc9380).
- CFRG, [*BLS Signatures*, draft-irtf-cfrg-bls-signature-07
  (work in progress, 6 July
  2026)](https://datatracker.ietf.org/doc/draft-irtf-cfrg-bls-signature/07/).
- NIST, [*NIST First Call for Multi-Party Threshold Schemes*, NIST IR
  8214C](https://doi.org/10.6028/NIST.IR.8214C), January 2026.
- NIST, [*MPTS 2026: NIST Workshop on Multi-Party Threshold Schemes
  2026*](https://csrc.nist.gov/Events/2026/mpts2026), January 2026.
- Policharla, [*Practical Witness Encryption Schemes and
  Applications*](https://www2.eecs.berkeley.edu/Pubs/TechRpts/2026/EECS-2026-243.html),
  UCB/EECS-2026-243, 13 August 2026.
- Glaeser, Seres, Zhu, and Bonneau,
  [*Cicada: A Framework for Private Non-Interactive On-Chain Auctions and
  Voting*](https://eprint.iacr.org/2023/1473).
- Shang and Chen, [*Economic Security of VDF-Based Randomness
  Beacons*](https://arxiv.org/abs/2604.04744), 6 April 2026.
- Gaži, Quader, and Russell, [*Taming Iterative Grinding Attacks on
  Blockchain Beacons*](https://eprint.iacr.org/2025/1974), ASIACRYPT 2025.
- Microsoft Research, [*ElectionGuard Specification
  2.1*](https://electionguard.vote/spec/).
- Cortier, Debant, and Gaudry, [*Breaking Verifiability and Vote
  Privacy in CHVote*](https://eprint.iacr.org/2025/080), ESORICS 2025.
- NIST, [*Considerations for Achieving Crypto Agility: Strategies and
  Practices*, CSWP 39 update
  1](https://csrc.nist.gov/pubs/cswp/39/upd1/final), 29 June 2026.

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

- Settle the merge candidate and pass focused data-model/Core/Torii tests,
  workspace tests, strict Clippy, formatting, and the source/model contract.
- Qualify the implemented live threshold-beacon partial-share transport,
  per-session runtime custody, threshold aggregation, candidate-effect
  assembly, and authoritative finalized-pulse persistence on at least four
  peers, including missing/invalid shares, restart, idempotent retransmission,
  mandatory NPoS boundary slots, optional Parliament demand slots, and key
  rotation.
- Qualify the implemented authenticated release-context read, bodyless local
  partial request, independently verifying multi-session custody/coordinator,
  canonical combine, ordinary `FinalizeOpenedBallot` submission tooling, and
  the bounded public broker projection/projected-signer validation boundary
  against a genuine authenticated broker/HSM provider. The public projection is
  not evidence of committed-state origin. Qualify the implemented consensus
  active-session cutover and the custody rule that forbids retirement while a
  session remains selectable or any committed ballot deadline references it;
  then demonstrate daemon-scoped broker admission,
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
- Add four-peer finalized-beacon, timed-release, missed-deadline, retry,
  restart/restore, authority-bound self-absence, conflicting/matching
  public-finding endorsements, stale-head supersession, and exact-height
  enactment tests.
- Qualify early impossible-quorum and post-deadline public-finding `NoResult`
  across restore and four peers, including permissionless-trigger availability.
  The reducer's terminal state is deterministic, but progress still assumes an
  eligible transaction eventually submits the deadline trigger.
- Complete Torii, MCP, CLI, OpenAPI, Rust/JavaScript/Kotlin/Java/Swift SDK, and
  shared-fixture coverage for the typed attempt and certificate surface; verify
  the retired equal Parliament ballot stays absent and remove remaining
  proposal-backed referendum surfaces.
- Qualify the aggregate-only transition/failure counters and committed
  status/stage gauges across restart and four-peer execution, then add reviewed
  stuck-attempt/deadline alarms. Keep identifiers, roots, registrations,
  ballots, shares, individual openings, and account labels out of metrics.
- Add focused and four-peer evidence for automatic enactment, stale-head
  supersession, and rollback-isolated `ExecutionFailed` recording, including
  restart validation and rejection of every signed terminal-outcome draft.
- Obtain an independent review of the exact timed-OVN arithmetic,
  Fiat--Shamir statements, constant-time/side-channel boundary, threshold-BLS
  corruption assumptions, implementation, build artifacts, and target matrix.
  The official publication manifest validator exists, but no external audit
  report or evidence archive is embedded or claimed by this repository.
- Run the bounded model as counterexample search and archive its same-source
  output. It is complementary evidence, not a replacement for proof review,
  cryptographic test vectors, implementation tests, or multi-peer execution.
