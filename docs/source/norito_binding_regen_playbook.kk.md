---
lang: kk
direction: ltr
source: docs/source/norito_binding_regen_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 077e4eec193614ce89aa230e0e7501bb1e383b24d9a2864f223f22d51b4177e9
source_last_modified: "2026-01-05T09:28:12.031632+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  Norito binding regeneration playbook covering Python, Java, and Android SDKs.
  Keep translations in sync if this file is localised.
-->

# Norito Binding Regeneration Playbook

Norito is developed in Rust, but the canonical fixtures and SDK bindings in this
repository must stay in lockstep so that Python, Java, and Android clients emit
the exact same payloads. This playbook captures the repeatable steps for
regenerating those bindings whenever `crates/norito`, the data model, or the
fixture exporter change.

- **Cadence:** Follow the twice-weekly Tuesday & Friday (09:00 UTC) rotations
  agreed with Android/Python/Swift maintainers. Emergency runs are allowed when
  a Norito discriminator or ABI hash lands outside that window.
- **SLA:** Publish regenerated artifacts, update the changelog/state files, and
  land parity evidence within 48 h of a required change.
- **State files:** Rotation metadata is written to
  `artifacts/android_fixture_regen_state.json` and
  `artifacts/python_fixture_regen_state.json`. Keep both under version control
  so governance can audit cadence adherence.
- **JDK posture:** Follow the policy captured in
  `docs/source/android_fixture_changelog.md#jdk-upgrade-policy` (JDK 21 LTS +
  quarterly CPU review) so rotations and exporter builds remain deterministic.

## Shared Preparation

1. **Detect drift:** Run the cross-language parity helper from the repo root:
   ```bash
   scripts/check_norito_bindings_sync.sh
   # or
   python3 scripts/check_norito_bindings_sync.py
   ```
   The script flags pending updates under `crates/norito`, `python/norito_py`,
   and `java/norito_java`. It is also executed automatically via
   `ci/check_norito_bindings_sync.sh` and the `norito` crate's `build.rs`.
2. **Update Rust fixtures if needed:** If the change depends on new Norito JSON
   goldens, regenerate them with `scripts/norito_regen.sh` before touching the
   SDKs.
3. **Confirm exporter inputs:** The Android fixture rotation relies on
   `scripts/export_norito_fixtures`. Verify the manifest referenced by
   `scripts/android_fixture_regen.sh` contains the new schemas.
4. **Validate cadence alignment:** Run the cross-SDK cadence checker to confirm
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

## Android (Canonical Fixtures)

Android resources in `java/iroha_android/src/test/resources` act as the
canonical Norito fixture set consumed by the other SDKs.

1. Regenerate fixtures and update the cadence metadata:
   ```bash
   ANDROID_FIXTURE_ROTATION_OWNER="<name>" \
   ANDROID_FIXTURE_CADENCE="twice-weekly-tue-fri-0900utc" \
   make android-fixtures
   ```
   This wraps `scripts/android_fixture_regen.sh`, runs the Rust exporter, and
   refreshes `artifacts/android_fixture_regen_state.json`.
2. Validate the manifest/hash metadata:
   ```bash
   make android-fixtures-check
   # executes scripts/check_android_fixtures.py
   ```
3. Run the full Android test harness (covers the Norito codec, keystore stubs,
   and REST client) to ensure the regenerated payloads stay in sync:
   ```bash
   make android-tests
   # calls ci/run_android_tests.sh
   ```
4. Record the rotation in `docs/source/android_fixture_changelog.md`
   (timestamp, owner, reference commit/PR, and notes).
5. Commit the refreshed fixtures plus
   `artifacts/android_fixture_regen_state.json`. Include any updated generated
   docs under `docs/source/sdk/android/generated/` if the exporter produced new
   summaries.

## Python (`norito_py` and `iroha_python`)

Python fixtures mirror the canonical Android set, and parity is enforced via the
same scripts referenced by CI.

1. Sync fixtures from Android resources:
   ```bash
   PYTHON_FIXTURE_ROTATION_OWNER="<name>" \
   PYTHON_FIXTURE_CADENCE="twice-weekly-tue-fri-0900utc" \
   make python-fixtures
   ```
   This calls `scripts/python_fixture_regen.sh`, copies the `.norito` payloads,
   and updates `artifacts/python_fixture_regen_state.json`.
2. Re-run the parity checks:
   ```bash
   make python-fixtures-check
   # executes scripts/check_python_fixtures.py
   ```
3. Execute the Python SDK test/linters to catch API regressions:
   ```bash
   ./python/iroha_python/scripts/run_checks.sh
   ```
4. Update `python/norito_py` or `python/iroha_python` sources as required,
   keeping `python/norito_py/CHANGELOG.md` and `python/iroha_python/README.md`
   notes in sync with the regenerated fixtures.
5. Commit the refreshed `python/iroha_python/tests/fixtures/` contents,
   `artifacts/python_fixture_regen_state.json`, and any code changes needed to
   satisfy parity.

## Java (`norito_java`)

Java is the second pure-language implementation of Norito. Changes typically
require mirror edits under `java/norito_java` plus the Android library.

1. Apply any schema or codec updates to `java/norito_java/src/main/java`.
2. Run the bundled test harness with assertions enabled:
   ```bash
   (cd java/norito_java && ./run_tests.sh)
   ```
   The script re-compiles the codec and executes round-trip tests covering the
   new schema hash.
3. If the change also touches the Android bindings, re-run
   `make android-tests` after the fixture regeneration step above so
   `ci/run_android_tests.sh` exercises the keystore, HTTP client, and Norito
   serializer together.
4. Update `java/norito_java/CHANGELOG.md` with a short note describing the sync
   point so `scripts/check_norito_bindings_sync.py` records the refresh.

## Final Checklist

Before merging a Norito change that impacts SDK bindings:

1. ✅ Run `scripts/check_norito_bindings_sync.sh` and ensure it passes locally.
2. ✅ Regenerate Android fixtures (`make android-fixtures && make android-fixtures-check`),
   update `docs/source/android_fixture_changelog.md`, and commit the resources +
   `artifacts/android_fixture_regen_state.json`.
3. ✅ Mirror fixtures into Python (`make python-fixtures && make python-fixtures-check`)
   and run `python/iroha_python/scripts/run_checks.sh`; commit the fixture and
   state-file updates.
4. ✅ Rebuild/test `java/norito_java` (and `java/iroha_android` when applicable)
   so Java bindings capture the Rust codec delta.
5. ✅ Ensure CI jobs (`ci/check_norito_bindings_sync.sh`,
   `ci/sdk_sorafs_orchestrator.sh`, Android/Python fixture checks) are green.

Following these steps keeps all SDKs deterministic and gives governance a single
reference detailing who rotated the fixtures, when it happened, and which
artifacts were touched.
