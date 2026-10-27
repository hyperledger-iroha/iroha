# Dependency audit

Reviewed 2026-10-25 (Asia/Tokyo). This record covers dependency resolution,
published advisories, implementation migrations and component validation. It is
not a release qualification or a guarantee that dependencies have no undisclosed
vulnerabilities.

## Resolved dependencies

The root Cargo graph has **zero RustSec vulnerabilities and zero informational
warnings**, including maintenance and soundness warnings. The scan uses
`cargo-audit 0.22.2`, database commit
`21d909e4c8bd4e14c7aea58bf980c881a0053ab1` (2026-10-24), no advisory ignores,
and no architecture or OS filters. `deny.toml` has no advisory exceptions.
All six standalone fuzz graphs also resolve and audit with zero vulnerabilities
and zero maintenance warnings. All 23 fuzz binaries compile with their locked
graphs and the host Xcode toolchain. The strict smoke campaign passes all 13
selected targets with 50,084 total executions under the unchanged per-target
4,000-run/30-second limits. This is bounded smoke coverage, not exhaustive fuzzing.

The final root lock contains 1,123 packages and has SHA256
`6c3773ce4ebc9c61cacb7a17e937e6f729dfa0318f40624ee0376bcf718d8fcc`.
Its last adjustment adds only the `mochi-ui` dependency on the already-resolved
egui package to enable upstream's opt-in emoji fonts. All package versions,
sources, checksums and other dependency edges are unchanged by that adjustment;
the matching OpenAPI lock pin and the final RustSec scan were refreshed.

The twelve previously reported unmaintained packages have left the root graph:

| Retired package family | Current implementation |
| --- | --- |
| `pqcrypto-internals`, `pqcrypto-mldsa`, `pqcrypto-mlkem`, `pqcrypto-traits` | RustCrypto `ml-dsa 0.1.1`, `ml-kem 0.3.3`; original deterministic SoraNet protocols retained |
| `paste`, `derivative` | Maintained macro/provider stacks; reviewed Arkworks, Halo2 and Metal patches use `pastey 0.2.3` |
| `atomic-polyfill` | Current upstream dependency graph |
| `audiopus_sys` | `opus 0.4.0` and its current native binding |
| `ttf-parser 0.25` | Current GUI/font stack |
| `zbus_names 4`, `zvariant 5`, `zvariant_derive 5` | Native accessibility migrated to `zbus 6.1.0` |

The earlier refresh also removed the reported Rustls/WebPKI/root-certificate,
Tokio and ctutils advisories and the yanked `clipboard-win` version. Updating
JWT to `jsonwebtoken 11.1` with its AWS-LC provider avoids introducing the
optional RSA implementation with its outstanding advisory.

## Rust migrations and retained boundaries

The compiler and workspace Rust requirement are **1.99.1**. Active build and
native-source checks use its actual compiler identity. Historical captured
receipts retain the compiler under which they were produced.

Current direct API families include Rand 0.10, RustCrypto digest 0.11,
SHA-2 0.11, SHA-3 0.12 plus SHAKE 0.1, AES 0.9, ChaCha20Poly1305 0.11,
HKDF/HMAC 0.13, P-256/K-256/SM2 0.14, Ed25519-dalek 3,
Curve25519-dalek 5, crypto-bigint 0.7, Syn 3, parking_lot 0.13,
reqwest 0.13, WebSocket 0.30, zstd 0.14, Arrow/Parquet 60,
rodio 0.23, eframe 0.37, PyO3 0.30 and JNI 0.22.

Orchard is pinned to the published `0.16.0` source, with Halo2 proofs 0.4,
gadgets 0.6 and Pasta 0.6.1. The native Pasta/PLONK stack uses ff/group 0.14
and fallible Rand 0.10 traits. Orchard's circuit-description digest remains
unchanged; its source identity and dependency profile are updated explicitly.

ICU4X 2.4 uses Unicode 18/CLDR 49. Exhaustive scalar normalization comparison
and the decomposition/allocation census were rerun before updating the profile
fingerprint. SmallVec 1.16.4 has byte-identical source files to the reviewed
1.16.3; its archive checksum is bound by the NFC source audit. Stable-sort
allocation bounds were reviewed against Rust 1.99.1 sources.

