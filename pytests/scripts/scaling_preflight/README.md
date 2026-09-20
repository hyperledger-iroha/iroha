# Fixed scaling preflight

The original protected bootstrap runs the complete inventory after the inner
release runner returns and before `PreparedScalingInputs.prepare` starts the
collector experiment clock. `CompleteScalingPreflight` invokes every unit with
the existing `_run_bounded` process owner, one original absolute deadline and
empty `pass_fds`. Gate and seed descriptors never enter test processes.

`--scaling-preflight-timeout-seconds` defaults to 28800 and accepts 600 through
86400 seconds. It is distinct from the 600-second helper-command policy and is
bound in the original invocation identity and bootstrap marker. The adequacy
of this default requires the full current-source run; focused protocol tests
are not timing or release qualification.

The first six units use `pytests/scripts/run_scaling_preflight.py` and actual
`-I -B -S`: bootstrap29, runtime28, provisioning37, dependency6, acquisition8,
and test_dependencies29. `phase_nodes.json` names every original unittest case
once. The first five receive the exact staged BLAKE3 source; test_dependencies
receives the separately staged pytest source. The phase harness contract itself
is a mandatory pytest suite in the complete inventory.

Each remaining unit runs one exact complete pytest file in a fresh protected
interpreter using `run_scaling_collector_preflight.py`. `inventory.json` binds
current source bytes, all suites and every complete node identity. Parametrized
negative cases can contain multi-megabyte IDs, so pytest node identities are
SHA-256 commitments to their complete UTF-8 bytes. Collection and every actual
setup/call/teardown report must match these commitments. Empty selection,
deselection, missing reports, skips, failures, errors and xfail outcomes reject
the unit. No case is removed because it needs a real process.

The parent retains two original package owners from `scaling_cli_bootstrap`:
`PythonDependencies` for BLAKE3 and `PythonTestDependencies` for the fixed pytest
closure. Each pytest child receives both complete path/inventory commitments
and freshly admits both existing owners. No installed package directory enters
`sys.path`; site processing, conftest/config discovery, inherited pytest options
and plugin autoload are disabled. The child executes source buffers instead of
adjacent bytecode caches.

`migrated_outcomes` plus `pending_outcomes` account for every original historical
assertion identity. The current inventory maps each assertion to its selected
current node. The runtime rejects any unresolved entry or missing observed
outcome; clearing a pending list without its selected mapping cannot satisfy
the contract. Complete execution remains a release qualification requirement.

The inventory binds the exact native budget/admission fixtures, Kura metric
projection and protected invocation specification alongside executable source.
Unselected data paths cannot enter the inventory. Package staging tests copy
the BLAKE3 package already admitted by the fixed driver, with no development
virtual-environment path or ambient package lookup.

Every phase command and result stays with its original parent owner. Wait/drain
errors cannot be called no-spawn or release dependencies before natural terminal
observation. The handoff closes live preflight inputs before its successful
response, preceding runtime pruning. Final bootstrap publication rechecks only
the retained original observations and their durable archive; an archived
success boolean is not execution authority.

The original parent publishes `scaling-preflight/` beneath protected bootstrap
evidence, separately from the pruned runtime and the measured scaling archive.
It preserves the selected inventory, exact child result bytes and full bounded
stdout/stderr for every required unit. Each command record contains its original
argv, environment digest and terminal observation. Fixed historical path roles
are inert data during portable verification; readers never open those paths or
reconstruct a process owner from them.

The parent publishes `index.json` last, then rechecks the original objects,
durable files and original deadline before exposing the mandatory `preflight`
binding in its execution record. A failed final check may leave an index for
diagnosis; it cannot publish a successful binding or release receipt. The
receipt and final marker carry the same compact binding. Its authenticated
timeout and invocation must match the original runner, and preflight completion
must precede the collector's original start.

The flat archive contains four files per required unit plus inventory and index.
Caps are 8 MiB for inventory, 1 MiB for index, and per unit 1 MiB for command,
8 MiB for result and 8 MiB for combined output. The total cap derives from the
current required unit count. Every archive file is a protected, owner-owned,
single-link regular file; extras, aliases, symlinks and altered modes reject.
Receipt creation and terminal replay validate the full selected-source archive
and capture/recheck each member through their publication fences. The standalone
bootstrap inventory validates the complete late top-level group; the protected
receipt reader owns full subtree semantics. Cleanup preserves this disjoint
bootstrap evidence directory.

Native compilation, real process inheritance, actual collector runs, scaling
measurements and portable release authentication remain separate qualifications.
