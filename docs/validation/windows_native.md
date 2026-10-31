# Native Windows diagnostics

These results are working-tree engineering checks on Windows 11 Pro
10.0.26200, an Intel Core i7-11800H, and Rust
`1.93.1-x86_64-pc-windows-msvc`. Native `cargo.exe` reads the checkout through
the WSL UNC share. Earlier builds used NTFS temporary or private executable
RAM targets; the recorded five-library diagnostic build used a verified cache
in persistent WSL storage exposed through the share. CAR executes in a same-boot
RAM target seeded from verified persistent inputs; those originals remain retained.
Runtime fixtures use Windows
NTFS temporary directories. The executables call native Windows APIs; these
are not Linux tests under WSL. The checkout contains concurrent changes, so this record
does not identify an immutable release candidate or attest a signed artifact.

The table records distinct source snapshots. The recorded lower run includes
the retained journal and staged-output owners and passes all 100 required controls,
including all 69 filesystem controls. Six genuine error-1314 failures keep the
combined gate rejected. Corrected source passes all five focused native controls and
all-targets/all-features filesystem/release strict lint. The recorded Linux lower gate passes all 365 controls
with two intentional child entries ignored, all 58 required controls, 22
documentation tests and strict lint. Shipping CAR/CLI executables and the optional QUIC image
require their own production, unit and integration checks.

