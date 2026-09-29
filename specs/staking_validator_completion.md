# Validator staking completion

This is the source-coupled completion record for the September 22 staking work.
It separates instruction implementation from production validator qualification.
No live deployment or release readiness is established by this record.

## Active implementation goal

The complete first-release implementation and qualification goal is active.
Implementation and validation use only the `optimizations` branch in
`/Users/takemiyamakoto/soramitsudev/iroha`. Results from another checkout do not
qualify this candidate; all outstanding gates must run against this source.
The selected design replaces retired layouts and paths; backward-compatible
decoders, aliases, shims and parallel implementations are prohibited.

| Milestone | Completion gate | Current state |
| --- | --- | --- |
| Custody and lifecycle | All focused staking, reserve, snapshot and restoration controls pass | The latest pre-merge focused Core run passed 182 and failed one strict snapshot fixture. Its committed NPoS/XOR policy repair is staged. The later isolated allocation cut passes 513 tests; integrated custody, rollback, fee and snapshot gates remain open |
| Canonical XOR | Genesis-pinned network XOR funds bonds, rewards and withdrawals; no synthetic staking definition or implicit production minting | Required immutable network XOR pin and canonical defaults implemented; production implicit minting removed; integration validation pending |
| Authority and election | Separate key generations and scheduling epochs; freeze E+2 membership at E and prepare through E+1 | Generation/authorization, authenticated native epoch graph and pristine E+2 boundary capture are integrated; Core check14 passed before the subsequent seal/storage changes; fresh compilation, runtime and network qualification remain open |
| Atomic transition | All target seats ready; current exact quorum certifies activation or retention and cancellation; restart preserves both sessions | Native prepare/activate/retain source capture, signed control/Pasta, atomic application barrier and current/predecessor restore join are integrated; snapshot chain/network identity is checked before typed restore, but positive-height startup and real restart/transition qualification remain open |
| Monetary fees | Exact signed effects, bounded claim processing with retained dust, and native execution equality checks | Source implementation present; Core, enacted-fee and network qualification pending |
| Production progress | One funded original execution reaches durable Apply; native lane runner is the sole production owner | The integrated cut retains original-pool native context archival and execution across refusal/publication. The earlier native component run passed 361 with two opt-ins unrun; it is not current integrated qualification. Original-tip deterministic history and the permanent storage fail-stop gate are integrated. Nested World/DA funding, complete native source retirement, positive-height startup/recovery, RS16 and liveness qualification remain open |
| Operator/client delivery | Canonical signing, provisioning, status, SDK and fixture workflows | Native status and stopped-Kura evidence collection are integrated; Python/JS status retirement is under validation. Current native evidence Python tests pass 153; the earlier 812-test result belongs to the superseded protocol overlay. Kotlin/Java codec component checks pass. The nine-field signed transaction layout and Rust production callers are integrated without the old admission slot. Genuine native fixture regeneration, the rebuilt Swift bridge and full SDK delivery remain open |
| Unchanged network qualification | Real 4→7→4 network, faults, replay, restart, penalties, rewards and full withdrawal; maintained formal/DA/workspace/SDK gates | Pending |

Rewards remain explicit treasury-funded distributions. Committee preparation
takes one full epoch. Every boundary advances the scheduling epoch; certified
retention keeps the current key generation without claiming forward security.
Missing target readiness cannot change the frozen roster in place. Missing the
current quorum does not authorize weakened voting rules.

Canonical XOR means the asset authenticated for the particular network, not a
second token named XOR. Taira's public identity and an operator-provisioned Nexus
identity are not interchangeable. Disposable-network allocations are explicit
genesis test allocations and are not claims of mainnet monetary value.

The [September 28 reconciliation record](../docs/history/2026-09-28/validator-staking-native-review.md)
binds the recent scoped evidence and records the remaining source-owner and
resource-funding gaps. The separate merge is now resolved and staged. The review is now applied on `optimizations`, preserving the staged merge index.
Core library check14 passed with 294 warnings before the latest seal/storage
changes. Fresh compilation, deployment allocation repair and runtime/network
qualification remain open; no integrated release completion is claimed.

## Implemented candidate under validation

