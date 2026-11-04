# Current standalone Parliament resource accounting

The current Rust 1.99.1/AES 0.9.3 source candidate is bound to the unchanged
standalone logical-work and requested-allocation formulas. The source review
maps 39 unchanged protocol Rust files and 17 reviewed differences to the prior
request-accounting review. Comparator cases require test/diagnostics builds.

Eight fresh compile-only probes cover portable and accelerated AES on ARM64
macOS, iOS and Android, plus x86-64 Linux musl. Each provider requests 960 bytes;
portable alignment is eight bytes and accelerated-wrapper alignment is 16.
Exact LLVM outputs, commands, compiler/package/std-library input hashes and
before/after source checks are retained. All 683 recorded inputs stayed stable.
No target-device code was executed.

An independent review of six Rust alloc source files preserves the existing
Vec reserve/append/collect requested-Layout formulas and conservative old/new
reallocation overlap. The exact current sources and historical diff are gzip
compressed here. Allocator internals, rounding, stack and process memory remain
excluded. AES/Keccak internal work is not priced by API-level block counts.

The 21 focused tests pass, including nine provider-evidence mutation controls.
Every historical numeric case is identical. The normal report exits zero;
`--require-complete` emits the same report and exits one. All five open production
terms remain, and `profile_qualified` and `complete_certificate` remain false.
The source is a shared dirty checkout, not clean/index-equal release provenance.

`receipt.json` records the source review and command results; `SHA256SUMS` binds
all retained files. Historical B6 artifacts have not been rewritten.
