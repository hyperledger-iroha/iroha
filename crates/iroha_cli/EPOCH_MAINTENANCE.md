# Native epoch maintenance contract

`taira epoch-maintenance` owns a validator-operator lifecycle operation. Its
configured ledger owner must already hold `CanSetParameters`; the separately
configured HTTP operator key does not grant ledger administration. Application
onboarding and dataspace deployment never receive validator mint seeds.

The public schedule is native Kagami output: `schema_version: 1`, `network_id`,
`genesis_roster` (epoch-zero public keys derived from the same private input),
`parameters` (one to 256 contiguous positive-epoch typed next-roster Parameters),
`payment_asset`, and `transaction_fee_maximum`. The genesis roster must equal the authenticated signed-genesis mint roster.
The schedule must bind the exact
four peers in the independently selected deployment trust profile. Fresh peer
registration and core-lane staking observations conservatively reject extra,
missing, substituted or ineligible members. These observations are not a proof
of a future election: native boundary validation remains authoritative.

The five seed-free transaction subcommands require explicit global `--fee-payer authority` and take `--trust`, `--schedule`, `--journal-dir`, finite
`--timeout-ms`, and `--operation-timeout-ms` (default 180000). `preflight`,
`prepare`, `apply`, and `status` take `--target-epoch`; `maintain` takes
`--stop-after-epoch`. The journal parent must already be an owner-private
existing directory. These five commands accept no seed, private mint key, or arbitrary
instruction payload.

Preflight reports the authenticated current epoch, boundary and observed
next-roster state. Prepare signs one `SetParameter` through normal
QueuePlanSynced admission and retains it without dispatch. Apply writes the
exclusive durable dispatch claim before its sole POST. The three real carrier
steps must fit before the original boundary: for boundary B, first dispatch
requires an authenticated parent at most B-4. Fee observation and a fresh
pre-dispatch checkpoint both preserve that budget. Quoting, signing, submission
and finality reconciliation retain the original per-operation deadline.

Maintain stages target E+1 only after authenticating actual current epoch E.
A boundary context with `next_epoch_snapshot` still has its old epoch and does
not authorize overwriting the next-roster parameter. Existing committed work
provides the heights; the command creates no empty blocks or synthetic work.
It performs at most one genuine maintenance transaction per scheduled epoch.

Each `epoch-<NetworkId>-<target>` journal contains immutable original `trust.json`, `plan.json`,
`prepared.json`, `submitted.json`, and `submission-result.json`. Prepared wire
and hash remain immutable. An extended public schedule can reuse an existing
operation only when its exact target parameter, network, owner, original trust and fee
policy agree; the initial full schedule digest remains provenance. On resume,
`plan.trust_sha256` still binds retained original `trust.json`. The explicitly
selected current `--trust` authenticates new observations. Native genesis
validation applies to both profiles, and genesis wire/key, ordered peer IDs,
origins and node fingerprints must remain exact; only independently selected
build/config fingerprints may change. A missing original trust record fails;
there is no legacy journal fallback or automatic fingerprint discovery.

Completion requires four exact Applied observations and native canonical
executed carriers bound to the independently authenticated finality prefix.
The actual carrier must precede the stored boundary and belong to the original
predecessor epoch. Apply/maintain then retain `carrier.nrt` and
`completion.json`, and emit the public completion receipt. Neither an accepted
POST nor a saved completion file substitutes for those checks.

Status is strictly read-only and emits either typed `state: pending` progress
or a freshly verified completion receipt. It can reconcile an old retained
hash after its write deadline, without creating carrier or completion files.
Maintain rechecks claims that existed at invocation start through this same
read-only path, even after same-epoch cancellation. Fresh dispatches retain
their original completion deadline. No command renews a write deadline, signs
a replacement, or reposts.

The finite whole-monitor budget may cover multiple unchanged per-operation
budgets and deliberate validator restarts. Only native transport interruptions
are pending; fixed identity, permission, codec, roster and proof failures remain
errors. Killing an owned monitor after its workload is safe because it holds no
validator seed and dispatch custody is durable. Reconcile every submitted claim
with status before claiming the monitored workload complete.


