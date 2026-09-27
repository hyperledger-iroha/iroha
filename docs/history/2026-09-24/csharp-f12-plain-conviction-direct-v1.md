# C# F12 public conviction update slice — 2026-09-24

The maintained C# SDK now constructs the direct registered
`iroha.instruction.v1::governance::UpdatePlainConviction` instruction. Its managed
Norito payload writes exactly `referendum_id`, `owner`, `amount`, and
`duration_blocks` under the native schema. The typed constructor and strict
`FromCanonicalFields` entry reject alternate selectors, noncanonical or zero
quantities, extra choice/action fields, and noncanonical unsigned durations. The
encoder requires the owner's canonical account identity to equal the transaction
authority. The choice is never represented by this instruction.

The C# builder exposes this direct instruction through
`UpdatePlainConviction(referendumId, newTotalBond, durationBlocks)`; the existing
transaction sign/quote path carries its registered InstructionBox frame. This
uses the maintained managed Norito encoder and does not require a new native bridge
symbol or a compatibility layout. Account-address validation still uses the
packaged ABI-23 bridge, as it does for other C# transactions.

Focused validation:

- `dotnet run --project csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj --no-build -- -class Hyperledger.Iroha.Sdk.Tests.UpdatePlainConvictionInstructionTests -noColor`: 4 passed.
- `dotnet build csharp/tests/Hyperledger.Iroha.Sdk.Tests/Hyperledger.Iroha.Sdk.Tests.csproj -c Release --no-restore -warnaserror --verbosity quiet`: passed with zero warnings.
- The same focused class under `-c Release --no-build`: 4 passed.
- The shared Rust KAGEMUSHA fixture class passed 2/2 after adding the exact
  32-byte `app_policy_binding_digest` before the credential's governance
  signature in the C# V1 model and codec. The C# KAGEMUSHA class passed 15/15,
  including canonical roundtrip and reserved-zero/short-digest rejection.
- The unfiltered built Release suite passed 5,796/5,796, with no failures,
  skips, or errors.
- `git diff --check`: passed.

These tests inspect the frame, exact four field order, canonical construction,
authority binding, and builder route. They do not establish a Rust/Kotlin/JS
cross-SDK golden vector or production standalone-election qualification. The
previous unfiltered-suite KAGEMUSHA fixture failure came from the C# credential
omitting Rust's `app_policy_binding_digest` field; the exact V1 wire field is now
present without a compatibility decoder.
