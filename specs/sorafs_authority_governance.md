# Parliament-voted SoraFS operating authorities

Status: implemented candidate, 2026-11-08, on `optimizations`, descended from
Taira's deployed `59db2e29177c68f59da81d2cc228e2eb7dcc882e`. The combined candidate still requires
native validation, copied-history replay and genuine governed activation; it is
not active on Taira. It refines the Claude design discussion
“Parliament-voted SoraFS authority grants on Taira”.

## Recommendation

Use addition-owned typed proposals with the existing Parliament attempt engine,
activated together with a new immutable execution services profile. Every grant
and every revocation requires its own proposal, vote and certified effect.
Keep `ProposalKind`, existing proposal records and existing wire layouts frozen.
Do not delegate these decisions to existing permission holders or the admission
council, and do not reset genesis.

The proposal-host approach is sound only with explicit selection of its new
hook and authorization semantics. Looking in an extra table under the unchanged
baseline services identity is insufficient: unchanged behavior before activation
does not establish an unchanged contract after activation. The existing
`ReleaseCatalog::lifecycle_successor` and `service_enabled` in
[`runtime/catalog.rs`](../crates/iroha_core/src/runtime/catalog.rs) demonstrate
selected services composition. Extend that mechanism; a general dynamic hook
plugin system is unnecessary.

## Findings that change the initial options

| Finding | Design consequence |
| --- | --- |
| Attempt creation, lifecycle status writes, certificate validation and due-height execution require `world.governance_proposals` today (`smartcontracts/isi/world.rs`). | Add a closed typed proposal resolver at those boundaries; a new propose instruction alone cannot work. |
| Ordinary holders can grant/revoke all three target permissions; holders can also delegate the empty compliance role (`executor_initial_permission_authority.rs`). | A certified grant alone does not enforce one vote per change. Select authorization rules covering every mutation path. |
| `CompleteReplicationOrder` checks both the scoped permission and the exact current owner/completion-signer policy (`smartcontracts/isi/sorafs.rs`). | Preserve both checks. Parliament approval does not replace provider-owner consent. |
| The suggested head records only `held = false/true`. | A grant/revoke cycle recreates the head and can revive a stale certificate. Use a monotonic revision and retain revoked heads. |
| `publish_addition` declares dependencies on every earlier addition. `GAME_PARTITIONS` explicitly awaits live-obligation support before activation. | Blindly appending the SoraFS addition makes unrelated game functionality a rollout prerequisite. Give this new addition explicit dependencies without rewriting existing descriptors. |

## Runtime and proposal ownership

1. Publish new tolerant instruction IDs for proposing an authority action and
   creating its Parliament attempt. Neither accepts an arbitrary instruction,
   permission token, role payload, privileged account or execution flag.
2. Install three new top-level World tables, initially empty:
   `world.sorafs_authority_proposals` for immutable content/proposer plus lifecycle
   status, and `world.sorafs_authority_heads` for per-subject revision, grant state
   and exact enactment receipt; `world.sorafs_authority_leases` retains the paired
   term approvals and their actual start and termination receipts. Register all
   three in the canonical State field,
   commitment, snapshot and restore inventories.
3. Keep all attempts in `world.parliament_attempts` and use
   `put_parliament_attempt`. Preserve its derived counts, member references,
   resource reservations, casting candidates, beacon slots, due index and key
   retention. Do not introduce a second attempt scheduler or voting engine.
4. Resolve proposal content to a closed internal type: baseline proposal or
   SoraFS authority proposal. Resolution supplies policy, canonical content/effect
   hashes, subject, manager rule, current head, effect and status write-back.
   Domain-separate new hashes; reject missing, mismatched or multiply resolved
   content rather than choosing whichever table is searched first. Preserve
   baseline manager, retry, eligibility, body and ballot rules. A proposer being
   a bonded citizen does not itself confer Parliament-manager authority.
5. Publish a successor services profile selecting the exact new Parliament hook
   and authority-mutation behavior. Publish replacement implementation descriptors
   wherever their services dependency changes. Rebind lifecycle handling to the
   new implementation while leaving its wire layout unchanged. Dispatch and
   node-side support checks must recognize the same exact compiled closures;
   table presence, module name or local configuration is not a feature switch.
6. Extend the canonical addition draft builder to emit one closed change set for
   these services, implementations, bindings and tables. Preserve any existing
   services and restrictions. The present implementation supports the baseline
   services profile, including the already selected SoraFS repair kernel, without
   rerunning repair or changing its kernel. Ironwood and lifecycle service
   compositions are refused until explicitly implemented and qualified. Retain old definitions for
   historical selections until the ordinary retirement floor permits deletion.

