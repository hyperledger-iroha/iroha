---
title: Swift Reproducible Build Checklist
summary: Evidence bundle and command checklist for deterministically rebuilding IrohaSwift and NoritoBridge releases (IOS8).
---

# Swift Reproducible Build Checklist

This checklist gates every Swift SDK release candidate, GA, and hotfix. It satisfies
the IOS8 “publish reproducible builds” requirement by spelling out the artefacts,
commands, and evidence that auditors need to replay the build. Use it alongside the
Iroha 3 release runbook (`specs/release_runbook.md`) and archive the
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
| `IrohaSwift-tests.log` / `IrohaSwift-build.log` | Captured stdout/stderr from release `swift test` and `swift build` commands that use `Package.resolved` with automatic resolution disabled. |
| `NoritoBridge.xcframework.zip` | Built via `make bridge-xcframework`; keep the unzipped directory for local debug but only archive the zip. |
| `NoritoBridge.xcframework.zip.sha256` | `swift package compute-checksum dist/NoritoBridge.xcframework.zip > …/sha256`. |
| `swift_fixture_state.json` | Both sealed owner-publication identities and the tracked-tree `norito-rpc-verify` result proving which canonical fixture snapshot shipped. |
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
- Exact Rust 1.93.1 `cargo`, `rustc`, and `rustdoc` plus the bridge targets:
  `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-apple-darwin`.
- SwiftPM (`swift` CLI), `zipinfo`, exact Python 3.12, `jq`, and `shasum`.
- One pre-created, canonical, non-symbolic, writable `CARGO_TARGET_DIR` outside
  the Iroha source tree. Ordinary builds use the root `Cargo.lock`; the
  authenticated privacy lane uses its distinct frozen external lock selected by
  `IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH`. Arbitrary CLI lock selection is
  rejected.
- Clean workspace (`git status` must be empty) checked out at the release tag.
- Access to the repository's reviewed canonical Norito fixture tree and an
  absent absolute external path for a create-only owner publication.
- Optional: Buildkite metadata access if you are mirroring CI smoke artefacts.

Set helper variables for the session:

```bash
export SWIFT_RELEASE_VERSION="2.1.0"
export SWIFT_RELEASE_DIR="$PWD/artifacts/releases/${SWIFT_RELEASE_VERSION}/swift"
mkdir -p "${SWIFT_RELEASE_DIR}"
export CARGO_TARGET_DIR=/absolute/non-symlink/path/to/iroha-apple-cargo
mkdir -p "$CARGO_TARGET_DIR"
export CARGO_BUILD_JOBS=1
export CARGO_INCREMENTAL=0
export CARGO_NET_OFFLINE=true
export RUSTC_BOOTSTRAP=1
export RUSTC="$(rustup which --toolchain 1.93.1 rustc)"
export RUSTDOC="$(rustup which --toolchain 1.93.1 rustdoc)"
export MOBILE_SDK_PYTHON_BINARY=/absolute/path/to/python3.12
export SOURCE_DATE_EPOCH="$(git show -s --format=%ct HEAD)"
```

## Checklist

