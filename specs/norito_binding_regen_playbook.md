<!--
  Norito binding regeneration playbook covering Python, Java, Android, and Swift SDKs.
-->

# Norito Binding Regeneration Playbook

Norito is developed in Rust, but the canonical fixtures and SDK bindings in this
repository must stay in lockstep. `fixtures/norito_rpc/` is the sole fixture
source. The owner command publishes that corpus and its managed mirrors together:

```bash
cargo run --locked -p xtask --features dev-tools --bin xtask -- \
  norito-rpc-fixtures --output-root /path/to/first-new-norito-rpc-publication
cargo run --locked -p xtask --features dev-tools --bin xtask -- \
  norito-rpc-fixtures --output-root /path/to/second-new-norito-rpc-publication
```

Both external roots must be absent. Before any tracked update, require identical
exact path sets, entry types, modes, completion manifests, and every file byte.
Apply the reviewed identity-relative patch from either sealed root, then run
`norito-rpc-verify` and the consumer checks.

Python and Swift receive descriptor-only mirrors. Java receives a generated
descriptor-and-blob mirror. None of those SDK directories is an input or an
independent fixture owner.

- **Cadence:** Follow the twice-weekly Tuesday & Friday (09:00 UTC) rotations
  agreed with Android/Python/Swift maintainers. Emergency runs are allowed when
  a Norito discriminator or ABI hash lands outside that window.
- **SLA:** Publish the regenerated corpus and mirrors and land parity evidence
  within 48 h of a required change.
- **JDK posture:** Follow the policy captured in
  `specs/android_fixture_changelog.md#jdk-upgrade-policy` (JDK 21 LTS +
  quarterly CPU review) so Java mirror validation remains deterministic.

## Shared Preparation

1. **Detect drift:** Run the cross-language parity helper from the repo root:
   ```bash
   scripts/check_norito_bindings_sync.sh
   # or
   python3 scripts/check_norito_bindings_sync.py
   ```
   The script flags pending updates under `crates/norito`, `python/norito_py`,
   `java/norito_java`, and `kotlin/core-jvm`. CI executes it via
   `ci/check_norito_bindings_sync.sh`. Ordinary Cargo builds skip this
   multi-SDK guard; set `NORITO_CHECK_BINDINGS_SYNC=1` only when you explicitly
   want `cargo build -p norito` to run the same check locally.
2. **Regenerate from the canonical descriptor:** Update
   `fixtures/norito_rpc/transaction_payloads.json` when an authoritative fixture
   descriptor changes, then run the owner command shown above. It regenerates
   canonical payload blobs, the manifest, schema and compact-hash data, and all
   managed SDK mirrors. Do not run a language-specific generator first.
3. **Validate cadence alignment:** Run the cross-SDK cadence checker to confirm
   the latest Android/Python (and optional Swift/JS) rotations happened within
   the agreed skew/age limits before asking governance to sign off:
   ```bash
   python3 scripts/check_fixture_cadence.py \
     --platform android \
     --platform python \
     --max-age-hours 72 \
     --max-skew-hours 6 \
     --json-out artifacts/fixtures/cadence_report.json
   ```
   The helper reads `artifacts/*_fixture_regen_state.json`, enforces the shared
   Tue/Fri cadence, and emits a JSON summary so the roadmap item “Align Norito
   fixture regeneration cron (Android & Python maintainers)” has determinism
   evidence ready for governance reviews.

## Canonical Norito RPC fixtures

The authoritative inputs and outputs live under `fixtures/norito_rpc/`.
`transaction_payloads.json` is the structured descriptor input; the same owner
regenerates `transaction_fixtures.manifest.json`, `schema_hashes.json`, the
compact-hash vector, and every owned `.norito` payload.

This shared corpus deliberately pins `TransactionAdmissionIntent::Ordinary` to
exercise the general codec and internal-transaction wire form. It is not a
public Torii-submission golden: externally signed public SDK transactions and
their SDK-owned fixtures must instead bind `QueuePlanSynced` before signing.

