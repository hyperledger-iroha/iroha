# NoritoBridge Release Packaging

This guide covers producing and publishing the authenticated `NoritoBridge`
XCFramework release asset. Swift Package Manager consumes that exact artifact
from an ignored local `dist/` directory or an explicitly configured external
artifact directory. Native CocoaPods delivery is not complete; see
[CocoaPods](#cocoapods) below. The workflow keeps the Swift artifact in
lock-step with the Rust crate releases that ship Iroha's Norito codec. For
end-to-end instructions on consuming a published artifact inside an app, see the
[public Swift SDK tutorial](https://docs.iroha.tech/guide/tutorials/swift.html).

The `.github/workflows/mobile_sdk_artifacts.yml` workflow builds, validates,
packages, and publishes tagged Apple artifacts on macOS. The steps below mirror
that workflow for local release verification.

## Prerequisites

- A macOS host with the latest stable Xcode command line tools installed.
- Exact Rust 1.93.1 `cargo`, `rustc`, and `rustdoc`.
- Python 3.12.
- Swift toolchain 5.9 or newer.
- Access to the Hyperledger Iroha release signing keys for tagging Swift artifacts.

## Versioning model

1. Determine the Rust crate version for the Norito codec (`crates/norito/Cargo.toml`).
2. Tag the workspace with the release identifier (`v<version>`).
3. Keep `IrohaSwift/VERSION`, the Swift loader's expected version, and the
   reviewed release version map aligned.
4. When the Rust crate increments its version, publish a matching authenticated Swift
   artifact.

## Build steps

1. From a clean pinned commit, create dedicated canonical build and artifact
   directories outside the repository and invoke the helper:

   ```bash
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
   export NORITO_BRIDGE_OUT_DIR=/absolute/cache/iroha-apple-artifacts
   export NORITO_BRIDGE_BUILD_DIR=/absolute/cache/iroha-apple-build
   export NORITO_BRIDGE_ARCHIVE_OUTPUT=/absolute/cache/NoritoBridge.xcframework.zip
   mkdir -p \
     "$NORITO_BRIDGE_OUT_DIR" \
     "$NORITO_BRIDGE_BUILD_DIR" \
     "$(dirname "$NORITO_BRIDGE_ARCHIVE_OUTPUT")"
   ./scripts/build_norito_xcframework.sh \
     --privacy-production-enabled \
     --archive-output "$NORITO_BRIDGE_ARCHIVE_OUTPUT"
   ```

   The release command requires a clean dependency-closure source tree, compiles the Rust
   bridge for the iOS device, arm64 and x86_64 iOS simulator, and arm64 and x86_64 macOS targets,
   and writes `$NORITO_BRIDGE_OUT_DIR/NoritoBridge.xcframework`. The canonical manifest is embedded at
   `$NORITO_BRIDGE_OUT_DIR/NoritoBridge.xcframework/NoritoBridge.artifacts.json`; the companion
   `$NORITO_BRIDGE_OUT_DIR/NoritoBridge.artifacts.json` path is a stable relative symlink to that file, so
   one atomic XCFramework exchange publishes the binaries and manifest together. The
   manifest binds exact native bridge ABI 22, the privacy-production feature state,
   source commit and fingerprint, header digest, required-symbol inventory, and
   per-slice SHA-256 hashes. Before publication the helper invokes
   `scripts/check_mobile_sdk_artifacts.sh --apple-only` against the staged generation; a
   checker or `xcodebuild` failure leaves the live generation unchanged. The
   first-release builder has no skip-build, preserved-target, alternate-lock, or
   manual XCFramework fallback mode. Override only the recorded bridge version with
   `--bridge-version <version>` when needed. `--allow-dirty-source` is for local
   integration artifacts and must not be used for a release artifact.
   `scripts/update_norito_bridge_swift_pins.py` has no in-place update mode: use
   `--check` for a read-only repository verification or `--output` with the exact
   `--expected-preimage-sha256` to exclusively create a reviewed projection in an
   external directory. The owner never rewrites `NativeBridge.swift`; incorporate an
   approved projection through the normal guarded source-edit workflow.
   The Cargo target, artifact, build, and archive-parent directories must already
   exist as owned, writable, non-symbolic canonical directories. The archive output
   itself must be absent, and the external build directory used for retained archive
   snapshots must be outside both the repository and archive-parent tree. An existing
   generation is accepted only
   with its embedded manifest and canonical public manifest symlink already in
   the first-release layout; the builder does not migrate an older layout.

2. Confirm the builder-owned archive publication:

   ```bash
   test -f "$NORITO_BRIDGE_ARCHIVE_OUTPUT"
   /usr/bin/unzip -t "$NORITO_BRIDGE_ARCHIVE_OUTPUT"
   zipinfo -1 "$NORITO_BRIDGE_ARCHIVE_OUTPUT"
   ```

   Before releasing its authenticated artifact-publication lock, the builder invokes
   the sole archive owner on the generation it just published. The owner retains a
   unique source snapshot and re-authenticates the exact ABI-22 inventory,
   recomputes source and tool provenance, verifies each Mach-O architecture and the
   required/forbidden export policy with the sealed Xcode toolchain, sorts entries,
   stores them without host-zlib variance, normalizes modes and ZIP timestamps from
   `SOURCE_DATE_EPOCH`, and fsyncs a temporary archive while retaining its open file
   descriptor and authenticated inode identity. Publication uses one atomic
   no-replace rename. The owner never creates or removes an archive-destination lock,
   and any pre-existing or concurrently created destination is rejected and left
   untouched. Failed runs retain their uniquely named snapshot and archive residue
   for inspection instead of deleting a path that another process may have swapped.
   A concurrent builder or archiver is rejected; do not invoke `ditto` or `zip`
   directly. CI also feeds the published ZIP to a fresh local SwiftPM binary target
   and compiles a consumer against `NoritoBridge`.

3. Authenticate the archive against its embedded manifest and retain the signed
   release evidence outside the source tree. The checked-in Swift package uses
   the authenticated external artifact directory and does not embed generated
   release archives or their checksums.

4. Update `IrohaSwift/IrohaSwift.podspec` with the new version, checksum, and archive
   URL.

5. **Regenerate headers if the bridge gained new exports.** The Swift bridge now exposes
   `connect_norito_set_acceleration_config` so `AccelerationSettings` can toggle Metal /
   GPU backends. Ensure `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h`
   matches `crates/connect_norito_bridge/include/connect_norito_bridge.h` before zipping.

6. Run the Swift validation suite before tagging:

   ```bash
   swift test --package-path IrohaSwift --disable-automatic-resolution
   make swift-ci
   ```

   The first command ensures the Swift package (including `AccelerationSettings`) stays
   green; the second validates fixture parity, renders the parity/CI dashboards, and
   exercises the same telemetry checks enforced in Buildkite (including the
   `ci/xcframework-smoke:<lane>:device_tag` metadata requirement).

7. Record the signed publication evidence and tag the reviewed source commit.
   Generated `dist/*` outputs remain untracked; only `dist/.gitkeep` belongs in Git.

## Publishing

### Swift Package Manager

The checked-in package manifest uses a path-based binary target. Before package
resolution, materialize the verified release asset either under the ignored
repository `dist/` path or in a canonical external directory selected with
`MOBILE_SDK_APPLE_ARTIFACT_DIR`. Reviewed builds additionally set
`MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1`. A Git tag by itself does not
materialize the XCFramework and must not be reported as an installed native
package.

### CocoaPods

Native CocoaPods publication remains blocked. The current podspec does not yet
define an authenticated vendored-XCFramework archive path. The lint wrapper now
fails when CocoaPods is unavailable, but a local lint does not close artifact
delivery. Do not run `pod trunk push` or claim native CocoaPods readiness until
the vendored artifact design, install smoke, and signed provenance are
implemented and reviewed.

## CI considerations

- `.github/workflows/mobile_sdk_artifacts.yml` runs the packaging and artifact
  checkers on `macos-14`, uploads the generated archives, and publishes release
  assets for `v*` tags.
- `ci/check_swift_samples.sh` gates the Swift demos against the freshly
  generated framework.
- Release logs and artifact manifests retain the source fingerprint, ABI,
  feature state, required-symbol inventory, and per-slice hashes.
