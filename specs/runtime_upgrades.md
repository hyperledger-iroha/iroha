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

A live asset escrow or lock (status `Open`, `Accepted`, `PaymentSent`, `Disputed` or `Locked`) is
an obligation of the native escrow implementation (design §7.2, `iroha_core::runtime::obligations`);
its committed `world.asset_escrows` record is its registration. A restriction never removes its
settlement authority:

- **Dispatch.** While a definition behind an escrow instruction has admission closed, the
  instructions that continue or complete a live escrow still run: accepting its offer continues
  it, and marking payment sent, releasing, cancelling, opening or resolving a dispute, attesting a
  condition, drawing down and expiring (with the controlled-lock forms) complete it. While the
  definition is completion-only, only the completions run. Opening new custody runs under
  neither, and disabling stops every instruction.
- **Activation.** A change set whose successor leaves a completion of a live escrow unbound, or
  changes its original binding, transitive declared calls, storage, kernel, commitment, services
  or host profiles, or disables any required definition, lapses at its effective height. The
  baseline bundle conservatively declares all its native callees. Keeping a replacement active
  does not transfer the original custody obligation to it. Until captured completion capabilities
  exist, rebinding those dependencies requires draining custody first.
- **Executor replacement.** `Executor::migrate` refuses while any native escrow or lock remains
  live, before loading code or running migration effects. This includes identical-code migrations,
  which can still change permissions or State. Replacing the authorization component requires
  drained custody until captured completion authority or a certified transfer is implemented.

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

Migration cursors, AMX participations and contract-created obligations still need kernel
registration. Escrows do not yet retain a captured completion capability that dispatch honors after
a rebinding. These protections do not close the whole obligation lifecycle of design §7.2.

Persisted pending proposals and triggers require their original capture. There is no restore-time
fallback that manufactures consent from the current selection. A predecessor snapshot containing
uncaptured deferred work cannot be imported unchanged: adoption must explicitly certify its
preserved context or first drain/cancel that work. This is a populated-State acceptance change even
though no World table was added, and is not an encoding-neutral live deployment.

## Unsupported transition effects

The shared successor resolver rejects storage retirement, installation of storage cells,
activation of migration definitions and manual retention/release of completion capabilities: the
cursor, write-barrier, initialization, cutover and kernel capability engines are not implemented.
Proposal/enactment and node support attestation use this resolver; block start
also checks it and lapses an already scheduled unsupported transition without changing the
selection. Empty-table additions remain admissible; an executing release still has to implement
the resulting selection. Recognizing every definition in a successor is not evidence that a
release can perform a storage transition.

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

## Still open

Tracked in [`live_runtime_upgrades.md`](live_runtime_upgrades.md) §14 and `roadmap.md` N14:
kernel obligation registration and completion capabilities, migrations and storage capabilities,
complete-State commitments, checkpoints and the startup floor, and governed adoption on Taira.
Parliament lifecycle parameter updates are validated, but Core still needs the frozen
governance-clock policy and anonymous-ballot integration owned by
[`parliament_ballot_implementation_goals.md`](parliament_ballot_implementation_goals.md) E2.
Its current block-height/timed-OVN lifecycle is not a substitute for that integration.
The closure resolver still uses the baseline's ordinary graph allocations; complete funded graph
custody and trigger-fanout resource qualification remain part of the complete-State work. Component
tests alone do not qualify live deployment.
