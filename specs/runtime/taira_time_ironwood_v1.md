# One-time Taira Time and Ironwood activation

The network owner authorized this exception on 2026-11-08, in two answers relayed by the session
that asked them: a one-time Time and Ironwood activation on Taira, and these terms for it. It applies
only to chain `fc56984b-2be7-431d-840e-21514d1883f0` and its existing genesis-derived network
identity. It is not permission for a reset, a general Parliament, runtime-proposal, executor-upgrade
or role-management grant, a chain parameter change, or a second transition. Implementation is not
deployment qualification, and the timing of the validator roll and of the final signing need the
owner's separate go.

The retained authorization is `owner-authorization-time-ironwood-20261108.json`, SHA-256
`7cc548e3d095b55164866888464a5853ace65cabc6806e1dacbca74582800966`, amended by
`owner-authorization-time-ironwood-20261108-addendum-1.json`, SHA-256
`b6c42a283b1db297994aa1ddabd076aab45fdc8fec4c268dd281ad04f5fd4771`, which names the first digest.
The implementation pins the addendum digest and the exact Taira chain and network identities.

## Why a certificate

Taira selected the SoraFS repair (`specs/sorafs/taira_repair_v1.md`), so its selection `f4f73c12…`
and kernel differ from the baseline. No account holds `CanProposeRuntimeChangeSet`,
`CanManageParliament` or `CanManageRoles`, and the first two cannot be delegated under the Initial
executor, so the governed route has no proposer. The compiled Ironwood constructors also accept only
the baseline kernel.

## Certificate and boundary

`ScheduleTairaTimeIronwoodV1` has the new tolerant wire ID
`iroha.runtime.taira_time_ironwood.schedule.v1`. No selection binds it. Its certificate contains a
canonical `TairaActivationPlanV1` signed independently by every member of the exact four-seat
committee scheduled for the executing height. The plan binds the chain, network, operation identity,
complete change set, frozen notice, committee, expiry, owner authorization, release and source
closure digests, and the exact Ironwood pool. Swapped, repeated or missing signatures fail.

The certificate must be the sole instruction of a direct external signed transaction on the
universal root. A sealed reveal, contract call, batch or nested invocation cannot enter this boundary.
The boundary validates the deployment, roster and expiry, and computes the only allowed change set
from the release catalog. An invalid certificate stays the unbound envelope it is on a release that
kept this payload opaque, so old and new validators commit the same rejection. A valid certificate
requires all four validators to run this release first.

The certificate takes the place of Parliament's approval and of nothing else. Its handler calls
`change_set::enact`, the path Parliament enactment uses, and retains the complete certificate at
`runtime/taira_time_ironwood/certificate` as the once-only receipt. Normal signed-transaction
admission, resource metering and fees remain mandatory. Scheduling selects nothing.

## Activation

Every current validator then attests with the ordinary `AttestRuntimeSupportV1`. At the start of the
effective height `change_set::activate_due` activates the change set if all four attested, its notice
elapsed and it still applies; otherwise it lapses and never applies. A lapse consumes the receipt, so
another attempt needs an owner decision and a new release.

The change set moves the repaired selection to the compiled Time and Ironwood composition and keeps
the repair kernel. It uses the byte-preserving Ironwood identity cutover and rewrites no row. The spent
repair instruction is unbound and deactivated. Its services profile, implementation and storage set
are the compiled composition's, so the world schema equals the existing Ironwood family entry and the
SDK supported pair is unchanged. No new storage, definition kind or kernel is introduced.

`lifecycle_activation::committed_policy` now accepts the retained repair certificate as the provenance
of the one step that has no change-set record. Without this every lineage read on Taira refuses.

## Pool bootstrap

The Ironwood pool needs the successor selected, so it is a second transaction after activation: the
ordinary `iroha.privacy.bootstrap_ironwood_pool.v1` with the plan's exact values. While the certified
change set is activated and its successor selected, the retained certificate stands in for
`CanEnactGovernance` for exactly that pool. Every other check of the instruction stays, and a repeat
refuses as already initialized. Any other pool still needs the permission.

## Qualification before activation

- Read-only preflights against authenticated exports: `trigger_lifecycle_preflight` (live parameters
  report no output-support error; a complete trigger export is still needed for capture sizing) and
  the exact change-set construction against the authenticated selection.
- All-four native replay of retained histories on the release that will activate.
- Copied-State execution of the schedule, attestations, activation block and pool bootstrap, without
  consensus voting. Production validator keys must not sign a divergent rehearsal fork.
- The four-peer campaign `specs/ironwood_activation_runbook.md` requires.
- The decoder gate in `specs/ironwood_runtime_transition.md`, plus this wire ID: every block and
  snapshot consumer has the new decoders before activation, including BOI Core and the Kotlin and
  Swift SDKs. Base-tree observers stop at the effective height with `UnsupportedRuntime`.

Time and Ironwood go ahead of the SoraFS authority work (`specs/sorafs_authority_governance.md`),
which then needs a further composition, schema-family entry and reader rebuild.
