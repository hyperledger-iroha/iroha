# Dependency audit

Dependency review 2026-10-28; policy and timestamp follow-up completed
2026-10-29 (Asia/Tokyo). This record covers resolved dependencies,
published advisories, local source review and component validation. A clean
advisory scan does not establish that code has no undisclosed vulnerabilities.
The working tree is concurrently edited; these results are not release
qualification for a later source candidate.

## Advisory coverage

| Ecosystem | Checked graph | Result |
| --- | --- | --- |
| Rust | Root workspace and all nine standalone Cargo roots | Zero RustSec vulnerabilities or informational warnings; no ignores or platform filters |
| npm | All four lockfiles, including extension packaging and EDR tooling | Zero `npm audit` findings |
| Python | 139 lock/input entries, 88 unique PyPI versions; scripts and native CI environments additionally checked with `pip-audit` | Zero known advisory matches |
| JVM/Android | 400 unique resolved Maven versions across five Gradle roots, including buildscript and Android Lint dependencies | Zero OSV matches and zero resolution failures |
| .NET | Restored solution, including transitive NuGet dependencies | Zero published vulnerability matches |

Rust uses `cargo-audit 0.22.1` and RustSec commit
`99670cfc0bc574433034b75876a4b791ccaa2efb` (2026-10-27; 1,380 advisories).
The final root lock has 1,124 packages and SHA-256
`4bab4e970f8b4eb21698ba2c45e06d36af9329cb54ca2a837b1bafc888049a68`.
The root graph has 32 upgrades and one added package; upstream Wayland selects
secure quick-xml 0.41 rather than the retired patch's 0.42. Root and all nine
standalone `cargo update --dry-run` checks report no compatible upgrades.
`deny.toml` contains no advisory exceptions. Vendored Swift/C++ source has
incomplete advisory-database coverage and is checked separately for source
identity and component behavior. Go's fixture module has no external modules.

Two newly published advisory families were removed:

