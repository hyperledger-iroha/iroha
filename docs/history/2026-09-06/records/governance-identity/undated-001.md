# Historical project evidence

This page is historical evidence, not current release readiness.
See [the archive index](../../index.md) for provenance and reconstruction.


<a id="record-d925ec7f3aa62d0cd18be20dedb9fac66fba3564781573d6645067f1d93cfc82"></a>

<!-- Original context: Roadmap / Additive SNS dataspace bootstrap qualification -->
## Additive SNS dataspace bootstrap qualification



<a id="record-8da61a1a4731539a348a070ed0b0000009d4b1c8960e3ed1e2782296f6e05a34"></a>

- Carry the passing focused height-bound alias-registry and owner-bound
  bootstrap-grant tests into the settled release candidate. Preserve legacy
  committed plans, nested routing (including the corrected settlement/multisig
  height pre-walk), native quote/payment enforcement and static namespace
  ownership checks; source-level passes are not deployment qualification.


<a id="record-cb13f7e39e4b1539434f17a47b735b7b81bbd783600f415e66ca0434d675b8f0"></a>

<!-- Original context: Roadmap / SORA Parliament release qualification -->
## SORA Parliament release qualification



<a id="record-d3a350350b7878b139a61f540bdeeb545fb680c4c411edf89c2c50a7c0626979"></a>

- Update grouped Core governance tests to the current ballot, lock, and
  referendum model. They still reference removed public types and fields and
  block the broad Core test check; do not reintroduce those APIs to make the
  fixtures compile.


<a id="record-d247669bb982f60a53fcb42d48fe49cb732603a761d3ffdce21a128f94e4be7e"></a>

- From one settled fresh-genesis candidate, run the focused Parliament model,
  reducer, restore, configuration, and Torii route suites followed by strict
  all-target Clippy and the workspace test gate. Exercise the atomic
  Policy-to-Confirmation boundary, redraw exhaustion, restart recovery, and
  malformed snapshot rejection on a four-validator network with mandatory
  signed RS16 DA/RBC; regenerate and compare public API artifacts and SDK
  projections before declaring the first release qualified.


<a id="record-cfe0eb22b1b3de58141d01308af7509445ff14d5d94f8ea631b807dcce2bf111"></a>

- On that sealed candidate, require the single 18-variant governance event
  schema, exact negative pins for every retired council/roster,
  citizen-service, duplicate-event, and unreachable-event symbol, and
  byte-identical embedded/latest/current OpenAPI documents with matching
  manifest and version-index byte counts and digests plus clean externally
  signed provenance.



<a id="record-c4f1bd73968095db8fcdeac4a8cbef61f9f236994f1e2dfbccca7ae2022e86aa"></a>

<!-- Original context: Roadmap / Iroha configuration first-release closure -->
- Convert the remaining ordinary-input unwind surfaces in Governance and its
  SoraFS policy leaves, SoraCloud submission/posture, SoraNet VPN, content,
  Torii push/history/RAM-LFE, and DA/SoraFS parsing to the existing aggregated
  `Emitter<ParseError>` contract. Reject invalid bounds exactly instead of
  silently clamping or treating zero as a compatibility default.


<a id="record-79f28b80fef0958b690d85c67b9247097a7cbc4c733366eeff25414d3f944f44"></a>

<!-- Original context: Roadmap / SORA Parliament hardening -->
## SORA Parliament hardening



<a id="record-b126090e27e44d972215fd03a6cbb408d31b843176484f4314d7fca2a41e3de9"></a>

- Obtain an independent review of the exact timed-OVN arithmetic,
  Fiat--Shamir statements, aggregate-only opening, fixed-round secret
  exponentiation, threshold-BLS corruption model, key custody/rotation,
  implementation, release binaries, and CPU/accelerator target matrix. The
  publication-manifest verifier is implemented, but no external audit report or
  evidence archive is checked in or claimed.


<a id="record-b1c3046c07a2cf2455cbaae4e0906d1d12829e4d010a888d46d05b74d0e9ba17"></a>

