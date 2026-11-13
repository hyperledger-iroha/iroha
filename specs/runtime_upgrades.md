# Runtime change sets — as built

Status: as built, 2026-11-11. [`live_runtime_upgrades.md`](live_runtime_upgrades.md) is the
design; this document records the implemented surface of its change-set lifecycle (§6) for the
universal root. The ABI-v1 manifest records, their `ProposeRuntimeUpgrade`,
`ActivateRuntimeUpgrade` and `CancelRuntimeUpgrade` instructions, the
`CanProposeRuntimeUpgrade` and `CanManageRuntimeUpgrades` permissions, the
`world.runtime_upgrades` table, and the `governance.runtime_upgrade_provenance` node configuration
and its admission check are deleted.

## Model

- A World is founded on the release's baseline selection (`iroha_core::runtime::catalog`,
  pinned in [`runtime_baseline_catalog.json`](runtime_baseline_catalog.json)).
  `world.runtime_definitions` holds the published definitions and `world.runtime_selection` the
  one `RootSelectionV1` the universal root executes. Every instruction resolves against that
  selection; an unbound wire ID or a restricted definition fails with a stable
  `RuntimeRejectionV1`.
- A `ChangeSetV1` (`iroha_data_model::runtime::lifecycle`) moves the root from its `predecessor`
  selection to its declared, closed `successor` at `effective_height`, publishing new definitions
  and applying ordered `SelectionTransitionV1`s. Its identity is the domain-separated hash of its
  canonical bytes.
- `world.runtime_change_sets` keeps one `ChangeSetRecordV1` per change set: the change set, its
  status (`Scheduled`, `Activated(h)`, `Lapsed(h)`, `Cancelled(h)`), the notice frozen at proposal,
  proposal and approval times, and the latest `SignedSupportAttestationV1` of each validator.

## Lifecycle

1. **Propose.** The baseline `ProposeRuntimeUpgradeProposal { action: RuntimeUpgradeActionV1 }`
   submits `Schedule(change_set)` or `Cancel(id)` to SORA Parliament. It needs the unit permission
   `CanProposeRuntimeChangeSet`. Admission reads the committed policy
   `runtime_upgrade_policy_v1` (`RuntimeUpgradePolicyV1`); a root without it admits no change set.
   The release addition `ProposeEndorsedRuntimeUpgradeProposal { action, provenance }` carries the
   release signers' endorsements the committed provenance policy requires (below); the baseline
   proposal keeps its frozen layout and checks no provenance.
   A change set must name this network and the universal root (data-space roots are a TODO of
   §11), give the policy's block notice (the disable notice if it restricts, deactivates, unbinds
   or retires anything) and apply to the selection that scheduled change sets will leave at its
   height. The notice in force is frozen into the proposal with its committed time.
2. **Approve.** When Parliament enacts the proposal, the change set is admitted again against the
   current selection and recorded `Scheduled` (event `RuntimeUpgradeEvent::Scheduled`), or the
   scheduled record is `Cancelled` (`Canceled`).
3. **Attest.** `AttestRuntimeSupportV1 { attestation }` records one validator's signed support:
   the change set, its predecessor and successor, network, root, the digest of the current
   committee as a set of keys, the tested checkpoint, a validity interval and the release digest.
   The signer must be a current committee member; a newer attestation replaces the validator's
   earlier one (`Attested`). The node decides support, not its operator
   (`iroha_core::runtime::attestation`): it resolves the successor the change set produces from
   committed State, refuses unless its release implements every definition that selection relies
   on and its key is a committee member, and signs with its own validator key, which never leaves
   it. The attestation counts exactly at the effective height and binds the release digest,
   `H(RELEASE_DOMAIN ‖ build status ‖ every definition the release implements)`. Until checkpoints
   exist, its tested checkpoint is the committed height whose State the node resolved the
   successor against.
4. **Activate or lapse.** At the start of the effective height, before ordinary execution, a
   scheduled change set activates only if every committee member attested for this committee, the
   attestations count at that height, the committed block time satisfies the frozen notice and it
   still applies to the current selection. Otherwise it lapses and never applies (`Activated`,
   `Lapsed`). Change sets due at one height apply in identity order.
5. **Stop the unsupported.** If the resulting selection names a definition this release does not
   implement, block start fails with `StateStorageAdmissionError::UnsupportedRuntime`, which
   consensus classifies as recovery-required: the node halts with
   `PublicationRecoveryRequired(height)` before judging the block (served by
   `/v1/sumeragi/status` and `sumeragi_halted{reason="publication_recovery_required"}`; the halt
   reason names the selection and the first definition the release lacks) and resumes once a
   release that executes the selection replaces it. `GET /v1/runtime/release` reports the same
   before the boundary: for the committed selection and the successor of each scheduled change
   set, whether this release executes it and, if not, the definition it lacks.

IVM admission accepts a contract only when its ABI hash is one of the host profiles the committed
selection activates.

## Release additions

Once a network installs the baseline it executes it unchanged, so a release adds instructions and
World fields after it (design §4, "Add an instruction", "Add storage"):