Local patches retain Iroha's actual allocation custody and portable/hardware
behavior. `bytes 1.12.1`, `http-body-util 0.1.5`, `num-bigint 0.5.1` and
AES 0.9.3 were rebased rather than replaced by unmodified registry sources.
The Ark BLS subgroup check retains the same public-constant equation without
introducing GLV heap allocation. Native accessibility patches use real D-Bus
messages and the maintained upstream wire stack. New patches contain
`IROHA-PATCH.md` provenance where applicable.

The remaining older versions are imposed by maintained upstream APIs or exact
upstream requirements, rather than advisory suppressions:

| Dependency | Reason retained |
| --- | --- |
| Ark serialization 0.5 | Required by the current `w3f-bls 0.2` provider |
| ff/group 0.13 and pairing 0.23 | Required by current Halo2curves 0.10 and blstrs 0.7 boundaries; the native Pasta stack uses 0.14 |
| Rand ChaCha 0.3 / Rand core 0.6 | Required at the W3F/BLS provider boundary |
| generic-array 0.14.7 | Exact requirement of upstream crypto-common 0.1.7 |
| matchit 0.8.4 | Exact requirement of current Axum 0.8.9 |
| psm 0.1.24 | Optional exact requirement of pest 2.9.4; absent from the active all-features workspace build graph |

Maintained transitive consumers can also require older compatible hash traits.
The dormant Vega manifest and historical fuzz campaign receipts are not active
root dependencies. Standalone fuzz manifests now use current libfuzzer-sys and
arbitrary versions and the applicable root source patches.

## Other ecosystems

All four npm projects have zero `npm audit` findings after clean installs.
Noble curves/hashes/ciphers are 2.4, BIP39 is 2.4, TypeScript is 7.0.2,
ESLint is 10.13, esbuild is 0.28.2, VS Code languageclient is 11,
vsce is 4 and EDR is 0.24. The EDR adapter preserves committed execution and
gas fixtures. Node type definitions intentionally follow the supported Node 22
API surface. SoraFS package custody checks retain exact original registry
archive integrity, content, source projection and package recipe checks.
Supported npm runtime floors now require Node 22.23.4; OpenAPI tooling retains
its stricter Node 24 floor. The Node 22 floor and rejection cases were exercised
on the actual minimum runtime as well as the host runtime.

The EVM compiler is now Solidity 0.8.38. The official compiler archives for
Linux x86_64, Linux aarch64 and macOS were verified, and the macOS compiler
reproduced the canonical artifact. The complete contract record, including
creation/runtime bytecode, ABI and immutable references, is unchanged; only
compiler/input provenance changed. All 140 compiler-corridor tests and all 568
EDR checks pass without changing gas or behavior fixtures.

Python Poetry locks, script requirements and SDK minimums were refreshed.
Requests 2.34.2, cryptography 50.0.2, certifi 2026.10.23, urllib3 2.8,
setuptools 84.2.1 and wheel 0.48 remove the reported matches. Torii/Norito
packaging consistently requires Python 3.10 or newer. Both locked environments,
script dependencies and the Python 3.10 resolution pass `pip-audit`;
Python 3.10 resolution is not a runtime test. Mypy now passes all 46 SDK files.
The certifi SDK requirement remains `>=2026.10.19`, above the advisory's patched
version; current lockfiles select 2026.10.23.
The separate native CI requirements lock was also regenerated with current
versions and artifact hashes: all 17 packages audit clean, hash-required binary
installation passes, and its two exact metadata/lock tests pass.

Kotlin 2.4.21, Gradle 9.8.2, AGP 9.4.1, compileSdk 37 and NDK r30 LTS
are current. Bouncy Castle is 1.86, OkHttp is 5.5.1, Okio is 3.18.2,
Netty is 4.2.19 and zstd-jni is 1.5.7-24. The JDK 8 API compile guard,
minimum Android versions and native authentication remain enforced.
The expanded advisory check covers 438 unique resolved Maven package versions
across all four Gradle roots, including application, build-plugin, detached and
Android Lint classpaths. The affected Jackson, Commons Lang, jose4j, BC, JDOM
and HttpClient build-tool dependencies were upgraded; the final scan reports
zero advisories and zero resolution failures. Real CycloneDX tasks, all 421
managed Android tests and positive/negative Lint controls pass.
JUnit 5.14.4 preserves JDK 8 compatibility because JUnit 6 requires Java 17.

