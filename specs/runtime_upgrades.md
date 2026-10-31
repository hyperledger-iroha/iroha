# Runtime change sets — as built

Status: as built, 2026-10-31. [`live_runtime_upgrades.md`](live_runtime_upgrades.md) is the
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

1. **Propose.** `ProposeRuntimeUpgradeProposal { action: RuntimeUpgradeActionV1, provenance }`
   submits `Schedule(change_set)` or `Cancel(id)` to SORA Parliament. It needs the unit permission
   `CanProposeRuntimeChangeSet`. Admission reads the committed policy
   `runtime_upgrade_policy_v1` (`RuntimeUpgradePolicyV1`); a root without it admits no change set.
   `provenance` holds the release signers' endorsements the committed provenance policy requires
   (below).
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
  until it is declared.
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

TODO(§5.5): snapshot writers omit, and readers default, fields their selection does not install.
The first release that adds a World field needs this so that a previous release can still read
its snapshots before activation; this release adds none, so its strict reader is unchanged.

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
  `world.runtime_containments` records each restriction it replaced (`Contained`).
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

CLI: `iroha runtime contain list` reads the records; `iroha runtime contain endorse --file
<containment.json> --member-key-file <path>
[--at-ms <ms>]` prints one member's `EndorsementV1`; `iroha runtime contain submit --file
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
- **Activation.** A change set whose successor changes an original binding of a live obligation's
  completions or of their transitive declared calls (rebinding, unbinding or newly binding it), or
  stops executing a required definition the predecessor executes (its storage, the kernel, the
  commitment or services profile, and for escrows the host profiles, since controlled locks invoke
  contracts), lapses at its effective height. AMX completions are native and retain no host
  profile. A binding the predecessor already leaves out, or a definition it already disables, is
  no part of the executable closure: keeping it so blocks no change set, and lifting such a
  restriction restores the same code. The baseline bundle conservatively declares all its native
  callees. Keeping a replacement active does not transfer the original obligation to it. Until
  captured completion capabilities exist, rebinding those dependencies requires draining the
  obligations first: closing admission to new escrows, `Begin` and `Prepare`, then completing
  what is live.
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

## Deferred consent

`iroha_core::runtime::expectation` captures the resolved bindings and their transitive immutable
definitions, including the kernel, commitment, services and host profiles. Cyclic calls are visited
once. A domain-separated canonical digest of this closure and the exact installed executor code
is committed with each deferred action; loaded runtime caches do not enter it:

- A multisig proposal occurrence owns its digest under `multisig/proposal-expectation/`. Approval
  checks it before recording a vote or executing. Changed semantics require a fresh proposal nonce
  and new approvals; the old occurrence can still be cancelled or expire. Pruning removes both
  records. Raw contract storage cannot access the native `multisig/` namespace.
- Trigger registration commits the digest in reserved `__runtime_expectation_v1` metadata. A
  caller cannot supply, replace or remove it. Data callbacks with changed or restricted semantics
  skip the matched event without rejecting its originating transaction or consuming a repetition.
  By-call execution rejects before effects; Time and Pipeline execution keep their existing
  persistent retry, exhaustion and quarantine policies. Restoring semantics never replays an event
  skipped during suspension.
- Restrictions are checked independently of the digest. Local codec/resource refusals retain the
  execution deferral; they are not a reason to skip a callback or commit a rejection.

The baseline native implementation declares a conservative superset of callees, so one native
capture can include the entire baseline. Opaque IVM, contract-call and custom-instruction captures
cover the complete callable binding set; expanding that surface requires renewed consent too.
Explicitly following current bindings and narrower authenticated dynamic-call manifests remain
unimplemented.

Migration cursors and contract-created obligations still need kernel registration. Escrows and AMX
participations do not yet retain a captured completion capability that dispatch honors after a
rebinding. These protections do not close the whole obligation lifecycle of design §7.2.

Persisted pending proposals and triggers require their original capture. There is no restore-time
fallback that manufactures consent from the current selection. A predecessor snapshot containing
uncaptured deferred work cannot be imported unchanged: adoption must explicitly certify its
preserved context or first drain/cancel that work. This is a populated-State acceptance change even
though no World table was added, and is not an encoding-neutral live deployment.

## Unsupported transition effects

The shared successor resolver rejects storage retirement, activation of migration definitions and
manual retention/release of completion capabilities: the cursor, write-barrier, cutover and kernel
capability engines are not implemented. Proposal/enactment and node support attestation use this
resolver; block start also checks it and lapses an already scheduled unsupported transition
without changing the selection. The rule depends only on the change set, so every node rejects the
same transitions. Installing tables and cells is supported (release additions, above); an
executing release still has to implement the resulting selection. Recognizing every definition in
a successor is not evidence that a release can perform a storage transition.

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
committed, a proposal that schedules a change set needs valid endorsements of that identity from
`threshold` distinct trusted signers, and carries at most one endorsement per trusted signer;
endorsements of another change set, from other keys or with bad signatures do not count. A
cancellation, or a proposal under a root without the policy, carries no endorsements. Admission
checks them when the proposal executes; they are not stored with the proposal, whose transaction
keeps them in the block history.

## Torii