- **Declaration.** `iroha_core::runtime::catalog::additions::ADDITIONS` lists each addition: a
  native module, the wire IDs it adds, the earlier wire IDs it rebinds and the World tables and
  cells it adds (a cell with the canonical bytes of its initial value and the installer that writes
  them). The baseline is everything compiled that no addition claims. The pinned inventory
  (`runtime_baseline_catalog.json`) lists only the baseline, so a new instruction or field fails it
  until it is declared. Every baseline instruction's payload layout is pinned too
  (`runtime_baseline_instruction_layouts.tsv`, checked by `iroha_data_model`'s
  `baseline_instruction_layouts_stay_frozen`), because an interface descriptor names only the
  payload's nominal type: a changed layout under an unchanged wire ID fails that pin. The pin holds
  the baseline layouts this release brings to Taira. They match a660d601da, which founded Taira's
  selection, except `iroha.register` and `RegisterSmartContractCode`. Those reach trigger filters,
  which gained `RuntimeRejectionV1::MalformedPayload` and the `ContainmentReviewed` and
  `ContainmentResolved` bits of `RuntimeUpgradeEventSet`. Both additions are appended, so every
  payload a660d601da wrote still decodes. a660d601da cannot decode a payload that uses them, and
  its peers fail this release's handshake (`ConsensusConfigCaps`), so Taira's validators switch to
  this release together. `iroha.kagemusha.wallet.ledger.v1` likewise gained three appended
  actions, the native-suffix Unload stages `SuffixUnloadBegin`, `SuffixUnloadAppend` and
  `SuffixUnloadFinalize` (wire tags 11–13); its earlier actions keep their tags and bytes, and the
  stages keep their rows in the installed `world.kagemusha_wallet_ledger` table, so they need no
  addition and no new table. The threshold-key lifecycle retains its original three actions and frozen layout.
  The owner-authorized Taira SoraFS repair is a separate instruction and kernel
  (`sorafs/taira_repair_v1.md`); it does not reinterpret that lifecycle.
  A storage descriptor likewise names only its
  value's nominal type, and World value layouts are not pinned yet (TODO: pin them from the State
  field registry the same way). This release declares the following additions, in activation order:
  `iroha.runtime.endorsed_change_set_proposal.v1`, and `iroha.game_partition.v1`, which adds the
  six `iroha.game_partition.*.v1` instructions and the `world.game_partitions` table. Operator
  drafting and node-owned support signing refuse newly selecting its export definitions
  until in-flight unit exports own captured completion obligations. This also refuses dependent
  additions and service recompositions that introduce those definitions. Pure catalog assembly
  remains available for schema inventories and component fixtures; it grants no activation
  authority. This is a release qualification guard, not a retrofit to the frozen kernel's
  consensus rules. Already selected historical definitions remain executable, and containment,
  drain and retirement remain possible. The third addition, `iroha.sorafs.bootstrap.repair.v1`,
  publishes a new interface, implementation and separately identified kernel; its exact
  Taira-only transition changes no existing binding, profile or installed storage.
  The independent `iroha.sorafs.authority_governance.v1` candidate adds two tolerant
  proposal/attempt interfaces and three authority tables, selecting nondelegable
  permission-mutation services. Its draft keeps the existing repair kernel and
  does not install game partitions. It supports the baseline services composition;
  other service compositions fail closed. Both the Parliament-grantable completion
  permission and the provider owner's exact completion-signer policy remain
  required. Its protected gateway and provider-source HTTP operations refuse
  delegated account signing keys under the selected authority services.
  [Authority governance](sorafs_authority_governance.md) describes the
  paired term approvals and the outstanding current-governance activation prerequisites.
  The independent `iroha.runtime.lifecycle.v1` addition is the kernel's lifecycle State: the
  `world.runtime_obligations`, `world.runtime_migrations` and `world.runtime_boundaries` tables,
  the two `iroha.runtime.obligation.*.v1` instructions and the two `iroha.runtime.boundary.*.v1`
  instructions (Obligations, Migrations and Checkpoint records, below). It is independent of the
  ordered additions, which do not declare its tables. An implementation commits its services profile and
  the closure it may touch, and an interface the events it may emit, so the catalog compiles
  the addition once per predecessor family it supports (`runtime::catalog::kernel_lifecycle`):
  the baseline, Time, Ironwood and Ironwood with Time. Each composition keeps the addition's
  tables and wire IDs and takes the family's services profile, the events of its native bundle
  and that bundle's declared closure; the baseline family's composition is the addition as
  declared. The draft picks the composition by the services profile the root selects, so a root
  activates the lifecycle State from the selection it executes, Taira after its Time and
  Ironwood activation; a root whose services profile no family uses gets no draft.
- **Definitions.** Each addition publishes one native implementation, one interface per wire ID it
  adds and one storage definition per field; a cell's definition carries its initial bytes. Like
  the baseline bundle it declares a conservative superset of dependencies, every storage definition
  and interface of the baseline and of the additions before it, so additions activate in
  declaration order. The catalog publishes them without installing or activating anything.
- **Activation.** `ReleaseCatalog::activation(module, current, published, network, height,
  qualification)` drafts the change set that moves a root to an addition: it publishes the
  definitions the root lacks, installs the addition's storage, activates its interfaces and
  implementation and binds its wire IDs and rebinds to it. Until it activates, every release
  rejects an added wire ID as `definition_inactive`, and a release without the addition stops
  before the effective height.
- **Dispatch.** Execution runs the implementation the selection binds: `runtime::resolve`
  returns it, an addition that rebinds a wire ID answers it with its own handler
  (`ADDITION_HANDLERS` in `smartcontracts/isi/mod.rs`, required for every rebind), and every
  other implementation runs the instruction type's native handler.
- **Storage installation.** Installing a table starts it empty. Installing a cell writes the
  initial bytes its published definition declares through the installer of the release that adds
  it; a node whose release lacks the storage stops at block start like any unsupported node. The
  World root commits installed storage from its installation height.
- **Snapshots.** A snapshot holds the World members its selection installs (design §5.5). The
  writer leaves out each top-level member whose storage is not installed
  (`runtime::uninstalled_world_members`), and a restore reads it as its empty default and refuses a
  snapshot that carries it. Until a change set installs an addition's storage, a node of this
  release therefore writes the snapshots a node of the previous release writes, and a rollback
  before activation is a binary swap. A field is installed when the selection installs a storage
  definition of its namespace, whichever definition that is, so a field stays in snapshots when
  an identity cutover replaces its definition.
- **Nested fields.** An addition may also add a field inside a nested World owner. The field
  registry names each field's snapshot member (`authority_registry::Field::member`), the writer
  leaves an uninstalled nested member out of its owner's object, and the owner's reader refuses
  a snapshot that carries it and reads it as empty. The trigger set (`world.triggers`) is the one
  nested owner; it decodes and re-encodes through the selection, so the canonical-bytes check of
  a restore still holds. `runtime::SELECTION_AWARE_OWNERS` lists the owners whose reader applies
  the selection, a test requires every nested owner of the World to be listed, and the catalog
  refuses an addition whose field has no such member.

## Envelopes

Every release decodes the same bytes alike and stops at the same envelope (design §5.2):

- **Opaque envelopes.** Every `InstructionBox` decoder goes through
  `isi::decode_instruction_envelope`. A wire ID the release registers decodes into its typed
  instruction; any other well-formed wire ID (`isi::is_well_formed_wire_id`: the interface wire-ID
  rule, at most 192 bytes of ASCII letters, digits and `. _ : - @ /`) decodes into an
  `OpaqueInstructionV1` that keeps the exact frame and re-encodes as the same pair, so hashes and
  signatures do not depend on the release. A wire ID breaking the rule, and malformed pair framing,
  stay decode errors. `isi::decode_instruction_from_pair` remains the strict typed decoder for
  clients. Signed attachment actions name wire IDs under the same rule, never by registration.
