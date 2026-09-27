# F12 public conviction fixture contract inventory, 2026-09-24

This is a source inventory, not SDK or release qualification. The current canonical
Norito-RPC corpus has no `UpdatePlainConviction` entry. The registered V1 instruction
has exactly `referendum_id`, `owner`, `amount`, and `duration_blocks`, with no choice
field; Rust owns its schema and direct wire registration in
[`governance.rs`](../../../crates/iroha_data_model/src/isi/governance.rs) (lines 335–361)
and [`wire_ids.rs`](../../../crates/iroha_data_model/src/isi/registry/wire_ids.rs) (line 441).
[`framed_instruction_payload`](../../../crates/iroha_data_model/src/isi/mod.rs)
(line 785) returns the registered wire ID and canonical framed bytes from the
concrete `InstructionBox`. That is the suitable Rust authority for one exact
direct-instruction golden. Its test must decode the frame back through the
registered pair decoder and reject appended direction, aliases, malformed
quantity, and noncanonical framing. Transaction-boundary tests must separately
reject owner/authority mismatch. No fallback decoder is needed.
Use one new, Rust-authored
`fixtures/governance/plain_v1/update_plain_conviction_instruction_v1.json`
descriptor containing the four semantic inputs, registered wire ID, nominal
schema name/hash, canonical framed instruction base64, and byte length. Derive
the frame from the concrete Rust struct rather than a pasted payload; have
Rust re-derive it in a test, then make each maintained SDK compare its encoded
frame to the same bytes and decode/re-encode them exactly. This is an instruction
golden, not a synthetic signed transaction.

The existing transaction-fixture owner is
[`tools/norito_codegen_exporter/src/norito_rpc.rs`](../../../tools/norito_codegen_exporter/src/norito_rpc.rs),
not the thin `xtask` dispatcher. It reads
[`transaction_payloads.json`](../../../fixtures/norito_rpc/transaction_payloads.json),
builds each Rust instruction and transaction, signs with the deterministic test
key, and emits canonical payload and signed frames, hashes, `.norito` blobs,
[`transaction_fixtures.manifest.json`](../../../fixtures/norito_rpc/transaction_fixtures.manifest.json),
and [`schema_hashes.json`](../../../fixtures/norito_rpc/schema_hashes.json)
(exporter lines 52–94, 760–796, 791–795, 920–1000). It copies descriptors to
Python, Java resource, and Swift fixtures; Kotlin, JavaScript, and C# consume the
shared root. `schema_hashes.json` currently covers transaction and SNS DTOs,
not this instruction (exporter lines 438–461). A direct-instruction golden should
assert its concrete `NoritoSchema` frame hash from Rust, not invent a second type
name or mutate the RPC schema table merely for one instruction.

The ordinary transaction corpus cannot yet serve as a byte-identical production
builder golden. Its parser explicitly requires `TransactionAdmissionIntent::Ordinary`
(exporter lines 1210–1218), while C# `TransactionBuilder` exposes only
`QueuePlanSynced` ([`TransactionBuilder.cs`](../../../csharp/src/Hyperledger.Iroha.Sdk/Transactions/TransactionBuilder.cs),
lines 42–44), and the native Swift bridge signs with `QueuePlanSynced`
([`connect_norito_bridge/src/lib.rs`](../../../crates/connect_norito_bridge/src/lib.rs),
lines 4700–4745). Keep the ordinary-only corpus and production builders strict.
Do not add an ordinary SDK signing fallback or weaken the exporter policy to
manufacture parity. A separate Rust-authored `QueuePlanSynced` signed fixture
would need its own V1 generator, ownership and verifier contract before SDK
builders can claim byte-identical signed-transaction parity. The direct frame
golden can be implemented and compared across SDKs independently now.