The 2026-09-28 lane-owner audit requires the current `lanes::LaneRunner` and
`SumeragiLaneMerge` path from [the lane specification](sumeragi_lanes.md).
Superseded Native Decision/QueuePlan runtime, proof and fixture integration cannot
count as completion or be activated beside it. Preserve its substantive custody
and execution assertions on the actual production owner. The canonical execution
proof/archive and strict current/predecessor snapshots now use complete
`SumeragiLaneState` in the integrated source. Native routing uses the same
committed policy for direct inputs and merged suffixes. Shared lane evidence
verification reproduces admission and checks the actual native quorum. The
compiler/runtime gates, physical funding and old source-owner removal remain open.

- The global native scheduler, signed-genesis committee builder and historical
  committee reader now require the exact `3f + 1` geometry (4 through 31).
  Restored windows also validate retained entries and canonical signer order.
  The schedule retains the original authenticated proofs and roster after genesis;
  mutable candidate registrations and key expiry cannot change voting authority.
  The complete native epoch graph, pristine election capture and boundary barrier
  compiled in an earlier source cut. Core library check14 passed before the latest
  native seal/storage changes; their fresh compilation, unit/integration and
  unchanged network gates remain open.
  Focused runtime and real transition qualification remain
  pending; signed control and Pasta integration are still in progress.
- Account-owned lifecycle and signed peer consent: `isi/staking.rs` in the data
  model and Core, Initial/default executor dispatch, canonical instruction
  registry and generated record fixtures. Consent binds network and exact
  activation tenure; rebind also binds the previous peer. Ordinary peer
  administration retains its permission gate. Fresh candidates enter the future
  election pool with exact funded XOR custody. The native scheduler now retains
  the authenticated roster and original proofs instead of promoting live
  Validator-role registrations; its candidate-pool and retention tests await Core
  validation. The integrated pristine boundary producer freezes the exact E+2
  target for a complete E+1 preparation interval; its signed control/Pasta and
  network gates remain open. Requested election exit and actual end of service are distinct fields;
  retention extends pending-unbond slashing and release heights.
- CLI candidate registration, signed rebinding, bond/delegation, scheduled and
  finalized unbond, reward recording and claiming. Runtime peer signing inputs
  use the existing owner-private file loader and remain outside the repository.
- Treasury-owned fee-funded reward distributions. Exact source-asset reserves
  protect unpaid rewards through transfers, burns, aggregate batches and
  snapshot current/predecessor restoration. Per-validator custody pins the exact
  scoped source asset; an aggregate index reserves all bonded and pending-unbond
  funds in addition to rewards. Only authenticated deposits, matured withdrawals
  and verified slashes change those reserves. Generic debits cannot consume
  bonded escrow, including a shared fee/stake account. Restore validates both
  ledger reconciliation and combined backing; deletion and account migration
  preserve the retained source. Configuration and alias drift cannot redirect a
  withdrawal, and incompatible new deposits are rejected. Epoch-zero and
  deferred small claims remain payable; changing fee policy cannot change an
  existing entitlement's source.
- Beacon startup authenticates the exact installed session and checks the local
  provider's non-signing capability for its actual seat. A present provider
  handle alone does not establish usable custody.
- The disposable scenarios in
  `integration_tests/tests/sumeragi_npos_committee_transition.rs` cover an
  eight-candidate pool, missing incumbent keys, missing target custody, and real-XOR
  4→7→4 transitions. Their native-journal/context migration is staged; compiling and running that
  unchanged candidate remain required. The
  harness source is not evidence that those transitions currently work.
- Candidate keys require BLS peer consent plus possession of both generation-bound
  Pasta keys. Every prepared seat additionally proves custody of its exact beacon
  share under the finalized DKG transcript and complete transition context. A
  lifecycle certificate finalizes a pending beacon; only certified committee
  activation retires the old key and activates the new one. Genesis bootstrap
  requires the exact current genesis authority.
- The seven-seat Core fixture exercises fresh dealer commitments, distinct
  private shares, transcript validation, native custody import and real
  threshold signatures with the signed-genesis network XOR. It constructs
  dealer secrets centrally for deterministic testing only. The first-release
  DKG data model and reducer instead carry signed attempt-bound recipient keys,
  dealer commitments, encrypted dealer-to-recipient edges and signed recipient
  acceptances; no public complaint or private-share reveal record remains.
  The daemon's per-seat command path still needs independent broker custody,
  genesis orchestration, restart and network qualification. The Core fixture
  does not establish a 4→7→4 network transition.
