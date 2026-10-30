# Live runtime upgrades

Status: accepted design, 2026-10-29; implementation in progress (section 14). Independent model
reviews (Codex, `gpt-6-astra`, read-only) shaped it. The definition contracts, the founded baseline
selection with selection-aware dispatch, and the change-set lifecycle of the universal root
(proposal, notice, attestation, activation, lapse and the unsupported-node stop) are implemented;
[`runtime_upgrades.md`](runtime_upgrades.md) records that surface as built. It amends `AGENTS.md`,
[`first_release_history_cutover.md`](first_release_history_cutover.md),
[`runtime_upgrades.md`](runtime_upgrades.md) and `sumeragi.md` §16. This document authorizes no
live reset, deployment or movement of value.

This design specifies one permanent live-upgrade protocol. Implementation milestones add components
of that protocol without introducing temporary wire formats, reduced lifecycle rules or
network-specific execution paths. A live deployment is qualified only after every mechanism required
by its selected definitions, obligations and transition is implemented and tested. This is the first
release, so nothing here preserves compatibility with layouts, decoders or histories that predate
it, and old code is deleted rather than kept.

## 1. Outcome

### 1.1 Problem

A live network cannot take a new release without a fresh genesis, even when the release only adds
instructions. The new instruction is not the cause. These are:

- **Registry-dependent roots.** The certified World root is `H(ROOT ‖ S ‖ entries ‖ lanes)`, where
  `S` hashes the identity, kind and key/value schema of every World field compiled into the
  process (`state/world_state_accumulator.rs`, `FieldIndex::build`). One new empty table changes
  `S` and so the root of every stored block.
- **Strict startup.** Startup re-executes stored blocks and requires each result to equal its
  certificate (`sumeragi/node.rs`, `executor/replay.rs`); the native snapshot reader requires the
  exact compiled field list (`state/deserialize_core.rs`). A new binary cannot start on the old
  store.
- **Unversioned semantics.** A new binary accepts instructions, permission names and parameters an
  old one rejects, and native instruction gas is "a constant of the release" (`gas.rs`). A mixed
  cohort disagrees as soon as anyone uses new behaviour.

Each reset deletes accounts, assets and deployed contracts, which breaks every application built
on the network.

### 1.2 Goals

1. A release that adds or changes functionality is installed one validator at a time while the
   chain keeps committing. Network identity, State, accounts and contracts survive.
2. Adding one instruction publishes only its own new definitions. Unchanged definitions are never
   republished, and no runtime version is cut.
3. Applications keep working. A live interface keeps its encoding and behavioural contract until
   governance retires it; retirement is announced and then fails cleanly.
4. Several runtimes run on one network: each execution root executes its own selection.
5. Governance can disable specific versions, including by emergency, and enable them again,
   without stranding obligations in flight.
6. Every node computes the same result for every block. A node re-executes every block above the
   checkpoint it starts from and verifies the blocks below it by their certificates.
7. No backward compatibility and no bridges. Old code is deleted once governance has retired it
   and checkpoints have passed its last use. A genesis reset is never an upgrade mechanism.

### 1.3 Acceptance journey

The design is done when, on Taira after it adopts the foundation (section 13):

1. a release that adds one instruction and one table rolls through the validators one at a time,
   and old and new releases produce identical results until activation;
2. governance activates both; validators restart and replay across the activation height;
3. governance disables and re-enables the instruction without affecting existing applications or
   obligations;
4. an older version is retired, a checkpoint passes its last use, its code is deleted, and a new
   node starts from that checkpoint;
5. older supported SDK and indexer builds keep working against every interface still active, and get
   stable errors for retired ones (8.4).

## 2. Where runtime code lives

A runtime is everything that decides how a chain executes a block. Code can reach validators in two
ways:

| | Compiled (native) | On-chain IVM |
| --- | --- | --- |
| Who ships a change | Developers release binaries; governance activates an immutable definition | Authors publish bytecode; governance activates its hash |
| What needs a node release | Any new native behaviour | Only new host functions, VM operations or protocol formats |
| Blocks above a node's checkpoint replay with | The native implementation, until it is retired and deleted | The bytecode, under the host semantics it ran against |
| Cost | Native code to audit; deleted after retirement | Metering and execution overhead |
| Fits | Cryptography, consensus-adjacent and high-volume instructions | Business rules, policy and applications |
| Polkadot analogue | Host functions | The runtime |

With compiled code, operators install a binary containing implementations A and B, and governance
then switches to B; governance cannot activate code that is not installed. With on-chain code,
governance publishes bytecode and validators execute those exact bytes with their installed IVM and
host functions.

**Recommendation: a hybrid.** Native Rust is the platform kernel, and each implementation in the
definition graph is native or IVM bytecode. Business features that fit a Kotodama contract already
ship without a node release (section 10).

## 3. Definitions and selections

### 3.1 Definitions are the unit of publication

A definition is immutable. Its identity is
`id = H(domain ‖ version ‖ kind ‖ length-framed canonical descriptor)`, and the descriptor names
the identities it depends on.

| Kind | What its descriptor fixes |
| --- | --- |
| Interface | One instruction wire ID: payload schema and a behavioural contract (observable outcomes, rejection codes, the event definitions it may emit, metering class) |
| Implementation | One named execution entrypoint (instruction handler, hook, query or migration step): native or IVM code, its interface if any, the storage it reads and writes, events, permissions, parameters, invoked interfaces, host profile and services it relies on |
| Storage | One canonical logical namespace: key and value schemas (recursive, with discriminants and layout flags), canonical key order, initial bytes, invariants and commitment projection |
| Event | One event identity and schema |
| Permission | One permission name and payload schema |
| Parameter | One parameter identity, schema and initial value |
| Host profile | IVM behaviour and host semantics: syscall and pointer-type surface, numeric rules, query projections, ordering, fault precedence and metering, at least everything the ABI descriptor binds today (`ivm_abi/syscalls.rs`) |
| Execution services profile | Root-wide behaviour no single instruction owns: ordered block hooks, authorization evaluation, query semantics inside execution, event projections and their side effects, fee charging, resource accounting, recursion and reentrancy limits, error precedence |
| Commitment profile | How State is committed: the World root rule, its projections and proof formats |
| Kernel | Sequencing, framing, atomic publication and restricted recovery |
| Migration | A bounded implementation between storage definitions, with a cursor schema, metering and deterministic failure behaviour |