- Qualify the implemented future-pulse draw and private-ballot attempt reducer
  on at least four peers: archive same-revision finalized-beacon batching,
  invitation sealing, authority-bound self-absence, conflicting and matching
  two-thirds public-finding endorsements, exact phase boundaries,
  missed-deadline classification, objective release failure,
  opening-deadline expiry, fresh-TLE retry, restart/restore replay,
  narrow-result fresh Confirmation Jury, stale-head supersession, exact-height
  enactment, and rollback-isolated execution failure. The reservation-bound
  carrier attestation, manifest-less certified-Fetch Phase-B completion, and
  ordinary/recovered exact-retransmission ownership-history refresh are
  implemented and covered by focused source/regression checks. Include the
  sealed QC/timeout-only pacemaker escape from a missing Validate sidecar and
  certified-view supersession of only an unprotected older wait. Fresh strict
  four-validator evidence for their resulting source is still required.


<a id="record-72443759a0ed625e23ee7343522fd03a7a54423bca44fc7914e4b4568fda9447"></a>

- Carry the completed sub-three hidden-electorate capacity path, bounded
  generation-16 exhaustion, live-candidate bond retention, atomic narrow-Policy
  Confirmation capacity decision, the persisted proposal-wide sixteen-redraw
  budget across successor attempts, sortition, and timed-OVN retries,
  permissionless exact-next proof-checked ballot-corpus chunks, account-rekey
  containment, and protected validation-fee restore checks through that
  four-peer matrix. Include a three-to-two survivor-dropout attempt that rejects
  before ballot acceptance or opening, reaches permissionless deadline
  `NoResult` without persisting an exact tally, and then follows the ordinary
  bounded retry path. Preserve the fail-closed terminal shapes and prove that
  retries, restore, and exact-height
  execution cannot strand a bond, an unfillable body requirement, an account
  identity, or fee admission behind missing provenance. The isolated target now
  contains six source-budgeted four-peer scenarios covering bond
  retention/release, Confirmation-capacity abort, exact stale-head supersession,
  fail-fast execution-failure unchanged-state isolation, state equality,
  restart, the lifecycle, and both mandatory threshold-beacon paths, plus one
  static bypass guard. Run all seven target tests from the settled candidate
  before treating the matrix as evidence.
  The same immutable-candidate matrix must cover same-block trigger lifecycle
  ordering and rollback, pre-effect gas reservation, nested-work retention,
  exact prepared-overlay and live-batch block-gas accounting,
  authenticated sealed-reveal alias recovery, replay-terminal QueuePlan
  obligation compaction and exact evidence, opaque contract-state boundaries,
  and sibling-carrier queue retirement across live cleanup and restart.
  Proposal-local runtime lookups now enumerate only the bounded canonical V1
  attempt-key space, while restore retains its complete-map rejection of corrupt
  key-to-attempt bindings. The derived exact required- and
  unavailable-pulse-slot indexes are maintained on attempt replacement/removal
  and rebuilt from authoritative attempts on restore; the candidate matrix must
  exercise bounded consensus lookup, late-pulse conflict rejection, and index
  reconstruction without weakening the complete restore contradiction scan.
  Certified attempts also use an exact derived enactment-height index so block
  construction never rescans historical reducer payloads. Candidate derivation
  is complete but fail-closed at 65,536 citizens and an 8 MiB canonical snapshot
  payload, with the same limits enforced at request admission and restore.
  Current QueuePlan evidence is composite: the full
  subset produced 34 passes and three stale-control failures, and the corrected
  controls then passed 4/4. It is not a clean single-run qualification result.


<a id="record-61b08c8160dc1540a94e4b5e5c2f99c40778770aede113a097b925847c042a08"></a>

- Candidate-qualify the exact Ready-Proposal-Sign producer-point preemption and
  the certified-response queue-refresh retry. The first patched four-validator
  rerun showed no recurrence of the former queue-cut fail-stop, but its
  threshold-key installation transaction timed out after 600 seconds under
  severe unrelated host contention, so it is diagnostic evidence rather than a
  liveness receipt. Repeat below-threshold install, epoch-boundary activation,
  and the full Policy-to-enactment corridor on an uncontended same-source
  four-validator candidate.