| Owner | Observed result | Scope |
| --- | --- | --- |
| Corrected `iroha_fs` library | Recorded native compilation passes; all 267 library controls execute: 265 pass, two genuine symlink fixtures fail with error 1314 and none are ignored. All 69 required controls pass. The fresh Linux library passes 275 controls with one intentional child entry ignored; its release library and helper pass 51 and 39 controls respectively, with one intentional helper child entry ignored. The recorded Linux gate also passes all 58 required controls, 22 documentation tests and strict lint. | Recorded source, lock, independent archives and original streams rejoin; the subsequently corrected source builds a fresh filesystem image and passes all five focused controls. The separate all-targets/all-features filesystem/release strict-lint continuation passes. The full Windows gate remains rejected; these lower-owner results do not qualify CAR, Core, CLI or Node consumers. |
| Recorded adopted-source `sorafs_car` library | Native compilation and all 383 library controls pass, with zero failures or ignores. All 47 required controls pass: 15 native sink, five fragment, four HTTP/fixture, 20 directory-planner and three Taikai identity controls. Source, lock, current image, independently verified gzip archive, persistent seed inputs and original streams remain unchanged. | The CAR gate passes for its recorded source, including the final bounded inventory recheck. The preceding 380-test run exposed one namespace-mutation failure and remains rejected. This result does not qualify shipping callers, moderation/provenance runtime, the changed five-library graph, a whole service or a release. |
| Windows consumer source implementations | The implemented source covers Taikai artifact alias identity, bounded moderation-model reads through retained roots/files, provenance-log locking/publication, private council-key reads and ordinary moderation signing-key reads through final custody checks. | All three Taikai native controls pass on the recorded CAR image. The 11 model and 17 provenance native controls remain unexecuted. The moderation signing-key port adds nine native controls and one shared parser control, also unexecuted; its input buffers zeroize and earlier read/parser errors retain priority over final custody errors. Council-key and real pack/deploy/toolkit/manifest-builder regressions also require final-source compilation and native execution. These source additions are separate from the planner source qualified by the recorded CAR pass. |
| `iroha_measurement` | 108 unit tests pass, one fixture-regeneration test is intentionally ignored; five layering and six public-API tests pass. Strict all-targets Clippy passes. | Real thread/process CPU clocks and peak working-set bytes; native failure and unit-conversion controls. Address-space limits, load average and thermal state remain explicitly unavailable on Windows. |
| CLI transaction-load output | Eight tests pass in a small harness including the exact production Windows output module and the real `iroha_fs` owner. | Bounded private publication, exact original bytes, destination collisions, unsafe paths and empty-output rejection. This does not compile or qualify the complete CLI dependency graph. |
| `iroha_fhe` | The ignored required-AVX2 execution gate passes with `simd`; the BGV, canonical and RNS-chain integration vectors pass with SIMD and with defaults disabled (7, 9 and 4 tests each). | The instruction gate requires real AVX2 execution and rejects scalar fallback. It is separate from performance qualification and whole-node determinism. |
| Native source delivery | Eight real Rust helper tests and ten selected native Python custody/reader controls pass. Strict helper Clippy passes. | Original-object preservation, bounded framing and digest checks, interrupted delivery, destination races, exact authored intent/receipt readback and binary readback through the retained Windows sharing fence. Inert wheel fixtures do not qualify an SDK artifact. |
| Python delivery gates | The five maintained suites pass 75 tests and fail five symlink fixtures with Windows error 1314. All seven native shell-wrapper controls pass. | Actual Windows CPython 3.12.10 from the official NuGet distribution, a real virtual environment, pytest 8.4.2 and Git Bash. Isolation and `venv` assertions run without substituting an embedded interpreter. |
| Executable publication | Three native executable-seal controls pass, including compilation and loading of a genuine tiny Rust DLL. A separate Python-driven transaction through the actual helper also loads that DLL after completion. | Creator-only owner read-and-execute sealing admits native loading. Ordinary receipt sealing retains exact read-only access and refuses DLL loading. This is loader and custody evidence, not the Iroha SDK ABI or authenticated installed-package gate. |
| Recovered executable reader | Eight native controls and four opaque-API compile-fail examples pass. | A retained reader holds a genuine compiled executable through process execution and refuses writable access, receipt-only access, shared identities, junctions and ACL drift. It provides no creator or writer conversion; this foundation does not authenticate an installed SDK package. |
| Immutable flat batch and retained moves | Thirteen native batch tests, sixteen retained-move controls, twenty documentation tests and strict lower-crate Clippy pass with unchanged recorded sources. | All five originals survive the directory seal and reader handoff; four compiled PE fixtures execute while their originals remain retained. Actual writer/delete exclusion, hardlinks, interrupted sealing, parent-only ACL drift and preseal ACL change/restoration execute. These are filesystem component checks; the release publisher and integrated resolver remain separate gates. |
| Canonical release bundle | The recorded 55-control native library run passes 51 tests; four genuine symlink fixtures fail with error 1314. The helper passes 38 tests with one intentional child entry ignored. All 15 library and 16 helper required controls pass; root-exit/live-descendant controls retain admission until whole-Job completion. | Actual PE launches, original writer/delete exclusion, canonical inventory/content checks and publication failures execute. All 22 combined filesystem/release documentation tests pass. Corrected source passes all five focused native controls and all-targets/all-features filesystem/release strict lint. The six combined symlink-privilege failures remain failures; integrated consumer and active release-corridor checks remain separate. |
| Native process owner | All 18 selected portable/Windows process controls pass; strict all-targets Clippy passes. | Creation-time Job containment retains original process, thread, Job and stream handles through whole-Job completion. Standard and asynchronous release wrappers share this owner. The integrated consumer refresh and complete runtime checks remain required. |
| Native profile fixtures | Twelve native controls pass against the exact maintained private-file, peer-shutdown and primary-token helpers; seven applicable Linux controls also pass. | Real private fixture/log custody, original child stdin EOF and actual non-elevated token admission execute. The four-validator profile-network test remains unexecuted. |
| Optional original-file readers | All five focused native reader tests pass after the public optional-reader wrapper is added. | Initial absence stays distinct from later original-custody failure; the wrapper retains the same native owner. This does not execute downstream archive or provider suites. |
| Canonical publication source keeper | Eleven native tests pass; four POSIX-only controls are skipped and the genuine symlink control fails with error 1314. The latest copied Linux scanner/keeper suites pass 117 controls with four existing Windows-only skips, including all 12 retained-reader exception cases. The recorded native results retain their earlier source scope; final-source native qualification remains open. | The existing canonical aggregate reads original retained files and link targets. Native identity, metadata/security snapshots, writer exclusion, bounded inventory and permanent namespace/root-security latches remain held through a publisher call. The structured publisher retains this keeper together with the reviewed registry and independently authenticated entry/bridge owners; these component checks do not establish hermetic Cargo provenance or release-host qualification. |
| Source-registry path identity | All 25 native tests pass against the exact maintained integration suite, including the same golden digest as the Linux run. | A before-fix test reproduced the Windows separator refusal. Native filesystem paths now serialize canonically while Git wire-path validation stays strict. The actual UNC checkout still refuses its WSL symlink, as described below. |
| `sorafs_node` | Native `cargo check --locked -p sorafs_node --all-targets` passes for the integrated checkpoint and directory-publication changes, with unchanged recorded source hashes. | The complete library, binary and test targets compile on MSVC. A subsequent moderation-checkpoint migration still needs a refreshed check. Warnings remain; a metadata check does not execute the suite or establish whole-service qualification. |
| `iroha_core` | Native `cargo check --locked -p iroha_core --lib` passes for the integrated source, including the membership-segment Windows port. All recorded source hashes remain unchanged during the command. | Original Windows parent/file authority and positioned membership I/O compile in the complete Core library. The integrated test-target check below also compiles its test feature graph; runtime, restart-root and State-publication requirements remain open. |
| Integrated consumer test compilation | The complete native `--tests` check passes for Core, SCCP wallet/RPC, test-network and CLI after the profile fixture port. Recorded Rust/manifest inputs and a separate 64-file supplement remain unchanged. | Actual test bodies compile on MSVC. This input subset is not a complete Cargo compilation-input closure, and later proof-observation edits require refreshed compilation. Archive/FASTPQ and CLI Journal runtime controls remain pending; the subsequent canonical release producer/consumer implementation requires matching integrated compilation and runtime checks. |
| `iroha_sccp_wallet` | The complete maintained native library passes 74 tests and fails one genuine symlink fixture with error 1314; the ordinary harness ignores the vector regenerator and a child entry explicitly invoked by its passing parent. Recorded inputs remain unchanged. | Submission/completion and marker reads recheck the exact stored operation before and after their action. Changed bytes, unsafe ACLs and added links refuse before creating markers. The local-NTFS relative-path child passes. Whole-node and final integrated qualification remain separate. |
| `iroha_sccp_rpc` | All 146 tests in the maintained native library pass with zero ignored. Recorded sources, manifests, SCCP fixtures and local vendor inputs remain unchanged. | The real native file-backed header, key and request paths execute in the complete library. This recorded input subset does not establish hermetic Cargo provenance, whole-node qualification or the separate wallet runtime. |
| Daemon volume observations | Six actual native tests pass for the lower filesystem owner; five platform-independent cases and strict lower-crate Clippy also pass on Linux. | Native volume GUID/capacity observations and malformed-path rejection replace the daemon's direct unsafe calls. These observations supply no directory or file custody authority. |
| Daemon stdin shutdown | The repeat executes all six tests of the exact maintained module with real logger/futures dependencies. Its persisted before/after receipt reports unchanged recorded inputs and no observation failure. | Real child input, interrupted reads and EOF drive the production shutdown signal. The repaired driver retains the original child through wait errors and captures source additions; its recorded input subset does not establish hermetic Cargo provenance. Whole-peer supervision remains separate. |
| Test-network ports and process liveness | All 17 tests pass using the exact maintained port-lease and process-observation modules with real filesystem dependencies; recorded source hashes remain unchanged. | Genuine cross-process leases, abrupt exit, socket conflicts, ACL/junction/hardlink refusal, environment-independent OS namespace selection and a terminated child with exit code 259 execute on Windows. Whole-peer supervision remains a separate integrated gate. |
| Checkpoint custody | Eleven native tests pass in the exact production durable-forwarder module with real `iroha_fs` and Norito. Its separately ignored child-probe entry executes through a passing subprocess test. | Original private parent/file/lock custody, foreign ACL rejection without repair, hardlinks, namespace pinning, cross-process exclusion, replacement and optimistic content recheck after staging. This is a module runtime check; subsequent filesystem APIs and the complete SoraFS service require their own checks. |
| Moderation checkpoint custody | Nine native tests pass against the exact production checkpoint module and its maintained tests. Recorded source hashes remain unchanged. | Real `iroha_fs` admission, bounded private reads, competing equal-byte publication, exact readback, ACL and junction rejection, original writer/delete exclusion and publication-error classification. The small harness projects four parent error variants; the complete orchestrator still requires its refreshed crate check. |
| `iroha_config_base` | Fifty native library tests pass; the source-symlink fixture fails with error 1314. | Complete native file identity, source replacement and sharing controls execute. The symlink rejection assertion still requires an enabled runner. |
| `iroha_operation_journal` | All 23 native library tests pass. | Original parent admission, exclusive creation, immutable records, lock retention and bounded recovery inventory. One concurrent Unix-only filesystem edit changed the receipt's aggregate source hash; the Windows implementation and journal sources remained unchanged. |
| Recorded five-library build | Core, CLI, TestNetwork, SoraFS and Torii test executables compile together on MSVC. All five original and preserved images, recorded sources, compiler events and logs remain unchanged after reboot. | One unified dependency-feature graph on the earlier source; later filesystem/CAR, fixture and deadline changes require fresh compilation and runtime evidence. This does not qualify five isolated default builds, immutable compilation or a release source census. |
| Recorded Torii and TestNetwork owners | All ten selected Torii checkpoint/file-store controls and all 69 selected TestNetwork owner controls pass on the recorded batch. | The complete actual inventories are checked before selection. These controls do not execute the four-program release corridor or a complete multi-peer network. |
| Recorded Core and CLI owners | Core selection refuses before runtime because an outer Unix-only include omits all 34 required Kura reader controls. The CLI selection runs all 169 controls: 130 pass and 39 fail. | Thirty-seven CLI failures stem from Kura opening a Windows directory with ordinary file flags, one from a billing fixture retaining a writable handle, and one from missing symlink privilege. Reviewed source corrections remain pending integrated compilation and runtime qualification. No failed result is converted into a pass. |
| Recorded complete SoraFS library | The complete 1,555-test inventory executes: 1,308 pass, 244 fail and three intentional integration entries remain ignored. All recorded sources, images and logs remain unchanged. | Failures include 126 direct private-checkpoint fixture refusals, 99 unsupported chunk-sink calls, two genuine symlink-privilege failures, two late deadline results, one competing-handle lock fixture, three directory-cleanup failures and eleven dependent assertions. The integrated native sink, retained-parent sync, fixture and monotonic deadline repairs need fresh compilation and runtime evidence. |
| Real Kubo integration | Both separately invoked Kubo 0.42.0 tests pass on the same recorded SoraFS image: IPNS publication and signed-head publication. | These two results do not waive the 244 library failures or qualify a changed source candidate. |