**Native identity.** Machine code differs across compilers and targets, so a native definition's
descriptor binds its observable semantics (errors, events, metering) and a reference-source closure
digest: the handler plus every shared helper, generated code and dependency implementation it
relies on. At activation the closure is archived as a content-addressed source snapshot. Each
release attests which definitions its binary implements, with the binary digest, target, toolchain,
features and provenance.

**Changing shared code.** Source is not frozen. A shared helper or generated registry may change
when CI shows that every active definition depending on it keeps its observable semantics: its
conformance corpus and every replayable history still match. When a helper must change behaviour,
the definitions that relied on the old behaviour keep a private copy of it until they are retired
and deleted.

**Storage identity.** A storage definition is a contract, and a native World field or generic
definition-owned storage implements it. A Rust type may evolve under the same storage definition
only while every fixed property in its descriptor stays identical. Otherwise it is a new storage
definition with a migration. Data is never decoded into a newer Rust type because a nominal name
matches.

**Generic carriers.** A definition implemented in IVM bytecode must not need new Rust code. Its
payload, storage, events and permissions are described by bounded canonical Norito schemas in its
descriptors: storage with ordered keys, explicit initial values and capability-controlled reads,
writes and scans. The World's generic contract storage (`smart_contract_state`, keyed by
`StatePath`) can back it, but a raw path prefix does not establish authority; the storage capability
does.

**Baseline.** Today's native behaviour is registered as ordinary immutable definitions, bundled
conservatively where its dependencies cannot yet be separated. Later decomposition publishes new
definitions; it never reinterprets the bundle.

### 3.2 Two graphs

- **Artifact graph.** Descriptors reference identities that already exist, so this graph is acyclic
  and adding a definition never changes an old one.
- **Execution graph.** Resolving a selection adds edges from each interface to its bound
  implementation; triggers and callbacks add more, so it can contain cycles. Frozen recursion,
  reentrancy and gas limits in the services profile bound it.

Native code must not bypass resolution. Today multisig calls
`Revoke::account_role(..).execute(..)` directly (`smartcontracts/isi/multisig.rs`), so rebinding
`Revoke` would not reach that call. A native cross-instruction call goes through selection-aware
dispatch, or the callee is a private primitive inside the caller's reference closure.

### 3.3 Each root executes one resolved selection

```text
RootSelection {
  kernel, commitment_profile, services_profile,
  installed_storage, active_implementations,
  bindings:     Map<WireId, ImplementationId>,  // exactly one per active interface
  restrictions: Map<DefinitionId, Restriction>,
  completion_closures: Map<ObligationId, CompletionClosure>,  // retained for live obligations
}
selection_id = H(canonical RootSelection)
```

- An execution root (the global chain, or a data space with an independently signed root,
  `sumeragi.md` §11) has exactly one selection at each height, derived from the authenticated
  predecessor State plus the change set scheduled for that height.
- Bindings, installed storage, host semantics and system hooks are root-wide. A data space inside a
  root may only add restrictions on what its routed transactions use.
- **Closure.** Every dependency of an active definition is active or installed, through services,
  host profiles and migrations. Every active interface has exactly one bound implementation. Every
  live obligation keeps its completion closure (7.2). A change set that would break closure is
  rejected.

## 4. Change rules

- **Add an instruction.** Publish its interface, implementation and any new storage, events,
  permissions or parameters. The change set moves the root from selection `S` to `S'`. Nothing
  unchanged is republished. Qualification covers the invariant families the new implementation
  touches.
- **Add storage.** Publish a storage definition. The World root changes only when it is installed,
  and installation writes the definition's initial bytes.
- **Fix behaviour within an interface's contract.** Publish a new implementation and rebind the
  interface at a height. Applications keep the wire ID. Correcting observed buggy behaviour can
  still disrupt applications that relied on it, and the change set says so.
- **Change behaviour outside the contract.** Publish a new interface under a new wire ID. The old
  interface is deprecated and later retired.
- **Change a storage layout.** Publish a new storage definition and a migration (5.4). The old
  storage is retired by that migration's cutover.
- **Retire.** Restrict, drain obligations, disable, and delete the code once checkpoints pass its
  last use (section 7).

Every change goes through the same change-set mechanism. A change set requesting an operation the
running release does not implement yet is rejected; there is no interim path.

## 5. Historical execution and commitments

### 5.1 Complete State, selected by history

- The foundation's first commitment profile is the LtHash accumulator
  (`state/world_state_accumulator.rs`), extended to the complete State and with its schema chosen
  by the selection. Today it commits only the World: transaction membership, the canonical runtime
  policy and commit topologies sit outside it (finding G1-D2; `first_release_history_cutover.md`,
  "Snapshots and checkpoints"). Checkpoints and code deletion need them committed, so that
  extension lands with the foundation.
- The keyed commitment of `sumeragi.md` §16 (task G.3, not yet selected) follows as a governed
  commitment-profile transition, without a reset. The LtHash code is deleted once the checkpoint
  startup floor passes that transition.
- The commitment covers the storage installed in the selection: every canonical field is a storage
  definition, cells stay explicit at their initial value, and storage not installed leaves every
  root unchanged.