| Route | Result |
| --- | --- |
| `GET /v1/runtime/abi/active` | `{ abi_version }` |
| `GET /v1/runtime/abi/hash` | `{ policy: "V1", abi_hash_hex }` |
| `GET /v1/runtime/metrics` | ABI version and change sets by status (`scheduled`, `activated`, `lapsed`, `canceled`) |
| `GET /v1/runtime/upgrades` | Committed selection and its identity, committee keys and digest, every change-set record |
| `POST /v1/runtime/upgrades/propose` | Body: `{ change_set, provenance }`, the `ChangeSetV1` and its `ChangeSetEndorsementV1`s (empty without a provenance policy). Drafts the `Schedule` proposal for local signing |
| `POST /v1/runtime/upgrades/cancel/{id}` | Drafts the `Cancel` proposal for local signing |
| `POST /v1/runtime/upgrades/attest/{id}` | The node signs its support with its validator key and drafts the `AttestRuntimeSupportV1` instruction; refused unless its release executes the successor and its key is a committee member |
| `GET /v1/runtime/release` | Build, release digest, and whether the release executes the committed selection and each scheduled successor, naming the first definition it lacks |
| `GET /v1/runtime/containments` | Every containment record by authorization height |

The list, propose and cancel routes back the `iroha.runtime.upgrades.*` MCP tools, beside the
`iroha.runtime.abi.*` and `iroha.runtime.metrics` tools.

## CLI

- `iroha runtime abi active|active-query|hash`, `iroha runtime status`, `iroha runtime capabilities`
- `iroha runtime upgrade list`
- `iroha runtime upgrade endorse --file <change_set.json> --signer-key-file <path>` prints a
  release signer's `ChangeSetEndorsementV1`; the key is read at run time and never printed.
- `iroha runtime upgrade propose --file <change_set.json> [--endorsement <file>...]` submits the
  `Schedule` proposal with the endorsements.
- `iroha runtime upgrade cancel --id <hex>` submits the `Cancel` proposal.
- `iroha runtime upgrade attest --id <hex>` has the node sign its support and submits the
  drafted `AttestRuntimeSupportV1`; the operator never handles the validator key.
- `iroha runtime release` shows the node's release support.

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
§9.2; `crates/irohad/src/replay_check.rs`, `iroha_core::sumeragi::node::replay_check`).

- The store is opened read-only under its store-root lock, as `--check-storage` opens it, and is
  never written; nor are the snapshot and tiered-state directories. Signed genesis is
  authenticated against the configured trust anchor first.
- The whole store (blocks, native execution context records, lane stores and geometry journals;
  every file but the store-root lock) is copied into a temporary directory, and the original is
  released. The State is built on the copy as a Strict startup builds it: restored from the
  newest authenticated snapshot, or empty to re-execute signed genesis. Blocks at or below
  `snapshot.upgrade_checkpoint` are never re-executed.
- Every later block's certificate is verified and the block re-executed through the startup
  executor. The first block whose execution result differs from its certificate ends the check
  with `outcome: "diverged"`, naming the first differing commitment component in report order
  (`validity`, `height`, `parent_world_state_root`, `transaction_inputs`,
  `transaction_results`, `transaction_outputs`, `events`, the witnessed roots,
  `world_state_root`, `native_lanes`, `schedule`, `beacon`, `finality_header`) with both values.
  A body the store no longer holds is `outcome: "missing_history"`, never a divergence. After the
  last block the replayed World is checked against a cold capture
  (`world_state_accumulator`).
- The JSON report is the last line of standard output and the command exits nonzero unless
  every replayed block matched.

## Still open

Tracked in [`live_runtime_upgrades.md`](live_runtime_upgrades.md) §14 and `roadmap.md` N14:

- **Before the first World-field addition:** snapshots that follow the selection (a writer omits
  and a reader defaults every field the snapshot's selection does not install), so a previous
  release reads this release's snapshots until activation; a catalog test refuses such an
  addition until then.
- **Operations:** the operator route that drafts an addition's activation from the node's catalog
  (`iroha_core::runtime::draft_addition` is implemented; its Torii route, client and CLI command
  are not), and the Taira launcher treating `--check-replay` as an offline command.
- **Obligations:** kernel registration of contract-created obligations and migration cursors,
  completion capabilities retained across rebinding, explicit current-binding consent and
  narrower authenticated dynamic-call manifests.
- **Transitions:** migrations with cursors, write barriers and atomic cutover, storage retirement,
  storage capabilities and generic carriers.
- **Commitment and history:** complete-State commitments, which first need the node-configured
  policy cells (G1-D3, ZK delivery F.4) in committed State and a decision on how the pinned
  history meets the new root; checkpoints and the startup floor.
- **Adoption:** the governed adoption carrier that replaces the interim configuration import
  (`state::upgrade_checkpoint`): a predecessor-recorded authorization naming this baseline and
  the adoption height, verified at import, with the adoption record as trust anchor.

Parliament lifecycle parameter updates are validated, but Core still needs the frozen
governance-clock policy and anonymous-ballot integration owned by
[`parliament_ballot_implementation_goals.md`](parliament_ballot_implementation_goals.md) E2.
Its current block-height/timed-OVN lifecycle is not a substitute for that integration.
The closure resolver still uses the baseline's ordinary graph allocations; complete funded graph
custody and trigger-fanout resource qualification remain part of the complete-State work. Component
tests alone do not qualify live deployment.