- Current and predecessor restoration require the exact authenticated preparation
  and terminal boundary history, including missing-row detection and exact beacon
  activation state. A snapshot without the required retained finality is rejected;
  authenticated snapshot-bootstrap history delivery remains to be qualified.

## Execution inputs on the Sumeragi node

Staking, committee preparation and threshold-key lifecycle instructions execute
inside blocks, so they read only committed state that every node shares, never a
node's own certificates (certificates are per node, `specs/sumeragi.md` §12.7):

- The scheduling epochs are the committed NPoS epochs `[e·L + 1, (e + 1)·L]`,
  with `L` the signed, immutable `SumeragiNposParameters.epoch_length_blocks`.
  Frozen preparations in World fix their own target intervals after the current
  epoch and must follow it (selection at the preceding epoch's end, contiguous
  start). Global-lane tenure uses these intervals; other lanes use the epochs
  alone.
- The initial KAGEMUSHA authority is the signed generation-zero authority of
  the genesis handshake metadata bound to the network. The native schedule
  carries the complete authority-generation and epoch-authorization graph;
  certified boundary application advances that graph atomically. The shared
  proof reader and Pasta signing integration remain under qualification.
- A threshold-key lifecycle certificate at height `h` is signed by the committee
  World's lag-2 consensus schedule holds for `h` (committed in `R_{h−2}`), in the
  core's canonical order; Torii admission checks the next height against the
  same entry.

## Outstanding protocol and runtime outcomes

| Outcome | Exact dependency and completion criterion | Owners |
| --- | --- | --- |
| Dynamic election and mint-finality keys | The native E+2 selector/producer, complete retained epoch graph and immediate next-epoch application barrier are integrated. TODO: finish signed control and Pasta integration and qualify complete preparation and certified retention without fresh incumbent keys on actual networks. Registration alone must never add voting authority. | Core/data model, KAGEMUSHA and deployment |
| Prepared beacon transition | The signed encrypted all-edge DKG model and reducer bind the frozen exact roster and fail closed before finalization if an edge or acceptance is absent. The deterministic Core fixture still constructs secrets centrally; daemon per-seat custody, authenticated exchange, genesis orchestration, current/pending session restart and atomic activation need qualification. Parliament can require an early pulse independently of the next epoch-end election pulse. Do not bypass finalized pulse or certificate checks. | Beacon, Parliament, Sumeragi and daemon |
| Staking under an enacted DS-transfer validation-fee policy | Exact signed monetary bindings and native effect checks are implemented. The policy counts every actual signed real-XOR staking transfer leg under `PerQualifyingTransferInstruction`, even when the DS fee asset differs; principal cannot satisfy the fee coordinate. Reward reservations and claim dust with no transfer leg incur no transfer fee. The focused Core fee suite passed 110/110 on 2026-09-23. TODO: qualify bounded claims, multisig/proved overlays, and the canonical native execution owner on an unchanged network candidate. Ordinary Nexus/PipelineGas charging already uses signed `FeePaymentIntent`; staking-specific runtime qualification of those payer bounds remains outstanding. | Core/native execution and fees |
| Production liveness | Complete the original Validate-to-Apply owner, admitted resources, durable publication and autonomous lane runner together. TODO: close the silent-initial-author counterexample and retirement/restart cuts in `sumeragi_liveness_redesign_goals.md`; a second signer or local retry bypass is not a completion. | Core/Sumeragi, Queue, Kura and formal owners |
| Reward allocation | The selected policy is explicit treasury-funded canonical-XOR distributions. TODO: qualify funding, signed recording and bounded payment together. Automatic participation formulas, commission and issuance programs are outside this implementation. | Treasury/governance and Core |
| Network qualification | TODO: one unchanged candidate proves admission, prepared 4→7→4 rotation, queued Parliament pulse, missing target signer, all-seat restart, replay rejection, rewards, slashing and final withdrawal. Run the maintained fault/DA/formal gates and complete workspace checks. | Integration, release and subsystem owners |

### Native epoch boundary contract

The selected integration changes the native lag-2 membership contract directly.
If B ends epoch E, the B+1 configuration is explicitly unavailable until the
incumbent exact quorum certifies B and its original execution is applied. The core
must not propose, vote, time out, or establish signing authority under B+1 before
that application. Ordinary chain-parameter scheduling retains its lag-2 rule.
Restart and catch-up must preserve the same pending boundary and reject a
conflicting replacement. No provisional B−1 election authorizes a transition.

