# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-59bd5ad1b1f69a5ae1734cedd889d81e62d0859b620e2d155c08814fda5e2f36"></a>

<!-- Original context: Roadmap / Release and Stabilization -->
- Keep extending the Sumeragi formal corridor with independent TLC
  cross-checks. After the 2026-07-03 parse-order repair, the monolithic central
  `deep` Apalache bundle still exhausts heap up to `64 GiB`, and the top-level
  TLC fast run did not reach progress output in the local 19-minute smoke; split
  probes for selected root obligations, including the end-to-end safety root,
  also exhausted heap at length 1. The top-level model now has direct-commit and
  delivered-first proof-corridor action relations for narrower tuning, but
  aggregate state-root, all-state-invariant, consensus/finality state-group,
  and single-property `EventuallyCommit` probes still exhausted a `16 GiB`
  Apalache heap, while top-level TLC corridor probes stayed silent in local
  smoke windows. The finite `direct-delivered-first-corridor-fast` gate now
  gives bounded Apalache/TLC evidence for the delivered-first direct commit
  path (`NoError` to length `1`; TLC `29` generated, `15` distinct, depth `2`),
  and its `12` `direct-delivered-first-corridor-bug-*` artifact mutations are
  now expected-failure checked by Apalache and routable through TLC. The
  TLC-only `direct-delivered-first-corridor-progress` mode now proves fair
  eventual installation of the delivered-first finality stack over the ordered
  `15`-state path, with `direct-delivered-first-corridor-progress-bug-*`
  modes confirming that missing any delivered-first finality-stack component is
  rejected by the progress-mode safety/progress surface. Its clean and
  mutation progress CFGs now also
  check `DirectDeliveredFirstProgressSafetyEnvelope`, tying temporal eventual
  finality to `TypeInvariant` and `DirectDeliveredFirstCorridorExactness` while
  the formal coverage guard pins the aggregate and CFG contract. The
  complementary `direct-vote-first-corridor-fast` gate now gives the same
  bounded positive Apalache/TLC evidence for the buffered vote/stake quorum
  before delivery path, and its `14` `direct-vote-first-corridor-bug-*`
  artifact mutations are expected-failure checked by Apalache and routable
  through TLC. The TLC-only `direct-vote-first-corridor-progress` mode now
  proves fair eventual installation of the vote-first finality stack over the
  ordered `15`-state path, with `direct-vote-first-corridor-progress-bug-*`
  modes confirming the same safety/progress finality-stack boundary. Its clean
  and mutation progress CFGs now also check
  `DirectVoteFirstProgressSafetyEnvelope`, tying temporal eventual finality to
  `TypeInvariant` and `DirectVoteFirstCorridorExactness`; the formal coverage
  guard pins the aggregate and CFG contract. These ordered path progress modes
  are kept out of Apalache PR CI because Apalache ignores weak-fairness
  clauses.
  The finite `direct-commit-interleaving-fast` transition-system
  gate now checks both orderings plus mixed RBC/vote interleavings in one graph
  (`NoError` to length `14`; TLC `87` generated, `50` distinct, depth `14`),
  and its `15` `direct-commit-interleaving-bug-*` mutations are
  expected-failure checked by Apalache and routable through TLC. The formal
  coverage guard now pins the direct delivered-first, vote-first, and
  interleaving exactness aggregate bodies, plus their correctness and
  progress-safety envelopes, so clean progress checks cannot keep naming an
  aggregate whose safety content has drifted. It also requires every source
  safety mutation CFG to keep the same `TypeInvariant`, model exactness, and
  correctness-envelope surface as its clean fast CFG. The TLC-only
  `direct-commit-interleaving-progress` mode now proves fair eventual
  installation of the committed phase, delivered RBC state, READY quorum,
  complete chunks, header/digest seed, quorum vote/stake counters, and
  commit-certificate evidence over that no-fault `50`-state interleaving graph,
  with `DirectCommitProgressSafetyEnvelope` now checked in the clean and
  mutation progress CFGs so temporal eventual finality remains tied to
  `TypeInvariant` and `DirectCommitInterleavingExactness`. The formal coverage
  guard pins that source progress-safety aggregate and its CFG check surface.
  `direct-commit-interleaving-progress-bug-finality-not-latched`,
  `direct-commit-interleaving-progress-bug-phase-not-committed`,
  `direct-commit-interleaving-progress-bug-commit-evidence-votes-missing`, and
  `direct-commit-interleaving-progress-bug-commit-evidence-stake-missing`
  confirming that missing any finality-stack commit component is a temporal
  progress failure before the Byzantine extension. The
  `direct-commit-interleaving-progress-bug-propose-skips-rbc`,
  `direct-commit-interleaving-progress-bug-header-not-seeded`,
  `direct-commit-interleaving-progress-bug-digest-not-seeded`,
  `direct-commit-interleaving-progress-bug-drop-second-chunk`,
  `direct-commit-interleaving-progress-bug-ready-quorum-under-counted`, and
  `direct-commit-interleaving-progress-bug-skip-deliver-state` modes extend the
  same temporal evidence to the direct RBC path; this is kept out of Apalache
  PR CI because Apalache ignores weak-fairness clauses. The
  `byzantine-commit-interleaving-fast` gate now adds bounded mixed
  honest/Byzantine evidence for the same corridor with `F = 1`,
  Byzantine-vote budget, weighted stake, and `CommitQuorum - F` honest support
  (`NoError` to length `14`; TLC `160` generated, `78` distinct, depth `15`),
  and its `17` `byzantine-commit-interleaving-bug-*` mutations are
  expected-failure checked by Apalache and routable through TLC. The TLC-only
  `byzantine-commit-interleaving-progress` mode now proves fair eventual
  installation of the committed phase, delivered RBC state, quorum vote/stake
  counters, `CommitQuorum - F` honest support, and commit-certificate evidence
  over that source `78`-state interleaving graph, with
  `ByzantineCommitProgressSafetyEnvelope` now checked in the clean and
  mutation progress CFGs so temporal eventual finality remains tied to
  `TypeInvariant` and `ByzantineCommitInterleavingExactness`. The formal
  coverage guard pins that source progress-safety aggregate and its CFG
  check surface, independently pins every source progress safety envelope to
  `TypeInvariant` plus its model-specific exactness predicate, and requires
  `ByzantineCommitInterleavingExactness` to extend the direct interleaving core
  with `ProposedRoundInitializesRbc`. Source progress safety alignment
  inventories must stay duplicate-free, so duplicate source envelope-alignment
  rows cannot inflate the progress-safety coverage surface. This keeps the temporal proofs from
  drifting away from their safety baselines or dropping no-fault
  RBC/vote/certificate obligations under the Byzantine source layer. The
  `byzantine-commit-interleaving-progress-bug-finality-not-latched`,
  `byzantine-commit-interleaving-progress-bug-phase-not-committed`,
  `byzantine-commit-interleaving-progress-bug-commit-evidence-votes-missing`,
  `byzantine-commit-interleaving-progress-bug-commit-evidence-stake-missing`,
  and `byzantine-commit-interleaving-progress-bug-commit-without-honest-support`
  confirming that missing any Byzantine finality-stack commit component is a
  temporal progress failure before projection. The
  `byzantine-commit-interleaving-progress-bug-propose-skips-rbc`,
  `byzantine-commit-interleaving-progress-bug-header-not-seeded`,
  `byzantine-commit-interleaving-progress-bug-digest-not-seeded`,
  `byzantine-commit-interleaving-progress-bug-drop-second-chunk`,
  `byzantine-commit-interleaving-progress-bug-ready-quorum-under-counted`, and
  `byzantine-commit-interleaving-progress-bug-skip-deliver-state` modes extend
  the same temporal evidence to the source RBC path before projection; this is
  kept out of Apalache PR CI because Apalache ignores weak-fairness clauses. The
  `byzantine-delivered-first-top-fast` mode now typechecks a top-level
  delivered-first Byzantine direct-commit corridor in `Sumeragi.tla` and is
  explicitly tracked as Apalache-only because local TLC probes over that
  central corridor did not reach initial-state completion in bounded smoke
  windows. The complementary `byzantine-vote-first-top-fast` mode now
  typechecks the top-level vote-first Byzantine direct-commit corridor where
  buffered prepare/commit votes wait for RBC delivery to install finality. The
  `byzantine-direct-top-fast` mode now typechecks the combined top-level
  Byzantine direct-commit corridor for mixed RBC/commit-vote interleavings and
  its ordered-corridor coverage bridge back to the delivered-first and
  vote-first exactness envelopes. The formal coverage guard now also pins the
  three top-level Byzantine CFG check surfaces plus the direct
  conjunct/implication contracts for those `Sumeragi.tla` aggregate bridge
  operators. The
  `byzantine-commit-projection-fast` mode now checks named projection
  envelopes for the delivered-first, vote-first, and combined top-level
  Byzantine direct-commit evidence obligations (`NoError` to length `14`; TLC
  `160` generated, `78` distinct, depth `15`). It also now carries the full
  source interleaving exactness aggregate through the projection bridge, tying
  projected top obligations back to RBC evidence shape, proposal/RBC
  initialization, vote handoff, commit-certificate shape, and buffered-vote
  delivery semantics. Its `17`
  `byzantine-commit-projection-bug-*` mutations now match the source
  Byzantine interleaving mutation set: finality/accounting faults plus the
  proposal/RBC seeding, prepare-quorum, honest-stake, chunk/READY, and delivery
  state faults that were previously only projected through temporal progress
  mutations. All projection safety mutations check the same full bridge before
  expected rejection, and are expected-failure checked by Apalache and routable
  through TLC. The formal coverage guard now pins the `TypeInvariant`,
  projected direct-top exactness/correctness, ordered-top bridge, and
  interleaving bridge invariants in every projection mutation CFG, and pins the
  projection gate aggregate conjunct contracts, including bridge implication
  antecedents. It also compares the
  source/projection Byzantine mutation suffix families: projection safety must
  match source Byzantine interleaving safety, projection progress must match
  source Byzantine interleaving progress, and progress modes may omit only the
  documented safety-only Byzantine faults. It also pins the direct
  safety/progress mutation suffix families: delivered-first progress must match
  delivered-first safety, vote-first progress may omit only the documented
  pre-delivery safety faults, and direct interleaving progress may omit only
  the documented direct safety-only quorum/stake/pre-delivery faults. It also
  pins the clean projection fast/progress CFG check surfaces. The guard now
  also pins the internal top-corridor family split:
  `ByzantineDeliveredFirstTopExactness` remains the
  common direct-commit core, while `ByzantineVoteFirstTopExactness` and
  `ByzantineDirectTopExactness` remain that common core plus the
  delivered-without-finality wait obligations. The guard now also requires
  that the top/projection Byzantine direct-commit contracts stay aligned, so
  projected delivered-first, vote-first, combined top-corridor, and
  ordered-bridge implication obligations cannot drift from their central
  `Sumeragi.tla` counterparts. Byzantine top/projection operator alignment
  inventories must stay duplicate-free, so duplicate source or projected
  operator mapping rows cannot collapse before mirror validation. Byzantine
  top/projection implication inventories must stay duplicate-free, so duplicate
  source or projection implication rows cannot collapse before antecedent
  mirror validation. Byzantine top/projection literal conjunct inventories must
  stay duplicate-free, so duplicate literal source or projected literal rows
  cannot collapse before projected-conjunct derivation. The guard now also requires
  `ProjectionBridgeMatchesInterleavingCore` to mirror
  `ByzantineCommitInterleavingExactness`, keeping the projected bridge core
  tied to the source Byzantine interleaving proof surface. The guard also
  requires that the projection bridge interleaving exactness composes projected
  direct-top and source core obligations, deriving the full bridge surface from
  `ProjectedByzantineDirectTopExactness` plus
  `ProjectionBridgeMatchesInterleavingCore` instead of trusting a separate
  manual list. The TLC-only
  `byzantine-commit-projection-progress` mode now proves fair eventual
  installation of the projected finality stack over the same `78`-state
  projection graph. Its clean progress config now carries
  `ProjectedCommitProgressSafetyEnvelope`, tying the fair eventual-finality run
  to the ordered top-corridor bridge and full source/projection exactness
  envelope. The
  `byzantine-commit-projection-progress-bug-finality-not-latched`,
  `byzantine-commit-projection-progress-bug-phase-not-committed`,
  `byzantine-commit-projection-progress-bug-commit-evidence-votes-missing`,
  `byzantine-commit-projection-progress-bug-commit-evidence-stake-missing`,
  and `byzantine-commit-projection-progress-bug-commit-without-honest-support`
  TLC modes reject missing projected finality-stack commit components, including
  honest support, under the safety/progress surface. The
  `byzantine-commit-projection-progress-bug-propose-skips-rbc`,
  `byzantine-commit-projection-progress-bug-header-not-seeded`,
  `byzantine-commit-projection-progress-bug-digest-not-seeded`,
  `byzantine-commit-projection-progress-bug-drop-second-chunk`,
  `byzantine-commit-projection-progress-bug-ready-quorum-under-counted`, and
  `byzantine-commit-projection-progress-bug-skip-deliver-state` modes extend
  the same expected-rejection evidence to the RBC stack components. Every
  projection progress mutation CFG now also carries `TypeInvariant` and
  `ProjectedCommitProgressSafetyEnvelope`, and the formal coverage guard pins
  that type/safety/property surface. The guard also pins
  `ProjectedCommitProgressSafetyEnvelope` itself to
  `ProjectionBridgeCoversOrderedTopCorridors` plus
  `ProjectionBridgeMatchesInterleavingExactnessCorrectnessEnvelope`, so the
  temporal progress run cannot drift away from the bridge composition it
  depends on. The guard also pins that the projection progress spec composes
  the named fairness aggregate, keeps direct `[][Next]_vars` transition
  closure, and uses exactly the proposal, prepare, honest/Byzantine commit
  vote, RBC chunk, RBC READY, and RBC deliver fairness actions.
  The guard also requires that source progress specs compose their named
  fairness aggregates and keep their family-specific transition closure and
  fair action sets, so the source temporal proof specs cannot silently fall
  back to stale raw `WF_vars(...)` lists.
  The guard also requires that top-level Sumeragi specs compose their named
  fairness aggregates and keep the documented top-level commit, direct-commit,
  delivered-first, and vote-first transition closures and fair action sets.
  It also pins temporal CFG behavior bindings: clean and mutation progress
  modes must load their named `SPECIFICATION` operators, top-level Byzantine
  corridor CFGs must keep `INIT Init` with the family-specific `NEXT` operator,
  and the root fast/deep/TLC-fast CFGs must keep their documented
  `INIT`/`NEXT` versus `SPECIFICATION Spec` split, so a CFG cannot keep the
  right checks while model-checking a stale transition surface. Every local TLA
  module must pass the same module validation surface, so orphaned modules
  cannot keep malformed headers, unchecked dependencies, assumptions/proofs,
  duplicate declarations, namespace overlaps, or variable/`vars` drift outside
  runner-selected paths. CFGs must
  pass the global directive-shape scanner, so empty configs, unknown or
  malformed directives, duplicate `CHECK_DEADLOCK`, missing behavior, and
  missing proof checks cannot hide outside runner-referenced modes. CFGs must
  define exactly one behavior surface: either a single `SPECIFICATION` or
  exactly one `INIT` plus one `NEXT`, so newly added CFGs cannot rely on
  implicit behavior defaults or mix temporal and transition surfaces. CFG
  required behavior inventories must stay duplicate-free before expected
  behavior contracts compare a CFG's bound transition or temporal surface, so
  malformed expected behavior contracts cannot be collapsed before comparison.
  Every
  CFG must also check `INVARIANT TypeInvariant` plus at least one
  non-`TypeInvariant` invariant or property, so formal evidence cannot come
  from type-only or untyped proof surfaces. CFG semantic proof-target checks
  must propagate malformed operator inventories before deciding whether a CFG
  has semantic coverage, so malformed proof directives cannot hide the missing
  semantic obligation. CFG normalized proof-target
  inventories must stay duplicate-free, so normalized `INVARIANT`/`INVARIANTS`
  and `PROPERTY`/`PROPERTIES` entries cannot repeat a proof target or assign
  one target to both invariant and property roles before downstream CFG
  coverage can collapse them. CFG required proof-check inventories must stay
  duplicate-free before required-check and exact-check-set contracts compare
  expected obligations, so malformed expected proof surfaces cannot be
  collapsed before comparison. CFG filenames must belong to their
  inferred owning modules, so suffix fallbacks cannot accidentally let a CFG
  count against an unrelated TLA module. CFG operator references must
  resolve to zero-arity non-trivial targets on the inferred owning TLA module,
  so stale behavior/check/constraint names, parameterized targets, and literal
  or `TypeInvariant` aliases cannot satisfy coverage. CFG operator references
  must also be duplicate-free and role-disjoint, so repeated behavior or
  constraint directives, duplicate proof checks, invariant/property kind
  conflicts, and behavior/constraint/proof target reuse cannot count as formal
  evidence. CFG duplicate and non-trivial operator guards must propagate
  malformed operator inventories before checking role-disjointness or trivial
  target chains, so malformed proof directives cannot hide stale duplicate or
  vacuous target evidence. Clean fast CFGs must use model-specific `*CorrectnessEnvelope`
  proof checks and avoid generic correctness checks, so fast evidence cannot
  regress to broad root aliases when a direct corridor envelope is expected.
  Fast generic-check guards must propagate malformed operator inventories before
  deciding whether a fast CFG has model-specific correctness-envelope coverage,
  so malformed proof directives cannot hide stale generic or missing envelope
  evidence.
  Correctness-envelope shape guards must propagate malformed operator
  inventories before validating envelope structure, so malformed proof
  directives cannot hide stale or incomplete envelope composition.
  Direct-exactness shape and pairing guards must propagate malformed operator
  inventories before validating exactness bodies or envelope pairing, so
  malformed proof directives cannot hide stale direct exactness evidence.
  CFG proof-target shapes must preserve correctness-envelope/direct-exactness
  structure, so checked envelopes compose `TypeInvariant` and direct
  model-specific exactness conjuncts, direct exactness checks inline concrete
  predicates, and each checked exactness target is paired with a checked
  envelope in the same CFG.
  TLC runner constraint injections stay on documented singleton-or-empty
  families, so only the documented pure candidate-enumeration `*-fast` and
  `*-bug-*` branches may narrow TLC with `TlcSingletonOrEmpty`, and those
  branches must keep that exact constraint.
  TLC singleton constraint-family diagnostics must stay line-aware, so missing
  required singleton constraints identify the runner case line and wrong or
  undocumented `tlc_constraint` assignments identify the exact runner
  assignment line before constraint-family errors are grouped.
  The TLC runner must keep an empty top-level `tlc_constraint` default, so
  modes without documented per-branch constraints cannot inherit a hidden
  global state-space bound.
  Top-level runner default diagnostics must stay line-aware, so duplicate
  `expect_failure`, `typecheck_only`, or `tlc_constraint` defaults identify
  their exact runner assignment lines before default errors are grouped.
  Sumeragi formal proof commands must not set `APALACHE_LENGTH`, so CI,
  workflow, and counted README evidence cannot silently shrink documented
  per-mode bounds.
  Sumeragi formal proof commands must not set model-checker toolchain override
  variables, so evidence cannot silently swap the pinned Apalache/TLC
  installation.
  CI, workflow, and README formal commands must use strict Apalache/TLC runner
  shapes, so direct evidence invocations cannot hide extra arguments or
  malformed mode tokens outside the central scripts.
  Runner case block shape diagnostics must stay line-aware, so duplicate
  `case "$mode" in` declarations and missing `esac` terminators identify their
  runner script line before malformed runner-case errors are grouped.
  Apalache/TLC runner case inventories must stay duplicate-free, so repeated
  runner branches cannot overwrite or obscure the checked proof mode body.
  Apalache runner-mode CI reachability diagnostics must stay line-aware, so
  exact runner branches missing PR or formal CI identify the runner case line
  before runner inventories are collapsed.
  README Apalache command support diagnostics must stay line-aware, so
  unsupported documented Apalache commands identify the README command line
  before README mode sets are collapsed.
  README TLC command support diagnostics must stay line-aware, so unsupported
  documented TLC commands identify the README command line before README mode
  sets are collapsed.
  README formal runner commands must be standalone command lines, so inline
  prose, `echo`, or backtick mentions cannot satisfy documented mode coverage.
  README formal runner commands must live in shell fenced code blocks, so
  standalone prose outside command blocks cannot satisfy documented mode
  coverage.
  README shell fences containing formal runner commands must be closed, so one
  opening fence cannot make later prose count as command evidence.
  YAML `run:` formal commands are only accepted in workflow files, so shell
  scripts and Markdown cannot use workflow syntax to satisfy proof inventory or
  ordering checks.
  Workflow and baseline formal entrypoint commands must be active `run:` or
  script lines, so commented commands and prose mentions cannot satisfy
  proof-entrypoint coverage or ordering.
  Workflow active command extraction must ignore block-scalar bodies, so a
  command hidden inside `run: |` cannot satisfy single-line workflow entrypoint
  coverage.
  Formal preflight, sweep, and success ordering checks must use exact command
  lines, so preflight-looking commands with extra arguments cannot satisfy
  proof ordering before the real guard runs.
  CI and workflow mode inventories must count only active direct runner
  commands, so comments, prose, or `echo` lines cannot satisfy proof-mode
  coverage.
  Active CI/workflow TLC modes must be supported by the TLC runner and
  documented by README TLC commands, so direct TLC proof evidence cannot drift
  outside the checked inventory.
  Active CI/workflow TLC inventory diagnostics must stay line-aware, so
  unsupported or undocumented active TLC commands identify the offending proof
  surface command before mode sets are collapsed.
  Active CI/workflow Apalache support diagnostics must stay line-aware, so
  unsupported active Apalache commands identify the offending proof surface
  command before mode sets are collapsed.
  Active CI/workflow Apalache README coverage diagnostics must stay line-aware,
  so active Apalache commands missing README documentation identify the
  offending proof surface command before mode sets are collapsed.
  PR baseline TLC cross-check diagnostics must stay line-aware, so missing TLC
  parity identifies the Apalache runner case line and unexpected
  Apalache-only TLC evidence identifies the TLC runner or README command line.
  Active CI/workflow Apalache modes must be duplicate-free, so repeated active
  Apalache invocations cannot inflate or obscure PR, expected-failure, or
  scheduled/manual proof evidence.
  Active CI/workflow TLC modes must be duplicate-free, so repeated active TLC
  invocations cannot inflate or obscure the direct TLC evidence inventory.
  Active CI/workflow Apalache modes must not overlap across proof surfaces, so
  an Apalache proof mode cannot count in more than one of PR, expected-failure,
  or scheduled/manual evidence.
  Active CI/workflow TLC modes must not overlap across proof surfaces, so a TLC
  proof mode cannot count in more than one of PR, expected-failure, or
  scheduled/manual evidence.
  README Apalache/TLC command inventories must stay duplicate-free, so repeated
  documented command rows cannot collapse before proof-mode coverage is
  compared.
  README Apalache length table modes must stay duplicate-free, so repeated mode
  rows cannot collapse before runner-length comparison.
  README Apalache length table coverage diagnostics must stay line-aware, so
  missing PR-baseline rows identify the runner case line and unsupported rows
  identify the length-table line before length mode sets are collapsed.
  README Apalache length mismatch diagnostics must stay line-aware, so
  documented length drift identifies both the README length-table line and the
  Apalache runner case line before row maps are collapsed.
  Apalache length proof-input diagnostics must stay line-aware, so invalid or
  duplicate runner `apalache_length` assignments identify the exact assignment
  line before length proof-input errors are grouped.
  Apalache/TLC expected-failure assignment diagnostics must stay line-aware, so
  duplicate or explicit per-case `expect_failure=0` assignments identify the
  exact runner assignment line before expected-failure assignment errors are
  grouped.
  Apalache typecheck-only assignment diagnostics must stay line-aware, so
  duplicate, explicit default, or unallowlisted `typecheck_only=1` assignments
  identify the exact runner assignment line before typecheck-only mode errors
  are grouped.
  README fast-mode table modes must stay duplicate-free, so repeated fast-mode
  rows cannot collapse before TLC fast coverage comparison.
  README fast-mode table TLC support diagnostics must stay line-aware, so
  unsupported TLC-runner rows and rows missing README TLC commands identify the
  fast-mode table line before table mode sets are collapsed.
  TLC exact fast-mode README coverage diagnostics must stay line-aware, so
  exact TLC `*-fast` runner cases missing from the README fast-mode table
  identify the TLC runner case line before runner mode sets are collapsed.
  README mutation-mode TLC runner support diagnostics must stay line-aware, so
  documented mutation modes unsupported by TLC identify the README command line
  before Apalache/TLC bug-mode sets are collapsed.
  README mutation-mode TLC expected-failure diagnostics must stay line-aware,
  so documented mutation modes missing `expect_failure=1` identify both the
  README command line and the TLC runner case line.
  README mutation-mode expected-failure CI coverage diagnostics must stay
  line-aware, so README mutation commands missing from expected-failure CI
  identify the README command line before mutation mode sets are collapsed.
  Apalache expected-failure CI marker diagnostics must stay line-aware, so
  expected-failure CI Apalache modes missing `expect_failure=1` identify the
  expected-failure CI command line and the Apalache runner case line.
  Apalache baseline expected-failure diagnostics must stay line-aware, so PR
  or scheduled/manual Apalache modes with unexpected `expect_failure=1`
  identify the CI command line and the Apalache runner case line.
  Unexpected expected-failure marker diagnostics must stay
  assignment-line-aware, so PR, scheduled/manual, or non-mutation TLC modes
  with unexpected `expect_failure=1` identify the exact runner assignment line
  before marker errors are grouped.
  Apalache CI mutation-surface diagnostics must stay line-aware, so PR CI
  mutation modes and expected-failure CI non-mutation modes identify the active
  CI command line before CI mode sets are collapsed.
  README mutation CFG equivalence diagnostics must stay line-aware, so
  documented mutation modes with Apalache/TLC CFG drift identify the README
  command line before mutation mode sets are collapsed.
  README mutation CFG-name diagnostics must stay line-aware, so documented
  mutation modes with CFG filename drift identify the README command line
  before mutation mode sets are collapsed.
  TLC module identity diagnostics must stay line-aware, so Apalache/TLC module
  drift identifies the README or CI command line before TLC mode sets are
  collapsed.
  TLC module proof-input diagnostics must stay line-aware, so dynamic,
  duplicate, or invalid TLC `module` assignments identify the exact runner
  assignment line before module proof-input errors are grouped.
  TLC module assignment-count diagnostics must stay line-aware, so duplicate
  TLC `module` assignments identify their exact runner assignment lines before
  module proof-input errors are grouped.
  TLC constraint proof-input diagnostics must stay line-aware, so dynamic,
  duplicate, undefined, arity-mismatched, or trivial TLC `tlc_constraint`
  assignments identify the exact runner assignment line before constraint
  proof-input errors are grouped.
  Apalache/TLC unused runner-case diagnostics must stay line-aware, so stale
  runner branches identify the runner case line before used-case sets are
  collapsed.
  Apalache/TLC missing formal-file diagnostics must stay line-aware, so
  missing spec, CFG, or TLC module references identify the runner case line
  before missing-file sets are collapsed.
  Apalache/TLC proof-input path diagnostics must stay line-aware, so dynamic,
  duplicate, wrong-suffix, nested, or escaping proof-input paths identify the
  exact runner assignment line before proof-input errors are grouped.
  Apalache/TLC proof-input assignment-count diagnostics must stay line-aware,
  so duplicate `spec_file` or `cfg_file` assignments identify their exact
  runner assignment lines before proof-input errors are grouped.
  Apalache/TLC CFG shape diagnostics must stay line-aware, so malformed
  runner-selected CFGs identify the runner case line before CFG-shape errors
  are grouped.
  Apalache/TLC TLA validation diagnostics must stay line-aware, so malformed
  runner-selected TLA modules identify the runner case line before module
  validation errors are grouped.
  Apalache/TLC CFG inventory diagnostics must stay line-aware, so duplicate
  runner-selected CFG bindings and proof targets identify the runner case line
  before inventory errors are grouped.
  Apalache/TLC CFG proof-target diagnostics must stay line-aware, so weak
  runner-selected CFG semantic checks identify the runner case line before
  proof-target errors are grouped.
  Apalache/TLC CFG module-binding diagnostics must stay line-aware, so
  runner-selected CFG ownership and operator-reference failures identify the
  runner case line before module-binding errors are grouped.
  Apalache/TLC CFG constant-binding diagnostics must stay line-aware, so
  runner-selected CFG constant bindings identify the runner case line before
  constant-binding errors are grouped.
  Apalache/TLC CFG trivial-target diagnostics must stay line-aware, so
  runner-selected trivial CFG checks identify the runner case line before
  trivial-target errors are grouped.
  Apalache/TLC CFG correctness-envelope diagnostics must stay line-aware, so
  runner-selected correctness-envelope checks identify the runner case line
  before envelope-shape errors are grouped.
  Apalache/TLC CFG direct-exactness diagnostics must stay line-aware, so
  runner-selected direct exactness checks identify the runner case line before
  exactness-shape errors are grouped.
  Apalache/TLC CFG exactness-pairing diagnostics must stay line-aware, so
  runner-selected direct exactness/envelope pairing checks identify the runner
  case line before pairing errors are grouped.
  TLC non-mutation expected-failure diagnostics must stay line-aware, so
  non-mutation TLC modes with unexpected `expect_failure=1` identify the
  README or CI command line and the TLC runner case line.
  Formal coverage audit must run before Apalache, TLC, and expected-failure
  evidence commands, so stale inventory wiring cannot execute proof jobs before
  the guard fails.
  Formal workflow triggers must keep the checked PR and scheduled/manual
  surfaces as exact top-level `on:` blocks, so PR evidence remains on
  `pull_request` for `main` without extra nested filters and nightly evidence
  remains both manually dispatchable and scheduled at the reviewed cron.
  Formal workflow names must stay exact, so the `${{ github.workflow }}`
  concurrency group and evidence identity cannot drift while proof commands
  stay unchanged.
  Formal workflow name inventories must stay duplicate-free, so duplicate
  workflow rows cannot collapse or contradict the reviewed proof workflow
  identity.
  Formal workflow header key inventories must stay exact, so `run-name`,
  duplicate `env`, or other unreviewed top-level controls cannot change proof
  metadata or inherited workflow behavior.
  Formal workflow header-key contract rows must stay duplicate-free, so
  duplicate workflow rows or expected header keys cannot collapse before
  header-only proof workflow controls are validated.
  Formal workflow full top-level key inventories must stay exact, so
  post-`jobs` `permissions`, `defaults`, duplicate `jobs`, or other late
  top-level controls cannot bypass the header-only proof workflow guards.
  Formal workflow full top-level key contract rows must stay duplicate-free, so
  duplicate workflow rows or expected top-level keys cannot collapse before
  late top-level proof workflow controls are validated.
  Formal workflow trigger event inventories must stay exact, so extra `push`,
  `pull_request_target`, or inline trigger forms cannot run checked proof jobs
  outside the reviewed PR and scheduled/manual surfaces.
  Formal workflow trigger block inventories must stay duplicate-free, so
  duplicate workflow trigger rows or expected trigger lines cannot collapse
  before exact trigger block validation.
  Formal workflow trigger event inventories must stay duplicate-free, so
  duplicate workflow trigger-event rows or expected events cannot collapse
  before checked trigger surfaces are validated.
  Formal workflow path filters must keep the reviewed ignored-path set, so PR
  proof evidence cannot be skipped for formal specs, scripts, or workflow edits
  by adding broader `paths_ignore` entries.
  Formal workflow path-filter inventories must stay duplicate-free, so
  duplicate workflow path-filter rows or expected ignored paths cannot collapse
  before PR proof path filtering is validated.
  Formal workflow concurrency must keep reviewed cancellation behavior as an
  exact top-level block, so PR evidence can supersede stale branch pushes while
  scheduled/manual nightly evidence cannot be canceled by an extra, duplicate,
  or drifted concurrency policy.
  Formal workflow concurrency inventories must stay duplicate-free, so
  duplicate workflow concurrency rows or expected concurrency lines cannot
  collapse before proof-run cancellation behavior is validated.
  Formal workflow headers must not set defaults or token permissions, so
  checked proof jobs cannot inherit a different shell, working directory, or
  workflow-wide token scope while their job-level proof commands stay
  unchanged.
  Formal workflow header-control job inventories must stay duplicate-free, so
  duplicate workflow rows or checked-job names cannot collapse before inherited
  header controls are scanned.
  Formal workflow top-level env keys must stay reviewed, so checked proof jobs
  inherit only the approved PR CI variables and no scheduled/manual nightly
  workflow environment.
  Formal workflow top-level env key inventories must stay duplicate-free, so
  duplicate workflow rows or expected env keys cannot collapse before inherited
  environment keys are validated.
  Formal workflow top-level env bindings must stay exact, so approved inherited
  PR CI variables cannot change values while keeping the same keys.
  Formal workflow top-level env binding inventories must stay duplicate-free, so
  duplicate workflow rows or expected env bindings cannot collapse before
  inherited environment values are validated.
  Formal workflow step-name inventories must stay exact, so GitHub proof
  transcript labels cannot drift while the underlying checked actions and proof
  commands remain pinned.
  Formal workflow step-name contract rows must stay duplicate-free, so
  duplicate workflow/job rows or checked-job names cannot collapse or
  contradict the reviewed proof transcript labels.
  Formal workflow job key inventories must stay exact, so checked proof jobs
  cannot gain job-level metadata, outputs, concurrency, or duplicate `steps`
  blocks while runner, timeout, and command guards still pass.
  Formal workflow job-key contract rows must stay duplicate-free, so duplicate
  workflow/job rows, checked-job names, or expected job keys cannot collapse or
  contradict the reviewed proof-job metadata surface.
  Formal workflow step key inventories must stay exact, so checked proof steps
  cannot gain `id` metadata or other step-level fields while the reviewed
  names, actions, inputs, and commands remain unchanged.
  Formal workflow step-key contract rows must stay duplicate-free, so
  duplicate workflow/job rows or checked-job names cannot collapse or
  contradict the reviewed proof step structure.
  Formal workflow checked job declarations must stay singular, so PR and
  nightly formal evidence cannot disappear or be shadowed by duplicate
  `sumeragi_formal` or `frontier-nightly` declarations while unrelated PR jobs
  remain allowed.
  Formal workflow checked-job declaration inventories must stay duplicate-free,
  so duplicate workflow rows or checked-job names cannot collapse before
  checked job singularity is validated.
  Formal nightly workflow job inventory must stay exact, so scheduled/manual
  formal evidence cannot gain unrelated reporting, publishing, or status jobs
  beside `frontier-nightly`.
  Formal nightly workflow job inventory rows must stay duplicate-free, so
  duplicate nightly workflow rows or expected job names cannot collapse before
  the dedicated scheduled/manual proof-job inventory check runs.
  Workflow Apalache install and toolchain version pins must come from active
  commands, so comments, step names, duplicate checked-job names, or duplicate
  active/static install and toolchain pins cannot satisfy the pinned
  model-checker install contract.
  Formal workflows must verify the pinned Apalache binary before running proof
  jobs, preserving install -> version-probe -> proof-job ordering in both PR
  and nightly evidence.
  Formal baseline script must verify the pinned Apalache binary after coverage
  and before proof jobs, so direct baseline runs record the same model-checker
  binary before producing proof evidence.
  Expected-failure script must verify coverage and the pinned Apalache binary
  before mutation proof jobs, so standalone counterexample sweeps cannot run
  under stale inventory or an unrecorded model-checker binary.
  Standalone expected-failure script must stay Apalache-only, so direct TLC
  mutation evidence remains in the checked PR/README TLC inventory instead of
  drifting into the standalone mutation sweep.
  Formal baseline script must run the expected-failure sweep after all positive
  Apalache and TLC proof commands, so mutation counterexample evidence cannot
  be reported before the clean proof surface has completed.
  Formal baseline success marker must run after all proof and mutation evidence
  commands, so the success line cannot appear before a later proof or
  counterexample sweep can still fail.
  Expected-failure success marker must run after all mutation evidence
  commands, so the standalone mutation sweep cannot print success before a
  later mutation counterexample check can still fail.
  Formal CI proof scripts must stay linear and free of shell control-flow
  blocks, so proof commands cannot be made conditional while still satisfying
  inventory coverage.
  Formal CI proof scripts must not contain here-documents, so command-shaped
  text inside shell data blocks cannot satisfy proof inventory.
  Formal CI proof scripts must not contain early exits or error-handling
  overrides, so proof commands cannot become unreachable and proof failures
  cannot be masked after the strict shell preflight.
  Formal CI proof scripts may contain only allowlisted direct evidence commands,
  so unrelated shell commands or shell-composed substitutes cannot change proof
  runtime behavior while satisfying inventory checks.
  Formal workflow proof jobs may contain only single-line allowlisted run
  commands, so workflow block scripts or shell-composed run steps cannot wrap,
  skip, or mask the pinned formal evidence entrypoints.
  Formal workflow run steps must set `run` at most once and cannot combine `run`
  with `uses`, so ambiguous YAML keys cannot inflate proof-command inventory or
  hide whether GitHub Actions executes a shell command or an action step.
  Formal workflow run-command job inventories must stay duplicate-free, so
  duplicate workflow rows or checked-job names cannot collapse before
  single-line run-command and ambiguous-step validation.
  Formal workflow run inventories must match the checked proof jobs, so PR
  evidence runs only install/version/baseline and scheduled evidence runs
  install/version/baseline/frontier/docs-metadata in the reviewed order.
  Formal workflow run inventories must stay duplicate-free, so duplicate
  workflow/job rows, checked-job names, or expected run commands cannot be
  collapsed before exact proof-transcript checks run.
  Formal workflow proof jobs may use only allowlisted action steps, so arbitrary
  marketplace or local actions cannot mutate the checked workspace, toolchain,
  or proof environment while leaving the visible proof commands unchanged.
  Formal workflow action-step job inventories must stay duplicate-free, so
  duplicate workflow rows or checked-job names cannot collapse before
  allowlisted action scanning runs.
  Formal workflow action steps must set `uses` at most once, so duplicate action
  keys cannot hide which setup or reporting action GitHub Actions will execute.
  Formal workflow action inventories must match the checked proof jobs, so PR
  evidence uses only checkout/setup-java and scheduled evidence uses
  checkout/setup-java/report-upload in the reviewed order.
  Formal workflow action inventories must stay duplicate-free, so duplicate
  workflow/job rows, checked-job names, or expected action steps cannot be
  collapsed before exact setup-action transcript checks run.
  Formal workflow action inputs must match the pinned proof environment, so
  checkout cannot switch refs or paths, Java setup cannot drift from Temurin
  17, and report upload cannot silently change the evidence artifact contract.
  Formal workflow action input inventories must stay duplicate-free, so
  duplicate action rows or pinned input lines cannot be collapsed before proof
  environment input checks run.
  Formal workflow action-input job inventories must stay duplicate-free, so
  duplicate workflow rows or checked-job names cannot collapse before pinned
  action inputs are validated.
  Inline `with:` values count as input drift, so same-line YAML maps cannot
  bypass the pinned input contract.
  Formal workflow setup action steps must not use conditionals, execution
  modifiers, or continue-on-error, so checkout and Java setup cannot be skipped,
  masked, or run under workflow-local environment overrides while the later
  proof commands remain unchanged.
  Formal workflow setup-action job inventories must stay duplicate-free, so
  duplicate workflow rows or checked-job names cannot collapse before setup
  action controls are scanned.
  Formal workflow proof jobs must use the pinned runner label, so PR and
  scheduled/manual proof evidence stays on the reviewed `ubuntu-latest`
  GitHub-hosted environment instead of silently moving to a self-hosted or
  otherwise different runner.
  Formal workflow runner-label inventories must stay duplicate-free, so
  duplicate workflow/job rows cannot collapse before the checked proof runner
  environment is validated.
  Formal workflow proof jobs must keep pinned timeout budgets, so PR proof
  evidence keeps its 45-minute budget and scheduled proof evidence keeps its
  90-minute budget.
  Formal workflow timeout contract rows must stay duplicate-free, so duplicate
  workflow/job timeout rows or checked-job names cannot collapse or contradict
  the reviewed proof-job budget.
  Formal workflow proof jobs must not use dependency or environment gates, so
  checked proof evidence cannot be skipped behind unrelated `needs` jobs or
  GitHub environment approvals.
  Formal workflow proof jobs must not set job-level token permissions, so
  checkout and artifact actions use the reviewed default token scope rather
  than a workflow-local permission override.
  Formal workflow proof entrypoints must stay inside the checked formal jobs,
  so required install, version-probe, baseline, and nightly proof commands
  cannot be satisfied by unrelated workflow jobs.
  Formal workflow entrypoint inventories must stay duplicate-free, so duplicate
  workflow/job rows, checked-job names, required proof commands, or order pairs
  cannot be collapsed before entrypoint scoping and ordering checks run.
  Formal workflow proof commands must not appear outside checked formal jobs in
  any workflow, so unrelated workflow files or jobs cannot run hidden formal
  proof entrypoints outside the guarded install, version, ordering, and
  inventory contracts. Formal workflow allowed-job inventories must stay
  duplicate-free, so duplicate workflow-path or checked-job rows cannot be
  collapsed before proof-command scoping checks run.
  Formal workflow job scoping must only recognize jobs under the top-level jobs
  block, so same-named keys in `env`, `on`, or other workflow sections cannot
  satisfy or hide formal proof jobs.
  Formal workflow job scoping must normalize top-level and job-name YAML key
  spacing, so `jobs :` and `frontier-nightly :` cannot hide or drop checked
  formal jobs.
  Formal workflow proof steps must not use job or step conditionals, execution
  modifiers, or continue-on-error, so checked proof evidence cannot be skipped
  or reported green after a proof command fails, and cannot run under
  workflow-local `env`, `defaults`, `shell`, `working-directory`, step-level
  `timeout-minutes`, `container`, `services`, or `strategy` overrides, or
  job-level dependency or environment gates, or job-level `permissions`
  overrides. Job-level timeouts remain pinned workflow budgets. The guard
  normalizes YAML field spacing around colons, so `run :`, `if :`, and
  `continue-on-error :` cannot hide proof evidence or failure-masking controls.
  Formal workflow proof-step control job inventories must stay duplicate-free,
  so duplicate workflow rows or checked-job names cannot collapse before proof
  step and proof-job control fields are scanned.
  Formal workflow mode and toolchain inventories must stay scoped to checked
  formal jobs, so unrelated workflow jobs cannot satisfy pinned-toolchain
  evidence or add scheduled/manual formal modes to the proof inventory, and
  modes cannot overlap across PR, expected-failure, and scheduled/manual CI
  surfaces.
  Formal singleton evidence commands must appear at most once, so coverage
  audits, pinned-version probes, formal workflow entrypoints, expected-failure
  sweeps, and success markers cannot duplicate evidence with an ambiguous
  transcript.
  Formal CI singleton command inventories must stay duplicate-free, so
  duplicate script rows or singleton command rows cannot be collapsed before
  duplicate-command validation runs.
  Formal shell entrypoints must use `set -euo pipefail`, so failed proof,
  installation, or piped evidence commands stop the script instead of being
  masked by later commands.
  Formal shell entrypoint preflight diagnostics must stay line-aware, so
  missing or drifted shebang and strict-mode lines identify the exact script
  line and observed text before strict-shell errors are grouped.
  Apalache runner proof invocations must route through
  `run_with_expected_status`, so positive proofs and expected counterexample
  checks cannot bypass the shared exit-status contract in local,
  installed-binary, or Docker execution paths.
  TLC runner proof invocations must preserve the `PIPESTATUS[0]`
  status-capture contract, so piped TLC output remains logged while the
  expected-failure branch still interprets the model-checker exit status rather
  than `tee` success.
  The top-level Sumeragi proof CFGs must remain unconstrained, so root fast,
  deep, TLC-fast, and focused Byzantine top CFGs cannot satisfy correctness
  checks only under a hidden static CFG `CONSTRAINT`.
  The non-temporal top-level Sumeragi CFGs must not bind CHECK_DEADLOCK, so
  root fast, deep, and focused Byzantine top CFGs keep transition-safety proof
  surfaces separate from the TLC-fast temporal deadlock policy.
  The top-level sentinel CFG proof-check sets must remain exact, so
  `Sumeragi_fast` and focused Byzantine top CFGs cannot grow extra proof checks
  beyond their documented sentinel obligations. Byzantine top auxiliary CFG
  contract inventories must stay duplicate-free, so focused top behavior and
  constant-envelope rows cannot be overwritten while the checker maps them by
  CFG filename.
  Top-level Apalache/TLC CFG proof checks must stay in parity, so a deep-only
  or TLC-only root obligation cannot count as shared consensus-core evidence.
  Apalache-only top-level corridor modes must stay typecheck-only unless listed
  as the bounded `deep` exception, so un-TLC-cross-checked top corridors cannot
  be counted as bounded proof evidence. Apalache-only proof-mode inventories
  must stay duplicate-free, so repeated allowlist, typecheck-only, or
  bounded-exception rows cannot be collapsed before TLC-parity and
  typecheck-contract validation. Apalache typecheck-only runner inventories
  must stay duplicate-free, so repeated typecheck-only runner allowance rows
  cannot be collapsed before runner-case validation.
  Aggregate proof root conjuncts must stay named zero-arity operators, so
  shared TLA root/envelope contracts cannot add inline literals, formulas, or
  anonymous wrappers outside the named conjunct comparison, except for
  documented implication/spec wrappers with dedicated shape guards.
  The state-safety temporal wrapper must keep the direct
  `[] SumeragiConsensusCoreStateMatchesEnvelope` zero-arity shape, so the
  state+temporal aggregate cannot silently substitute a weaker alias or
  compound temporal formula for the checked state envelope.
  The top-level Sumeragi CFG constant sets must remain exact, so those same
  proof CFGs cannot bind extra model constants beyond the documented quorum,
  fault, stake, view, and RBC chunk envelope.
  CFG constant bindings must also match the inferred owning TLA
  module, so CFGs cannot omit declared constants, bind undeclared constants,
  or repeat a binding while still counting as formal evidence. It also pins
  that CFG duplicate constant-binding guards must propagate malformed binding
  inventories before checking duplicate assignments, so malformed config
  directives cannot hide repeated constants. CFG required constant-value
  inventories must stay duplicate-free before required-value and exact-set
  constant envelope checks run, so malformed required constant contracts cannot
  be collapsed before comparison. It also pins
  the root fast, deep, and TLC-fast quorum/fault constant envelopes, so those
  top-level proofs cannot silently become evidence for a different validator,
  quorum, stake, view, or RBC chunk bound. The top-level Byzantine
  delivered-first, vote-first, and combined direct corridor CFGs now carry the
  same pinned fast quorum/fault envelope. It also pins
  that temporal CFGs keep `CHECK_DEADLOCK FALSE` for the root TLC-fast and
  source/projection progress proof configs, so weak-fairness liveness runs
  cannot drift to a different TLC deadlock obligation. Every CFG that binds
  `SPECIFICATION` must also set `CHECK_DEADLOCK FALSE`, so newly added temporal
  proof configs cannot reintroduce TLC deadlock checking while preserving the
  right temporal surface. Clean temporal progress configs must also keep
  `Bug = "none"`, so positive liveness runs cannot accidentally check a
  mutation surface. Any non-mutation CFG that binds a
  mutation selector must keep it disabled (`Bug = "none"` or legacy `Bug = 0`,
  and boolean `Bug...` selectors set to `FALSE`) and must not mix exact and
  boolean selector styles. Progress mutation configs must bind
  `Bug` to their filename suffix, so expected-failure liveness runs cannot
  silently check a different injected fault. The guard now also applies that
  suffix rule to every quoted-string mutation `Bug` constant while preserving
  legacy numeric bug selectors as numeric selectors; numeric selectors must
  remain unique within each mutation CFG family so legacy selector-based modes
  cannot alias each other. Mutation configs must use exactly one selector
  style, so exact `Bug` selectors and boolean `Bug...` switches cannot be
  mixed or omitted, and selector constants must be duplicate-free within each
  CFG; every mutation CFG must check `INVARIANT TypeInvariant`, check at least
  one non-`TypeInvariant` invariant or property, resolve each semantic proof
  target to a zero-arity non-trivial operator on the owning TLA module, and bind
  the expected behavior surface. Custom mutation INIT exception inventories
  must stay duplicate-free, so duplicate exception rows cannot be collapsed
  before mutation behavior validation. Exact `Bug`
  selector values must remain quoted suffix strings or decimal
  legacy selectors, quoted selectors must be used by reachable TLA expressions
  that explicitly mention the exact `Bug` selector, and numeric selectors must
  be written in canonical positive decimal form and used as reachable TLA `Bug`
  relation operands. Boolean `Bug...` selector configs must remain one-hot, with only the
  documented double-sign compound mutation allowed to enable two gates; the
  TRUE selector must also match the mutation file suffix
  by normalized name or an explicit semantic alias, remain unique within its
  CFG family, be declared by the owning TLA module, and bind every declared
  boolean selector in that module, and be used by reachable TLA expressions
  outside declarations. Exact `Bug` selector configs must declare
  `Bug` in the owning module as well. Boolean mutation selector alias
  inventories must stay duplicate-free, so duplicate alias rows cannot be
  collapsed before selector-name validation. Boolean mutation compound selector
  inventories must stay duplicate-free, so duplicate compound rows or selector
  rows cannot be collapsed before one-hot validation. The alias/compound
  exception tables must stay live, necessary, and exact. Source safety and projection bridge mutation
  configs follow the same suffix rule, so bounded
  expected-failure safety runs cannot drift to a different fault either. Those
  safety mutation configs must also keep `INIT Init` / `NEXT Next`, so the
  expected-failure safety checks cannot run against a stale transition surface.
  Progress safety-only mutation allowlists must stay duplicate-free, so
  duplicate safety-only suffix rows cannot be collapsed before source and
  projection mutation-family alignment.
  Custom mutation `INIT` exceptions must stay live, necessary, and exact, so
  documented seed-initializer CFGs cannot silently point at missing files,
  default `Init`, stale behavior bindings, or non-zero-arity/nonexistent TLA
  operators.
  Clean source/projection fast configs must keep `Bug = "none"` plus
  `INIT Init` / `NEXT Next` for the same reason: positive bounded safety checks
  should not silently become mutation checks or stale-transition checks.
  The fast
  sentinel CFG also must keep `TypeInvariant` and
  `SumeragiConsensusCoreFastCorrectnessEnvelope` as top-level invariants, so it
  cannot swap the documented fast correctness envelope for a narrower exactness
  aggregate.
  This is kept out of Apalache PR CI because Apalache ignores
  weak-fairness clauses. The central
  top-level bundle still needs more splitting or tuning before direct
  deep/TLC evidence can count again. Fresh `quorum-fast`,
  `rbc-fast`, `rbc-causality-fast`, and the
  timeout/view-change helper slice (`timeout-derivation-fast`,
  `cached-slot-timeout-fast`, `pending-fast-path-timeout-fast`,
  `stalled-pending-timeout-fast`, `stalled-pending-frontier-timeout-fast`)
  plus the pure-engine aggregate transition slice (`engine-proposal-fast`,
  `engine-prepare-fast`, `engine-commit-fast`, `engine-new-view-fast`,
  `engine-payload-fast`, `engine-validation-result-fast`, `engine-tick-fast`,
  `engine-committed-block-fast`) and auxiliary engine boundary slice
  (`engine-handle-*`, `engine-certificate-*`, `engine-qc-ref-*`,
  `engine-highest-qc-record-fast`, `engine-reconfiguration-*`,
  `engine-validation-*`, plus initial/read/state-preservation helpers) now give
  current Apalache/TLC parity for all `52` pure-engine fast modes. These passes
  plus the refreshed lock/fork-safety slice (`block-known-for-lock-fast`,
  `missing-locked-qc-recovery-fast`, `active-lock-reject-recovery-fast`,
  `locked-qc-helper-fast`, `precommit-qc-extends-locked-fast`,
  `drop-precommit-vote-for-lock-fast`, `same-height-vote-lock-fast`,
  `lock-rejected-sink-fast`, `committed-edge-conflict-fast`,
  `block-sync-locked-qc-fast`, `block-sync-commit-conflict-fast`) and commit/QC
  finality slice (`commit-roots-fast`, `commit-pipeline-recovery-fast`,
  `known-block-commit-qc-recovery-fast`, `stale-view-commit-qc-fetch-fast`,
  `commit-anchor-qc-fast`, `committed-height-qc-fast`,
  `commit-pipeline-scheduling-fast`, `precommit-vote-count-fast`,
  `commit-result-drain-fast`, `commit-drain-summary-fast`,
  `commit-pipeline-sample-fast`, `commit-pipeline-status-fast`,
  `commit-quorum-signers-fast`, `commit-qc-lookup-fast`,
  `precommit-signer-record-fast`, `prevalidated-commit-artifact-fast`) and
  proposal/admission slice (`vote-admission-fast`,
  `invalid-proposal-evidence-fast`, `proposal-mismatch-fast`,
  `proposal-cache-fast`, `proposal-hint-fast`,
  `stale-proposal-hint-repair-fast`, `stale-rbc-hint-repair-fast`,
  `proposal-admission-fast`, `block-created-admission-fast`,
  `proposal-budget-fast`, `non-rbc-payload-budget-fast`,
  `proposal-backpressure-fast`, `proposal-defer-warning-fast`,
  `proposal-batch-fast`, `idle-view-proposal-budget-fast`,
  `proposal-liveness-fast`, `actionable-vote-backed-proposal-fast`,
  `slot-proposal-evidence-fast`) and proposal/support/topology helper slice
  (`peer-admin-detection-fast`, `qc-signers-fast`,
  `missing-request-clear-fast`, `missing-block-clear-fast`,
  `lane-interleave-fast`, `commitment-snapshot-builder-fast`,
  `collector-selection-fast`, `topology-mutation-fast`,
  `prf-leader-shuffle-fast`) and roster/topology slice
  (`collector-plan-fast`, `topology-fanout-fast`,
  `topology-role-filter-fast`, `active-topology-selection-fast`,
  `p2p-topology-trusted-fast`, `p2p-topology-refresh-fast`,
  `build-signers-bitmap-fast`, `requester-roster-proof-fast`,
  `roster-validation-memo-fast`, `roster-validation-cached-fast`,
  `roster-validation-core-fast`, `roster-artifact-selection-fast`,
  `block-roster-caches-fast`, `block-sync-roster-evidence-fast`,
  `block-sync-history-roster-fast`, `persisted-roster-selection-fast`,
  `block-sync-update-roster-fast`, `roster-index-projection-fast`,
  `live-vote-roster-fast`, `canonical-round-roster-fast`,
  `vote-roster-selection-fast`, `vote-roster-cache-fast`,
  `roster-recovery-fsm-fast`) and pacemaker/new-view/retransmit slice
  (`post-commit-pacemaker-kick-fast`, `pacemaker-core-fast`,
  `pacemaker-evaluation-fast`, `pacing-governor-fast`,
  `pacing-backpressure-fast`, `pacemaker-backpressure-tracker-fast`,
  `missing-qc-timing-fast`, `new-view-stats-fast`,
  `new-view-tracker-fast`, `new-view-highest-qc-votes-fast`,
  `late-new-view-emission-fast`, `near-quorum-new-view-rebroadcast-fast`,
  `quorum-retransmit-fast`, `retransmit-backpressure-fast`,
  `paced-retransmit-targets-fast`, `quorum-reschedule-backoff-fast`,
  `rbc-availability-reschedule-fast`, `vote-backed-reassembly-stall-fast`,
  `completed-quorum-view-advance-fast`, `quorum-rebroadcast-dispatch-fast`,
  `isolated-vote-backed-handoff-fast`,
  `preemptive-vote-backed-retransmit-fast`,
  `near-quorum-preemptive-escalation-fast`,
  `manifest-gate-reschedule-fast`) and worker/control slice
  (`worker-tick-gap-fast`, `commit-worker-config-fast`,
  `vote-verify-worker-config-fast`, `qc-verify-worker-config-fast`,
  `worker-drain-fast`, `actor-gate-fast`, `worker-budget-fast`,
  `worker-ingress-fast`, `worker-loop-stage-fast`,
  `worker-queue-status-fast`) and protocol-boundary slice
  (`fork-fast`, `admission-fast`, `highest-fast`,
  `block-sync-recovery-fast`, `handshake-fast`,
  `consensus-handshake-caps-fast`, `effective-mode-fast`,
  `effective-timing-fast`, `consensus-params-ingress-fast`,
  `distinct-vote-epochs-fast`, `autoscale-transition-fast`,
  `commit-quorum-status-fast`, `commit-inflight-status-fast`) and
  commit/pending/Kura lifecycle slice (`commit-job-dispatch-fast`,
  `commit-stage-timing-threshold-fast`, `commit-inflight-timeout-fast`,
  `commit-evidence-replay-fast`, `commit-topology-state-fast`,
  `post-commit-cleanup-fast`, `kura-commit-fast`, `kura-retry-fast`,
  `pending-progress-fast`, `pending-block-lifecycle-fast`,
  `pending-block-marker-fast`, `requeue-transactions-fast`) and frontier
  slot/live-owner slice (`frontier-new-view-catch-up-fast`,
  `frontier-proposal-grace-fast`, `frontier-slot-helpers-fast`,
  `frontier-slot-tracker-fast`, `slot-tracker-state-fast`,
  `round-view-helpers-fast`, `phase-tracker-fast`,
  `tick-deadline-helpers-fast`, `idle-backlog-signals-fast`,
  `frontier-live-owner-work-fast`, `keep-frontier-pending-active-fast`,
  `superseded-frontier-payload-retention-fast`) and configuration/membership/
  signature-index slice (`da-gate-fast`, `manifest-guard-fast`,
  `mode-flip-fast`, `membership-view-hash-fast`, `membership-advert-fast`,
  `membership-mismatch-ingress-fast`, `tip-extension-helpers-fast`,
  `verify-cache-key-fast`, `signature-index-recovery-fast`,
  `signer-index-normalization-fast`, `voting-signer-count-fast`,
  `highest-optional-fast`) and proposal/precommit/vote/QC slice
  (`proposal-fast`, `precommit-fast`, `precommit-qc-view-change-fast`,
  `precommit-signer-history-fast`, `vote-backed-evidence-fast`,
  `vote-payload-actionable-fast`, `same-height-vote-conflict-fast`,
  `same-height-vote-recovery-gap-fast`, `qc-round-compatibility-fast`,
  `qc-signer-count-fast`, `qc-status-fast`, `vote-verify-async-fast`,
  `qc-verify-async-fast`) and reconfiguration/recovery/election/timing/
  frontier/proposal-edge slice (`reconfig-fast`, `recovery-fast`,
  `view-change-fast`, `npos-vrf-fast`, `validator-election-fast`,
  `stake-snapshot-fast`, `timing-monitor-fast`,
  `counter-backpressure-cooldown-fast`, `round-liveness-fast`,
  `fast-finality-inline-validation-fast`,
  `observer-signature-recovery-fast`, `ingress-dedup-cache-fast`,
  `kura-store-status-fast`, `proposal-parent-resolution-fast`,
  `proposal-stale-vote-fast`, `proposal-sccp-root-fast`,
  `frontier-gap-realign-fast`) and validation/QC evidence slice
  (`evidence-horizon-fast`, `evidence-canonicalization-fast`,
  `evidence-validation-fast`, `double-vote-recording-fast`,
  `invalid-qc-shape-fast`, `qc-validation-evidence-fast`,
  `qc-validation-reason-fast`, `validation-evidence-qc-fast`,
  `validation-fast`, `validation-priority-fast`,
  `validation-failure-finalize-fast`, `validation-reject-reason-label-fast`,
  `validation-reject-status-fast`, `validation-stall-redrive-fast`,
  `validation-redrive-label-fast`, `validation-ownership-cleanup-fast`,
  `validation-worker-config-fast`, `vnext-validation-fast`) and vNext
  control/rechain/signing slice (`vnext-chain-order-fast`,
  `vnext-stake-weight-fast`, `vnext-rechain-fast`,
  `vnext-rechain-error-label-fast`, `vnext-signature-fast`,
  `vnext-signing-preimage-fast`, `vnext-control-ingress-fast`,
  `vnext-slot-lifecycle-fast`, `vnext-deadline-protection-fast`,
  `vnext-performance-config-fast`) and BlockSync/QC
  recovery slice (`block-sync-qc-fallback-fast`,
  `block-sync-qc-status-fast`, `known-block-qc-enqueue-fast`,
  `known-block-qc-work-fast`, `known-block-qc-drain-fast`,
  `signed-quorum-fetch-fallback-fast`,
  `commit-qc-only-fetch-response-fast`, `block-sync-update-targets-fast`,
  `apply-cached-qcs-fast`, `block-sync-selected-signatures-fast`,
  `block-sync-selected-qc-fast`, `block-sync-selected-quorum-fast`,
  `block-sync-recovery-mode-fast`, `block-sync-selected-apply-fast`,
  `block-sync-selected-qc-prefilter-fast`,
  `block-sync-selected-qc-process-fast`,
  `block-sync-selected-qc-cache-fast`, `block-sync-future-window-fast`,
  `fetch-response-deferral-fast`) and RBC emission/rebroadcast/status slice
  (`rbc-ready-emission-fast`, `rbc-deliver-emission-fast`,
  `rbc-delivered-rebroadcast-fast`, `rbc-rebroadcast-cursor-fast`,
  `rbc-rebroadcast-action-fast`, `rbc-next-due-fast`,
  `pending-rbc-status-fast`, `rbc-status-lookup-fast`,
  `rbc-status-retention-fast`, `rbc-status-persistence-fast`,
  `rbc-status-handle-fast`, `rbc-backlog-status-fast`) and RBC chunk/session
  slice (`rbc-deliver-acceptance-fast`, `rbc-commit-processing-fast`,
  `rbc-chunk-target-fast`, `rbc-chunk-payload-cap-fast`,
  `rbc-rebroadcast-selection-fast`, `rbc-chunk-allocation-fast`,
  `rbc-payload-chunking-fast`, `rbc-payload-layout-fast`,
  `rbc-session-chunk-ingest-fast`, `rbc-session-ready-deliver-fast`,
  `rbc-delivered-payload-bytes-fast`, `rbc-rs16-initial-fanout-fast`,
  `rbc-chunk-broadcast-order-fast`, `pending-rbc-stash-fast`) and RBC
  repair/store/recovery slice (`rbc-abort-status-fast`,
  `rbc-mismatch-status-fast`, `rbc-progress-stage-fast`,
  `rbc-hot-repair-fast`, `rbc-repair-request-fast`,
  `rbc-targeted-repair-fast`, `rbc-outbound-flush-fast`,
  `rbc-chunk-post-debug-fast`, `rbc-deferral-throttle-fast`,
  `rbc-missing-init-rebroadcast-fast`, `rbc-sampling-fast`,
  `rbc-store-fast`, `rbc-store-status-fast`,
  `rbc-store-pressure-log-fast`, `round-gap-status-fast`,
  `rbc-recovery-helper-fast`, `rbc-payload-hydration-fast`,
  `rbc-missing-block-recovery-fast`, `rbc-unverified-roster-fast`,
  `rbc-preimage-fast`) and signing/penalty/execution-witness slice
  (`classic-preimage-fast`, `vrf-material-derivation-fast`,
  `vrf-penalties-report-fast`, `classic-signature-fast`,
  `invalid-signature-labels-fast`, `invalid-signature-throttle-fast`,
  `vote-validation-drop-status-fast`, `penalty-offender-selection-fast`,
  `consensus-penalty-action-fast`, `penalty-status-fast`,
  `local-peer-removed-status-fast`, `exec-witness-roots-fast`,
  `exec-witness-recorder-fast`, `exec-witness-access-key-fast`,
  `smt-path-hash-fast`) and status/observability slice
  (`ingress-status-counters-fast`, `consensus-message-labels-fast`,
  `phase-latency-status-fast`, `telemetry-status-fast`,
  `lane-detail-status-fast`, `settlement-status-fast`,
  `nexus-economics-status-fast`, `npos-repair-coverage-status-fast`,
  `mode-status-fast`, `consensus-caps-status-fast`,
  `effective-timing-status-fast`, `tx-queue-backpressure-status-fast`,
  `history-status-fast`, `online-validator-relay-counters-fast`,
  `membership-mismatch-status-fast`, `round-trace-status-fast`,
  `da-gate-status-fast`, `hotspot-log-summary-fast`,
  `adaptive-observability-fast`, `peer-key-policy-status-fast`,
  `view-change-cause-status-fast`, `view-change-proof-status-fast`) and
  block-message/frontier recovery slice
  (`block-message-rbc-compact-fast`, `block-message-priority-fast`,
  `block-message-height-view-fast`, `block-message-kind-fast`,
  `kura-replica-advert-fast`, `message-projection-fast`,
  `pipeline-event-emission-fast`, `block-message-wire-fast`,
  `block-created-frontier-wire-fast`,
  `block-payload-canonicalization-fast`, `cached-proposal-rebroadcast-fast`,
  `frontier-block-sync-hint-fast`, `frontier-same-slot-activity-fast`,
  `frontier-reassembly-activity-fast`,
  `frontier-quorum-owner-actionable-fast`,
  `frontier-sidecar-retarget-fast`,
  `frontier-sidecar-expected-hash-fast`,
  `contiguous-frontier-payload-hint-fast`,
  `frontier-parent-qc-hint-retarget-fast`,
  `live-frontier-idle-missing-qc-fast`,
  `missing-qc-reacquire-admission-fast`,
  `missing-qc-reacquire-action-fast`,
  `missing-commit-qc-actionable-fast`, `missing-qc-height-stall-fast`,
  `missing-qc-stall-range-pull-fast`,
  `missing-payload-fetch-window-fast`,
  `canonical-frontier-reanchor-fast`, `frontier-repair-view-change-fast`,
  `frontier-recovery-advance-fast`,
  `same-height-no-proposal-storm-fast`) and payload/fetch/recovery helper slice
  (`certified-fetch-fast`, `missing-block-ingress-fetch-fast`,
  `payload-progress-availability-fast`,
  `highest-qc-fetch-body-known-fast`, `local-payload-availability-fast`,
  `block-known-locally-fast`, `local-signed-block-lookup-fast`,
  `authoritative-payload-progress-fast`,
  `authoritative-block-payload-fast`,
  `pending-block-active-for-tip-fast`, `pending-fast-unblock-fast`,
  `blocking-pending-blocks-fast`, `quorum-recovery-vote-drain-fast`,
  `frontier-body-gap-payload-drain-fast`,
  `rbc-authoritative-payload-progress-fast`,
  `slot-authoritative-payload-fast`, `missing-block-fetch-fast`,
  `recovery-fsm-reason-fast`) and recovery-prune/cleanup helper slice
  (`empty-block-qc-drop-fast`, `failure-recovery-helpers-fast`,
  `highest-qc-dependency-deferral-fast`, `missing-block-hard-cap-fast`,
  `missing-block-hard-cap-cleanup-fast`, `missing-block-view-change-fast`,
  `restart-replay-fast`, `consensus-recovery-prune-fast`,
  `stale-view-pending-prune-fast`,
  `stale-missing-block-request-prune-fast`,
  `stale-missing-commit-qc-prune-fast`,
  `stale-rbc-session-prune-fast`,
  `highest-qc-defer-marker-prune-fast`) and Native AMX helper slice
  (`native-amx-attestation-fast`, `native-amx-journal-fast`,
  `native-amx-routing-plan-fast`, `native-amx-receipt-fast`,
  `native-amx-ingress-fast`) and recovery/status-counter helper slice
  (`recovery-status-counters-fast`, `qc-rebuild-status-fast`,
  `qc-rebuild-quorum-fast`, `collector-targeting-status-fast`,
  `deferred-recovery-status-fast`, `missing-qc-liveness-status-fast`,
  `sidecar-no-proposal-status-fast`,
  `deterministic-committee-status-fast`, `timing-status-counters-fast`,
  `roster-recovery-status-fast`, `range-pull-recovery-fast`,
  `range-pull-status-fast`, `round-recovery-bundle-window-fast`) and
  VRF/vote/embedded-QC slice
  (`vrf-admission-fast`, `vrf-epoch-window-fast`,
  `vrf-epoch-boundary-fast`, `vrf-epoch-restore-fast`,
  `vrf-local-state-fast`, `vote-duplicate-key-fast`,
  `embedded-qc-roster-fast`) and BlockSync roster/recovery slice
  (`block-sync-roster-fast`, `block-sync-roster-status-fast`,
  `block-sync-vote-deferral-fast`, `block-sync-known-hintless-fast`,
  `block-sync-implicit-recovery-fast`,
  `block-sync-vote-placeholder-fast`, `block-sync-snapshot-hint-fast`,
  `block-sync-snapshot-roster-fast`, `block-sync-no-roster-fast`,
  `block-sync-known-roster-fast`,
  `block-sync-known-selected-roster-fast`) and fetch/background/block-body slice
  (`block-sync-stale-view-fast`, `block-sync-warning-throttle-fast`,
  `qc-insufficient-warning-fast`, `fetch-block-body-handle-fast`,
  `background-frame-cap-fast`, `background-dispatch-fast`,
  `background-bypass-fast`, `background-fallback-fast`,
  `fetch-pending-response-send-fast`, `fetch-pending-responses-batch-fast`,
  `pending-response-flush-fast`, `deferred-block-sync-helper-fast`,
  `deferred-block-sync-cache-fast`, `deferred-block-sync-replay-fast`,
  `block-body-repair-fast`, `block-body-request-stash-fast`,
  `same-height-block-body-repair-fast`, `block-body-repair-epoch-fast`,
  `direct-commit-qc-for-block-fast`, `materialize-qc-fast`,
  `block-body-direct-commit-qc-fast`,
  `block-body-detached-commit-qc-fast`,
  `block-body-response-dispatch-fast`) confirm the current tractable path is
  decomposed helper-mode parity, not direct top-level obligation selection.
  The existing
  local TLC slices cover the top-level commit-path fast model under the
  fairness-backed `Spec`, including finality and
  finality latch/phase equivalence, commit-certificate finality equivalence,
  live commit-gate finality equivalence, NPoS stake-quorum fork-safety
  correctness envelope via
  `fork-npos`,
  live commit-gate RBC evidence binding,
	  inbound RBC READY/DELIVER key-header-signature evidence binding,
	  inbound RBC CHUNK key-header-signature evidence binding and digest matching,
	  RBC READY gate full-chunk matching,
	  RBC DELIVER gate complete-evidence matching,
	  RBC DELIVER finality buffered-commit matching,
	  RBC DELIVER finality-step commit-artifact installation,
	  RBC DELIVER finality-step committed-delivery completion,
	  RBC DELIVER finality-step complete committed-delivery entry,
	  RBC DELIVER pending-branch missing-commit-evidence matching,
	  RBC DELIVER pending-step commit-artifact preservation,
	  RBC DELIVER pending-step delivered-evidence/no-finality handoff,
	  RBC DELIVER pending-step complete wait-state entry,
	  RBC DELIVER delivery-entry finality/wait-state outcome split,
	  RBC DELIVER delivery-entry commit-artifact outcome matching,
	  RBC DELIVER delivery-entry post-gate surface matching,
	  RBC DELIVER delivery-entry consensus-frame outcome matching,
	  RBC DELIVER delivery-entry certified source-stack matching,
	  RBC DELIVER delivery-entry committed post-state invariant bundle,
	  RBC DELIVER delivery-entry finality post-state gate split,
	  RBC DELIVER delivery-entry pre-GST finality post-state gate branch,
	  RBC DELIVER delivery-entry post-GST finality terminal branch,
	  RBC DELIVER delivery-entry pending non-final wait surface,
	  RBC DELIVER delivery-entry pending timer-gate split,
	  RBC DELIVER delivery-entry pending pre-GST wait timers,
	  RBC DELIVER delivery-entry pending post-GST timeout/progress split,
	  RBC DELIVER delivery-entry pending delivered-wait predicate bridge,
	  RBC DELIVER delivery-entry pending continuation surface,
	  RBC DELIVER delivery-entry commit-evidence exact continuation split,
	  RBC DELIVER delivery-entry commit-evidence exclusive outcome discriminator,
	  RBC DELIVER delivery-entry commit-evidence exclusive gate outcome,
	  RBC DELIVER delivery-entry commit-evidence exact consensus frame,
	  RBC DELIVER delivery-entry commit-evidence exact action source,
	  RBC DELIVER delivery-entry commit-evidence certified/pending stack split,
	  RBC DELIVER delivery-entry commit-evidence exact witness surface,
	  RBC DELIVER delivery-entry commit-evidence live commit gate crossing,
	  RBC DELIVER delivery-entry commit-evidence continuation mode,
	  RBC DELIVER delivery-entry commit-evidence view handoff surface,
	  RBC DELIVER delivery-entry commit-evidence delivered evidence surface,
	  RBC DELIVER delivery-entry commit-evidence GST/timer surface,
	  RBC DELIVER delivery-entry commit-evidence progress action surface,
	  RBC DELIVER delivery-entry commit-evidence vote/stake budget surface,
	  RBC DELIVER delivery-entry commit-evidence threshold classifier,
	  RBC DELIVER delivery-entry commit-evidence pending commit-vote progress split,
	  RBC DELIVER delivery-entry commit-evidence pending non-commit-vote progress split,
	  RBC DELIVER delivery-entry commit-evidence pending progress partition,
	  RBC DELIVER delivery-entry commit-evidence post-state classifier,
	  RBC DELIVER delivery-entry commit-evidence certificate/progress disjointness,
	  RBC DELIVER delivery-entry commit-evidence action-family classifier,
	  RBC DELIVER delivery-entry commit-evidence Byzantine commit-vote boundary,
	  RBC DELIVER delivery-entry commit-evidence residual gate partition,
	  RBC DELIVER delivery-entry commit-evidence complete handoff,
	  RBC DELIVER delivery-entry commit-evidence continuation-state seed,
	  RBC DELIVER delivery-entry commit-evidence pending action-surface seed,
	  RBC DELIVER delivery-entry commit-evidence pending timer-surface seed,
	  RBC DELIVER delivery-entry commit-evidence pending counter-frame seed,
	  RBC DELIVER delivery-entry commit-evidence pending complete wait-state seed,
	  RBC DELIVER delivery-entry commit-evidence delivered-pending wait-state handoff,
	  RBC DELIVER delivery-entry commit-evidence complete continuation aggregate,
	  RBC DELIVER delivery-entry commit-evidence continuation envelope aggregate,
	  RBC DELIVER delivery-entry complete outcome envelope aggregate,
	  RBC DELIVER commit-evidence branch handoff,
	  RBC delivered-pending commit-evidence wait-state handoff,
	  RBC delivered-pending named complete wait-state closure,
	  RBC delivered-pending named commit-vote split,
	  RBC delivered-pending named commit-vote preservation handoff,
	  RBC delivered-pending named commit-vote finality handoff,
	  RBC delivered-pending named commit-vote certified-commit envelope,
	  RBC delivered-pending named classifier committed-certified outcome,
	  RBC delivered-pending named classifier non-committed wait envelope,
	  RBC delivered-pending named classifier complete outcome envelope,
	  RBC delivered-pending named prepare-vote split,
	  RBC delivered-pending named timeout/NewView handoff,
	  RBC delivered-pending named NewView-vote split,
	  RBC delivered-pending named proposal handoff,
	  RBC delivered-pending named GST preservation,
	  RBC delivered-pending named exact action-branch classifier,
	  RBC delivered-pending named stutter preservation,
	  RBC delivered-pending named complete branch classifier,
	  RBC delivered-pending commit-vote preservation handoff,
	  RBC delivered-pending commit-vote finality handoff,
	  RBC delivered-pending prepare-vote handoff,
	  RBC delivered-pending timeout/NewView handoff,
	  RBC delivered-pending NewView-vote handoff,
	  RBC delivered-pending proposal handoff,
	  RBC delivered-pending GST preservation,
	  RBC delivered-pending Next coverage,
	  RBC delivered-pending spec-step closure,
	  RBC delivered-pending spec-step outcome split,
	  RBC delivered-pending spec-step delivered-evidence preservation,
	  RBC delivered-pending spec-step commit-artifact outcome,
	  RBC delivered-pending spec-step GST boundary,
	  RBC delivered-pending spec-step view boundary,
	  RBC delivered-pending spec-step view-evidence boundary,
	  RBC delivered-pending spec-step vote-counter handoff,
	  RBC delivered-pending spec-step post-gate handoff,
	  RBC delivered-pending spec-step timer-gate handoff,
	  RBC delivered-pending spec-step finality-source handoff,
	  RBC delivered-pending spec-step finality witness-frame,
	  RBC delivered-pending spec-step finality-stack outcome,
	  RBC delivered-pending spec-step finality-gate outcome,
	  RBC delivered-pending spec-step finality-quorum outcome,
	  RBC delivered-pending spec-step non-final handoff phase shape,
	  RBC delivered-pending spec-step action-surface closure,
	  RBC delivered-pending spec-step phase-change source,
	  RBC delivered-pending spec-step counter-change source,
	  RBC delivered-pending spec-step exclusive action source,
	  RBC delivered-pending spec-step stutter action-surface preservation,
	  RBC delivered-pending spec-step commit-artifact change source,
	  RBC delivered-pending spec-step commit-artifact certified-delivery bundle,
	  RBC delivered-pending spec-step exact-source certified-delivery bundle,
	  RBC delivered-pending spec-step stable-artifact non-final handoff,
	  RBC delivered-pending spec-step stable-artifact non-final source,
	  RBC delivered-pending spec-step stable-artifact counter footprint,
	  RBC delivered-pending spec-step stable-artifact phase/gate footprint,
	  RBC delivered-pending spec-step stable-artifact timer footprint,
	  RBC delivered-pending spec-step stable-artifact view/evidence footprint,
	  RBC delivered-pending spec-step stable-artifact finality footprint,
	  RBC delivered-pending spec-step stable-artifact RBC surface,
	  RBC delivered-pending spec-step stable-artifact complete wait state,
	  RBC delivered-pending spec-step complete handoff envelope aggregate,
	  RBC delivered-state complete lifecycle envelope aggregate,
	  Byzantine fault corruptible-RBC gate matching,
	  Byzantine fault digest-only corruption step,
	  RBC INIT gate repairable-state matching,
	  RBC INIT step header/digest evidence installation,
	  RBC CHUNK step chunk-evidence advancement,
	  RBC CHUNK partial/full-coverage handoff,
	  RBC READY step ready-evidence advancement,
	  RBC READY partial/quorum handoff,
	  RBC READY quorum-step DELIVER handoff,
	  live RBC header/digest handoff gating,
  live RBC chunk handoff gating,
  live RBC READY handoff gating,
  RBC end-to-end lifecycle envelope aggregate,
  committed-phase terminality,
  committed consensus-state stability,
  committed post-finality GST-only movement,
  committed+GST full-state quiescence,
  committed+GST disabled action guards,
  committed+GST Next rejection,
  committed-state terminal envelope aggregate,
  GST observation provenance,
  timeout no-progress preemption,
  view-advance timeout provenance,
  live-progress timeout-reset provenance,
  view-evidence quorum/timeout provenance,
  NewView vote-counter provenance,
  prepare-vote counter provenance,
  commit-vote/stake counter provenance,
  phase-transition provenance,
  timeout/view-change recovery envelope aggregate,
  prepare-phase entry provenance,
  commit-vote phase entry provenance,
  propose-phase entry provenance,
  NewView phase entry provenance,
  committed-phase finality-source entry provenance,
  committed-phase certified finality-stack entry,
  committed-phase commit-certificate witness installation,
  committed-phase commit-certificate witness-change equivalence,
  committed-phase commit-view witness-change matching,
  committed-phase commit-view witness installation,
  committed-phase live-commit gate crossing,
  committed-phase commit-artifact installation equivalence,
  committed-phase exact finality-source effects,
  committed-phase NewView handoff exclusion,
  committed-phase post-entry progress-gate closure,
  certified finality-installation envelope aggregate,
  post-finality state stability envelope aggregate,
  pre-commit proposal/prepare handoff envelope aggregate,
  commit-vote finality handoff envelope aggregate,
  finalized certificate/evidence retention envelope aggregate,
  committed-state Byzantine commit-vote gate closure,
  RBC state protocol/fault provenance,
  RBC global state-change exit classification,
  RBC evidence protocol/fault provenance,
	  RBC global evidence-change effect classification,
	  RBC global progress/evidence mutation classifier,
	  RBC global progress/evidence handoff-envelope aggregate,
	  Sumeragi consensus core end-to-end safety envelope aggregate,
	  Sumeragi consensus core state+temporal safety envelope aggregate,
	  Sumeragi consensus core correctness envelope aggregate,
	  RBC header installation provenance,
  RBC header evidence monotonicity,
  RBC digest installation provenance,
  RBC digest invalidation fault provenance,
  RBC corruption entry provenance,
  RBC corruption repair exit classification,
  RBC Idle exit classification,
  RBC INIT entry provenance,
  RBC INIT exit classification,
  RBC chunk-counter increase provenance,
  RBC chunk-counter reset provenance,
  RBC chunking entry provenance,
  RBC chunking exit classification,
  RBC chunk-completion entry provenance,
  RBC chunk-complete exit classification,
  RBC READY-vote increase provenance,
  RBC READY-vote reset provenance,
  RBC READY partial-entry provenance,
  RBC READY partial exit classification,
  RBC READY quorum-entry provenance,
  RBC READY quorum exit classification,
  RBC delivered evidence stability,
  RBC DELIVER step complete-evidence preservation,
  RBC delivery-entry provenance,
  RBC delivery-entry ReadyQuorum/finality branch classification,
  RBC Withheld unreachable-state proof,
  RBC Withheld transition-target exclusion,
  commit-artifact finality-only installation,
  commit-artifact finality-source provenance,
  commit-artifact certified finality-stack change,
  finality-latch complete-stack installation,
  finality-latch/commit-artifact coupling,
  committed-phase complete-stack entry,
  committed-phase/finality-latch entry coupling,
  finality-latch committed-transition monotonicity,
  finality-latch/live-commit-gate crossing equivalence,
  finality-latch commit-certificate witness installation,
  commit-certificate witness component coupling,
  commit-certificate witness certified finality-stack change,
  commit-certificate witness commit-view installation,
  nonzero finality commit-view witness installation,
  finality-latch commit-view witness installation,
  commit-view witness certified finality-stack change,
  commit-view witness commit-certificate installation,
  finality-latch NewView handoff isolation,
  finality-latch source classification,
  finality-latch source-effect exactness,
  finality-latch source quorum-gate evidence,
  finality-latch certified source-stack classification,
  committed-view witness stability,
  committed-view witness step stability,
	  commit-view future-view exclusion,
	  GST elapsed pre-GST gate matching,
	  GST elapsed flag-only step,
	  GST monotonicity,
	  view monotonicity,
	  commit-view monotonicity,
	  commit-evidence monotonicity,
	  timeout stalled-progress gate matching,
	  timeout Byzantine-only commit progress independence,
	  timeout-step fresh NewView reset,
	  timeout-step commit-vote gate clearing,
  timeout-step fresh NewView vote handoff,
	  timeout-step RBC evidence preservation,
  view-change quorum evidence for nonzero active views,
  NewView quorum handoff and complete-only view evidence,
	  nonzero view-evidence active-view witness,
	  NewView vote quorum-step proposal handoff,
	  proposal handoff evidence matching,
	  proposal-step prepare/RBC installation,
	  proposal-step prepare-vote handoff,
	  live NewView vote handoff and fresh-evidence gate matching,
	  NewView vote quorum-branch evidence matching,
	  NewView vote quorum-step view-evidence installation,
	  NewView vote pending-branch missing-evidence matching,
	  NewView vote pending-step view-evidence preservation,
	  live prepare-vote handoff and proposal-evidence gate matching,
	  prepare-vote quorum-branch evidence matching,
	  prepare-vote quorum-step commit-vote handoff,
	  prepare-vote quorum-step commit-vote gate handoff,
	  prepare-vote pending-branch missing-evidence matching,
	  prepare-vote pending-step commit-artifact preservation,
	  prepare-vote pending-step prepare handoff preservation,
	  live commit-vote handoff and prepare-evidence gate matching,
	  Byzantine commit-vote prepare-evidence gate matching,
	  honest commit-vote finality-branch evidence matching,
	  honest commit-vote finality-step commit-artifact installation,
	  honest commit-vote finality-step committed-delivery completion,
	  honest commit-vote pending-branch missing-evidence matching,
	  honest commit-vote pending-step commit-artifact preservation,
	  honest commit-vote pending-step commit-vote handoff preservation,
	  Byzantine commit-vote finality-branch evidence matching,
	  Byzantine commit-vote finality-step commit-artifact installation,
	  Byzantine commit-vote finality-step committed-delivery completion,
	  Byzantine commit-vote pending-branch missing-evidence matching,
	  Byzantine commit-vote pending-step commit-artifact preservation,
	  Byzantine commit-vote pending-step commit-vote handoff preservation,
	  pending protocol GST preservation,
	  top-level end-to-end pending protocol GST preservation composition,
	  delivered RBC progress-gate closure,
	  complete-only commit evidence,
  pre-commit stale commit-vote reset across view changes,
  pre-prepare stale prepare-vote reset across view changes,
  pre-finality commit-artifact absence,
  finality certificate-stack completeness,
  finality certificate-stack exactness,
  finality NewView handoff cleanup,
  finality-source exact-source committed-delivery completion,
  finality-source certified-source stack classification,
  finality-source finality-latch change matching,
  finality-source committed-phase entry matching,
  finality-source finality-certificate stack installation,
  finality-source commit-or-delivery source classification,
  finality-source exact source-effect classification,
  finality-source quorum-gate satisfaction,
  finality-source commit-artifact change matching,
  finality-source live commit-gate crossing,
  finality-source post-commit progress quiescence,
  finality-source GST preservation,
  finality-source GST-only remaining gate,
  finality-source commit-certificate witness installation,
  finality-source commit-certificate witness-change matching,
  finality-source commit-view witness-change matching,
  finality-source commit-view witness installation,
  finality-source NewView handoff isolation,
  finality-source current-view commit witness exactness,
  committed-phase current-view commit witness exactness,
  committed-phase GST preservation,
  committed-phase GST-only remaining gate,
  commit-artifact exact-source committed-delivery completion,
  commit-artifact current-view witness exactness,
  commit-artifact GST preservation,
  commit-artifact GST-only remaining gate,
  commit-certificate exact-source committed-delivery completion,
  commit-certificate GST preservation,
  commit-certificate GST-only remaining gate,
  commit-view exact-source committed-delivery completion,
  commit-view GST preservation,
  commit-view GST-only remaining gate,
  finality-latch exact-source committed-delivery completion,
  finality-latch GST preservation,
  finality-latch GST-only remaining gate,
  committed-phase exact-source committed-delivery completion,
  live commit-vote prepare-quorum gating,
  commit-evidence roster-budget boundedness,
  run-level prepare-quorum commit gating, commit-certificate evidence stability,
  commit-certificate vote/stake traceability,
  live stake-accounting traceability,
  live stake roster-budget boundedness,
  honest commit-support preservation,
  live vote/stake quorum preservation,
  RBC finality evidence preservation,
  RBC progress-state evidence causality,
  RBC partial-progress counter causality,
  RBC progress-state evidence envelope aggregate,
  RBC live evidence causality envelope aggregate,
  RBC startup/defensive boundary aggregate,
  RBC corrupted digest invalidation,
  RBC ready-quorum deliver-gate availability,
  RBC delivered-without-finality certificate absence,
  RBC delivered finality commit-vote source,
  RBC delivered finality committed-delivery completion,
  RBC delivered finality current-view binding,
  RBC delivered finality GST-only remaining gate,
  RBC delivered finality commit-certificate witness installation,
  RBC delivered finality commit-certificate witness-change matching,
  RBC delivered finality commit-view witness-change matching,
  RBC delivered finality live commit-gate crossing,
  RBC delivered finality post-commit progress quiescence,
  RBC delivered finality certified source-stack matching,
  RBC delivered finality finality-certificate stack installation,
  RBC delivered finality committed-phase entry matching,
  RBC delivered finality commit-artifact change matching,
  RBC delivered finality latch-artifact coupling,
  RBC delivered finality exact commit-vote witnesses,
  RBC delivered finality delivered-RBC evidence preservation,
  RBC delivered finality view/prepare handoff evidence preservation,
  RBC delivered finality exact protocol frame,
  RBC delivered finality exact commit-vote action frame,
  RBC delivered finality committed post-state safety bundle,
  RBC delivered finality post-state gate split,
  RBC delivered finality pre-GST post-state gate branch,
  RBC delivered finality post-GST terminal branch,
  RBC delivered finality certified commit envelope aggregate,
  post-finality pre-GST only-enabled gate invariant,
  post-finality pre-GST GST-elapsed terminalization,
  post-finality pre-GST Next/GST-elapsed exclusivity,
  post-finality pre-GST spec-step stutter/GST split,
  committed+GST spec-step terminal stuttering,
  committed spec-step non-stuttering GST observation exclusivity,
  committed spec-step stutter/GST closure,
  committed spec-step finality-stack preservation,
  committed spec-step GST-only data-change footprint,
  committed spec-step no protocol-action footprint,
  post-finality progress-action quiescence,
  committed spec-step progress-gate quiescence, honest/fault roster-budgeted
  vote counters, RBC delivery stability, committed spec-step budgeted-RBC
	  evidence stability, fast
	  canonical frontier recovery, small exhaustive frontier recovery,
	  frontier recovery correctness envelope aggregate,
	  frontier committed source future-stage isolation,
  frontier view-bound drop future-stage isolation,
  frontier zero-evidence drop future-stage isolation,
  frontier zero-evidence staged-future expected-failure mutation,
  frontier zero-evidence drop consensus-evidence absence,
  frontier future-promotion fresh second-slot installation,
  frontier terminal outcome exclusivity,
  frontier rotated source future-stage isolation,
  frontier promotion-ready rotation isolation,
  frontier promotion-ready active-marker cleanup,
  frontier promotion-ready active-marker expected-failure mutation,
  frontier promotion-ready rotated-marker expected-failure mutation,
  frontier rotated terminal retransmit evidence,
  frontier promotion-ready wrapper cleanup,
  frontier quorum-retransmit window cleanup,
  frontier quorum-retransmit window cleanup expected-failure mutation,
  frontier payload recovery ownership,
  frontier payload recovery ownership expected-failure mutation,
  frontier stale-recovery unlock owner cleanup,
  frontier stale-recovery unlock owner cleanup expected-failure mutation,
  frontier view-bound drop retransmit evidence,
  frontier view-bound drop retransmit evidence expected-failure mutation,
  direct validation redrive labels, direct raw QC signer-bitmap population counting, and
  direct signer-index normalization, precommit vote-progress counting, precommit
  locked-payload vote gating, commit-QC
  signer quorum gating, commit-QC cache/history lookup, precommit signer record
  admission, validation ownership cleanup, direct stable worker-loop stage helpers,
  direct worker tick-gap scheduling, direct vNext performance config conversion,
  direct pending-block validation worker config derivation, commit-worker channel
  capacity normalization, slow commit-stage timing threshold detection,
  commit-inflight timeout reporting, commit-inflight timeout mark persistence
  expected-failure mutation, post-commit pacemaker kickstart gating,
  post-commit no-queue hard-stop expected-failure mutation, idle-view proposal
  budget preservation, idle-view no-queue hard-stop expected-failure mutation,
  cached-slot timeout selection, cached-slot timeout correctness envelope
  aggregate, cached-slot streak saturation
  expected-failure mutation,
  pending fast-path timeout derivation, pending fast-path timeout correctness
  envelope aggregate, pending fast-path DA-floor cap
  expected-failure mutation, stalled pending-block timeout
  decisions, stalled pending-block timeout correctness envelope aggregate,
  stalled pending commit-pipeline evidence expected-failure
  mutations, stalled pending-frontier timeout derivation,
  stalled pending-frontier timeout correctness envelope aggregate, exact-frontier
  proposal grace derivation, frontier proposal full-grace transaction-budget
  correctness envelope aggregate, frontier proposal full-grace transaction-budget
  expected-failure mutations, exact-frontier slot helper semantics,
  exact-frontier slot helper correctness envelope aggregate,
  frontier slot body-available helper expected-failure mutations,
  frontier slot same-candidate peer-evidence expected-failure mutations,
  exact-frontier slot tracker FSM behavior, exact-frontier slot tracker
  correctness envelope aggregate, exact-frontier apply-wrapper
  slot lifecycle expected-failure mutations, code-level exact-frontier slot
  single-source state cleanup, formal nested slot-state consistency alignment,
  slot tracker state map semantics, slot tracker state correctness envelope
  aggregate, proposal-seen horizon expected-failure
  mutations,
  timeout/cooldown derivation semantics, timeout/cooldown derivation
  correctness envelope aggregate, round/view helper semantics, round/view
  helper correctness envelope aggregate,
  PhaseTracker mutable state semantics, failed-commit/block-sync helper
  correctness envelope, same-height missing-QC height-stall dampening
  correctness envelope aggregate, missing-QC timing derivation,
  missing-QC timing correctness envelope aggregate, idle backlog signal derivation,
  idle backlog signal
  correctness envelope aggregate,
  proposal-liveness state transitions, proposal-liveness correctness envelope
  aggregate, direct actionable vote-backed proposal
  evidence admission, slot proposal evidence correctness-envelope lookup/fall-through,
  direct round-liveness no-bug evidence aggregation,
  round-liveness correctness envelope aggregate, direct
  roster-unavailability recovery FSM no-bug transitions,
  roster recovery FSM correctness envelope aggregate, consensus-recovery
  clear/prune retention semantics,
  consensus recovery prune correctness envelope aggregate,
  direct frontier live-owner work preservation semantics,
  frontier live-owner work correctness envelope aggregate,
  frontier live-owner conflict-adapter
  expected-failure mutations, direct keep-frontier pending-active
  preservation semantics,
  keep-frontier pending-active correctness envelope aggregate,
  direct stale-view pending prune no-bug cleanup
  semantics, stale-view pending prune correctness envelope aggregate,
  direct superseded frontier payload retention semantics,
  superseded frontier payload retention correctness envelope aggregate,
  direct stale missing-block request prune no-bug semantics,
  stale missing-block request prune correctness envelope aggregate,
  direct stale missing commit-QC
  request prune no-bug semantics,
  stale missing commit-QC request prune correctness envelope aggregate,
  direct stale RBC session prune no-bug
  semantics, stale RBC session prune correctness envelope aggregate,
  direct highest-QC defer-marker prune semantics,
  highest-QC defer-marker prune correctness envelope aggregate,
  fast-finality inline validation component/anchor semantics,
  fast-finality inline validation correctness envelope aggregate,
  observer signature-mismatch recovery semantics,
  observer signature-mismatch recovery correctness envelope aggregate,
  direct validation failure finalization semantics,
  validation failure finalization correctness envelope aggregate,
  direct validation reject reason-label classification semantics,
  validation reject reason-label correctness envelope aggregate,
  validation reject status accounting component/anchor semantics,
  validation reject status correctness envelope aggregate,
  peer-key policy status accounting component/anchor semantics,
  peer-key policy status correctness envelope aggregate,
  view-change cause status accounting component/anchor semantics,
  view-change cause status correctness envelope aggregate,
  view-change proof status accounting component/anchor semantics,
  view-change proof status correctness envelope aggregate,
  QC status projection component/anchor semantics,
  QC status correctness envelope aggregate,
  commit-quorum status projection component/anchor semantics,
  commit-quorum status correctness envelope aggregate,
  commit-inflight status projection component/anchor semantics,
  commit-inflight status correctness envelope aggregate,
  history status projection component/anchor semantics,
  history status correctness envelope aggregate,
  RBC abort status accounting component/anchor semantics,
  RBC abort status correctness envelope aggregate,
  RBC mismatch status accounting component/anchor semantics,
  RBC mismatch status correctness envelope aggregate,
  direct RBC progress-stage synchronization semantics,
  direct RBC progress-stage correctness envelope aggregate,
  direct RBC hot-repair/backpressure semantics,
  direct RBC hot-repair/backpressure correctness envelope aggregate,
  direct RBC repair request cooldown/targeting semantics,
  direct RBC repair request correctness envelope aggregate,
  direct RBC targeted READY/DELIVER repair semantics,
  direct RBC targeted READY/DELIVER correctness envelope aggregate,
  direct RBC outbound chunk flush semantics,
  direct RBC outbound chunk flush correctness envelope aggregate,
  direct RBC chunk post scheduling/debug-mask semantics,
  direct RBC chunk post scheduling/debug-mask correctness envelope aggregate,
  direct RBC READY/DELIVER deferral throttle semantics,
  direct RBC READY/DELIVER deferral throttle correctness envelope aggregate,
  direct RBC missing-INIT broad rebroadcast semantics,
  direct RBC missing-INIT broad rebroadcast correctness envelope aggregate,
  round-gap marker/snapshot/EMA status component/anchor semantics,
  round-gap status correctness envelope aggregate,
  direct RBC missing BlockCreated recovery and authoritative-only
  materialization semantics,
  direct RBC missing BlockCreated recovery correctness envelope aggregate,
	  direct RBC unverified-roster escape-hatch semantics,
	  direct RBC unverified-roster correctness envelope aggregate,
	  RBC signing-preimage component/anchor binding semantics,
	  RBC signing-preimage correctness envelope aggregate,
	  classic Vote/VRF signing-preimage aggregate exactness,
	  classic Vote/VRF signing-preimage correctness envelope aggregate,
	  classic Vote/QC signature-verification component/anchor semantics,
	  classic Vote/QC signature-verification correctness envelope aggregate,
	  direct invalid-signature telemetry label semantics,
	  invalid-signature telemetry label correctness envelope aggregate,
  invalid-signature throttle/penalty component/anchor semantics,
  invalid-signature throttle/penalty correctness envelope aggregate,
  direct penalty offender-selection attribution semantics,
  penalty offender-selection correctness envelope aggregate,
  consensus penalty-action derivation/application semantics,
  consensus penalty-action correctness envelope aggregate,
  penalty status projection component/anchor semantics,
  penalty status correctness envelope aggregate,
  local peer removed flag component/anchor semantics,
  local peer removed flag correctness envelope aggregate,
  direct execution-witness recorder lifecycle/keying correctness envelope
  aggregate,
  direct execution-witness access-key parser correctness envelope aggregate,
  direct execution-witness root projection component/anchor semantics,
  execution-witness root projection correctness envelope aggregate,
  sparse-Merkle path/hash helper correctness envelope aggregate,
  direct RBC compact block-message exactness/component semantics,
  RBC compact block-message correctness envelope aggregate,
  direct consensus block-message priority exactness/component semantics,
  consensus block-message priority correctness envelope aggregate,
  direct block-message height/view exactness/component semantics,
  block-message height/view correctness envelope aggregate,
  direct block-message log/status kind exactness/component semantics,
  block-message log/status kind correctness envelope aggregate,
  direct Kura replica advert ingress correctness envelope aggregate,
  direct consensus message projection semantics,
  consensus message projection correctness envelope aggregate,
  pipeline event emission semantics,
  pipeline event emission correctness envelope aggregate,
  direct cached block-message wire-frame component/anchor semantics,
  cached block-message wire-frame correctness envelope aggregate,
  direct BlockCreated frontier metadata wire/rebuild component/anchor semantics,
  BlockCreated frontier metadata wire/rebuild correctness envelope aggregate,
  direct BlockCreated payload admission aggregate exactness,
  BlockCreated payload admission correctness envelope aggregate,
  direct cached proposal rebroadcast component/anchor semantics,
  cached proposal rebroadcast correctness envelope aggregate,
  direct exact-slot frontier recovery activity semantics, exact-slot aggregate
  activity-source exactness,
  exact-slot frontier recovery activity correctness envelope aggregate,
  direct frontier reassembly activity semantics, frontier reassembly aggregate
  activity-source exactness,
  frontier reassembly activity correctness envelope aggregate,
  direct frontier quorum-owner cleanup preservation semantics, frontier
  quorum-owner aggregate cleanup exactness,
  frontier quorum-owner cleanup correctness envelope aggregate,
  direct contiguous-frontier sidecar retarget semantics, contiguous-frontier
  sidecar retarget aggregate exactness,
  contiguous-frontier sidecar retarget correctness envelope aggregate,
  direct contiguous-frontier sidecar expected-hash semantics,
  contiguous-frontier sidecar expected-hash aggregate exactness,
  contiguous-frontier sidecar expected-hash correctness envelope aggregate,
  direct contiguous-frontier payload-hint selection semantics,
  contiguous-frontier payload-hint aggregate exactness,
  contiguous-frontier payload-hint correctness envelope aggregate,
  direct contiguous-frontier parent-QC hint retarget semantics,
  contiguous-frontier parent-QC hint retarget aggregate exactness,
  contiguous-frontier parent-QC hint retarget correctness envelope aggregate,
  direct vote-verification worker config derivation, direct vote-verification worker config
  aggregate exactness,
  vote-verification worker config correctness envelope aggregate,
  QC aggregate-verification worker config derivation, QC aggregate-verification
  worker config aggregate exactness,
  QC aggregate-verification worker config correctness envelope aggregate,
  voting-roster support counting, voting-roster support-count aggregate
  exactness, voting-roster support-count correctness envelope aggregate,
  collector retry/gossip plans, collector retry/gossip plan aggregate
  exactness, collector retry/gossip plan correctness envelope aggregate,
  direct collector fanout/selection semantics, plus collector fanout/selection
  direct exactness, collector fanout/selection correctness envelope aggregate,
  direct topology ordered-roster mutation no-bug semantics, topology
  ordered-roster aggregate exactness, topology ordered-roster mutation
  correctness envelope aggregate, PRF leader/shuffle topology semantics,
  PRF leader/shuffle aggregate exactness, PRF leader/shuffle correctness
  envelope aggregate, direct topology fanout/redundant-send semantics,
  direct topology fanout/redundant-send aggregate exactness, topology
  fanout/redundant-send correctness envelope aggregate, topology role-filter
  semantics, topology role-filter aggregate exactness, topology role-filter
  correctness envelope aggregate, active topology-selection semantics, active
  topology-selection aggregate exactness, active topology-selection correctness
  envelope aggregate, trusted-peer P2P topology semantics, trusted-peer P2P
  topology aggregate exactness, trusted-peer P2P topology correctness envelope
  aggregate, P2P
  topology refresh semantics, P2P topology refresh aggregate exactness, P2P
  topology refresh correctness envelope aggregate, quorum
  retransmit target semantics, direct quorum retransmit target aggregate exactness,
  quorum retransmit target correctness envelope aggregate,
  direct retransmit backpressure aggregate exactness, retransmit backpressure
  correctness envelope aggregate, direct paced retransmit
  target aggregate exactness, paced retransmit target correctness envelope
  aggregate, direct quorum reschedule backoff aggregate exactness, quorum
  reschedule backoff correctness envelope aggregate,
  direct RBC availability reschedule aggregate exactness, RBC availability
  reschedule correctness envelope aggregate, direct vote-backed reassembly stall
  aggregate exactness, vote-backed reassembly stall correctness envelope
  aggregate, direct completed quorum view-advance component semantics, completed
  quorum view-advance correctness envelope aggregate,
  direct quorum rebroadcast dispatch aggregate exactness, quorum rebroadcast
  dispatch correctness envelope aggregate, isolated vote-backed handoff
  aggregate exactness, isolated vote-backed handoff correctness envelope
  aggregate, direct preemptive vote-backed retransmit aggregate exactness,
  preemptive vote-backed retransmit correctness envelope aggregate,
  direct near-quorum preemptive escalation aggregate exactness,
  near-quorum preemptive escalation correctness envelope aggregate,
  manifest-gate reschedule aggregate exactness, manifest-gate reschedule
  correctness envelope aggregate, QC signer-bitmap admission aggregate
  exactness, QC signer-bitmap admission correctness envelope aggregate,
  direct raw QC signer-count aggregate exactness, raw QC signer-count
  correctness envelope aggregate, QC signer-bitmap construction aggregate
  exactness, QC signer-bitmap construction correctness envelope aggregate,
  direct signer-index normalization aggregate exactness, signer-index
  normalization correctness envelope aggregate, commit-root consistency aggregate
  exactness, commit-root consistency correctness envelope aggregate,
  commit-pipeline recovery aggregate exactness, commit-pipeline recovery
  correctness envelope aggregate,
  direct known-block commit-QC recovery aggregate
  exactness, known-block commit-QC recovery correctness envelope aggregate,
  stale-view commit-QC fetch aggregate exactness, stale-view commit-QC fetch
  correctness envelope aggregate, direct commit-anchor QC promotion aggregate
  exactness, commit-anchor QC promotion correctness envelope aggregate,
  committed-height QC admission aggregate
  exactness, committed-height QC admission correctness envelope aggregate,
  direct empty-block QC drop component semantics, empty-block QC drop
  correctness envelope aggregate, pending-progress accounting aggregate
  exactness, pending-progress accounting correctness envelope aggregate,
  direct pending-block lifecycle no-bug exactness, pending-block lifecycle
  correctness envelope aggregate, direct pending-block marker/cooldown no-bug
  exactness, pending-block marker/cooldown correctness envelope aggregate,
  pending-block Kura retry aggregate exactness, pending-block Kura retry
  correctness envelope aggregate, commit-pipeline scheduling aggregate
  exactness, commit-pipeline scheduling correctness envelope aggregate,
  commit-QC cache/history lookup aggregate exactness, commit-QC cache/history
  lookup correctness envelope aggregate, cached-QC precommit signer-record
  aggregate exactness, cached-QC precommit signer-record correctness envelope
  aggregate, roster-validation memo cache aggregate exactness,
  roster-validation memo cache correctness envelope aggregate, cached
  roster-validation wrapper aggregate exactness, cached roster-validation
  wrapper correctness envelope aggregate, core roster-validation aggregate
  exactness, core roster-validation correctness envelope aggregate, roster
  artifact selection aggregate exactness, roster artifact selection correctness
  envelope aggregate, block roster cache aggregate exactness, block roster cache
  correctness envelope aggregate, block-sync roster evidence aggregate
  exactness, block-sync roster evidence correctness envelope aggregate,
  block-sync history roster aggregate exactness, block-sync history roster
  correctness envelope aggregate,
  persisted block-sync roster selection aggregate exactness, persisted
  block-sync roster selection correctness envelope aggregate, BlockSyncUpdate
  roster hydration aggregate exactness, BlockSyncUpdate roster hydration
  correctness envelope aggregate, direct roster index projection no-bug
  exactness, roster index projection correctness envelope aggregate,
  direct membership-view hash no-bug exactness, membership-view hash
  correctness envelope aggregate, membership mismatch status aggregate
  exactness, membership mismatch status correctness envelope aggregate,
  membership advert publication aggregate exactness, membership advert
  publication correctness envelope aggregate, membership mismatch
  ingress/fail-closed aggregate exactness, membership mismatch ingress
  correctness envelope aggregate, consensus-params ingress aggregate
  exactness, consensus-params ingress correctness envelope aggregate,
  prevalidated commit artifact trust aggregate exactness, prevalidated commit
  artifact correctness envelope aggregate, commit-job dispatch aggregate
  exactness, commit-job dispatch correctness envelope aggregate,
  commit-worker config aggregate exactness, commit-worker config correctness
  envelope aggregate, slow commit-stage timing threshold aggregate exactness,
  slow commit-stage timing threshold correctness envelope aggregate,
  commit-inflight timeout aggregate exactness, commit-inflight timeout
  correctness envelope aggregate, post-commit pacemaker kick aggregate
  exactness, post-commit pacemaker kick correctness envelope aggregate,
  idle-view proposal budget aggregate exactness, idle-view proposal budget
  correctness envelope aggregate, pacemaker core correctness envelope aggregate.