Swift uses immutable current Nearby and BoringSSL source revisions. The latest
Firebase wrapper still embeds April 2025 crypto sources, so a small local Swift
package binds unmodified BoringSSL commit
`90c6b370770adce64ff435789bfd1940067df415` (2026-10-22). It uses upstream's
generated source list, assembly guards and runtime CPU dispatch. Per-file
hashes, license files, refresh ownership and build commands are recorded in
[`vendor/boringssl-swiftpm/IROHA-PATCH.md`](../vendor/boringssl-swiftpm/IROHA-PATCH.md).
This is repository-owned packaging, not a new Firebase release. The real Nearby
product links and its UKEY2 handshake, tamper and replay controls pass. Nearby's
local package manifest points directly to this provider: a sibling override in
the SDK did not propagate to nested Swift package consumers. The repaired graph
resolves correctly through an ordinary three-level consumer. All 1,416 upstream
Nearby files retain their pinned bytes, with identical target/source membership;
only repository-owned packaging changes the dependency edge. Exact source and
release guards retain missing, changed, extra and aliased-input rejection.
Nearby's
current Abseil wrapper pins the January 2026 LTS (20260107.1), which remains
within upstream's critical-fix support period; the newest LTS is 20260817.0.
The wrapper also retains older files removed upstream. Some emit linked hash
and random-pool routines, although no callers were found in the current Nearby
source or compiled objects. Official LTS support does not establish maintenance
of those wrapper-only routines. This is a retained upstream dependency boundary,
not a claim that every embedded file is the latest release. Embedded-source OSV
coverage is incomplete.

.NET targets supported .NET 10, SDK 10.0.402/runtime 10.0.13,
with xUnit v3 4.0.2. NuGet restore, compilation and all-transitive advisory
checks pass. Native fixtures must come from the same current source candidate.

Nix/AppImage sources replace the unsupported NixOS 22.05/FUSE 2 graph with
NixOS 26.05, maintained Type2 runtime, Squashfuse 0.6.5 and libfuse 3.18.2.
Immutable source archives and NAR hashes were verified. The bundle now points
at its actual `iroha3d` executable. Seven source-policy tests pass. A disposable
ARM Linux guest also passes locked Nix evaluation, the static PIE runtime and
Type 2 structure checks, and the real smoke-image extraction and FUSE launch
paths. The runtime uses upstream-compatible Clang/compiler-rt; the GCC startup
failure found during qualification was repaired. Ubuntu's user-namespace
restriction remained enabled, with a guest-only AppArmor profile allowing the
exact test image to create its required namespace. This qualifies the ARM
bundler smoke, not the full daemon image or the x86_64 execution path. See
[the Linux qualification commands](../nix-appimage/README.md).

GitHub Actions use current immutable release commits. The fuzz toolchain is
pinned to nightly 2026-10-23, which supports the workspace's current Rust
requirement. CI container jobs now receive an immutable image reference only
after a host-side preflight validates it. The builder recipe pins Rust 1.99.1,
official grcov 0.10.8 archive hashes and a required-hash Poetry 2.5.1 environment;
its 48 resolved Python packages have zero OSV matches. Both supported Linux
architectures pass binary-only package resolution. Building, scanning and
exercising the updated Linux image before promoting its digest remains required.
The attempted image build in an ARM Linux guest reached Arch Linux's package
manager, whose sandbox could not run under x86 user-mode emulation. The sandbox
was not disabled, so this is not a qualified image build. A valid
digest establishes identity, not freshness or absence of vulnerabilities. See
[the CI image qualification procedure](../.github/CI_IMAGE.md).

## Validation

Completed component checks include:

- The complete locked Cargo workspace passes `cargo check --workspace
  --all-targets --keep-going --locked` through the stable local metadata wrapper.
  This includes test and benchmark compilation; existing warnings remain, so
  it is not a strict Clippy result or a full workspace runtime test.