B captures its election and activation inputs before transactions, including the
authenticated beacon pulse, exact custody and the complete frozen target. That
checked view guards stake obligations throughout the same original execution;
transactions in B cannot remove selected custody or rescue a missed preparation
cutoff. Applying B publishes the E+1 authorization, complete authority and beacon
disposition together, and first inserts the immutable E+2 preparation. Missing
target readiness produces certified retention and cancellation; a later attempt
uses a new transition and transcript. Missing the current quorum remains a halt.

Each preparation must freeze the exact network-pinned XOR definition, global
balance scope, scale-nine precision, positive self-bond and nomination floors,
maximum committee size and target epoch length. Later configuration changes
govern subsequent selections. Current custody and complete credentials still
must satisfy the frozen attempt at activation. Mandatory finality-authorized
slashing remains executable; voluntary withdrawals and rebinding cannot release
the boundary's retained obligations. The native selector ranks already eligible
candidates uniformly using network, selection epoch, target epoch, authenticated
seed and canonical peer identity, then freezes the largest allowed exact `3f+1`
committee. Stake gates eligibility and does not multiply votes. Future Pasta
keys and beacon shares are prepared during E+1.

The unimplemented concentration, seat-band and entity-correlation settings have
been removed from canonical parameters, profiles and genesis templates. Their
old fields are rejected, including zero-valued fields. Frozen policy/member
records and their native producer consumers are integrated. Earlier Core library
compilation passed; the subsequent native seal/storage changes require fresh
compilation and runtime qualification. This contract does not attest completed rotation.

The canonical epoch identity must bind the complete generation, authorization,
ordered roster and original PoPs, network, mode and fresh leader seed. Every
signature must consume its epoch/context binding, and topology must consume the
fresh seed even when membership is retained. An empty boundary still requires its
authenticated boundary witness and Pasta seals; forced-empty progress cannot
strip mandatory attestation. These connected changes and their replay, restart,
formal and network tests remain under implementation.

### Distributed beacon ceremony cutover

The first-release source candidate uses an all-edge Das–Ren ceremony over the
frozen target, with no public complaint or private-share reveal layout.
Each target seat publishes a fresh hybrid encryption public key signed by its
actual BLS consensus key. The signed key record binds the network, transition
ID, authority generation, complete DKG session, roster hash and recipient
index; the private key stays in that seat's broker custody. One dealer process
per seat generates only its own polynomial, signs its public commitment and
encrypts one verified contribution to each recipient. The private envelope is
signed by that dealer and uses canonical associated data consisting of a domain
tag, complete encoded DKG session, transition ID, authority generation, dealer
and recipient indexes, dealer commitment hash, recipient key digest and the
authenticated sharing height. The recipient checks that signature, exact
context and share equation, then signs one acceptance for the exact edge.

Use `iroha_crypto::hybrid::{HybridKeyPair, encapsulate, decapsulate}` for the
existing X25519/ML-KEM-768 key exchange and
`iroha_crypto::encryption::SymmetricEncryptor<ChaCha20Poly1305>` for nonce-bearing
AEAD over the associated data. The public reducer requires all `n` distinct
commitments and all `n²` distinct signed acceptances before the finalized
acceptance cutoff. It derives the transcript from those exact commitments; a
recipient aggregates only its locally decrypted contributions. A missing,
invalid, replayed or late edge cannot finalize the target. The current exact
quorum instead certifies retention and cancellation, and a later election uses
a new attempt and transcript. This keeps current-chain progress but lets one
uncooperative target block that transition's activation.

Each dealer and recipient needs a durable, owner-private, one-shot attempt
journal keyed by network and transition ID. It must retain the original public
commitment, encrypted outbound envelopes, accepted edges and local aggregate
across restart, reject conflicting replays and never regenerate a polynomial
for the same attempt. The dealer polynomial can be erased after every original
outbound envelope is durably stored. The source cutover spans DKG records in
`iroha_data_model::consensus`, public reduction and share verification in
`iroha_core::beacon`, and per-seat broker custody and authenticated exchange in
`irohad`. The public complaint/reveal fields and the all-secrets production
coordinator are retired. CLI and disposable-network orchestration remain to be
qualified. Exact incumbent-quorum
transcript finalization, all-seat readiness and boundary activation still need
their separate authenticated evidence; an all-edge receipt is not activation.