<a id="record-724ea1cde37e71b28330bf2d4ad4c6d5bac3116769bf252ba99cb3a26ac9ff9c"></a>

- Qualify the implemented live threshold-beacon partial-share transport,
  per-session runtime custody, threshold aggregation, candidate-effect
  assembly, and authoritative finalized-pulse persistence on at least four
  peers. Include mandatory Parliament demand batches and NPoS boundary slots,
  missing/invalid shares, selective withholding, domain separation, restart,
  idempotent retransmission, and key rotation. Prove that a missing threshold
  share stalls only the exact requested slot and that recovery finalizes that
  same slot without a redraw. The certified lifecycle
  now compare-and-sets the expected active
  predecessor and makes a block-`H` global key change effective at `H + 1`;
  qualification must prove that all requested pulses authorized by the parent
  state are resolved by the session active at the pulse height and
  cannot be reinterpreted through the post-block successor pointer.


<a id="record-79f102a91ccdc0a0ada4b23e96b7118be9a5cbcf89a6b2f96fbc8c192e963b7b"></a>

- Qualify the implemented timed-release operating seams: full-public-transcript
  release-context read, bodyless authenticated local partial request,
  runtime-only multi-session custody/provider injection, independent proof
  verification, canonical combine, and ordinary `FinalizeOpenedBallot`
  submission, plus the bounded public broker projection and separately
  revalidated projected-signer boundary. Exercise the implemented bounded
  operator CLI against up to 31 strict signer-peer roots: it exact-matches the
  immutable statement, verifies/de-duplicates every partial, canonicalizes the
  threshold, and re-fetches and revalidates the primary context and aggregate at
  the refreshed finalized height immediately before normal signed submission.
  Qualify the implemented certified
  public-session install, atomic active-pointer cutover/retirement, mandatory
  next-height activation, inclusive selection/expiry and use bounds, new-ballot
  selection guard, immutable session-to-ordered-roster/lifecycle binding,
  committed-use recount, and local-custody retirement guard against every
  committed ballot/retry deadline.
  Startup now derives the local seat separately for the active and every
  deadline-retained historical session and requires an exact non-signing
  key-session/transcript/seat lookup through the same signer. The authenticated
  broker operation and software custody implementation are complete, including
  independent result matching, surrounding requalification, mismatch poisoning,
  and fail-closed defaults. Source tests now exercise exact active-plus-retained
  daemon call sets, historical expiry, all three independent result-binding
  substitutions, and truncated replies. Qualify that source against the
  provider-neutral custody boundary. Software-backed signing and custody are
  sufficient; no HSM or hardware-backed custody is required. Custody
  implementations remain deployment-owned and are not Iroha profiles or
  adapters. Then demonstrate old-share
  retention/zeroization, restart recovery, peer
  authentication/rate limits, freshness expiry, canonical collection, and
  operator submission on at least four peers. Do not describe a point-in-time
  custody attestation as future availability, aggregate opening as operationally
  automatic, or the software adapter as secure erasure.


<a id="record-a3fbdee310c676f2b6c917765d105a1cb85feeb839ae4e6f66dbb5a9d86a6186"></a>

