# Validator staking completion

This is the source-coupled implementation and qualification plan for validator staking.
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
| Custody and lifecycle | All focused staking, reserve, snapshot and restoration controls pass | Focused Core custody, rollback, fee and restoration controls pass on the integrated test artifact; CLI, daemon and unchanged-candidate network qualification remain open |
| Canonical XOR | Genesis-pinned network XOR funds bonds, rewards and withdrawals; no synthetic staking definition or implicit production minting | Immutable network XOR pin, global scale-nine genesis/operator validation and early staking/reward custody checks implemented; production implicit minting removed; current-candidate component and integration validation pending |
| Authority and election | Separate key generations and scheduling epochs; freeze E+2 membership at E and prepare through E+1 | Generation/authorization, authenticated native epoch graph and pristine E+2 boundary capture are integrated; current Core, runtime and network qualification remains open |
| Atomic transition | All target seats ready; current exact quorum certifies activation or retention and cancellation; restart preserves both sessions | Native prepare/activate/retain source capture, signed control/Pasta, atomic application barrier and current/predecessor restore join are integrated; snapshot chain/network identity is checked before typed restore, but positive-height startup and real restart/transition qualification remain open |
| Monetary fees | Exact signed effects, bounded claims with retained dust, and principal distinct from assessed fees | Monetary plans and native effect checks are present. Required explicit fee-reward claim bindings and additive shared custody are implemented; canonical reward-currency admission, current-policy integration, multisig/proved execution and paid network coverage remain under validation |
| Production progress | One funded original execution reaches durable Apply; native lane runner is the sole production owner | Native execution/context retention, deterministic history and Kura's fail-stop gate are integrated. Complete original-pool funding through World/DA/proof graphs, publication, replay and retained-generation reclamation remains open |
| Operator/client delivery | Canonical signing, provisioning, status, SDK and fixture workflows | Candidate and credential commands and native evidence readers are present. Same-source native artifacts, regenerated fixtures, complete SDK consumers and operator network qualification remain open |
| Unchanged network qualification | Real 4→7→4 network, faults, replay, restart, penalties, rewards and full withdrawal; maintained formal/DA/workspace/SDK gates | Pending |

Rewards remain explicit treasury-funded distributions. Committee preparation
takes one full epoch. Every boundary advances the scheduling epoch; certified
retention keeps the current key generation without claiming forward security.
Missing target readiness cannot change the frozen roster in place. Missing the
current quorum does not authorize weakened voting rules.

The current production DLMM pool artifact is 80,629 bytes. Its nested artifact
charge alone is 5,160,256 gas at the existing 64-gas-per-byte price, exceeding the
default four-million-gas block limit before execution. Private numeric-literal
helper folding and a shared swap-direction body reduce the artifact without
changing its public interface or gas prices. The component fixture authenticates a finite eight-million-gas limit in
its original signed genesis. TODO: finish optimizing and qualify the enacted
payout under the default policy on the disposable network; the component limit
does not establish that the default policy can execute the payout.

Canonical XOR means the asset authenticated for the particular network, not a
second token named XOR. Taira's public identity and an operator-provisioned Nexus
identity are not interchangeable. Disposable-network allocations are explicit
genesis test allocations and are not claims of mainnet monetary value.

Kagami generation and profile verification require global scale-nine XOR for
both Taira and an operator-provisioned Nexus identity. Core's shared
`state/network_xor.rs` guard binds the committed currency before staking deposits,
reward claims and fee-policy use. Retained custody and current/predecessor
stake/reward restoration reject incompatible definitions and scoped asset IDs
without rewriting the source. Positive, substitution, scope, precision and actual
transaction-rollback controls are implemented; their current-candidate gates
remain open.

Focused Core compilation and custody/lifecycle checks pass. Complete resource
funding and unchanged-candidate real-network transition/monetary qualification
remain open.

## Implemented candidate under validation

The production owners are `lanes::LaneRunner` and `SumeragiLaneMerge`, specified in
[the lane specification](sumeragi_lanes.md). Custody and execution tests must
exercise these owners. The canonical execution proof/archive and strict
current/predecessor snapshots use complete
`SumeragiLaneState` in the integrated source. Native routing uses the same
committed policy for direct inputs and merged suffixes. Shared lane evidence
verification reproduces admission and checks the actual native quorum. The
current compiler/runtime gates and complete resource funding remain open.

Canonical executed blocks now use one `SharedSignedBlock` control admitted from
the original allocation pool. Certification moves the existing block graph into
that reserved control; publication, Kura and authenticated readers retain the same
handle. Local history allocation refusals remain typed through lane anchors,
restoration and DA reconstruction. A refused DA reconstruction publishes no
partial index and caches no invalid verdict. The control-owner regressions do not
qualify funding of nested decoded graphs or the full Validate-to-Apply path;
those obligations and current-candidate Core/network execution remain open.

