# Indexed key coefficients, roles and snapshots — 2026-09-22

This candidate adds a bounded coefficient reader, explicit fixed/permutation/mask
storage identities and a private one-column snapshot adapter. Original source
authentication, profile admission and post-read failure poisoning still belong to
the consuming Core owner. Normal stored/Core proof entry points retain dense keys;
this is component evidence, not production or device qualification.

Work remains in `/Users/takemiyamakoto/dev/iroha`, branch `optimizations`, captured
HEAD `8209033deae7409d800fcc576a63609a0c2c5f9d` plus the recorded working changes.
Receipts are under `target/kagemusha-validation/20260922/`.

| Window | Result | Compile / test time |
| --- | --- | ---: |
| `indexed-key-batch-default-r1` | 79 passed, zero failed or ignored | 58.659 s / 144.262 s |
| `indexed-key-batch-no-multicore-r1` | Same 79 passed, zero failed or ignored | 47.743 s / 128.876 s |
| `indexed-key-batch-production-check-r1` | Non-test library check passed | 3.569 s |

All six before/after maps match: **267 source/build inputs**. Both recorded test
executables remain unchanged in their windows. The selection contains:

- 30 structured-key tests: fourteen codec, four native-reader, four new coefficient/
  cross-coset and eight snapshot helper/integration tests. Both Pasta fields compare
  against the frozen original dense codec and transform; tests include fixed modes,
  compressed/direct encodings, short I/O, invalid geometry, source/output faults,
  exact descriptors and guarded cleanup. The transform-boundary unwind is a
  synthetic hook; it does not establish allocator-abort or whole-process erasure.
- Nine new key-role tests: four canonical digest/layout, three independent transform
  and two actual completed-advice-owner substitution rejection tests. New tags
  preserve existing tags 0–11; fixed/sigma identities and three mask kinds are
  separate and masks reject Lagrange representation.
- Thirteen existing role/layout tests and the full previous 27-function inner-IPA
  selection. Its satisfiable generic PLONK fixture verifies both Pasta curves in
  three instance modes with exact dense bytes, transcript and next-RNG agreement.
  These six generic cases are not final recursive monetary circuit qualification.

The 16-file installed batch also adds two tests against the actual encrypted Core
polynomial store. They are **not included** in the 79 vendor tests and await root
Core compilation/execution. Native bridge/JVM failures and physical OEM qualification
remain separate open gates. No source-snapshot comparison here covers the full
Core dependency graph or third-party Cargo caches.

The coefficient adapter reuses one caller-owned field column. Its snapshot helper
charges the actual field capacity and workspace header including a 256-scalar
encoding chunk. Original index/VK/domain, FFT twiddles, backend windows, allocator,
stack and arithmetic temporaries remain additional costs; no 128 MiB RSS or latency
claim follows. The later borrowed-owner/quotient integration is staged separately
and is not part of this installed, tested candidate.

| Receipt | SHA-256 |
| --- | --- |
| All source maps | `0406bb3f06ed1043cec18a27365f1fcf067e01eb77c5e6ce2776d89c007db11e` |
| Default `result.json` | `afb0eff19b7bdc72bf47942145a286a4422e5e73351e4fa6b255ba4a99b8dd19` |
| Default executable | `8f6fc48f1b729c7b003929157fa9e8909742d60dc5ba49ab82938508031b8198` |
| No-multicore `result.json` | `ff3f1fd91e514ad20a4ab87c7be8addd20139da55a82be001a7b621e7300ba59` |
| No-multicore executable | `11fcef6431325a12970009bba715afb7f390dedd9d95c8e54426e39e2b8611f3` |
| Non-test `result.json` | `39d3b9ba2d64090e33d234370b83badf2b2ab162c1ff7525b672a4f9780420bd` |

The exact selection is `indexed-key-batch-selection.json`; each test receipt retains
compile command/result, Cargo artifact metadata, all names/counts, stdout/stderr and
source maps. The installed pre/postimage pins are `indexed-key-batch-install.json`.
Commands use `cargo iroha-fast -- test --manifest-path vendor/halo2-axiom/Cargo.toml
--locked --offline --target-dir vendor/halo2-axiom/target --config
'patch.crates-io.halo2curves-axiom.path="/Users/takemiyamakoto/dev/iroha/vendor/halo2curves-axiom"'
--lib --no-run --message-format=json`. No-multicore adds `--no-default-features
--features batch,circuit-params`; non-test uses `check ... --lib`. This is the
standalone vendor manifest/lockfile, not the full root workspace.

See [current readiness](../../../../specs/kagemusha_v1_production_readiness.md)
for unfinished protocol, native ownership, SDK and release goals.