- Qualify the implemented Core-authorized pre-seal timed-OVN casting-context
  archive read and its maximum-4,194,304-byte canonical header-framed Norito
  `ParliamentTimedOvnCastingContextArchiveV1`. The archive is public diagnostic
  material and never a continuing authorization. Qualify the separate app-signed
  casting-proof route and exact ABI 23 proof-only C/JNI wallet surface. The native
  verifier must continue to consume an explicit immutable raw-network/checkpoint/
  context/ballot trust anchor, authenticate strictly advancing nonterminal pages
  for durable checkpoint promotion without seed access, verify terminal finality,
  the fixed witness, and membership, replay-validate the embedded archive, and
  exact-match its rederived compact binding before any seed-bearing operation.
  Preserve
  removal of every archive-only wallet export. Qualify Core's exact half-open
  phase-window and nonmonotone-schedule rejection, the generation-bound Android
  seed handles, and the Swift/Kotlin/Java immutable no-default trust-anchor APIs.
  Keep the aligned served OpenAPI and JavaScript/Kotlin/Java/Swift projections
  strict, including negative tests for malformed proofs, fake chains, wrong
  network/context/ballot anchors, non-advancing intermediate pages, intermediate
  pages at terminal seed-bearing entry points, archive substitution, and binding
  tampering, plus a positive multi-page checkpoint-promotion path. Cargo-qualify
  the bounded proof decoder and
  multi-registration native ballot path before treating the corridor as
  operational. Rebuild and execute same-source ABI-23 Swift XCFramework and
  Android native artifacts rather than treating parse, JVM-descriptor, or
  source-contract gates as native execution evidence; keep the packaged ABI-21
  XCFramework as a truthful blocker until it is replaced.


<a id="record-0d1a71631af7cd8cd0e8481f096b02608c6667df8343fc817ddd1be04d56934d"></a>

- Finish the generated public-contract closure from a settled candidate.
  Regenerate the static served OpenAPI authority and truthful provenance after
  the source and pinned Cargo input are sealed, then publish the exact
  ten-kind proposal, ten-kind no-result, six-action contract-lifecycle, body,
  and route inventory. Keep the retired equal Parliament ballot
  route and proposal-backed referendum/finalize/enact surfaces absent from the
  served OpenAPI as they already are from source and SDKs. Current dirty-tree
  artifacts and their dirty, unsigned manifests bind the current bytes but are
  not candidate evidence and cannot be promoted, even where focused enum/schema
  parity checks pass. Regenerate and byte-compare every mirror from the sealed
  candidate, then rerun the complete SDK matrix. Standalone
  referenda must remain explicitly separate from Parliament attempts, and
  automatic execution must remain only its non-submit-able audit outcome.


<a id="record-26202808faabbc85c7e9323f0c9717573a706b64e408fd21f31454ced24cde37"></a>

- Review and candidate-qualify the implemented aggregate-only Parliament alert
  rules for stuck attempts and deadline misses. Their five-rule `promtool` suite
  is green locally; restart and four-peer behavior still require evidence
  without identifiers, free-form labels, or private ballot material.


<a id="record-6456059b376a61f95d8b0d2a1b442c705c43d63defcafafbcdcb5d2a55d14f66"></a>

- Freeze and rerun the implemented 23-case threshold-BLS/timed-OVN Criterion
  and logical-allocation matrix from the same immutable candidate. The local
  evidence checker passes all 18 collected cases across 15 test functions, and
  byte-identical pre-merge allocation runs establish the harness contract, but
  do not replace fresh candidate measurements and an archived sealed report.


<a id="record-5b14a65e175bf0a2c16dd87d25af39cddf1dc8616e8b5aacf96a08f647403404"></a>

- Re-run and archive the configured state space exhaustively with pinned TLC
  2.19 plus the deterministic source/model contract from the immutable
  candidate, then pass focused data-model/Core/Torii tests, the legacy-codec
  guard, workspace tests, strict all-target Clippy, formatting, strict TLAPS,
  pinned Verus, chaos/soak qualification, and a clean externally signed release
  corridor. The changed model's pinned TLC 2.19 exact run passes with 17,303,220
  generated states, 8,896,344 distinct states, and depth 50; repeat and archive
  it from the immutable candidate before promoting it to release evidence. The
  deterministic source/model gate passes; the lifecycle corridor passes 15/15
  tests and 217 subtests; and the OpenAPI fail-closed suite passes 12/12. The
  dedicated target contains six four-peer scenarios plus one static bypass
  guard; exact candidate execution of all seven target tests remains pending.
  The authored, embedded, and versioned-current OpenAPI mirrors are reconciled at
  3,722,852 bytes with SHA-256
  `9f6fef02069e0cb30bbd2f89ba9f27355fe7685d8abae4681b832526656a7918`
  and BLAKE3
  `716617ce637f19f9c22981533f87d642b4e077604059a0743a1c7c6cf662637b`.
  Both manifests and the version index bind those current bytes, but remain
  dirty and unsigned; version verification is expected to fail closed until the
  bundle is regenerated from a clean commit and externally signed.
  The existing `Executable::IvmProved` ZK proof-verification and replay surface
  is preserved. Live-required and terminally unavailable Parliament beacon
  slots are tracked by exact logical-session/height derived indexes, rebuilt on
  restore and updated atomically on attempt replacement or removal; block
  validation and beacon production now use bounded required-slot lookups. Still
  pending are qualification of the exact certified-enactment and bounded
  candidate-snapshot indexes plus the active timed-OVN casting-candidate index
  under the four-validator matrix, immutable-candidate TLC replay and archival,
  independent review, signed OpenAPI provenance, full workspace tests and strict
  all-target Clippy, a same-source Swift native bridge, and one immutable release
  candidate. Model checking and these focused source checks remain regression
  evidence, not production-readiness evidence.