| Step | Command(s) | Evidence |
|------|-----------|----------|
| 1. Sync release tag | `git fetch --tags`<br>`git checkout <tag>`<br>`git submodule update --init --recursive` | Record `git status --short` in release ticket to prove a clean tree. |
| 2. Refresh fixtures & parity | Run `cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-fixtures --output-root <absent-absolute-external-root>` at two independent roots; require identical exact path sets, entry types, modes, completion manifests, and every file byte before applying the reviewed identity-relative tracked patch; then run `norito-rpc-verify` and `make swift-fixtures-check` | Record both sealed publication identities and the tracked-tree verification output in `${SWIFT_RELEASE_DIR}/swift_fixture_state.json`. Note any fallback cadence env vars you set. |
| 3. Run Swift tests | `export IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN="$(bash ci/build_offline_cash_swift_fixture.sh --locked --offline)"`<br>`swift test --package-path IrohaSwift --configuration release --disable-automatic-resolution 2>&1 \| tee ${SWIFT_RELEASE_DIR}/IrohaSwift-tests.log` | Log must show `Test Suite 'All tests' passed`, consume the reviewed `Package.resolved`, and execute the same-revision authoritative Offline Cash fixture. |
| 4. Build release bits | `swift build --package-path IrohaSwift --configuration release --disable-automatic-resolution 2>&1 | tee ${SWIFT_RELEASE_DIR}/IrohaSwift-build.log` | Confirms the reviewed resolution builds deterministically before packaging. |
| 5. Build NoritoBridge | `make bridge-xcframework` with the exact environment above (wraps `scripts/build_norito_xcframework.sh`) | Copy `dist/NoritoBridge.xcframework.zip` into the release dir and capture `swift package compute-checksum dist/NoritoBridge.xcframework.zip > ${SWIFT_RELEASE_DIR}/NoritoBridge.xcframework.zip.sha256`. |
| 6. Verify bridge bundling (SPM/Carthage/Pods) | ```bash<br>ls dist/NoritoBridge.xcframework/*/libNoritoBridge.a<br>/usr/bin/unzip -t dist/NoritoBridge.xcframework.zip<br>zipinfo -1 dist/NoritoBridge.xcframework.zip | grep '/libNoritoBridge.a$'<br>swift package --package-path IrohaSwift --disable-automatic-resolution describe --type json \\<br>  | jq '.targets[] | select(.name == "NoritoBridge")' \\<br>  > ${SWIFT_RELEASE_DIR}/NoritoBridge-spm-target.json<br>``` | Attach the archive integrity/inventory output and JSON blob to the release ticket. The Apple artifact workflow additionally copies this exact ZIP into a fresh local SwiftPM package and compiles a `NoritoBridge` consumer. Keep the XCFramework zip adjacent to the repository `dist/` directory before running CocoaPods/Carthage packaging so `ConnectCodec` remains fail-closed in downstream artefacts. |
| 7. Capture dashboards | `make swift-ci` (validates fixtures + dashboards)<br>`cp dashboards/data/mobile_parity.sample.json ${SWIFT_RELEASE_DIR}/mobile_parity.json`<br>`cp dashboards/data/mobile_ci.sample.json ${SWIFT_RELEASE_DIR}/mobile_ci.json` | Keeps the exact feeds that the exporter consumed; auditors can diff them later. |
| 8. Export status bundle | ```bash<br>SWIFT_PARITY_FEED_PATH=${SWIFT_RELEASE_DIR}/mobile_parity.json \\<br>SWIFT_CI_FEED_PATH=${SWIFT_RELEASE_DIR}/mobile_ci.json \\<br>SWIFT_STATUS_EXPORT_OUT=${SWIFT_RELEASE_DIR}/swift_status.md \\<br>SWIFT_STATUS_SUMMARY_OUT=${SWIFT_RELEASE_DIR}/swift_status.json \\<br>SWIFT_STATUS_METRICS_PATH=${SWIFT_RELEASE_DIR}/swift_status.prom \\<br>SWIFT_STATUS_METRICS_STATE=${SWIFT_RELEASE_DIR}/swift_status_state.json \\<br>ci/swift_status_export.sh<br>``` | The markdown summary is pasted into the release ticket; the Prometheus textfile proves parity cadence, success counters, and alert status. The exporter now also copies the readiness doc metadata (repro checklist + support playbook) into the digest/summary by default so reviewers see the evidence without extra uploads. |
| 9. Archive XCFramework smoke logs (if run locally) | `scripts/ci/run_xcframework_smoke.sh 2>&1 | tee ${SWIFT_RELEASE_DIR}/xcframework_smoke_report.txt` with the same exact environment | Copy `artifacts/xcframework_smoke_result.json` when applicable so the IOS6 gate can be replayed. The harness always rebuilds the bridge and fails if prerequisites or packaging are unavailable. |
| 10. Package source snapshot | `git archive --format=tar.gz --prefix=IrohaSwift/ <tag> IrohaSwift > ${SWIFT_RELEASE_DIR}/IrohaSwift-v${SWIFT_RELEASE_VERSION}.tar.gz` | Tarball is signed/hashed with the other artefacts. |
| 11. Generate checksums | ```bash<br>(cd "${SWIFT_RELEASE_DIR}" && \\<br>  shasum -a 256 NoritoBridge.xcframework.zip IrohaSwift-v${SWIFT_RELEASE_VERSION}.tar.gz \\<br>         mobile_parity.json mobile_ci.json swift_status.prom \\<br>         > SHA256SUMS)<br>``` | Attach `SHA256SUMS` + individual `.sha256` files to the release ticket. |
| 12. Update docs & ticket | Link `${SWIFT_RELEASE_DIR}` contents from the release ticket and reference this checklist row-by-row. Update `status.md` with a short summary and cite the evidence path. | Keeps roadmap/status in sync and gives auditors a single location to inspect. |

### Notes

- The owner command is create-only and rejects an existing output root. Use the
  two-root procedure above before recording both identities in the state file.
  SDK-specific archives are not fixture-generation inputs.
- `make bridge-xcframework` invokes the sole archive owner. It authenticates an
  immutable snapshot while holding the shared output lock, recomputes source/tool
  provenance, authenticates Mach-O architectures and native exports, sorts every
  entry, normalizes modes and timestamps from `SOURCE_DATE_EPOCH`, and atomically
  replaces the destination. Never delete the prior archive or invoke `zip`/`ditto`
  manually.
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
