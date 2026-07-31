# NoritoBridge Release Packaging

This guide outlines the steps required to publish the `NoritoBridge` Swift bindings as
an XCFramework that can be consumed from Swift Package Manager and CocoaPods. The
workflow keeps the Swift artifacts in lock-step with the Rust crate releases that ship
Iroha's Norito codec. For end-to-end instructions on consuming the published
artifacts inside an app, see the
[public Swift SDK tutorial](https://docs.iroha.tech/guide/tutorials/swift.html).

The `.github/workflows/mobile_sdk_artifacts.yml` workflow builds, validates,
packages, and publishes tagged Apple artifacts on macOS. The steps below mirror
that workflow for local release verification.

## Prerequisites

- A macOS host with the latest stable Xcode command line tools installed.
- Rust toolchain that matches the workspace `rust-toolchain.toml`.
- Swift toolchain 5.9 or newer.
- CocoaPods (via Ruby gems) if publishing to the central specs repository.
- Access to the Hyperledger Iroha release signing keys for tagging Swift artifacts.

## Versioning model

1. Determine the Rust crate version for the Norito codec (`crates/norito/Cargo.toml`).
2. Tag the workspace with the release identifier (`v<version>`).
3. Use the same semantic version for the Swift package and the CocoaPods podspec.
4. When the Rust crate increments its version, publish a matching Swift
   artifact.

## Build steps

1. From the repository root, invoke the helper script to assemble the XCFramework:

   ```bash
   ./scripts/build_norito_xcframework.sh --privacy-production-enabled
   ```

   The release command requires a clean dependency-closure source tree, compiles the Rust
   bridge for the iOS device, arm64 and x86_64 iOS simulator, and arm64 macOS targets,
   and writes `dist/NoritoBridge.xcframework`. The canonical manifest is embedded at
   `dist/NoritoBridge.xcframework/NoritoBridge.artifacts.json`; the companion
   `dist/NoritoBridge.artifacts.json` path is a stable relative symlink to that file, so
   one atomic XCFramework exchange publishes the binaries and manifest together. The
   manifest binds exact native bridge ABI 21, the privacy-production feature state,
   source commit and fingerprint, header digest, required-symbol inventory, and
   per-slice SHA-256 hashes. Before publication the helper invokes
   `scripts/check_mobile_sdk_artifacts.sh --apple-only` against the staged generation; a
   checker failure leaves the live generation unchanged. Override only the recorded bridge version with
   `--bridge-version <version>` when needed. `--allow-dirty-source` is for local
   integration artifacts and must not be used for a release artifact.

2. Zip the XCFramework for distribution:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     dist/NoritoBridge.xcframework \
     dist/NoritoBridge.xcframework.zip
   ```

3. Update the Swift package manifest (`IrohaSwift/Package.swift`) to point to the new
   version and checksum:

   ```bash
   swift package compute-checksum dist/NoritoBridge.xcframework.zip
   ```

   Record the checksum in `Package.swift` when defining the binary target.

4. Update `IrohaSwift/IrohaSwift.podspec` with the new version, checksum, and archive
   URL.

5. **Regenerate headers if the bridge gained new exports.** The Swift bridge now exposes
   `connect_norito_set_acceleration_config` so `AccelerationSettings` can toggle Metal /
   GPU backends. Ensure `NoritoBridge.xcframework/**/Headers/connect_norito_bridge.h`
   matches `crates/connect_norito_bridge/include/connect_norito_bridge.h` before zipping.

6. Run the Swift validation suite before tagging:

   ```bash
   swift test --package-path IrohaSwift
   make swift-ci
   ```

   The first command ensures the Swift package (including `AccelerationSettings`) stays
   green; the second validates fixture parity, renders the parity/CI dashboards, and
   exercises the same telemetry checks enforced in Buildkite (including the
   `ci/xcframework-smoke:<lane>:device_tag` metadata requirement).

7. Commit the generated artifacts in a release branch and tag the commit.

## Publishing

### Swift Package Manager

- Push the tag to the public Git repository.
- Ensure the tag is reachable by the package index (Apple or the community mirror).
- Consumers can now depend on `.package(url: "https://github.com/hyperledger/iroha", from: "<version>")`.

### CocoaPods

1. Validate the pod locally:

   ```bash
   pod lib lint IrohaSwift.podspec --allow-warnings
   ```

2. Push the updated podspec:

   ```bash
   pod trunk push IrohaSwift.podspec
   ```

3. Confirm the new version appears in the CocoaPods index.

## CI considerations

- `.github/workflows/mobile_sdk_artifacts.yml` runs the packaging and artifact
  checkers on `macos-14`, uploads the generated archives, and publishes release
  assets for `v*` tags.
- `ci/check_swift_samples.sh` gates the Swift demos against the freshly
  generated framework.
- Release logs and artifact manifests retain the source fingerprint, ABI,
  feature state, required-symbol inventory, and per-slice hashes.
