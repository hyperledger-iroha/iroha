# N=3 positive smoke gate

`private_settlement_smoke_campaign.py` runs the required ten fresh correctness
runs before a release measurement campaign. Each invocation starts four global
validators and three disjoint four-validator participant committees, using the
feature-isolated Rust entrypoint
`nexus::atomic_private_settlement_localnet::atomic_private_settlement_n3_real_process_smoke`.
The source-coupled protocol lives in
`integration_tests/tests/nexus/atomic_private_settlement_localnet.rs` and
`atomic_private_settlement_real_process_harness.rs` in the same directory.

The source checkout must be clean at the exact signed commit. The driver checks
`git verify-commit` and each signed blob's actual bytes, including changes hidden
by Git index flags. Cargo home must be absolute and canonical, and Cargo
configuration cannot traverse symlinks. Unsigned Cargo configuration in the checkout, ancestor
directories, or Cargo home is rejected. Use a new absolute target directory and
a new absolute evidence directory outside the repository. Both release builds
use locked offline dependencies, four build jobs, no incremental artifacts and
no sccache reuse. The installed compiler, dependency cache, linker and system
libraries remain trusted build-environment prerequisites; retained compiler and
Cargo version logs do not constitute hermetic toolchain qualification.

```sh
python3 scripts/private_settlement_smoke_campaign.py run \
  --repo /absolute/path/to/signed-checkout \
  --commit FULL_SIGNED_COMMIT \
  --target-dir /absolute/path/to/new-smoke-target \
  --output /absolute/path/outside-the-repo/new-smoke-evidence

python3 scripts/private_settlement_smoke_campaign.py validate \
  /absolute/path/outside-the-repo/new-smoke-evidence --commit FULL_SIGNED_COMMIT
```

The driver builds `iroha3d` with `test-network-message-control` and the grouped
integration test with `atomic-private-settlement-release` once, discovers the
exact ignored smoke test, then executes ten serial fresh requests. It requires
one executed passing test, zero failures and zero ignored tests from every
invocation. `IROHA_TEST_REQUIRE_NETWORK=1` and
`IROHA_TEST_NETWORK_START_ATTEMPTS=1` are set explicitly. The driver does not
retry or count skipped networks as success. A failed command, state invariant,
source/binary check, or evidence check ends the campaign; the failed directory
is retained and cannot validate as a passing campaign.

Each request has exactly `version`, `protocol`, `kind`, `request_id`,
`invocation_nonce`, `commit`, `seed`, and `run`. The version is 1, protocol is
`AtomicPrivateSettlementV1`, kind is `smoke`, and run is in `0..9`. The request ID
is SHA-256 of canonical compact JSON of the other seven fields. Request IDs,
256-bit invocation nonces and 64-bit seeds must be distinct across the campaign;
network, bundle, and all validator identities must also be fresh in every run.

Each owner-only `run-NN` directory contains the request, raw command log,
terminal command receipt, Rust result, source/binary seals before and after
execution, and an initially empty owner-only `evidence` directory. The Rust
result binds an exact 80-file inventory with lengths and SHA-256 digests:

- The bound request, before/after process inventories and all 16 restart records.
- Three participant authorities, the all-Prepare barrier, Commit certificates,
  and the three-leg finalized receipt.
- All 16 raw financial responses at `before`, `collecting`, `audited`, `prepared`,
  `registered`, `commit-certified`, `finalized`, and `replay` stages, plus an
  all-16 state snapshot after each of the 16 sequential restarts.
- Sixteen continuous raw observation streams, bound to their exact phase
  attempts, classifications, counts, checkpoint coverage and hash chains.
- Sixteen full `BridgeFinalityProof` files before restart and sixteen afterward.

Validation checks the 300-height activation notice, disjoint four-member
committees, all 16 distinct process IDs and configuration commitments, changed
process IDs for every restart, and the final atomic financial deltas (three
roots, six nullifiers, nine commitments/encrypted outputs, one replay marker and
one receipt). It rejects intermediate partial application and rollback,
including a mismatch between retained responses and continuous observer
summaries. Replay and every restart must retain the exact finalized financial
state and signed finality decision. Observation/file bounds are strict;
exceeding them fails and retains the run instead of truncating observations.

The Rust client verifies canonical finality and BLS signatures before writing
the genuine proof objects. Python checks structural and cross-record bindings,
including the exact global roster, signed RS16 layout, finalized height and
semantic height context. Protocol hashes must retain Iroha's low marker bit;
the reserved empty Prepare digest is `00…01`, and Commit must bind a nonempty
Prepare barrier. It does not independently verify BLS or recompute
Norito protocol hashes. Independent cryptographic review remains a separate
release requirement.

The read-only `validate_campaign(path, expected_commit=...)` API rechecks every
artifact plus the retained signed checkout and executable bytes. Keep those
files available and unchanged while using the gate as a release prerequisite.
Synthetic unit tests are in
`scripts/tests/private_settlement_smoke_campaign_test.py`; they never start a
network or create measured release evidence.