- Storage not installed is inaccessible to execution, not merely unrecorded: dispatch and storage
  capabilities keep it unreachable, and a capture asserts that it is empty.
- At a transition the parent root uses the predecessor schema and commitment profile, migration
  reads use predecessor codecs, and the successor root uses successor codecs and profile. Today
  `WorldStateAccumulator::root` reads one process-global schema and `world_state_transition` uses it
  for both parent and successor; the replacement takes each from its own selection.
- Cold and incremental roots agree across activation, aborted publication, block replacement and
  restart.

Every commitment profile, the LtHash one included, covers every canonical State field, is correct
at the publication cut, rejects tampering, and a State restored from a checkpoint reaches the same
tip roots as replay. `sumeragi.md` §16 is amended accordingly: the keyed commitment arrives as a
commitment-profile transition, selects schemas historically and keeps its typing, absence and range
proofs. Because the commitment profile is itself a definition, every later improvement of it is a
governed transition, not a fresh genesis.

### 5.2 Envelopes and dispatch

- An instruction travels in a stable envelope: bounded framing (wire ID and length-framed payload)
  that is parsed before any payload is interpreted. `InstructionBox` already carries a wire ID and
  a framed payload; what changes is that resolution consults the selection instead of the compiled
  registry (`isi/mod.rs`, `decode_instruction_from_pair`).
- Validation walks envelopes in canonical pre-order and stops at the first one whose wire ID is
  unknown or whose interface is not active. It rejects the transaction with the stable code
  `definition_inactive { wire_id }` for that envelope, and the block stays valid. A release that
  knows an inactive interface treats it exactly like an unknown one and never decodes its payload,
  so every release stops at the same envelope.
- Framing checks, traversal order, metering and error precedence are frozen. Malformed outer
  framing remains a structural error.
- Instructions emitted during execution (contracts, triggers) are checked at execution time.
- The selection is passed explicitly to validation, execution, the IVM host, permission and
  parameter checks, hooks and queries inside execution. The process-global instruction registry
  (`INSTRUCTION_REGISTRY`) cannot change with height, because replay, independent roots and
  concurrent queries need different selections at once.
- Lane batches: global execution resolves wire IDs against the selection at the merge height.
  Certification before `H` does not grandfather authorization.

### 5.3 One execution and effects contract

Native and IVM implementations implement the same definition and effects contract. It extends the
existing contract machinery (`CoreHost`, `HostExecutionArtifacts` in `smartcontracts/ivm/host.rs`)
rather than adding a second effects engine:

- the exact implementation, entrypoint, payload schema, dependencies, host and services profiles and
  resource limits, with the root, selection, authority and invocation ancestry supplied by the
  engine;
- unforgeable storage capabilities and bounded, ordered effects;
- explicit nested-call resolution, read-your-writes, reentrancy, failure ordering, rollback and fee
  semantics;
- obligation registration and completion through the kernel (7.2), and bounded migration
  entrypoints (5.4).

Shared assets, fees, reserves and authority stay behind versioned services, never arbitrary World
writes. The replaceable executor remains an authorization component; replacing it is not how
definitions are published.

### 5.4 Migrations

A migration runs in stages. Each stage is atomic and certified; the migration as a whole is not one
transaction. Until the final cutover the old storage stays authoritative and the successor storage
is inaccessible to execution. Writes to the old storage go through a write barrier that keeps the
successor consistent, the cursor survives restart, and the cutover that installs the successor and
retires the old storage is one atomic transition. Obligations whose completion closure needs either
storage stay executable, or are transferred explicitly.

A defective migration can be contained: governance suspends it, its cursor and write barrier stay in
place so neither storage version is corrupted, and a certified repair resumes or replaces it.

### 5.5 Snapshots

- The native snapshot reader takes its schema from the snapshot's authenticated selection. A
  missing installed field is corruption; storage not installed is neither written nor expected.
- Portable `WorldStateSnapshotV1` consumers compare `schema_hash` with the schema selected at the
  snapshot's height, not with the compiled constant `State::native_world_schema_hash_v1`.
- A previous release reads a newer release's snapshot while the newer definitions are not in the
  selection, so a rollback before activation is a binary swap.

## 6. Activation

**Who governs.** SORA Parliament governs the selection of the universal root: it approves its change
sets, restrictions and retirements. The admins of a permissioned data space govern that data
space's own selection and nothing else. Every rule that decides whether a change set is valid
(provenance requirements, notice, the checkpoint startup floor) comes from committed State, never
from node configuration. The notice policy is the committed parameter `runtime_upgrade_policy_v1`;
the manifest provenance check that read node configuration is deleted, and provenance requirements
return only as committed State.

1. **Stage.** Publish the definitions and artifacts. A node stores and authenticates definitions it
   cannot execute: admission checks only that the change set applies to the selection it will meet
   (`runtime/change_set.rs`), never that this release implements it.
2. **Install.** Validators install supporting releases one at a time (section 9) while the old
   selection stays active.
3. **Attest.** Each validator attests on chain that its release executes the exact successor
   selection, bound to the expected predecessor selection, the transition and obligation closure,
   the kernel and commitment profiles, the committee epoch, the checkpoint it tested and a validity
   interval. A compact catalog commitment can carry the per-definition detail. Supporting
   definitions A and B separately does not establish support for their composition.
4. **Activate.** Governance approval schedules the change set at its effective height. At the
   start of that height it activates only if every committee member attested for the current
   committee and the notice elapsed in committed time; otherwise it lapses and never applies.
   Attestations are revalidated at activation; a committee change requires them again.
