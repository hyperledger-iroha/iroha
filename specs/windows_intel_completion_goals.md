# Windows and Intel completion goals

This record coordinates the platform work under the existing SDK/release owners,
[DX1–DX7](kagami_mochi_devex_goals.md), [ZK C.2/E.1/H.3/V.4](zk_delivery_plan.md)
and [Parliament B6](parliament_ballot_implementation_goals.md). It does not replace
their acceptance requirements. The active goal is to complete the implementation,
native execution and installed-release work below; it remains open while any
required outcome is unverified.

| Goal / owner | Current result | Remaining completion evidence |
| --- | --- | --- |
| WI1 — `iroha_fs`, DX1 | The recorded Windows lower suite passes 281 filesystem controls, 51 release-library controls and 38 helper controls, with six genuine error-1314 failures and one intentional child entry ignored. All 116 required controls, including all nine directory-mutation controls, 22 documentation tests and strict lint pass. The copied Linux sealed-image candidate passes 381 controls, all 74 required controls, 23 documentation tests and strict lint, with three intentional child entries ignored. | The combined Windows gate remains rejected until all six failing fixtures pass on a symlink-enabled native runner. New borrowed cross-parent publication and strict retained-input owners require fresh lower validation; qualify affected final-source consumers and preserve separate crash/power-loss and complete-service requirements. See [native Windows scope](../docs/validation/windows_native.md). |
| WI2 — CLI/Core retained evidence | The earlier completed native graph records Core 238 passes/75 failures and CLI 186 passes/38 failures. All 16 marker crash/recovery pairs, 19 native strict-bound progress, 11 membership and new Inrou/workspace/host-boundary controls pass on that recorded source. The recorded native check of the five library test configurations passes, including the corrected archive call; it does not produce runnable test images or qualify the changed dependency graph. | Rebuild and execute the integrated repairs for FASTPQ sharing, geometry creation, pipeline ancestry, finalized archives, tiered geometry and native fixtures. New persistence, reader and credential ports require all existing and added controls. Genuine symlink privilege remains required; no earlier image or component result qualifies changed source or the complete CLI graph. |
| WI3 — Rust workflow storage, SDK S3/S6/S10 | Journal extent admission and reading retain the same original file; zeroizing buffers survive the final directory check, and runner leases retain original ancestry. The actual Linux and native Windows SDK libraries each pass all 12 storage and 14 engine tests; those results retain their recorded source and image scope. C# package instructions name the implemented ABI 33. | Repeat affected Rust workflow checks when integrated sources change. Run installed C#/JS/Python native packages through their existing authenticated build/delivery owners; qualify private SDK storage, not only Rust primitives. |
| WI4 — Intel arithmetic, C.2 | Actual AVX2 execution passes on the i7-11800H under WSL2 and native Windows MSVC. SIMD/scalar independent vectors and instruction-versus-model controls pass on Linux; required qualification rejects scalar fallback. IVM SHA-NI exercises 462 production blocks. See [source and execution scope](../docs/validation/intel_avx2.md). | Maintain the mandatory instruction checks on native runners. Metal/CUDA and whole-node determinism keep their separate qualification requirements. No Intel instruction test substitutes for a named performance host. |
| WI5 — resource observation, E.1 | The recorded maximum proof completes and independently replays at 9,412,944 bytes, 1,752.654 seconds and kernel peak RSS 8,298,721,280 bytes, with a real 32 GiB address-space limit. The unchanged 300-second target and live observation failure keep that run rejected. Corrected observer source passes fresh normal publication and all 25 unchanged public/native methods, including the 19 `NativeRunTests`; the independent 39-case suite retains its separate scope. Canonical ordered SHA residue emission feeds MAIN directly; the copied candidate passes production/default-test compilation, 103 ordinary tests and the independent oracle covering all 49 providers. The final test-only collecting adapter annotations pass a separate production check. | Qualify final integrated and installed delivery, reliable full-invocation observation and maximum proving time under unchanged limits. Windows has no `RLIMIT_AS` equivalent in this schema; unavailable address-space, load-average and thermal metrics remain explicit. See the [maximum-proof record](../docs/validation/maximum_x509_intel.md). |
| WI6 — Parliament B6.5 | [Supplementary i7 scalar/accelerated proof-vector evidence](../docs/validation/parliament_ballot_intel.md) is recorded separately. | Run the exact Xeon Silver 4310 baseline: one physical core at 2.10 GHz, turbo off and idle SMT sibling. Keep named phones, thermal/battery, cancellation and independent review requirements intact. |
| WI7 — release owners, DX6/DX7 and SDK S6 | Existing native bundle and C# delivery workflows cover Linux/macOS ARM64 and x86-64 plus Windows x86-64. The focused `platform_native` workflow adds filesystem, mandatory AVX2 and full Python delivery-control suites. | Execute clean same-source native bundles, authenticated installed SDKs and signed releases using the existing owners. Native Windows needs at least 50 GiB build storage; available C: storage is below that requirement. macOS Intel and exact reference-host latency evidence remain separate. |
| WI8 — SoraFS and Python delivery owners | Windows two-slot custody checks original exact SID/DACL state; native tests exposed and repaired stable-compiler metadata, directory-sync and mandatory-lock ordering defects. Python delivery has a persistent retained native transaction, distinct executable sealing and a Windows-compatible original-file reader. A genuine DLL loads after a Python-driven native transaction completes. CLI, daemon, Kagami, Izanami and SoraFS consumers use the shared stable native metadata owner. | Complete full-crate native Windows two-slot/provider checks and authenticated installed Python qualification. The broader delivery controls retain actual symlink assertions that require a privileged runner. Keep the reviewed Unix-only external-software-signer transport policy explicit; this work does not qualify a new Windows authenticated signer transport. |
| WI9 — confidential gas calibration, Phase C M2 | The workspace enables Criterion CSV reporting with an offline-resolved lockfile; actual metadata confirms the reporter feature and dependency. The [calibration ledger](confidential_assets_calibration.md) preserves historical Apple-host scope. This is not a new benchmark result, and available i7 instruction parity does not change committed gas accounting. | Capture neutral and AVX2 Criterion baselines on the specified Xeon Gold 6430 (neutral also permits AMD EPYC 7B12), with deterministic seed, complete raw artifacts, telemetry and tolerance checks. Preserve the separate Graviton3 NEON requirement and named-host acceptance rules. |
| WI10 — native filesystem consumers | Native owners replace length/time identity guesses and empty permission checks in Config, CLI token/release-signing, SoraFS checkpoint/policy/provider/operator and SCCP journal/key consumers. The earlier five-library graph completes its recorded queue: all 69 selected TestNetwork and 22 Torii controls pass; Core and CLI retain the failures in WI2. The full Node suite records 1,553 passes, two error-1314 failures and three intentional ignores. All source, parent and image final checks pass; completed execution does not turn failed suites into accepted gates. | Validate the implemented repairs on fresh integrated images. Both previously recorded real Kubo publication/restart/tamper controls pass; final-source rerun remains required. Moderation/model/provenance/council-key and shipping consumers remain unqualified. The separately recorded integration/private-settlement smoke and SCCP results retain their source scope. Native Taira workspace/check controls pass; complete the remaining consumer audit below; a supported compile target does not imply every runtime operation is implemented. |
| WI11 — release-prebuilt native qualification host | The canonical producer and public/cached consumers share one flat immutable bundle, a Norito JSON manifest and original source/tool/lock custody. Retained standard/asynchronous children use creation-time Windows Job containment. The structured Python entry requires independently pinned entry and shared-bridge artifacts; ordinary observation failures retain its initiating process and originals until completion can be established. A recorded prior native producer candidate executes real artifacts and passes its non-symlink controls, docs, strict Clippy and fixture builds; genuine symlink-privilege failures keep its full verdict rejected. The recorded prior TestNetwork/integration/CLI test-target graph, including private-settlement smoke, compiles. These earlier component and compilation results do not qualify the changed combined source. | Complete native integrated consumer runtime tests and resolve the genuine symlink-privilege prerequisite. Run all four actual shipping executables and the authenticated release corridor on the same source candidate. Native source-link admission and independent artifact trust remain prerequisites. Darwin original-image launch remains unresolved. |