Use explicit dependencies for this new addition on the baseline and the actual
services/storage/interfaces it consumes. Preserve the IDs and dependency sets of
existing additions; do not reorder them or alter their published definitions.
The new declaration/draft path must not activate game partitions as a side effect.

## Exact action and head

The new typed proposal has a canonical target `AccountId`,
`Grant | AuthorizeTerminalRevoke | Revoke`, a term on the committed epoch grid,
`expected_revision`, and exactly one authority:

| Authority | Scope/effect |
| --- | --- |
| `CompleteReplicationOrder { provider_id }` | Exact provider-scoped `CanCompleteSorafsReplicationOrder` direct grant |
| `SetReservePolicy` | Direct `CanSetSorafsReservePolicy` grant |
| `UpsertProviderCredit` | Direct `CanUpsertSorafsProviderCredit` grant |
| `GatewayComplianceOperator` | Membership of the reserved empty `sorafs_gateway_compliance_operator` role |

Bind network/root, the typed authority/scope, target, action, expected revision
and any provider-policy precondition into the canonical signed proposal/effect
domain. The subject is `(authority including scope, target)`; action is not part
of the subject, so opposing changes contend. `expected_revision` is part of the
content identity: granting again after a revocation is a new decision, not reuse
of an already enacted proposal. Retries of the same decision retain its original
content and the existing attempt-sequence rules.

An absent subject begins at revision zero. Each successful effect increments
with checked arithmetic and retains a head even when revoked. The expected head
binds revision, grant state and relevant owner/policy context. Compare it again
at enactment. A stale certificate becomes superseded; a changed-and-restored
boolean does not make it current. Deterministic due-index order resolves two
certificates against the same revision: after one applies, the other is stale.

For completion grants, require the target to be the provider's currently named
completion signer, and bind the exact owner and signer-policy identity/revision
and digest. Recheck at enactment. Normal completion continues to require the
current permission **and** the exact owner-approved policy. Revocation must remain
possible after a provider is removed or its signer policy changes; do not apply
grant-eligibility preconditions to revocation. Owner rotation does not silently
create or transfer a Parliament grant.

The grant itself remains scoped to account and provider until its approved term
ends or a separate early-revocation decision enacts;
the captured owner/policy is an admission and enactment precondition, not its
lifetime identity. A later owner-approved policy can use the same approved signer,
including returning to it after a rotation. Binding the grant's entire lifetime
to one policy would instead require new policy-aware consumer checks and is not
part of this proposal.

## Paired term approvals

One term lasts 64 committed native epochs on the height-one anchored grid. Its
approval window opens one term before its start and closes one epoch before the
start. The proposal's epoch length must equal the committed NPoS cadence; callers
cannot supply arbitrary start, cutoff or end heights.

Grant and terminal revocation are separate immutable proposals and certificates
for one action-normalized plan. Both must enact before the cutoff. Neither
approval alone installs authority. At the exact term start the native owner
rechecks the pair, current head, account and completion-policy preconditions,
then installs the grant once and captures its completion implementation and
services profile. At the term end that retained terminal mandate removes the
authority. A separately voted early revocation may complete the same lease
earlier; completion cannot run twice. Each decision role shares a 17-unit fresh
randomness allowance across its term family, including retries.

The grant and terminal approvals, actual grant, and actual removal are distinct
records. Pending approvals close deterministically at their cutoff. A refusal at
the one permitted start does not reschedule or silently extend the term. Native
obligation checks retain the selected completion closure while a lease needs it.

## Nondelegation and atomic effects

Under the successor profile, the four authorities have one mutation owner: an
internal operation reached only after validation of the retained due certificate.
Keep its authorization context private and nonserializable. Do not use a
transaction-supplied bypass flag or a magic governance account. Native `Grant`
ignoring its authority is an implementation detail, not proof of authorization.

Reject ordinary grants/revocations of these direct tokens, including by existing
holders; embedding them in role registration or role-permission edits; granting
or revoking roles that carry them; and delegation of the reserved compliance role.
Also cover contract-emitted instructions, triggers, multisig execution and native
cross-instruction calls. Prevent ordinary precreation, mutation, removal or
recreation of the reserved role. Account deletion must not silently remove active
governed authorities; require their voted revocation first and retain subject
revision tombstones. Audit every cleanup/mutation owner, not only Torii ingress.

