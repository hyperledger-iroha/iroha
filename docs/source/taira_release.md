# Local Taira release preparation

Run the native CLI checks, build the four AArch64 Linux executables, and capture
read-only copies through one maintained command. This replaces per-release local
build and capture scripts. Python 3.11+, Git, the repository Rust toolchain,
cargo-zigbuild, Zig and an existing warm Cargo target directory are required.

Run only the early native gate:

    python3 scripts/taira_release.py check

Before signing an immutable release, use an exact focused diagnostic in the same
warm development lane:

    python3 scripts/taira_release.py check \
      --focus-regression core=state::tests::historical_autonomous_merge_recovers_certified_carrier_before_world_replay

Repeat `--focus-regression HARNESS=EXACT_TEST` for more selected regressions. The
diagnostic compiles the complete shared native harness graph, runs mandatory
configuration checks first, then only the named tests. Names must already belong
to the chosen `--native-check-scope`; unknown or repeated selections fail before
Cargo starts. Independent failures are aggregated; dependent network tests run
only after those checks pass. This mutable-source diagnostic writes no release
qualification checkpoint. `prepare` has no focus option and still requires its
complete immutable gate. Omit the option to run the normal development gate.

Prepare binaries from an explicitly selected signed commit in the optimizations repository:

    python3 scripts/taira_release.py prepare \
      --expected-commit FULL_SIGNED_COMMIT \
      --expected-signer REVIEWED_SIGNING_KEY_FINGERPRINT \
      --output-dir /absolute/private/output-for-this-build \
      --zig /absolute/real/zig \
      --zig-sha256 REVIEWED_ZIG_SHA256 \
      --cargo-zigbuild /absolute/real/cargo-zigbuild \
      --cargo-zigbuild-sha256 REVIEWED_CARGO_ZIGBUILD_SHA256

Use independently reviewed tool digests and the full signing-key fingerprint
(GPG uppercase hexadecimal, or SSH SHA256 form). A valid signature from another
locally known key is rejected. Paths must be absolute and contain no
symlinks; PATH must resolve cargo-zigbuild to the supplied executable. Source
verification binds the maintained helper scripts to the supplied signed commit.
Git replacement refs are disabled. Every `BUILD_SOURCES` controller file is checked
against its signed Git blob and executable mode, independently of the mutable
index and flags that conceal local edits. Imported controller modules must come
from those checked paths; additional checkout modules cannot enter the controller.
Unrelated staged, unstaged and untracked work stays untouched and is never copied
into the build. Gitlinks retain their signed mode and commit in the capture and
remain empty there; worktree submodule contents are excluded. There are no
embedded tool digests or release-number-specific paths to update.

Fresh preparation and resume use the same source admission: the selected repository
must be on `optimizations`, and `--expected-commit` must name the exact signed Git
commit with the supplied full signer fingerprint and matching controller sources.
The selected commit can differ from HEAD, so unrelated commits already made on
`optimizations` do not enter a new build. The command does not move HEAD, modify
the index, or require another checkout.
It reads the selected commit's Git objects into a private, read-only source
capture under the selected Cargo target's `taira-release-sources/`. It creates no Git repository,
worktree or branch. Both native checks and the Linux build consume this capture;
One explicit `source/target` binding points to the selected existing warm Cargo
target for native fixture output; source inventories verify this binding without
traversing generated files. Native fixture processes also run from that external
target directory. The snapshot covers signed Git entries; the output binding is
recorded separately as `source_output_target`. Every signed source path remains
read-only. Subsequent
checkout edits, merges or HEAD changes cannot mix source versions into
the build. Native test selection is loaded from the captured gate helper, including
a resumed check after the checkout has changed. Resume additionally authenticates
the recorded request and captured bytes. Unrelated HEAD advancement is allowed
before fresh preparation and resume; a different branch, signer, or executing
controller is rejected before source capture. Changes to controller files require
a preparation selecting the signed commit containing those exact controller files.