Configured telemetry integrity checkpoints use retained private Windows directory and
lock originals, bounded reads, and exact-predecessor atomic publication. Hash-chain,
sink-binding and pending-record validation remain common with Unix. Integrity is
enabled by default, while `state_dir` defaults to `None`; this is support for
explicit durable checkpoint configuration. Native checkpoint, recovery and custody
controls still require execution on the final integrated source.

Torii webhook persistence now has a retained Windows backend with private queue
custody, bounded canonical reads and durable publication/removal recovery. Optional
governed APS storage retains private root/staging/lock originals through restart,
canonical validation and atomic retirement; its protocol and reservation logic are
shared with Unix. Remote repair credentials, restricted auditor files and retained
network-bundle inputs also have native owners; bundle identity uses mandatory
platform-tagged snapshots. All new native controls and their full consumers remain
unexecuted on the integrated source. These ports do not change the separate
Unix-only external-signer or coordinated host-administration policies.

## WI11 release-prebuilt native host

The source owners are `iroha_fs`, `iroha_release_bundle`, `iroha_process_bridge`,
`iroha_test_network` and the structured
`scripts/run_release_pipeline.py prepare-sumeragi-programs` operation.
The implemented ownership contract is:

- One canonical manifest binds the native target, source and Cargo.lock digests,
  tool transcripts, exact four executable names and native custody policy.
  Original directory and file identities supplement content authentication;
  they are machine-local invocation identities, not consensus artifacts.