Release waits use reusable registrations prepaid from the original pool;
there is no zero-argument or first-poll allocation path. The driver retains each
prepare, append, commit, payload, witness and control refusal with its exact work
and source. Its fixed 1,024-sender input bank and waiter controls are prepaid;
inline control values retain their complete context and a local occurrence token
through worker moves. Context cancellation and shutdown unlink affected waiters
before releasing original resources. Pending wire admission retains partially
admitted bytes and its typed refusal, and defers original-pool refund callbacks
until ingress mutexes release. Source-less failures use bounded backoff. The
charged wake unparks the driver without allocating a channel message.

Completed replay keeps its exact receipt through encoding refusal. The serialized
executor and startup replay error preserve original storage and publication
refusals instead of converting them to strings. Cold World-root verification
retains the actual storage refusal separately from a stored-root mismatch.
Native beacon readiness, attestation, driver and subscription startup failures
also retain their concrete errors through the serialized worker and node boundary.
Committed-history startup assembly preserves original cold-read refusal for both
global and lane callers; absent or corrupt history remains a separate completed
failure.
Connected regressions exercise
retirement after success, actual archive failure and post-publication unwind.
These implementations require current-candidate compilation, regression and
mutation qualification; component passes do not qualify the whole driver or network.

TODO: complete original-pool funding of nested execution, proof and startup graphs,
including beacon transcripts and cryptographic scratch. Polling, rearming, cancellation
and release must remain allocation-free after admission. Physical-history and State
readers must retain actual release owners, with callbacks outside enclosing fences.

- The global native scheduler, signed-genesis committee builder and historical
  committee reader now require the exact `3f + 1` geometry (4 through 31).
  Restored windows also validate retained entries and canonical signer order.
  The schedule retains the original authenticated proofs and roster after genesis;
  mutable candidate registrations and key expiry cannot change voting authority.
  The native epoch graph, pristine election capture, signed control/Pasta paths
  and boundary barrier require current-candidate component and real transition
  qualification.
- Account-owned lifecycle and signed peer consent: `isi/staking.rs` in the data
  model and Core, initial executor dispatch, canonical instruction
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
  `integration_tests/tests/sumeragi_npos_committee_transition.rs` define pools of
  five, eight and eleven candidates, missing incumbent keys, missing target
  custody, and real-XOR 4→7→4 transitions. They use the native finality journal
  and restart all peers while provisioning pending credentials. Setup without
  an actual network fails the qualification. The scenarios explicitly exclude
  an enacted retail validation-fee policy. The complete rotation now includes
  funded treasury rewards, exact signed claims and full withdrawal after retained
  liability expiry, with separately settled native fees; compilation and network
  execution are still required. Slashing, Parliament pulses and the connected
  enacted-policy scenario remain open.
  Compiling and running the harness on one unchanged candidate is required.
- The finality-owned lane penalty component
  `native_lane_original_genesis_escrow_is_debited_only_by_delayed_authenticated_admission`
  now covers signed full-unbond scheduling, a delayed physical XOR transfer to a
  distinct slash sink, reduced pending liability, and repeated original-chain
  replay. Its exact remaining withdrawal is rejected while the unchanged global
  and lane seats retain custody. This extension is under validation; successful
  withdrawal after actual replacement remains transition/network qualification.
  Torii exposes evidence reads, not an external evidence-submission endpoint;
  the Initial executor admits manual slashing only in genesis. An offline proof
  cannot currently drive the disposable network's finality-owned penalty through
  an existing external submission command. Keep mandatory-slash simulator and
  component evidence separate from network qualification; do not add a manual
  authority or node fault switch to make that gate pass.
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
| Dynamic election and mint-finality keys | The native E+2 selector/producer, retained epoch graph, signed control/Pasta paths and immediate next-epoch application barrier are integrated. TODO: qualify complete preparation and certified retention without fresh incumbent keys on actual networks. Registration alone must never add voting authority. | Core/data model, KAGEMUSHA and deployment |
| Prepared beacon transition | The signed encrypted all-edge DKG model and reducer bind the frozen exact roster and fail closed before finalization if an edge or acceptance is absent. The deterministic Core fixture still constructs secrets centrally; daemon per-seat custody, authenticated exchange, genesis orchestration, current/pending session restart and atomic activation need qualification. Parliament can require an early pulse independently of the next epoch-end election pulse. Do not bypass finalized pulse or certificate checks. | Beacon, Parliament, Sumeragi and daemon |
| Staking under the enacted validation-fee policy | `RetailMonthlyAllowance` is the canonical policy: calendar maintenance, included retail payments and overage, with an institutional per-payment fee and separate immutable reward custody. Native assessment checks qualifying payments of the enacted fee asset. TODO: complete and qualify staking integration, distinct fee-asset accounting, bounded claims, multisig/proved overlays and signed payer bounds through original execution. Staking principal cannot satisfy a fee obligation. | Core/native execution and fees |
| Production liveness | Complete the original Validate-to-Apply owner, admitted resources, durable publication and autonomous lane runner together. TODO: qualify silent authors, saturation, final-transaction progress and retirement/restart cuts, with named deterministic regressions and mutation controls. | Core/Sumeragi, Queue, Kura and formal owners |
| Reward allocation | The selected policy is explicit treasury-funded canonical-XOR distributions. TODO: qualify funding, signed recording and bounded payment together. Automatic participation formulas, commission and issuance programs are outside this implementation. | Treasury/governance and Core |
| Network qualification | TODO: one unchanged candidate proves admission, prepared 4→7→4 rotation, queued Parliament pulse, missing target signer, all-seat restart, replay rejection, rewards, slashing and final withdrawal. Run the maintained fault/DA/formal gates and complete workspace checks. | Integration, release and subsystem owners |