Each selected warm Cargo lane has one stable source path. A lane-wide lock covers
capture refresh, native checks, Linux compilation and artifact capture, including
an active child if its launcher exits. Source replacement occurs only between
preparations. Unchanged files and complete unchanged directory subtrees retain their timestamps.
Directory watches in native build scripts therefore remain fresh when only
unrelated source changes. Added, removed, renamed, mode-changed, or edited
descendants invalidate their ancestors. Routine releases can
reuse Cargo's cache. Interrupted source publication is recoverable; previous
source directories and unfinished copies remain retained alongside the current
capture.

Cargo can retain dependency records pointing at an older source directory even
when the selected manifest changes. Before compiling, preparation inspects local
package records under Cargo's profile locks. Native checks inspect only the
debug profile family; Linux release builds inspect only the release profile
family, including their host build-script records. One family's admission never
retires the other's reusable fingerprints. Records for another source tree are
retained under `taira-release-cache-retired/`, outside Cargo's active fingerprint
lookup. Compiled outputs and registry/Git caches remain in place. Current-source
records are reused. After building, the same admission must pass without repairs;
Cargo's profile locks remain held through artifact capture. The regression suite
reproduces the stale host build script with two real source directories and proves
the corrected build executes the new source.

`check` defaults to the existing sibling `.taira-testnet-build-targets/routine`
development lane. Set `TAIRA_TESTNET_CARGO_TARGET_DIR` or `--target-dir` to select
another established development lane; when both are set, they must agree.
`prepare` defaults to the checkout's existing `target/` release lane and ignores
the development-only environment override. Its `--target-dir` must select an
established release lane. The commands enforce separate lane ownership: mutable
development checks cannot use a captured release lane, and authenticated
preparation cannot use the routine development lane. Preparation uses the
unchanged release profile and six jobs for exactly iroha3d_taira, iroha,
sorafs-node and kagami. Preparation selects the Rust toolchain from the captured
`rust-toolchain.toml`, then runs Cargo from `/` with the captured manifest and
`.cargo/config.toml` explicitly selected. A persistent config-free Cargo home
under the project target reuses the existing registry and Git caches. Ancestor
and home Cargo configuration cannot override these inputs; a root-level Cargo
config stops preparation with an actionable error. The captured Zig driver and
installed sccache remain in use. No target or cache is cleaned or replaced.
Compiler overrides, interpreter hooks and runtime credentials are not forwarded.
The native gate receives the same source, toolchain and explicit target directory.

Both scopes also verify that idle governance sweeps create no execution fragment,
while successful and failed due sweeps retain their effects and audit records.

Both native qualification scopes require certified catalog and bootstrap parameter
commit and recovery tests, plus rejection of changed parameters or mismatched
runtime effects after staging. The four-validator catalog test separately proves
the committed topology and transaction history survive restart and full replay.

Both scopes also require the generated validator configuration projection and
occupied-runtime recovery tests. Prior daemon, CLI, SoraFS, configuration, genesis,
genesis hash and service unit each have an explicit source revision, path, digest,
size and mode. The configuration selector and exact process argv are bound
separately. An initialized installation may therefore retain artifacts from
different releases without treating its configuration revision as its executable
revision. Candidate artifacts still use one canonical release directory.

After verifying the complete native Cargo output set, a bounded owner-private
ledger records only final test executables and retires recorded superseded outputs
before checking copy capacity. Retirement runs under Cargo's locks after exact
inode checks and an OS open-file check; a later copy failure leaves the current
verified Cargo outputs intact.
The first run only records current outputs; unrecorded files, production binaries,
libraries, object files and warm compiler caches are retained. Busy files or an
unavailable/inconclusive `lsof` check cause retention. A private quarantine closes
the old pathname before the final open-file check; interrupted retirement remains
recorded. Retries recover both rename windows using the recorded inode and stable
metadata, recheck open-file status, and never adopt an unrelated replacement. A
full ledger retries pending cleanup before admitting successors, so closing an
old reader restores progress without manual ledger edits.

