# NoritoBridge Release Packaging

This guide covers producing and publishing the authenticated `NoritoBridge`
XCFramework release asset. Swift Package Manager consumes that exact artifact
from an ignored local `dist/` directory or an explicitly configured external
artifact directory. CocoaPods consumes the same ZIP through the generated,
checksum-pinned `NoritoBridge` binary pod. `IrohaSwift/VERSION` owns the shared
source-pod, binary-pod, tag, and archive SemVer. The Rust sources are instead
bound by the reviewed commit, source fingerprint, and selected authenticated
lockfile. Ordinary builds use the root lock; the privacy lane uses the distinct
frozen lock selected by `IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH`. For
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
- CocoaPods for the package-first binary/source lint.
- Access to the Hyperledger Iroha release signing keys for tagging Swift artifacts.

## Versioning model

1. Select the canonical pod/archive SemVer in `IrohaSwift/VERSION`.
2. Tag the workspace with that release identifier (`v<version>`). This one tag
   owns the IrohaSwift source pod and the NoritoBridge binary release asset.
3. Keep `IrohaSwift/VERSION`, the Swift loader's expected version, and the
   reviewed release version map aligned.
4. Do not require numeric equality with `crates/norito/Cargo.toml`; authenticate
   the exact Rust source commit, source fingerprint, and selected authenticated
   lockfile instead.

## Build steps