- The immutable flat batch retains every child original across the directory
  seal and reader handoff. Publication holds source, tool, lock and original
  parent owners through exact-byte verification and durable completion.
- Public and cached resolvers return retained programs. Commands, synchronous
  and asynchronous children, and reapers preserve complete release admission.
  Diagnostic paths cannot replace these owners. Partial release identities
  refuse before developer overrides, caches or optional tool lookup.
- The shared process bridge retains each original session. The Python entry
  transfers source/registry/entry/bridge owners before spawning and synchronously
  supervises unresolved ordinary observation errors; it does not return a live
  unknown session to a CLI that can exit. Windows observes the complete Job;
  Unix observes the original root. Forced interpreter or machine termination
  is outside this ordinary-error contract.

Completion still requires the final native integrated suites and actual four-
program release corridor, including owner/DACL drift, absent execute access,
writer/delete exclusion, reparse/hardlink rejection, equal-byte substitution,
source/tool/lock drift and publication failures. Independently trusted entry and
bridge artifacts cannot be replaced by hashes generated from test fixtures.
The Windows UNC source-link refusal and missing symlink privilege remain real
failures. On Linux, the separately copied sealed-image candidate passes genuine
recursive Cargo/Rustc controls through two build-script levels. Anonymous
kernel-sealed originals provide executable authority; a retained descriptor or
named filesystem witness alone is insufficient. Darwin original-image launch
remains unsupported. This isolated result does not qualify the combined source
or the four-program release corridor; component DLL, source-delivery and copied-
consumer results do not close those gates.

## Consumer and runtime audit