The HTTP fixture correction remains separate from directory traversal. Windows
`CarBuildPlan::from_directory(_with_profile)` now uses retained read-only local-NTFS
directory and single-link file owners, canonical UTF-8 ordering and the existing
fixed inventory/path/payload bounds. It observes each file while reading and each
directory around traversal; it does not provide an atomic whole-tree snapshot or
repair input permissions. All 20 planner controls pass, including canonical UTF-8 ordering, original-handle
exclusions, unsafe-input refusals, exact public bounds and final bounded
re-enumeration that rejects added, removed, renamed or duplicate names. Actual shipping pack, deploy, manifest-builder, toolkit, Soracloud
discovery/static-site/Inrou and node streaming-directory callers still require
native qualification. Existing native custody refusals remain enforced.

The accepted CAR receipt is
`target/qualification/windows-native/maintained-native-car-ram-r1-inventory-recheck-gzip.json`
(`764df5c3dee14b619e40f0bce78a92dff56aed625cbf643b2d4a1d75e1f8b513`).
It records one newly compiled native image, fresh complete and ignored listings,
and the whole-suite execution. All four original commands exit zero without an
observation error. The independent gzip verifies the exact 23,146,496-byte PE;
these recorded source and byte checks do not establish immutable execution or
hermetic compiler provenance.

