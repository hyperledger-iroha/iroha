# Native contract deployment service

`iroha_contract_deploy` owns deployment orchestration shared by Musubi and native
Rust consumers. The `iroha` SDK owns transport, fee quoting, signing, and canonical
transaction waits; this crate owns immutable artifact verification, the native
upload/manifest/atomic-commit sequence, and filesystem journals.

`DeploymentService` receives one runtime `Config`. `prepare` takes complete
artifact bytes, a typed alias, and an explicit fee intent; it verifies account
existence, exact effective registrar and alias grants, deployment CAS state, and
every quoted native transaction. `persist` records the exact signed plan without
submission. `execute` persists before dispatch; `resume` authenticates that plan
and polls every attempted hash without signing a replacement or replaying it.
Bare governance identities are rejected because they are not approval evidence.

`execute` and `resume` receive one explicit typed progress observer. It sees the
authenticated plan before dispatch, durable submitting/recovering stages by exact
hash, Applied evidence only after it is persisted, and the start of final
readback. Progress is observational and never changes signing or recovery policy.
Musubi shows the complete network, signer, alias, and fee review plus stage progress
on human stderr; machine mode retains one final JSON document and no progress output.

Journal directories must be owner-only (0700); records are bounded regular
single-link files (0600), opened relative to the locked directory without
following symlinks. Both file and directory are synced before dispatch. A crash
between recording an attempt and sending it deliberately remains unresolved;
resume must not guess whether a request reached Torii. Partial records fail
closed and are never silently overwritten. Unix filesystems are qualified;
other platforms return an explicit unsupported-filesystem error before dispatch.

`inspect_journal` returns a typed `Pending`, `Failed`, `Completed`, or `Cancelled` disposition.
A fixed failure contains the SDK's exact global, state-resolved rejection or
expiry response. Inspection rechecks the exact hash and persists that proof.
An explicit new deployment requires confirmed failure, completion, or cancellation
of a fully unattempted local plan.
Transport uncertainty never becomes a fixed failure. Resuming a completed
historical journal verifies its commit without requiring the alias to remain
at its historical address.

Read-only historical validation binds every envelope and manifest provenance to
the retained deployment authority. A current reader with a different signing
account may inspect the same network, chain, and address profile. Persisting,
executing, or resuming still requires the original deployment authority and key.

`cancel` durably abandons only a fully unattempted local plan. It holds the
exclusive journal lock, rejects any execution or unknown evidence, and retains
the exact ordered hashes in an immutable cancellation record. Repeated cancel
is idempotent; a cancelled plan cannot execute or resume. Cancellation does not
claim transaction expiry and cannot release an attempted ambiguous deployment.

`receipt.json` is produced only after the exact commit reaches global,
state-resolved `Applied` and authenticated alias/nonce plus stored artifact bytes
agree. `completed_receipt` authenticates all retained evidence and rechecks the
exact commit against the configured network without submission. Signed plans
are runtime artifacts; only the finalized receipt is intended for sharing.