5. **Stop the unsupported.** A node whose release cannot execute the successor selection stops voting
   and executing before the boundary, reports the selection and height in `/status` and telemetry,
   and resumes after its release is replaced. It never classifies a valid upgraded block as invalid.
   Block start returns `StateStorageAdmissionError::UnsupportedRuntime`, whose reason names the
   selection and the first definition the release lacks; consensus classifies it as
   recovery-required, so the node halts with `PublicationRecoveryRequired(height)`, served by
   `/v1/sumeragi/status` and telemetry. `GET /v1/runtime/release` names an unsupported scheduled
   successor before the boundary.

- **All `n`.** Attestation comes from every validator, not a quorum: of `q = n − f` reports, `f` may
  be Byzantine, leaving `q − f` honest upgraded validators, fewer than `q`. One Byzantine validator
  can therefore delay an activation until governance rotates it out. Certificates still carry
  exactly `n − f` votes.
- **Scheduling.** Block `H` runs under the predecessor selection plus the change set scheduled for
  `H`, applied before ordinary execution; a change submitted during `H` takes effect at `H + 1` at
  the earliest. Compatible scheduled changes are ordered canonically; conflicting preconditions are
  rejected. Migration, schema selection and State publication are atomic.
- **Notice.** An ordinary change becomes eligible only when it has both waited the minimum number of
  blocks and reached `time ≥ max(proposal_time + notice, approval_time + approval_cushion)`, with
  times taken from committed block time. The policy in force when the proposal is admitted is frozen
  for it. Notice is a lower bound: it never bypasses governance and never produces empty blocks.
  Recommended committed defaults, set through the same parameters on both networks:

  | Action | Taira | Minamoto |
  | --- | --- | --- |
  | Ordinary activation | 100 blocks and 24 hours | 3,600 blocks and 14 days |
  | Ordinary disable | 100 blocks and 7 days | 3,600 blocks and 30 days |
  | Emergency restriction | Next block | Next block |
  | Approval cushion for ordinary changes | 1 hour | 24 hours |

  The block floors assume the genesis-profile cadences of 4 s on Taira and 100 ms on the Nexus
  profile (`iroha_deploy/src/genesis/profile.rs`). Parliament's own procedure adds its windows on
  top: its permanent defaults (one-hour lifecycle windows, 24-hour evidence, 24-hour ballot notice
  and 24-hour casting) put a full runtime procedure near ten days. Taira sets shorter committed
  Parliament values (five-minute lifecycle windows, one-hour evidence, fifteen-minute ballot notice,
  three-hour casting, one-minute enactment delay) while keeping every body and voting rule;
  three-hour casting is the shortest that satisfies the ballot capacity rule
  (`parameter/parliament_ballot.rs`) at a 4 s cadence. Minamoto keeps Parliament's production
  defaults.
- **Recovery.** Governance can cancel before activation. After activation, recovery is forward: a
  corrective definition, rebinding or migration. A binary downgrade is safe only when it supports
  the whole selection history above its checkpoint.
- **Handshake.** It distinguishes dormant capabilities from active compatibility requirements. A
  peer may carry extra definitions and may lag; it must support what the active selection requires.
  The equality check on the release's compiled gas schedule became a check against the active host
  profiles: each peer advertises the host profiles its release implements
  (`iroha_p2p::HostProfileSet`), and a node refuses a peer that lacks one its committed selection
  activates (`RequiredHostProfiles`, refreshed whenever a change set activates).
- **Fees.** The predecessor fee-policy cut (`StateTransaction::root_fee_policy`) is preserved across
  transitions.

## 7. Retirement and deletion

### 7.1 Restrictions

| State | Applies to | Meaning |
| --- | --- | --- |
| Deprecated | Interfaces, implementations | No new deployments, bindings or dependents; existing use continues |
| Admission closed from `H` | Interfaces, implementations | No new work from `H`; work already admitted continues |
| Completion-only from `H` | Implementations | Only narrowly scoped completion capabilities for live obligations run |
| Disabled from `H` | Interfaces, implementations | No execution from `H`; allowed only once no live obligation needs it |
| Re-enabled from `J` | Interfaces, implementations | A new committed authorization interval; earlier restrictions remain historical facts |
| Installed, retired | Storage | Installed storage stays committed and readable; a migration retires it |

- A pending transaction that uses a restricted definition gets a stable rejection when otherwise
  eligible. It is never redirected to another implementation.

**Emergency containment.** Full Parliament cannot act at the next block: a runtime proposal passes
its whole body pipeline (`governance/parliament.rs`, `parliament_attempt_policy_v1`), and so does a
contract emergency hold. The existing fast pause track covers SCCP only
(`governance_pipeline.md`, "Fast pause track"). Containment therefore needs its own standing body:

- **Universal root:** a Parliament-delegated runtime emergency panel of nine primary and nine
  disjoint backup members with seven-day terms. Six endorsements from either panel authorize
  containment; an endorsement expires after 30 minutes. Endorsements are a typed runtime-emergency
  kind, not SCCP's and not ordinary findings.
- **Permissioned data space:** two of three separately held admin keys, authorized by that data
  space's committed governance policy.
- **Effect:** the block after the authorization commits. No wall-clock latency is promised.
- **Powers:** close admission and keep narrowly scoped completion. Containment cannot activate,
  rebind, migrate, move value, delete storage or fully disable; a full disable is an ordinary
  governance decision after the obligation checks of 7.2.
- **Review:** containment automatically opens a bound ratification case. Parliament, or the data
  space's ordinary admin governance, must ratify, repair or lift it. A lift must come from an
  approval started after the incident, so a stale approval cannot undo a newer restriction.
- **Expiry:** containment is a lease of at least 48 hours on Taira and 14 days on Minamoto, or the
  qualified review latency plus 24 hours if that is longer. Renewal opens halfway through and needs
  fresh endorsements and recorded review progress. Unratified containment that is not renewed
  expires; a permanent restriction needs ordinary governance.
