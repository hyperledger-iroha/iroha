# Parliament-voted SoraFS operating authorities

Status: proposed design, 2026-11-06. Reviewed against `optimizations` at
`11470b6a77`. This is an implementation plan, not an implemented addition or a
Taira qualification result. It refines the Claude design discussion
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
2. Install two new top-level World tables, initially empty:
   `world.sorafs_authority_proposals` for immutable content/proposer plus lifecycle
   status, and `world.sorafs_authority_heads` for per-subject revision, grant state
   and exact enactment receipt. Register both in the canonical State field,
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
   Ironwood/lifecycle services and restrictions. Compile and test each supported
   composition; reject unsupported combinations. Retain old definitions for
   historical selections until the ordinary retirement floor permits deletion.

Use explicit dependencies for this new addition on the baseline and the actual
services/storage/interfaces it consumes. Preserve the IDs and dependency sets of
existing additions; do not reorder them or alter their published definitions.
The new declaration/draft path must not activate game partitions as a side effect.

## Exact action and head

The new typed proposal has a canonical target `AccountId`, `Grant | Revoke`,
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

The grant itself remains scoped to account and provider until voted revocation;
the captured owner/policy is an admission and enactment precondition, not its
lifetime identity. A later owner-approved policy can use the same approved signer,
including returning to it after a rotation. Binding the grant's entire lifetime
to one policy would instead require new policy-aware consumer checks and is not
part of this proposal.

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

Activation needs an explicit deterministic reconciliation rule for existing
direct grants, inherited grants and reserved-role contents. The initial Taira
candidate should require an empty targeted-authority inventory and an absent
reserved role, checked at the activation cut with bounded, funded validation.
Do not infer emptiness from genesis alone or silently adopt existing authority
as Parliament-voted. If that precondition is false, prepare a separately governed
reconciliation design; do not delete or grandfather the grants automatically.

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

Implement in three reviewable changes: selected proposal-host services and
dependency declaration; the authority proposal/effect plus mutation controls;
then native client drafts/reads, documentation and four-validator qualification.
Ship activation only when the combined candidate meets all of these checks:

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

TODO: implement and qualify this design. Read-only public probes during this
review returned HTTP 404 for `/v1/runtime/release` and HTTP 401 for
`/v1/runtime/upgrades`. They do not establish the deployed release, selection,
authority inventory or an outage. No deployment or live grant was performed.
