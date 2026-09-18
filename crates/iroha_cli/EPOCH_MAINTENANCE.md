# Native epoch maintenance contract

`taira epoch-maintenance` owns a validator-operator lifecycle operation. Its
configured ledger owner must already hold `CanSetParameters`; the separately
configured HTTP operator key does not grant ledger administration. Application
onboarding and dataspace deployment never receive validator mint seeds.

The public schedule is native Kagami output: `schema_version: 1`, `network_id`,
`parameters` (one to 256 contiguous positive-epoch typed next-roster Parameters),
`payment_asset`, and `transaction_fee_maximum`. The schedule must bind the exact
four peers in the independently selected deployment trust profile. Fresh peer
registration and core-lane staking observations conservatively reject extra,
missing, substituted or ineligible members. These observations are not a proof
of a future election: native boundary validation remains authoritative.

All five subcommands require explicit global `--fee-payer authority` and take `--trust`, `--schedule`, `--journal-dir`, finite
`--timeout-ms`, and `--operation-timeout-ms` (default 180000). `preflight`,
`prepare`, `apply`, and `status` take `--target-epoch`; `maintain` takes
`--stop-after-epoch`. The journal parent must already be an owner-private
existing directory. No command accepts a seed, private mint key, or arbitrary
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

Each `epoch-<NetworkId>-<target>` journal contains `plan.json`,
`prepared.json`, `submitted.json`, and `submission-result.json`. Prepared wire
and hash remain immutable. An extended public schedule can reuse an existing
operation only when its exact target parameter, network, owner, trust and fee
policy agree; the initial full schedule digest remains provenance.

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