The global committee remains exactly `3f + 1` with `2f + 1` equal validator
votes. Observers and admitted candidates cannot pad quorum. Signed RS16 DA,
canonical replay, source-bound evidence and the offline monetary-authority
policy remain mandatory.

## Validation

The [September 27 native quorum checkpoint](../docs/history/2026-09-27/validator-staking-native-quorum.md)
records 329 ordinary consensus library/simulator passes and both separately run opt-in passes, five
focused strict mutation kills, and the allocation custody passes from this
checkout. Global committee geometry and exact certificate cardinality are
implemented. Core compilation and real-network transition/monetary qualification
remain open; no historical or simulator result closes those gates.

The custody checkpoint passed 141 focused Core tests with no failures or skips.
The command selected staking, reward reserves, pinned custody, admission, snapshot,
configuration restoration and fee guards; the source checkpoint and complete log
are retained outside the repository under `/tmp/iroha-staking-custody-checkpoint`
and `/tmp/iroha-staking-core-checkpoint.log`. This result predates the authority,
monetary-plan and XOR schema changes now in progress.

Validation is in progress. The earlier focused model, executor and codec staking
selection passed 31 tests (the deliberate fixture generator remained ignored),
and the complete default-executor library passed 180 tests. On 2026-09-23 the
typed current-protocol printers recaptured the privacy qualification record,
the staking monetary and peer-consent records, and the updated evidence nested
in penalty cancellation. The canonical instruction-record selection now passes
323 tests with zero failures and one intentional ignored printer. Its 321 rows,
357 cases, strict identity checks and four-frame roundtrips remain enforced;
no old decoder or identity fallback was added. The focused Core fee suite passes
110 tests with no failures on the same monetary source. The current combined
Core and network candidate still needs its own runtime qualification.

The six-package test-target check passed before the subsequent pinned-stake
custody and global-pool guard additions. Final Core, daemon, CLI, Torii and network
validation is still pending. Compilation and source checks do not establish
live finality or the outstanding outcomes above.

Current integration work also includes explicit reviewed validator allocations for
`xtask kagami-profiles`; signing must fail on an unfunded manifest rather than
reintroducing implicit minting. The Rust-authored seven-row validator/staking
Norito fixture and Kotlin typed decoder pass their focused three-test consumer
suite, including strict quantity-decimal rejection. The committed seven-row
fixture also passes a Rust-authored byte-for-byte pin test. The focused
JavaScript staking codec passes three tests covering exact consent, monetary
roundtrips, rejected retired JSON shapes and unsafe integers. The full Kotlin core suite
fails 385 of 1,587 tests without the required same-source ABI-23 native bridge
and Kotlin fixture-generator executable; that result is not an SDK pass. Swift
source typechecking and a standalone Rust-fixture and strict quantity-decimal
runtime smoke pass; its full package test still requires a rebuilt ABI-23 local
bridge. CLI plan-file signing remains an intermediate workflow; complete SDK
and operator qualification
are outstanding. The new Kotlin/Swift source and shared fixture are in both
maintained SDK source closures; their 25 focused Python closure/OpenAPI-pin
controls pass. The maintained multilane formal structural/source-binding checker
passes after rebinding the current Native preparation and State merge owners;
the focused mutation suite and TLC/Apalache runners remain separate, and the
checker must be rerun after the production-source cutover.

The updated bridge successor regression passes on the current data-model source:
one focused test rejects a self-consistent, re-signed authority, epoch authorization,
or execution-policy substitution before hostile BLS verification. The retired
layout scan is driving remaining generated schema and SDK fixture replacement.
The funded Native validator's focused service selector passes 15 tests, including
original-candidate readiness refusal and retry. Complete pre-execution admission,
the production runner cutover and an unchanged-candidate Core rerun remain open;
these focused checks do not qualify committee transitions or staking execution.

The allocation-control checkpoint passed all 481 Concread library tests. The MV
checkpoint reached 246 passes and one failing new restore-footprint oracle; the
oracle was corrected to account for retained inline leaf capacity, and its rerun
remains pending. These controls do not close production N0: native waiter/control
allocations, complete original World payload admission, restoration and sole-owner
production cutover remain open. Current Core integration is rebuilding after the
wire and lifecycle replacements; no unchanged-candidate qualification is claimed.
