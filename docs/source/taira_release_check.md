# Taira release checks

Run either `python3 scripts/taira_release.py check` or
`python3 scripts/taira_release_check.py` before the Taira four-binary Linux
release build. Both compile focused CLI, Core, proof, Torii and consensus
harnesses plus native network binaries with six Cargo jobs
and report build, stage and test durations. Python 3.11+, the repository Rust
toolchain, and previously fetched dependencies are required; Cargo runs offline.

The Torii harness executes the shipping routes through plan, prepare and submit,
checks SDK receipt verification, and applies queued fixture transactions. It
covers a 33-hour-old committed anchor, expiry without a new block, exact replay,
onboarding identity binding, faucet funding and proof-of-work anchor aging. A
fresh onboarding receipt uses service time for its lifetime; committed height,
hash and ledger time remain the anchor for state and lease observations. These
contract tests use real route and ledger code with disposable inputs; they do
not replace the deployed four-validator consensus and public application checks.

The first gate compiles the dependency-free production consensus FSM directly
with the selected Rust compiler. It lists and runs every reducer test without
Cargo, rejecting missing, ignored, duplicated or failed cases. The four-reducer
traces exercise reordered messages, duplicates, loss and recovery, including
durable append before acknowledgement; they simulate authenticated I/O and do
not replace cryptographic or network execution.

The Core gate then exercises bounded worker backpressure, durable-sidecar retry
ownership, QueuePlan handoff across view changes and inventory changes, and
participant predecessor recovery with the exact reservation retained. It also
exercises the real candidate provider with multiple routable lanes.
Ordinary transactions remain eligible for global proposals; `QueuePlanSynced`
transactions require their autonomous reservations, and actual reservation
conflicts still defer ordinary work. A regression in any of these paths stops
preparation before the Linux release build.

The proof gate produces and verifies fully witnessed eight- and sixteen-row
transfers with the default resource limits, checks the maximum admitted proof
shape against its canonical frame, and rejects proofs beyond explicit limits.
Payload accounting and framed persistence use separate ceilings derived from
the same canonical geometry. The CLI confirmation gate follows a queued hash
through its exact Applied wire proof, retaining ambiguous expiry as pending and
rejecting malformed or failed status responses without resubmission.

The next gate launches four validators from the freshly emitted native
`iroha3d` binary using the same three-route fixture as the consensus integration
suite. It submits a signature-bound `QueuePlanSynced` public transaction, as
required by public Torii admission, and requires state-resolved Applied
in both local and global status on every validator at the same committed height.
The CLI gate also checks that prepared Inrou pin operations preserve sponsored
fees and the public QueuePlanSynced intent through signing and replay validation.
Dedicated service-owned Ordinary admission remains a separate contract.
Global status can query other peers, so only the additional local observation
establishes each validator's own application. Peer clients ignore ambient client
identity and endpoint overrides; status observations have five-second requests.
Public submission retains the SDK's routed request budget and reconciles its
original hash before the separate 90-second all-peer observation phase.
Cargo emits both node and client paths explicitly; fallback builds and
sandbox skips are disabled. The focused `taira_consensus_contracts` harness avoids
compiling the full consensus suite, and keeps private fixture logs in the warm
target for failure diagnosis. Test ports use lifetime-held OS leases in an
owner-private host runtime directory. A reserved but unbound port remains
exclusive; process exit releases its lease without writing into the source
tree or deleting shared lock files.

Each temporary peer has an explicit 1 GiB storage budget on the shared host.
The gate requires 8 GiB free before compilation and checks again before peer
startup: 4 GiB for the four budgets and 4 GiB for scratch files and logs.
Production filesystem auto-sizing remains unchanged. Codec table fixtures are
embedded, materialized without source permissions, and reused across restarts.

For a testnet attempt stuck on an unresolved canary before edge staging,
`iroha taira public-reset abandon` takes the original signed inventory,
authorization and SSH custody, `--expected-journal-sha256`, and the explicit
`--abandon-pending-mutations` flag. It preserves the exact unresolved journal
before running ordinary rollback; cache expiry never becomes a chain rejection.
Keep the original fixed host dispatcher until rollback completes. Use the
original journal digest again to resume an interrupted abandonment.