The Linux operator worker `taira epoch-maintenance supervise` renews these native
public schedules. It takes `--policy`, `--trust`, `--custody`, `--journal-dir`, and
a finite `--timeout-ms` (default 86400000). Deployment must select the public
policy through authenticated release admission and configure OS supervision;
this command does not install a service or admit a release signature itself.
The explicit ongoing authorization covers only this fixed network/administrator,
fixed-four roster, epoch parameter scope and fee caps until the owner stops the
service; it is independent of a reset transaction lease. It has no default.
The current native worker source commit and executable SHA256, Kagami executable
SHA256, and current trust SHA256 are explicit policy pins. Kagami executes from
its held read-only descriptor; Darwin does not support this production execution
path. Mac finite-maintainer tests do not qualify Linux worker renewal/restart.

The closed `schema_version: 1` policy contains `intent` (`authorization: "until_stopped"`, `network_id`,
`administrator`, `payment_asset`, `transaction_fee_maximum`, `first_epoch`,
`batch_epochs` in 2..=256, and `operation_timeout_ms`), `release_source_commit`,
`iroha_sha256`, `kagami` (`path`, `sha256`), `observation_trust_sha256`, and
`provision_timeout_ms`. Administrator identity must match the normal CLI config
and its signing key, have an explicit signed-genesis `CanSetParameters` grant,
and retain the live permission. The HTTP operator key is separate. The worker
never publishes the administrator config or a digest of private config/seed bytes.

The owner-private custody file contains `schema_version: 1` and `seeds`, exactly
four entries sorted by `validator` PeerId, each containing that ID and the
absolute canonical `path` to its original 32-byte seed. Files must have mode0600,
one link and stable owner/inode metadata. A zeroizing fixed-size buffer transfers
exactly128 bytes with EOF into an owned inherited pipe. Each invocation rederives
its retained public schedule, and native epoch-zero public keys must exactly
match signed genesis before any future parameter submission. No seed reaches argv,
environment, public receipts, or application deployment.

One held worker lock owns `epoch-worker-<NetworkId>`. The stable intent and
original trust remain immutable across admitted release changes; release policy
records and public schedules are immutable. After native maintain freshly proves
completion E, the next batch is [E..E+B-1]. The one-epoch overlap reconciles E
before maintenance waits for the actual next epoch; no future snapshot or cursor
can authorize early replacement. The durable cursor is only a restart hint and
its retained completion is reauthenticated before it advances work. Original
per-epoch journals retain their deadline, signed wire, exclusive dispatch claim,
and once-only POST across worker cancellation and restart. All provisioning
children and the whole worker invocation are finite. OS restart never renews a
retained transaction deadline or creates synthetic blocks.

First-target readiness is separate from full batch completion. Only a freshly
verified completion for the currently authenticated next epoch can publish
`ready-<policy-sha256>-<boot-id>-<pid>-<start-ticks>.json`; reconciling an older
overlap is insufficient. The receipt contains `schema_version: 1`, the exact raw
public policy file SHA256, `worker` (`boot_id`, `pid`, `start_time_ticks`), and the
native completion receipt. Service admission must match that process identity
against its current manager-owned worker. A prior process's file, an active
service, or batch cursor alone cannot establish new-service readiness.


Service readiness is verified through the seed-free read-only native command
`taira epoch-maintenance supervisor-status --policy POLICY --trust TRUST
--journal-dir JOURNALS --boot-id BOOT --pid PID --start-time-ticks TICKS
--timeout-ms MS`, with the same normal administrator/HTTP client inputs and
explicit authority fee selector. The manager supplies the expected process
incarnation. The command verifies kernel liveness, exact retained policy/ready
schedule, the original readiness transaction, and the presently authenticated
next-epoch transaction through the native status/finality path. It emits one
closed JSON report (`schema_version`, `policy_sha256`, `worker`,
`initial_completion`, `current_completion`). It acquires no worker lock and never
prepares a plan, repairs a journal, signs a transaction, or submits a POST.

Existing CLI exit classes remain: command failure (including the finite worker
budget)1, config3, input4, internal7, success/help0; OS stop uses signal semantics.
A nonzero command exit does not authorize replacement or reposting. The service
owner must configure bounded restart/backoff and stop/quiescence behavior
explicitly. The owned Linux provisioner also receives a parent-death kill signal
and has finite execution/output bounds.
