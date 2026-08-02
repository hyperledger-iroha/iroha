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

- A macOS host with the reviewed Xcode command line tools installed.
- An absolute canonical Python 3.12 executable.
- The exact Rust toolchain pinned by `rust-toolchain.toml` (currently 1.93.1).
- The `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `x86_64-apple-ios`,
  `aarch64-apple-darwin`, and `x86_64-apple-darwin` Rust targets.
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
   cargo fetch --locked
   mkdir -p /absolute/path/apple-artifacts /absolute/path/apple-build
   MOBILE_SDK_PYTHON_BINARY=/absolute/path/python3.12 \
   NORITO_BRIDGE_OUT_DIR=/absolute/path/apple-artifacts \
   NORITO_BRIDGE_BUILD_DIR=/absolute/path/apple-build \
   NORITO_BRIDGE_PRESERVE_CARGO_TARGETS=1 \
     ./scripts/build_norito_xcframework.sh --privacy-production-enabled
   ```

   The release command requires a clean dependency-closure source tree, compiles
   the Rust bridge for the iOS device plus arm64 and x86_64 variants of both the
   iOS simulator and macOS, and writes the XCFramework under the selected
   artifact directory. The
   architecture-specific libraries are combined into the canonical
   `ios-arm64_x86_64-simulator` and `macos-arm64_x86_64` slices. The manifest
   is embedded at `NoritoBridge.xcframework/NoritoBridge.artifacts.json`; the
   companion `NoritoBridge.artifacts.json` path is a stable relative symlink to
   that file, so one atomic XCFramework exchange publishes the binaries and
   manifest together. The
   manifest binds exact native bridge ABI 21, the privacy-production feature state,
   source commit and fingerprint, header digest, required-symbol inventory, and
   per-slice SHA-256 hashes. Before publication the helper invokes
   `scripts/check_mobile_sdk_artifacts.sh --apple-only` against the staged generation; a
   checker failure leaves the live generation unchanged. Override only the
   recorded bridge version with `--bridge-version <version>` when needed.
   `--allow-dirty-source` is for local integration artifacts and must not be
   used for a release artifact. Never relabel an older ABI artifact.

2. Reauthenticate the published pair and run Swift against the same external
   artifact:

   ```bash
   MOBILE_SDK_PYTHON_BINARY=/absolute/path/python3.12 \
   MOBILE_SDK_APPLE_ARTIFACT_DIR=/absolute/path/apple-artifacts \
     bash scripts/check_mobile_sdk_artifacts.sh --apple-only

   MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT=1 \
   MOBILE_SDK_APPLE_ARTIFACT_DIR=/absolute/path/apple-artifacts \
     swift test --package-path IrohaSwift \
       --disable-automatic-resolution \
       --scratch-path /absolute/path/swift-build
   ```

   `IrohaSwift/Package.swift` admits only an artifact with readable embedded
   metadata declaring exact ABI 21. It does not use a remote URL/checksum binary
   target. The external directory must be canonical, exist already, and remain
   outside the reviewed repository.

3. Create the versioned archive, copied manifest, checksum inventory, and
   package manifest with the repository helper:

   ```bash
   MOBILE_SDK_PYTHON_BINARY=/absolute/path/python3.12 \
   MOBILE_SDK_APPLE_ARTIFACT_DIR=/absolute/path/apple-artifacts \
   MOBILE_SDK_PACKAGE_OUT_DIR=/absolute/path/mobile-sdk-release \
     bash scripts/package_mobile_sdk_artifacts.sh \
       --apple --version <release-version>
   ```

4. Inspect the package manifest and checksum inventory, then tag the clean source
   commit. Generated `dist/*` and external build/package outputs remain untracked;
   only `dist/.gitkeep` belongs in Git. The tag workflow rebuilds and publishes
   its own authenticated release assets.

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