Ordinary checks use the existing sibling `.taira-testnet-build-targets/routine`
directory. `--target-dir` or `TAIRA_TESTNET_CARGO_TARGET_DIR` can select another
existing development lane; both must agree if supplied. Ambient
`CARGO_TARGET_DIR` does not select this lane. Neither command creates or cleans
a Cargo target. Keep a stable lane for repeated checks.

The focused native gate enables incremental compilation in that existing lane.
Incremental native runs bypass sccache, which rejects `CARGO_INCREMENTAL=1`.
Nonincremental native and Linux release builds retain the persistent sccache.
An explicit `CARGO_INCREMENTAL=0` preserves a constrained or CI build policy;
only `0` and `1` are admitted. This preference is passed only to native builds
and tests, and preparation records it with the native-check checkpoint. Linux
release compilation retains its original sanitized environment and release
profile. Each feature graph keeps its own Cargo cache; no test features are
added or removed to force reuse. The first incremental run populates those
caches, so a speed improvement must be measured on subsequent focused changes.
The FSM and focused Core regressions precede the four-validator runtime check, so source-staging or consensus
failures stop before compiling the independent contract harnesses. Its log
records the actual Cargo-selected native binary paths and profiles separately
from the later Linux release artifacts.

Authenticated `prepare` retains the repository's `target/` lane and its fixed
Git-object source capture. Its explicit `--target-dir` override remains available;
the development environment selector does not affect preparation. Checks refuse
the exact repository `target/`, existing source capture lanes, and lanes marked
for release. Preparation refuses the routine lane and lanes marked for development.
A stable `.taira-build-lane/` owner-private directory records the role and canonical
repository root and holds a nonblocking mode lock through Cargo and every harness
child. Another checkout must select its own stable lane, so alternating checkouts
cannot churn the same lane's source paths. Preparation also
retains its original source-custody and output locks. A busy lane fails before
compilation. Existing target permissions remain unchanged.

Both modes use the same isolated `target/taira-release-cargo-home`, including the
same lexical registry paths, selected Rust toolchain, explicit repository Cargo
configuration, optional persistent sccache and the existing linker selection.
Cargo starts from `/` to exclude ancestor configuration. Ambient Cargo settings
and runtime credentials are not inherited. Diagnostic checks still compile the
mutable working tree and verify HEAD continuity; they never qualify a release.
The mode locks coordinate these entry points only: arbitrary direct Cargo commands
do not acquire them. Keep those commands out of an active preparation lane.

