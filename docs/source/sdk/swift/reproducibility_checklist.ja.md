---
lang: ja
direction: ltr
source: docs/source/sdk/swift/reproducibility_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3d8d1f1218ade1fd6e850049b9d98c56d34efae086bd346a429eb818c763efee
source_last_modified: "2026-01-03T18:08:01.424254+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Swift Reproducible Build Checklist
summary: Evidence bundle and command checklist for deterministically rebuilding IrohaSwift and NoritoBridge releases (IOS8).
---

# Swift Reproducible Build Checklist

This checklist gates every Swift SDK release candidate, GA, and hotfix. It satisfies
the IOS8 “publish reproducible builds” requirement by spelling out the artefacts,
commands, and evidence that auditors need to replay the build. Use it alongside the
dual-track release runbook (`docs/source/release_dual_track_runbook.md`) and archive the
outputs under `artifacts/releases/<version>/swift/`.

## Scope & Deliverables

Run the checklist when:

- Tagging a new Swift SDK or NoritoBridge release (RC/GA/hotfix).
- Refreshing artefacts after a security fix.
- Re-running release evidence for an audit or governance request.

### Evidence bundle layout

Create `artifacts/releases/<version>/swift/` and populate it with:

| File | Notes |
|------|-------|
| `IrohaSwift-v<version>.tar.gz` | `git archive --format=tar.gz --prefix IrohaSwift/ <tag> IrohaSwift`. |
| `IrohaSwift-tests.log` / `IrohaSwift-build.log` | Captured stdout/stderr from `swift test --configuration release` and `swift build --configuration release`. |
| `NoritoBridge.xcframework.zip` | Built via `make bridge-xcframework`; keep the unzipped directory for local debug but only archive the zip. |
| `NoritoBridge.xcframework.zip.sha256` | `swift package compute-checksum dist/NoritoBridge.xcframework.zip > …/sha256`. |
| `swift_fixture_state.json` | Copy of `artifacts/swift_fixture_regen_state.json` proving which canonical fixture snapshot shipped. |
| `mobile_parity.json` / `mobile_ci.json` | Feeds produced by `make swift-ci` or pulled from CI; use them as the source of truth for dashboards. |
| `swift_status.md` / `swift_status.json` | Output of `ci/swift_status_export.sh` (use env vars below to write into the release directory). |
| `swift_status.prom` / `swift_status_state.json` | Prometheus textfile + persistent counter state emitted by the exporter (`SWIFT_STATUS_METRICS_PATH`, `SWIFT_STATUS_METRICS_STATE`). |
| `SHA256SUMS` | Combined checksums covering the tarball, XCFramework zip, prom file, and dashboard feeds. |
| `xcframework_smoke_report.txt` / `xcframework_smoke_result.json` | Optional when the Buildkite smoke job ran out-of-band; copy the artefacts for reproducibility. |

Document any deviations (e.g., simulator fallback, manual fixture slot) in a short
`README.txt` inside the same directory.

## Prerequisites

- macOS host with the release-approved Xcode toolchain (>= 15.3 at the time of writing);
  run `xcodebuild -version` and record it in the release issue.
- Rust toolchain from `rust-toolchain.toml` plus the bridge targets:
  `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-apple-darwin`.
- SwiftPM (`swift` CLI), `zip`, `zipinfo`, `python3`, `jq`, and `shasum`.
- Clean workspace (`git status` must be empty) checked out at the release tag.
- Access to the Norito fixtures source (Android canonical directory or signed archive).
- Optional: Buildkite metadata access if you are mirroring CI smoke artefacts.

Set helper variables for the session:

```bash
export SWIFT_RELEASE_VERSION="2.1.0"
export SWIFT_RELEASE_DIR="$PWD/artifacts/releases/${SWIFT_RELEASE_VERSION}/swift"
mkdir -p "${SWIFT_RELEASE_DIR}"
```

## Checklist