- **Tolerant registration.** An addition's instructions register with
  `built_in_wire_id!(T => "id", tolerant)`: a frame that is not the canonical frame of a payload
  the decoder accepts (invalid bytes or the decoder's own protocol limits) yields the opaque
  envelope an earlier release keeps, never a decode error; a local refusal (allocator failure or
  the caller's enclosing decode budget) stays an error. Baseline instructions register strictly.
- **Validation.** Before any instruction-dependent step (admission, fees, policies, scope),
  transaction validation refuses the transaction at its first top-level envelope (instructions and
  batch instruction items, in order) that execution can never apply: an unbound wire ID
  (`definition_inactive`), or a bound envelope the release kept opaque (`malformed_payload`, after
  its restrictions), committed exactly as execution commits it (`runtime::envelope`). A decoded
  envelope's restrictions stay with execution and its obligation context. Queue admission refuses
  such a transaction against the committed selection (`queue::Error::RuntimeRejected`, Torii code
  `definition_inactive` or `malformed_payload`). Envelopes emitted during execution (contracts,
  triggers, approved proposals) get the same refusals when they execute, before any executor's
  own admission.

## Emergency containment

`ContainRuntimeV1 { containment, endorsements }` lets the runtime emergency panel restrict
definitions without waiting for Parliament (design §7.1). The committed parameter
`runtime_emergency_panel_v1` (`RuntimeEmergencyPanelV1`) holds the sorted, disjoint primary and
backup members, the threshold (at least two thirds of each panel), the term end and the lease
bounds; a root without it admits no containment. Any account may submit; Core requires the
threshold of distinct members of one panel to have endorsed the `ContainmentV1` identity within
30 minutes before the block's committed time, the term to be running, the lease to lie within the
bounds and the containment to name this network and the universal root.

- **Effect.** Every target is an active interface or implementation that answers no governance
  (`iroha.instruction.v1::governance::*`), parameter (`iroha.set_parameter`) or lifecycle
  (`iroha.runtime.*`) wire ID. From the next height it is `AdmissionClosed` (or `CompletionOnly`),
  unless already restricted at least as much. The `ContainmentRecordV1` in
  `world.runtime_containments` records each restriction it replaced (`Contained`). A root that
  has activated the kernel's lifecycle State also contains a running migration (its record is
  `Running`), only by closing its admission: its stages stop while its cursor and write barrier
  stay, and expiry or a lift resumes it. Containment never cuts a migration over; any other
  root refuses a migration target as the baseline does.
- **Ratification case.** Each containment opens a case that ordinary governance closes with change
  sets. When Parliament admits or approves a change set that addresses a contained definition (it
  activates, deactivates, restricts or lifts it, or binds or unbinds a wire ID behind it), the
  record's `review` notes that change set and height (`ContainmentReviewed`). When the change sets
  activating at a height take over every restriction the containment held, it becomes `Resolved`
  (`ContainmentResolved`). A change set approved before the incident names the selection that
  containment replaced, so it no longer applies and cannot undo it.
- **Renewal.** A containment of the same incident, targets and kind, with fresh endorsements, takes
  the lease over once half of it has passed, and only with review progress recorded since the
  containment it renews; the old record becomes `Renewed` and the new one carries the progress
  over.
- **Expiry.** At block start, after the height's change sets, a containment whose lease ended
  restores every restriction it still holds and becomes `Expired` (`ContainmentExpired`). A
  restriction governance changed in the meantime is no longer held, so ratification, repair or a
  lift through an ordinary change set is never undone.

CLI: `iroha ops runtime contain list` reads the records; `iroha ops runtime contain endorse --file
<containment.json> --member-key-file <path>
[--at-ms <ms>]` prints one member's `EndorsementV1`; `iroha ops runtime contain submit --file
<containment.json> --endorsement <file>...` submits them.

## Obligations

Two kinds of committed State are obligations (design §7.2, `iroha_core::runtime::obligations`);
each committed record is its registration:

- a live asset escrow or lock (status `Open`, `Accepted`, `PaymentSent`, `Disputed` or `Locked`)
  in `world.asset_escrows`;
- a live AMX participation (`sumeragi.md` §11): on the global chain, a transaction of
  `world.sumeragi_amx` whose `Begin` is recorded and that is not decided; on a participant root,
  an escrow in `world.sumeragi_amx_participant` of a `Yes` vote that no decision has settled. A
  `No` vote escrows nothing and is not live.

A restriction never removes their settlement authority:

- **Dispatch.** While a definition behind an instruction has admission closed, the instructions
  that continue or complete a live obligation still run; while it is completion-only, only the
  completions run. Starting new work runs under neither, and disabling stops every instruction.
  Accepting an escrow's offer continues it; marking payment sent, releasing, cancelling, opening
  or resolving a dispute, attesting a condition, drawing down and expiring (with the
  controlled-lock forms) complete it; opening custody is new work. On the global chain, relaying a
  `Prepared` proof or expiring completes an undecided transaction, and relaying a participant's
  handoff completes the undecided transactions it takes part in. On a participant, settling
  completes its unsettled escrow of the decided transaction, and relaying a global handoff, which
  a decision certified in a later global epoch needs, completes its unsettled escrows. Registering,
  beginning and preparing are new work: a participant cannot tie a `Prepare` to an admitted
  transaction before verifying its proof, and refusing one strands nothing, because the global
  chain aborts by the deadline and the `Yes` voters settle that abort. The deadline step is a
  block hook, not an instruction.
- **Activation without captured custody.** A change set whose successor changes an original binding of a live obligation's
  completions or of their transitive declared calls (rebinding, unbinding or newly binding it), or
  stops executing a required definition the predecessor executes (its storage, the kernel, the
  commitment or services profile, and for escrows the host profiles, since controlled locks invoke
  contracts), lapses at its effective height. AMX completions are native and retain no host
  profile. A binding the predecessor already leaves out, or a definition it already disables, is
  no part of the executable closure: keeping it so blocks no change set, and lifting such a
  restriction restores the same code. The baseline bundle conservatively declares all its native
  callees. Keeping a replacement active does not transfer the original obligation to it. Until
  a root selects native escrow capture below, rebinding those escrow dependencies requires
  draining the obligations first: closing admission to new escrows, `Begin` and `Prepare`, then completing
  what is live. AMX requires the same drain until its independent capture service is selected.
- **Closure.** A selection's completion closure may name the kernel and the commitment and
  services profiles besides active and installed definitions, but no disabled one
  (`SelectionError::ObligationStranded`). A restriction targets only an active or installed
  definition; the kernel and profiles are replaced, never restricted.
- **Executor replacement.** `Executor::migrate` refuses while any obligation remains live (an
  escrow or lock, an undecided AMX transaction or an unsettled AMX escrow), before loading code or
  running migration effects. This includes identical-code migrations, which can still change
  permissions or State. Replacing the authorization component requires drained obligations until
  captured completion authority or a certified transfer is implemented.
- **Every execution path.** Resolution, with restrictions and the obligation an instruction acts
  on, runs before any check that depends on what the instruction is, on every path that executes
  one: the native dispatch table and the controlled-lock, linked-FI and multisig paths that bypass
  it. A containment therefore holds whichever path runs an instruction.

### Captured native escrow custody

The independent `iroha.runtime.native_escrow_capture.v1` addition selects a new escrow-opening
implementation and services profile with the `world.runtime_escrow_captures` table. Activation
drains earlier live custody; it never reconstructs an original capture from current bindings.
The frozen escrow record and baseline definitions remain unchanged.

Before funding, each native owner prepares its canonical completion graph and binds it to the
escrow's immutable custody identity, fixed recipient and conditional release terms. The row and
capture publish together. Ordinary progress retains the capture; successful terminal custody
updates remove it. A child orderbook lock inherits its parent's original graph before partitioning
funds. Restore refuses missing, orphaned or substituted captures.

Direct marketplace, generic-lock, conditional and controlled-lock operations select that captured
graph before permissions and native execution, retaining current restrictions. Acceptance remains
`Continue`, so completion-only still refuses it. A caller's registered terminal effect cannot
replace the escrow's context or turn acceptance into completion; matching registered consent is
consumed only after success. Nested contexts must agree, and scope cleanup restores the enclosing
selection on rejection or unwind without changing the canonical selection.

Controlled-lock captures retain native runtime bindings and host profiles. Existing controller
authentication still requires the active contract instance; these captures do not retain a
deployed contract's earlier code or bypass its lifecycle. Contract-owned captured execution and
the new host-profile kernel call remain separate work.

Orderbook operations can act on several locks, so their original graph remains pinned while any
orderbook custody lives. Internal monetary paths check the actual execution context before moving
funds, including when an enclosing registered obligation selected older code. Root-wide profile
changes and executor replacement still require draining custody. These source paths require the
current candidate's funded and transition regressions; they do not qualify AMX or game exports.
Until native parent completion dispatch is implemented, live orderbook custody also prevents
admission-closed or completion-only restrictions on cancellation, maintenance and settlement-receipt
interfaces or their shared implementations. New-order and matching interfaces can still close so
custody can drain. Emergency disabling retains its separate quarantine semantics.

### Captured native AMX custody

The independent `iroha.runtime.native_amx_capture.v1` addition installs
`world.runtime_amx_captures` and a services profile owning native AMX decisions, settlement and
automatic deadline expiry. Activation drains prior undecided global transactions and unsettled
participant Yes custody; it never infers their original authority from current bindings. The
frozen baseline and original AMX record layouts remain unchanged.

Before a Begin witness or a verified Yes witness and debit, the native owner captures its
original completion graph and immutable opening identity. Global captures bind the original
Begin, timing and participant tracker identities; participant captures bind the authenticated
global root, original vote and exact funded monetary effects. Only a successful authoritative
decision or settlement removes its capture. Local codec and allocation refusals retain their
typed resource custody and leave the original owner retryable.

Direct relays, expiry and settlement execute the captured bindings with current restrictions.
Every operation that changes a shared tracker checks all its live owners; differing original
contexts refuse before mutation. A creator, unknown transaction or empty handoff cohort has
ordinary admission and cannot borrow an enclosing completion capability. New work must also
agree with existing shared tracker custody. Restore refuses missing, orphaned or substituted
captures and inconsistent cohorts.

Automatic expiry checks every due capture and disabled dependency before its first witness,
then removes captures only after all original decision writes succeed. Its pinned services
profile supplies the fixed finalizer. AMX operations invoke no IVM code, so their captures retain
no unrelated host profile; transitive implementation dependencies remain retained. Root kernel,
commitment or services changes and executor replacement still require draining live custody.
These connected source paths require the current candidate's signed-root, resource-refusal and
transition regressions before qualification.

### Registered obligations

An account registers an obligation with the kernel (design §7.2;
`runtime::obligations::registry`, `iroha_data_model::runtime::obligation`). The registry is
storage of the `iroha.runtime.lifecycle.v1` addition, so a root that has not activated it holds
no record and executes exactly as before.

- **Register.** `RegisterRuntimeObligationV1 { contract, kind, terminal_effects, expires_ms }`
  commits to the exact instructions that complete the obligation: at most 16, each identified by
  the hash of its canonical envelope, none one of the lifecycle's own. The kernel captures their
  completion closure from the selection the root executes (`CompletionCaptureV1`): the binding of
  each terminal effect, the binding of every interface those implementations invoke, the kernel,
  both profiles and the active host profiles. The record in `world.runtime_obligations` is keyed
  by its owner, who holds one obligation; registering again replaces it. It expires at most 30
  days later. Registering is new work: it is refused while the interface or implementation of a
  terminal effect is restricted.
- **Contracts.** The first-release ABI gives a contract no kernel call: its descriptor fixes
  every syscall and operation tag, and the host profile commits that descriptor. The account that
  controls a contract's lifecycle therefore registers for it (`contract: Some(address)`), and the
  contract's subject account owns the obligation.
- **Complete.** When the owner issues one of the committed instructions, the kernel executes it
  as a completion and marks it performed on success: admission closure and completion-only on the
  definitions behind it, or behind what it invokes, do not stop it. Each committed instruction
  lends that capability once, to its owner alone, so a capability creates no new work. The
  instructions a contract queues carry its subject account as their authority and complete its
  obligation in the same way. `CompleteRuntimeObligationV1 { owner }` ends the obligation: its
  owner, the account that controls the owning contract's lifecycle, or anyone once it has
  expired or performed every committed instruction.
- **Activation.** A new binding may coexist with a registered obligation's captured implementation.
  The completion reads an ephemeral selection containing its captured bindings, host profiles and
  dependency closure, with the root's current restrictions. Native dispatch, authorization and IVM
  host selection read that same context. Calls outside the captured closure receive no completion
  capability. The context never replaces the committed selection or its undo journal, and is
  released after success, a returned error or unwind. A rejected effect remains pending even when its
  caller catches the rejection and commits surrounding work; the registry remains live during
  execution, so an in-flight completion cannot bypass executor-replacement drain checks.
  Root-wide policy writes explicitly preserve the canonical selection.
  Removing or disabling a required definition, removing captured storage, or changing the kernel,
  commitment profile or services profile still lapses while the obligation is live. Executor
  replacement still waits for registered obligations as it does for escrows.

Selection-level `RetainCompletion` and `ReleaseCompletion` stay refused: the registry holds every
completion closure, and no engine reads one from the selection. Native escrow and AMX captures use
their separate custody tables above; roots without those services retain the original drain rules.

## Deferred consent

The independent `iroha.runtime.deferred_consent.v1` addition publishes the
`world.runtime_deferred_consents` table, explicit current-binding registration interfaces and new
services-profile compositions. Its activation selects the service and table together and replaces
native implementations with definitions that declare the consent table. The frozen baseline stores
and checks no capture; its proposal and trigger layouts remain unchanged. Activating or retiring
the service requires every existing trigger and pending multisig occurrence to drain; current
bindings never manufacture consent for earlier work.

Under the selected service, default registrations capture resolved bindings and transitive declared
dependencies for the exact trigger incarnation or multisig occurrence. Opaque contract execution
conservatively captures the whole selected call surface. The record binds its authority and
executable digest. Each approval and execution checks the capture; every nested instruction must
also satisfy each enclosing expectation. Changed or restricted captures suspend scheduled triggers
without consuming their repetitions. Multisig renewal uses authorized cancellation or invalidation
followed by a fresh nonce and fresh approvals.

`RegisterCurrentBindingTriggerV1` and `ProposeCurrentBindingMultisigV1` explicitly consent to current
bindings when creating a new occurrence, through the same native authorization paths. There is no
endpoint that changes an existing occurrence's consent. Opt-in execution still obeys ordinary
permissions and runtime restrictions. Narrower authenticated dynamic-call manifests remain open.

## Migrations and storage retirement

The kernel moves the rows of old storage into successor storage in bounded, certified stages
(design §5.4; `runtime::migration`). A root admits a migration and a storage retirement only once
it has activated the lifecycle State (`migration::lifecycle_installed`, judged on the selection
the root executes when the rule is evaluated). Every validator then runs a release with the
engine, because the addition activated with every validator's support. On any other root both
are refused as before.

- **Declaration.** A release compiles a migration as native code
  (`runtime::catalog::migrations`): the old and successor World fields, the cursor's codec, the
  rows a stage moves, and four handlers. The catalog publishes one migration definition per
  migration and execution services profile it compiles. This release declares none.
- **Start.** A change set installs the successor storage and activates the migration. Its record
  in `world.runtime_migrations` (`MigrationRecordV1`: status, cursor, stages, rows) starts
  `Running`. At the start of every block, after the height's change sets and containment leases,
  one stage copies the next rows after the committed cursor into the successor. The successor is
  committed from the first stage, so every stage is certified and survives a restart. The old
  storage stays authoritative.
- **Write barrier.** As the last World write of every block, before the block's World commitment
  is folded, the barrier mirrors the old rows that block touched at or before the cursor. It
  reads the block's own journal of touched rows, so it covers every writer of the old storage:
  instructions, hooks and the block's tail writes. When no old row lies after the cursor the
  record is `Complete`, and the barrier keeps holding.
- **Suspension and faults.** Closing a migration's admission suspends its stages and lifting the
  restriction resumes them; the cursor and the barrier stay in place. A stage that fails writes
  nothing and faults the migration: no further stage runs, the barrier keeps holding, and a
  change set that deactivates it and activates a replacement over the same storage and cursor
  hands the cursor over.
- **Cutover.** A change set restricts the migration to completion only, with the transitions
  that move the release's readers and writers to the successor (including the execution services
  profile, when a block hook writes the old storage). It activates only once the record is
  `Complete`. From its height the successor is authoritative, the barrier is lifted, and each
  block one stage drains the next old rows. A write that still reaches the old storage stops the
  drain (`StaleWrite`).
- **Retirement.** A change set retires the old storage and deactivates the migration. It
  activates only once the record is `Drained`. The World commitment drops a retired field only
  when it is empty and unchanged in that block (`world_state_accumulator`), the snapshot leaves
  it out, and the record stays as the retirement receipt.
- **Structure.** Storage is retired only together with the deactivation of the migration whose
  old storage it is, and a migration is otherwise deactivated only together with its
  replacement. Two active migrations share no storage. The Ironwood identity cutover is the one
  retirement outside a migration: it replaces storage definitions over the same fields and moves
  no row.
- **Drafts.** `ReleaseCatalog::migration_step` (`runtime::migration::draft_step`) drafts the
  change set of each step from the committed selection; an operator has the node draft it with
  `POST /v1/runtime/migrations/draft` (`iroha ops runtime migration draft --migration <module>
  --step start|cut-over|retire --effective-height <height> --qualification <hex>`), and endorses,
  proposes and attests it like any change set.

A storage definition names the definition a release founded or added the field with. A field
whose definition an identity cutover replaced is migrated by a migration compiled against the
successor definition. Because the baseline bundle declares every baseline table, retiring a
baseline table also means replacing that bundle, which lapses while an escrow or AMX
participation is live.

## Checkpoint records

The checkpoint policy and the two certified facts about a root's history are committed State
(design §7.3, §13; `runtime::checkpoint`, `iroha_data_model::runtime::checkpoint`).

- **Policy.** The custom parameter `runtime_checkpoint_policy_v1` (`RuntimeCheckpointPolicyV1`)
  holds the production cadence, the retention minimums, the recovery window and the floor
  notice, all in committed milliseconds. A height is a checkpoint height when its block is the
  first committed in a cadence interval. A root reads the policy only once it has activated the
  lifecycle State and judges it when it reads it, because the frozen baseline stores whatever
  `SetParameter` writes under that identity; from activation on, `SetParameter` refuses a
  malformed one.
- **Boundaries.** `world.runtime_boundaries` holds the adoption anchor and the startup floors,
  keyed by height, as storage of the lifecycle addition. Only the addition's two instructions
  write it and activation starts it empty, so nothing a root committed before it activated the
  lifecycle State is ever read as a boundary. Every validator's statement is the authority of a
  record, so any account relays one.
  - `RatifyRuntimeAdoptionAnchorV1 { anchor }` commits the `AdoptionRecordV1` with one statement
    of it from every validator. The root holds no boundary yet, and the record names this
    network and the block the chain committed at its height. A ratified anchor never changes.
    `authorization` is zero for a network whose node configuration authorized the adoption.
  - `RaiseRuntimeStartupFloorV1 { floor, attestations }` commits a floor. A raise needs the
    committed policy, names the block the chain committed at its height, and lies above the
    latest floor and the anchor. The kernel, not the relayer, sets the time the floor comes into
    force: the policy's notice after the block that raises it, when its checkpoint must be at
    least the recovery window old. One raise waits out its notice at a time, and a raise drops
    the floors below the one in force, so the table holds the anchor, the floor in force and
    the floor waiting.

All `n` validators sign, never a quorum. A node derives the adoption record from its own store
(`checkpoint::adoption_record`) and signs a record only after checking it against its own
certified history (`checkpoint::attest_anchor`, `attest_floor`, over `CertifiedChain`): for the
anchor, that it is the upgrade checkpoint the node was configured to adopt at, the block, the
World root the predecessor certified after it, the root the first foundation block executed on,
the committee of that height and the founded selection; for a floor, the block, its committed
time and that the node holds the checkpoint.

`checkpoint::history_boundary` is the height at or below which no release re-executes a block:
the highest boundary in force. Kura retains the highest authenticated boundary in its private,
atomically synchronized `runtime-history-boundary.norito` record, bound to the durable genesis and
boundary hashes. It loads this record before choosing a snapshot, even without adoption
configuration. A lower configured anchor or older State never lowers it; malformed, foreign or
conflicting records fail closed. Certified publication persists an advance before State visibility,
and restored State advances it only after its snapshot claim authenticates. A snapshot below the
boundary is not a restore candidate. `[snapshot.upgrade_checkpoint]` remains the original adoption
anchor, separate from the highest learned startup floor. Isolated replay that cannot seed from an
authenticated checkpoint refuses a store with a boundary instead of copying and replaying genesis.

- **Operations.** `POST /v1/runtime/boundaries/anchor/attest` has a node derive the adoption
  record from its own store and return it with its statement; `POST
  /v1/runtime/boundaries/floor/attest` with `{ height }` has it state the floor at a checkpoint
  it holds from its own history (the block and committed time it stores there) and return it
  with its statement. The validator key never leaves the node. `iroha ops runtime boundary
  anchor-statement` and `floor-statement --height <h>` print a node's statement;
  `ratify-anchor --statement <file>...` and `raise-floor --statement <file>...` check that every
  statement names one record and verifies for it, from distinct validators, and submit the
  instruction. `GET /v1/runtime/boundaries` (`iroha ops runtime boundary list`) shows the
  policy, anchor, floors and history boundary, and the upgrade checkpoint and checkpoints the
  node holds.

### Checkpoint production and retention

A checkpoint at height `h` is the published State after block `h`, where `h` is the first block
committed in a cadence interval of the committed policy (`snapshot::checkpoint_store`). The
publication owner captures that height's canonical State from the finalized, frozen overlay before
visibility. Capture and persistence retries retain that original overlay and its exact prepared
bytes. After every State/World writer and commit fence is released, a nonblocking bounded channel
requests persistence of the same image. The publisher defers locally until the worker acknowledges
its durable private artifacts; it neither recaptures nor reexecutes the original. One writer-owned
job and one queued job bound work. Encoding checks the local payload limit, and queue, startup and
I/O refusals preserve bounded typed retry reasons. Original execution and reader resource failures
move once to publication with their actual cause; only real capacity refusals supply a pool release
observation. The writer cannot replace an unobserved refusal. Permanent artifact corruption retains
the original typed cause and image and requires recovery, without scheduling a resource retry.
Serialization and durable acknowledgement add
latency at cadence heights; writing an older image never moves the rolling snapshot pointer.

The durable spool is `checkpoint-pending/<height>-<committed_ms>`, separate from rolling GC and
committed retention. Its quota is two checkpoint identities, including unfinished staging copies.
Before acknowledging, the worker verifies and syncs bounded private copies of all five signed
artifacts and their parents. Pending images grant no floor or retention authority. Original State
visibility permits atomic directory promotion into the retained store, followed by parent syncs and
retention using the current committed schedule. After successful startup recovery, the worker first
reconciles orphaned pending images through the ordinary strict decoder and complete-State
certificate authentication, with no durable-journal recovery on the isolated decoded State. The
native tip and undo prove the original cadence. A valid image without its actual certified `H+1`
remains pending; the second slot allows the chain to progress. Damaged artifacts, excess quota,
configuration mismatch, and below-floor candidates remain refusals rather than being silently
removed. Pending startup candidates use the same strict chain, floor, custody and successor checks.

The writer keeps the generation's five canonical artifacts under
`checkpoints/<height>-<committed_ms>` as private independent files. Bounded copies verify exact
lengths and digests and preserve the strict single-link custody of both the retained checkpoint and
its source generation; rolling GC never removes the retained files. Strict restart discovers signed
immutable generations even if a crash preceded the first rolling snapshot pointer, and retained
checkpoints after rolling GC removes their source generations, with the same chain, floor,
successor and complete-State authentication checks. It then keeps what the policy retains (the floor's own checkpoint, the newest, every one
younger than `retain_all_ms` and one per `retain_spacing_ms` above the floor) and every floor
waiting out its notice, and removes the other checkpoints. A root without a committed policy
produces and keeps none. A node holds a checkpoint at `h` when an entry of its checkpoint store
at `h` has a manifest that verifies under its snapshot key and names its chain and network, and
five canonical artifacts whose payload matches the signed digest
(`snapshot::checkpoints::SnapshotCheckpoints`); only then does it sign a floor at `h`.

The configured writer is also owned by synchronous certified startup replay. If Kura append survives
without a capture or acknowledgement, replay prepares the certified block's original frozen overlay,
persists that exact image before visibility, and retries the same execution owner. It promotes each
visible replay image without retention, so a suffix with many cadence cuts does not fill the bounded
pending spool. A repeated restart verifies and syncs the exact retained original instead of making a
duplicate pending image. An interrupted staging copy is rebuilt only for that same certified height
and committed time; foreign or excess entries remain refusals. Rolling publication, orphan
reconciliation and policy retention still wait for successful startup. Crash and resource regressions
for these paths require current-candidate qualification. Transferred checkpoint bootstrap still requires a portable,
body-independent certificate-chain witness and sparse Kura history support; a locally signed
snapshot alone grants neither authority nor permission to discard earlier bodies.

The startup re-validation of the signed genesis execution policy stays after an anchor is
ratified. It is the only guard on the validity inputs a node still takes from its
configuration; it goes when those inputs are committed State (ZK delivery F.4).

## Notice policy

`runtime_upgrade_policy_v1` is a custom parameter holding canonical JSON of
`RuntimeUpgradePolicyV1 { activation, disable }`, each a `RuntimeNoticeV1 { blocks, ms,
approval_cushion_ms }`. `SetParameter` rejects a malformed value or a disable notice shorter than
the activation notice. A change set becomes eligible only when both the block floor and
`time ≥ max(proposed_ms + ms, approved_ms + approval_cushion_ms)` hold, in committed block time.
The type's defaults are the production values of the design (3,600 blocks; 14 days to activate,
30 days to disable; a 24-hour cushion); a network commits its own values explicitly.

## Provenance policy

`runtime_provenance_policy_v1` is a custom parameter holding canonical JSON of
`RuntimeProvenancePolicyV1 { signers, threshold }`: the trusted release signers, sorted and unique,
and how many of them must vouch for the build, SBOM and SLSA evidence of what a change set
publishes. `SetParameter` rejects an empty, unsorted or repeated signer list, a threshold of zero or
above the number of signers, and a payload over 16 KiB or not canonical. A signer endorses a change
set with `ChangeSetEndorsementV1 { change_set, signer, signature }`, a signature over
`iroha:runtime:change-set-endorsement:v1\0` followed by the change-set identity. While the policy is
committed, an endorsed proposal (`ProposeEndorsedRuntimeUpgradeProposal`) that schedules a change
set needs valid endorsements of that identity from `threshold` distinct trusted signers, and carries
at most one endorsement per trusted signer; endorsements of another change set, from other keys or
with bad signatures do not count. A cancellation, or a proposal under a root without the policy,
carries no endorsements. Admission checks them when the proposal executes; they are not stored with
the proposal, whose transaction keeps them in the block history.

The endorsed proposal is the release addition `iroha.runtime.endorsed_change_set_proposal.v1`
(`catalog/additions.rs`). The baseline `ProposeRuntimeUpgradeProposal` keeps its frozen semantics
and checks no provenance, so the policy binds a root's proposals only once a change set activates
the addition and governance restricts the baseline proposal.

## Restricted data space read delegation

The independent addition `iroha.dataspace.restricted_read_delegation.v1` rebinds
`iroha.grant`, `iroha.revoke`, `iroha.register` and `iroha.unregister`. The frozen baseline
keeps `CanReadRestrictedDataspace` genesis-only. After explicit governed activation, the
active ledger-backed SNS owner of the exact data space may grant or revoke that token.
Possessing a read token never confers delegation authority. Role membership, role permission
changes, role creation and role deletion retain the same owner check; a native handler also
enforces it when a user executor dispatches the instruction. Ownership alone grants no read
access, and existing tokens remain effective until revoked.

The catalog compiles distinct successors for its exact baseline, lifecycle, Ironwood and
Ironwood/lifecycle bundles. Each preserves its predecessor's interfaces, storage, services,
and containment. An unknown or partially rebound predecessor fails drafting. No storage is
installed, no token is issued by activation, and a binary upgrade does not select the addition.
Use the ordinary authenticated runtime draft, governance notice, provenance, validator
attestation and activation flow. This addition does not admit restricted asset homes on a
Global root or qualify their independent read/write boundary.

## Torii

| Route | Result |
| --- | --- |
| `GET /v1/runtime/abi/active` | `{ abi_version }` |
| `GET /v1/runtime/abi/hash` | `{ policy: "V1", abi_hash_hex }` |
| `GET /v1/runtime/metrics` | ABI version and change sets by status (`scheduled`, `activated`, `lapsed`, `canceled`) |
| `GET /v1/runtime/upgrades` | Committed selection and its identity, committee keys and digest, every change-set record |
| `POST /v1/runtime/upgrades/draft` | Body: `{ addition, effective_height, qualification_hex }`. Drafts, from the node's release catalog and the committed selection, the `ChangeSetV1` that activates that addition from `effective_height` (`iroha_core::runtime::draft_addition`), with its identity as `id_hex`; refused for a module the release does not add, naming those it adds, and for an activation that does not apply to the committed selection |
| `POST /v1/runtime/upgrades/propose` | Body: `{ change_set, provenance }`, the `ChangeSetV1` and its `ChangeSetEndorsementV1`s (empty without a provenance policy). Drafts the `Schedule` proposal for local signing: the baseline proposal without endorsements, the endorsed proposal (a release addition) with them |
| `POST /v1/runtime/upgrades/cancel/{id}` | Drafts the `Cancel` proposal for local signing |
| `POST /v1/runtime/upgrades/attest/{id}` | The node signs its support with its validator key and drafts the `AttestRuntimeSupportV1` instruction; refused unless its release executes the successor and its key is a committee member |
| `GET /v1/runtime/release` | Build, release digest, and whether the release executes the committed selection and each scheduled successor, naming the first definition it lacks |
| `GET /v1/runtime/containments` | Every containment record by authorization height |
| `POST /v1/runtime/migrations/draft` | Body: `{ migration, step, effective_height, qualification_hex }`, `step` one of `start`, `cut_over`, `retire`. Drafts, from the node's release catalog and the committed selection, the change set of that step of a migration the release compiles, with its identity as `id_hex` |
| `GET /v1/runtime/boundaries` | Whether the root activated the lifecycle State, its checkpoint policy, ratified adoption anchor and startup floors, the history boundary at the committed tip's time, and the upgrade checkpoint and checkpoints this node holds |
| `POST /v1/runtime/boundaries/anchor/attest` | The node derives the adoption record from its own store and returns it with its statement, signed with its validator key; refused when the node records no upgrade checkpoint, is not a committee member or its history does not certify the record |
| `POST /v1/runtime/boundaries/floor/attest` | Body: `{ height }`. The node states the floor at that height from its own history and returns it with its statement; refused unless it holds a checkpoint at exactly that height and is a committee member |

The list, propose and cancel routes back the `iroha.runtime.upgrades.*` MCP tools, beside the
`iroha.runtime.abi.*` and `iroha.runtime.metrics` tools.

Release, upgrade and containment reads use the operator-signature boundary, as do
the upgrade draft, proposal, cancellation and attestation endpoints. Anonymous
reads return `401 operator_signature_missing`; an account permission or API token
does not replace the dedicated allow-listed operator key. The client signs these
requests against the exact network, method, target, body, timestamp and nonce.
The Taira doctor's anonymous runtime probes check only that this boundary is
mounted, and fail on a missing route. They do not verify a node's selection.

## CLI

- `iroha ops runtime abi active|active-query|hash`, `iroha ops runtime status`, `iroha ops runtime capabilities`
- `iroha ops runtime upgrade list`
- `iroha ops runtime upgrade draft --addition <module> --effective-height <height> --qualification <hex>`
  Use the addition name `iroha.keyed-state.radix-blake3.v1` to draft the next adoption
  step from the root's committed selection. The foundation kernel first switches to
  the adoption kernel. Once that change has activated, a second draft imports the
  existing State owners and switches the commitment profile (see
  [governed State commitment](live_runtime_upgrades.md#51-complete-state-selected-by-history)).
  prints the change set the node drafts to activate one of its release's additions, after checking
  it against the identity the node reported; it is the `--file` input of `endorse` and `propose`.
- `iroha ops runtime upgrade endorse --file <change_set.json> --signer-key-file <path>` prints a
  release signer's `ChangeSetEndorsementV1`; the key is read at run time and never printed.
- `iroha ops runtime upgrade propose --file <change_set.json> [--endorsement <file>...]` submits the
  `Schedule` proposal with the endorsements.
- `iroha ops runtime upgrade cancel --id <hex>` submits the `Cancel` proposal.
- `iroha ops runtime upgrade attest --id <hex>` has the node sign its support and submits the
  drafted `AttestRuntimeSupportV1`; the operator never handles the validator key.
- `iroha ops runtime release` shows the node's release support.
- `iroha ops runtime migration draft --migration <module> --step start|cut-over|retire
  --effective-height <height> --qualification <hex>` prints the change set the node drafts for
  one step of a migration of its release.
- `iroha ops runtime boundary list|anchor-statement|floor-statement --height <h>` read the
  boundaries and print a node's signed statement; `iroha ops runtime boundary ratify-anchor
  --statement <file>...` and `raise-floor --statement <file>...` submit the instruction that
  relays every validator's statement.

For these operator reads, pass `--operator-private-key-file <absolute-runtime-key>`
and `--config <runtime-client.toml>` before `ops`. Keep both inputs outside source
control. Use a current client: a deployed client that still sends an unsigned
runtime request fails with `operator_signature_missing` even when its global
operator-key option is supplied.

## Contract kernel host profile

The independent `iroha.runtime.contract_lifecycle.v1` addition selects
`iroha.ivm.runtime_lifecycle.v1` beside installed lifecycle State. It uses ABI version 1 with its
own immutable descriptor hash. Baseline header admission and operation tags keep their existing
meaning. `koto build --host-profile iroha.ivm.runtime_lifecycle.v1` emits the exact named header,
embedded interface and manifest. The root must select that profile for execution, including queries,
triggers, nested calls and warmed cache entries. Effective restrictions apply at the original
execution height; only an original captured completion can borrow its retained host profile.

The named profile extends `0xA0` with registration and completion tags. Registration has no
caller-supplied address or owner: it uses the authenticated active contract subject and entrypoint
permission, then queues the canonical instruction through ordinary host gas, output and authorization
checks. It cannot register from pending lifecycle hooks or borrow a retained scope for new work.
The kernel captures the exact terminal instructions; completion uses the existing registry handler.
Registration retains runtime definitions, not deployed contract code. Broad captures conservatively
refuse new obligations that would retain an already restricted named profile; narrower authenticated
dynamic-call manifests remain open. The contract, cache, compiler and deployment regressions require
current-candidate qualification.

## Handshake

Each peer's handshake advertises the host profiles its release implements
(`ConsensusConfigCaps::host_profiles`, an `iroha_p2p::HostProfileSet` of at most eight sorted
identities). A node refuses a peer whose set is not canonical or lacks a host profile the committed
selection activates (`ConsensusHandshakeCaps::required_host_profiles`); extra, dormant profiles never
block a connection. `irohad` seeds the requirement from the committed selection at startup and
refreshes it after every `RuntimeUpgradeEvent::Activated`. The compatibility probe reports
`host_profiles` instead of the former compiled gas-schedule hash.

## Telemetry

`runtime_abi_version` and `runtime_upgrade_events_total{kind=scheduled|canceled|attested|contained}`.

## Operations

`iroha3d --check-replay` replays a stopped validator's store, or an immutable copy of it, with a
candidate release before it is installed ([`live_runtime_upgrades.md`](live_runtime_upgrades.md)
§9.2; `crates/irohad/src/replay_check.rs`, `iroha_core::sumeragi::node::replay_check`). The
Taira launcher `iroha3d_taira` runs it offline, as it runs `--check-config` and `--check-storage`,
without reading its runtime-signer descriptors.

- The store is opened read-only under its store-root lock, as `--check-storage` opens it, and is
  never written; nor are the snapshot and tiered-state directories. Signed genesis is
  authenticated against the configured trust anchor first.
- The whole store (blocks, native execution context records, lane stores and geometry journals;
  every file but the store-root lock) is copied into a temporary directory, and the original is
  released. The State is built on the copy as a Strict startup builds it: restored from the
  newest snapshot whose World the next stored block certifies (a snapshot at the tip has no
  successor yet, so an older generation is used), or empty to re-execute signed genesis when none
  restores. Blocks at or below `snapshot.upgrade_checkpoint` are never re-executed.
- Every later block's certificate is verified and the block re-executed through the startup
  executor. The first block whose execution result differs from its certificate ends the check
  with `outcome: "diverged"`, naming the first differing commitment component in report order
  (`validity`, `height`, `parent_world_state_root`, `transaction_inputs`,
  `transaction_results`, `transaction_outputs`, `events`, the witnessed roots,
  `world_state_root`, `native_lanes`, `schedule`, `beacon`, `finality_header`) with both values.
  A body the store no longer holds is `outcome: "missing_history"`, never a divergence, and a
  store without blocks is `outcome: "empty"`: nothing was replayed. After the
  last block the replayed World is checked against a cold capture
  (`world_state_accumulator`).
- The JSON report is the last line of standard output and the command exits nonzero unless
  it replayed blocks and every one matched.
- `--check-storage` reads the store in place and never restores a snapshot, which needs the
  certified chain above it; this command is the snapshot check. A release qualifies against a
  node's snapshot only when the report shows `start.source = "snapshot"` with
  `0 < start.height < tip_height`, `outcome = "matched"` and `replayed_to = tip_height`: a replay
  that fell back to signed genesis qualifies the history, not the snapshot.

## Adoption

Taira adopted the foundation at its upgrade checkpoint `H` with iroha a660d601da, whose one-time
import of the predecessor's (iroha f31b64d8c5) World is deleted, with the predecessor's root rule
(design §13). Node configuration alone authorized that import.

The checkpoint stays as the history boundary (`iroha_core::state::upgrade_checkpoint`).
`[snapshot.upgrade_checkpoint]` names `H` and its block hash, and Kura refuses a store whose block at
`H` has another hash. Blocks up to `H` are never re-executed or replayed: startup restores a
snapshot of this release above `H`, and `--check-replay` refuses to start below it.

### Taking over a Taira store

This release reads and replays a store that a660d601da wrote; no reset and no new checkpoint.
That was verified on a disposable a660d601da network (replays matched from signed genesis and
from snapshots; Strict start on stores holding a660d601da triggers and a
`ProposeRuntimeUpgradeProposal` block). Only Taira's own store establishes Taira's exposure:

1. Confirm the running build. Taira reports `git_commit_sha` `e7f7840ee2…`, which this repository
   does not hold. Its `wire_schema_hash`, which covers block, consensus and certified-result
   framing and the IVM ABI, and its feature set equal a660d601da's; instruction payloads and
   execution are not covered.
2. On a consistent copy of a stopped validator's store, run this release's `--check-config`,
   `--check-storage` (every retained block decodes, including rejected transactions), and
   `--check-replay`. The replay qualifies only from a snapshot above `H` (Operations). A Strict
   start on a copy without peers then shows that every stored State row restores, including
   trigger executables, pending multisig instructions, Kaigi relays, containment records and
   private-settlement locks.
3. Switch every validator at once. Unlike the rolling sequence of design §9, a660d601da peers fail
   this release's handshake (`ConsensusConfigCaps`) and cannot decode the appended trigger-filter
   encodings.
4. Then, through governance, activate `iroha.runtime.endorsed_change_set_proposal.v1` (its change
   set from `POST /v1/runtime/upgrades/draft`). Commit `runtime_provenance_policy_v1`, and propose
   a change set that restricts the baseline `ProposeRuntimeUpgradeProposal` interface
   `AdmissionClosed`; a restriction takes the disable notice, so it is its own change set. Only
   then does every scheduled change set carry endorsements. The baseline interface is deactivated,
   and its code deleted, once no selection needs it.

## Still open

Tracked in [`live_runtime_upgrades.md`](live_runtime_upgrades.md) §14 and `roadmap.md` N14:

- **Obligations:** qualify the selected native escrow and AMX captures and the selected contract
  kernel bridge; register in-flight game-unit exports with a captured closure;
  narrower authenticated dynamic-call manifests. Registered account/contract obligations now
  execute captured bindings, but replacing root-wide profiles still requires draining them.
- **Migrations:** storage capabilities and generic carriers.
- **Checkpoints:** qualify original-image replay across pre-acknowledgement crashes, interrupted
  staging and repeated startup, including typed resource retries and pending-spool crash recovery; starting a node from a
  transferred checkpoint without the block bodies below it (a body-independent certified prefix,
  sparse Kura history and peer transfer); the deletion guard and the retirement rehearsal.
- **Taira's anchor:** Taira's validators run a release with these records, a change set
  activates `iroha.runtime.lifecycle.v1` (its Ironwood with Time composition, after Taira's Time
  and Ironwood activation), each validator signs the record against its own store, and any
  account relays the statements. Taira has no proposer for a governed change set yet
  (`runtime/taira_time_ironwood_v1.md`, "Why a certificate"), so the activation waits for
  Parliament's integration or another owner decision.
- **Commitment and history:** complete-State commitments, which first need the node-configured
  policy cells (G1-D3, ZK delivery F.4) in committed State and a decision on how the pinned
  history meets the new root; deleting the startup re-validation of the signed genesis policy
  then.

Parliament lifecycle parameter updates are validated, but Core still needs the frozen
governance-clock policy and anonymous-ballot integration owned by
[`parliament_ballot_implementation_goals.md`](parliament_ballot_implementation_goals.md) E2.
Its current block-height/timed-OVN lifecycle is not a substitute for that integration.
The closure resolver still uses the baseline's ordinary graph allocations; complete funded graph
custody and trigger-fanout resource qualification remain part of the complete-State work. Component
tests alone do not qualify live deployment.
