# SORA Parliament V1 lifecycle model

`SoraParliamentV1.tla` is a finite safety model of the attempt reducer in
`crates/iroha_core/src/governance/parliament.rs` and its native execution
boundary in `crates/iroha_core/src/smartcontracts/isi/world.rs`.

The model covers these consensus bindings:

- the eligible-citizen snapshot is frozen before a strictly future finalized
  threshold-beacon pulse, and the initially required bodies are consumed as one
  simultaneous draw batch. A sortition pulse may be classified unavailable
  only strictly after its exact height and only while no authoritative pulse is
  known. That objective failure records `NoRoster`; a retry supersedes the
  complete failed initial generation, freezes a fresh snapshot, uses the exact
  next sequence no earlier than the failure height, and the final permitted
  sequence rejects the governance attempt;
- active timed-OVN ballots reserve both heavy-work windows in one global
  checked set. Admission rejects a duplicate, an intersecting reservation, or
  the first entry beyond the exact capacity without changing the committed
  set; terminal release returns the slot. The model explores both a conflicting
  pair and a nonconflicting pair, while the source-bound Core tests additionally
  exercise deterministic derived-index rebuild after restore;
- an absence is an authority-bound declaration for the member's exact seated
  assignment, is immutable, and never lowers original-seat quorum; each seated
  public-body assignment can endorse only one immutable result root, and Core
  finalizes only at the original-seat two-thirds quorum while binding the
  strict canonical endorser sequence, its derived root, count, and quorum into
  the later certificate. After either a self-absence or endorsement, Core also
  computes `eligible = original roster - absences`, `remaining = eligible -
  immutable endorsements`, and the strongest existing root; if that root plus
  every remaining seat cannot reach quorum, it deterministically sets the body
  to `NoResult` and rejects the governance attempt;
- entry into a public body's Reflection phase freezes an exact endorsement
  deadline. Endorsements and any still-eligible self-absence declaration are
  accepted through that height; after it, the payload-minimal permissionless
  failure trigger derives `DeadlineExpired`, sets the body to `NoResult`, and
  rejects the attempt;
- registration and survivor freeze close only at their exact derived heights.
  The modeled policy requires at least `max_corpus_entries + 1` registration
  blocks, at least `max_corpus_entries` survivor blocks, and enough commitment
  blocks for `ceil(max_corpus_entries / 32)` maximum-size chunks; the corpus
  bound also covers every original seat.
  During `(survivor_freeze_height, commitment_close_height]`, the concrete
  reducer accepts contiguous chunks of 1 through 32 records and terminalizes
  only when the rolling accepted count exactly equals the frozen survivor
  count. `FreezeCommitmentInWindow` abstracts intermediate
  prefixes as stuttering and represents only the terminal append; the
  structural source contract separately pins the window, chunk cap,
  contiguous-prefix state, capacity bound, and terminal-only reducer advance.
  An incomplete prefix after the close remains `TimedCommitment` and is
  objectively eligible for `CommitmentDeadlineExpired`. Release consumption
  and aggregate finalization remain inside the inclusive
  `release_height + opening_phase_blocks` window;
- a missed deadline or an objectively absent finalized release pulse becomes
  `NoResult`; finalized-pulse availability is authoritative, a still-awaiting
  or opening ballot becomes eligible for deadline failure after the frozen
  window, an invalid aggregate opening is rejected without mutating state, and
  no plaintext, manual-opening, or fallback transition exists;
- a retry uses the exact next sequence, remains within the frozen retry bound,
  and consumes a fresh TLE session no earlier than its predecessor failure;
  failure of the final permitted ballot sequence rejects the governance
  attempt instead of leaving an unretryable active attempt;
- Core automatically constructs a certificate only from an approved aggregate
  result and fixes `enact_at_height = certified_at_height +
  min_enactment_delay` in the native execution boundary; and