<a id="record-8fcda760efc885594c9a012aedddb416d4f517dba4d499696f5bfa9a3e323f88"></a>

- Candidate-qualify both contract-governance effects and the append-only
  emergency-hold retrospective. Exercise exact revision/head supersession,
  owner and delegation changes, activation/deactivation, ABI/artifact mismatch,
  hold expiry without erasure, early and cross-bound retrospective rejection,
  certified clearing, replay rejection, complete event post-state, and a later
  independent hold across persistence restore and four peers.



<a id="record-5f7f51006b1ba48a5e8bb9f430ce709abd861c311c61acb484ba50ce657e5dba"></a>

<!-- Original context: Roadmap / ZK algorithm release qualification -->
- Define an explicit governance recovery/archive policy for abandoned or
  corrupt Kaigi relay rows whose relay and canonical rekey successor cannot
  authorize `UnregisterKaigiRelay`. Keep exact metadata-key/embedded-ID binding,
  fail-closed corruption handling, domain pinning, and account/domain teardown
  guards until that policy exists. Supporting aliasless relays would likewise
  require a signed home-domain field or a protected relay-to-home index; do not
  restore global allowlist discovery.


<a id="record-9c4e75e4776f063b9d01264eb702608774a102b2ed62306fe4c4d15f65d3bf73"></a>

<!-- Original context: Roadmap / Public Taira node onboarding -->
## Public Taira node onboarding



<a id="record-c987b9b3a4692352dd6ebaee012b5edbf36c1bafff1358c0105ae9fa584b7437"></a>

- Treat Taira as the persistent public testnet. Provide one supported node-init
  flow: `iroha taira join --data-dir <owner-only-directory>`.
  The command must discover the canonical bootstrap URL by default, consume a
  signed public bootstrap manifest, generate local node keys, write and validate
  the complete runtime configuration, and start the node with sensible
  first-release defaults as a permissionless observer. Validator activation is
  a subsequent on-chain transition through the existing staking registration
  and peer lifecycle after synchronization; do not introduce an operator-issued
  admission-token format or a second bootstrap path. Generated keys remain
  runtime-only. Joining must not require running the disposable four-validator
  or Inrou canary corridors.


<a id="record-b3a0ce5d0052bc498523f4314472fd05906891ac86d1bde2cdd7af721c60daa1"></a>

- Publish the current public Torii/MCP roots, genesis trust anchor, seed peers,
  permissionless observer policy, validator staking/peer activation
  requirements, and upgrade procedure as one versioned bootstrap bundle. Fail
  closed on an expired, unsigned, or
  network-mismatched bundle. Do not expose a matrix of raw genesis, peer, or
  network-ID flags on the public join path; custom networks use their own
  explicit tooling. Keep ordinary observer setup to the single command above.



<a id="record-b2ca62f7139af639dfda909e19d4365e7b818fab2c68e953d3ecde9f4a2d0025"></a>

<!-- Original context: Roadmap / JavaScript governance private-file release closure -->
## JavaScript governance private-file release closure