1. Regenerate two complete publications:
   ```bash
   cargo run --locked -p xtask --features dev-tools --bin xtask -- \
     norito-rpc-fixtures --output-root /path/to/first-new-norito-rpc-publication
   cargo run --locked -p xtask --features dev-tools --bin xtask -- \
     norito-rpc-fixtures --output-root /path/to/second-new-norito-rpc-publication
   ```
   Both external roots are create-only and must not already exist. Require
   identical exact path sets, entry types, modes, completion manifests, and
   every file byte before applying the reviewed identity-relative tracked patch.
   There are no SDK-specific regeneration delegates or alternate modes.
2. Validate the reviewed canonical and mirror parity:
   ```bash
   cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-verify
   scripts/check_norito_bindings_sync.sh
   ```
3. Review the canonical and generated mirror diffs as one publication. Never
   hand-edit a generated manifest, hash, or SDK mirror to make a check pass.

## Python (`norito_py` and `iroha_python`)

`python/iroha_python/tests/fixtures/` mirrors only
`transaction_payloads.json` and `transaction_fixtures.manifest.json` from the
canonical directory. Canonical `.norito` blobs are deliberately absent.

1. Run the canonical owner command with a new `--output-root`.
2. Re-run descriptor parity checks:
   ```bash
   python3 scripts/check_python_fixtures.py
   ```
3. Execute the Python SDK test/linters to catch API regressions:
   ```bash
   ./python/iroha_python/scripts/run_checks.sh
   ```
4. Update `python/norito_py` or `python/iroha_python` sources as required,
   keeping `python/norito_py/CHANGELOG.md` and `python/iroha_python/README.md`
   notes in sync with the regenerated fixtures.
5. Commit the refreshed descriptor pair with its canonical publication and any
   code changes needed to satisfy parity.

## Swift (`IrohaSwift`)

`IrohaSwift/Fixtures/` is also descriptor-only. The canonical owner writes the
payload descriptor and manifest there while preserving Swift-owned fixtures;
canonical `.norito` blobs must not be copied into this directory.

1. Run the canonical owner command with a new `--output-root`.
2. Verify the descriptor mirror:
   ```bash
   python3 scripts/check_swift_fixtures.py
   ```
3. Run the Swift parity suite with the required native bridge according to the
   Swift SDK playbook.

## Java (`norito_java`)

Java is the second pure-language implementation of Norito. Changes typically
require codec edits under `java/norito_java` plus the Android library. Fixture
resources under `java/iroha_android/src/test/resources/` are generated outputs:
the canonical owner publishes both descriptors and canonical `.norito` blobs
there. Never use this directory as a regeneration input.

1. Apply any schema or codec updates to `java/norito_java/src/main/java`.
2. Run the bundled test harness with assertions enabled:
   ```bash
   (cd java/norito_java && ./run_tests.sh)
   ```
   The script re-compiles the codec and executes round-trip tests covering the
   new schema hash.
3. If the change also touches the Android bindings, re-run
   `make android-tests` after the canonical fixture regeneration step so
   `ci/run_android_tests.sh` exercises the keystore, HTTP client, and Norito
   serializer together.
4. Update `java/norito_java/CHANGELOG.md` with a short note describing the sync
   point so `scripts/check_norito_bindings_sync.py` records the refresh.

## Final Checklist

Before merging a Norito change that impacts SDK bindings:

1. ✅ Run `scripts/check_norito_bindings_sync.sh` and ensure it passes locally.
2. ✅ Run `cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-fixtures --output-root /path/to/first-new-norito-rpc-publication`, then repeat with a second absent root. Require identical exact path sets, entry types, modes, completion manifests, and every file byte before applying the reviewed identity-relative patch to the canonical, Java, Python, and Swift tracked paths.
3. ✅ Verify the Python and Swift descriptor-only mirrors, the Java generated
   mirror, and the canonical Norito RPC publication; run the SDK test suites.
4. ✅ Rebuild/test `java/norito_java` (and `java/iroha_android` when applicable)
   so Java bindings capture the Rust codec delta.
5. ✅ Ensure CI jobs (`ci/check_norito_bindings_sync.sh`,
   `ci/sdk_sorafs_orchestrator.sh`, Android/Python fixture checks) are green.

Following these steps keeps all SDKs deterministic and gives governance a single
reference detailing who rotated the fixtures, when it happened, and which
artifacts were touched.