- **Activation:** every validator's support attestation (section 6) covers the containment
  operator, so an incident's narrowing of an already supported selection needs no fresh unanimous
  attestation; the resulting selection is still certified.

Containment must not stall the network's own recovery: governance recovery stays outside the
restricted definitions, and containment never reverses finalized effects or removes settlement
authority.

### 7.2 Obligations

The dependency graph cannot infer obligations from arbitrary Rust or bytecode, so every effect that
creates one (escrow, pending consent, a scheduled trigger, a migration cursor) registers it through
the kernel, with its completion requirements. Every live obligation keeps an authenticated
completion closure: its implementations, host and services, codecs, verification rules and budgets.
AMX settlement needs a certified decision, the retained participant State and shared monetary code
(`sumeragi/amx/native.rs`).

A completion capability binds the original obligation, its authority, its retained dependencies,
the terminal effects it may perform and its resource commitments. It cannot create new work, and it
lives inside the root's selection rather than as a second runtime. A change set that restricts a
definition proves that the obligations needing it are drained, atomically transferred under their
preserved invariants, or still executable through completion capabilities. Escrow is never stranded
and never released unilaterally. If completion code itself is unsafe, the remedy is a certified
repair or transfer, never removing settlement authority.

So "disable this version at the next block" and "never strand its obligations" cannot always both
mean "run none of its code from the next block": an emergency restriction closes admission at once,
and full disabling follows once completion has drained or been transferred.

| Deferred kind | Rule |
| --- | --- |
| Data, time and by-call triggers | Captured definitions; suspended when one is restricted; the existing persistent retry and exhaustion rules apply (`trigger/model.rs`) |
| Multisig proposals | Captured resolved closure; a changed closure requires renewed approval (8.2) |
| Migrations | Committed cursor, write barrier and resumption; suspension and certified repair (5.4) |
| AMX participations | Completion capabilities until decision and settlement |

Re-enabling never resurrects cancelled transactions or runs skipped triggers.

### 7.3 Checkpoints

In plain words: today a node that starts rebuilds the chain's State by re-executing every block since
genesis, so it needs the code of every version any block ever used. A checkpoint is a copy of the
complete State at one height, proven by the validator signatures on that height. A node checks the
signatures, loads the copy and re-executes only the blocks after it. Once every node starts from a
checkpoint later than the last block that used an old version, nobody needs that version's code.

- **What is certified.** A checkpoint at height `h` is the published State `P_h`, after block `h`'s
  apply-time writes, authenticated by block `h + 1`'s certified parent root. Block `h`'s own
  certificate covers its execution State `X_h`, which those writes still change (`sumeragi.md`
  Appendix E, E51). Authority comes from consensus certificates carrying exactly `q = n − f` votes,
  never from whoever exported the bytes, so any validator or archive may produce them. A node
  verifies the certificate chain from genesis to `h + 1` (committee handoffs and CommitQCs) without
  re-executing, checks `P_h` against the complete-State root (5.1), then re-executes every later
  block.
- **Startup floor.** The oldest checkpoint the current software promises to load. It is certified
  policy in committed State, it only moves forward, and a downloaded snapshot cannot declare its own
  acceptance.
- **Policy.** Recommended committed defaults:

  | Policy | Taira | Minamoto |
  | --- | --- | --- |
  | Production cadence | Every 15 committed minutes | Every committed day |
  | Minimum retained checkpoints | Every one from the last 24 hours, then one per day back to the floor | Every one back to the floor |
  | Oldest-start recovery window | At least 7 days | At least 30 days |
  | Notice before raising the startup floor | 24 hours | 7 days |

- **Advancing the floor.** The new floor checkpoint is at least the recovery window old when the
  change takes effect; at least `q` validators have restored from it and caught up; three
  independent custodians hold the checkpoint and every later block; and the outgoing code's release
  and evidence are archived. Parliament authorizes this maintenance policy once, and routine
  advancement runs within that mandate. Missing data blocks advancement, and with it code deletion.
- Retention counts are minimums, not eviction caps: the current floor, a proposed floor and their
  later blocks are pinned. An idle chain keeps its latest eligible checkpoint; no empty block is
  produced to make one.

### 7.4 Deleting old code

A definition's code is deleted from the source in the next release once all of these hold:

1. governance has disabled it on every root that ever selected it, as shown by an authenticated
   registry of supported roots with certified retirement and floor receipts; an unreachable
   registered permissioned data space blocks deletion of code it still needs, and an unregistered
   fork imposes nothing;
2. no live obligation's completion closure needs it;
3. every such root's startup floor is above its last use;
4. nothing still live needs it. Stored-value codecs, checkpoint readers, proof verifiers and the
   certificate verification from genesis to the floor stay live after the instruction handlers they
   served are deleted.

Every deletion release is tested by starting from the oldest accepted checkpoint, not only the
newest. Descriptors and retirement records outlive the code. Bringing deleted functionality back
means publishing it again, shipping release support where needed, qualifying and activating it; it
is never a flag.

Deletion is a source change, not a chain transition. Blocks below the checkpoint stay verifiable by
their certificates. Re-executing them needs the archived release that produced them, built from its
recorded revision, as "Obsolete histories" in `first_release_history_cutover.md` already requires:
the archive keeps evidence and releases, and the source tree keeps only live code. A private copy of
an old helper (3.1) is deleted with the last definition that used it.

## 8. Applications

### 8.1 Interfaces and calls

- An interface's behavioural contract is part of its identity. Rebinding is allowed only within it;
  anything else is a new interface.
- A transaction without an expectation follows the current bindings and is promised only the
  interface's contract.
- A transaction may sign an expectation over the resolved closure of what it uses: the bound
  implementations, the interfaces they call transitively, and the host and services profiles.
  Dispatch enforces it at every dynamic call. A mismatch rejects with a stable code; an expectation
  never selects old semantics.

### 8.2 Deferred consent

