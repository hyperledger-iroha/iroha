# Dependency audit

Reviewed 2026-10-24 (Asia/Tokyo). This is a review of repository dependency
manifests and resolved package graphs, not a release or cryptographic security
certification. Public registries and advisory services were queried during the
review. A clean advisory result means no matching published advisory was found;
it does not establish that a package is free of vulnerabilities.

## Current result

- The Cargo lockfile refresh fixes eight RustSec vulnerability matches and one
  Tokio soundness warning. The final scan has zero vulnerability, unsoundness or
  yanked-package matches, with twelve unmaintained-package warnings below.
- All four tracked npm projects have zero `npm audit` findings, down from five
  vulnerable package entries across the SDK and editor extension. Unsupported
  ESLint 8 was replaced by ESLint 10 and a tested flat configuration.
- Both Poetry locks, script requirements, external Python SDK minimum versions,
  the installed validation environment and a Python 3.10 resolution pass
  `pip-audit`. Python 3.9 support was removed from Torii/Norito packaging because
  patched Requests and setuptools releases require Python 3.10 or newer.
- JVM and NuGet advisory checks are clean within their stated coverage. Swift
  retains an upstream-constrained BoringSSL dependency; see below.
- This is **not** a claim that every dependency uses its newest major release.
  Crypto API migrations, reviewed local forks, native artifacts and platform
  constraints remain explicit work.

## Rust

The scan used `cargo-audit 0.22.2` and RustSec database commit
`84d943a6deeb8cc6c6f632531aaf4f1c62275239` (2026-10-23), without advisory ignores
or architecture/OS filters. The compatible Cargo refresh updated more than
250 package names, including additions/removals; `cargo update --dry-run --verbose` then
reported zero further compatible updates before the reviewed SmallVec pin was
restored. That pin is an intentional exception to the resolver's newest-compatible
selection; certificate generation and epoch reclamation pins were separately
advanced to rcgen 0.14.10 and crossbeam-epoch 0.9.21. The declared Rust 1.92
floor and pinned compiler were retained; resolved metadata did not introduce a package
with a declared Rust requirement above 1.92.

| Dependency | Previous | Updated | Addressed advisories |
| --- | --- | --- | --- |
| `rustls` | 0.23.45 | 0.23.46 | RUSTSEC-2026-0346, -0347, -0348, -0349 |
| `ctutils` | 0.4.2 | 0.4.5 | RUSTSEC-2026-0385 |
| `rustls-webpki` | 0.103.15 | 0.103.16 | RUSTSEC-2026-0382 |
| `webpki-root-certs` | 1.0.7 | 1.0.11 | RUSTSEC-2026-0383 |
| `webpki-roots` | 1.0.7 | 1.0.11 | RUSTSEC-2026-0381 |
| `tokio` | 1.52.3 | 1.54.1 | RUSTSEC-2026-0386 (soundness) |

The workspace now requires patched Rustls/Tokio minima. P2P, SoraNet relay and
DNS resolver inherit the workspace Rustls requirement, retaining their crypto
provider features. `clipboard-win 5.4.1`, which was yanked, was replaced by 5.4.3.

The nine root `[patch.crates-io]` overrides remain resolved from local source.
A separate synthetic registry-version scan of their upstream versions found no
published advisory matches. That check does not review Iroha's modifications to
those forks. In particular, `bytes`, `http-body-util`, `axum-core`, `concread`,
`num-bigint` and `aes` carry allocation or acceleration behavior that must be
preserved when rebasing them. The exact-pinned legitimate `arrayref 0.3.9` and
revision-pinned Orchard source remain intact.

Remaining unmaintained packages:

| Package | Resolved version | Advisory |
| --- | --- | --- |
| `atomic-polyfill` | 1.0.3 | [RUSTSEC-2023-0089](https://rustsec.org/advisories/RUSTSEC-2023-0089.html) |
| `audiopus_sys` | 0.2.2 | [RUSTSEC-2026-0150](https://rustsec.org/advisories/RUSTSEC-2026-0150.html) |
| `derivative` | 2.2.0 | [RUSTSEC-2024-0388](https://rustsec.org/advisories/RUSTSEC-2024-0388.html) |
| `paste` | 1.0.15 | [RUSTSEC-2024-0436](https://rustsec.org/advisories/RUSTSEC-2024-0436.html) |
| `pqcrypto-internals` | 0.2.11 | [RUSTSEC-2026-0163](https://rustsec.org/advisories/RUSTSEC-2026-0163.html) |
| `pqcrypto-mldsa` | 0.1.2 | [RUSTSEC-2026-0166](https://rustsec.org/advisories/RUSTSEC-2026-0166.html) |
| `pqcrypto-mlkem` | 0.1.1 | [RUSTSEC-2026-0161](https://rustsec.org/advisories/RUSTSEC-2026-0161.html) |
| `pqcrypto-traits` | 0.3.5 | [RUSTSEC-2026-0162](https://rustsec.org/advisories/RUSTSEC-2026-0162.html) |
| `ttf-parser` | 0.25.1 | [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192.html) |
| `zbus_names` | 4.3.5 | [RUSTSEC-2026-0371](https://rustsec.org/advisories/RUSTSEC-2026-0371.html) |
| `zvariant` | 5.15.1 | [RUSTSEC-2026-0370](https://rustsec.org/advisories/RUSTSEC-2026-0370.html) |
| `zvariant_derive` | 5.15.1 | [RUSTSEC-2026-0372](https://rustsec.org/advisories/RUSTSEC-2026-0372.html) |

The post-quantum packages require a coordinated implementation migration with
cross-SDK vectors and deterministic hardware validation. Proc-macro, GUI/font,
audio and D-Bus warning removal requires updates in their owning dependency
stacks. No new ignore entries were added to `deny.toml`.

Current newer Rust releases outside the resolved constraints include the
RustCrypto/dalek trait families, Arkworks/BLS, Arrow/Parquet, reqwest 0.13,
WebSocket 0.30 and zstd 0.14. Some newest GUI/locking releases require Rust 1.95.
Exact pins for BLST and Unicode data need owner-specific verification before
changing their behavior. SmallVec 1.15.1 is retained because the NFC allocation
proof and its checksum test explicitly depend on that implementation; updating
only the checksum would not revalidate the memory bound. Standalone
fuzz workspaces and captured validation sources are not covered by the root
lockfile scan; externally owned fuzz locks need independent regeneration and
advisory checks.

## JavaScript

Updated the SDK, OpenAPI tooling, Kotodama editor extension and EVM runtime with
coherent npm locks. Notable versions are ESLint 10.13.0, esbuild 0.28.2,
`@scure/bip39` 2.4.0, ethers 6.17.0 and `@noble/curves` 1.9.7. The BIP39 update
removes `@scure/base`; exact dependency-count and bundle assertions now reflect
that real removal. The ESLint migration retains the existing rules and includes
negative lint-policy tests.

Development ESLint requires Node 20.19+, 22.13+, or 24+; the published SDK runtime
floor remains unchanged. Remaining explicit migrations are Noble v2,
TypeScript 7, VS Code languageclient 11 / vsce 4 and EDR 0.24. EDR remains on
0.12.1, which is enforced by the native execution adapter; changing it requires
execution-fixture qualification. Ethers owns older nested Noble versions that
currently audit clean. Node types remain on the intended Node 22 line.

## Python

Poetry locks and script pins were refreshed, with secure SDK lower bounds for
Requests 2.34.2, cryptography 50.0.2, certifi 2026.10.19, urllib3 2.8.0,
setuptools 84.2.1, wheel 0.48.0 and pytest 9.1.1. The lockfiles select the latest
compatible certifi release. Script markers preserve Python 3.10-compatible
BLAKE3 and pycparser versions. Both Python metadata formats and README
prerequisites agree on the new Torii/Norito Python 3.10 minimum.

Resolved advisories included Black 26.3.1, cryptography 48.0.1, certifi
2025.10.5 and urllib3 2.7.0. These are package matches, not assertions that every
affected feature executes here: for example, the Black advisory concerns its
bundled GitHub Action. Wildcard Poetry test dependencies were replaced with
current lower bounds and the deprecated dev-dependency table was consolidated.

## JVM, Android, Swift and .NET

Updated Kotlin to 2.4.21, Gradle to 9.8.2 with official distribution SHA256s,
Bouncy Castle to 1.86, OkHttp/MockWebServer to 5.5.1, zstd-jni to 1.5.7-24,
Netty to 4.1.139.Final, serialization to 1.12.0 and coroutines to 1.11.0.
Bouncy Castle 1.78.1 matched four OSV advisories; the old Gradle 9.3.0 matched
three official security advisories. OkHttp 5.5.1 also blocks compromised eMudhra
intermediate certificates.

OkHttp's Android AAR requires compileSdk 37. Android SDKs and examples therefore
use AGP 9.4.1 / compileSdk 37, while retaining minSdk, targetSdk, JDK 8 API guards
and the native build policy. Instrumentation JNI source registration now uses
AGP's public LibraryExtension API; it retains the same generated native-library
path. Android and test libraries were refreshed within that configuration.
This build requires the Android 37 SDK to be installed.

.NET remains on the supported 8.0 line, updated to SDK 8.0.426/runtime 8.0.32 and
Microsoft.NET.Test.Sdk 18.11.0. All four NuGet solution projects pass the
all-transitive vulnerability scan. OSV checks of 31 direct Maven coordinates and
22 resolved core-runtime coordinates found no findings; this is not an audit of
every Android plugin transitive dependency.

Swift remains constrained by Nearby's current upstream requirement
`BoringSSL-SwiftPM >=0.7.1,<0.8.0`: the lock's 0.7.2 is the latest satisfying
release, while upstream offers 0.41.0. Nearby and Abseil revisions remain
immutable. OSV commit queries returned no findings for these three revisions,
but coverage of embedded crypto code is incomplete. Updating BoringSSL needs a
Nearby migration or a separately maintained and qualified fork. A fresh Swift
native build remains required.

JUnit 5.14.4 is retained because JUnit 6 requires Java 17; Netty 4.1 is a
maintained security line. xUnit remains on its current 3.x release. Pinned Rust,
cargo-ndk and NDK source-custody inputs were preserved.

## Validation

- `cargo metadata --locked --format-version 1` and compatible-update dry run.
- `cargo audit --deny unsound --json`: zero vulnerabilities/unsound/yanked;
  twelve maintenance warnings, without ignored advisories.
- `scripts/cargo_fast.sh --stable-local-metadata --incremental -- check
  -p iroha_p2p -p sorafs_orchestrator -p soranet_pq --lib --locked`: passed.
- `scripts/check_no_legacy_codec.sh`: passed.
- Rust futures/PQ unit suites: 19 and 217 passed respectively, with two PQ
  tests ignored. No protocol bypasses or ignore-list changes were introduced.
- NFC source/checksum allocation test: passed with the reviewed SmallVec pin.
- `cargo test -p mv --test ebr_allocation_custody --locked`: six passed with
  crossbeam-epoch 0.9.21, preserving the actual reclamation/refund assertions.
- OpenAPI: all 154 tests passed after canonical pin regeneration (including
  the 19 lock-owner tests).
- JVM dependency-focused transport/crypto/Norito/Java-consumer tests: 115 passed;
  tools: 25 passed. Android managed compilation, sample AAR metadata checks
  and CycloneDX SBOM generation pass with compileSdk 37.
- .NET restore and solution compilation passed, with 2 integration tests passed.
  Broader initial JVM tests reported 1,580 passed / 472 failed and .NET tests
  reported 2,105 passed / 1,519 failed; native bridge and fixture prerequisites
  dominate those failures. These suites are not reported as passing.
- All four npm projects: clean install without lifecycle scripts, package-lock
  audit and outdated-package review. SDK distribution build, bundle budgets and
  ESLint pass. Browser/crypto/type/bundle tests: 108 passed; lint/engine-policy
  tests: 62 passed; editor extension: 16 passed; EDR: 10 passed; EVM Python
  assertions: 2 passed. BLS finality fixtures reproduce byte-for-byte.
- Python: both `poetry check --lock` runs pass; `pip check` passes; five pure
  Python wheels build. Focused Norito/Torii tests: 717 passed plus 260 subtests.
  Python 3.10 binary-wheel resolution passes; it is not a Python 3.10 test run.
- SDK native-dependent tests require fresh authenticated bridge/addon/wheel
  artifacts. Existing artifacts have missing ABI or source-provenance errors.
  No authentication or codec fallback was introduced to make tests pass.
- The broad Python script suite was still running at report time; it is not
  counted as passed. Mypy 2.4 and the prior 1.19.1 report the same two existing
  assignment errors across 46 SDK files, with no new type-check regressions.
- Full Rust workspace, real-network and device qualification have not passed as
  part of this review. `cargo fmt --all -- --check` finds unrelated existing
  source formatting differences; dependency changes do not reformat that work.

The shared checkout was concurrently committed and merged by another actor.
That merge introduced unrelated conflicts and a stale internal lock dependency,
which Cargo metadata reconciled. Existing changes and unresolved conflicts were
preserved. The OpenAPI lock pin was regenerated through its canonical owner;
its release provisioner additionally requires matching committed HEAD,
index and working files, so an uncommitted pin is not release-qualified. All
154 OpenAPI tests pass after regeneration.

## Continued maintenance and sources

Dependabot now covers all four npm roots, JVM/Android roots, Swift packages,
NuGet and Python SDK/CI manifests alongside existing Cargo, Docker and action
coverage. Configuration does not establish successful server-side runs or
provide Gradle advisory coverage by itself.

Registry version inventories: [crates.io](https://crates.io/),
[npm](https://www.npmjs.com/), [PyPI](https://pypi.org/), Maven Central, Google Maven
and NuGet. Advisory and compatibility references:

- [RustSec advisory database](https://github.com/RustSec/advisory-db)
- [Rustls secret-debugging advisory](https://rustsec.org/advisories/RUSTSEC-2026-0346.html)
- [ESLint support policy](https://eslint.org/version-support/)
- [ESLint 10 migration](https://eslint.org/docs/latest/use/migrate-to-10.0.0)
- [Requests advisory](https://github.com/advisories/GHSA-gc5v-m9x4-r6x2)
- [setuptools advisory](https://github.com/advisories/GHSA-h35f-9h28-mq5c)
- [Gradle daemon advisory](https://github.com/gradle/gradle/security/advisories/GHSA-mvvg-497x-hmj8)
- [OkHttp changelog](https://square.github.io/okhttp/changelogs/changelog/)
- [Nearby package manifest](https://github.com/google/nearby/blob/main/Package.swift)
- [Dependabot ecosystem support](https://docs.github.com/en/code-security/reference/supply-chain-security/supported-ecosystems-and-repositories)