| Step | Command(s) | Evidence |
|------|-----------|----------|
| 1. Sync release tag | `git fetch --tags`<br>`git checkout <tag>`<br>`git submodule update --init --recursive` | Record `git status --short` in release ticket to prove a clean tree. |
| 2. Refresh fixtures & parity | `make swift-fixtures`<br>`make swift-fixtures-check` | Copy `artifacts/swift_fixture_regen_state.json` to `${SWIFT_RELEASE_DIR}/swift_fixture_state.json`. Note any fallback cadence env vars you set. |
| 3. Run Swift tests | `swift test --configuration release --package-path IrohaSwift 2>&1 | tee ${SWIFT_RELEASE_DIR}/IrohaSwift-tests.log` | Log must show `Test Suite 'All tests' passed`. |
| 4. Build release bits | `swift build --configuration release --package-path IrohaSwift 2>&1 | tee ${SWIFT_RELEASE_DIR}/IrohaSwift-build.log` | Confirms SPM resolves deterministically before packaging. |
| 5. Build NoritoBridge | `make bridge-xcframework` (wraps `scripts/build_norito_xcframework.sh`) | Copy `dist/NoritoBridge.xcframework.zip` into the release dir and capture `swift package compute-checksum dist/NoritoBridge.xcframework.zip > ${SWIFT_RELEASE_DIR}/NoritoBridge.xcframework.zip.sha256`. |
| 6. Verify bridge bundling (SPM/Carthage/Pods) | ```bash<br>ls dist/NoritoBridge.xcframework/*/NoritoBridge.framework/NoritoBridge<br>zipinfo -1 dist/NoritoBridge.xcframework.zip | grep NoritoBridge.framework/NoritoBridge<br>swift package describe --type json --package-path IrohaSwift \\<br>  | jq '.targets[] | select(.name == "NoritoBridge")' \\<br>  > ${SWIFT_RELEASE_DIR}/NoritoBridge-spm-target.json<br>``` | Attach the `ls`/`zipinfo` output and the JSON blob to the release ticket to prove every slice ships. Keep the XCFramework zip adjacent to the repository `dist/` directory before running CocoaPods/Carthage packaging so `ConnectCodec` remains fail-closed in downstream artefacts. |
| 7. Capture dashboards | `make swift-ci` (validates fixtures + dashboards)<br>`cp dashboards/data/mobile_parity.sample.json ${SWIFT_RELEASE_DIR}/mobile_parity.json`<br>`cp dashboards/data/mobile_ci.sample.json ${SWIFT_RELEASE_DIR}/mobile_ci.json` | Keeps the exact feeds that the exporter consumed; auditors can diff them later. |
| 8. Export status bundle | ```bash<br>SWIFT_PARITY_FEED_PATH=${SWIFT_RELEASE_DIR}/mobile_parity.json \\<br>SWIFT_CI_FEED_PATH=${SWIFT_RELEASE_DIR}/mobile_ci.json \\<br>SWIFT_STATUS_EXPORT_OUT=${SWIFT_RELEASE_DIR}/swift_status.md \\<br>SWIFT_STATUS_SUMMARY_OUT=${SWIFT_RELEASE_DIR}/swift_status.json \\<br>SWIFT_STATUS_METRICS_PATH=${SWIFT_RELEASE_DIR}/swift_status.prom \\<br>SWIFT_STATUS_METRICS_STATE=${SWIFT_RELEASE_DIR}/swift_status_state.json \\<br>ci/swift_status_export.sh<br>``` | The markdown summary is pasted into the release ticket; the Prometheus textfile proves parity cadence, success counters, and alert status. The exporter now also copies the readiness doc metadata (repro checklist + support playbook) into the digest/summary by default so reviewers see the evidence without extra uploads. |
| 9. Archive XCFramework smoke logs (if run locally) | `scripts/ci/run_xcframework_smoke.sh 2>&1 | tee ${SWIFT_RELEASE_DIR}/xcframework_smoke_report.txt` | Copy `artifacts/xcframework_smoke_result.json` when applicable so the IOS6 gate can be replayed. |
| 10. Package source snapshot | `git archive --format=tar.gz --prefix=IrohaSwift/ <tag> IrohaSwift > ${SWIFT_RELEASE_DIR}/IrohaSwift-v${SWIFT_RELEASE_VERSION}.tar.gz` | Tarball is signed/hashed with the other artefacts. |
| 11. Generate checksums | ```bash<br>(cd "${SWIFT_RELEASE_DIR}" && \\<br>  shasum -a 256 NoritoBridge.xcframework.zip IrohaSwift-v${SWIFT_RELEASE_VERSION}.tar.gz \\<br>         mobile_parity.json mobile_ci.json swift_status.prom \\<br>         > SHA256SUMS)<br>``` | Attach `SHA256SUMS` + individual `.sha256` files to the release ticket. |
| 12. Update docs & ticket | Link `${SWIFT_RELEASE_DIR}` contents from the release ticket and reference this checklist row-by-row. Update `status.md` with a short summary and cite the evidence path. | Keeps roadmap/status in sync and gives auditors a single location to inspect. |

### Notes

- `make swift-fixtures` respects the cadence metadata recorded in
  `artifacts/swift_fixture_regen_state.json`. When consuming a signed archive set
  `SWIFT_FIXTURE_ARCHIVE=/path/to/archive.tar.gz` so the provenance hash is included in
  the state file.
- `make bridge-xcframework` already strips timestamps (`zip -X`), ensuring zip entries
  are deterministic. If you rerun the command, delete the destination zip first or the
  `zip` command will append.
- If you must rerun `ci/swift_status_export.sh`, reuse the same parity/CI JSON files and
  `swift_status_state.json` so counters remain monotonic.
- Store large artefacts (XCFramework zip, tarball) in LFS or an external bucket if the
  release issue cannot host them directly, but always keep hashes + logs in
  `artifacts/releases/<version>/swift/` for parity with the rest of the release pipeline.
- Keep the NoritoBridge zip checked into the release bundle you hand to SwiftPM, CocoaPods,
  and Carthage consumers—without it `ConnectCodec` will now fail closed (no JSON fallback),
  so missing artefacts immediately surface as install-time errors instead of silent drift.

Following the steps above yields a fully reproducible Swift SDK evidence bundle that the
release manager can reference from the roadmap and `status.md`, closing the IOS8 “build
reproducibility checklist” action item.
