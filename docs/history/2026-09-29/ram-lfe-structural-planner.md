# RAM-LFE structural lowering candidate

Date: 2026-09-29. Test-only candidate; encryption and complete proof execution
remain unavailable. The implementation follows the eleven-operation contract in
[the scalar packing specification](../../../specs/ram_lfe_plaintext_packing.md).
It consumes the actual validated `HiddenRamFheProgram` and instruction enum,
without introducing a second tape representation, production caller, key,
ciphertext, proof, profile or admission path.

The private `UnqualifiedPlan` records active primitive counts, logical ranks,
output snapshots, Galois key roles and symbolic peak residue storage. Immutable
value IDs and reference counts distinguish aliases from distinct ciphertext
owners. It counts operand-alignment copies and the three-component product
alongside the two-component result; NTT, basis conversion, decomposition, keys,
sanitizer and prover storage remain explicit unresolved requirements. Logical
rank is not an RNS level or a noise estimate. Fixed 256-position private routing
and inactive-step constraints remain outside active native-operation counts.

All eleven operations have explicit lowering. Selection retains eight squarings,
the multiply by embedded one, and the branch multiplication: ten ciphertext
multiplications and relinearizations. The 255-independent-select fixture requires
2,550 multiplications at output rank 10. The separate 255-input-load fixture
requires 1,785 Galois key switches. Sixty-four outputs require 64 masks and 63
accumulator additions. These are distinct structural extrema, not a jointly
attainable program or an encrypted performance claim.

Planning borrows the tape and accepts only public envelope/associated-data sizes,
checked before private scratch allocation. It accepts no secret, plaintext or
coins. Its metadata owner has redacted Debug and clears every initialized owned
word on success, error and unwind. The fixed scratch holds at most
`34 + 256*20 = 5,154` value slots; each slot is two `usize` words. Caller and
compiler-created copies are outside this owned-storage guarantee. Nine typed
qualification blockers remain unconditional, including input admission,
security parameters, noise, physical levels, key ownership, sanitization,
canonical codecs and the complete relation within existing limits.

The isolated exact-source harness passed nine native controls, zero ignored, and
strict `cargo clippy --tests -- -D warnings` for the harness. Two pre-existing
crypto dependency dead-code warnings are reported separately; no whole-crypto
warning-free or cfg(test) integration result is claimed. The independent ninth
control uses actual `Rc` owners and destructor timing, with no planner IDs or
manual reference counts, and compares unique live owners and peak components
after every operation in an aliased load/store/arithmetic/select/output schedule.
Other controls cover all operations, zero-register reads, output snapshots,
256/257 instructions, 64/65 outputs, rank 16/17, operand ranges, checked byte
arithmetic, redaction, and actual initialized-cell clearing.

Evidence is retained beneath ignored
`dist/zk-remediation/2026-09-29/ram-lfe-lowering-plan/`. The final run
`native-20260929T120810Z` took 121.529 seconds with 1,852 unchanged actual-source
inputs and 21 freshly compiled local artifacts from an empty target. Its
immutable binary SHA-256 is
`343f9fc3f6b5cadbdb3d0146bd7586f9fa2e1bbd75c8093cfbbbad39dde18e80`.
The generated harness lock SHA-256 is
`5662b674333564d80c24cda13479d82758a6b75a8939afb5370aa6d0bab0ca61`,
unchanged through locked lint. Rust/Cargo 1.93.1 fingerprints, complete exact test
names, local compiler paths/output hashes and source guards are retained.
No Rust flags, wrappers, build-target, profile or stack overrides were inherited.
The harness uses the actual frozen default-feature crypto dependency; it does
not substitute a simplified owner or enable diagnostic encryption fixtures.

The final source SHA-256 is
`6e00f13d64c680f0ae9810da7cad25f7c731656143fa5a51e500e911ed04521d`;
the two-file source/module-declaration amendment is
`393260ae5894b9b0559120fb5242e54e6afca9b95da818dd116455b397a705d0`.
Earlier attempts remain: initial harness lock refusal, a compile error from an
unsupported nested-slice zeroize call, and an eight-test pass whose lint could
not start because of unused harness patch entries. The final source clears each
word explicitly and the harness declares only its actual two patched packages.
No failed receipt was overwritten or relabeled.