Use of the protected operating instructions is also nondelegable. Ordinary
`CanRegisterTrigger` or `CanExecuteTrigger` grants must not let a different caller
run `CompleteReplicationOrder`, `SetSorafsReservePolicy` or `UpsertProviderCredit`
as the holder. Under the selected services, reject those effects at the common
opaque-execution boundary before applying any instruction in the group. This
covers existing triggers, contract/IVM output, proved overlays and recursively
resolved deferred multisig proposals or approvals. Keep ordinary trigger
capabilities and predecessor execution unchanged; they confer no exemption.
Direct signed instructions and multisig instructions approved by signed quorum
transactions retain the normal holder authorization and both completion checks.

Gateway compliance HTTP operations and assigned provider source reads also require
the account's controller signature or full controller witness once these services
are selected. An otherwise valid request signed only by the account's registered
off-chain signing key is refused: installing a separately held key must not confer
the governed role's API powers or the exact completion signer's source access.
The HTTP handlers check the selected services alongside their existing current
role or native source-policy checks; source reads repeat the checks after I/O.
Unsupported services fail closed. This does not change predecessor HTTP behavior,
general account-signing-key semantics, or the two native completion checks. An
account with an active off-chain key must revoke it through the normal signed
transaction path before using controller-authenticated protected HTTP operations.

The first successful compliance grant registers exactly the empty role and its
target membership atomically. Later grants verify its exact reserved shape;
refuse a preexisting role with extra permissions. Revocation removes only the
membership and retains the empty role. Do not grant `CanManageRoles` or any
general delegation capability.

The authority head is canonical; existing direct permissions/role memberships
are its enforced execution projection. Apply projection, revision, receipt,
proposal/attempt status and events in the same State transaction. Restore must
check their agreement. An effect failure discards the whole transaction and
records the deterministic terminal failure in a fresh transaction, following
`State::apply_block_start_parliament_enactments`. Missing/changed ordinary target
state is a superseded or failed effect, not a malformed certificate that panics
the block hook. Storage/resource deferral retries block-start execution at the
same exact due height; it must not skip or reschedule enactment, or be misreported
as a rejected vote.

The initial Taira candidate requires an empty targeted-authority inventory and
an absent reserved role, checked completely at the actual activation cut with
bounded, funded native validation. Do not infer emptiness from genesis alone or
silently adopt existing authority as Parliament-voted.

Before activation, explicitly authorized existing holders may voluntarily
surrender valid grants through the currently selected ordinary `Revoke` policy.
Retain the exact native inventory, instruction scope and signed transaction and
certified-result evidence. This is ordinary holder surrender, not Parliament-voted
revocation, and does not itself require a new retirement host. Removing a role
membership leaves its protected role definition in the inventory; role-token
removal affects every member. An existing reserved role requires explicit lawful
unregistration, including existing `CanManageRoles` under the Initial executor.
Do not infer that authority from role possession or operator custody.

If existing lawful authority cannot clear the inventory, prepare a separate
governed reconciliation design. Never remove or grandfather rows automatically.
Surrender neither supplies missing runtime proposer/Parliament manager authority
nor replaces governed upgrade approval. Both the provider-scoped completion
permission and the provider owner's exact completion-signer policy remain required.
After activation, protected authority changes use the selected Parliament host.

## Completion, retirement and evidence

New proposal admission may be closed without stranding an existing attempt.
Admission closure, completion-only mode and retirement must preserve the exact
host/storage/service closure until all hosted attempts are terminal. Applied
grants and their revocation path also remain obligations while authority exists.
Integrate those checks with the runtime obligation owner; an empty due index
alone does not prove a host can be retired. Queries must retain exact proposal,
attempt, certificate, head revision and execution outcome for recovery.

Provide typed draft and exact read surfaces for the new proposal host. Existing
APIs that promise baseline `ProposalKind` records retain that contract. A generic
status query must resolve the selected host or explicitly reject unsupported
content; it must not falsely report an admitted hosted proposal as absent.

## Implementation and acceptance

The daemon's offline inspection runs within `--check-replay`, while the actual
matched State and its copied Kura remain alive:

```sh
iroha3d --config /absolute/offline/config.toml --check-replay \
  --runtime-addition-review-request /absolute/review/request.json \
  --runtime-addition-review-output /absolute/review/new-package
```

The closed request uses schema `iroha.sorafs.authority.inspect-request.v1`, mode
`inspect` and module `iroha.sorafs.authority_governance.v1`. Its `expected_cut`
contains native JSON values for `network_id`, `genesis_hash`, `height`, `tip_hash`
and `selection_id`. These independently selected values must equal the native
observation; they never construct State or authenticate it. Unknown fields and
draft/qualification inputs are refused.