- Crypto: 1,979 unit tests passed, five existing ignored; SoraNet PQ: 214 passed,
  two existing ignored. Forced-software Keccak also passed all 214 tests and
  four immutable ML-KEM/ML-DSA vector tests.
- Orchard: all 18 current real-proof, immutable-vector and mutation tests pass.
  P-256 AIR: all 16 arithmetic, differential, mutation and erasure tests pass.
  Bootle/Jindo/Falcon sampler and RNG filters: all 42 tests pass.
  Poseidon: all 28 current field-boundary and hash tests pass. FastPQ resource
  preflight, fallible entropy and erasure checks pass all three focused tests.
  Current PQ/SM2 integration tests pass 56 cases, borrowed ML-DSA passes six,
  and wallet resource/faucet controls pass four.
- Native Pasta: 149 tests passed, six existing ignored. PLONK: 441 unit tests
  passed, 19 existing ignored; all six current golden-shape integration tests
  passed. Native gadgets pass 243 tests with ten existing ignored tests.
  Existing native proof-byte vectors remain unchanged. Recursive verification
  passes 38 ordinary tests; the two remaining descriptor tests now pass after
  aligning stale expectations with the committed norm-product layout. The
  production Omega-cap test also passes and rejects the measured oversized
  configurations; production bounds remain unchanged. Both full compact-proof
  component tests pass with optimization, debug assertions and overflow checks
  enabled, including every-cell tampering, changed public inputs and known/unknown
  layout parity on Pallas and Vesta. Their source, binary and 97-dependency closure
  hashes match the tested candidate; production bounds remain unchanged.
- Account-name normalization and scratch-allocation controls: all 37 focused
  tests passed with the final SmallVec source checksum. Procedural macros:
  126 unit tests passed across model, schema, telemetry, Iroha and Norito derives.
  Current data-model UI tests pass all 11 cases without changing expected
  compiler diagnostics.
- Ark BLS: 138 upstream tests passed, including fixed-multiplication subgroup
  parity; crypto allocation controls pass. Bigint: 170 upstream tests passed;
  existing Iroha primitive allocation tests also pass.
- AES FIPS vectors pass on hardware and software; real D-Bus/AT-SPI and
  accessibility tests pass. TLS loopback trust modes, zstd codec tests and
  JNI exception/thread-attachment probes pass.
  Torii's current authentication/JWT suite passes all 76 tests, including
  actual signatures, Ed25519/P-256 validation, zero/high-S rejection and exact
  JWT labels; all 5,883 Torii library tests compile.
- The current VOLE dependency graph passes 396 tests with one existing ignored
  test. All-target checks pass for SCCP RPC, SoraFS node/orchestrator and P2P.
  Integer migrations preserve both samplers' tables and 7,202 decay inputs;
  256 independent full-width arithmetic cases and the P-256 high-limb regression
  pass. Restoring reversed modulus limbs fails the latter regression.
- JavaScript: 237 focused SDK tests, TypeScript CLI/consumer checks, lint and
  bundles, 16 extension tests and real VSIX packaging passed. The complete EDR
  smoke has 568 passing checks. SoraFS custody: 839 passed; native tooling:
  228 passed, with actual original package archives checked.
  The final source-authenticated addon passed the complete 4,120-test unit
  profile on the actual minimum Node 22 runtime, with no skips, plus all 18
  native bond tests.
- The complete current Rust client library suite passes 1,263 tests with three
  existing ignored tests. The compiler resolver crash was removed by explicit
  test imports; strict parameter decoding and its native error cause remain
  tested. The affected CLI modules pass all 25 tests, and Mochi's exhaustive
  private-query batch rejection control passes.
- The upgraded optional Mochi GUI passes all 77 runtime tests, with one existing
  ignored real-network preview. The migration preserves interaction and icon
  assertions, explicitly consumes or releases headless texture updates, checks
  the selected font face and enables the maintained bundled emoji-font feature.
  Mochi, Kagami and the daemon also build together against the updated control
  API. The macOS bundle manifest and staging assertions pass. After the
  read-only executable handoff below, the actual packaged Mochi `--help` also
  passes without observer intervention. The explicit-provider changes below
  also remove the observed validator-startup panic. The final actual run passes
  the entire ordinary developer flow: all three contract forms execute across
  four peers, restart preserves them, context isolation succeeds, and reset
  removes the executed old state before fresh deployment and execution.
  Publication now uses a separate fresh network explicitly created with
  `--with-storage`, after the ordinary network's authenticated cleanup succeeds.
  Its nine focused fixture tests pass; provider/finality/cold-consumer assertions
  remain unchanged. The complete matrix/smoke assertion still fails: native
  storage-service bootstrap exceeds its unchanged 30-second startup deadline,
  with custody readback returning HTTP 503 `stream_token_custody_unavailable`.
  The publication network's cleanup also succeeds. No startup deadline,
  authentication requirement or finality/custody check was relaxed.
