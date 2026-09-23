# Validator staking completion

This is the source-coupled completion record for the September 22 staking work.
It separates instruction implementation from production validator qualification.
No live deployment or release readiness is established by this record.

## Active implementation goal

The complete first-release implementation and qualification goal is active.
The selected design replaces retired layouts and paths; backward-compatible
decoders, aliases, shims and parallel implementations are prohibited.

| Milestone | Completion gate | Current state |
| --- | --- | --- |
| Custody and lifecycle | All focused staking, reserve, snapshot and restoration controls pass | Repairing the 13 failures found by the 126-pass focused Core runs; rerun in progress |
| Canonical XOR | Genesis-pinned network XOR funds bonds, rewards and withdrawals; no synthetic staking definition or implicit production minting | Synthetic `nexus.universal/xor` defaults and local bootstrap paths identified; replacement pending |
| Authority and election | Separate key generations and scheduling epochs; freeze E+2 membership at E and prepare through E+1 | Canonical model, signer and recursive-proof replacement in progress |
| Atomic transition | All target seats ready; current exact quorum certifies activation or retention and cancellation; restart preserves both sessions | Implementation pending |
| Monetary fees | Exact signed effects, bounded claim processing with retained dust, and native execution equality checks | Implementation in progress |
| Production progress | One funded original execution reaches durable Apply; native lane runner is the sole production owner | Complete N0/L1–L6 implementation and qualification remain open |
| Operator/client delivery | Canonical signing, provisioning, status, SDK and fixture workflows | Implementation in progress |
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

## Implemented candidate under validation

- Account-owned lifecycle and signed peer consent: `isi/staking.rs` in the data
  model and Core, Initial/default executor dispatch, canonical instruction
  registry and generated record fixtures. Consent binds network and exact
  activation tenure; rebind also binds the previous peer. Ordinary peer
  administration retains its permission gate. Global candidate-pool changes,
  including future eligibility changes through exit or minimum self-bond
  crossings, remain refused until the prepared key transition is implemented.
  Unchanged eligibility and independent participant-lane custody remain usable.
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
- Four-validator plus observer admission-safety integration scenario in
  `integration_tests/tests/sumeragi_npos_candidate.rs`; it proves a fresh global
  candidate is refused without partial state while the prepared transition is
  unavailable. It does not qualify permissionless global onboarding or committee
  replacement. Independent participant-lane and existing global-peer consent are
  covered separately in Core tests.

## Outstanding protocol and runtime outcomes

| Outcome | Exact dependency and completion criterion | Owners |
| --- | --- | --- |
| Dynamic election and mint-finality keys | The current last-block selection and fixed-four maintenance cannot prepare replacement custody in time. TODO: replace epoch-coupled keys with authority generations, freeze E+2 membership at E, and prepare exact keys through E+1. The current quorum must be able to certify one-epoch retention without each incumbent publishing fresh keys. | Core/data model, KAGEMUSHA and deployment |
| Prepared beacon transition | A successor needs exact-roster DKG, current-committee authorization, retained current/successor shares and atomic activation. Parliament can require an early pulse independently of the next epoch-end election pulse. TODO: make the prepared successor and activation condition explicit, and qualify current/pending session startup and restart. Do not bypass finalized pulse or certificate checks. | Beacon, Parliament, Sumeragi and daemon |
| Staking under an enacted DS-transfer validation-fee policy | `validation_fee.rs` admits reviewed balance-neutral lifecycle actions, but rejects monetary staking and reward reservations whose state-selected effects lack explicit signed bindings. TODO: carry exact staking effects through policy admission and the canonical native execution owner. Ordinary Nexus/PipelineGas charging already uses signed `FeePaymentIntent`; staking-specific runtime qualification of those payer bounds remains outstanding and is separate from this policy blocker. | Core/native execution and fees |
| Production liveness | Complete the original Validate-to-Apply owner, admitted resources, durable publication and autonomous lane runner together. TODO: close the silent-initial-author counterexample and retirement/restart cuts in `sumeragi_liveness_redesign_goals.md`; a second signer or local retry bypass is not a completion. | Core/Sumeragi, Queue, Kura and formal owners |
| Reward allocation | The selected policy is explicit treasury-funded canonical-XOR distributions. TODO: qualify funding, signed recording and bounded payment together. Automatic participation formulas, commission and issuance programs are outside this implementation. | Treasury/governance and Core |
| Network qualification | TODO: one unchanged candidate proves admission, prepared 4→7→4 rotation, queued Parliament pulse, missing target signer, all-seat restart, replay rejection, rewards, slashing and final withdrawal. Run the maintained fault/DA/formal gates and complete workspace checks. | Integration, release and subsystem owners |

The global committee remains exactly `3f + 1` with `2f + 1` equal validator
votes. Observers and admitted candidates cannot pad quorum. Signed RS16 DA,
canonical replay, source-bound evidence and the offline monetary-authority
policy remain mandatory.

## Validation

Validation is in progress. The focused model, executor and codec staking
selection passed 31 tests (the deliberate fixture generator remained ignored),
and the complete default-executor library passed 180 tests. The canonical
instruction-record selection passed 321 tests and failed two checks of the same
pre-existing privacy fixture: commit `c27a25ee13` changed the SDK-consumer enum
from `JavaAndroid` (wire 2) to `JavaSourceKotlin` (wire 10), while that captured
fixture remained unchanged. Its row is byte-identical to HEAD and contains no
staking-registry dependency. The accepted plan now includes its separate repair:
the canonical typed fixture generator regenerated that one record after the
enum change; the identity assertions remain intact and their rerun is pending.
No baseline runtime rebuild is claimed.

The six-package test-target check passed before the subsequent pinned-stake
custody and global-pool guard additions. Final Core, daemon, CLI, Torii and network
validation is still pending. Compilation and source checks do not establish
live finality or the outstanding outcomes above.
