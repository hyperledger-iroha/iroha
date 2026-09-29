# RAM-LFE bounded initializer and execution trace

Status: source implementation and focused native controls pass. This is the next
ZK03 stage after the integrated candidate's unavailable-relation refusal. It does
not activate proof policies or implement the missing execution circuit.

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

The descriptor is independently Blake2b-hashed with the Iroha marker. Its current
1,008-byte body hashes to
`e90e6314da02be784ba7d2d2a3e18ff171711135e80a8eddb7ad31d7b28d3d01`.
The native descriptor pin passes. The captured profile and public-parameter
schema identities remain `dc94cd0e80cf1f3b4e092b0de6448d55` and
`fc0b05d927cc5d2835b824943b099b7d`; no schema fixture was rewritten. A fixed
127-byte canonical input frame and its 1,024-byte XOF stream are retained in the
crypto test fixtures and checked together with all 32 resulting residues.

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

Remaining ownership work includes the caller/config hidden-program DTO, its raw
instruction storage and decode scratch, plus BFV internal arithmetic temporaries.
Compiler-created copies and target side channels remain separate qualification.
Do not infer complete evaluator erasure from the new owners.

JavaScript and Swift require and preserve the exact marked descriptor hash;
Kotlin profile propagation is being added. Torii's program-ID associated data
uses the canonical writer independently of ambient layout flags. A maximum Name
test checks the 512-byte initializer bound. OpenAPI marks the field required.

## Validation ledger

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
- Initial JavaScript test invocation stops before tests because the installed
  dirty native artifact no longer matches source provenance. No bypass is used.
- Initial Swift test invocation stops at package validation because the main
  checkout lacks `dist/NoritoBridge.xcframework`. No decoder tests execute.
- Refreshed native artifacts, SDK tests and same-candidate runtime qualification
  remain pending. Earlier Apple/wallet evidence applies
  to its retained source candidate, not these new profile changes.

Full proof completion still requires the
[execution relation contract](../../../specs/ram_lfe_execution_proof.md).