One retained State view supplies the selection, complete published definitions,
exact committed upgrade-policy parameter and decode result, change-set records,
canonical tip time and full authority-inventory result. A failed inventory remains
visible without changing the replay verdict. The package records whether replay
started from genesis or a snapshot, and includes canonical selection bytes with
a native roundtrip check. It produces no change set, certificate or transaction.

The same view also exports the exact stored direct grants, role definitions and
memberships for the protected SoraFS permissions, the reserved compliance role,
and `CanManageParliament`, `CanProposeRuntimeChangeSet`, `CanSetParameters` and
`CanManageRoles`, including role-manager-only roles needed for explicit cleanup.
Scan counts and executor kind/semantic identity accompany these records. Native
unit-token comparisons distinguish exact administrative tokens from malformed
payloads with the same name. This describes stored possession and its origin;
it does not evaluate an arbitrary user-provided executor's authorization policy.
The local 16,384-row ceiling refuses an oversized inspection rather than marking
a partial scan complete.

The request is limited to 64 KiB and the package to 64 MiB. Output must be a new
private directory outside node stores and the configuration directory; symlinks
and replacement are refused. `manifest.json` is published last as the completeness
marker. A partial directory without it is not a successful package. This unsigned
historical inspection needs retained original replay inputs and build provenance;
it is neither a State proof nor permission to activate the addition.

The implementation covers selected proposal-host services and dependencies,
authority proposals/effects and mutation controls, and native client drafts/reads.
Four-validator qualification remains outstanding. Ship activation only when the
combined candidate meets all of these checks:

- Frozen baseline catalog/layout fixtures and old-selection replay remain exact.
  Both new wire IDs reject before activation, including malformed payloads;
  uninstalled storage is absent from roots and snapshots. The actual draft works
  without installing game partitions and preserves unrelated services/restrictions.
- Existing Parliament reducer, due-effect, failure-rollback and fee-policy tests
  pass unchanged. Hosted attempts use all shared accounting and restore indexes.
  Baseline and hosted proposals cannot cross-bind or collide silently.
- Grant and revoke each of the four authorities to an exact account. Check other
  accounts/providers are unchanged, first/subsequent role grants, role-shape
  refusal, overflow and atomic failure. Exercise the real initial executor for
  every delegation, role, account-cleanup and nested-instruction negative above.
- Reject stale certificates after grant/revoke/regrant; resolve concurrent
  certificates deterministically. Preserve the existing completion-authority
  tests and cover signer/owner/policy changes, revocation before completion and
  revocation after provider removal. Retry does not create a second effect.
- Test snapshot/replay/restart before and after activation and enactment,
  terminal-failure recovery, resource deferral, partial rollout support refusal,
  closure restrictions and attempted retirement with pending votes/active grants.
- Run focused `cargo test -p iroha_core --lib` modules, affected model/Torii/client
  suites, strict changed-crate clippy with `--no-deps`, formatting and the
  serialization guard. Seeded certificates are component tests; also complete a
  genuine proposal → vote → certificate → exact due-height effect → restart
  journey on four validators, including grant and revocation evidence.

Taira binary rollout and governance activation are distinct steps. Use the
existing runtime change-set workflow, committed notice/provenance policy and
every current validator's authenticated support attestation. Preserve chain
history. Recover the exact signed transactions after uncertain submission;
verify state-resolved `Applied`, certificate enactment, exact authority readback
and restart persistence. Permission to deploy updates does not replace those
protocol decisions or identify the accounts Parliament should approve.

TODO: qualify the combined exact-source candidate and establish the current
Parliament prerequisites from actual committed State. The selected V1 runtime
already constructs certificates from verified timed-private ballot results and
enacts runtime-upgrade proposals through its retained exact-due certificate path.
The unfinished anonymous-ballot replacement and its absent qualification record
do not, by themselves, disable this existing mechanism. Candidate seals,
component-test certificates, release signatures and validator support attestations
are not Parliament votes.

The current public Parameters observation has no `runtime_upgrade_policy_v1`,
so native proposal admission refuses a change set. Its initialization requires
the current exact `CanSetParameters` permission and a canonical reviewed policy;
missing State never substitutes compiled defaults. Actual proposer/manager
permissions, eligible rosters, ready native cadence, verified beacon/TLE custody
and genuine ballots must also be established before using the existing flow.

Authenticated observations on 2026-11-08 found the release and upgrade routes
working on all five Taira ingress roots at height 12438, with deployed source
`59db2e2917`. Anonymous operator reads correctly return
`401 operator_signature_missing`. The earlier release-route 404 is resolved.
These observations establish neither a complete authority inventory nor governed
activation. This candidate has submitted no live grant or activation transaction.