The canonical producer and cached/public consumers now share the flat native
release manifest and retained program/process owners. The structured entry requires
independent entry and bridge provenance pins on both Linux and Windows. Its shared
bridge and synchronous Python refusal supervisor retain originals through known
completion; an unresolved ordinary observation keeps the initiating process alive.
Windows completion includes the entire original Job, while Unix observes the root.
The copied TestNetwork crate passed 72 selected Linux runtime controls; the
projected producer passed 111 Python controls on Linux. On the recorded native
projection, the shipping-named shared bridge passes six controls and its explicitly
invoked child fixture; the helper passes 31 controls and its explicitly invoked
child fixture. Both strict Clippy gates pass. The complete native Python suite
has 112 passes, two genuine `ERROR_PRIVILEGE_NOT_HELD` (1314) failures and no
skips. The failed cases require a module file symlink and a repository target
directory symlink; all six one-shot supervision controls and the real descendant
custody cases pass. Recorded sources, bridge and emitter remain unchanged, with
no observation error. The full producer gate remains rejected. Actual maintained
producer checks after adoption and integrated consumer compilation/runtime remain
pending; these component results do not qualify the release inputs.

The recorded filesystem library executes all 267 controls: 265 pass, including
all 69 required controls, and two native reparse fixtures fail with symlink
privilege errors
(`ERROR_PRIVILEGE_NOT_HELD`, 1314):

- `windows::custody_io::tests::reparse_entries_are_classified_without_following_and_never_removed_as_files`
- `windows::private_files::tests::borrowed_reopen_rejects_native_file_and_directory_reparse_points`

