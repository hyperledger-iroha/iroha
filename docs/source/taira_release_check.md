# Taira release checks

Run either `python3 scripts/taira_release.py check` or
`python3 scripts/taira_release_check.py` before the Taira four-binary Linux
release build. Both compile focused configuration, crypto, P2P, CLI, daemon, Core, proof, Torii and consensus
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

After the standalone FSM and lifecycle source checks, the dedicated
`iroha_config --test taira_config_contracts` target checks production descriptor
defaults, malformed collection values and the maintained Taira Nexus profile.
It loads no runtime signers and has no Core or test-network dependency. These
four unchanged contracts execute before the shared Core/library build, so schema
failures stop before that compilation. Storage-budget and ambient-client isolation
checks remain in the test-network harness. The gate computes and reports the
complete native census from its required test selections. The configuration target uses the same source capture, Cargo environment,
warm target, profile and inherited locks; it retains all package defaults. Its real
model/crypto/codec dependency graph may require a separate first warm compilation
when its feature union differs from the later node graph. No first-run or total
runtime improvement is claimed without measurement.

The combined native test build selects the exact `iroha_cli --bin iroha`
and `irohad --lib` test harnesses. The daemon cases execute signed genesis
and check the deployment account in its final staged state, including role and
revocation semantics; the deployment flag is restricted to offline `--check-config`. The genesis fixtures share the canonical daemon configuration and production staging setup; all affected unconditional manifest/crypto consumers are selected with them. A single Cargo invocation unifies the selected packages and their
default/dev-dependency features; it does not add feature overrides. The CLI test
copy executes under the original immutable artifact owner with all independent
library and daemon checks, before the separate production `iroha3d`/`iroha`
build and four-validator test. It needs no second Cargo test build. Its manifest,
bin target kind and test profile distinguish it from the Rust SDK's `iroha`
library harness and the production CLI. Each completed test copy is released
through its existing owner. Any collected test failure stops before production
compilation or network execution; production snapshots remain retained. Configuration and proof checks retain their
existing separate gates. Adding the CLI changes the combined test feature union,
so the first run must warm and qualify that union; latency savings require actual
measurement and are not inferred from these orchestration checks.

CLI client admission binds the configured chain, genesis NetworkId and account
address discriminant to the signed inventory during assembly, apply and recovery.
The same check runs before convergence child custody; stale runtime or validator
clients fail before operator-key admission or stage snapshots.

Core checks cover the complete Soracloud and SoraFS instruction inventories at
the Initial executor boundary. Dispatch and admission come from the same typed
registry; unsupported operations remain explicitly closed. Before Cargo,
`check_taira_initial_executor.py --repo . --self-test` compares every wire type
with that registry and checks mutations which remove or alter reviewed entries. The selected cases
exercise validator identity, role grants and revocation, pin fees and ownership,
and rejection before state mutation. Exact transaction confirmation uses the
dedicated details endpoint, with separate coverage for restricted history access
and failed child processes that attempt to return successful reports. A real
Torii socket-to-SDK regression checks the canonical ErrorEnvelope response;
only the dedicated proof-absence code plus HTTP 404 becomes retryable absence.

The same library batch includes Rust SDK envelope verification and Torii's exact
retained-payload, detached-signature and canonical response checks. The public
HTTP contract fixture then runs before node compilation and the four-validator
gate. It prepares and signs the exact QueuePlan payload through real routes;
its synthetic ledger has no certified committee, so submission must fail without
local enqueue. Separate execution overlays retain contract state assertions.
Only the four-validator and deployed checks establish canonical Applied state.

Every Cargo-produced harness and native executable is copied under Cargo's
profile locks into a private read-only directory before execution. Captured
release checks verify source fingerprints while holding those locks; foreign
checkout fingerprints fail the check. Later stages and CLI capture use the
copies, preserving the original Cargo metadata. Another build cannot replace
the selected executable between compilation and execution. Each private test-harness
copy is released after its final subprocess finishes, including a failed test.
If an earlier stage fails, the same isolation owner releases its unused harnesses.
Only those exact copied inodes are eligible: Cargo outputs, native `iroha` and
`iroha3d` snapshots, fixture logs and artifact observations remain retained.
Changed paths fail cleanup without deleting their replacements. Abrupt preparer
termination or a failure before copy publication can leave copies for diagnosis;
this workflow never scans old runs or deletes other owners' files. Keep concurrent
development builds in their own stable Cargo target slots to avoid lock waits
and source-fingerprint conflicts; the warm release target remains reusable.

The crypto and P2P gates check authentication deadlines, puzzle cancellation,
memory ownership during in-flight work, exact solution verification and validator
dial/retry ownership. Dial plus preauth bounds outbound authentication and standby
takeover independently of established-session idle. Dev/test profiles optimize
Argon2 and Blake2 arithmetic while retaining the production puzzle policy.

The Core gate then exercises bounded worker backpressure, durable-sidecar retry
ownership, QueuePlan handoff across view changes and inventory changes, and
participant predecessor recovery with the exact reservation retained. It also
exercises the real candidate provider with multiple routable lanes.
Ordinary transactions remain eligible for global proposals; `QueuePlanSynced`
transactions require their autonomous reservations, and actual reservation
conflicts still defer ordinary work. A regression in any of these paths stops
preparation before the Linux release build.

QueuePlan admission authenticates immutable certificate bytes before opening a
State view or acquiring the publication fence. Fresh history, committee, route,
incarnation, registry and application checks remain under the coherent view.
The regression observes the real decoder and fails if signature authentication
holds the publication lock or repeats during classification, first persistence
or exact replay. Companion cases retain historical authority, future-frontier,
conflicting certificate and bounded one-ahead publication behavior.

Finalization regressions keep a recipient permanently backpressured and require
the exact durable output handoff to release its retained work. Foreign finality
authority must fail without retiring output. Live and already-applied restart
recovery drain their finite ingress prefix before reaching this handoff; remote
delivery cannot block the local durable boundary that reconstructs that output.
Before lane preflight, committed global finality and exact Kura sources can
already release independently reconstructible fanouts. This frees shared
capacity for pending historical responses and lane certification, including when
older output owns an actor ticket, a parked route or an unfinished writer flush.
The partial handoff preserves unresolved lane evidence, exact surviving FIFO
ownership and sidecar receipts; it does not fabricate network delivery.

The proof gate produces and verifies fully witnessed eight- and sixteen-row
transfers with the default resource limits, checks the maximum admitted proof
shape against its canonical frame, and rejects proofs beyond explicit limits.
Payload accounting and framed persistence use separate ceilings derived from
the same canonical geometry. The CLI confirmation gate follows a queued hash
through its exact Applied wire proof, retaining ambiguous expiry as pending and
retaining the last observation when a status read times out under the remaining
confirmation budget. It continues polling within that deadline;
shorter configured request timeouts, malformed responses and other lookup
failures remain errors. Confirmation never resubmits the transaction.

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
The FSM, source checks and independent native regressions precede the
four-validator runtime check. Unit test failures stop before production binary
compilation; the contract test harnesses share the earlier combined graph. Its log
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
ignored, failed or empty selections fail the command. All independent cases in each combined library, daemon and CLI harness run
before reporting their combined failures, so one bad fixture cannot hide
another defect until the next build. Missing selected tests, artifact custody
failures and infrastructure errors still stop immediately. Fix the named failures and
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