Maintained consumer paths for the direct frame are Kotlin's
[`UpdatePlainConvictionInstruction.kt`](../../../kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/core/model/instructions/UpdatePlainConvictionInstruction.kt)
and `TransactionPayloadAdapter.kt` (exact known-wire encode/decode), its
[`Java-source consumer test`](../../../kotlin/core-jvm/src/test/java/org/hyperledger/iroha/sdk/core/model/UpdatePlainConvictionJavaConsumerTest.java),
Swift's [`TransactionEncoder.swift`](../../../IrohaSwift/Sources/IrohaSwift/TransactionEncoder.swift)
and `NativeBridge.swift`, JavaScript's [`norito.js`](../../../javascript/iroha_js/src/norito.js)
and `instructionBuilders.js`, Python's native
[`iroha_python_rs/src/lib.rs`](../../../python/iroha_python/iroha_python_rs/src/lib.rs)
and `tx.py`, and C#'s
[`UpdatePlainConvictionInstruction.cs`](../../../csharp/src/Hyperledger.Iroha.Sdk/Transactions/UpdatePlainConvictionInstruction.cs).
The existing corpus checks are Kotlin `TransactionFixtureParityTest`, Swift
`NoritoRpcFixtureParityTests`, JavaScript `transactionFixturesParity.test.js`,
Python `scripts/check_python_fixtures.py`, and C# `NoritoRpcFixtureParityTests`.
C# hardcodes the current 27-entry corpus (line 16); adding an RPC entry would
require its count and [`generated-files.toml`](../../../generated-files.toml)
output inventory to change. Kotlin and Java-source runtime tests need a
same-source ABI-23 `connect_norito_bridge`; Swift needs its same-source
XCFramework; JavaScript native parity needs a same-source host build; Python
native construction needs a same-source `iroha-native` extension. Source-only
roundtrips or stale native artifacts do not establish cross-SDK parity.

The documented RPC generator command is `cargo run --locked -p xtask --features
dev-tools --bin xtask -- norito-rpc-fixtures --output-root ABSENT_EXTERNAL_ROOT`,
then `norito-rpc-verify`, with two independent matching publications
([`fixtures/norito_rpc/README.md`](../../../fixtures/norito_rpc/README.md),
lines 7–29). Its `FixtureOptions::resolve_paths` canonicalizes the requested
parent and rejects every `output_root` within this checkout, including ignored
`target/` (exporter lines 150–228). `TMPDIR` under `target/` moves only the two
internal render trees; it cannot make an in-repo output root legal. The OpenAPI
release generator/check similarly requires private external staging and a
sealed source replay ([`generated-files.toml`](../../../generated-files.toml),
lines 108–145); this direct instruction adds no typed Torii route, so OpenAPI
generation is a separate candidate gate rather than a source for its wire bytes.

The local integration cut adds explicit `norito-rpc-fixtures --local-integration
--output-root <checkout>/target/norito-rpc-local/<absent-name>` parsing. The
`target/norito-rpc-local` parent must already exist as an owner-owned, mode-0700,
non-symlinked directory. The exporter accepts only a direct child there, rejects
existing destinations, captures parent identity, checks it before and after
rendering and again before the final seal, and creates both independent temporary
render trees inside that same checkout-local parent. The existing byte/mode
comparison, source preimage checks, owned-path inventory, and create-only
manifest-seal publication remain the same. The parser/path tests cover explicit
mode selection, out-of-lane, symlink, existing destination, identity drift,
and unchanged rejection by the external mode. Focused Cargo tests passed 1/1
for the exporter path policy and 1/1 for the `xtask` parser. An actual
`norito-rpc-fixtures --local-integration` run into the absent
`target/norito-rpc-local/f12-local-integration-20260924` root published 27
entries create-only. Readback verified all 27 payload files against manifest
base64, lengths, and modified BLAKE2b-256 hashes; all 32 published files were
byte-identical to their canonical owned counterparts. This is local test
evidence, never signed promotion evidence. The external-root release command
and its in-repo rejection remain unchanged; neither it nor OpenAPI release
scripts should run under the user's repository-only restriction.