1. From a clean pinned commit, create dedicated canonical build and artifact
   directories outside the repository and invoke the helper:

   ```bash
   export CARGO_TARGET_DIR=/release/apple-a-cargo
   mkdir -p "$CARGO_TARGET_DIR"
   export CARGO_BUILD_JOBS=1
   export CARGO_INCREMENTAL=0
   export CARGO_NET_OFFLINE=true
   export RUSTC_BOOTSTRAP=1
   export RUSTC="$(rustup which --toolchain 1.93.1 rustc)"
   export RUSTDOC="$(rustup which --toolchain 1.93.1 rustdoc)"
   export MOBILE_SDK_PYTHON_BINARY=/absolute/path/to/python3.12
   export SOURCE_DATE_EPOCH="$(git show -s --format=%ct HEAD)"
   export NORITO_BRIDGE_OUT_DIR=/release/apple-a-artifacts
   export NORITO_BRIDGE_BUILD_DIR=/release/apple-a-build
   export NORITO_BRIDGE_ARCHIVE_OUTPUT=/release/apple-a-archive/NoritoBridge.xcframework.zip
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
   first-release builder has no skip-build, preserved-target, arbitrary CLI
   alternate-lock, or manual XCFramework fallback mode. The ordinary lane uses
   the root lock; the authenticated privacy lane may select its frozen external
   `Cargo.lock` only through `IROHA_PRIVACY_RELEASE_CARGO_LOCKFILE_PATH`.
   Override only the recorded bridge version with
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

   The hosted Apple CI lane uses the same owner in an explicit five-job build
   matrix because one release build can consume nearly all memory on a standard
   macOS runner. Each job selects one closed slice ID with `--produce-slice` and
   writes exactly one library plus `slice-evidence.json` beneath a fresh
   `--slice-output-root`. The evidence binds the workflow run/attempt, source and
   lock seals, Rust and Xcode identities, SDK and deployment target, feature
   state, architecture, symbols, size, and library digest. The verifier job
   downloads the five immutable same-run artifacts into one fresh root and
   invokes `--assemble-slices`; assembly recomputes the complete local envelope,
   requires the exact five-directory/two-file inventory, and authenticates every
   byte before lipo or XCFramework publication. These closed producer/assembler
   modes are not a prebuilt-library fallback and cannot accept caller-selected
   target tuples.

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

4. Select a canonical existing external parent and an absent dedicated package
   destination whose basename contains `mobile-sdk`, then package and lint the
   final archive before tagging. `--version` is a diagnostic artifact label; the
   pod/archive SemVer still comes only from `IrohaSwift/VERSION`.

   ```bash
   export MOBILE_SDK_APPLE_ARTIFACT_DIR="$NORITO_BRIDGE_OUT_DIR"
   export MOBILE_SDK_PACKAGE_PARENT=/absolute/cache/iroha-mobile-packages
   mkdir -p "$MOBILE_SDK_PACKAGE_PARENT"
   export MOBILE_SDK_PACKAGE_OUT_DIR="$MOBILE_SDK_PACKAGE_PARENT/mobile-sdk-release"
   test ! -e "$MOBILE_SDK_PACKAGE_OUT_DIR"
   export MOBILE_SDK_VERSION="local-$(git rev-parse --short=12 HEAD)"
   scripts/package_mobile_sdk_artifacts.sh \
     --apple \
     --version "$MOBILE_SDK_VERSION"
   ci/check_swift_pod_bridge.sh
   ```

   The package command creates
   `NoritoBridge-v<version>.xcframework.zip` and invokes
   `scripts/render_norito_bridge_podspec.py`, which reads `IrohaSwift/VERSION`,
   requires the embedded bridge-manifest version to match it, validates the
   bounded deterministic ZIP, computes its SHA-256, and exclusively creates
   `NoritoBridge-<version>.podspec` in the private package stage. The lint wrapper
   reauthenticates that final package before compiling both pods. Do not hand-edit
   a release URL or checksum, and do not place generated package outputs in Git.

5. **Regenerate headers if the bridge gained new exports.** The Swift bridge now exposes
   `connect_norito_set_acceleration_config` so `AccelerationSettings` can toggle Metal /
   GPU backends. Ensure `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h`
   matches `crates/connect_norito_bridge/include/connect_norito_bridge.h` before zipping.

6. Run the Swift validation suite before tagging:

   ```bash
   export IROHA_KOTLIN_OFFLINE_CASH_FIXTURE_BIN="$(
     bash ci/build_offline_cash_swift_fixture.sh --locked
   )"
   swift test --package-path IrohaSwift --disable-automatic-resolution
   make swift-ci
   ```

   The built executable is the same-revision authority for the closed Offline Cash
   40-row parity inventory; the Swift suite fails instead of skipping if it is absent
   or invalid. The `swift test` command ensures the package (including
   `AccelerationSettings`) stays green; `make swift-ci` validates fixture parity,
   renders the parity/CI dashboards, and exercises the same telemetry checks enforced
   in Buildkite (including the `ci/xcframework-smoke:<lane>:device_tag` metadata
   requirement).

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

`IrohaSwift` is a source pod with an exact same-version dependency on the
`NoritoBridge` binary pod. The generated binary podspec uses the immutable
`v<version>` GitHub release URL, pins the exact ZIP with CocoaPods `:sha256`, and
declares `NoritoBridge.xcframework` as its vendored framework. CI first packages
the final archive into an atomically published current-UID-owned mode-0700
directory, requires single-link regular inputs that are not writable by others,
authenticates the closed checksum/manifest inventory, renders
an explicit `file://` copy into a current-UID-owned mode-0700 temporary directory,
then runs binary `pod spec lint` and source `pod lib lint --include-podspecs`.
The dependency archive is package-local, but CocoaPods itself may still consult
configured spec sources; this lane is not evidence of network isolation.

This closes repository source wiring and package-local dependency compilation only.
CocoaPods registry publication remains blocked until the immutable GitHub asset
and both same-version specs are published in dependency order and a clean public
`pod install`/Release build plus signed provenance are captured. Generated
`dist/*` stays untracked; only `dist/.gitkeep` belongs in Git.

## CI considerations

- `.github/workflows/mobile_sdk_artifacts.yml` runs the packaging and artifact
  checkers on `macos-14`, uploads the generated archives, and publishes release
  assets for `v*` tags.
- `ci/check_swift_samples.sh` gates the Swift demos against the freshly
  generated framework.
- Release logs and artifact manifests retain the source fingerprint, ABI,
  feature state, required-symbol inventory, and per-slice hashes.