| Owner | Required outcome / current boundary |
| --- | --- |
| `iroha_config_base::toml` | Complete native source identity detects replacement and identifies hard-linked include cycles consistently; Windows originals exclude writers and deletion. Native identity/sharing controls pass. The real source-symlink fixture still requires a symlink-enabled runner. |
| CLI transaction output, billing, Taikai, onboarding token and release toolkit | Retain original evidence identity, reject unsafe secret ACLs and publish without clobbering an existing object. Linux output/billing checks pass for their recorded source; subsequent token/toolkit changes need integrated tests. The Windows Taikai alias check compares two retained attribute-only file identities before writes; all three native controls pass on the recorded 383-test CAR image. |
| SoraFS rooted storage and atomic checkpoints | Original directory/lock/file custody, single-link identity, protected current-owner Windows ACLs and conservative postpublication errors remain mandatory across PDP, PoTR, forwarding and ingest. The rebuilt Windows Node library completes with 1,553 passes, two genuine error-1314 symlink failures and three intentional ignores; its full gate remains rejected. Both current-image real Kubo controls pass; recorded Linux library/Kubo results retain their separate scope. The historical CAR source passes all 383 controls, including all 47 required and all 20 native directory-planner controls. The local-NTFS planner retains bounded, canonically ordered read-only originals; this is not an atomic tree snapshot or permission repair. Shipping packing/deployment, manifest-builder, toolkit and Soracloud discovery/static-site/Inrou callers still require qualification after the reviewed repairs. The native CLI sparse inventory helper passes its isolated control and strict lint; the final shipping binary and large fixture remain unqualified. |
| SoraFS provider/store/orchestrator | Shared native custody replaces empty permission checks and length/time identity guesses. Store staging, cross-parent publication, rollback and recursive retirement retain original directory authority. Moderation retains its original checkpoint parent and treats all errors after publication as uncertain; its exact production helper passes nine native controls. Optional checkpoint reads distinguish initial absence from later custody loss. Windows implementations add bounded model reads through retained directory/file owners and provenance-log locking/publication custody. Ordinary signed-moderation commands retain the private key original through bounded zeroizing reads, shared parsing and final custody checks. Their nine native controls and one shared parser control await execution; the external signer transport remains Unix-only. All 11 new model and 17 new provenance native controls remain unexecuted. Compile and execute these owners and the final complete consumer graph; the nine checkpoint controls do not qualify them. |
| Remaining native consumer implementation | The composed source covers operator-key files, SoraFS handshake/token/cache files, provider adverts, direct command outputs, browser capability output, optional QUIC file bridges, direct lane artifact reads, retained geometry journals and retained process working directories. Verified fetch spools retain atomic publication through an owned private staging file. Ordinary pipeline/FASTPQ append and generic indexed-sidecar rewrites retain original Windows files through creation, publication and recovery; the shared rewrite policy and Unix publication behavior are preserved. Held-predecessor replacement uses native POSIX semantics and rechecks displaced originals before release. Strict bound progress uses the same canonical intent and recovery algorithm with a retained Windows storage owner; all 19 native controls pass after the count-marker repair; changed-source and complete consumer qualification remain required. Production activation remains a separate admission and recovery-custody TODO. Shared recovery now retains the initiating error before later classification. Compile and execute the final combined source, all affected existing and new owner controls, and shipping consumers. Ordinary direct outputs may remain partial after I/O errors; implemented source alone does not qualify runtime or release behavior. |
| Core membership progress storage | Native Windows positioned I/O, original directory binding and single-link segment creation compile in the complete Core library, retaining the creator handle before fallible checks. All 11 native membership controls pass after the count-marker repair; changed-source and complete recovery qualification remain required. Existing restart/authenticated-root TODOs are independent requirements and remain open. |
| Core native-context archive and FASTPQ source storage | The Windows source candidate now retains original archive/root custody, bounded immutable reads and creator-only no-replace publication. Retry cleanup verifies exact staged bytes and the selected full native snapshot; all crash boundaries retain explicit native rename states. The rebuilt native context archive group records 36 passes and one default-identity fixture failure; all 23 receiver FASTPQ source-archive controls pass. The distinct FASTPQ artifact store records six passes and 29 sharing failures. Native fixture and retained durability repairs require integrated execution. |
| CLI data space deployment | Public profile export has a Windows retained-input/private-output candidate. The native Journal, preflight and authority paths are now wired with an original-handle refresh transaction, explicit Plan recovery and a reader/Apply barrier. The Unix exchange implementation remains intact; both hosts reject unresolved native transaction markers before consuming a journal. The eleven native refresh controls pass on the recorded image; the broader deployment tests retain private geometry creation and genuine symlink-privilege failures. Repaired-source qualification remains pending. Pure retirement/Inrou validators and the executed-genesis fixture are now shared across host configurations; their focused Linux runtime checks pass. Unix host administration remains explicitly gated. |
| Taira Inrou custody and host boundary | Native online qualification reads, workspace creation and read-only stage checking use retained native directory/file owners, complete identity, bounded copying and private no-replace output. Source originals survive final stage checks; ordinary failures preserve the first error and may leave owned private partial output. All eight native qualification-reader, nine portable owner and two early host-boundary controls pass, along with the modeled local fixture; no live Taira operation is implied. Coordinated staging still requires Unix validator-config binding and canary actions require the Unix inherited-descriptor protocol; an early Run guard refuses unsupported hosts before context access, staging or network side effects. External software-signer transport remains a separate Unix-only policy. |
| Native proof source identity | Native path components now serialize to canonical Git census names, including restored references and resolved symlink targets; strict Git byte validation is unchanged. The maintained source-identity test source passes 29 Linux and 25 Windows controls through direct native compilation, including an identical fixture digest. A recorded Linux checkout census passed; the existing active Git merge now prevents a new strict release census. Native Windows refuses the WSL UNC representation of the tracked Swift bridge symlink because the host cannot read it as a symlink. A native checkout with representable source links and compiled registry freshness remain required. |
| Other consumer triage | The missing test-only Torii `Write` import is corrected. The earlier recorded five-library graph completes and all 22 selected Torii controls pass, including ISO reference-data/Torii gateway compliance. Changed integrated sources still require runtime qualification. SCCP journal, key and file-backed RPC-header readers retain native private custody; headers keep their byte bounds, zeroizing buffers and sensitive HTTP values. The maintained SCCP RPC library passes all 146 native tests. The wallet passes 74 native tests after repairing exact-operation guards and its relative-path fixture; its genuine symlink control fails with error 1314. Both runs preserve recorded inputs; final integrated qualification remains required. Canonical release-prebuilt admission is implemented for native Windows; the complete WI11 runtime and release corridor remain unqualified. The optional APS live-test controller and Torii route-control feature remain Unix-only, separately from external-signer policy and the four-program WI11 owner. Native Windows loss/hold/heal and restart qualification remains open; selected TestNetwork owner tests do not establish this corridor. |