- Executable publication now replaces its writable creation anchor with a clone
  of the already-authenticated read-only descriptor before closing the writer.
  The same inode remains held continuously, and all snapshot, hash, permission,
  alias and substitution checks remain enforced. This avoids the documented
  [Linux `execve` rejection of executables open for writing](https://man7.org/linux/man-pages/man2/execve.2.html).
  The new write-refusal regression fails on the old implementation; all 67
  focused CLI, Mochi and namespace custody tests pass with the handoff.
- P2P TLS and QUIC constructors explicitly select their existing Ring provider,
  following Rustls' [explicit provider API](https://docs.rs/rustls/latest/rustls/crypto/struct.CryptoProvider.html#using-a-specific-cryptoprovider).
  This removes the startup ambiguity introduced when Reqwest also enables
  AWS-LC. All 69 focused transport, listener, streaming and datagram tests pass
  with both providers enabled, including TLS 1.3/ALPN enforcement and wrong-key
  rejection. Trust, certificate pinning and replay protections are unchanged.
  The resolver, VPN helper, relay and optional SoraFS proxy likewise select their
  existing providers explicitly. Four resolver, three VPN and four relay tests
  pass under the combined provider graph. Optional-proxy validation also exposed
  a non-special URL parsing mismatch, final QUIC replies dropped during shutdown
  and noncanonical macOS test paths. The authority parser now accepts canonical
  public hosts while rejecting alternate numeric/escaped addresses; DNS address
  filtering remains enforced. Final replies retain connection custody until
  acknowledged or the existing ten-second timeout expires. All seven focused
  proxy regressions and then the complete 24-test proxy module pass, including
  unchanged authentication and file-custody refusal assertions. These four
  additional owners pass 35 unique tests. The daemon's matching test-only TLS
  helper change is reviewed but was not separately runtime-tested in this suite.
- The final daemon admission/SCCP fixture checks pass all 29 tests, and seven
  deployment readiness, phase-storage and private-program fixture checks pass.
  All six genesis-policy tests pass with the fixture bound to the authenticated
  proposal's network identity; strict rejection causes and original manifest
  inputs remain checked.
  The full deployment run recorded 1,295 passes, 15 failures and five existing
  ignored tests. Every failure now has a passing focused rerun: the authority,
  generated-runtime, journal and native-bootstrap/history groups pass 118 tests;
  the seven fixture checks above and the AMX path control cover the remainder.
  The long history test exercises all 64 natural expirations and rejects oldest
  material substitution. One earlier group's source check correctly refused a
  concurrent Rustdoc-only edit; reversing exactly that sentence reproduces its
  original source hash. The refusal and runtime results are both retained.
- Kotlin: the complete current native core suite passes 2,053 tests, tooling
  passes 25, and Android host JNI consumers pass 47, with no skips or failures.
  Updated Netty/Okio checks and all 421 managed Android unit tests passed.
  The complete .NET native SDK suite passes all 5,808 tests with no skips.
  Core/JNI and .NET were repeated against the same newly authenticated bridge.
  The final bridge also passed all 2,476 Swift tests with strict source checks
  before and after execution. Its SHA256 was
  `c3d13dc9cf3ec69efcf5f732d065431a378650c8898565463956cdf415867dcc`.
- Swift dependency qualification: 1,936 upstream BoringSSL crypto tests with
  assembly, 1,896 with the portable backend, and 484 SSL tests pass (two
  existing platform/vector skips and four upstream disabled crypto cases).
  All 23 source-integrity controls pass. The actual Nearby product link passes;
  ten real UKEY2 handshakes, 80 binary roundtrips, 40 tamper rejections and
  40 replay rejections passed.
- Python: Poetry lock checks, dependency consistency, five pure wheel builds,
  717 focused Norito/Torii tests plus 260 subtests and Mypy passed.
  The four installed package owners pass 5,203 tests plus 284 subtests.
  The final SDK run passes all 3,044 tests and strict installed-byte/source
  checks before and after execution. Its isolated Requests worker explicitly
  uses `-B` to preserve authenticated package contents; all 109 real-process
  deadline and custody controls pass, without cleanup after testing.
  The broad script run completed with 8,436 passing tests, 16,638 passing
  subtests and three existing skips. Its 29 failures were in four files changed
  during the dependency/CI migration; a fresh complete rerun of those four files
  passes all 2,352 tests.
- Source/signature validation prerequisites were repaired without bypasses:
  GPG-backed source capture/retention tests passed (104 tests, 66 subtests),
  native package/source guards passed (438 tests, five subtests), and reviewed
  CI/compiler/source inventories were updated while retaining negative cases.
  The full Sumeragi mutation-script suite passes 1,961 tests. Current target,
  release/toolchain, Kotodama and Taira policy guards pass 584 tests and
  9,252 subtests; the shipping-source inventory passes 11 tests and 26 subtests. The cryptographic operation inventory matches the source;
  new instruction classifications retain injected-verifier rejection controls.
  The actual Rust 1.99.1 data-model test build uses 36 compile units, below the
  unchanged 41-unit baseline and 44-unit permitted limit.
- Current CI image, release-profile, workspace gate and shipping-feature policy
  tests pass all 428 cases; release-profile/prebuilt-provenance tests pass all
  163 cases. Release bootstrap hashes were updated after reviewing their exact
  source deltas, retaining source substitution, permission and signature
  refusals. The broader clean-source release seal was not relaxed or refreshed
  to admit unrelated working-tree changes.
  After the Nix closure helper migration, all 312 release-profile,
  prebuilt-provenance and feature-graph checks pass.

The optional privacy-feature integration target also compiles with its locked
graph. The maximum Orchard fixture test and all 15 focused P-256 integration
tests pass. The complete macOS developer smoke remains unqualified at native
storage-service startup, despite its passing ordinary packaged workflow.
The completed native SDK runs precede the final GUI-only lock edge described
above and the later transport/proxy repairs. Their package versions and non-GUI
dependency edges are unchanged, but their source records bind the original
candidate and are not release attestations for newly rebuilt artifacts.
External cleanup removed the previously qualified JavaScript addon and
Swift/JVM/.NET bridge directories, some original generated inputs and JVM XML
outputs. Successful saved test logs
remain evidence of the completed runs; they do not establish a fresh source
verification or the uncompleted whole-SDK external-consumer check after deletion.
Interrupted or build-lock-waiting runs are not counted as passes. Shared source
is concurrently edited, so component evidence does not qualify a different
later candidate. Full Linux daemon AppImages, physical devices and whole-network
release qualification require their own current-candidate runs.

Release source-custody tools require matching committed source. Local diagnostic
builds may authenticate the current working tree through supported source seals;
they do not replace a clean-source release attestation. No signing, native-load,
codec or source-authentication gate is disabled for this migration.

## Maintenance and primary sources

Dependabot covers Cargo (including all six fuzz roots), the four npm roots,
Python, JVM/Android, Swift, NuGet, Docker and action dependencies. Configuration
alone does not establish successful server-side update runs.

- [RustSec advisory database](https://github.com/RustSec/advisory-db)
- [Rust registry](https://crates.io/)
- [Rust release channel](https://static.rust-lang.org/dist/channel-rust-stable.toml)
- [ESLint 10 migration](https://eslint.org/docs/latest/use/migrate-to-10.0.0)
- [Gradle security advisories](https://github.com/gradle/gradle/security/advisories)
- [OkHttp changelog](https://square.github.io/okhttp/changelogs/changelog/)
- [Nearby pinned source](https://github.com/google/nearby/tree/c80c38efe4b8a5fa72586b58d381bb546702f062)
- [BoringSSL pinned source](https://boringssl.googlesource.com/boringssl/+/90c6b370770adce64ff435789bfd1940067df415)