- Guava [GHSA-xxph-c9ww-hj94](https://github.com/google/guava/security/advisories/GHSA-xxph-c9ww-hj94):
  buildscript and Lint classpaths now select `33.7.2-jre`. The standalone Android
  attestation probe also receives the existing reviewed Jackson, Commons Lang,
  jose4j, Bouncy Castle, JDOM and HttpClient tooling constraints.
- MSAL Node [GHSA-xcx6-jx8p-x3h5](https://github.com/advisories/GHSA-xcx6-jx8p-x3h5):
  extension packaging selects `@azure/msal-node 7.1.2` through a narrowly scoped
  override because the current Azure Identity package retains the affected major.
  The extension README records the removal condition and compatibility check.

## Current versions and retained boundaries

The compiler and workspace Rust requirement remain **1.99.1**. This refresh
updates the root and standalone graphs, including crypto-bigint `0.7.10`,
ctutils `0.4.6`, elliptic-curve `0.14.3`, W3F BLS `0.3.0`, Halo2 proofs `0.4.1`,
tonic `0.14.8`, pest `2.9.5` and the AccessKit family. The yanked
crypto-bigint `0.7.6` is gone. W3F API changes are integrated into both the
implementation and deterministic reference tests. Orchard's explicit dependency
profile binds the selected Halo2 version and has a lock-consistency regression.

AccessKit Unix `0.24.1` and AT-SPI common `0.21.1` retain the reviewed zbus 6
migration while rebasing on upstream sources. Local allocation, codec and
hardware-determinism patches remain owned by their documented providers.
The obsolete Wayland scanner patch is retired: upstream `0.31.11` now meets its
recorded quick-xml retirement condition. Source inventories and negative tests
reject reintroducing the retired vendor path. The root fuzz graph now includes the same Metal patch as the workspace, removing
its retired `paste` dependency. The native source snapshot helper uses SHA-2
`0.11.1`. The OpenAPI lock identity follows the root lock.

Some versions remain behind another published major because the maintained
consumer requires them; no advisory is suppressed for these boundaries:

| Dependency | Reason retained |
| --- | --- |
| Ark serialization 0.5 | Required by W3F BLS 0.3 |
| ff/group 0.13 and pairing 0.23 | Halo2curves 0.10 and blstrs 0.7 APIs; native Pasta uses 0.14 |
| Rand ChaCha 0.3 / Rand core 0.6 | W3F/BLS provider boundary |
| generic-array 0.14.7 | Exact crypto-common 0.1.7 requirement |
| matchit 0.8.4 | Exact Axum 0.8.9 requirement |
| psm 0.1.24 | Optional pest 2.9.5 requirement; absent from the active all-features graph |
| JUnit 5.14.4 | JDK 8 compatibility; JUnit 6 requires Java 17 |

The native RustCrypto migrations, maintained PQ providers, Orchard 0.16 and
native Pasta/PLONK stack remain in place. Retired pqcrypto, derivative, paste,
old zvariant/zbus_names and other previously removed unmaintained providers have
not been reintroduced. Norito, pointer ABI, source custody and exact protocol
fixtures remain mandatory.

Other direct updates include EDR `0.25.0`, pytest `9.2.0`, platformdirs `4.13.6`,
zstd-jni `1.5.7-25`, .NET SDK `10.0.403`/runtime `10.0.14`, and the Go fixture CI
line `1.27.x`. Solidity remains `0.8.38`; Node typings continue to follow the
supported Node 22 API surface. Kotlin `2.4.21`, Gradle `9.8.2` and AGP `9.4.1`
remain current. The JDK 8 compile guard is unchanged.

The Python custody recipes bind the refreshed inputs. Pytest's shortened default
parameter IDs are overridden explicitly where exact consumer case identities are
part of custody; all original assertions remain. The Poetry CI tool lock is
resolved for native Linux x86_64 and aarch64. Both architectures have identical
46-package graphs and pass required-hash binary downloads; conditional browser
and Wasm transports are not included.

Swift now binds BoringSSL commit
`3769c110eab4ddbc0c159fadb4f1d0d98b2cf0cc` and Nearby commit
`2cd9371cd0aab2d3ea1c2e74c92563288ea10784`, verified against upstream trees.
All 671 BoringSSL and 1,416 Nearby retained files match their reviewed source
identities. Two Nearby timestamp repairs record separate local-patch hashes while
preserving their original upstream Git blob and SHA-256 identities. BoringSSL
retains upstream assembly guards and runtime dispatch;
Nearby retains its original dependency/submodule pins and the repository-owned
SwiftPM edge to this BoringSSL provider.

Nearby's upstream Abseil wrapper still uses January 2026 LTS `20260107.1`; August
2026 is the newest LTS. The wrapper retains some files removed upstream, so LTS
support does not establish maintenance of every wrapper-only routine. Updating
that independent wrapper remains an upstream compatibility boundary. Nearby's
only published release tag is from 2021; this is a verified source snapshot,
not a new supported Google release. All seven embedded commit OSV queries had
zero matches, with the database coverage limitation stated above.

## Buffered stream authorization

Source review found a shared authorization defect in event, contract-event and
account-history SSE. A stream admitted for data space A could later acquire B,
queue B output, then emit it after B was revoked while A remained. The original
admission check still passed. The affected buffers can contain pipeline metadata,
contract payloads and account movements; fresh projections were already scoped.

The local repair retains every scope observed during the connection and makes
revocation terminal, including after a later regrant. Each consumer checks this
shared authorization before dequeuing. Event SSE additionally applies canonical
per-item scope immediately before emission and preserves fail-closed capacity
and history errors. No authority is inferred from serialized JSON.

A related WebSocket path authorized a whole pipeline batch before awaiting its
individual frame sends. Revocation during a slow send could therefore leave the
remaining frames authorized under stale scope, including an original admission
grant. The sender now checks authorization and canonical scope before each
subsequent frame; an already-started send cannot be retracted. The focused
regression revokes authority after the first awaited send and verifies that no
second frame starts; unchanged authority preserves batch order.

The review also sampled operator/webhook authorization and SSRF controls,
authentication/replay caches, protected-content serving, bounded proxy responses,
P2P pre-authentication admission, credential storage and signed-query execution.
No additional confirmed finding survived those checks. This is bounded source
coverage; it is not an exhaustive review of consensus, cryptography, VM behavior,
all SDK paths, secrets or deployed configuration.

## Validation and qualification limits

Completed checks for this refresh include:

- Rust crypto: 1,991 tests passed with five existing ignored before the W3F
  migration; the final signature suite then passed 335 with two existing ignored,
  including BLS, GOST/SM vectors and allocation controls. AccessKit's common,
  default Unix and Tokio variants passed 24 tests. The source snapshot helper
  passed four Rust tests and 144 Python controls. Orchard's 19 real-proof,
  immutable-vector, mutation and descriptor tests passed. Upstream Wayland's five
  parser/macro tests and all 68 reviewed-vendor inventory tests passed. The IVM
  BN254 suite passed all eight scalar/NEON, bounds and refusal tests.
- npm: extension tests (16), real VSIX packaging and an isolated Azure client
  credential compatibility exercise passed. EDR passed all 568 smoke checks
  without fixture or gas changes.
- Python: 697 source/custody/CI checks and both native lock tests passed; both
  Poetry locks validate. Scripts (20 packages) and native CI (17 packages) also
  passed `pip-audit`.
- Kotlin/Java: all 57 pure Norito codec tests passed with the new zstd binding.
  An additional native adapter run could not load the required authenticated
  ABI-33 bridge; its ten failures are not counted as passes or bypassed.
- .NET: the official SDK archive passed SHA-512 verification, solution restore
  and build passed with zero warnings/errors, and the Petal lane-render regression
  passed. A C# 14 reserved-keyword local was renamed without changing behavior.
- Go fixtures: `go test ./...` passed. Workflow/action, image/release-profile and
  C# consumer guards passed; two stale test expectations were aligned with the
  actual existing native export and Rust toolchain, and all 116 tests in those
  two files passed on rerun. Legacy codec and immutable-action checks passed.
- Swift/C++: BoringSSL passed 1,939 hardware and 1,899 portable crypto tests
  (two platform/vector skips and four disabled upstream cases in each variant),
  plus 484 SSL tests. The rebuilt Nearby library passed 80 UKEY2 handshakes,
  fresh-session, binary-roundtrip, tamper and replay checks and ten malformed
  message refusals. Seven real payload-manager lifecycle cases passed through
  [`test_nearby_payload_lifecycle.sh`](../scripts/test_nearby_payload_lifecycle.sh),
  using an upstream fake channel without radios and the exact SDK dependency lock.
  Four additional exact timestamp roundtrips cover pre-epoch, epoch, historical
  millisecond and year-2100 values. The shared native file provider now reads and
  writes actual modification times, and incoming payloads cache the received
  timestamp before writes; completion restores it. All 26 unchanged upstream
  factory tests pass in isolated temporary directories. The combined source-custody
  suites pass 337 tests (25 Nearby, 23 BoringSSL and 289 Nearby source-seal controls),
  including patch mutation and original-upstream identity controls. Independent
  review verifies the two original blobs against the pinned upstream tree.
- Stream authorization: all 25 Torii component tests passed (13 SSE, eight
  shared authorization and four WebSocket), with no failures or ignored tests.
  The command was `scripts/cargo_fast.sh --stable-local-metadata --incremental --
  test -p iroha_torii --lib --locked -- sse_stream_tests::
  stream_read_authorization::tests:: routing::event::tests:: --nocapture`.
  An isolated Rust harness using the exact production decision/refresh methods
  additionally fails before the fix and passes after it; that separate harness
  uses State/visibility doubles. Independent source review found no remaining
  actionable defect in these changes.

Earlier whole-SDK, full workspace, fuzz and native-proof results bind their own
source candidates. They are not fresh qualification of this dependency refresh.
The full workspace runtime suite, physical devices, whole-network behavior and
native release artifacts still require current-candidate validation.

The current-tree IVM policy scan (`bash ci/check_std_only.sh`) now passes, as do
all 75 guard regressions. The 217 archived-evidence matches are in 58 reviewed
JSON reports, bound by exact path and SHA-256 in
[`ivm_policy_evidence.json`](../scripts/ivm_policy_evidence.json); their original
bytes are unchanged. New or modified reports undergo the ordinary scan, and
artifact checks run before evidence exemptions. The native-target rejection
assertion is recognized narrowly, with positive/runtime mutation controls; its
actual source-identity suite passes all 28 tests in a standalone native harness.
The AES, Arkworks and Halo2curves vendored crates no longer declare alternate
standard-library-free builds. Retired Wasm arithmetic branches, Halo2 build/CI
targets and the bytes concurrency-test exclusion are removed. The guard now
also catches multiline Rust `target_family` declarations. All 68 reviewed-vendor
inventory tests pass after these changes.

The native vendor checks pass: AES has 24 all-feature and 17 portable tests;
Ark-ff's two fixed-vector tests pass with and without its `std` feature; the full
Ark BLS12-381 suite passes 138 tests; Halo2 BN256 passes 98 default-feature and
102 `std`-feature tests; and the bytes eight-thread, 1,000-iteration stress test
passes. Nonmember vendor suites use isolated copies with byte-identical Rust
sources and manifests and the reviewed Ark-ff override. The repository lockfile
is unchanged by these policy repairs. Two existing Ark-ff `must_use` warnings
remain in the standalone build; no test failed or was ignored in these runs.

The earlier source-seal suite passed 112 tests and 414 subtests, with two stale
public-fixture pin tests passing after their reviewed metadata update. No source,
signing, codec, native authentication or allocation guard was disabled.

The Linux CI image recipe still requires a real build, vulnerability scan and
exercise before its digest is promoted. Its previous emulated build could not run
the package-manager sandbox; that sandbox was not disabled. An immutable image
digest establishes identity, not freshness. See the
[CI image qualification procedure](../.github/CI_IMAGE.md) and
[Linux AppImage qualification](../nix-appimage/README.md).

Dependabot covers Cargo, including all six fuzz roots and the three standalone
script crates, plus npm, Python, JVM/Android, Swift, NuGet, Docker and Actions.
Configuration does not prove that server-side update jobs have run. All 30 remote
Action repositories were checked through immutable Git tags or their maintained stable branch.
The install action is updated to `v2.87.32`; release diagnostics and coverage now
supply its required tool inputs explicitly. Its action/installer code is unchanged
from the prior pin. Pin enforcement and the required-tool regression pass.

## Primary sources

- [RustSec advisory database](https://github.com/RustSec/advisory-db)
- [Rust registry](https://crates.io/)
- [Rust release channel](https://static.rust-lang.org/dist/channel-rust-stable.toml)
- [OSV vulnerability database](https://osv.dev/)
- [.NET 10 release metadata](https://dotnetcli.blob.core.windows.net/dotnet/release-metadata/10.0/releases.json)
- [BoringSSL source ownership](../vendor/boringssl-swiftpm/IROHA-PATCH.md)
- [Nearby source ownership](../vendor/nearby/IROHA-PATCH.md)
