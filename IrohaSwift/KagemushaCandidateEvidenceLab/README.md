# Kagemusha candidate evidence lab

This is a non-shipping, physical-iPhone-only XCTest host for the Taira-testnet
Kagemusha V4 candidate. It deliberately has no simulator target and links only
the marker-bearing `NoritoBridgeCandidateLab.xcframework`.

The two test methods are run by separate `xcodebuild test-without-building`
processes. The proof launch durably fsyncs and reopens its checkpoint before
exit. The restart launch reinstalls the exact eight native artifacts, reopens
that checkpoint, validates and redeems every branch, and requires the native
duplicate-input error `-311`.

Both launches sample a real `NWPathMonitor` before, through, and after native
execution. Every sample must be `unsatisfied`, and the installed
`URLProtocol` request observer must report zero requests. Simulator execution
is compile-time rejected.

The public operator procedure and evidence contract are documented in
`specs/sdk/swift/readiness/kagemusha_candidate_ios_lab.md`.
