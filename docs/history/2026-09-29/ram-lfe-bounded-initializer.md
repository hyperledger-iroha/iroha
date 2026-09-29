# RAM-LFE bounded initializer and execution trace

Status: all 61 initializer, trace and immutable hidden-program owner controls
pass under ordinary native Cargo, both owner API doctests pass, and all five
focused config integration controls pass. These ZK03 stages follow the integrated candidate's unavailable-relation
refusal. They do not activate proof policies or implement the missing execution circuit.

## First-release contract

`ram_lfe/initialization.rs` defines one compiled descriptor, required in
`BfvRamProgramProfile.initializer_descriptor_hash`. There is no sampler selector
or old-profile decoder. The descriptor binds:

- BLAKE3 derive-key mode and distinct policy-secret, private-tape and initializer
  contexts, with canonical Norito framing and explicit nominal schemas.
- A fixed 1,024-byte initializer expansion, split into 32 ascending consecutive
  32-byte big-endian integers and reduced modulo 257.
- Secret length 1..4,096, associated-data length 0..512, 32 state lanes and four
  zeroed registers. Secret length bounds are not an entropy guarantee.
- The exact fixed fold `x = byte + 257 - residue`, followed by constant-time
  selection of `x` or `x - 257`. Each lane performs exactly 32 folds.

The descriptor is independently Blake2b-hashed with the Iroha marker. The new
owner stage's 1,130-byte body hashes to
`bb343e3afee77e875518c41910390d42ad81c9ea830446ec339e1f0810065f1f`.
The native predecessor descriptor pin was
`e90e6314da02be784ba7d2d2a3e18ff171711135e80a8eddb7ad31d7b28d3d01`.
The captured profile and public-parameter
schema identities remain `dc94cd0e80cf1f3b4e092b0de6448d55` and
`fc0b05d927cc5d2835b824943b099b7d`; their schema fixtures are unchanged. A fixed
127-byte canonical input frame and its 1,024-byte XOF stream are retained in the
crypto test fixtures and checked together with all 32 resulting residues. The
owner stage rotates these vectors and explicitly projects the two borrowed
secret-preimage types onto their descriptor-declared frame identities, avoiding
erased Rust lifetime arguments in the wire identity. The 61-control native run
confirms these rotated frames, schemas and vectors.