Temporary executable copies, including non-CLI native network binaries, are
released after their last child exits, on success or failure. The published native
`iroha` CLI snapshot remains available to operator custody and deployment consumers;
the `cli` test harness is temporary. Exact identity and closed-file checks preserve
replaced or busy copies. If isolation fails partway through a batch, only already
verified copies are cleanup-owned, including an unpublished CLI; incomplete or
unrecorded files are retained. Observations and fixture logs remain. Cargo producers
and Linux release artifacts are outside temporary-copy cleanup.

Before Cargo, the gate compiles the dependency-free consensus reducers and the
shared lifecycle source assertions directly with the pinned Rust compiler. Both
must execute every listed test without skips. Lifecycle mutation controls check
that removing or reordering required retries still fails. The gate also reconciles
the shipping binary table with Cargo manifests and the early compilation targets.
Configuration library and integration tests, CLI, SDK, Torii, crypto, P2P, Core, proof and fixture harnesses,
including all four shipping entry points, then share one Cargo invocation,
resolving the union of their existing default features. Configuration runs first
and fails immediately, including when an independent-test checkpoint can be reused.
Core, Torii and daemon startup recovery checks run next; failures are collected
across those startup groups before stopping, without running CLI or network tests.
These include bounded regressions for failure reporting and worker teardown under
a held lifecycle operation, plus retained-output recovery through real actor admission.
Live Decision cleanup also exercises the shared runner reconciliation after an idle
runtime turn, before exact acknowledgement can release the Apply fence.
Recovered Decision Fetch checks run real periodic runtime turns before the signed
response arrives and while its persistence is queued, then complete Store,
Validate and the cold Apply handoff. An exact retry retains the original request
owner; unrelated or unauthenticated work cannot claim its coordinates.
CLI and the canonical Kagami projection checks then precede the proof, crypto,
transport, consensus and fixture selections. Every independent failure stops
before daemon startup. Shipping targets without selected tests provide actual
compilation evidence, with no invented test passes. The native production build
uses the same four shipping packages and binaries, plus the ordinary `iroha3d`
fixture launcher; the four-peer test continues to launch that ordinary binary.
The Linux release command and its production features remain unchanged. Proof
harnesses are not rebuilt separately after the network test. This reduces repeated
dependency work; it does not promise a fixed build duration.
The fixture checks read the public Taira Nexus profile without runtime inputs
and verify that collection decoding preserves declared configuration defaults
while rejecting malformed values.

Rerun the exact same `prepare` command and output directory after interruption.
The command locks that owner-private preparation directory, checks that its
recorded inputs still match the fixed signed source capture and tools, and resumes locally:

- Completed native checks are reused for those exact inputs unless foreign
  local-package fingerprints had to be retired.
- A failed or interrupted build runs Cargo again in the same warm target. Cargo
  reuses its cache; each attempt gets a fresh private log and capture directory.
- A completed read-only capture is revalidated and reused without running checks
  or Cargo, including a crash before the final result was published.
- Changed input identity or a modified captured binary stops with a specific
  error. Active preparations retain their lock; a second command stops promptly.

The command reports stage durations and emits elapsed time, compiler-output size,
and the log path every 30 seconds during Linux compilation. It does not repeatedly
hash source or artifacts while reporting progress and does not impose an arbitrary
cold-build deadline. Captured source, tools and artifacts are checked at actual consumption
and reuse boundaries. Runtime secrets remain excluded from the child environment.

Before initial source capture, local admission counts the signed Git blobs' exact
byte sizes without reading unrelated worktree files.
Before compilation, it groups requirements by filesystem and checks an 8 GiB
Cargo working-space floor plus 256 MiB capture headroom. Before capture,
it checks the exact binary-copy bytes plus that headroom. The build floor is an
operational minimum, not a prediction of Cargo's peak use. This local check cannot
observe a remote guest's sparse backing disk. Run the deployment capacity check
below on the guest and its backing host. No cache or output is deleted
automatically, and the warm Cargo target is never replaced with a new lane.