The check runs focused regressions for secure inherited configuration and signing FDs,
network-369 inventory decoding, aggregate timeout admission before custody,
required preseed budgets, per-host carrier verification and the exact action ledger,
native genesis-path rebasing with secret-free errors and owner-only output,
configuration artifact custody and signed genesis startup,
generated stages frozen to 0400 and their native consumers, preseed receipt
ordering, KVM ioctl error preservation on a regular file, and all five read-only
host preflights. The process-stream checks send an exact-digest 64 MiB closure
through real pipes with both outputs backpressured, bound each output turn,
retry interruptions, and verify absolute deadlines, early stdin closure, and
cleanup of a descendant retaining the output pipes. They also preserve a remote
rejection and its bounded stderr after early stdin closure, drain diagnostics
larger than pipe capacity, reject a successful exit with an incomplete frame,
and kill and reap a child that closes stdin then hangs. They use the system
`/usr/bin/python3` only for harmless disposable child fixtures. The KVM regression needs no KVM device or root access and runs
on macOS and Linux. The receipt namespace checks cover every host action and
canonical artifact role, including underscore-bearing labels, and retain path,
control-byte and Unicode rejection. Systemd checks use captured numeric
`ExecMainCode=1` evidence for a completed manager operation and require
`active/running` for the long-running validator. They cover terminal failures,
pending jobs, malformed evidence and read-only recovery after a deadline.
Start and restart also wait for the signed Python launcher to execute the exact
daemon within the original deadline, without submitting another manager job.
Changed launcher commands remain immediate failures.
The active journaled restart path also waits for four actual Torii backends before
onboarding, using the deadline captured before its one restart submission.
Convergence waits within that deadline until the active height has advanced beyond
a positive committed frontier. A CommitQC or an `Applied` body at the same height
can still precede the durable application anchor needed by onboarding. Tests keep
both intermediate states out of retained proof; identity mismatches and restart
requirements remain immediate failures, with public progress in deadline errors.
Converged certificates use Core's committed-decision comparison, allowing
different re-proposal rounds for the same subject and execution commitment while
retaining each validator's actual certificate and requiring a higher committed
height after restart.
Candidate tests exercise the real HTTP producer and strict host receipt consumer,
direct signed probe origins, private signer descriptor lifetime, ordered recovery
and failure before edge cutover. Prepared-envelope tests cross the actual typed
write producer into the authenticated host consumer and all four Inrou variants
through inherited descriptors and predecessor decoding. Binding checks cover
canonical object metadata before submission and after commit, rejecting changed
fields and JSON strings substituted for objects. Unknown envelope fields remain
invalid. Stopped-owner tests preserve the slot lock while
releasing exact empty worker cgroups, reject live or substituted runtime state,
and verify idempotent cleanup before a stop receipt can be reused. Firewall
fixtures admit only exact slot-owned rules and real construction/deletion cuts;
foreign references or changed rules keep the barrier in place. Cleanup removes
each admitted rule explicitly and never flushes a chain.
Linux also runs a real OpenSSH
configuration-only check that verifies parent-held descriptor paths survive its
descriptor cleanup and replacement of the original paths. Every selected test
must exist and execute exactly once. Missing,
ignored, failed or empty selections fail the command. All independent cases in
a harness run before reporting their combined failures, so one bad fixture cannot
hide another defect until the next build. Fix the named failures and
rerun the same command to reuse compiled dependencies.

The selected toolchain's Cargo executes this command from `/` with the isolated
environment and selected `CARGO_TARGET_DIR`:

```sh
cargo --config /absolute/repo/.cargo/config.toml test --manifest-path /absolute/repo/Cargo.toml --locked --offline -p iroha_cli --bin iroha --no-run --message-format=json-render-diagnostics
```

Only existing disposable test fixtures are used. The gate accepts no live
configuration, keys, tokens, SSH, signing or deployment inputs, and writes no
report files. It is an early regression check and does not replace existing
release qualification, exact signed-source and artifact checks, or the offline
probes against actual Linux release binaries. Public readiness still requires
the end-to-end live checks.

The `build` job in `.github/workflows/workspace_release.yml` runs this check
before its full workspace build. CI provisions the stable `target/taira-native-checks`
subdirectory and passes it explicitly; the existing root target cache includes that
independent diagnostic target. CI runs `cargo fetch --locked` first to initialize
the same isolated registry cache, using the selected toolchain, explicit source
configuration and mode lock. Only this fetch sets `CARGO_NET_OFFLINE=false`; the
gate itself remains offline. The subsequent
full workspace build acquires the same development lane lock and uses the same
target, isolated Cargo home and explicit configuration. It explicitly preserves
CI's `CARGO_INCREMENTAL=0` after sanitizing the environment, as does the native gate. Matching dependencies can
therefore reuse the gate's artifacts instead of being rebuilt into a second tree. The existing local Taira release caller should run
it before cross-compilation. The canonical
release artifact producer remains `scripts/run_release_pipeline.py`; it does
not gain a hidden build step or additional runtime authority.

Validate the runner without Cargo:

```sh
python3 -B -m unittest discover -s pytests/scripts -p test_taira_release_check.py
```

Cargo compiler errors retain their rendered diagnostics while the gate consumes
JSON artifact events; accelerator progress is also preserved.