- enactment happens only at that exact height and only when the current
  compare-and-set head equals the certified head. A different head yields
  `Superseded` without applying the effect. With an equal head, the automatic
  block-start step applies the effect in a rollback-isolated transaction. An
  effect error drops that transaction, then a fresh transaction records
  `ExecutionFailed` with a deterministic root derived by Core from the retained
  certificate and exact due height. Certificate construction and all terminal
  execution outcomes are absent from the public lifecycle transition enum.
  The latter use the separate, domain-separated
  `ParliamentAutomaticExecutionOutcomeV1` audit payload, which is not
  submit-able. The automatic step cannot advance a still-certified attempt
  past its due height.

The abstraction treats domain-separated identifiers, canonical roots,
zero-knowledge proofs, finalized beacon pulses, Das--Ren partial proofs, and
the final threshold-BLS release signature as already verified inputs. It does
not prove those primitives, network liveness, coercion resistance, receipt
freeness, side-channel resistance, or operational key custody. TLC success is
bounded counterexample-search evidence only, not deductive proof or release
approval.

The model admits split endorsements, but mirrors the reducer's deterministic
terminal rule: once `max endorsements on one existing root + remaining
eligible, uncommitted seats < ceil(2N/3)`, the body becomes `NoResult` and the
attempt becomes `Rejected`. This closes mathematically irreversible splits
without allowing a manager to choose a root. The model also covers
post-deadline non-response rejection after the frozen Reflection window.
Progress still depends on a
permissionless caller eventually submitting the deadline trigger; TLC checks
safety and does not prove that weak-fairness assumption.

The small configuration uses two abstract bodies, two seated assignments, two
competing public-finding roots, a two-block public-finding deadline, three TLE
sessions (two with an authoritative release pulse and one with an objectively
absent pulse), a two-block commitment window, a two-block opening window, and
two permitted sortition retries plus two permitted ballot retries. Its three
abstract resource reservations include one symmetric conflict pair and an exact
capacity of two. It is
intentionally large enough to explore self-absence, early impossible-root
rejection, conflicting and quorum-matching endorsements, post-deadline
non-response rejection, successful and unavailable sortition pulses, fresh
sortition retries, exhausted sortition rejection, private deadline/release
failure, exhausted ballot retry rejection, stale-head supersession, and
rollback-isolated effect-failure paths.
Run it with a compatible TLA2Tools installation:

```text
java -cp /path/to/tla2tools.jar tlc2.TLC \
  -config formal/sora_parliament/SoraParliamentV1.cfg \
  formal/sora_parliament/SoraParliamentV1.tla
```

The PR workflow uploads `sora-parliament-formal-pr` with stable
`inputs/SoraParliamentV1.{tla,cfg}`, `run-metadata.json`,
`source-contract.log`, and `tlc.log` paths plus separate exit-status files. TLC
runs the archived input copies. The metadata binds the input and output byte
sizes and SHA-256 digests, both recorded exit statuses, the checkout commit,
and the pinned TLA2Tools JAR digest. A separate always-run closure check rejects
missing, altered, or internally inconsistent evidence before upload.

The checked configuration must be exhaustively rerun with pinned TLA2Tools
v1.7.4 (TLC2 2.19) whenever this refinement or its commitment-window constant
changes. Bounded TLC results are revision-specific counterexample-search
evidence, not a proof about larger deployments.

Run the deterministic implementation/model binding check independently:

```text
python3 scripts/formal/check_sora_parliament_source_contract.py
```

The source contract is deliberately structural. It detects accidental removal
of the code-side guards represented by the model and separately pins the
authority-bound registration/dropout and reducer-derived registration/survivor
boundaries, authority-bound absence and public-finding endorsements. It also
structurally pins the opaque Core release authorization, independently verified
runtime partial, canonical combiner, authenticated bodyless local-partial route,
non-enumerable multi-session software custody seam, and bounded public broker
projection whose independently validated form cannot mint Core authorization.
The projection is not evidence of committed-state origin. Those operating
seams are outside the TLA state machine and the check is not a refinement proof.
Release still requires a settled source revision, focused Rust tests, qualified
live finalized-pulse production, consensus-enforced TLE key rotation, a genuine
authenticated broker/HSM share provider, four-peer timed-release/restart/retry
evidence, and the independently reviewed timed-OVN publication manifest
described in the roadmap. Zeroizing software buffers are not secure-erasure or
hardware-custody evidence.
