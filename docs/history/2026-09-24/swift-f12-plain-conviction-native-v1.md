# Swift public-conviction update and native bridge, 2026-09-24

`IrohaSwift` now has one direct `UpdatePlainConvictionRequest` and signing builder. The instruction carries exactly `referendum_id`, `owner`, `amount`, and `duration_blocks`; it has no choice field. The SDK validates canonical I105 authority/owner identity, exact owner–authority equality, the V1 referendum selector, and canonical `Quantity` before calling native code. The native bridge exports one algorithm-aware V1 encoder that repeats those checks, constructs the registered `UpdatePlainConviction` instruction, and clears outputs on failure. There is no default-algorithm alias or retired decoder for this instruction.

The C header, Swift loader, XCFramework build inventory, and package validator all require the same symbol. Rust tests assert the one-instruction wire shape, its choice-free fields, selector and amount boundaries, owner authorization, and refusal before key parsing. The strict Swift tests cover invalid inputs and the signing builder.

Checks completed on `optimizations` in this checkout:

- `swiftc -frontend -parse` on all four edited Swift source/test files: pass.
- `rustfmt --edition 2024 --check crates/connect_norito_bridge/src/lib.rs`: pass.
- `scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p connect_norito_bridge --lib governance_update_plain_conviction -- --nocapture`: 2/2 pass.
- Same-source `connect_norito_bridge --lib` host build: pass; rebuilt `target/debug` library used by Kotlin and Java-source `UpdatePlainConviction` runtime tests, 4/4 pass (3 Kotlin, 1 Java).
- `TMPDIR=.../target/swift-sdk-temp python3 -m pytest -q scripts/tests/validate_norito_bridge_xcframework_test.py`: 22/22 pass.
- `bash -n scripts/build_norito_xcframework.sh` and scoped `git diff --check`: pass.

The focused Swift runtime command stopped during package-manifest validation before compilation: `dist/NoritoBridge.xcframework` is absent. The ignored checkout-local integration lane `target/norito-bridge-local` is supported by the builder and Package.swift; Rust Apple targets, Xcode 27.0 SDKs, and Python 3.12 are present. The same-source local XCFramework build/runtime check remains open until the combined source and its other native SDK tests are stable. This source slice does not close full SDK fixture parity or release packaging.