The cross-platform ABI v1 implementation, fail-closed publication probe,
macOS execution tests, and Windows cross-target compile checks are complete.
Release closure still requires executing the secure-storage tests on the
Windows packaging runner, creating the authoritative clean signed source commit
and signed parent, regenerating its exact sealed lock and source-tree digest,
and rebuilding the wallet's signed native artifacts from that one provenance
set. Cross-compilation evidence must not be promoted as Windows runtime
evidence.



<a id="record-dda62b5c682c2832a296e048869628f6428f1db9493210eafd2423f91bb0f91a"></a>

<!-- Original context: Roadmap / Alias/SNS release evidence -->
## Alias/SNS release evidence



<a id="record-f443f268bfec3679cea720d732b7460b299c5c80d3222f46c607e5b2ec87221f"></a>

- Run the Kotlin and mirrored-Java suites when a Java runtime is available.


<a id="record-7d2d3e901f7269b7aae357759d0f06be0ce4bce7b196a2b8cf23ca947f419524"></a>

- Replay the already-green complete Apple/Swift package suite from the signed
  final Norito bridge artifact on the release machine and device matrix.


<a id="record-9c962731bc03e974d717fe460bd192092ea0c5a2b2edcef9ad9820467dac1a5c"></a>

<!-- Original context: Roadmap / SORA Economic Constitution -->
## SORA Economic Constitution

**Status:** specified design; implementation pending.



<a id="record-447c12bd655a416f5d7637193153b3417124fdc9d68f51052db0a9aeca3f32dc"></a>

- Complete the SORA purchasing-power basket, multi-source oracle, intervention
  band, balance-sheet accounting, liquidity/solvency coverage, and reserve
  regime specifications before enabling a stability claim for XOR.


<a id="record-45877edb35d6540379515e157eb7eb5791958f40fb5b529441c712d67197b0c4"></a>

- Implement Phoenix as capped, term-dated subordinated capital certificate
  series with no demand redemption, governance rights, XOR collateral role, or
  reserve value.


<a id="record-32d375c5d5610de9c46cea2bc0cfe05e6fb7c36c68d7eab53dac84370e413a31"></a>

- Pilot one ring-fenced Producer Credit Facility with prefunded milestones,
  direct supplier payments, junior/senior loss allocation, portfolio limits,
  and explicit default/restructuring behavior.


<a id="record-d762e6c3bab3508d1256cece015d13b0ff89681ce0ae34a89c15de39b8e6e713"></a>

- Add Routine, Standard, Constitutional, and Emergency governance lanes with
  bounded authority, expiry, challenge, and rollback rules.


<a id="record-d46d388087fb90aabe7c786387fd10c88097403ed279905b8b6727e3a8638e9e"></a>

- Build reproducible stress and agent-based simulations for monetary runs,
  correlated producer defaults, oracle capture, Phoenix repricing, identity
  farms, cartels, external short attacks, and challenge flooding.


<a id="record-2dfd0444e926cae83058c4f14a41cf36542aa844bdf657b0132e7fd32e483015"></a>

<!-- Original context: Roadmap / Community and Governance -->
## Community and Governance

**Status:** active growth work.



<a id="record-322f5aca8bcd08166074122c72c013deaf5de5d1c645961e7d58155a53b64221"></a>

- Use the official X account, [`@hl_iroha`](https://x.com/hl_iroha/), as the
  primary public cadence for recurring X Spaces, demos, and roadmap Q&A.


<a id="record-ef2ce6cee9432b964ea1c853c39c74f3739a5bab351a31910648187441336b54"></a>

- Publish recaps or recording links when available so contributors can follow
  progress asynchronously.


<a id="record-ca4aca6b11501f58a73a7d074f461ef82850cfe9fdeed0d2ff5b8c3ade2e424c"></a>

- Grow contributor and maintainer diversity by turning testnet interest,
  CBDC/regulated-finance adoption, and LFDT ecosystem connections into repeat
  reviewers and subsystem owners.

**Next checkpoints:** monthly X Spaces cadence, clearer contributor onboarding,
public follow-up notes for LFDT governance review items, and timed-OVN/
threshold-release hardening for SORA Parliament policy juries.