For an ideal uniform 256-bit lane, the exact statistical distance after reduction
is `256 / (257 * 2^256)`; the 32-lane union bound is less than `2^-251`.
The actual XOF requires its computational security assumption; this arithmetic is
not an independent security review or a claim of perfectly uniform output.
BLAKE3's domain-separated derivation uses the
[upstream derive-key contract](https://docs.rs/blake3/latest/blake3/fn.derive_key.html).

## Ownership and developer contract

The locked BLAKE3 1.8.5 library's `zeroize` feature clears owned `Hasher` and
`OutputReader` state. Borrowed framed inputs avoid secret preimage vectors.
Expansion bytes, residues, machine state, displaced registers, emitted ciphertext
copies and the trace have clearing owners. The directly reached BFV scalar-add
temporary is also guarded. Successful public output leaves its owner only after
canonical encoding completes.

Tracing uses the existing interpreter, including all eleven operations. Its
maximum snapshot payload is 9,474,048 bytes, plus 12,288 bytes of normalized
instruction metadata. It does not create a second evaluator or serialize a proof.
Its Debug representation is redacted and it is not Clone. Callers who borrow
private trace cells are responsible for any copies they make.

The subsequent owner stage replaces the caller/config mutable program DTO with
one immutable shared tape. A typed builder reserves 12,288 bytes once and writes
48-byte canonical instruction slots directly into clearing storage. Every slot
has six little-endian words, with all unused words required to be zero. Clones
share the same allocation and Debug is redacted. The new first-release frame is
`iroha_crypto::ram_lfe::HiddenRamFheProgramV1`; its maximum size is 12,346 bytes.
There is no reader for the retired enum-sequence frame. Configuration's existing
`hidden_program_hex` key now parses directly into the validated owner through
bounded, clearing byte scratch. JSON accepts the exact unescaped lowercase hex
literal. The owner exposes no generic archive decoder; its explicit reader
validates the Norito header, version, schema, flags, length and checksum before
the borrowed payload decoder. Exact canonical re-encoding is checked by streaming
comparison. Caller-owned configuration source strings remain the source parser's
responsibility. `to_bytes` returns a clearing byte owner; callers selecting generic
serialization into their own buffers are responsible for clearing those buffers.
Typed instruction values read from or passed into the builder are private copies;
the tape's erasure guarantee does not cover those values.

Remaining ownership work includes BFV internal arithmetic temporaries.
Compiler-created copies and target side channels remain separate qualification.
Do not infer complete evaluator erasure from the new owners.

JavaScript and Swift require and preserve the exact marked descriptor hash;
Kotlin profile propagation is being added. Torii's program-ID associated data
uses the canonical writer independently of ambient layout flags. A maximum Name
test checks the 512-byte initializer bound. OpenAPI marks the field required.

## Validation ledger

- The owner stage's normal locked/offline Rust 1.93.1 crypto build and all 61
  selected native controls pass, with zero ignored tests and no source repair.
  The retained test binary has SHA-256
  `27ac9785b6c34c297c3ab0e1d63ec176adaaffe5d0450386e0a78bfcbd778bb2`.
  Build plus tests took 165.60 seconds; tests took 30.25 seconds. All 231 captured
  source/fixture files were unchanged. Exact
  captured source bytes, logs, manifests, independent-vector scripts/receipts and
  the binary are under
  `dist/zk-remediation/2026-09-29/ram-lfe-native/20260929T044810Z`.
  New owner controls include every instruction/reserved word, maximum frame,
  old identity and malformed headers, strict outer budgets, builder/decode
  semantics, shared clones, real-cell clearing on error/unwind, and typed hex/JSON.
  Cross-crate config execution has its separate evidence below.
- Both owner doctests pass through ordinary Cargo: the typed builder/clearing
  frame example and the compile-fail generic archive decoder boundary. The
  source manifest is unchanged. The first evidence parser mistook rustdoc's
  `- compile fail ... ok` spelling for a missing control despite both tests
  passing; its log/receipt are preserved, and the corrected parser's warm
  rerun passes in 2.63 seconds. No Rust source repair was needed.
- All five scoped RAM-LFE config controls pass under the ordinary
  `iroha_config_integration` Cargo target, including the fixture parser,
  typed-owner parity/redacted Debug, required private material, retired toggle,
  and empty/duplicate/malformed-program rejection. Zero tests were ignored and
  the captured source did not drift. The final warm build plus tests took
  3.35 seconds; the retained binary SHA-256 is
  `ea9972426c9175c3e99136b11227be9ddec4e8b7364569388b6dbb177794a433`.
  Evidence is under `ram-lfe-native/config-20260929T053011Z` in the same dated
  `dist` root. Preceding attempts remain separate: the initial runner named a
  module rather than the explicit aggregate test target; the first real compile
  rejected the new fixture test's access to private `user::Root.torii`; a warm
  run passed four tests but its malformed-input assertion expected an inner
  field name instead of the typed decoder's outer parameter context. A subsequent
  assertion edit matched the adjacent missing-field test and produced three
  passes/two failures; that edit was corrected before the final five passes.
  The fixture now uses public `Root::parse()`. The two negative cases separately
  assert a missing field or the parameter context plus bounded lowercase-hex
  diagnostic, and the malformed-input case rejects private-value echo. No
  configuration runtime or crypto repair was required.
- Source checks: scoped Rust formatting, JavaScript syntax and required OpenAPI
  field checks pass. The no-legacy-codec guard passes for the crypto slice.
- New Rust tests cover every byte transition, independent bitwise reduction,
  framing/domain separation, input bounds, retired-profile rejection, all-op
  traced/untraced equivalence, maximum trace geometry and error/unwind clearing.
  The normal locked/offline Rust 1.93.1 crypto build and all 52 selected native
  controls pass, with zero ignored tests. The immutable test binary has SHA-256
  `44562a508ef9b93078118417ebc290698445fcb9792ad26fb8095ec17eba82af`.
  Tests took 32.70 seconds; build plus tests took 180.51 seconds. All 223 captured
  source/fixture files were unchanged during the run. Logs, source manifests and
  the binary are under
  `dist/zk-remediation/2026-09-29/ram-lfe-native/20260929T035213Z`.
- The initial native run is preserved separately: 49 passed and three new trace
  fixtures failed before evaluation because their textual seed contained the
  forbidden word `fixture`. The fixtures now use fixed public binary test
  material. The all-operation and maximum-shape tests execute real evaluation;
  no validation rule was weakened to admit their inputs.
- An independent scalar Python implementation reproduced the KAT's canonical
  field framing, CRC64, BLAKE3 derive-key XOF and integer reduction. It matches
  all 1,024 stream bytes and 32 residues after two standard BLAKE3 controls.
  Its script and receipt are in the same `ram-lfe-native` evidence directory.
  This checks the public one-chunk vector, not the primitive's security.
- The subsequent owner stage independently reconstructs the 12,346-byte default
  tape frame and pins its digest to
  `6f4a296ecb6fb66f60c2c696369429de4b08ed1df062ecea706f15b6447d7d0b`.
  Its scalar tree/XOF calculation passes all 105 hash, keyed-hash and derive-key
  controls in the [upstream BLAKE3 1.8.5 vectors](https://github.com/BLAKE3-team/BLAKE3/blob/1.8.5/test_vectors/test_vectors.json).
  The downloaded vector file has SHA-256
  `dcb91ea8accc77e6d6e632af7cdc1a99a9f3ae78cf648da595c7d064db32f624`;
  the script and receipt are retained in `ram-lfe-native`. The 61-control native
  run agrees with this full-tape pin. These controls are not a security audit.
- Initial JavaScript test invocation stops before tests because the installed
  dirty native artifact no longer matches source provenance. No bypass is used.
- Initial Swift test invocation stops at package validation because the main
  checkout lacks `dist/NoritoBridge.xcframework`. No decoder tests execute.
- Refreshed native artifacts, SDK tests and same-candidate runtime qualification
  remain pending. Earlier Apple/wallet evidence applies
  to its retained source candidate, not these new profile changes.

Full proof completion still requires the
[execution relation contract](../../../specs/ram_lfe_execution_proof.md).
