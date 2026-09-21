# C# native host integration

Scope: macOS ARM64 development source; all release goals remain open. The host
SDK was obtained from Microsoft's .NET 8 release metadata at the repository's
exact `8.0.419` pin. Its archive SHA-512 matches that metadata. Download, toolchain
and extraction identities are in `target/first-release-dotnet-8.0.419-host-20260921`.
The system's different SDK was not substituted and `global.json` was not changed.

The complete solution restores and builds in Release with `-warnaserror`. The
unit/protocol assembly uses the authentic retained ABI-23 host bridge with
SHA-256 `1f709bb8a1c87cda7525be9c329644c753ee6a6bed00a662819aada3c8d87518`.
That bridge was rebuilt from observed Rust inputs for the Kotlin host work.
The first C# run passes 5,754 tests and fails nine, without skips or source drift.
Six fixtures supplied invalid Ed25519 public keys. Two contract substitutions
lacked the required authenticated caller. A faucet response test instead changed
the caller's input and was rejected by its original semantic binding before HTTP.

Test repairs derive real Ed25519 public keys, use the existing authenticated
contract client, and substitute the faucet response after a coherent original
request. Existing negative assertions remain; transport-reaching tests now also
assert dispatch. Production key, binding and authorization checks were unchanged.
The next run passes all 5,763 tests.

The privacy adapter then removes its private 16 MiB stack workers. Queries and
all native validators run directly on the caller stack, retaining input snapshots,
status rejection and native-buffer cleanup. A separate cold-process test initializes
the native privacy owner and exercises both archive formats, malformed input and
capability rejection from an ordinary managed thread-pool worker. That test passes,
followed by all **5,764 tests** in the complete unit/protocol assembly. The solution
build has zero warnings/errors. No test is skipped or left unexecuted; observed
managed/fixture inputs, the test assembly and the native artifact stay unchanged.

The three full-run packets are `target/first-release-csharp-host-20260921`,
`target/first-release-csharp-host-2-20260921` and
`target/first-release-csharp-host-3-20260921`. Each retains commands, assembly/native
hashes, XML results, failure details and source observations. The last packet also
retains the separate cold-process result. Source repairs are preserved under
`target/first-release-csharp-fixture-repair-20260921` and
`target/first-release-csharp-ordinary-stack-repair-20260921`.

The cross-SDK source guard initially passes 78 controls and fails 20 because its
exact JNI include inventory omits the existing Kotlin reserve-finality owner and
its browser check expects a retired error-helper spelling. The reviewed inventory
now includes that existing owner and scans it for unapproved privacy exports.
The browser check inspects `getNativeBinding` itself, requiring its exact error
code/status and throw while rejecting returned or global bindings; the separate
unavailable-status diagnostic is not a binding producer. All 104 source/mutation
controls then pass, including hidden-export injections in every included JNI part.
Both browser/global-binding runtime controls also pass. Their attempt to load the
old local N-API artifact rejects its source-provenance mismatch, so this is not a
fresh JavaScript native-package pass. Records are under
`target/first-release-sdk-parity-owner-repair-20260921`. The source audit identifies
itself as a prerequisite, not native release authority.

This does not qualify full maximum-size proofs, live Torii, the other four native
targets, reproducible signed packages, or a final immutable candidate. Later shared
Rust integration requires rebuilding native packages for that later candidate.