Multisig proposals keep their instruction bytes and hash and execute through the current executor
(`smartcontracts/isi/multisig.rs`); trigger actions keep their executable, authority and firing
policy (`trigger/model.rs`). Both capture an expectation by default, or opt in explicitly to
following current bindings. If the resolved closure changed, the proposal needs renewed approval and
the trigger is suspended. Multisig consent is never silently reinterpreted.

### 8.3 Interfaces apps read

- **Discovery.** Torii exposes the root's selection at a specified certified height: definitions,
  bindings, restrictions, schedules, deletion eligibility and each definition's descriptor.
- **Events** travel in stable, length-framed envelopes carrying their event definition. Clients
  keep unfamiliar payloads instead of failing the stream.
- **IVM contracts** keep their `expected_code_hash` pins and durable storage at their immutable
  address (`transaction/executable.rs`, `smartcontracts/ivm/host.rs`), and record the interfaces
  and host profile they were built against. Admission checks that those are active; execution fails
  cleanly if one is restricted.
- **Results.** Certified results bind World roots, ordered events and executed block bytes, so
  rejection bytes are protocol material (`sumeragi_finality/commitment.rs`,
  `transaction/error.rs`). New outcomes use stable codes with canonical parameters; human
  diagnostics stay outside commitments.
- **Metering and fees.** Instruction-specific metering belongs to implementations; the total budget,
  fee conversion and charging order belong to the services profile. Queries inside execution need
  historical semantics; external Torii APIs need versioned responses and explicit height selection.

A breaking change is a changed payload interpretation, permission meaning, event meaning, committed
rejection bytes, syscall behaviour, storage interpretation or metering class outside an interface's
contract. It ships as a new definition with coexistence or a migration, and notice.

### 8.4 What clients are promised

Clients that conform to the foundation keep working against active interfaces; retired
functionality is retired, not executed forever. Recommended committed defaults:

| Promise | Taira | Minamoto |
| --- | --- | --- |
| Current and preceding SDK release lines supported after a replacement | 30 days | 180 days |
| Minimum interface deprecation before disable | 7 days | 30 days |

- An interface may declare a longer window. Deprecation is never shorter than the ordinary-disable
  notice of section 6.
- While an interface is active, its signing bytes, schemas and behavioural contract are preserved,
  and so are contract code-hash pins and signed limits.
- An unknown event stays consumable as a raw envelope carrying its definition ID, height and
  selection identity; indexers neither lose it nor stop the stream.
- Deprecation, earliest disable, approved schedule, emergency scope and replacement guidance are
  published through certified discovery and lifecycle events, and the restriction itself is emitted
  at enactment. Emergency containment may override the deprecation notice, and its announcement says
  so at once.
- A disabled call returns a stable structured error. A request for history outside a server's
  retained coverage returns an explicit boundary or archive response.

## 9. Rolling operations

1. CI replays every retained history above the startup floor, and the conformance corpus of
   every active definition. Captures are never regenerated to make a candidate pass.
2. `iroha3d --check-replay` replays an immutable copy of a live store with the candidate, built on
   `NativeReplaySeed`/`IsolatedNativeReplay` (`sumeragi/node/replay.rs`). It reports the first
   differing commitment component and distinguishes missing historical bodies from divergence.
3. Validators are replaced one at a time: stop, `--check-storage`, swap, start, certified catch-up.
   Before each stop, the tooling checks that the remaining responsive validators still form a
   quorum with margin; it pauses on degradation, because an already exhausted fault budget plus one
   restart halts the chain.
4. Attestations are collected from every validator, then the change set activates (section 6).

Taira tooling runs this sequence per validator, with `--check-storage` available through the Taira
launcher (`irohad/src/taira_runtime_signer.rs`) and a rollback before activation that is a binary
swap. This is S1-B of `network_deployment.md`.

## 10. On-chain business logic

Ordinary IVM contracts already execute State transitions: the deployed-contract path authenticates
code and ABI, authorizes the entrypoint, runs `CoreHost`, and applies queued instructions, emissions
and durable storage effects transactionally (`executor.rs`, `smartcontracts/ivm/host.rs`
`HostExecutionArtifacts`). A business feature that fits a versioned Kotodama contract entrypoint
ships today without a node release, provided the installed host has the operations it needs.

An instruction definition implemented in IVM bytecode goes further: its interface, storage, events
and permissions are generic-carrier definitions (3.1), and it runs through the same execution and
effects contract as native code (5.3). Publishing it needs no node release as long as the installed
release supports every schema form, VM operation, host function and service it depends on.

The existing `SMARTCONTRACT_EXECUTE_INSTRUCTION` syscall is not that gateway; it accepts only
narrowly tagged operations. The current executor is not a coexistence mechanism either: its
validation runs on `DefaultHost` and returns a verdict (`run_executor_validation`), and installing
its data model removes undeclared permissions (`WorldTransaction::apply_executor_data_model`).

"Real time" means a governance transaction publishes and activates the bytecode. New cryptography,
host functions, VM behaviour or commitment semantics still need a node release. IVM is preferred for
business operations that installed primitives can express; native code remains for the kernel,
cryptography and performance-sensitive operations.

## 11. Several runtimes on one network

- **Per root.** The global chain and each independently rooted data space have their own selection
  and schedule. They interact through authenticated cross-root protocols such as AMX
  (`sumeragi.md` §11), whose formats are definitions on both sides and whose obligations every
  change set preserves.
- **Data spaces inside a root** share the root's selection and may only restrict. Different
  implementations on shared State would first need a root-committed
  `(data space, interface) → implementation` map, enforced storage capabilities, authenticated
  context transitions at every nested call, root-owned services for shared accounts, assets, fees and
  authorization, deterministic trigger behaviour and atomic cross-space effects. Global assets
  resolve to global balances regardless of the ambient data space (`state.rs`), fee execution
  switches to `UNIVERSAL` (`executor.rs`) and globally authorized data triggers exist
  (`smartcontracts/isi/triggers/mod.rs`), so storage namespacing alone does not provide that
  boundary.