Those tests and the four failing release-library symlink controls must run on a
symlink-enabled native runner; none were skipped or relabelled as passing. The
recorded source, lock, preserved images and original streams rejoin without an
observer error. The release helper passes 38 controls with one intentional child
entry ignored; the release library passes 51 controls and fails four genuine
symlink fixtures. All 22 combined documentation tests pass. Corrected source
passes all five focused native controls and all-targets/all-features strict lint;
the six symlink-privilege failures keep the full Windows gate rejected. The
recorded Linux suites pass 275 filesystem, 51 release-library and 39 helper
controls, with one intentional child entry ignored in the filesystem and helper
suites. All 58 required controls, 22 documentation tests and strict lint pass. The integrated run also covers exclusive creator slots
that retain the original file after failure, cross-parent directory moves and
optional retained readers. A native notification fence now rejects inventory
callback mutations even when directory timestamps or final names remain unchanged.
The new native CI preflight checks both file and directory symlink creation.
This run includes creator-to-mover conversion, immediate file-publication
observation, snapshot-bound private removal, reversible private moves, volume queries,
immutable flat batches, exact Cargo-output removal and OS-selected local-data APIs.

The maintained source-snapshot driver separately rejects the actual WSL UNC
checkout with `source symlink changed kind`. Its sole Git-indexed symlink,
`IrohaSwift/NoritoBridge.xcframework`, appears through the share as reparse tag
`0xa000001d`; native `readlink` returns Windows error 1 (`Incorrect function`).
Linux reads its exact target, `../dist/NoritoBridge.xcframework`, and completes
the 23,500-path census. The native diagnostic preserves this refusal and does
not claim an accepted source digest or weaken the symlink policy.

The five Python failures have the same missing-privilege prerequisite: one
source-delivery mutation fixture, three original wheel-fixture harness tests,
and one source-package admission fixture. The shell-owned wheel harness now
executes its unchanged program from a temporary script, so the native Windows
command-line length limit no longer prevents it reaching the symlink fixture.

The complete maintained native Rust SDK library passes all 12 workflow storage
and 14 engine tests, with zero failures or ignored tests. Recorded sources and
the compiled test image remain unchanged. The actual Linux SDK passes the same
26 controls. These are SDK component results; authenticated installed packages
and the final release corridor remain unqualified. Diagnostic logs are retained
under `target/qualification/`, outside tracked source; recorded local source and
binary observations do not establish installed-package or release qualification.
The final focused Python and genuine-DLL helper-transaction receipt is
`delivery-python-loader-final.log`; the complete five-suite result is in
`python-five-suites-final.log`. `token-privileges.log` records the missing token
privilege.

Reproduce the crate checks in PowerShell from the repository root, explicitly
selecting the MSVC toolchain. Build outputs were kept outside the UNC checkout:

```powershell
$env:CARGO_TARGET_DIR = "$env:TEMP\iroha-windows-native-fs"
$env:CARGO_INCREMENTAL = "0"
$env:CARGO_BUILD_JOBS = "1"
$env:CARGO_PROFILE_DEV_DEBUG = "0"
$env:CARGO_PROFILE_TEST_DEBUG = "0"
cargo +1.93.1-x86_64-pc-windows-msvc test --locked -p iroha_fs --all-targets -- --test-threads=1
cargo +1.93.1-x86_64-pc-windows-msvc clippy --locked -p iroha_fs --all-targets -- -D warnings
cargo +1.93.1-x86_64-pc-windows-msvc test --locked -p iroha_measurement --all-targets -- --test-threads=1
cargo +1.93.1-x86_64-pc-windows-msvc clippy --locked -p iroha_measurement --all-targets -- -D warnings
cargo +1.93.1-x86_64-pc-windows-msvc test --locked -p iroha_fhe --lib --features simd --config profile.dev.debug=0 --config profile.test.debug=0 accel::tests::required_avx2_execution -- --exact --ignored --nocapture
cargo +1.93.1-x86_64-pc-windows-msvc test --locked -p iroha_fhe --features simd --test bgv_vectors --test canonical_vectors --test rns_chain_vectors --config profile.dev.debug=0 --config profile.test.debug=0
cargo +1.93.1-x86_64-pc-windows-msvc test --locked -p iroha_fhe --no-default-features --test bgv_vectors --test canonical_vectors --test rns_chain_vectors --config profile.dev.debug=0 --config profile.test.debug=0
cargo +1.93.1-x86_64-pc-windows-msvc test --locked -p iroha_native_source_delivery --config profile.dev.debug=0 --config profile.test.debug=0
cargo +1.93.1-x86_64-pc-windows-msvc test --locked -p iroha_config_base --lib
cargo +1.93.1-x86_64-pc-windows-msvc test --locked -p iroha_operation_journal --lib
cargo +1.93.1-x86_64-pc-windows-msvc check --locked -p iroha_core --lib
cargo +1.93.1-x86_64-pc-windows-msvc check --locked -p sorafs_node --all-targets --config profile.dev.debug=0
cargo +1.93.1-x86_64-pc-windows-msvc test --locked -p iroha --lib workflow:: --config profile.dev.debug=0 --config profile.test.debug=0 -- --test-threads=1
```