The output directory contains read-only `request.json`, `checks.json`, and
`result.json`, a persistent private `session.lock`, and numbered `attempts/`
directories. Failed attempt logs and partial captures stay available. Successful
captures contain four 0500 executables and a 0400 `capture.json`; its artifact paths
are returned in `result.json`. The successful attempt, capture and top-level output
directories are 0500. The mutable Cargo binaries remain in place. Existing unrelated
output directories cannot be adopted as resumable preparations.

This result remains a local build observation with `release_qualified=false` and
`deployed=false`. It is not an authenticated prebuilt-provenance manifest accepted
by `run_release_pipeline.py`, and local resume does not resume a deployment journal.

Canonical release signing remains in run_release_pipeline.py. Controller import
and native public-reset source-manifest, assemble, authorize, preflight and apply
remain separate operations under their existing authority. This helper accepts
no runtime keys, tokens, SSH, import, activation or publication options.

Validate the local orchestration without Cargo or network:

    PYTHONPATH=scripts python3 -B -m unittest discover -s pytests/scripts -p 'test_taira_release*.py'

The gate's existing selection and diagnostics are documented in
[Taira CLI release checks](taira_release_check.md).

## Preparing validator configuration for a public reset

Generate the fresh four-validator Taira bundle with the qualified native Kagami
on the approved Linux validator host. Keep its private output outside Git at the
same absolute path throughout installation and operation. Project each generated
validator through the same-revision native CLI using an inherited owner-private
descriptor:

    iroha taira public-reset materialize-validator-config \
      --config-fd 198 \
      --localnet-dir /absolute/private/generated-network \
      --validator taira-validator-1 \
      --network-id REVIEWED_CHECKED_NETWORK_ID \
      --genesis-file /srv/taira/taira-validator-1/releases/COMMIT/genesis/genesis.json \
      --operator-public-key REVIEWED_CANONICAL_OPERATOR_PUBLIC_KEY \
      --output /absolute/private/taira-validator-1.toml

Descriptor 198 must already identify the corresponding generated peer config;
the command does not open a private input by pathname. It verifies the generated
public genesis identity, maps all eleven mutable paths into the validator's
reset-managed state directories, sets explicit snapshot storage, and binds the
installed signed genesis and operator-authentication key. It rejects inherited
configuration, changed source paths and preexisting output. It emits no private
configuration to stdout. The generated onboarding key, faucet key, public rANS
table and `nexus.registry.manifest_directory` paths remain in the configuration,
so their original directory must remain available on that host. The manifest
directory must be exactly `lane-manifests` under the generator directory; a
registry cache overlay is rejected. Native signing and startup bind its semantic
policy digest to signed genesis. Retain and revalidate the generated public
manifest receipt for exact byte custody; the reset inventory does not declare a
separate manifest artifact.

Derive the complete public identity bundle using the maintained CLI:

    iroha taira public-reset prepare-public-inputs \
      --localnet-dir /absolute/private/generated-network \
      --inventory-draft /absolute/private/inventory-draft.json \
      --output-dir /absolute/private/public-inputs

The unsigned draft must omit the generated `beacon_bootstrap` field, including
when it is derived from a predecessor inventory. Native validation extracts the
canary public identity from its exact onboarding request and validates the signed
genesis against the generated raw manifest. The command atomically writes five
public artifacts: `genesis.json`, `genesis.signed.nrt`, `genesis.hash`,
`canary-onboarding-request.json` and `public-inputs.json`, with mode0644 inside a
mode0700 directory. The typed record binds `raw_manifest_sha256` and distinguishes
the native consensus genesis hash from the signed wire's SHA256. An incomplete
four-file bundle is rejected; prepare a fresh complete output. Repeating an
identical complete request verifies the retained bundle without replacing it.
The explicit `--canary-public-key PATH` alternative is mutually exclusive with
`--inventory-draft` and reads only that public key.

