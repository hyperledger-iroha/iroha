---
lang: kk
direction: ltr
source: docs/norito_bridge_release.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b9dc9862d4806d355fd83c885de92775712a7b32c68c010d29f4fc74229d054b
source_last_modified: "2026-01-06T05:24:53.995808+00:00"
translation_last_reviewed: 2026-02-07
---

# NoritoBridge Release Packaging

This guide outlines the steps required to publish the `NoritoBridge` Swift bindings as
an XCFramework that can be consumed from Swift Package Manager and CocoaPods. The
workflow keeps the Swift artifacts in lock-step with the Rust crate releases that ship
Iroha's Norito codec. For end-to-end instructions on consuming the published
artifacts inside an app (Xcode project wiring, ChaChaPoly usage, etc.), see
`docs/connect_swift_integration.md`.

> **Note:** CI automation for this flow will land once macOS builders with the required
> Apple tooling come online (tracked in the Release Engineering macOS builder backlog).
> Until then the steps below must be executed manually on a development Mac.

## Prerequisites

- A macOS host with the latest stable Xcode command line tools installed.
- Rust toolchain that matches the workspace `rust-toolchain.toml`.
- Swift toolchain 5.7 or newer.
- CocoaPods (via Ruby gems) if publishing to the central specs repository.
- Access to the Hyperledger Iroha release signing keys for tagging Swift artifacts.

## Versioning model

1. Determine the Rust crate version for the Norito codec (`crates/norito/Cargo.toml`).
2. Tag the workspace with the release identifier (e.g. `v2.1.0`).
3. Use the same semantic version for the Swift package and the CocoaPods podspec.
4. When the Rust crate increments its version, repeat the process and publish a matching
   Swift artifact. Versions may include metadata suffixes (e.g. `-alpha.1`) while testing.

## Build steps

1. From the repository root, invoke the helper script to assemble the XCFramework:

   ```bash
   ./scripts/build_norito_xcframework.sh --workspace-root "$(pwd)" \
       --output "artifacts/NoritoBridge.xcframework" \
       --profile release
   ```

   The script compiles the Rust bridge library for iOS and macOS targets and bundles the
   resulting static libraries under a single XCFramework directory.
   It also emits `dist/NoritoBridge.artifacts.json`, capturing the bridge version and
   per-platform SHA-256 hashes (override the version with `NORITO_BRIDGE_VERSION` if
   needed).

2. Zip the XCFramework for distribution:

   ```bash
   ditto -c -k --sequesterRsrc --keepParent \
     artifacts/NoritoBridge.xcframework \
     artifacts/NoritoBridge.xcframework.zip
   ```

3. Update the Swift package manifest (`IrohaSwift/Package.swift`) to point to the new
   version and checksum:

   ```bash
   swift package compute-checksum artifacts/NoritoBridge.xcframework.zip
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

- Create a macOS job that runs the packaging script, archives artifacts, and uploads the
  generated checksum as a workflow output.
- Gate releases on the Swift demo app building against the freshly produced framework.
- Store build logs to assist in diagnosing failures.

## Additional automation ideas

- Use `xcodebuild -create-xcframework` directly once all required targets are exposed.
- Integrate signing/notarisation for distribution outside developer machines.
- Keep integration tests in lock-step with the packaged version by pinning the SPM
  dependency to the release tag.