The UNC diagnostic also sets Git's `safe.directory` for the exact provided
checkout through process-local `GIT_CONFIG_COUNT`, `GIT_CONFIG_KEY_0` and
`GIT_CONFIG_VALUE_0`. It does not change global Git configuration or trust a
wildcard. This permits the real inventory command to run; it does not override
the source-symlink refusal described above.

Run the Python gates with a full Windows Python installation and Git Bash
discoverable through `PATH`, after building the helper:

```powershell
$env:IROHA_NATIVE_SOURCE_DELIVERY_TEST_HELPER = "$env:CARGO_TARGET_DIR\debug\iroha_native_source_delivery.exe"
python -m pytest -q scripts/tests/python_native_source_delivery_test.py scripts/tests/python_native_delivery_gate_contract_test.py scripts/tests/python_wheel_byte_owner_test.py scripts/tests/python_source_owner_admission_test.py scripts/tests/sorafs_python_native_platform_test.py
```

The SDK build started only after checking at least 4 GiB free on `C:` and uses
disabled debug information and incremental compilation. The host has far less
than the 50 GiB required by the Windows native bundle build. No native bundle,
authenticated installed SDK package, signed release, complete Core service,
crash/power-loss qualification or macOS Intel execution is established here.
The remaining outcomes belong to the existing owners in
[`windows_intel_completion_goals.md`](../../specs/windows_intel_completion_goals.md).

Read-only volume inspection found only `C:` mounted, with available space below
the 4 GiB threshold used for the SDK test attempt. Task-owned inactive
incremental caches, abandoned compiler temporaries and old SDK code-generation
archives were removed when disk pressure rose; active metadata artifacts,
executables and diagnostic receipts were preserved. The other fixed volume is
an unmounted 1 GB partition with approximately 120 MB free, so it is not a build
destination. No global cleanup, storage provisioning or privilege-policy
changes were performed.

After the integrated Core check passed, only its superseded earlier metadata
artifact was removed; the verified current metadata remains available. The
superseded embedded Python installation and downloaded archives were also
removed after checking that no native Python process used them. The full NuGet
interpreter, real virtual environment and original loader probe were retained.
Each cleanup has a hashed task-artifact receipt alongside the native logs.

Three exact-file NTFS compression passes over inactive task-cache artifacts
recovered approximately 841 MiB. Before/after SHA-256, byte length, native identity and
modification time matched for every file. This preserved the cache contents;
those passes did not change directory policy. A subsequent scoped operation set
compression inheritance on 1,976 task-cache directories. All 10,625 existing
files retained identical bytes, identities, attributes, timestamps and security
descriptors; directory changes were limited to the compression flag.

A separate no-dependency Cargo control passes with its source and target under
a private RAM-backed UNC directory. Actual MSVC compilation and linking,
build-script execution, proc-macro DLL loading, a unit test and executable
execution all succeed; the emitted PE files target x86-64. This establishes a
possible diagnostic build location, not SDK installation or release qualification.
After the native compiler queue became idle, all 10,663 files in the task cache
(2,772,022,550 logical bytes) were copied into a private RAM-backed target. Every
copied SHA-256 matched. Once the original NTFS cache was confirmed inactive,
its generated build files were removed; all 323 executable/DLL artifacts and
all PDB files were preserved with matching hashes in the task-owned RAM target.
The full inventory and cleanup receipt remain available. This recovered enough
space for focused diagnostics (approximately 3.08 GB free on `C:`), while native
filesystem fixtures remain on NTFS. It does not satisfy the 50 GiB release-build
storage requirement or authenticate a packaged SDK.