Derive the fresh beacon request and exact per-validator credential paths from the
same draft and public bundle:

    iroha taira public-reset prepare-beacon-inputs \
      --inventory-draft /absolute/private/inventory-draft.json \
      --public-inputs /absolute/private/public-inputs \
      --output /absolute/private/beacon-inputs.json

Use the authenticated same-revision unit renderer for each returned final-unit
entry, preserving its initial runtime-key and mint-finality-seed paths and using
the exact native `credential_path` with `--global-beacon-credential` and
`--config-file beacon.toml`. The [maintained retry caller](taira_retry.md) verifies
the pinned renderer and initial units before rendering these four final mode0644
units. The native request is not hand-authored JSON.

Both `public-reset assemble` and `authorize` require the same `--public-inputs DIR`,
`--beacon-inputs PATH` and four ordered `--beacon-validator-unit` paths, in addition
to their original local inputs. Native assembly independently rederives the
request, seat map and required signed `beacon_bootstrap` plan; the original seven
artifacts and initial units remain unchanged. See the complete
[assembly example](../../configs/soranexus/taira/README.md#public-reset).
The existing execution, source, config and authorization checks remain required.
The signed genesis must leave room for onboarding, funding, the canary's real
QueuePlan admission and execution carriers, and certificate installation before
the first mandatory pulse. Finalization uses the authenticated observed height.
The sole threshold-key certificate uses signed Ordinary admission, retaining its
exact next-height and current-roster quorum checks; other public transactions
continue to use QueuePlanSynced admission.

Public validator client settings can reference the native-generated
`runtime/taira-runtime-signers/peerN.private_key` sidecar through
`account.private_key_file`. Use the exact checked `network_id_file`, account
`chain_discriminant = 369`, and each validator's Torii origin. Extract the
canonical public key from its generated public manifest account with the native
`iroha tools address convert ACCOUNT --profile taira --format public-key`.
This representation requires a single-signatory account and rejects multisig
controllers. No private configuration parsing is needed to construct these
public fields and file references; native loading validates the key pair.

Each candidate validator includes a seventh `validator_unit` artifact at
`systemd/<systemd_unit>` with exact mode 0644. Assembly requires its bytes to
match the explicit validator unit input and digest. Reset execution durably
retains the prior unit, records publication intent, atomically installs the
candidate unit and records the exact service-manager reload. Interrupted forward
and rollback publication resume only from that durable evidence. An occupied
validator additionally signs its required prior `service_state`: `running`, or
`stopped` with the independently selected state-root device/inode. There is no
implicit state or fallback from a failed running-process check. Both modes retain
all exact prior artifact, loaded-unit, selector and custody checks. Stopped mode
also proves no service job, PID, populated cgroup or escaped state reference.

Reset retains the original state inode at
`<reset_guard>/rollback/<authorization_nonce>/state` by same-filesystem rename;
it does not copy old ledger contents into the newly initialized chain or create
an off-host backup. Approval must identify the old/new genesis and resulting
active-state loss. Rollback before deployment proof restores the old unit,
selector and retained state. Running mode proves the restored old process;
stopped mode stays stopped and proves absence, without claiming recovery or
health. Cached and conservative rollback use the same signed state. Cleanup
protects every admitted prior artifact root. Proven deployments cannot roll back
through this workflow, and ambiguous writes require their retained recovery path.

These preparation operations do not authorize replacement of shared network
state. The reviewed inventory, explicit reset authorization, and independently
provisioned trusted host dispatcher and reset guard remain prerequisites for
`public-reset apply`. The candidate cannot provision its own host authority.

## Updating an initialized testnet

For a routine update of the existing four-validator Taira installation, use the
completed basic preparation in three explicit phases. First prepare the exact
same-release binaries at the operation's immutable release path:

    python3 scripts/taira_update.py \
      --prepare-artifacts \
      --deployment /absolute/owner-private/taira/deployment.json \
      --prepared-result /absolute/completed-preparation/result.json \
      --operation update-0123456789abcdef0123456789abcdef \
      --output /absolute/owner-private/taira/artifact-output

Then invoke that admitted candidate `bin/iroha` to materialize the immutable
supervisor generation. The preparation wrapper binds the same operation, original
and desired service states, original and actually installed unit bindings, and
successor policy/unit/custody. Its required `original_seed_sources` contains four
exact sorted `{validator, path}` references to the original validator seed files.
The native helper retains those exact bytes; it never generates replacement
seeds. Existing generations may select the already retained original files. The
operator supplies already-open administrator and HTTP private input descriptors;
Python does not read or hash any credential or seed contents:

    /absolute/runtime/release-COMMIT-update-0123456789abcdef0123456789abcdef/bin/iroha \
      taira public-reset epoch-supervisor-host materialize \
      --wrapper /absolute/owner-private/taira/epoch-generation-preparation.json \
      --administrator-config-fd ADMIN_FD --http-operator-key-fd HTTP_FD \
      --timeout-ms 90000

Bind the returned exact public provisioning receipt reference into the final
supervisor wrapper. Apply the reviewed transition using the same operation:

    python3 scripts/taira_update.py \
      --deployment /absolute/owner-private/taira/deployment.json \
      --prepared-result /absolute/completed-preparation/result.json \
      --operation update-0123456789abcdef0123456789abcdef \
      --supervisor-plan /absolute/owner-private/taira/epoch-supervisor-update.json \
      --output /absolute/owner-private/taira/update-output

The deployment record contains the approved SSH route and public host-key pins,
network and directory identities, and the exact completed predecessor receipt.
Keep it outside Git. Artifact preparation creates the exact same-release daemon,
CLI and Kagami from the maintained four-artifact preparation without overwriting
existing files. Apply requires all three already provisioned files and rechecks
their exact native digests, size, source and root-owned mode0755 custody; it never
creates a missing binary as a fallback. It preserves
configuration, signer custody and ledger state, and verifies native
Strict snapshot restoration and public basic health. It does not invoke Cargo.
All four validators must prove the candidate identity and restore their own stopped
retained tips. Every overlapping stopped prefix is checked before startup. Two
fresh samples must each contain at least three Ready validators agreeing on the
stopped cohort's highest committed block hash. Every sample attempts all four
validators and records missing, unready, and lagging peers explicitly; stale
observations never contribute to quorum. After the public health check, the
updater repeats both quorum samples and checks all four unchanged processes.
Identity, hash, malformed response, and process failures stop immediately; only
declared startup transport failures and HTTP 503 are polled. An idle chain does
not need to create another block to pass.
`--plan-only` writes the concrete plan locally without contacting the host.
The operation is explicit and determines the immutable candidate binary paths;
there is no random operation fallback. The required public supervisor wrapper
binds the exact raw policy, current observation trust, custody references, fixed
unit, same-release CLI/Kagami, and native provisioning receipt. Its original
`before` binding and `original_service_state` remain immutable across recovery;
`installed` separately records the unit actually published. The required
`successor_service_state` preserves an existing running or stopped state and
explicitly selects the first-install state when the original was absent.

The policy grants explicit ongoing `until_stopped` epoch maintenance; a finite
reset lease does not grant this authority. The administrator is a separately
provisioned genesis-authorized client, distinct from canary and HTTP identities.
Only the native `public-reset epoch-supervisor-host materialize` boundary consumes
inherited administrator-config and HTTP-operator-key descriptors or seed custody.
Python handles public bindings and receipts only. Existing original trust and
once-per-epoch journals remain unchanged.

The updater holds `/var/lib/taira-epoch-supervisor/.deployment.lock` throughout
the transition and rejects any retained `.reset-owner.json` without clearing it.
It journals the supervisor pause before stopping any validator and retains a
native journal-lock child across validator replacement and qualification. An
ambiguous pause or partial validator stop admits read-only reconciliation only.
After a confirmed stop, pre-start failure leaves both services paused; after
candidate start, failure contains the candidate validators and supervisor even
if evidence publication fails. The native journal guard is released immediately
before an explicitly authorized supervisor start. A running successor succeeds
only after native `supervisor-status` authenticates initial and current epoch
completion for the same policy and unchanged live worker and manager identity;
a stale receipt or active unit alone is insufficient. Stopped intent stays stopped.

After all four stopped checkpoints are recorded, the matching candidate CLI runs
`iroha taira stopped-owner-maintenance` once before unit replacement or startup.
It verifies the live updater parent and its update flock, the retained public plan,
the exact stopped units and state roots, and acquires all four existing signer
slot locks before cleaning stopped native owners. Python passes only public
operation and process identities through a read-only descriptor; the native
custody boundary owns cleanup. The operation retains a maintenance intent,
per-slot receipts and `stopped-owner-maintenance-result.json`. Native maintenance
has a 120-second deadline and its caller a 150-second timeout. Failure leaves
the cohort stopped for inspection and prevents candidate installation or startup.

Local and guest locks serialize updates. Each operation retains its own staging
and evidence paths, so a failed transfer or lock conflict can use the same
completed binaries in a fresh operation after inspection. Failed stages remain
on disk. An interrupted runtime mutation requires recovery before another update;
there is no automatic rollback to earlier execution rules after candidate start.
A successful update emits `next-deployment.json` for the next invocation. Confirm
an application transaction as state-resolved Applied after installation.

For an update that installed all four units and failed after starting the new
daemon, retain the last completed deployment record and pass
`--failed-start-chain /absolute/owner-private/taira/failed-chain.json`.
Use schema `taira.failed-start-chain.v1` with an `attempts` array ordered oldest
to newest. Each entry contains its exact `operation`, a `plan` reference, and
`records` references for `intent.json`, `before.json`, `checkpoint-stopped.json`,
`start-intent.json`, and `failure.json`. Every reference contains the absolute
public-record `path` and its `sha256`; `plan` and `intent.json` bind identical
bytes. Capture only these public records. When appending a failed corrective
attempt, preserve all earlier entries and historical records unchanged.

The chain admits 1–16 distinct operations, with an 8 MiB bound per public record
and a 32 MiB aggregate read budget. Each intent must authenticate the exact earlier
prefix. Installed unit bytes follow the immediately preceding attempt; accepted
health remains the unchanged completed baseline. Every ancestor must have all
four units installed and a recorded startup failure, with no completion or
rollback marker. The guest checks every retained Kura prefix before its normal
stop/checkpoint/start verification. Partial observations remain diagnostic.

Retry the same retained `--prepared-result` after an infrastructure failure; a
new operation does not require Cargo or a source change. Any reused source commit
must retain the exact daemon and CLI package, size, and digest throughout the
chain. A rebuilt binary with a different identity under that commit is rejected.
Every retry uses fresh staging and evidence, preserves the latest authenticated
snapshot and Kura prefix, and repeats all four-validator health checks. It never
promotes a failed attempt to completed health or automatically restarts old code.

Probe failures identify the validator, endpoint and native exit code. Process
observations include PID, invocation and restart count so an unavailable listener
can be distinguished from a restarted worker without exposing response bodies,
configuration or native error output.
Startup and unhealthy observations have a ten-minute limit. Each lagging peer
gets another ten minutes only when healthy observations of the same candidate
processes show that peer's committed height advancing. Another peer's progress
cannot conceal a stalled validator, and a height regression is rejected.
Catch-up has an absolute ninety-minute limit; the host adds a conservative
bounded allowance for staging, final checks and all sixteen possible ancestry
records. The updater returns as soon as
all four peers verify the existing common prefix and readiness. Waiting never
restarts a daemon, rebuilds a binary, requires empty blocks, or adds a fixed soak.

After native start succeeds, `cohort-observation-intent.json` identifies the
read-only observation phase by operation, candidate, updater PID/start time and
the existing update flock's device/inode. A separate observer can verify that
live owner through public procfs and check that no terminal receipt exists.
This records an in-progress rollout; it does not assert four-peer completion.

Validate this controller without Cargo or network:

    python3 -B scripts/tests/taira_update_test.py

## Deployment capacity and retrying an unchanged release

Keep the build identity separate from the deployment attempt. A storage or host
failure does not require another build, source import, or binary transfer when
the retained artifacts still match their successful preparation and transfer
receipts. After a completed native rollback, retain its terminal record and
create fresh deployment custody and authorization for the same artifacts. Never
replay the failed authorization or edit a terminal journal to resume it.

Use the maintained retry command with the retained preparation, transfer and
runtime evidence:

    python3 scripts/taira_retry.py \
      --plan /private/runtime/taira-retry/operator-plan.json \
      --output-root /private/runtime/taira-retry/local-evidence

It allocates a new attempt automatically, preserves completed rollback history,
runs native assembly and authorization, and completes seed continuity and boot
persistence. Repeating it before apply resumes the same operation; after the
durable apply frontier, a real native rollback terminal is required. See
[the retry command](taira_retry.md) for input custody, capacity admission and
public application validation. No numbered retry adapter needs to be edited.

Before preparing a deployment, and again immediately before apply, run:

    python3 scripts/taira_disk_capacity.py --plan /absolute/public-capacity-plan.json

The public JSON plan uses schema `taira.disk-capacity.plan.v1` and an
`allocations` list. Each allocation has exactly `path`, `label`, `bytes`, and
`inodes`. Quantities describe additional allocated space, including concurrently
live temporary files and explicit operating headroom. The helper resolves paths
without following symlinks, groups requirements by actual filesystem, and checks
both available bytes and inodes. It reads no configuration or credentials and
performs no cleanup. Failure returns exit code 2 with required and available
quantities. Success is an observation, not a disk reservation.

For the current four-validator deployment on one physical guest, a fresh attempt
requires `3*A + 2*S + 4*P + 4*R`, plus operating headroom:

- `A` includes the complete artifact sets for all four validators and the edge.
  The coordinator snapshot, host uploads, and installed releases coexist.
- `S` includes the full Inrou stage tree. Its coordinator snapshot and host upload
  coexist with the already retained source tree.
- `P` includes one complete SoraFS store: all three payloads, chunk allocation
  slack, manifest and PoR metadata, index files, and temporary publication files.
- `R` includes one replica's hydrated guest, independent writable root disk,
  application extraction and publication, data lease, ephemeral storage, and
  runtime metadata. All four replicas keep their own runtime files.

The helper's `allocation_bound` and `cohost_peak_plan` functions construct these
byte and inode bounds. See [the allocation contract](taira_disk_capacity.md) for
the persisted metadata limits and backing-volume requirements. Logical SoraFS quotas are not free-space checks. Existing
files are already charged against available space; do not subtract proposed
cleanup until it has actually completed. Check a sparse VM's physical backing
filesystem separately, including its possible additional allocation.

When reclaiming failed attempts, remove only identified obsolete generated
payloads that have no live references and are outside current deployment or
recovery inputs. Hold the deployment coordinator and physical-host action locks.
Retain terminal journals, manifests, cleanup receipts, runtime custody, and the
current artifact set. Repeatedly retaining every large failed upload eventually
exhausts the guest even when the next store fits its logical quota.

Validate the capacity helper without network access:

    python3 -B -m unittest discover -s scripts/tests -p 'taira_disk_capacity_test.py'