The current fee owners are `iroha_data_model::validation_fee`,
`iroha_core::retail_fee` and the common numeric-asset mutation path. The native
payment recorder returns without assessment when a transfer's asset differs
from the enacted policy's `ds_asset_id`. An XOR staking transfer therefore does
not by itself establish payment of a distinct fee asset. The signed monetary
plan still binds exact custody and effects; `deferred_authority` rejects opaque
contract-generated monetary staking and resolves multisig approvals against the
execution overlay. Retained multisig rows must reproduce the exact approved body
hash and physical account/hash key in execution, queue admission, migration and
restoration. Cancellation and expiry preserve unfinished local decoder reads
before recording terminal state or pruning; these connected regressions remain
under validation. Complete fee/custody conservation requires connected tests
under the current policy, not a restored retired charging mode.

Retail-policy enactment checks the activation time against the actual finalized
Parliament enactment time: at least thirty days later, at a Honiara month boundary.
Backdated notice metadata cannot shorten this interval. A fresh short-lived
disposable network can qualify genuine enactment and delayed activation, but
active-policy staking requires an authentically aged chain or the actual elapsed
interval. Do not seed an enacted registry or substitute a fabricated chain clock
for that network gate.

The canonical reward plan requires `fee_claim` as
`Option<PublicLaneFeeRewardClaimV1>`. Its complete lifecycle, beneficiary revision,
source and destination assets, positive amount and receipt sequence bind an
independently accrued fee reward before execution. Missing fields fail decoding;
explicit `None` performs no fee claim work. Shared custody protects the sum of
fee obligations, public rewards and stake. Current execution, restoration, client
and fixture qualification must establish this behavior together.

The beacon producer also retains an explicit allocation TODO: its lower
transcript/reducer must use original-pool admission for nested allocations before
production resource qualification. Component threshold-signature checks do not
close this ownership requirement.

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
records and their native producer consumers are integrated. Current component
and network qualification is required; this contract does not attest completed rotation.

The canonical epoch identity must bind the complete generation, authorization,
ordered roster and original PoPs, network, mode and fresh leader seed. Every
signature must consume its epoch/context binding, and topology must consume the
fresh seed even when membership is retained. Idle chains produce no blocks.
Any protocol-authorized boundary block must retain its authenticated witness
and mandatory Pasta seals. Replay, restart, formal and network qualification
remains open.

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

The global committee remains exactly `3f + 1`, with `1 <= f <= 10` and exactly
`q = n - f` equal validator votes. Observers and admitted candidates cannot pad
quorum. Signed RS16 DA, canonical replay, source-bound evidence and the offline
monetary-authority policy remain mandatory.

## Validation

Global committee geometry and exact certificate cardinality are implemented.
Qualification must cover current Core, daemon, CLI, Torii and network consumers,
authenticated XOR identity, stake/fee/reward conservation, epoch transitions,
restart, resource admission and adversarial custody. Captured instruction records
retain strict identity checks and canonical roundtrips; no fallback decoder is
admitted. Run the maintained Sumeragi simulator and both protocol and Core mutation
gates; faults belong in the simulator, not node configuration. Real-peer
coverage includes `sumeragi.rs`, `sumeragi_lanes.rs` and the committee-transition
target. Skipped network tests and historical component results do not qualify
the current candidate.