- **Lanes** certify batches and never execute them (`sumeragi_lanes.md` §0); a lane has no runtime.

## 12. Policy

`AGENTS.md` keeps its first-release rule and gains the rule for networks running this design:

> This is the first release: remove retired layouts, aliases, shims, fallback decoders and parallel
> legacy implementations, and delete old code. The live-upgrade foundation carries no compatibility
> path forward: a running network adopts it once at a governed checkpoint that becomes its trust
> anchor, and every bridge or import used up to and including that adoption is deleted afterwards.
> A network running the foundation changes only through governed definitions: a change
> publishes new immutable definitions and moves an execution root to a new selection. A definition's
> canonical encoding and semantics never change while any selection contains it. Governance
> restricts and retires definitions without stranding obligations or reinterpreting history, and a
> retired definition's code is deleted once no obligation needs it and the checkpoint startup floor
> is above its last use. Dispatch uses authenticated chain context and explicit
> identities; heuristic fallback decoders, ambiguous aliases and node-local protocol toggles are
> prohibited. State migrations are explicit, deterministic, bounded and certified. A genesis reset is
> not an upgrade mechanism.

`first_release_history_cutover.md` keeps "no version dispatch, no second decoder" for everything
outside the definition graph, states that dispatch through an authenticated selection is the
upgrade mechanism rather than compatibility, and replaces its `fresh_genesis` classification of
additive instruction, event and State changes with change sets. The `AGENTS.md` ABI paragraph that
lets data model and APIs "change freely" applies to code no live selection contains. "Every Iroha
build ships every syscall" forbids build-dependent omission; the selection decides which host
profile a contract may use. The history guard separates deleted execution code from verification
functionality that stays live (7.4).

## 13. Adopting a running network

Taira does not reset. A running network adopts the foundation once, at a checkpoint authorized on
its own chain, and that checkpoint becomes its trust anchor. Accounts, assets, contracts and block
history survive.

1. **Authorize.** The running chain commits an adoption authorization through the governance it
   already has: on Taira today the account holding `CanManageRuntimeUpgrades`, and SORA Parliament
   once it is integrated. It names the foundation release by its artifact digests, the adoption
   height `H` and the foundation's initial selection. Authority comes from that certified
   transaction, never from node configuration or from installing software.
2. **Stop.** The predecessor commits no block after `H`. The authorization is a scheduled runtime
   upgrade the predecessor release cannot activate, and an unsupported scheduled upgrade halts it at
   its start height (`State::apply_block_start_world_transitions`). How the predecessor recognizes
   it is fixed with the adoption carrier (section 14, step 5). This one switch is coordinated, not
   rolling: the foundation release never executes the predecessor protocol.
3. **Import.** The foundation release reads the predecessor's State at `H` and checks the World
   against block `H`'s certified World root under the predecessor's root rule. It maps every
   predecessor field onto a storage definition of the baseline catalog; a field the foundation
   retired must be empty, so nothing is dropped. State outside the World root (transaction
   membership, commit topologies, the canonical runtime; finding G1-D2) comes from the predecessor's
   signed snapshot. Its correctness rests on the authorization and on every validator importing the
   same bytes, which their attestations of the resulting keyed root confirm.
4. **Anchor.** The adoption record (network, `H` and its block hash, the predecessor roots, the keyed
   root of the imported State, the committee and epoch, and the foundation's selection) becomes the
   network's trust anchor. All `n` validators attest it before the foundation history continues at
   `H + 1`. New nodes start from the anchor or a later checkpoint and verify forward; blocks up to
   `H` are verified only with the archived predecessor release, as "Obsolete histories" requires.
5. **Delete.** The next release deletes the import, the predecessor's root rule (World-only coverage
   with a schema digest over every compiled field) and any legacy genesis profile, once every
   validator has adopted. From then on no
   release carries pre-foundation code.

An interim checkpoint import that keeps Taira on a newer release before the foundation is ready
follows the same steps but takes its checkpoint from node configuration. The governed adoption
replaces it, and it is deleted with the rest of the pre-foundation code.

## 14. Implementation order

Each step is permanent and closes with its gate (section 15). Taira's adoption is a readiness gate,
not a date: it happens once everything its first selection needs is implemented and qualified.

1. **Policy, evidence and contracts.** `AGENTS.md`, `first_release_history_cutover.md`,
   `runtime_upgrades.md` and the §16 amendment (done). The definition contracts as code in
   `iroha_data_model::runtime::definition`: every descriptor kind, content-addressed identities, the
   bounded carrier-schema language, root selections and their closure check, with pinned identity
   vectors (`crates/iroha_data_model/tests/runtime_definition.rs`). Still to specify: the execution
   and effects contract (5.3), obligations (7.2) and emergency containment (7.1) as types.
2. **Complete State and execution foundation.** The LtHash commitment extended to the complete
   State (G1-D2), historical schema selection with separate parent and successor contexts, the definition catalog with today's
   engine as one baseline bundle (its interfaces and storage enumerated one by one, its handlers and
   shared services one conservative implementation set with superset dependencies), root selection
   in State, stable envelopes and selection-aware dispatch with native cross-instruction calls
   included, storage capabilities, generic carriers, snapshots through the selection, and staging.
   Pre-foundation compatibility code is deleted. The guard inventories
   definitions, descriptors and reference closures, and the pinned history
   (`sumeragi/node/tests/history_cutover_tests.rs`, today genesis plus two `Log` blocks), which this
   cutover regenerates anyway, gains successes, rejections, permissions, triggers, contract emissions
   and populated State.
