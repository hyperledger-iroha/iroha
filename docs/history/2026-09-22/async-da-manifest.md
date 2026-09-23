# Asynchronous DA manifest ownership

The storage proof workflow previously entered the SDK's synchronous HTTP path
from an asynchronous method. The blocking transport correctly rejected that
call inside Tokio, so manifest retrieval prevented the proof workflow from
reaching provider selection. The canonical public read is now
`Client::da().manifest(&StorageTicketId).await`; its explicit blocking facade
uses the same implementation and reusable runtime.

The request uses the immutable context's transport and deadline, one JSON
representation and a 64-MiB response ceiling. It dispatches once, preserves
structured HTTP/transport/decode/deadline errors, and binds the response to the
exact requested ticket. The HTTP record has one owner in
`iroha_torii_shared::da`; Torii constructs that record directly. Storage owns
artifact decoding, chunk planning, proof work and filesystem persistence.

Storage validates the exact Norito artifact length and BLAKE3 hash, manifest
version, ticket/blob/chunk identities, lane, epoch and complete JSON projections.
The chunk-plan projection uses the same CAR implementation as Torii. Comparing
only parsed chunk specs would discard unknown nested fields; the final
conversion compares the complete canonical projection and rejects that case.
Review also caught Norito's numeric equality coercing float `1.0` to integer `1`.
The conversion now traverses both projections explicitly, checking numeric shape
without recursive calls or changing the process stack allowance.
No alternate field spellings, missing-length defaults or retired raw SDK method
remain. CLI manifest reads use the blocking capability and the canonical storage
bundle. The single endpoint override is `--torii-url`, an immutable Torii base
URL; the manifest-specific endpoint flags are removed.

## Scoped qualification

The exact Cargo-emitted library harnesses ran with ordinary stacks and four
test workers each. The SDK passes 917 tests and shared Torii models pass 310,
with no failures or ignored tests. After the nested-plan correction, the full
storage library passes 53 tests with no failures or ignores after numeric-shape
validation. SDK/shared source
hashes are identical between those builds; storage's final run has its own
source and executable receipt. These are 1,280 distinct library passes, not a
whole-workspace or release qualification.

The tests cover current-thread executor responsiveness, injected asynchronous
transport, exact ticket binding, configured cancellation, response bounds,
content negotiation, structured failures without replay, blocking-runtime
rejection/reuse, artifact/projection corruption, canonical persistence and
proof-workflow cancellation before any provider input is consumed.

Build command:

```sh
scripts/cargo_fast.sh --stable-local-metadata --no-sccache --incremental --jobs 2 -- test -p iroha -p iroha_storage_client -p iroha_torii_shared --lib --no-run --locked --offline --message-format=json
```

The initial build failed on two test macro paths, corrected before the passing
build. Both attempts are retained. Strict Clippy failed: selecting the three
libraries reports 19 existing shared-crate diagnostics; selecting SDK/storage
test targets reports seven existing SDK diagnostics. They cover unchanged
documentation, function/enum sizes, visibility/conversions and a dependency
feature-name lint. No lint was suppressed and no passing strict-lint result is
claimed. The codec guard, 666-route inventory check, all 20 feature-resolved
dependency boundaries, whole-workspace formatting and historical archive
verification pass.

The complete source-file budget check fails with 276 findings against the
existing baseline, including preexisting oversized files and stale downward
ratchets. All new DA modules are below their respective production/test limits.
No budget or exception was expanded; repository-wide decomposition remains open.

The first CLI/Torii test-target check failed with 17 diagnostics in concurrent
Core membership changes (unsafe-code policy, retirement API and visibility).
It also records a Torii routing-source formatting change during capture. Those
diagnostics were sent to their owner and corrected; that failed, drifting
capture is retained and does not qualify consumers.

The coordinated Core/Config/Torii library test build then passes, retaining its
concurrent Core source-change record. Eleven scoped Torii DA controls report
nine passes and two failures. One exposes missing normalization of the operating
system's no-follow symlink error in the ticket-directory reader; it now uses the
existing canonical directory-error mapper, preserving `InvalidData` and refusing
the symlink. The other expects obsolete wording from the bounded regular-file
reader. Both manifest and PDP assertions now use its current diagnostic. These
are explicit corrections, not broadened rejection assertions.

The CLI binary test target compiles with unchanged captured CLI inputs. Its exact
emitted harness passes all 35 DA command tests and five new manifest adapter and
argument tests, with no failures or ignores. They cover immutable base-URL
overrides, invalid input before dispatch, structured failures, asynchronous-only
injected transport through the blocking facade, and removal of the old flags.
The library and affected CLI suites therefore record 1,320 distinct passes.

All 13 final Torii manifest/PDP controls pass on the corrected retained executable
`e541f638dcbe9f2bff43925c8f0e996c740efd0917741d27df673c27049d48cc`.
They join the coordinated `generation166-append/append-core-build3` capture,
which passes with unchanged captured repository Rust sources and manifests.
The initial failed assertions
and a rejected zero-match selector preflight are retained separately. Together,
the library, affected CLI and selected Torii suites record 1,333 distinct passes,
with no failures or ignored tests in their final runs.

The retried complete CLI/Torii test-target compilation passes in 389.6 seconds
with unchanged selected inputs (`consumers-check-2.json`). The subsequent
storage-only numeric correction passes its rebuilt full library suite separately;
the earlier CLI/Torii results retain their original source scope.
Core's separate runtime qualification belongs to its own evidence record; these
DA results do not attest complete Core/Torii or network behavior.

Raw Cargo streams, before/after source hashes, complete compiler artifact
records, executable hashes and runtime logs are retained under
`target/architecture-redesign/da-manifest-async-2026-09-22/`. The final storage
runtime receipt is `runtime-storage-3.json` (SHA-256
`dccbd3c06120fdca24f17764b6b42dbd4b5d7857f18f787aeb92e01cdde2c6e4`);
the SDK/shared runtime receipt is `runtime-1.json` (SHA-256
`d34277e9373796acc5ee6af6f0f6fc99a9139e45105753e27184c13b326f9d33`).
These development captures do not supply peak-memory, release, native/device or
four-validator evidence.

The remaining DA inventory records 14 synchronous methods, including four proof
operations whose HTTP calls currently omit the account signatures required by
the route catalog. Their migration to the account capability, shared proof DTOs,
signed ingest and storage-owned ingest persistence remains open. The full
first-release redesign and final consistent-candidate qualification remain open.
