# Swift current-checkout build prerequisite — 2026-09-23

On the existing `optimizations` checkout, `swift test` from `IrohaSwift/`
stopped during package-manifest evaluation before compiling Swift sources or
running tests. `Package.swift` requires an ABI-23 `NoritoBridge.xcframework` and
its artifact manifest; the checkout currently has no
`dist/NoritoBridge.xcframework`. The exact failure named that missing path.

This is an unexecuted SDK gate, not a Swift test failure or evidence of parity.
Once the native interface settles, the same-source local-integration builder
can produce the bridge under `target/norito-bridge-local/artifacts` for developer
tests. The final reviewed candidate still requires the separately authenticated
five-target native package, reproducibility and installed SDK/platform matrix.
No external checkout, alternate branch, or synthetic framework was used.

The six changed Swift source/test files passed `swiftc -parse` in this checkout;
this checks syntax only and does not substitute for bridge-linked package tests.
The Swift native release-contract Python guard passed 13/13 checks after its
fixtures were synchronized with the current builder. Its Python 3.12 preflight
now fails when the required interpreter is absent instead of skipping checks.