An attempted SDK rerun stopped before Cargo because Windows memory fell below the required 36 GiB reserve; no compilation or test pass is claimed for that attempt. After the recorded five-library test-configuration check passed, that integrated check stopped before telemetry because the tmpfs reserve was short; telemetry and daemon checks remain unrun. Windows release-build storage remains below its required 50 GiB. Persistent Windows diagnostic caches remain available after reboot; the previous
Linux RAM cache was lost and cannot be reused under its old identity. A fresh
Linux build requires a complete new input graph and sufficient peak storage.
Diagnostic storage does not satisfy native release or immutable-source requirements. Only inactive
artifacts owned by this task may be removed during recovery; sources, qualification
receipts and unrelated concurrent builds must be preserved.

Native focused commands:

```sh
cargo test --locked -p iroha_fs --all-targets -- --test-threads=1
cargo test --locked -p iroha_fs --doc
cargo test --locked -p iroha_config_base --lib
cargo test --locked -p iroha_operation_journal --lib -- --test-threads=1
cargo test --locked -p iroha_plonk --test source_identity
cargo clippy --locked -p iroha_fs --all-targets -- -D warnings
cargo test --locked -p iroha_fhe --lib accel::tests::required_avx2_execution -- --ignored --exact --nocapture
cargo test --locked -p iroha_fhe --lib --test canonical_vectors --test clearing --test rns_chain_vectors
cargo test --locked -p iroha_fhe --no-default-features --lib --test canonical_vectors --test clearing --test rns_chain_vectors
cargo test --locked -p iroha --lib workflow::storage
cargo test --locked -p iroha --lib workflow::engine_tests
cargo test --locked -p iroha_cli_lib --lib transaction_load::output::
cargo test --locked -p iroha_core --lib canonical_evidence
cargo test --locked -p iroha_sccp_wallet --lib journal::tests
cargo test --locked -p iroha_sccp_wallet --lib pure::evm::tests
cargo test --locked -p iroha_sccp_rpc --lib endpoints::tests
```

The host is Windows 11 Pro on an Intel i7-11800H with a WSL2 Linux environment.
Windows MSVC tests execute Win32/NTFS calls directly. WSL2 instruction results
are hardware-backed but do not establish bare-metal OS or named Xeon timing
behavior. Generated test logs, source projections and packages remain untracked.
Missing privilege, unavailable hosts, insufficient build storage, source drift
and failed checks remain visible; none is converted into a passing platform gate.