3. **Transition and lifecycle engine.** Change sets, all-`n` attestation of resolved selections,
   committed notice and provenance policy, scheduling, the unsupported-node stop, handshake
   profiles, restrictions, completion capabilities, the deferred-kind rules for every obligation the
   adopted State carries (triggers, multisig, AMX), expectations and deferred consent, stable
   rejection codes and event envelopes. Done for the universal root: change sets with proposal
   through Parliament, the committed notice policy, all-`n` attestation bound to the committee
   and signed by each node for its own release, scheduling, activation and lapse at block start,
   the unsupported-node stop with a reason naming the selection, stable rejection codes and
   lifecycle events, and handshake host profiles. Still open: committed provenance policy,
   completion capabilities and obligation kinds, expectations and deferred consent.
4. **Governance.** Parliament's Core integration (still a TODO in
   `parameter/parliament_lifecycle.rs`) with its committed timing policy, and the runtime emergency
   panel. Neither gets a temporary substitute on Taira. The panel's containment is implemented
   (`runtime_emergency_panel_v1`, `ContainRuntimeV1`, renewal and expiry); its bound ratification
   case and recorded review progress are still open.
5. **Checkpoints, adoption and deletion.** Checkpoint export and certificate-chain bootstrap, the
   startup floor and its advancement, the deletion rule and its guard, the adoption carrier, import
   and trust anchor (section 13), and a rehearsal that adopts a copy of a running network, physically
   deletes the pre-foundation code and then starts a node from the oldest accepted checkpoint.
6. **Operations and clients.** `--check-replay`, per-validator Taira tooling with rolling recovery,
   certified discovery and the client contract (8.4), tested against archived SDK and indexer
   builds.
7. **Taira adoption** at a governed checkpoint (section 13), then the acceptance journey (1.3). The
   next release deletes the import and every pre-foundation path.
8. **Later governed upgrades** on the running network, each using only extension contracts that
   already exist: the keyed commitment profile of `sumeragi.md` §16 (G.3), new rebinding
   implementations, specific migrations, the IVM instruction adapter (section 10), and independently
   rooted data spaces with their own selections. A change set that
   asks for anything not yet implemented is rejected.

Minamoto adopts the foundation only after every gate passes on Taira.

## 15. Gates, costs and decisions

Gates:

- Every retained history above the startup floor replays byte for byte, and every active
  definition passes its conformance corpus.
- Definitions compiled in but not in the selection change no byte, result or root.
- Cold and incremental roots agree across activation, aborted publication, block replacement and
  restart.
- Four-validator old/new cohorts at 1/3, 2/2 and 3/1 agree before activation; stragglers stop before
  it; restarts around `H` and lane batches certified before `H` but merged after behave as
  specified. Deterministic simulator scenarios and named mutation tests cover each new safety or
  liveness rule.
- For every nesting of unknown and inactive envelopes, every release stops at the same envelope.
- Stale-committee attestations and interrupted activations are refused or resumed as specified.
- Restriction never strands an obligation: AMX escrow, a migration cursor, a scheduled trigger and a
  multisig proposal each complete, suspend or require renewed approval as specified.
- No implementation touches storage it did not declare, by test-build access tracing kept apart
  from the undo journal (which cannot record reads, `mv/storage.rs`) and by storage capabilities in
  production.
- Existing calls, rejections, contract hashes and event consumption, from older SDK and indexer
  builds too, survive an unrelated addition.
- A node started from a checkpoint reaches the same tip and roots as one that replayed, and a
  tampered checkpoint is refused.
- Retirement rehearsal: a version is restricted, drained, disabled, its code physically deleted, and
  a node then starts from the oldest accepted checkpoint and catches up.
- Emergency rehearsal: containment authorized by the panel takes effect at the next block, expires
  when unratified, and is ratified, renewed or lifted only by approvals that postdate the incident.

Costs:

- One coordinated switch on Taira at the adoption checkpoint: validators stop at `H` and restart on
  the foundation release.
- Discipline on shared code: a change that alters any active definition's semantics is a new
  definition.
- A broad execution refactor: direct native calls, authorization, fees, hooks, queries and events
  all go through the selection and storage capabilities, and generic schemas and storage must exist
  before publishing bytecode instructions means anything.
- Composition qualification: individually correct definitions can break shared monetary or
  authorization invariants together. Attestations bind the tested composition; they do not prove it
  correct.
- All-`n` attestation lets one validator delay an activation.
- Conditional availability: with four validators, one withholding and one restarting leave fewer
  than three, so a rolling upgrade pauses rather than promising zero downtime.
- Complete-State coverage for LtHash, checkpoints, Parliament's Core integration and the emergency panel
  all land before Taira's adoption, which puts it later than the execution foundation alone would.
- Some validity inputs still come from node configuration (`status.md`, ZK delivery task F.4, for
  example `governance.runtime_upgrade_provenance`); they move to committed State with the
  foundation. A definition must not add fields to the genesis-signed `execution_policy_hash` or
  `nexus_amx_context_hash`.

Decided by the owner: SORA Parliament governs the universal root, and the admins of a permissioned
data space govern only that data space (section 6). Taira adopts the foundation at a governed
checkpoint that becomes its trust anchor, keeping balances and history (section 13). The
first-release rule holds: the import and all pre-foundation code are deleted after adoption, and old
code is deleted rather than kept (7.4). The foundation starts on the LtHash commitment extended to the
complete State; the keyed commitment follows as a governed profile transition (5.1).

Recommended by review and written in as committed defaults, for the owner to confirm or change: the
emergency panel and its leases (7.1), the notice values (section 6), the checkpoint policy (7.3),
a readiness-gated Taira adoption
(section 14), and the client contract (8.4). Every value is a committed parameter, so changing one
later is itself a governed change.
