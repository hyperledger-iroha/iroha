# Sumeragi lane-context foundation — September 16, 2026

This is bounded development evidence for the [six open redesign goals](../../../specs/sumeragi_liveness_redesign_goals.md). It does not qualify network liveness or a release. The [previous root status paragraph](sumeragi-liveness-status-before-lane-context.md) is preserved verbatim with its SHA-256 file.

The fifth four-validator attempt finalized the transaction admission at height five, then failed the 600-second finality deadline. All three surviving validators continued timeout certificates through seven views. The next native lane author was the stopped validator. The existing view-change path depends on an already-created payload and cannot replace that silent initial author. All four validators exited normally; restart was not reached.

The replacement foundation freezes exact post-carrier lane authority and the oldest unresolved admitted atomic group. State owns canonical admission order `(first carrier height, canonical index)`; replay preserves that position. Contexts persist across unrelated global advances and close or reopen when their exact head or applied predecessor changes. A compact complete-set commitment, including the empty set, is bound into the execution witness and retained with global finality. Native consumers must authenticate the current complete set and each opening carrier before it can authorize signing.

| Checkpoint | Result | Scope |
| --- | --- | --- |
| Finalized-state reducer 03 | 224 passed | Pure shared reducer and explicit external-finality anchor |
| Lane-context 01 | Build passed; 31/37 passed | Initial frozen context and authority fixtures; six failures retained |
| Lane-context 02 | Build passed; 38/40 passed | Corrected network identity; two fixture failures retained |
| Lane-context 03 | Build failed | Test used nonexistent `NetworkId::default`; no tests ran |
| Lane-context 04 | Build passed; 95/96 passed | Rank, head replacement, snapshots, compact proof, Kura retention, and existing timeout regression; one catalog-baseline fixture failed before authority resolution |
| Lane-context 05 | Build passed; 95/96 passed | The same authority fixture fails earlier because lane-specific governance is unsupported as an autoscale base; unchanged 1,649 inputs, original failure retained |
| Lane-context 06 | Build failed; no tests | New native DTO integration lacked schema/codec traits and shadowed macro paths; 1,653 unchanged inputs, errors retained |
| Lane-context 07 | Build passed; 106 Core + 58 data-model tests passed, zero ignored | Authenticated current-set reader, historical opening binding, State-guard release, snapshots, native BLS/QC/TC/WAL codec and all prior foundation controls; 2,185 unchanged inputs |
| Lane-context 08 | Build passed; five affected Core reader tests passed, zero ignored | Missing historical opening is an explicit error; current publication race and snapshot controls remain passing; 2,185 unchanged inputs |
| Lane-context 09 | Build failed; no tests | Model-owned enum JSON declarations lacked explicit tags; 2,189 unchanged inputs, original diagnostics retained in local ignored `lane-context-checkpoint-07` |
| Lane-context 10 | Build failed; no tests | Public native value DTO lacked required Copy implementation; 2,189 unchanged inputs, original diagnostics retained in local ignored `lane-context-checkpoint-08` |
| Lane-context 11 | Build passed; 110 Core + 66 model tests passed, zero ignored | Canonical model ownership, all prior foundation controls, native physical WAL/fsync/replay and pre-payload TC recovery; 2,189 unchanged inputs |
| Lane-context 12 | Build passed; all ten affected Core and nine model tests passed, zero ignored | Signed native availability commitment, manifest substitution and old-signature rejection, physical WAL replay controls; 2,189 unchanged inputs |
| Lane-context 13 | Build passed; 454 Core and ten model tests passed; Torii 57 passed and one failed, zero ignored | Model-owned admission DTOs and existing request/journal/routing policies; 2,626 unchanged inputs. Public missing-transport fixture signed Ordinary intent and failed before its intended transport boundary. |
| Lane-context 14 | Build failed; Rust tests not run | Complete input/publication/pre-dispatch sizing integration reached seven test-only sealed-transaction path errors; 2,628 unchanged inputs. Source-contract baseline exposed three stale declarations. |
| Lane-context 15 | Build passed; Core 437 passed and five failed; all 61 Torii and 13 model tests passed, zero ignored | Corrected sealed paths and four reviewed source declarations; 2,628 unchanged inputs. Failures cover one changed authentication-observer fixture and four Kura/recovery paths; original logs retained. |
| Lane-context 16 | Build passed; Core 481 passed and eight failed; all 61 Torii and 13 model tests passed, zero ignored | Actual unsigned carrier sizing and single-body gossip; 2,629 unchanged inputs. New tests exposed invalid relay identity, missing route authority, illegal chunk-count and all-zero-signature fixture setup; four prior recovery failures remain. |

| Lane-context 17 | Build passed; Core 477 passed and 12 failed; all 61 Torii and 13 model tests passed, zero ignored | Actual signed 160 KiB gossip delivery/replay/retry and one-byte carrier framing boundaries pass. The unchanged 800 KiB input exposes the complete decoder’s insufficient allocation budget. The authenticated startup fixture affects nine tests; ABA finality setup and completed publication repair also remain failing. |

| Lane-context 18 | Core/Torii test binaries compiled; Core 487 passed and two failed; all 61 Torii tests passed, zero ignored; model-test compile failed | Shared bounded decoder fixes the actual 800 KiB batch; all previous carrier and gossip checks pass. Authenticated cold recovery and eight merge-frontier checks pass. Model fixtures called private `Header::write`; production compiled. Certified-slot ABA retirement and completed publication repair remain failing. |

| Lane-context 19 | Combined library-test build passed; all 16 admission model tests and 17 scoped formal tests passed, zero ignored | Model fixture header mutation corrected; Core and Torii artifacts are byte-identical to checkpoint 18, with its 487/489 and 61/61 behavioral results reused explicitly. Exact 1 MiB decode and 1 MiB+1 rejection pass. |
| Lane-context 20 | Combined library-test build failed; no tests run | The completed-repair implementation compiled as a library, but two old test assertions referenced the renamed marker field and one new fixture used an invalid hash-array conversion. All 2,629 captured inputs stayed unchanged. |
| Lane-context 21 | Combined build passed; 133/133 selected Core and 61/61 Torii tests passed, zero ignored | All eight new completed-repair/origin controls and the full real economic recovery/two-cycle replay regression pass. All 17 scoped source checks pass; the unchanged model executable retains its 16/16 result. Live certified archival remains an invalid positive fixture and is being split from actual completed archival. |

Checkpoint 04 captured 1,649 unchanged local source/include/config inputs. Its Core test executable SHA-256 is `38d83c4ea7add01897260cd85d7fa60b8f77cacc3570335271508a3fc0048742`. These captures do not attest transitive external binaries. Local ignored evidence is retained under `dist/sumeragi-liveness-redesign-20260916/lane-context-checkpoint-01` and `lane-context-checkpoint-02`, with inventories, original failures, patch/source copies and hashes. Network evidence is retained under `network-checkpoint-05` in the same parent directory.

The replacement is not yet driving production lane votes. Native runtime and transport integration, migration of economic entry points, retirement of independent lane voting, and unchanged-candidate four-/seven-validator fault qualification remain outstanding. A lane Decision certifies immutable admitted executable input and AMX role; the global merge Decision continues to certify execution against an exact global state base and canonical results.

Checkpoint 05 Core executable SHA-256 is `79ac337870d82f39b865853aa813245311b7f92720b6950f5067e68dfd4fab71`; its evidence and exact source are retained in local ignored `lane-context-checkpoint-03`. The subsequent fixture uses the legal default autoscale profile and installs validator rules matching its actual alias and governance before native scale-out. Its result passed in checkpoint 07.

Checkpoint 07 is retained under local ignored `lane-context-checkpoint-05`. Core SHA-256: `88a20f5975aad88286f797b6762429a06ee55939fced03674a3c2b554e17e753`; data-model SHA-256: `f377cf71721ad26a9aff354ef09383ea4ab08486fe70f165c8a2af3ba0c503d2`. The authority fixture now passes. Native opening and codec tests remain boundary tests: no replacement lane driver or network qualification is claimed. Checkpoint 08 subsequently passed all five authenticated-reader tests, zero ignored, on 2,185 unchanged inputs. Missing required historical opening custody now returns an explicit error; only an actual in-progress current publication can return Pending. Core SHA-256: `4059e6d7c67e497933649b7a9689e77a37d01a7c6099c88bd844c4ca58879daf`. Exact evidence is retained in local ignored `lane-context-checkpoint-06`.

Checkpoint 11 is retained under local ignored `lane-context-checkpoint-09`. Core SHA-256: `1a76281c832ccc23b82780b1dad898bdb381f628d8a37912cc64a21f92d0d2e7`; model SHA-256: `7261fb309afadb5e1c3c4fc0e77fd9f1b6e6ee20a243d9dc03a62d2c7433444d`. The physical native WAL test advances three survivors with real BLS and no payload, reopens after fsync-before-ack, retransmits the retained TC after another reopen, and advances a fourth reducer that missed all initial traffic. A second control rejects foreign-key intents and complete-frame corruption. It does not invoke the live lane driver.

Post-checkpoint review found that the native Commit value did not yet bind all manifest fields: a nonzero same-layout chunk-root substitution could preserve QC signatures. The subsequent correction adds a canonical availability hash to the voted value and checks it at manifest/proposal/Decision projection. Its root, same-stripe length, valid layout/count, highest-Prepare and old-signature mutation controls pass in checkpoint 12. This is a pre-integration correction, not a diagnosed deployed failure.

Checkpoint 12 is retained under local ignored `lane-context-checkpoint-10`. Core SHA-256: `e616daaef9c50a5a7cc0a1556876d97307aa6273d5583b233c6b05bdac00f284`; model SHA-256: `052ae0985d2f0b8a5741824ccef276e7ce32026f10064ef96455107e1b2d415a`. The combined build completed in 452.53 seconds; all 2,189 captured inputs remained unchanged through both focused suites. This checks the affected native value/manifest/signature boundary and disk-backed reducer recovery; live-driver integration remains outstanding.

Checkpoint 13 is retained under local ignored `lane-context-checkpoint-11` with the original Torii failure. The combined build took 1,031.04 seconds. Core SHA-256: `db044905154acf28a22570bf8b76078d99806a466e6b17e6fb96fea420bbd058`; model: `30463d0e31ea7020b6f785f069957cdae59e7b7a1ad1578e3bc54de8aca0635b`; Torii: `fb2ae0e0f38f45033641609109f7cdd25869bffc68b839d50a93bbb7a935d546`. The early emitted Core artifact matches the final combined build exactly, and all three test suites retain unchanged captured inputs. Ten scoped semantic-request source controls and the codec guard passed. The next candidate corrects only the failing fixture’s signed admission intent and introduces complete-input custody and pre-dispatch per-control sizing; it has not yet been tested. These counts do not close production lane failover or complete signed DA feasibility.

Checkpoint 14 is retained under local ignored `lane-context-checkpoint-12`. The combined build failed after 390.28 seconds; all captured inputs stayed unchanged. The sealed transaction fixture referenced `transaction` instead of its actual `transaction::signed` owner. The broad pending-membership/publication source test invocation was stopped after the positive baseline identified three stale reviewed token declarations (90 failures, three passes, 47 selected cases not completed). Its full output and the direct baseline counterexample are retained; this is not a source-contract pass. The next candidate corrects those exact declarations and test paths. During this interval the unchanged working sources were committed as `75babb4d23` by another actor; this task did not create that commit.

Checkpoint 15 is retained under local ignored `lane-context-checkpoint-13`. The combined build took 585.01 seconds. Core SHA-256: `7e670348907aaa4924742133e9c53f1d9cfc62482c49c9bbad46d4740a800e9f`; model: `89869bd9ffcc54886b612f6d55cf476e23b5bbba4792a21f101dfbda36eb78a1`; Torii: `3f3b908066f76b3ad8734119ffd0cf5190f604d7d0f32fb8f278a9dd0c02af7d`. Both early emitted artifacts match the final build. The scoped ledger/publication checks passed (17 tests); four complete-input source owners passed positive checks and nine adverse mutations were rejected. The full source gate remains open on older State/Kura include-inventory drift. The next candidate addresses actual carrier-envelope sizing and duplicate gossip body transmission; recovery failures remain separately tracked.

Checkpoint 16 is retained under local ignored `lane-context-checkpoint-14`. The combined build took 522.57 seconds. Core SHA-256: `ce910e46d90e0df2c2e341e9151b7dfb2c6c5506c36f91a2c0d607b757997808`; model: `89869bd9ffcc54886b612f6d55cf476e23b5bbba4792a21f101dfbda36eb78a1`; Torii: `79dc6622cec329e1bf2e1e066ffffc8c5a6258bd8f8a0ec14f7bd9821bf888fe`. Early Core/model artifacts match the final build. Exact unsigned sizing matches actual signatures across five algorithms; the existing candidate suite and gossip substitution/schema controls pass. The large-body regressions failed before their intended boundaries and are not qualified. The scoped source checks pass (17 tests; four owners and nine rejected mutations), while the whole-source formal inventory remains open. The successor corrects the fixtures without lowering payloads or raising the 2 MiB/256 KiB limits. A separately diagnosed completed Native publication repair path remains open.

Checkpoint 17 is retained under local ignored `lane-context-checkpoint-15`. The combined build took 168.85 seconds on 2,629 unchanged inputs. Core SHA-256: `47da3d3f9fff144877b443df11c80d5396d1f46217362d113fb6676e5aa9e409`; model: `89869bd9ffcc54886b612f6d55cf476e23b5bbba4792a21f101dfbda36eb78a1`; Torii: `f70306532eb92c8d076e1e97565cce0bfcaa219b4e411504e396372697e414a3`. All 17 scoped source checks pass, with four exact owner clauses and nine rejected mutations. The carrier regression fails while decoding a valid roughly 800 KiB complete input: cumulative allocation reaches 4,918,008 bytes against the inherited certificate-only 4 MiB allowance. The successor uses one model-owned structural decoder with Norito’s existing frame-derived allocation budget, retaining the 1 MiB complete-frame limit and independent field/element/depth bounds. It adds exact-cap, one-byte-over-cap, malformed-frame and stricter-caller-budget controls. Recovery fixtures now use the authenticated fresh-start sequence and real contiguous finalized carriers. These changes are unqualified until rebuilt; the production completed-repair correction remains outside the candidate.

Checkpoint 18 is retained under local ignored `lane-context-checkpoint-16`. The combined build stopped after 402.54 seconds on two private `Header::write` calls in the new model test. The emitted Core and Torii artifacts were tested with all 2,629 captured inputs unchanged: Core SHA-256 `ccd0d68d0e577bca3d60b5aa964fa709df3a23795e31ac532e75538b7a90af07`, Torii `ab9ea5ab816891313f2674478844f4ef565eaae906e5ee6680a5329860322db9`. The real three-input carrier regression passes with its original 800 KiB/2 MiB sizes and retained deferred custody. Cold-state replay passes in 145.47 seconds; a retained read-only stack sample shows active canonical replay, not startup deadlock. The two remaining failures are a certified ABA attempt lacking canonical terminal settlement and an already completed Native sidecar repair incorrectly entering append admission. All 17 scoped formal checks and the codec guard pass. The next candidate changes only the model fixture’s header mutation; no production source changes or recovery fixes are included.

Checkpoint 19 is retained under local ignored `lane-context-checkpoint-17`. The combined build passed in 90.25 seconds with 2,629 unchanged inputs. The sole source delta from checkpoint 18 is the model test fixture. Core and Torii artifacts match checkpoint 18 exactly; the evidence receipt explicitly reuses those complete runs without claiming the two Core failures passed. The model executable SHA-256 is `5308aa6293688c12aede7caa951ea0315476e71ef6dd25c79431ee2f07c16887`. All 16 admission model tests pass, including valid exact 1 MiB decoding, valid one-byte-over-cap refusal, malformed framing, and a stricter surrounding allocation budget. All 17 scoped formal checks pass; the four reviewed boundaries reject nine adverse mutations. This completes the four review corrections. The wider formal inventory, certified-slot retirement fixture, completed Native publication repair, live lane reducer integration and network qualification remain open.

Checkpoint 20 is retained under local ignored `lane-context-checkpoint-18`. The combined build exited 101 after 161.56 seconds, with no tests run. Its separate-origin completed-repair correction requires exact canonical body/finality/WSV authority, retains a complete-carrier publication owner, and proves every non-target already terminal before admitting a partial repair. Candidate 21 corrects the three test compile sites and additionally asserts that the original failed-write cleanup record retains `CanonicalWrite` origin. Production behavior is unchanged from candidate 20; qualification remains pending.

Checkpoint 21 is retained under local ignored `lane-context-checkpoint-19`. The combined build passed in 318.66 seconds with all 2,629 captured inputs unchanged through testing. Core SHA-256: `2a99d8379e16327677f5c753a392da97f65a3b020376474551a0043fb403b0ed`; Torii: `383224ccfd270cc1cf23fc52fa5876570282e0b36b518bf2812c43015f48e3a6`. The model executable is byte-identical to checkpoint 19 (`5308aa6293688c12aede7caa951ea0315476e71ef6dd25c79431ee2f07c16887`), so its 16 passing results are explicitly reused, not reported as rerun. The original complete economic recovery regression now passes in 216.01 seconds, including the second autonomous cycle and public QC replay. The retained read-only sample shows active merge verification after the former failure. Partial A1 repair preserves already advanced B2 files and latest pointer through cold reopening; missing non-target proof, incorrect wire, unfinished-index loss and lost restart authority refuse without growing durable owners. Original canonical-write rollback and publication capacity controls pass. All 17 scoped ledger/publication tests, nine source mutations, codec guard and history verification pass. This is a 133-test affected Core selection, not a full Core/workspace qualification. The separate live-owner ABA fixture and successful completed-secondary archival coverage, full formal source inventory, live lane driver and four-/seven-validator qualification remain open.

Checkpoint 22 is retained under local ignored `lane-context-checkpoint-20`. The combined build exited 101 after 328.46 seconds with all 2,635 captured inputs unchanged; no Rust tests ran. The new first-admission reader race fixture called a test observer that was private to its sibling module. All 17 scoped formal tests passed, and four reviewed owner declarations rejected nine adverse mutations. Candidate 23 restricts that observer to the State module, preserving its test-only status, and adds actual economically completed secondary archival/reopen coverage. The new immutable input DTO, authenticated first-carrier reader, signed-layout capacity handle, and corrected live-owner refusal test remain unqualified until this build and their tests finish. These are foundations; the process-lived lane driver and silent-author failure remain open.

Checkpoint 23 is retained under local ignored `lane-context-checkpoint-21`. The combined build passed in 435.59 seconds, and all 2,636 captured inputs stayed unchanged through testing. Core SHA-256: `11ecdd886922e278afee9b4b6ec4d0490c641727375e9eccc343d28894e318ab`; Torii: `6a192647a7e50cee51920910413c177ae31b653bab9b28f0903a1cba4ca22aaf`; model: `24020e2ad1c4575b4ebf749fefc59aa3b65d46f115ae10bef2f88220e9078b57`. All 22 admission/input model tests and 61 Torii tests pass. The affected Core run reports 145/146 passes, zero ignored: all five first-carrier reader controls, both authenticated capacity controls, live certified ABA refusal, existing repair controls and the full two-cycle recovery regression (215.19 seconds) pass. The new completed-secondary test reaches actual economic Apply, Queue completion and lifecycle archival, then fails before cold reopening because its whole-tree assertion incorrectly requires the live `.lane-incarnation.norito` marker bytes to survive archival. Production deliberately reseals the marker with exact immutable archive paths/content digests; the parsed failure differs in that file only. The successor checks the existing completed journal/path/content seal explicitly, preserves every non-marker and merge-log byte, and compares the fully sealed archive through subsequent rejection and cold-open checks. All 17 scoped formal tests, four reviewed owners/nine adverse mutations, the codec guard and historical archive verification pass. Successful completed-secondary cold reopening remains unqualified until the corrected regression runs; runtime integration and network qualification remain open.

Checkpoint 24 is retained under local ignored `lane-context-checkpoint-22`. The combined build exited 101 after 154.24 seconds with all 2,638 captured inputs unchanged. Its sole compiler error is the new maximum-route sizing fixture passing a `u32` expression to `DataSpaceId::new(u64)`; no Rust tests ran. All 17 scoped formal tests pass, with four reviewed owner declarations and nine rejected mutations. The archive-seal assertion correction, exact canonical native/publication envelope sizing and all-route immutable body preparation remain unqualified. The successor fixes the test conversion without changing lane/dataspace semantics.

Checkpoint 25 is retained under local ignored `lane-context-checkpoint-23`. The combined build passed in 419.15 seconds with all 2,640 captured inputs unchanged through testing. Core SHA-256: `1e9805d7a465afa3fe15f408f6d81b8b6bbadc1d3028873b8e97d658d48c1590`; model: `c57d1f02dfbaf2d7f1b8366157a17ed9b4f95e6314efbd102b31d5ea79429302`; Torii: `eaee08132eaab8f95fca7785dd7ed34a1b725d2a37e5009fb07f7adf19cafea5`. All 22 model and 61 Torii tests pass; Core reports 187/189 selected passes, zero ignored. All-route immutable input preparation, exact signed RS16 materialization, native WAL witness recovery and the existing carrier/gossip/recovery controls pass. The corrected archive path/content seal checks are traversed, but the test then rejects intentionally invalidated accounting caches before its cold-end result can be qualified. Its correction checks initialized caches before refreshing only invalid caches. The second failure occurs in the oversize-input fixture: a JSON string at the byte cap needs additional quote bytes and panics before admission sizing. The successor uses a legal large QueuePlan instruction to reach the intended bound. All 17 scoped formal checks and four-owner/nine-mutation controls pass. Codec retirement, historical archive verification and diff checks pass. Thin native body custody and reverse native effect projection are reviewed outside-repository drafts, not production runtime integration; all six redesign goals remain open.

Checkpoint 26 is retained under local ignored `lane-context-checkpoint-24`. The Core-only test build passed in 417.42 seconds with all 2,644 captured inputs unchanged through testing. Core SHA-256: `0050172d7a1a7a834ef91b6b0ce7f3889887b2b4d365bff870fcdbc9a90ae28c`. All 46 selected Core tests pass, zero ignored. The two corrected fixtures pass: actual completed-secondary archival reaches cold reopening (19.66 seconds), and a legal large instruction reaches the intended complete-input envelope bound. Five native body-store controls cover fsynced reopen, every manifest origin, corruption/foreign identity/oversize, directory replacement, publication error and post-publication readback fencing. Four native effect projection controls cover every WAL/signing intent, pre-payload Timeout, signature/witness refusal, and exact preservation of signed proposal timeout evidence. Existing WAL and wire controls pass. Model/Torii tests were not rerun; their 22/61 passes remain candidate 25 evidence. All 17 scoped formal checks pass; four reviewed source owners reject nine adverse mutations. Codec retirement and diff checks pass. No production native driver is activated. Cross-route input immutability additionally requires the sole economic pipeline to settle every affected group atomically before any member frontier can change; current independent ordinary/native frontier writers do not yet enforce that prerequisite. Runtime cutover, whole-source formal binding and network qualification remain open.

The post-26 formal inventory audit updates 24 existing literal Rust include edges across 12 parents and explicitly pins the nested runtime-catalog test include. Each addition was checked against its actual parent call site and retained content digest; no provider, semantic token, source/index check or proof obligation is removed. The prior inventory's canonical digest was `6e1ec3ac5894f47d67836e78662289d8e3bf2852f9176ceea82cb5eb302ced0d`, while the helper still expected `051c0774b6dd8e843a834d8ff99bd27b977f3d3d4de199f660d38d4e4e9f0cb2`. The updated canonical inventory digest is `d8e4368ae97b1f782ee82f5ec7b58b5efc4706d5b19238dcdd71adf582a45027`. All 22 selected reviewed-source and complete-input ledger/publication tests pass (17.89 seconds). The full structural contract check still exits 1: 148 reported errors (147 unique), now exposing semantic-anchor drift in lifecycle drain, Native publication/pruning, ingress completion and related source/test bindings. This is not a full formal pass; those declarations must be reviewed against their actual surviving owners, not mechanically replaced with whatever tokens happen to exist. Audit patches, provider hashes and both full logs are retained in the local checkpoint workspace under `formal-inventory-27`.

Checkpoint 27 is retained under local ignored `lane-context-checkpoint-25`. The Core-only build passed in 239.40 seconds with all 2,648 captured inputs unchanged through testing. Core SHA-256: `ba8ed8394be5010cdf196f9b7e613394cd89bd152134fa04fbbd8b910b31f9ea`. All 55 selected Core tests pass, zero ignored, including all five process-lived instance-owner controls and four Decision-group controls. Real State/Kura/four-key tests cover silent initial leadership, fsync-before-ack reopening, a saturated bounded outbox while TC persistence/view change proceeds, exact healed TC delivery, higher-Prepare custody with repeated body fetches, unchanged timers/completions across unrelated finalized global publication, frozen-key/output refusal, and checked deadline overflow without losing obligations. The group consumer verifies one exact CommitQC per distinct route, preserves independent lane views, names missing instances/earlier heads, and rejects cryptographically valid substituted descriptor/input/kind/codeword and foreign source/binding. Both prior corrected fixtures and all 46 prior selected boundary tests still pass, including completed-secondary cold reopening (19.88 seconds). All 22 selected inventory/admission-binding checks pass (18.39 seconds), four reviewed owners reject nine mutations, and codec/diff checks pass. These tests exercise the intended owner with physical storage and cryptography, not P2P routing or economic Apply. No production runner constructs it; body worker integration, native Decision consumption by global execution, atomic old-signer/bypass removal, the full formal contract and real four-/seven-validator qualification remain open.

Checkpoint 28 is retained under local ignored `lane-context-checkpoint-26`. The Core/model library-test build exited 101 in 264.17 seconds with all 2,649 captured inputs unchanged. Two new test-only calls do not compile: the model test returns a String error inside Norito's Error-typed decoding-budget closure, and the Core fixture asks State for a height method that belongs to StateReadOnly. No Rust tests ran. The successor corrects those two fixture calls; native single-input Decision-group decoding/import, authenticated source-recovery continuation and pre-carrier batch preflight remain unqualified until its run. No production activation or economic Apply is claimed.

Checkpoint 29 is retained under local ignored `lane-context-checkpoint-27`. The Core/model test build passed in 287.50 seconds with all 2,649 captured inputs unchanged through testing. Core SHA-256: `2c446bd0a906ec50d32a3b18155299bdee8a928c027a8cb863c931f3181db223`; model: `3b512eb3a670e519e5a1b9ac6cf793df0792d55075f657d4d0dbbe4fdb225283`. All 24 model tests pass, including the single-body 800 KiB group, exact frame cap, malformed framing and stricter outer allocation budget. Core passes 58/59, zero ignored: source import refuses valid re-signed first-carrier/certificate-subset substitution; current-base preflight rejects changed frontier, closed head and duplicate groups. The new missing-first-body test uses `force_hash_only_block_for_testing`, which creates a zero-length index slot. The strict fallible read correctly rejects that shape outside an authenticated imported prefix; actual eviction retains the signed executed wire length. The successor supplies that real evicted geometry without weakening production reads. Recovery continuation remains unqualified pending that run. All 22 scoped source checks and four-owner/nine-mutation controls pass; codec/diff checks pass. The economic executor and physical body-worker extension remain outside-repository drafts. No production activation, economic Apply or network qualification is claimed.

Checkpoint 30 is retained under local ignored `lane-context-checkpoint-28`. The Core/model test build exited 101 after 67.90 seconds with 2,651 captured inputs unchanged. The new instance adapter's mutable witness inserter shared the name of the immutable native-witness trait getter; two calls resolved to the getter and produced five type/`?` diagnostics. No tests ran. The successor gives the inserter the distinct name `retain_value`, without altering native signature or reducer semantics.

Checkpoint 31 is retained under local ignored `lane-context-checkpoint-29`. The combined build passed in 176.18 seconds with 2,651 captured inputs unchanged through testing. Core SHA-256: `93b0eb510a8518899cc0a35bc431971abb5cc0298cd150b7b3b41ba9b1307e99`; model: `3b512eb3a670e519e5a1b9ac6cf793df0792d55075f657d4d0dbbe4fdb225283`. All 24 model tests pass; Core passes 66/67 with zero ignored. All eight new physical-worker controls pass: the same authentic three-of-four owners progress past the silent initial leader through actual threaded body acquisition/fsync/readback/validation and native Decision while holding Apply; Set B fallback retains its exact body witness; CommitQC persistence continues with a worker in flight; closed/foreign completions preserve physical custody; signed wrong input is refused; missing first-carrier recovery and earlier-route waits preserve timeout service. The corrected length-preserving eviction fixture reaches authenticated global CertifiedBody recovery and exact source re-import. The one failure is an older timeout test that assumes every offered vote is consumed while TC persistence is pending. The updated adapter explicitly reports the shared reducer's Busy as backpressure; the fixture must retain and retry that message after the real TC acknowledgement, retaining its saturated-outbox checks. All 22 scoped formal checks pass (18.75 seconds), four reviewed owners reject nine mutations, and codec/diff checks pass. This is instance/consumer boundary evidence, not production P2P, economic execution, full formal closure or release qualification. The economic executor and portable transcript remain outside-repository drafts pending review and tests. All six redesign goals remain open.

Checkpoint 32 is retained under local ignored `lane-context-checkpoint-30`. The combined Core/model test build exited 101 after 405.78 seconds with 2,652 captured inputs unchanged. One new model test match arm returned the header setter's mutable reference instead of unit. No Rust tests ran; the successor adds the required statement terminator. This failure does not invalidate or expand candidate 31 evidence.

Checkpoint 33 is retained under local ignored `lane-context-checkpoint-31`. The combined build passed in 134.04 seconds with all 2,652 captured inputs unchanged through testing. Core SHA-256: `3babd3c57936ce84fe61f5e18a0f92f2fa098ba5fe4be77a3ddda4b4be2cd080`; model: `7326c6902b85d65a9d757aac16c7f6205acf690850f10d1f2d57937980077fdc`. All 82 selected Core and 29 model tests pass, zero ignored. The corrected timeout fixture retains explicit Busy/backpressure through actual TC fsync, retries after acknowledgement, and preserves saturated-outbox/restart assertions. The direct review regressions pass again on this candidate: sealed transaction helper tests compile and run, three approximately 800 KiB complete inputs are prefix-selected to fit the real 2 MiB carrier including framing, and the 160 KiB authenticated gossip input fits and delivers once under the default frame cap. Five new model controls qualify single-body economic transcript framing, exact cap/outer limits, all-route/identity ownership and cycle-free replay-marker commitments; this is an inactive wire model, not economic execution. All 22 scoped formal checks pass (18.78 seconds; 20 preexisting pytest temporary-directory cleanup warnings), four reviewed owners reject nine adverse mutations, and codec/diff checks pass. The latest full formal failure remains 148 reported semantic-binding errors. The genuine economic executor, portable replay builder and their transfer/rejection tests are reviewed outside-repository drafts at this checkpoint. Production owner/transport integration, sole economic consumption, old-signer/bypass retirement and real four-/seven-validator qualification remain open; all six redesign goals stay active.

The status paragraph before candidate 36 was:

The latest combined Core/model build passes with 2,652 unchanged captured inputs: 82/82 selected Core and 29/29 model tests pass, zero ignored. This includes the four-review regression boundaries, all eight physical-worker tests, the corrected timeout/backpressure/restart fixture, and the single-body economic transcript model. All 22 scoped inventory/admission-binding checks pass. These are boundary tests, not production native failover or economic publication. The last full structural run reports 148 semantic-binding errors; there is no full formal or workspace pass.

Candidate 34's combined test build exited 101 after 148.02 seconds with all 2,655 captured inputs unchanged during compilation. The new batch marker lookup lacked `StorageReadOnly`; no Rust test ran. Another actor then committed the accumulated source as `0deb9c370a993627eddb9198d4d5cafb6f4d8410` and started a merge. Post-build verification correctly detected the subsequent source change. The prematurely created local checkpoint 32 therefore records a post-commit snapshot, not a full captured candidate diff. The supplemental `candidate34-post-build-concurrency` evidence reconstructs all 2,655 inputs exactly from that commit and retains the failed verification and import correction. No concurrent merge changes were reset, staged or resolved by this work.

Candidate 35 moved to branch `codex/sumeragi-liveness-native-20260916` in an isolated worktree at that exact commit. The source-owner guard refused the main worktree's Cargo target in 1.45 seconds; no compiler or test ran. Local checkpoint 33 retains all 20,526 captured regular files unchanged. A private APFS-cloned dependency cache was then prepared, omitting incremental artifacts and invalidating every local-path package fingerprint (107 packages, 4,235 directories) so all local sources rebuild. The original target and source-owner guard remain untouched.

Candidate 36 is retained in local ignored `lane-context-checkpoint-34`. Its private Core/model test build passes in 685.47 seconds with all 20,528 captured tracked/unignored local regular files unchanged through the test run. Local Cargo artifacts were rebuilt; private immutable test executable copies have Core SHA-256 `1fddccdbb480c0be4fac6ca6c7c12b58481943e325f39f88c941ee7f75c7ceba` and model SHA-256 `d122c5947c89dfe6d93703e2521f94d5958a0f5155dcea1b30c79f770388be00`. Core passes 96/107 selected tests, model passes 29/29, zero ignored. All five new physical WAL worker tests pass, including another lane's progress while a real worker is held, foreign result custody, fsync-before-ack reopening, closure drain and terminal error/drop fencing. The prior review and body/instance boundaries pass again. Nine new economic tests do not qualify execution: seven fail at missing genesis asset incarnation, two abort from default-stack overflow. Two additionally selected old batch-carrier tests advertise two committed fragments when actual execution yields one. The successor seeds actual genesis incarnation before metadata, keeps scratch StateBlock ownership boxed end-to-end, and adopts the exact-fragment fixture correction already staged in the concurrent main merge. No stack limit or production authentication predicate is weakened. All 22 scoped formal tests pass (18.77 seconds, 20 preexisting cleanup warnings); four source owners reject nine mutations and codec/diff checks pass. Full formal still has the separately recorded 148 semantic-binding failures. Production activation and network qualification remain open.

The next candidate also addresses completed Native repair restart after the actual evidence writer fsyncs a manifest temporary but before rename. Pristine admission still rejects temporaries; an existing exact CompletedRepair index may authenticate only canonical expected artifact bytes in the bound physical namespace, with original stable receipt/latest and finality/WSV joins retained. Three added tests exercise cold restart and exact capacity accounting, same-owner retry, and four foreign/tampered/unindexed/wrong-height negatives. These changes are applied but not yet compilation/runtime-qualified at this entry.

The status paragraph before recording candidates 37 and 38 was:

The isolated Core/model build passes with all 20,528 captured local files unchanged. Candidate 36 passes 96/107 selected Core and 29/29 model tests: all earlier review boundaries and five physical WAL-worker controls pass; nine new economic tests fail before execution (genesis-incarnation fixture and default-stack ownership), and two additional old batch fixtures advertise an incorrect fragment count. Their successor corrections and authenticated repair-temporary recovery are applied but not yet qualified. All 22 scoped formal checks pass. The last full structural run still reports 148 semantic-binding errors; there is no full formal or workspace pass.

Candidate 37 is retained in local ignored `lane-context-checkpoint-35`. The isolated Core/model test build passes in 169.56 seconds, retaining all 20,528 captured local regular files unchanged through execution. Core SHA-256 is `b7872a1a0dd7d833efdbf6d297241b73ee7310ca4efb8cec28c854377b1b4334`; the unchanged model binary is `d122c5947c89dfe6d93703e2521f94d5958a0f5155dcea1b30c79f770388be00`. Core passes 123/128 selected tests, model 29/29, zero ignored. All 21 Native repair/publication-capacity controls pass, including the actual temporary-fsync/pre-rename crash cut (cold reopen, original receipt/latest preservation, exact quota accounting and completion), same-owner retry, and four tampered/foreign/unindexed/wrong-height negative cases. Existing partial repair, later frontier, physical binding, missing unfinished index, canonical reservation and no-growth controls pass. The five physical WAL-worker controls and all earlier admission-sizing/gossip/path regressions also pass; the two old batch fixtures now derive actual committed fragments and pass. Heap-owned scratch state removes the two default-stack aborts, and authentic genesis incarnation seeding lets every economic test reach execution. Five positive cases correctly reject empty signed fee limits under the fixture's unintentionally nonzero Nexus fee policy; four economic negative/rollback/replay cases pass. All 22 scoped formal checks pass (18.79 seconds, 20 preexisting cleanup warnings), four source owners reject nine mutations, and codec/format/history checks pass. These are scoped results; no full formal, workspace or production cutover is claimed.

Candidate 38 is retained in local ignored `lane-context-checkpoint-36`. It changes only the economic fixture to an explicit zero-charge policy before initial Nexus configuration, matching the existing autonomous transfer fixture; signed fee validation is unchanged. The combined test-target build passes in 78.35 seconds and all nine affected economic tests pass with zero ignored and all 20,528 captured inputs unchanged. Core SHA-256 is `275984c9fa9fce6ace0327b1ea121ab56b832d53c2e627a66f1c2018e7fc1c08`; the model binary remains byte-identical to candidate 37 and is not retested. Actual transfer, aggregate gas prefix selection, sealed commitment order and alias authentication, terminal rejection, marker-conflict rollback, canonical transcript decoding and exact scratch replay now pass on ordinary stacks. The older 21 repair/capacity and scoped formal/model results retain candidate 37's scope; they are not reported as another full combined pass. Native nonzero-fee execution, canonical global publication/Apply, historical finalized carrier inclusion, production owner/transport activation, old-signer/bypass retirement and four-/seven-validator qualification remain open.

The six-file repair patch and validated WAL-worker/heap/genesis/zero-charge corrections were applied to the main checkout as scoped hunks. Both integration receipts verify the existing staged merge diff is byte-for-byte unchanged; no staging, commit, merge resolution or reset was performed. Main's extra pure index tests and capacity comments remain intact. The concurrent merge itself was not qualified by the isolated test results. The persistent redesign goal and all six milestones remain active.


## 2026-09-17 carrier and process-owner boundary qualification

Preceding current-health paragraphs, preserved verbatim:

The isolated Core/model test target compiles with 20,528 captured local files unchanged. Candidate 37 passes 123/128 selected Core and 29/29 model tests, including all 21 repair/capacity regressions, the actual post-fsync crash cut and all five physical WAL-worker controls. Its five remaining economic fixture failures are corrected in candidate 38, where all nine affected economic tests pass on the default stack. The repair and validated worker/economic corrections are integrated without changing the concurrent staged merge. All 22 scoped formal checks pass; the last full structural run still reports 148 semantic-binding errors, with no full formal or workspace pass.

Production instance/transport/worker integration, deterministic global economic consumption, atomic old-signer and ordinary-bypass retirement, and real four-/seven-validator fault/restart qualification remain open. The inactive economic executor now has disposable transfer/replay/rollback evidence under an explicit zero-charge fixture; global native publication, nonzero-fee qualification and runtime cutover remain open. Exact checkpoint evidence and failures are in the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md); the [previous status paragraph](docs/history/2026-09-16/sumeragi-liveness-status-before-body-worker.md) is preserved verbatim.

Preceding active-goals boundary paragraph, preserved verbatim:

Move-owned body jobs now pass all eight real threaded State/Kura controls,
including silent-leader replacement through native Decision with Apply still
held. The same reducer services Timeout/TC and CommitQC while body work waits;
actual fsync/readback and all-route validation are required for body-dependent
signing. The exact first-carrier recovery obligation survives missing local
body, and foreign/closed completions return physical custody. The earlier
timeout control now retains/retries explicit Busy backpressure and passes with
its original saturation/restart assertions. Candidate 37 passes 123/128 Core,
29/29 model and 22 scoped formal checks, including all 21 repair/capacity and
five physical WAL-worker controls. Its remaining five economic fixture failures
are corrected in candidate 38: all nine affected economic tests pass on ordinary
stacks with actual disposable transfer, rejection, replay and rollback. These
balance/order controls use an explicit zero-charge policy; they do not qualify
native nonzero-fee execution or global publication. Both runs retain unchanged
captured source/private artifacts. The reviewed repair and worker/economic
corrections are integrated while preserving the concurrent staged merge.
Historical carrier inclusion/replay, sole production ownership/transport and
old-signer retirement remain next. These results do not close the real
four-validator counterexample or L1–L6.

Candidate 39 is retained in local ignored `lane-context-checkpoint-37`. Its isolated Core/model test-target build passes in 423.97 seconds with all 20,531 captured tracked/unignored local regular files unchanged through its tests. Core SHA-256 is `fef18f8d4fa450ddb67d83141435c9fc0b176b00eaaf7ff4ffa3a11b3dbcd283`. Core passes 127/133 selected controls, zero ignored.

The required nullable native economic carrier field, private finalized Kura inclusion and explicit-pre-State scratch replay are inactive. Four historical fixtures passed a payload-dependent application header to the builder; two controls aborted on the default stack. All 21 repair/capacity and nine economic controls pass. All 332 block-model tests and 122 scoped reviewed-source/pending-membership/publication checks pass (20 preexisting pytest temporary-directory cleanup warnings). The first direct pytest collection of included case fragments failed because those fragments require their parent module; the incorrect invocation is retained alongside the corrected run. Native global acceptance remains closed.

Candidate 40 is retained in local ignored `lane-context-checkpoint-38`. Its isolated Core/model test-target build passes in 368.34 seconds with all 20,533 captured tracked/unignored local regular files unchanged through its tests. Core SHA-256 is `accebd905cb6103d8da6176c22bc3c75ba8875c669edf162f64d61e46213c1fe`. Core passes 137/138 selected controls, zero ignored.

The carrier batch now has Box ownership end-to-end, including the real 800-KiB full-carrier codec control; Norito documentation records its owned-value length prefix. Four threaded physical opening/replay/adoption/drain controls pass, and the former body-closure default-stack abort passes without a stack override. Five of six historical tests pass, including exact two-input recovery retention. One restored-prefix fixture reaches exact replay but diverges because bare snapshot restore lacks its original static fee policy. All 332 block-model and 122 scoped formal checks pass; the full structural checker is rerun and reports 148 binding errors in 60.73 seconds. Format, diff and codec checks pass. These scoped boundaries do not establish native production progress or economic publication.

Candidate 41 is retained in local ignored `lane-context-checkpoint-39`. Its isolated Core/model test-target build passes in 247.04 seconds with all 20,534 captured tracked/unignored local regular files unchanged through its tests. Core SHA-256 is `7f590b20e5f45218d97fae7e929b52921c745fd82e3e49122d83fdedd7c3ce43`. Core passes 18/19 selected controls, zero ignored.

All four new nonzero Direct-XOR fee tests pass: exact signed limits and payer/supply burn, terminal cap/balance rejection, late batch rollback, and sequential payer exhaustion across two authenticated sources. Existing nine economic controls and five historical controls also pass. Restoring static policy through the actual startup setter correctly refuses the fixture because its initial setup never persisted the configured-catalog baseline. The next fixture correction establishes that authority before genesis rather than bypassing restart checks. The model executable remains byte-identical to candidate 40 and is not retested. Charged status events currently describe scratch execution; these results are not exactly-once committed fee telemetry or global Apply.


Candidate 42 is retained in local ignored `lane-context-checkpoint-40`. The combined test build passes in 130.25 seconds with all 20,536 captured files unchanged. Core SHA-256 is `bdec162b860d4ee4d99fc29882e0bc90af971a4ab88f1d9218884021469813d9`; the model artifact remains unchanged from candidate 40. The shared live/finalized carrier projection and common replay kernel compile, and 122 scoped formal checks plus codec/format/diff checks pass. All 25 selected runtime controls fail before reaching their targets because the shared fixture tries to publish a configured baseline through a blank Kura that never authenticated that catalog. No live-carrier runtime result is claimed. The next correction uses existing authenticated temporary Kura and complete pre-genesis startup APIs, preserving the explicit genesis NetworkId and restart guards. The prepared main integration patch was checked but not applied.


Candidate 43 is retained in local ignored `lane-context-checkpoint-41`. The test build passes in 59.56 seconds with all 20,536 captured files unchanged; Core SHA-256 is `38b705323afcd6af5d02499a4f2a2a516f902dfab7100c70b1a4fb37b6e8c70c`. All 25 controls still stop during shared fixture initialization: the convenience State constructor eagerly stamps default lane-incarnation markers before the authenticated configured-primary anchor. The next correction uses the existing fallible production State constructor and preserves explicit network identity, runtime test settings and the complete startup order; no physical marker is rewritten and no guard is relaxed. Model/formal results retain their prior scope.


Candidate 44 is retained in local ignored `lane-context-checkpoint-42`. The combined test target builds in 59.36 seconds with all 20,536 captured local regular files unchanged. Core SHA-256 is `26c59d4c8c0ba1d44342b98f8cd33d545647d2d1dd469818e20f028e2194c9e3`. Sixteen of 25 selected Core tests pass, including all thirteen economic controls and all four direct nonzero-fee cases. Nine historical/live carrier cases stop at the shared projection's blanket rejection of the mandatory DA proof-policy snapshot. Three negative carrier controls pass at that earlier guard and do not yet establish their deeper intended boundary. The next correction binds the read-only snapshot bytes to the actual header and authenticates the exact active policy from the applying pre-State; State-changing controls remain unsupported. Both carrier fixtures must install their real two-lane policy. No model bytes changed from candidate 40, and no new model/full-formal pass is inferred.


Candidate 45 is retained in local ignored `lane-context-checkpoint-43`. The combined test target builds in 96.51 seconds with all 20,536 captured files unchanged. Core SHA-256 is `0688d90edfe5c51bd28be9a4a31555d8ad311575613de6a757c19fd1dfd82e26`. Twenty-four of 25 selected Core controls pass: thirteen economic, all six live carrier and five historical carrier tests. Mandatory read-only DA proof-policy snapshots bind to their actual header and exact height-specific pre-State policy; missing, foreign and unbound snapshots are rejected. State-changing additional controls remain unsupported. The remaining closure/snapshot test authenticates its exact prefix but produces a different execution transcript; reinstalling Nexus alone has not established equivalent runtime settings. Its successor also restores the source fixture's test runtime defaults and reports which transcript component differs. All 122 scoped formal tests pass (19.69 seconds; 20 preexisting cleanup warnings); codec and formatting checks pass. The model binary remains byte-identical to candidate 40. Native production acceptance and the concurrent main merge are not qualified.


Candidate 46 is retained in local ignored `lane-context-checkpoint-44`. The isolated Core/model test-target build passes in 243.77 seconds with all 20,538 captured local regular files unchanged through testing. Core SHA-256 is `3fd19b1f2b7ec390285df815bc7578aad7058c2fd222061868c56d3d4e9fe519`. The complete selected run takes 524.84 seconds and passes 151/153 Core controls, zero ignored. All 122 scoped formal checks pass (19.61 seconds; 20 preexisting cleanup warnings); codec/format/diff checks pass. The unchanged model artifact retains candidate 40's 332-test scope.

The process-level instance table and fixed, separate opening/WAL/body workers compile. Four new controls pass: a held opening permits another lane's actual timeout fsync/signing; full worker queues and foreign completions preserve the exact owner across unrelated global advancement; authenticated closure waits for actual body/WAL handle return before off-loop drain while retaining unacknowledged effects; corrupt/dropped opening custody fences output. All 21 repair/capacity controls, earlier complete-input/gossip sizing, physical opening/WAL/body and thirteen economic regressions pass again.

Two fixtures still fail. The snapshot helper now reinstalls the same test runtime defaults but still restores an empty lane manifest registry; actual transaction lane-policy enforcement rejects the input, so results/settlement differ despite matching WSV hashes. The daemon separately reinstalls frozen manifests and compliance before snapshot replay; the next fixture does the same and asserts the whole execution-policy digest. The process success test ignores Backpressured from an incoming CommitQC while a Prepare write still owns persistence. Its correction retains that exact input until accepted and consumes real worker/ack completions before expecting a Decision. The first outside correction accidentally edited two unrelated tests; review rejected it before application, and a replacement patch changes only the intended test. No production guard is relaxed. These results do not qualify native global publication or the real network counterexample.


Candidate 47 is retained in local ignored `lane-context-checkpoint-45`. Its test-target build passes in 100.98 seconds with all 20,538 captured files unchanged. Core SHA-256 is `92a5a7e533cbfea1f08520a2d2ff6e6ea1d0bc359b31e5359e8b149e6c8a93c4`. Only the two failed fixture bodies/helpers change from 46, and both affected tests pass (8.04 and 3.50 seconds), zero ignored. Restored-prefix replay now reinstalls the original manifest/compliance registry and requires identical execution-policy digests as well as the exact snapshot root. It replays the historical batch successfully after authenticated closure while the post-State is rejected. The process test retains its exact CommitQC across Backpressured, pumps actual physical persistence/signing/body completions, and consumes returned control acknowledgements before checking durable Decision and held Apply. It does not fabricate delivery or global application.

Candidate 46's other 151 selected Core passes and 122 scoped formal checks retain that source scope; this is not a fresh 153-test run on 47. The model artifact remains byte-identical to candidate 40, whose 332 block-model controls passed. No full formal, workspace, production native or four-/seven-validator pass is inferred. The latest full structural run remains candidate 40's 148 reported binding errors.

The 24-file carrier/opening/process/economic patch is applied to main with patch SHA-256 `d39506512a4d1b8d133c42f782c6aca754c1a1e46104cbac9c8eb6d89f19b4cf`. Exact reverse application in an outside copy reconstructs every selected main file's pre-application bytes, preserving independent edits in block.rs, state/tests.rs and norito.md. The staged merge diff and HEAD/MERGE_HEAD are unchanged. No staging, commit, merge resolution or reset was performed, and the concurrent main composition was not compiled. Integration receipts are retained in local ignored `dist/sumeragi-liveness-redesign-20260916/native-carrier-opening-integration-47`.

The canonical consumer audit requires a first-release order correction: authenticate native source authority against the applying pre-State, run mandatory start hooks once, then execute native economics under height-H policy before pipeline/Time and final witness/publication. The current inactive scratch implementation still skips those hooks; its replacement is an outside draft, not a validated global consumer. The intended native prefix commitment includes start-hook and native writes before replay markers. Full results/FASTPQ should have one canonical BlockResult owner, with exact per-position commitments in native proposal claims; this model adjustment is also not yet applied. Sole canonical commit-source authorization, result/query/proof projection, durable native Apply/cleanup receipts, fair production runner/transport/refresh integration, exactly-once committed fee observability and old-signer/Ordinary bypass retirement remain open. All six redesign goals stay active.


## 2026-09-17 ordered native stage qualification

Preceding current-health paragraphs, preserved verbatim:

The isolated Core/model test target compiles with all 20,538 captured local files unchanged. Candidate 46 passes 151/153 selected Core controls; candidate 47 changes only the two failing fixtures and both corrected tests pass. This includes all 21 repair/capacity controls, the actual post-fsync crash cut, complete-input sizing/gossip, physical opening/WAL/body ownership, nonzero-fee execution and the new process-level worker table. All 122 scoped formal checks pass on 46; the unchanged model artifact retains 332 passing block tests from candidate 40. The last full structural run still reports 148 binding errors. The scoped changes are integrated with the staged merge unchanged; the concurrent main composition is not compiled.

The replacement remains inactive in production. Exact live/historical input authentication and disposable economic replay are qualified at their recorded boundaries, including snapshot replay with the full startup policy and retained worker/backpressure custody. Canonical execution must still run height-effective start hooks before native economics, then complete results/Time/witness/publication and authenticated native Apply. Fair runner/transport/refresh integration, old-signer and Ordinary bypass retirement, committed fee observability and real four-/seven-validator qualification remain open. Exact scope and failures are in the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

Preceding active-goals boundary paragraphs, preserved verbatim:

Candidate 46 passes 151/153 selected Core controls, including all 21 repair/capacity
checks, the earlier admission/gossip regressions and thirteen economic controls
with four direct nonzero-fee cases. Candidate 47 changes only the two failing
fixtures; both now pass. Full runtime policy/manifests are restored before exact
historical replay, and the physical process pump retains Backpressured ingress
through actual fsync/ack. Candidate 46 passes 122 scoped formal checks; the
byte-identical model artifact retains candidate 40's 332 passing block tests.
The last full structural check reports 148 binding errors. These are scoped
boundary results, with unchanged captured source/artifacts per checkpoint;
they do not qualify the concurrent main merge or a production native runtime.

The carrier/opening/process changes are integrated while preserving independent
working-tree changes and the staged merge. The next canonical consumer must
first authenticate source groups against exact applying pre-State, then run
mandatory start hooks once before native economics, followed by pipeline/Time,
result projection and witness finalization on that same owned overlay. The
current inactive scratch kernel skips those hooks and cannot be published.
A constructor-owned stage seal must replace old merge-only source/membership
assumptions without granting arbitrary scratch commit authority. Native proposal
claims should commit exact results and FASTPQ evidence while the full values
live once in canonical BlockResult; this first-release model change is pending.
Durable global/native Apply receipts, exact cleanup/restart cuts, committed fee
observability, production ownership/transport and atomic old-signer/Ordinary
bypass retirement remain required. These results do not close the real
four-validator counterexample or L1–L6. Exact evidence and preceding health
paragraphs are retained in the foundation history.

Candidate 48 is retained in local ignored `lane-context-checkpoint-46`. Its isolated Core/model test-target build passes in 224.67 seconds with all 20,539 captured tracked/unignored local regular files unchanged through qualification. Core SHA-256 is `2d06c50b8657ef57b123416263436b757af58e8a2815bb2abf2f3e7d8b5d4028`; model SHA-256 is `166c469ba1a4bb9e02a845a217a021081112619675e76c98dcf1fb0b27ff7e33`. Core passes 408/413 selected controls in 728.42 seconds; all 332 model controls and 122 scoped formal checks pass, with zero ignored tests and 20 preexisting pytest cleanup warnings. The full structural checker still reports 148 binding errors in 55.46 seconds. Codec and format/diff checks pass.

The shared State constructor now owns exact pre-State authentication, mandatory start hooks and the native economic continuation on one heap-owned overlay. Live/finalized source wrappers share this kernel; source I/O precedes MV locks. Private stage identity, full source/alias membership and actual outputs survive later metadata handling. The prefix roots include shared start writes, and a stage cannot commit through old empty-merge authorization or replay flags. Five of seven new stage controls pass. Two fail before their target because a scheduled confidential-policy fixture omitted its mandatory positive conversion window. Three older block count fixtures also fail identically on captured47: an absent authority and unmatched pipeline events produce zero actual fragments. All previous 153 selected Core controls, including 21 repair/capacity and the post-fsync crash cut, pass on 48.

Candidate 49 is retained in local ignored `lane-context-checkpoint-47`. Only the policy and block fixture files change. The scheduled policy supplies a valid one-block conversion window; count attacks use an actual seeded domainless authority, while a rejection-only block correctly has zero applied fragments. The fresh test target builds in 104.76 seconds with all 20,539 captured inputs unchanged. Core SHA-256 is `86c7c40e789c131f8588bf20817495a3fd6502130c56272762cc210211b68a52`. All five affected tests pass in 9.53 seconds. The model binary is byte-identical to48 and not retested; the other408 Core and122 scoped formal passes retain48's scope.

Eight ordered-stage source files are integrated into main with patch SHA-256 `ddf1d5ec19282cd7be85a5e9bdd81d3955b3cf43d024dca1b263d2879babfe7c`. Reverse application in an outside copy reconstructs every selected pre-application byte. Main's independently staged block-fragment fixture corrections are left verbatim: its99-advertised test intentionally expects zero actual fragments, while isolated49 seeds a successful input and expects one. The staged diff and HEAD/MERGE_HEAD are unchanged; no staging, commit, reset or merge resolution occurred. Main composition is not compiled. Receipts are retained in local ignored `dist/sumeragi-liveness-redesign-20260916/native-ordered-stage-integration-49`.

The next prepared change commits native full results and FASTPQ vectors in compact proposal claims, stores their full values once in canonical BlockResult, preserves the physical-external proposal root and uses actual executed fragment counts. Actual FASTPQ capture custody must survive prefix execution through final common inventory validation. These remain outside drafts at this checkpoint. Native production acceptance, common final tail/publication/Apply, query consumers, committed fee telemetry, runner/transport activation, old-signer/Ordinary retirement and real four-/seven-validator qualification remain open; all six goals remain active.

## 2026-09-17 proposal/output circularity counterexample

Superseded status and goal paragraphs are retained verbatim below. Their compact-output proposal plan is rejected by the actual registration counterexample that follows.

The isolated Core/model test target compiles with all 20,539 captured local files unchanged. Candidate 48 passes 408/413 selected Core and all 332 block-model controls; candidate 49 changes only the two failing fixture files, and all five affected tests pass. This includes all 21 repair/capacity controls, the actual post-fsync crash cut and complete-input sizing/gossip. Candidate 48 passes all 122 scoped formal checks, while its fresh full structural run still reports 148 binding errors. The ordered-stage changes are integrated with the staged merge unchanged; the concurrent main composition is not compiled.

The replacement remains inactive in production. Exact pre-State authentication now precedes mandatory height-effective start hooks and native economics on one owned overlay; private stage identity and membership survive later metadata application, and commit explicitly rejects this unfinished native stage. Full result/FASTPQ ownership, pipeline/Time/witness/publication and authenticated native Apply remain open. Fair runner/transport/refresh integration, old-signer and Ordinary bypass retirement, committed fee observability and real four-/seven-validator qualification remain required. Exact scope and failures are in the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

Native proposal claims will commit exact results and FASTPQ evidence while full
values live once in canonical BlockResult. This first-release model change and
retention of actual FASTPQ capture provenance are prepared outside the checkout,
not yet qualified. Complete pipeline/Time and final witness, result/query/proof
projection, sole publication authorization and durable native Apply/cleanup
receipts remain required. Committed fee observability, fair production runner,
transport/refresh and atomic old-signer/Ordinary bypass retirement stay open.
These results do not close the real four-validator counterexample or L1–L6.
Exact evidence and preceding health paragraphs are retained in the foundation
history.

Candidate 50's compact result-model build failed at a test-only mutable borrow; candidate 51 corrects that borrow and builds both library test targets in 100.15 seconds. All 20,541 local tracked/unignored regular inputs remain unchanged. Its selected Core run passes 414/415 tests; all 341 model tests pass, with no ignored tests. The sole Core failure is `historical_native_batch_carrier_recovery_retains_exact_resultless_custody`: its updated actual-result fixture compared the result-bearing stored header with the correctly resultless recovered header. Core SHA-256 is `7eddb717f7c1b239ff23320a594ab0812a89d3716ad52f443cd0c0a34e8321c2`; model SHA-256 is `39db26b78fd3a699c39583c467839eaeaee17e46a33a87cca993940b22ad5658`. Local ignored checkpoint49 retains this failure and the known repeated-Time display-hash issue. The compact-output model is not integrated into main.

Candidate 52 adds only a real asset-registration fixture and regression, builds in 114.42 seconds, and preserves all 20,541 inputs through its one selected test. Core SHA-256 is `e1e53b97287c26304240a139cacdf5a8f65c241e15febe782a1f47b00683fa27`; the model binary is byte-identical to51 and not retested. Registration succeeds under the stripped preparation header and derives that header-bound AXT asset incarnation. Actual proposed-carrier replay then fails with `native execution economic writes differ from replay`. The complete failed test and binaries are retained in local ignored checkpoint50. This is an architectural counterexample in the inactive replacement, not a passing regression or evidence of a deployed new failure.

The proposal hash commits its execution-context bytes. Including native output/write claims there creates a cycle for real instructions that persist the current block hash; transfer-only controls missed it. Revision-4 Prepare/Commit signatures already authenticate the actual result-bearing wire and state roots through ExecutionCommitment. The next candidate therefore carries only exact applying pre-State and authenticated native input Decisions, executes under the actual source-bound proposal header, retains aliases/prefix roots privately, and projects complete outputs once into BlockResult. No stripped execution header, output commitment in proposal, or compatibility alias is retained. Its source/model/test migration is an outside draft and remains unqualified at this record.

The full-result versus resultless fixture correction, repeated-Time invocation ownership, actual FASTPQ capture retention and common ordinary-tail drafts remain separately scoped. Full native suffix/witness/publication/Apply, query readers, runner/transport, old-signer and Ordinary economic bypass retirement, and real four-/seven-validator qualification remain open. Scoped formal122 and full structural148-error evidence retain their preceding source epochs. Main remains integrated only through49; its independent staged merge is untouched and the composition is uncompiled. All L1–L6 remain OPEN.

## 2026-09-17 source-only proposal qualification

Superseded current-status and goal text is retained verbatim below.

The isolated candidate 51 builds with all 20,541 local inputs unchanged and passes 414/415 selected Core and all 341 model tests, including the repair crash-cut controls. One historical-recovery fixture incorrectly equates full-result and resultless headers. Candidate 52 then confirms an architectural failure: real asset registration succeeds in preparation but cannot replay because native output claims change the executing block hash. Neither compact-output candidate is integrated. Main retains the ordered-stage integration through49; its staged merge is unchanged and composition uncompiled.

The replacement remains inactive in production, with native State commit explicitly rejected. The next design uses source-only proposals and actual proposal-header execution; existing global ExecutionCommitment certifies complete outputs. Prefix roots and authenticated aliases stay private. Full result/FASTPQ ownership, pipeline/Time/witness/publication and native Apply, fair runner/transport/refresh, old-signer and Ordinary bypass retirement, committed fee observability and real four-/seven-validator qualification remain required. The full structural gate still has 148 binding errors. Exact scope, failures and superseded plans are in the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

Candidate 51's compact-output follow-up passes 414/415 selected Core and all
341 model controls on unchanged source; one historical full/resultless-header
fixture assertion fails. Candidate 52 demonstrates a deeper cycle with actual
asset registration: proposal output claims alter the executing header hash, so
real replay diverges from stripped-header preparation. Both remain unintegrated.

Replace the draft wire transcript with input-only LaneDecisionBatchV1: exact
applying pre-State and ordered authenticated Decisions. Execute under the actual
source-bound proposal header; keep actual aliases and prefix roots private.
Existing global ExecutionCommitment authenticates full outputs once in canonical
BlockResult. Do not normalize the execution header or retain output-claim shims.
Qualify actual registration/header identity and repeated Time invocation ownership,
then preserve complete FASTPQ capture custody through the common suffix/witness.
Full native result/query/proof projection, publication and durable Apply/cleanup,
committed fee observability, fair production runner/transport/refresh and atomic
old-signer/Ordinary bypass retirement remain open. All L1–L6 and the real
four-validator counterexample remain OPEN; four-/seven-validator acceptance must
use one unchanged completed candidate. Exact evidence and superseded plans are
retained in the foundation history.


Earlier blanket self-hash restriction, superseded by the exact acyclic proposal/output boundary:

   Ordinary, autonomous and Native participant frontier projections all enter
   this rule. Do not put a block's own hash into its WSV or witness commitment.

Candidate53 builds the source-only redesign but fails a model fixture compilation at its `u32`/`usize` priority index. Its scoped formal selection passes122 tests; the full structural checker still reports148 binding errors. Candidate54 corrects that test index and builds both library test targets. On 20,542 unchanged tracked/unignored local regular inputs it passes415/416 selected Core tests and344/345 block-model tests. The Core failure is an obsolete rejection of a valid successor timestamp under the removed duplicate-header rule; scratch replay leaves global chronology/signature/leader acceptance to ValidBlock. The model failure is the new global-output proof fixture's missing non-genesis parent authority. The source-only asset-registration regression passes under the actual proposal hash, including exact persisted incarnation, unchanged proposal identity after full outputs, repeat execution and rollback. Full-result versus resultless historical recovery, complete-input carrier/gossip regressions, and all21 selected Native AMX publication/repair/capacity controls pass. The repair controls include a real write/flush/fsync crash cut before temporary promotion, exact strict restart and same-index retry, and rejection of foreign, tampered and unowned temporaries. Candidate54 is retained in local ignored checkpoint52; its model failure remains recorded.

Candidate55 changes only three test sources: the Core carrier fixture now rejects the declared zero-time shape and positively exercises a valid re-signed later timestamp using the same source batch and actual overlay header, with exact rollback and no publication. Its two affected carrier controls pass. The model proof fixture and assertions are corrected: the exact native source batch declares the proof test's snapshot bootstrap parent before actual 3-of-4 BLS signing with four PoPs. The verifier is unchanged; no native State execution or source-quorum authority is fabricated. All345 selected block-model tests pass, including four independent full-output mutations that preserve proposal identity but invalidate the original finalized executed-wire anchor. Its exact build, binary reuse or rerun scope, hashes and unchanged inputs are in local ignored checkpoint53. Candidate54 retains415 Core passes and its obsolete fixture failure; candidate55 has two focused Core passes. Test results from distinct candidates are not pooled into a new full-suite pass.

The reviewed24-path source-only change is integrated into main, preserving its independent block fixtures and staged merge. The main composition is uncompiled. The proposal carries only the exact applying pre-State and immutable input Decisions; complete results occur once in BlockResult, and global ExecutionCommitment authenticates the result-bearing wire. Private constructor seals retain the actual carrier, authenticated aliases and prefix roots. No stripped execution header, public output/write claims, old native wire alias or result reconstruction remains.

The read-only witness audit exposes the remaining lifecycle gap: current capture starts after constructor hooks and resets inside the transaction kernel; native economics are suppressed; selected instrumented witness keys do not cover the complete persistent State delta. Complete staged snapshots have no necessary current-QC dependency, but deterministic candidate metadata currently sits behind finality-authorized application. Required work is one constructor-through-suffix capture lifetime, actual applied-fragment rollback/capture custody, complete deterministic pre/post projection through the existing global commitment, and exact projection recheck before publication. Finality-bearing physical receipts must remain outside the projection they certify. Actual proposal hashes may be persisted by instructions; output/current-QC/executed-wire commitments must never feed back into proposal-bound inputs.

Actual native FASTPQ retention and common full-result ordinary-tail drafts remain outside and unqualified at this checkpoint. Native production acceptance and State commit stay refused; full native suffix/publication/durable Apply, runner/transport/refresh and atomic old-signer/Ordinary economic bypass retirement remain open. The real four-validator silent-author counterexample is not closed. All L1–L6, full structural/workspace gates and real four-/seven-validator same-candidate acceptance remain OPEN.

## 2026-09-17 retained sources and shared ordinary finalization

Superseded current-status and goal text is retained verbatim below.

The source-only native proposal candidate passes415/416 selected Core tests, including actual proposal-hash registration and all21 publication/repair/capacity controls; its obsolete timestamp-rejection fixture is corrected and both affected carrier controls pass separately. The corrected proof fixture passes all345 selected block-model tests. Both preceding fixture failures remain recorded. The24-path source-only change is integrated into main with independent staged changes preserved. Exact artifact scopes and hashes are retained; main composition and full workspace remain uncompiled/unqualified.

The replacement remains inactive in production, with native State commit explicitly rejected. Source-only Decisions remove the demonstrated proposal/output hash cycle; actual results stay in BlockResult under global ExecutionCommitment. Activation still requires actual FASTPQ custody, one constructor-through-suffix witness lifetime, a complete deterministic State projection rechecked at publication, native Apply, fair runner/transport/refresh, old-signer and Ordinary bypass retirement, and real four-/seven-validator qualification. The full structural gate retains148 binding errors. Evidence and superseded failures are in the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

The source-only proposal candidate passes415/416 selected Core controls,
including actual proposal-hash registration, full-output identity stability, replay
and rollback. Its obsolete timestamp fixture is corrected and two focused carrier
controls pass separately. The corrected proof fixture passes all345 block-model
controls with exact-committee finality and four output mutations. Both preceding
fixture failures remain recorded; these are scoped artifact results,
not a full workspace or network pass. The24-path source-only change is integrated
into main, whose independent staged merge is preserved and composition uncompiled.

LaneDecisionBatchV1 carries exact applying pre-State and ordered authenticated
input Decisions. Actual aliases and prefix roots remain private. Full results
occur once in BlockResult under the existing global ExecutionCommitment. Do not
normalize the executing header or restore proposal output/write claims.

Next: retain actual native FASTPQ rows/captures through the common full-result
suffix; establish one capture lifetime before constructor hooks without inner
reset or nested lock acquisition; preserve applied-fragment witness rollback;
factor deterministic candidate metadata from finality-only publication authority;
and bind/recheck a complete final State projection through the global commitment.
Selected instrumented witness keys alone do not establish complete State coverage.
Native acceptance/commit remains closed until sole consumer, durable Apply and
atomic old-signer/Ordinary economic bypass retirement are qualified. Full native
query/proof/fee observability and fair runner/transport/refresh remain required.
All L1–L6 and the real four-validator counterexample remain OPEN; the full
structural gate retains148 errors and four-/seven-validator acceptance must use
one unchanged completed candidate. Exact evidence is in the foundation history.


Candidate 56 contains the retained native FASTPQ/common ordinary-tail extraction and fails compilation on two test-only API paths (DomainId ownership and Transfer::asset_quantity). No executable tests are claimed at 56. Its failure is retained in local ignored checkpoint 54. Candidate 57 corrects those fixture calls and isolates the entire standalone scratch constructor, including due start hooks, from the current unrelated global witness recorder. On20,548 unchanged tracked/unignored local regular inputs, the Core and model library test targets build. All 594 selected Core tests pass on that exact executable, including all 16 new controls and all 21 Native AMX publication/repair/capacity tests. The model executable is byte-identical to candidate 55's 345/345 passing artifact; model tests are not rerun at 57. Workspace format and the retired-codec guard pass. Exact source/binary hashes, selection and logs are retained in local ignored checkpoint 55.

Native output retention keeps actual FASTPQ rows and captures under the same StateBlock instead of extracting and losing source custody. A private executed-prefix seal rejoins exact rows, capture occurrences and explicit absence before the common source inventory can seal. Ordinary sequential and DAG paths share one finalization tail, preserving complete prefix results and binding Time receipts to actual invocation calls rather than repeated display hashes. Native production acceptance is still explicitly refused by that ordinary tail. The new scratch controls execute real due governance unlocks and unrelated pending witness overlays on one thread, and cover successful construction, constructor refusal and late marker failure with exact rollback/generation preservation.

Candidate 57's full structural check reports 167 errors: the prior 148 plus one newly missing include-inventory declaration repeated 19 times. A complete disposable source mirror adds only the two actual include paths, the regenerated authenticated inventory digest and omission controls. Its 34 selected source-loader tests pass; its full structural output is exactly the prior 148 diagnostics after path normalization. Neither gate is claimed clean. The 19-path reviewed runtime/inventory change is integrated into main with exact reviewed postimages and unchanged staged merge/refs. Its main composition remains uncompiled. No proof or release readiness follows from unit-source checks.

The scratch fix protects the existing lifecycle; the intended first-release design is StateBlock/StateTransaction-owned witness capture with atomic retirement of global SLOT/guard/reset/suppression authority. Applied FASTPQ maps/captures remain the sole transcript producer. Canonical rejected-result observations must be distinguished from abandoned speculative trials; failed writes/captures must not leak. Selected instrumented keys still do not establish a complete persistent State projection. Deterministic candidate metadata must be projected before signing and rechecked before publication without including current finality, executed-wire hashes or physical receipts in their own commitment.

A separate source audit identifies a pipeline result-owner gap: a direct independent batch has no seeded root call and is quarantined, while a nested ExecuteTrigger can produce actual receipts whose callback result is discarded and whose owner cannot join the block's network/Time result set. An outside real-execution counterexample is prepared but not compiled/executed at 57. The design direction is one typed canonical execution-output sequence for network, pipeline and Time calls, with full result/receipts once and explicit input-proof mapping. Synthetic Time display hashes and completion notifications cannot authenticate internal calls. Model, Core, proof/query and SDK consumers must migrate coherently without a compatibility path; this direction is not yet implemented.

Native publication/Apply, complete witness/projection, canonical output ownership, fair runner/transport/refresh, and atomic old-signer/Ordinary economic bypass retirement remain required. All L1–L6, the real four-validator silent-author counterexample, full structural/workspace gates and unchanged-candidate real four-/seven-validator acceptance remain OPEN.

## 2026-09-17 executed pipeline ownership counterexamples

Superseded current-status and goal text is retained verbatim below.

The replacement remains inactive in production, with native State commit explicitly rejected. Actual native FASTPQ custody and the shared ordinary full-result tail are qualified within the scoped candidate; whole scratch construction preserves unrelated witness ownership. Next are State-owned capture, a complete deterministic State projection, one typed network/pipeline/Time output owner, native Apply, fair runner/transport/refresh, old-signer and Ordinary bypass retirement, and real four-/seven-validator qualification. The pipeline output gap is source-audited with an unexecuted counterexample draft. Evidence and superseded failures are in the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

The pipeline counterexample is prepared outside and not executed at this checkpoint.

Candidate 58 adds the real pipeline execution tests and the already qualified include-inventory correction. Its nested callback test reproduces the receipt-owner refusal. Its direct callback test fails because the assertion searches the outer Display text for a nested typed invariant message; the public outer error is only "Validation failed". Candidate 59 changes that assertion to inspect the exact Validation/InstructionFailed/InvariantViolation and compares the completion against the actual public error display. Both tests then pass on 20,549 unchanged local regular inputs. Production Rust is unchanged from candidate 57. The model artifact remains byte-identical to candidate 55 and is not rerun. Local ignored checkpoints 56 and 57 retain the separate failure and corrected two-test pass; neither is pooled into a new full-suite claim.

The tests confirm an OPEN production defect: direct independent pipeline batches lack a root call identity and are quarantined before mutation. Nested ExecuteTrigger supplies a call, applies actual independent receipts and FASTPQ capture, but its callback result is discarded. Shared finalization cannot join those receipts to a network or Time owner, refuses, rolls back the carrier, and repeats the same refusal on retry. The test-only two-path change is integrated into main with exact reviewed postimages and unchanged staged merge/refs. Main composition remains uncompiled.

The complete candidate-57 source mirror with the three include-inventory corrections also passes all 124 selected queue_plan_pending formal controls, including the reviewed pending-membership ledger and persistence expectation. Its inputs remain unchanged. This is separate from the 34 passing source-loader controls and does not clear the 148 existing full structural diagnostics.

The next implementation gives every Network/Pipeline/Time invocation a pre-body call identity and one full result with owned receipts and completions. The existing canonical format must be replaced atomically across producers, replay, proofs and consumers; no compatibility format or native activation is introduced by these counterexamples. Actual State-owned witness capture, complete bounded State projection and aggregate output capacity remain open. All L1–L6 and unchanged-candidate real four-/seven-validator acceptance remain OPEN.

## 2026-09-17 isolated invocation identity and output model qualification

Superseded current-status text is retained verbatim.

The replacement remains inactive in production, with native State commit explicitly rejected. Actual native FASTPQ custody and the shared ordinary full-result tail are qualified within the scoped candidate; whole scratch construction preserves unrelated witness ownership. Next are State-owned capture, a complete deterministic State projection, one typed network/pipeline/Time output owner, native Apply, fair runner/transport/refresh, old-signer and Ordinary bypass retirement, and real four-/seven-validator qualification. Two actual-execution counterexamples now reproduce the pipeline output gap: direct callbacks lack a root call; nested callback results are discarded and finalization repeatedly refuses. The separate 124 pending-membership formal controls pass in the source mirror. Evidence and superseded failures are in the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

Candidate 60 introduced the inactive output DTO prototype and failed two schema derives because named enum variant fields were unsupported. Candidate 61 used named payload structs and tuple variants, added a borrowed ordinary/native input-source interface, and passed 15 model controls on 20,551 unchanged inputs. That 369.454-second build was model-only; neither Core nor existing full-block tests were run. Local ignored checkpoints 58 and 59 retain the separate failure and corrected pass.

Candidate 62 introduced the inactive fresh trigger-store action helper and failed four compilation checks: two missing Schedule test imports and two missing canonical Repeats schema identities. Candidate 63 imported Schedule from its defining module and gave the model-owned Repeats type its actual nominal NoritoSchema identity, with a variant/count/layout roundtrip control. The original helper package and both failed builds remain retained; neither failure is relabeled as a pass.

Candidate 63 builds Core and the model in 416.791 seconds on 20,553 unchanged tracked/unignored regular inputs. All eight actual-Set helper controls and all sixteen selected model controls pass. The helper authenticates use-time action preimages from actual persistent trigger state, including registration height, authoritative IVM artifact/code identities and retry fields; generation/refcount changes do not alter that persistent digest. The sixteen model controls comprise fourteen output-shape/identity controls, one borrowed native-source control, and the new Repeats canonical-frame control. Format and retired-codec checks pass. Full source and binary verification precedes local ignored checkpoint 61 sealing. Core binary SHA-256 is c9f4ab752e67c54d3c00c23b00a80b5869dc946c43232a9fb659b1ef66eef938; model binary SHA-256 is 76671b820d700ddec69197930fe38f00bcf964126eb829476b82dfb48e503ae1.

Both prototypes remain isolated and inactive. No BlockResult/header/proof/SDK format migration, live dispatcher, main-composition build, formal rerun, native Apply or network qualification is claimed. Full transaction errors, failed-root instruction projections and completion strings still prevent a bounded terminal guarantee. The outside State producer remains compile-fenced until pre-body resource reservation and all canonical consumers exist. Host allocation refusal must remain local non-voting failure; it cannot become a memory-dependent canonical transaction rejection. Completion custody, rollback on every refusal and Time incarnation guards are under separate source review. All L1–L6 and the real silent-initial-author counterexample remain OPEN.

## 2026-09-17 isolated canonical output format and capacity model

Superseded current-status and plan text is retained verbatim.

The replacement remains inactive in production, with native State commit explicitly rejected. Actual native FASTPQ custody and the shared ordinary full-result tail are qualified within the scoped candidate; whole scratch construction preserves unrelated witness ownership. Next are State-owned capture, a complete deterministic State projection, one typed network/pipeline/Time output owner, native Apply, fair runner/transport/refresh, old-signer and Ordinary bypass retirement, and real four-/seven-validator qualification. Two actual-execution counterexamples now reproduce the pipeline output gap: direct callbacks lack a root call; nested callback results are discarded and finalization repeatedly refuses. The separate 124 pending-membership formal controls pass in the source mirror. The isolated invocation-identity/output prototype separately passes 8 Core and 16 model controls; it is not integrated into main or live execution. Bounded terminal capacity and the canonical producer/consumer migration remain open. Evidence and superseded failures are in the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

An isolated output-model and fresh trigger-action identity prototype now passes
8 Core and 16 model controls on 20,553 unchanged local inputs (candidate 63).
These helpers are not integrated into main or the live dispatcher. The prior
model-only candidate passed 15 controls after two schema derive errors; the first
action-helper build then failed four compilation checks, corrected before the
candidate-63 run. None of these counts expands the scoped candidate-57 suite.
The actual State producer remains an outside draft. Before activation it requires
reserved bounded terminal capacity, complete rollback on refusal, and atomic
replacement of canonical block results, input/output proofs and consumers.

| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |

Candidate 64 failed four library checks while replacing the model format; candidate 65 corrected the borrowed encoder/import and failed two Copy-related lints. Candidate 66 compiled the library and failed four test fixture imports/macros. Candidate 67 corrected those paths, built on 20,556 unchanged local inputs, and ran the full default model unit suite: 3,865 passed, five failed and six ignored. All 388 block controls passed. Four failures were expected wire-capture drift from the deliberate header/output/query replacement. The fifth, `isi::generated_record_identity_tests::inventory::privacy_register_privacy_protocol_activation_v1`, fails identically on retained candidate 63 with `length mismatch`; this is a historical compiled-binary comparison, not current-source qualification. These failures are retained separately in local ignored checkpoints 62–65.

Candidate 68 builds with `http,fault_injection` in 370.387 seconds and runs four explicit fixture generators successfully with all 20,556 inputs unchanged. Each generator preserves the actual roundtrip, schema, truncation and borrowed/owned identity checks. The six JSON fixtures change only four header/block concrete families, three header-bearing generic families, the block-message projection, two RegisterVerifiedLaneRelay cases and two committed Network rows in each full/IDs query fixture. Unaffected captured values, including the separate merge-inclusion DTO, remain exact. Both query fixture digests change together. Candidate 68 and its captures are retained in checkpoint 66; no full-suite pass is asserted there.

Candidate 69 builds with `http,ids_projection,fault_injection` in 366.527 seconds, with no compiler warnings and all 20,556 inputs unchanged. Its three explicit capture generators pass and independently match the updated generic, Nexus and IDs fixtures. The complete serial model unit suite lists 3888 tests: 3878 pass, one fails and 9 are ignored in 123.561 seconds. All 388 block-model tests pass, including full-output proof/finality, exact executed-wire length, proposal-header stability, atomic attachment and linear terminal-capacity controls. The sole failure remains the unchanged privacy-activation capture above; the full suite is not green and it is not waived. Format, diff and retired-codec checks pass. Input and binary hashes remain unchanged. Model binary SHA-256 is `18fa6428ecc74125a0c87b66ce0e11354c9cb447d713eca716baa39aab8ae66c`. Local ignored checkpoint 67 retains exact sources, executable, commands and outcomes.

The isolated model removes synthetic Time inputs, the header result root, parallel result/completion vectors and the old merge query branch. `BlockResult` owns one typed output collection and its checked cache; every proof hashes the full row and explicitly joins a Network source. Complete metadata is checked before output attachment, and final signatures require full-wire revalidation. The non-Copy output budget reserves all terminal obligations; oversized results cannot consume another invocation's fallback. These model checks do not authenticate State policy or reserve host allocations/metadata, and no Core producer cutover is compiled or activated.

The next State-owned plan must freeze the Time/output policy before input execution: current code reads max_transactions after earlier work may mutate it. Each multi-route native group is one complete input, while its route/source/settlement bytes remain indivisible. Later registry/policy changes must preserve every accepted source's feasible terminal envelope. The 32-MiB authenticated proof cap and 256-MiB consensus ceiling still require one admission/delivery policy. Actual allocation/trace bounds, State-owned witness/capture and complete metadata sizing remain open. The planned client migration removes duplicate transaction-detail completion summaries in favor of the actual full output; that work remains uncompiled and unintegrated.

This format replacement remains isolated. Main retains its earlier reviewed repair and scoped runtime changes, with its independent staged merge preserved. No main-composition, current Core, SDK, formal, native runner/commit or four-/seven-validator qualification follows. All L1–L6 and the actual silent-initial-author counterexample remain OPEN.

## 2026-09-17 explicit proof context and rollback ownership

Superseded current text is retained verbatim.

The replacement remains inactive in production, with native State commit explicitly rejected. Actual native FASTPQ custody and the shared ordinary full-result tail are qualified within their scoped candidate; whole scratch construction preserves unrelated witness ownership. Two actual-execution counterexamples reproduce missing pipeline root identity and discarded callback outputs. The isolated canonical model now separates network inputs from typed Network/Pipeline/Time outputs, retains proposal-only headers, authenticates full output proofs and reserves bounded terminal rows. Its HTTP/ID-projection/fault-injection suite passes 3878 tests, fails one existing privacy capture, and ignores 9; all 388 block-model controls pass. The same privacy failure reproduces on the retained pre-migration binary. Core integration, frozen carrier/output admission policy, complete State-owned capture and wire accounting, native Apply, runner/transport/refresh and old-signer retirement remain open. The prior 8 Core identity and 124 pending-membership controls are separate evidence; no current full-suite, main-composition or network qualification is claimed. See the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

The isolated canonical model now has proposal-only headers and one complete typed
Network/Pipeline/Time output collection, with independent network-input/output
proofs and finite pre-reserved terminal arithmetic. Candidate 69 builds the model
with `http,ids_projection,fault_injection` on 20,556 unchanged local inputs:
3878 tests pass, one existing privacy-activation capture test fails, and
9 are ignored. All 388 block-model controls pass. The privacy failure also
reproduces on the retained pre-migration candidate-63 binary; the full suite is
not green. Generated header, stream, relay and query fixtures replace only the
reviewed changed records; all other captured values and assertions remain.
The prior eight actual-Set identity controls remain separate candidate-63 evidence.
No current Core, main composition, SDK, formal or network qualification follows.

Candidate 70 builds the isolated data-model HTTP/ID-projection/fault-injection unit executable in 147.927 seconds, with zero compiler warnings and all 20,556 inputs unchanged. Its complete serial suite lists 3891 tests: 3881 pass, one fails and 9 are ignored in 124.022 seconds. All 391 block-model tests pass. The sole failure remains `isi::generated_record_identity_tests::inventory::privacy_register_privacy_protocol_activation_v1`, already reproduced on the pre-migration candidate-63 binary; it is retained and not waived. Binary SHA-256 is `bf8b97f0d9e6faadc42496aa5c1548c1cc3be839302c39a67375c1bac5f52a69`. Exact inputs, source overlays, command, binary and results are retained in local ignored checkpoint 68.

Both trusted proof-anchor constructors now require an independently trusted target HeightContextId, checked before cryptography or wire work. A real alternate four-key roster with three Commit votes and four PoPs signs the same proposal and executed wire; it verifies on its own but is refused under the expected context by both constructors. Matching independently selected contexts remain accepted. Existing CLI and JS verification already authenticates pinned chains, so their call sites pass the verified target context after any transition. This closes the public model API's implicit trust prerequisite; it does not demonstrate a previous deployed caller bypass. Those CLI/JS edits are source-reviewed but not compiled in this model-only candidate.

Two further controls check rollback output shape through both per-row and aggregate validation. A rejected Network input has no callback completions, regardless of whether a forged record says Success or Failure. A rejected Pipeline or Time invocation retains exactly one matching callback-zero Failure for the whole invocation, for both returned and declared root diagnostics. Missing, wrong-root/ordinal, successful and nested completions are refused. General failure prose is diagnostic; the typed transaction rejection owns the error. The separately bounded OutputLimit terminal keeps its exact fixed reason contract. No execution producer or State rollback path is activated by these model checks.

The Rust-client/shared-DTO draft has a separately preserved reviewed successor: local bindings check both proof positions, rejected Network completions are refused, shared fixtures obey rollback shape, and the real-BLS reader test explicitly uses an independently pinned verifier. Rustfmt and patch applicability pass, but no client/shared compilation or execution is claimed. Its Torii/CLI/integration constructors, Core query producers and persisted query indexes still require atomic migration. The State producer remains outside and compile-fenced. Full resource admission, State capture, old-signer retirement and real four-/seven-validator qualification remain open; native production acceptance and State commit stay closed, and all L1–L6 remain OPEN.

## 2026-09-17 first canonical client build and remaining source owners

Superseded current wording is retained verbatim.

The replacement remains inactive in production, with native State commit explicitly rejected. Actual native FASTPQ custody and the shared ordinary full-result tail are qualified within their scoped candidate; whole scratch construction preserves unrelated witness ownership. Two actual-execution counterexamples reproduce missing pipeline root identity and discarded callback outputs. The isolated canonical model now separates network inputs from typed Network/Pipeline/Time outputs, retains proposal-only headers, authenticates full output proofs and reserves bounded terminal rows. Its HTTP/ID-projection/fault-injection suite passes 3881 tests, fails one existing privacy capture, and ignores 9; all 391 block-model controls pass. Proof anchors now require an independently trusted target context, and rejected output shapes discard rolled-back callback completions. The same privacy failure reproduces on the retained pre-migration binary. Core integration, frozen carrier/output admission policy, complete State-owned capture and wire accounting, native Apply, runner/transport/refresh and old-signer retirement remain open. The prior 8 Core identity and 124 pending-membership controls are separate evidence; no current full-suite, main-composition or network qualification is claimed. See the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

The isolated canonical model now has proposal-only headers and one complete typed
Network/Pipeline/Time output collection, with independent network-input/output
proofs and finite pre-reserved terminal arithmetic. Candidate 70 builds the model
with `http,ids_projection,fault_injection` on 20,556 unchanged local inputs:
3881 tests pass, one existing privacy-activation capture test fails, and
9 are ignored. All 391 block-model controls pass. The privacy failure also
reproduces on the retained pre-migration candidate-63 binary; the full suite is
not green. Both proof anchor constructors require an independently trusted target
height context; actual alternate-roster BLS controls reject circular self-asserted
roster authority. Existing CLI/JS callers already pin their chain and now pass the
verified target context; their current composition is uncompiled. Rejected Network
outputs forbid callback completions, while rejected internal outputs retain only
one matching root Failure for the whole invocation. The three new controls pass.
Generated format fixtures and all preceding assertions remain in force. The prior
eight actual-Set controls remain separate candidate-63 evidence. No current Core,
main composition, SDK, formal or network qualification follows.

query, proof and SDK consumer atomically. Remove duplicate transaction-detail
completion summaries; actual full outputs own completions. The State producer
is still a compile-fenced outside draft and the client migration is unqualified.

Candidate 71 applies the reviewed Rust-client/shared response source plus every remaining details-response constructor, their underlying Network proof fixtures and a Torii response assertion (13 paths). Its first combined SDK/shared unit build runs 189.170 seconds on 20,556 unchanged local inputs and fails on 15 SCCP library/test-fixture references to retired result/header APIs. No consumer tests execute. The failure, exact source capture and diagnostics are retained in local ignored checkpoint 69. The source remains isolated; main receives no model/client/protocol change.

Review then finds a shared-test feature leak: directly constructing CommittedTransaction fields depends on the SDK's transparent_api feature. The fixture is changed to its public Norito JSON decoder and getters without expanding production features. Standalone shared-crate validation is required. The three byte-identical authored/current/package OpenAPI copies remove only the retired generic header result root and duplicate details completion field, with corrected outer-entrypoint identity wording. All other JSON values remain identical. Existing document validation passes; the new runtime schema-vs-actual-header test is uncompiled, and changed release metadata must be regenerated from a clean authored source without reusing historical manifests.

The direct Core query migration remains an outside uncompiled draft. Inspection corrects earlier shorthand about persisted query indexes: Kura's transaction/Kaigi maps are resident derived data rebuilt from authenticated bodies; the old phase/position is serialized into Torii continuation cursors. Index construction/rebuild/update/prune and cursor representation must migrate atomically. Internal outputs cannot become synthetic CommittedTransaction rows; their history and proofs need the separate actual-output owner.

The earlier phrase "proposal-only header" described generic result-root removal too broadly. Output attachment demonstrably preserves the signed header, but the header still carries an SCCP root whose existing Core staging is outcome-dependent. Its complete proposal/metadata ownership must be resolved before cutover; no cycle-free SCCP execution or native delivery is established by the model tests. Native scratch currently rejects SCCP header roots. SCCP's consumer migration must retain actual QC/exact-wire/SCCP membership checks, not treat input or result-root presence as execution authentication. All native/runtime, full-resource, State-capture, workspace and real four-/seven-validator gates remain open.

## 2026-09-17 canonical client and SCCP consumer qualification

Superseded current wording is retained verbatim.

The replacement remains inactive in production, with native State commit explicitly rejected. Actual native FASTPQ custody and the shared ordinary full-result tail are qualified within their scoped candidate; whole scratch construction preserves unrelated witness ownership. Two actual-execution counterexamples reproduce missing pipeline root identity and discarded callback outputs. The isolated canonical model now separates network inputs from typed Network/Pipeline/Time outputs, removes the generic header result root, authenticates full output proofs and reserves bounded terminal rows. Its HTTP/ID-projection/fault-injection suite passes 3881 tests, fails one existing privacy capture, and ignores 9; all 391 block-model controls pass. Proof anchors now require an independently trusted target context, and rejected output shapes discard rolled-back callback completions. The same privacy failure reproduces on the retained pre-migration binary. Core integration, outcome-derived SCCP root staging, frozen carrier/output admission policy, complete State-owned capture and wire accounting, native Apply, runner/transport/refresh and old-signer retirement remain open. The prior 8 Core identity and 124 pending-membership controls are separate evidence; no current full-suite, main-composition or network qualification is claimed. See the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

query, proof and SDK consumer atomically. The Rust-client/shared isolated source
now removes duplicate details completions and migrates their remaining constructors;
its first build (71) stops on 15 retired SCCP APIs before any consumer tests run.
The State producer is still a compile-fenced outside draft. Resident transaction/
Kaigi indexes rebuilt from canonical bodies and serialized Torii phase cursors must
migrate together; there is no separately persisted index to translate. Internal
invocations require their own output-history/proof owner. Existing outcome-derived
SCCP header-root staging is another open dependency: generic result-root removal
and immutable attachment tests do not prove that path free of proposal/output
cycles. Native scratch still excludes SCCP roots. Three OpenAPI source copies now
match the changed header/details shapes and pass document validation; Torii runtime
checks and clean-source release metadata regeneration remain unqualified.

| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner with frozen carrier capacity and complete admitted-source budgets, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |

Candidates 72, 73 and 74 retain compile failures: a shared-test Norito JSON macro expression, two stale SDK header constructors, then SCCP proof-anchor imports from the wrong module. No tests ran on those failed builds. Candidate 75 compiles without warnings but its complete SDK/SCCP suites fail 10/38 tests, all at the same SCCP fixture: a physical transaction was supplied with an empty input root. Its 840 SDK, 301 shared and 159 SCCP passes are retained alongside those failures, not treated as a green suite. Standalone shared candidate 76 passes 301 tests but exposes an unused model import without transparent_api.

Candidate 77 fixes the fixture's input commitment before signing, explicitly validates the complete proposal and asserts that attaching outputs preserves the header and canonical resultless proposal. It also scopes the construction-only model import to its feature. Its combined build and full serial suites pass: 850 SDK, 301 shared and 197 SCCP tests, with no ignored tests. Standalone shared candidate 78 and SCCP candidate 79 pass all 301 and 197 tests. All three builds preserve the same 20,556 captured local source/include/config inputs. The combined and standalone shared builds have zero warnings; standalone SCCP reports one unused validate_capacity_intent method in unchanged governance code, so it is not warning-free or a strict Clippy pass. Exact source overrides, private executable hashes, inventories and logs are retained in local ignored checkpoints 75–77. Formatting, diff and retired-codec checks pass. This qualifies the isolated library consumers only: main's model/client/runtime composition, CLI/JS/Torii, actual State/Kura and real four-/seven-validator behavior remain unqualified. The earlier model suite still has its separately reproduced privacy capture failure.

The bounded SCCP source audit finds no production root-preparation owner in the inspected runner: candidate_attachments uses the default None root, while the execute-then-fill helper is cfg(test). Successful outbound records are checked against a header root derived from outcomes, and current header identity can itself affect execution. This is a source finding, not an executed network counterexample. The first-release design must remove this proposal field and derive a private bounded root/count from actual applied outbox fragments, authenticated by the existing global ExecutionCommitment. Actual Network/native/Pipeline/Time/nested execution and whole-invocation rollback must share that owner; input instruction scans and speculative header rewrites cannot replace it. Archives, remote SCCP membership/circuits and governed identities must move atomically to the QC subcommitment. Native SCCP remains rejected; no deployed corridor or new signing preimage is enabled here.

The outside direct Network query draft is independently reviewed and remains unintegrated. Three corrections are required: uncached inline reloads must use the existing exact published-finality body owner (trusted startup/admitted cached bodies already have earlier custody), authenticated durable wire bytes must be charged before decoding and complete-carrier validation with lazy row cloning, and FindTransactions must not introduce unbounded history-wide output retention. Existing explicit proof joins, sealed outer identity and cache-before-emission checks remain. Actual Kura corruption/reload, native carrier, internal-only byte-budget and early-stop controls are required before integration. No runtime exploit or test is claimed by this source review.

## 2026-09-17 canonical Core history and Kura index migration

Superseded current wording is retained verbatim.

The replacement remains inactive in production, with native State commit explicitly rejected. Actual native FASTPQ custody and the shared ordinary full-result tail are qualified within their scoped candidate; whole scratch construction preserves unrelated witness ownership. Two actual-execution counterexamples reproduce missing pipeline root identity and discarded callback outputs. The isolated canonical model now separates network inputs from typed Network/Pipeline/Time outputs, removes the generic header result root, authenticates full output proofs and reserves bounded terminal rows. Its HTTP/ID-projection/fault-injection suite passes 3881 tests, fails one existing privacy capture, and ignores 9; all 391 block-model controls pass. Proof anchors now require an independently trusted target context, and rejected output shapes discard rolled-back callback completions. The same privacy failure reproduces on the retained pre-migration binary. The isolated Rust SDK/shared/SCCP consumer suite now passes 850/301/197 tests; standalone shared/SCCP also pass 301/197. Core integration, the absent production SCCP outbox-root owner, frozen carrier/output admission policy, complete State-owned capture and wire accounting, native Apply, runner/transport/refresh and old-signer retirement remain open. The prior 8 Core identity and 124 pending-membership controls are separate evidence; no current full-suite, main-composition or network qualification is claimed. See the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

The State producer is still a compile-fenced outside draft. Resident transaction/
Kaigi indexes rebuilt from canonical bodies and serialized Torii phase cursors must
migrate together; there is no separately persisted index to translate. Authenticate
exact executed bodies and charge full carrier/output work before projection;
clone queried rows lazily without new whole-history retention.

| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner with frozen carrier capacity and complete admitted-source budgets, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |

Isolated candidates 80 and 81 implement one Network-only resident transaction/Kaigi index and remove merge-sidecar/bodyless promotion from that derived owner. Complete proposal/native-source/output/cache validation precedes all membership publication. Physical merge storage, receipt/finality authentication, publication capacity and repair remain separate and unchanged in authority. Existing physical repair controls now require that repair cannot resurrect the retired transaction projection. The cursor names an explicit u32 Network input index; internal Pipeline/Time outputs are not transaction entries.

The query source reuses Kura's exact published-finality body reader. A caller first admits the authenticated durable wire length; the reader rechecks that exact length and canonical hash under the existing guards, then binds all bytes to the CommitQC wire hash before decoding. Evicted replicas use the same exact physical byte limit. An immutable Arc and its input tree have one projector owner; complete carrier validation precedes emission, and borrowed seven-field row sizing plus a bounded counting serialization precedes each clone. Empty/nonmatching carriers consume cumulative work; internal outputs consume validation work without becoming transactions. The dedicated stored/ephemeral fallible page path replaces the unbounded FindTransactions ValidQuery owner. Torii status/details/cache source edits use the canonical Network reader; remaining completion/event and other consumers are not yet migrated or compiled.

Read-only review caught that the initial durable-length preflight reached get_block through retained-record authentication before the caller's byte admission. Candidate 81 uses the existing metadata-only replica/finality authority instead. Its outer CanonicalHistorySource regression uses actual per-BlockStore byte-read accounting: a denied cold-body request must read zero bytes, including when occupied body bytes are corrupt; admission must read exactly the signed length and either return the exact body or reject its wire. Lazy inline reloads also cannot republish a body/index without exact finalized-wire custody. Existing authenticated live-append cache custody is retained.

Candidate 80 fails its Core library check with 118 errors and 112 warnings after 205.422940958 seconds; candidate 81 fails with 106 errors and 110 warnings after 73.724821458 seconds. Both preserve all 20,559 captured local source/include/config inputs. The follow-up removes the projector's trivial-cast lint error, stale null-header constructor arguments and an impossible gossip Time arm. Neither check produces or runs a test executable. Nineteen new controls are authored (five resident-index, five physical/read-admission, nine structural query); none is claimed passing. Exact failed diagnostics and candidate sources are retained in ignored checkpoints 78/79. No strict Clippy, full Core/Torii, main-composition, formal, workspace or network qualification is claimed. Earlier 594-Core repair/common-tail and SDK/shared/SCCP passes remain separate source-bound evidence.

Two query resource obligations remain open beyond this slice: positive query planning still materializes resident height sets before scanning, and wire/work admission is not an allocation reservation for the decoded graph and retained query output. A bounded descending selection/continuation owner must preserve complete canonical prefix/index authority and allow small pages over mature indexes; refusing every high-cardinality index would make those queries permanently unusable. Canonical fanout stays explicitly refused until its resident-memory owner is integrated. State-owned callback output capture, frozen complete-source/output admission geometry, SCCP outbox authority, native Apply/runtime integration and real four-/seven-validator acceptance remain open. No liveness goal is closed.

## 2026-09-17 committed Network proof serving migration

Superseded current wording is retained verbatim.

The replacement remains inactive in production, with native State commit explicitly rejected. Actual native FASTPQ custody and the shared ordinary full-result tail are qualified within their scoped candidate; whole scratch construction preserves unrelated witness ownership. Two actual-execution counterexamples reproduce missing pipeline root identity and discarded callback outputs. The isolated canonical model now separates network inputs from typed Network/Pipeline/Time outputs, removes the generic header result root, authenticates full output proofs and reserves bounded terminal rows. Its HTTP/ID-projection/fault-injection suite passes 3881 tests, fails one existing privacy capture, and ignores 9; all 391 block-model controls pass. Proof anchors now require an independently trusted target context, and rejected output shapes discard rolled-back callback completions. The same privacy failure reproduces on the retained pre-migration binary. The isolated Rust SDK/shared/SCCP consumer suite now passes 850/301/197 tests; standalone shared/SCCP also pass 301/197. The isolated Kura/Network history migration now has exact finalized-wire reads, metadata-only byte admission, one validated Network index and lazy row projection. Its Core checks fail with 118 then 106 migration errors on unchanged inputs; 19 new controls are authored but unrun. Complete bounded index selection and decode/resident reservations remain open. Core integration, the absent production SCCP outbox-root owner, frozen carrier/output admission policy, complete State-owned capture and wire accounting, native Apply, runner/transport/refresh and old-signer retirement remain open. The prior 8 Core identity and 124 pending-membership controls are separate evidence; no current full-suite, main-composition or network qualification is claimed. See the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

The State producer is still a compile-fenced outside draft. Isolated Kura now
indexes only complete validated Network sources/outputs; Kaigi cursors use the
Network input index. Queries and status readers bind exact finalized wire and
charge metadata-authenticated body bytes before reading, then project one row
from an immutable carrier after complete validation. Old merge/bodyless index
promotion and the unbounded FindTransactions iterator owner are removed. Core
checks 80/81 fail with 118/106 migration errors; all 20,559 captured inputs stay
unchanged during each check. Nineteen new controls are authored but unrun.
Finish remaining State/Core/Torii consumers and existing fixture migration before
running them. A reviewed preflight that indirectly loaded cold bodies is corrected
through the existing metadata-only replica authority, with a zero-body-read denial
regression. Bound positive index selection/continuation without cloning historical
height sets; integrate decode-graph and resident-memory reservations before
claiming end-to-end query bounds or enabling canonical fanout.

Three OpenAPI source copies
match the changed header/details shapes and pass document validation; Torii runtime
checks and clean-source release metadata regeneration remain unqualified.

| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner with frozen carrier capacity and complete admitted-source budgets, finish bounded exact-finality Network history/index consumers and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |

Isolated candidate 82 moves committed Network proof serving onto the exact finalized Kura body authority. State captures one committed journal hash, releases its view, and rechecks that height after the read/projection; unrelated later appends do not invalidate it. The retired StateReadOnly proof builder and its equal input/result-count assumption are removed. Complete source/output/cache validation precedes selection, and the outer Network input index joins its full typed output in a separately sized output tree. Internal Pipeline/Time outputs do not become transaction inputs. The raw executed-wire endpoint returns the original QC-authenticated bytes from the same physical read, without re-encoding the carrier.

Finite per-request wire, work and response ceilings refuse zero allowances. Metadata-authenticated wire length is admitted before body I/O. Aggregate source/output/transcript work is admitted before structural validation/tree construction. Complete borrowed nine-field proof encoding, including its typed output and transcript map, is sized before large response clones; owned and borrowed lengths must agree. Torii derives the work cap from configured query_max_fetch_size and uses the existing 32 MiB proof protocol cap for wire/response bytes. Byte refusals return 413, work refusals 429; missing/corrupt finalized body authority is an internal failure rather than ordinary absence. These controls do not yet reserve the decoder graph, validation scratch or retained resident response, and the 32 MiB proof versus 256 MiB consensus ceiling remains unresolved.

The regression handoff contains fifteen dedicated Core proof tests, one preserved sealed-commitment wrapper and two Torii positive fixtures, all authored/migrated but unrun. It covers actual four-key/three-vote BLS+PoP finality, exact wire custody, unequal input/output trees, outer sealed identity, source/output/cache corruption, missing authority, denied zero-byte cold reads, exact resource limits, and original wire serving. These are fixture-based transport/proof controls, not actual economic execution, strict restart, native/SCCP qualification or independent remote trust-pin adversary evidence. Additional parent-owned Torii capacity/error mapping controls are also unrun.

Candidate 82's Core library check fails with 95 errors and 110 warnings after 74.76318483403884 seconds; all 20,562 captured local inputs remain unchanged. It reports no errors in the new production proof/storage-reader code but produces no test executable and runs no tests. Remaining diagnostics belong to State production, block execution, bridge/SCCP and other unmigrated consumers. Exact failed inputs/diagnostics are retained in ignored checkpoint 80. The prior 594-Core repair/common-tail and SDK/shared/SCCP passes remain separate source-bound evidence; no current Core/Torii, formal, workspace, main-composition or network qualification is claimed.

After that check, only three identical OpenAPI authorities and four status/plan/history documents change. All three authorities pass the existing static document validator (547 paths, 1,224 schemas); mirror and duplicate-member parser tests pass 2/2. Initial Python invocations lacked pytest/jsonschema; the existing private test environment received jsonschema and the same tests then passed. These checks do not regenerate or attest clean release provenance, compile Torii, or execute either endpoint. Source changes remain isolated; MAIN retains the previously qualified authenticated repair-temporary fix and current status documents. L1–L6 and the real four-validator silent-initial-author counterexample remain OPEN.

## 2026-09-17 Network admission and queue migration

Superseded current wording is retained verbatim.

The replacement remains inactive in production, with native State commit explicitly rejected. Actual native FASTPQ custody and the shared ordinary full-result tail are qualified within their scoped candidate; whole scratch construction preserves unrelated witness ownership. Two actual-execution counterexamples reproduce missing pipeline root identity and discarded callback outputs. The isolated canonical model now separates network inputs from typed Network/Pipeline/Time outputs, removes the generic header result root, authenticates full output proofs and reserves bounded terminal rows. Its HTTP/ID-projection/fault-injection suite passes 3881 tests, fails one existing privacy capture, and ignores 9; all 391 block-model controls pass. Proof anchors now require an independently trusted target context, and rejected output shapes discard rolled-back callback completions. The same privacy failure reproduces on the retained pre-migration binary. The isolated Rust SDK/shared/SCCP consumer suite now passes 850/301/197 tests; standalone shared/SCCP also pass 301/197. The isolated Kura/Network history migration now has exact finalized-wire reads, metadata-only byte admission, one validated Network index and lazy row projection. Committed Network proof serving now uses that exact body authority, separate input/output trees, pre-I/O wire admission and bounded proof serialization; raw serving returns the original finalized bytes. Core check 82 still fails with 95 errors and 110 warnings on 20,562 unchanged inputs. The prior 19 controls and the migrated proof fixtures remain unrun; no errors were reported in the new proof/storage reader. Complete bounded index selection and decode/resident reservations remain open. Core integration, the absent production SCCP outbox-root owner, frozen carrier/output admission policy, complete State-owned capture and wire accounting, native Apply, runner/transport/refresh and old-signer retirement remain open. The prior 8 Core identity and 124 pending-membership controls are separate evidence; no current full-suite, main-composition or network qualification is claimed. See the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).

Core check 82 fails with 95 errors and 110 warnings; all 20,562 captured inputs
stay unchanged. No diagnostic names the new proof/storage reader, but this is
not a successful build or test qualification. Fifteen dedicated Core proof tests,
one existing sealed-commitment wrapper and two Torii positive fixtures now use
real exact-wire finality; they and the prior 19 history/index controls remain
unrun. Finish remaining State/Core/Torii consumers and fixture migration before
running them.

| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner with frozen carrier capacity and complete admitted-source budgets, finish bounded exact-finality Network history/proof consumers, resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |

Candidate 83 applies a three-file, first-release Network admission/queue migration in the isolated checkout. Seventeen impossible Time-input branches are removed from tx.rs, queue.rs and queue/router.rs without a wildcard, shadow entrypoint or compatibility decoder. External, SealedCommitment and SealedReveal branch behavior is preserved, including signed network/signature/TTL/clock/size checks, outer/inner replay aliases, sealed duplicate protection, routed pre-block authentication, proposal gas bounds and direct External-only KAGEMUSHA operation custody. Internal invocations have no queue/admission representation. No producer, execution output, State lifecycle, SCCP root, native gate or signing path is enabled.

Two new tests cover retired tag-3 framed ingress refusal and supported sealed routing/identity/cost behavior. Three existing fixtures migrate from fake Time inputs to real signed/sealed inputs while retaining their assertions; the proposal-gas test additionally refuses a missing signed runtime gas limit wrapped in a reveal. Existing sealed replay, duplicate/full-width descriptor, routing, NTS/TTL/signature, sibling-release and KAGEMUSHA suites remain required. All six changed controls and those broader suites remain uncompiled/unrun. The outside draft was formatted and reconstructed exactly forward/backward, then applied only after all three preimage hashes matched. Parent workspace formatting, diff and codec checks pass.

The captured Core library check fails with 78 errors and 110 warnings after 72.57905283407308 seconds, with all 20,562 local inputs unchanged. It removes all 17 targeted diagnostics and introduces no errors in the migrated admission/queue owners. This is not a successful build and no test executable is produced or run. Exact sources/diagnostics are retained in ignored checkpoint 81 and the admission handoff. Only four status/plan/history documents change after the check. MAIN source and its HEAD/MERGE_HEAD/staged merge remain untouched by this migration.

A separate read-only telemetry audit records why its remaining flat-results accessor errors require authenticated Network input/output classification, rather than counting an output prefix or relying on a proposal header. The actor's captured State prefix, private chunk deltas, publication/restart/timeout behavior and resource limits must remain coherent; the HTTP proof cap must not strand valid larger consensus bodies. This is next-step design work, not implemented status qualification. Frozen capacity and actual State output/callback capture, SCCP applied-outbox ownership, native publication/Apply, full consumer and fixture migration, formal/workspace gates and unchanged real four-/seven-validator liveness acceptance remain open. No liveness goal closes.

## 2026-09-17 Frozen Time ceiling and bounded trigger descriptors

Superseded current wording is retained verbatim.

The replacement remains inactive in production, with native State commit explicitly rejected. The isolated model separates Network inputs from typed Network/Pipeline/Time outputs; history and proof readers use exact finalized bytes and validated input/output joins. Admission and queue routing now accept only the three Network variants. Core check 83 fails with 78 errors and 110 warnings on 20,562 unchanged inputs; new history/proof/admission controls remain unrun. The earlier model suite retains one existing privacy-capture failure. State output production, frozen carrier capacity, complete allocation reservations, SCCP applied-outbox authority, native Apply, runner/transport/refresh and retirement of old signing/economic paths remain open. Earlier repair/common-tail/model/SDK results are separate source-bound evidence in the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md); no current Core/Torii, main-composition, formal, workspace or network qualification is claimed. All L1–L6 and the real four-validator silent-initial-author counterexample remain open.

| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner with frozen carrier capacity and complete admitted-source budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |

Next implement one State-owned output plan frozen after start hooks and before
network execution. Current Time matching reads a parameter that earlier inputs
can mutate; the applying carrier must retain its own cap, with later carriers
seeing committed changes. Registry/policy changes must preserve future capacity
for already-admitted complete sources. A native multi-route group is one input
and one Network output, but all route certificates and metadata remain indivisible.
Pre-body allocation/trace bounds, invocation rollback/capture custody and complete
non-output wire accounting remain required; the 32-MiB proof versus 256-MiB
consensus ceiling is unresolved.

consumer suite now passes 850/301/197 tests (candidate 77); standalone shared/SCCP
also pass 301/197 (78/79). Inputs remain unchanged; standalone SCCP retains one
unused-method warning in unchanged governance code.

Core check 83 fails with 78 errors and 110 warnings; all 20,562 captured inputs
stay unchanged. The 17 retired Time-input errors in tx/queue/router are removed;
Network admission remains exhaustive over External, SealedCommitment and
SealedReveal, preserving signature/replay/routing/gas and direct KAGEMUSHA rules.
Two new controls, three migrated fixtures and one extended gas test are unrun.
Fifteen dedicated Core proof tests, one existing sealed-commitment wrapper and
two Torii positive fixtures use real exact-wire finality; they and the prior 19
history/index controls also remain unrun. No diagnostic names the new proof reader
or the migrated admission/queue owners, but this is not successful build/test
qualification. Finish remaining State/Core/Torii consumers and fixture migration
before running these controls.

This isolated change captures the existing on-chain Time invocation ceiling once in the StateBlock constructor. Ordinary execution captures after every existing start hook and before its after-start continuation. Replacement construction captures its actual reverted parameters after its existing narrower initialization; this does not claim that ordinary lifecycle effects ran. No-hook merge/probe construction retains no limit and refuses scheduled Time before events or maintenance. Same-carrier parameter changes do not revise its ceiling; later constructors observe published parameters. Five authored controls cover saturation, actual native parameter writes, later-carrier observation, reverted initialization and pre-effect probe refusal. None is compiled or run, and none tests a real due-governance enactment. This is not yet complete output, matcher-allocation, registry-work or memory capacity.

The thirteen-file descriptor patch removes only the redundant TriggerUse authority field, retaining ID, persistent registration height and canonical action hash. The Core helper still commits the real authority first in its persistent action preimage; descriptor-derived calls therefore bind it. The execution owner must authenticate and execute that same freshly loaded action. No authority digest, compatibility decoder or alternate execution owner was introduced. The actual Set rekey regression changes both Time/Pipeline authorities to a larger multisig and checks action/call identity and constant descriptor length; it remains uncompiled. The two new model tests reject the retired JSON/raw/framed authority slot and round-trip maximum trigger names/scalar terminal fields. Existing model assertions and all prior test names remain. The descriptor is bounded independently of authority size, but action/metadata hashing, registry scans, traces, receipts and allocation still need bounds. The outside producer remains unapplied and must rebase its Time contract and fixtures.

Candidate 84's Core check fails with 79 errors and 110 warnings after 71.82436266704462 seconds on 20,563 unchanged inputs. The added inference error in the unfinished fallible tail is corrected with an explicit Vec hash type. Model build 85 passes in 165.82168579101562 seconds with no warnings, and its retained exact binary runs 3,893 tests: 3,883 pass, one previously established privacy activation identity-capture test fails with length mismatch, and nine are ignored. All 393 block tests pass, including both new descriptor controls. This remains a failed full suite. Core check 86 fails with 78 errors and 110 warnings after 104.56706254207529 seconds; its normalized error multiset equals 83, not a successful Core/test build. Consumer build 87 passes in 86.21438849996775 seconds without warnings; the retained SDK/shared/SCCP binaries pass 850/301/197 tests, none ignored. Each build and suite run preserves its own 20,563 captured local inputs and exact binary hashes; candidates 85/86/87 share identical input captures, while 84 has eighteen earlier source differences; this is local source/binary evidence, not external dependency attestation or runtime qualification.

Three identical OpenAPI copies now name output_commitment/output_proof and carry a full typed receipt output, removing obsolete equal input/output count wording and the hash-only leaf slot. The strict outer output tag/object shape uses the existing generic JsonValue for variant details; no unsupported complete schema graph is claimed. Norito documentation records the descriptor shape and authentication contract. Workspace formatting, diff, retired-codec guard, both focused OpenAPI mirror/parser tests and three Node shape/property checks pass. Generator/runtime/clean-release provenance gates remain unqualified.

A read-only follow-up separates agreed applying policy from local resource refusal. The prospective row count is N+(N+1)P+T; its N=1 specialization alone is not source admission. Future policy/registry transitions must preserve the complete indivisible source, routes, mandatory metadata, terminal rows, framing/cache and host reservation under allowed later registry growth. The current Time/block-transaction coupling is only preserved interim semantics, not a final independent scheduler policy. No scalar, output-only byte check or caller-claimed cap becomes State admission authority.

After testing, only four authoritative status/plan/history documents are updated in MAIN and the isolated checkout. MAIN production source, HEAD, MERGE_HEAD and staged merge are preserved; the earlier repair-temporary change retains its separate candidate57 evidence. State output/capture production, complete reservations and future-invariant admission, SCCP applied-outbox authority, remaining consumer/fixture migration, native Apply/runner and old-path retirement, formal/workspace and unchanged real four-/seven-validator acceptance remain open. All L1–L6 and the real silent-initial-author counterexample remain open.

## 2026-09-17 Agreed output capacity and canonical consumer migration

Superseded current wording is retained verbatim.

The replacement remains inactive in production, with native State commit explicitly rejected. In the isolated checkout, the constructor freezes the current Time invocation ceiling before Network/Pipeline execution, and internal descriptors retain authority authentication in the action hash without duplicating authority bytes. Model candidate 85 passes 3,883 tests (all 393 block controls), with one existing privacy-capture failure and nine ignored; affected SDK/shared/SCCP suites pass 850/301/197 tests (87). Core check 86 still fails with the same 78 errors and 110 warnings as 83, on 20,563 unchanged inputs. New State/helper/history/proof/admission controls remain unrun. Complete output/allocation reservations and future-invariant source admission, actual State output production, SCCP applied-outbox authority, native Apply and runner/old-path retirement remain open. The [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md) preserves exact evidence scopes. No current Core/Torii, main-composition, formal, workspace or network qualification is claimed. All L1–L6 and the real four-validator silent-initial-author counterexample remain open.

| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner with State-frozen output capacity, future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |

The ordinary State constructor now freezes the existing Time invocation ceiling
after its actual start hooks and before Network/Pipeline execution. Replacement
construction captures its reverted world after its narrower existing lifecycle;
no-hook probes refuse Time before effects. Five new controls remain uncompiled.
This count snapshot is not a complete output or host-allocation reservation.
TriggerUse carries bounded ID, registration height and action hash; the actual
authority remains authenticated inside that hash, with no duplicate authority slot.
Both new model codec/maximum-terminal controls pass in candidate 85, whose full
suite reports 3883 passed, one existing privacy-capture failure and nine ignored;
all 393 block controls pass. The actual-Set authority-rekey regression is unrun.
Next establish one agreed applying output policy and a State-owned complete plan.
Local resource refusals must never change canonical matching or terminal results.
Registry/policy changes must preserve future capacity for already-admitted complete
sources, including all route certificates and metadata of an indivisible native
multi-route group (one input and one Network output). A positive input count alone
is insufficient. Pre-body allocation/trace bounds, invocation rollback/capture
custody and complete non-output wire accounting remain required; the 32-MiB proof
versus 256-MiB consensus ceiling is unresolved. Compose the canonical producer and every Core,
query, proof and SDK consumer atomically. The isolated Rust SDK/shared/SCCP
consumer suite passes 850/301/197 tests again after the descriptor change
(candidate 87, no ignored tests or build warnings). All 20,563 captured inputs stay
unchanged. Earlier standalone shared/SCCP results (78/79) remain separate evidence. Details
have one full Network output owner; real BLS tests bind exact executed wire and
reject changed source/output/cache/context claims. Earlier failed candidates,
including the SCCP fixture's missing proposal input root, remain retained.
The State producer is still a compile-fenced outside draft. Isolated Kura now
indexes only complete validated Network sources/outputs; Kaigi cursors use the
Network input index. Queries and status readers bind exact finalized wire and
charge metadata-authenticated body bytes before reading, then project one row
from an immutable carrier after complete validation. Old merge/bodyless index
promotion and the unbounded FindTransactions iterator owner are removed.
Committed Network proofs now use the same exact finalized-body authority and
explicit input-index/output joins with independently sized trees. State captures
and rechecks only the requested journal hash; no World view spans body I/O.
The raw endpoint returns original authenticated storage bytes, and complete
proof response sizing precedes large output/transcript clones. Wire and work
ceilings remain finite; complete decoded-memory reservations are still required.
Core check 86 fails with 78 errors and 110 warnings; all 20,563 captured inputs
stay unchanged. The error multiset matches candidate 83; the additional tail type
inference error in 84 is fixed. The new frozen-limit and authority-rekey controls,
Network admission controls, fifteen dedicated Core proof tests, one sealed-proof
wrapper, two Torii positives and prior nineteen history/index controls remain
uncompiled/unrun. Finish actual State output production and remaining Core/Torii
consumers and fixtures before claiming their qualification. Bound positive index selection/continuation without cloning
historical height sets; integrate decode-graph and resident-memory reservations
before claiming end-to-end query/proof bounds or enabling canonical fanout. Internal invocations require their own output-history/proof owner. Source audit
finds that the production runner leaves the SCCP header root unset; execution-then-
fill is test-only. Replace this outcome-dependent proposal field with one bounded,
rollback-safe applied outbox manifest and QC execution root/count, migrating State,
replay, archives, proofs/circuits, SDKs and formal bindings atomically. No speculative
header rewrite or native SCCP activation is qualified. Three identical OpenAPI source copies
now describe the changed header/details and finalized Network proof contracts,
including byte/work refusals and storage failures. Static document validation and
two mirror/parser checks pass; Torii runtime checks and clean-source release
metadata regeneration remain unqualified.

The isolated model adds an atomic seven-field ExecutionOutputPolicyV1 and a required independent active Time invocation count to BlockParameters. The finite bootstrap profile has 65,536 rows, 1 MiB per row, 64 MiB total row bytes, the protocol 256 MiB executed-wire ceiling, 256 total Pipeline registrations, 4,096 total Time registrations and at most 512 Time invocations; Pipeline zero disables registration. Old one-field BlockParameters and incomplete new JSON are rejected. This is the first-release shape, with no compatibility decoder. Canonical terminal ceilings are measured from real maximal bounded Network/Pipeline/Time rows and cover every allowed name byte length, fixed scalar fields and ambient codec modes. Model tests exercise all codec writers, schema, field requirements, independent setters and phase/origin boundaries. These limits are not complete source or host-memory admission.

State captures the actual on-chain policy, active Time count and total Pipeline population once after its real constructor hooks. Invalid restored policy remains a sticky refusal even if scratch state is later repaired. Genesis may replace the envelope only while active count and stored registries fit; later envelope changes are rejected. Active Time changes remain within the fixed maximum. Register checks total stored Pipeline/Time actions, including disabled or depleted ones, before loading programs. Existing registration-height guards defer newly registered/replaced incarnations from same-carrier execution. Ordinary reservation authenticates the applying header and real proposal commitments, deriving count/root from actual Network inputs. Native reservation follows exact after-start preflight and counts one complete group regardless of routes. The retained non-Clone plan binds source identity and blocks publication until its future producer is complete. Raw Set/restore mutation, source/routes, mandatory metadata, framing/cache, journals/traces, host allocations and complete future feasibility remain open. The conservative ordinary candidate count prevents knowingly overlarge terminal batches before queue selection/signing, but alone grants no whole-source admission.

Candidate 88 model build fails on the new atomic enum variant-size lint; a narrow documented expectation preserves the small fixed Copy envelope. Candidate 89 Core check adds eight missing-trait import errors; the defining SetReadOnly import fixes them. Model 90 builds with no warnings and its exact binary reports 3902 passed, one existing privacy activation identity-capture failure and nine ignored. All 400 block tests and all nineteen new policy/parameter/terminal controls pass. The preexisting failure remains a failed full suite, with its earlier reproduction retained. Core 91 returns to the same 78 errors/110 warnings as 86. New State controls cover independent/frozen counts, reverted/probe initialization, source/duplicate reservation, unfinished publication refusal, sticky invalid capture, actual genesis/later parameter writes and disabled/depleted registration caps. Candidate 95 full test compilation reports 1528 errors/104 warnings; two new missing test imports are corrected. This is not a runtime pass.

The first query fixture capture (92) intentionally stops before writing: ids_projection changes SelectorTuple encoding even in Full mode, so the actual capture differs in fourteen manual and eleven derived rows, beyond the three policy-bearing rows in each expected scope. Audit traces the additional SelectorMode bytes to unchanged feature-dependent code and retained candidate-69 generator evidence. The default-feature generator (96) changes exactly three manual and three derived rows, preserving the other 175 rows. Default fixtures use those actual bytes; new ids fixtures retain the actual 92 captures, and normal tests select their exact feature mode. Existing assertions remain. The signature generator (96) updates five populated rows and preserves two empty rows: header JSON removes the retired result root, while signature bytes change because the actual default confidential-feature policy hash changed; the second header also chains the changed first hash. No cryptographic bytes were guessed. Full manual-frame suites 93 and 99 each pass 21 regular tests with three ignored capture generators; schema 94 passes all nine controls, including complete referenced-type coverage. Capture failures and the agent's earlier 18-pass/3-failure manual run remain recorded separately.

The exact mechanical header migration removes 1039 literal fourth None arguments across 182 Core source/test files. They correspond to 1111 compiler diagnostics because macro expansions duplicate 72; all other argument tokens are preserved. Both actual Some cases were initially excluded. The direct-identity control now builds a real proposal with its input root, attaches a full checked Network output, validates output cache/wire, proves unchanged header/proposal hash/resultless projection and changed executed wire, then compares actual State direct-execution identity. Core 97 test compilation reports 414 errors/104 warnings, removing 1114 normalized diagnostics from 95 and adding none; Core 98 library check reports 74 errors/110 warnings, removing four obsolete production header calls. Core tests remain uncompiled. Six proposal-only stripped-context guards subsequently remove their now-unrepresentable output-root checks while retaining height/parent/view/input-root/timestamp checks; the existing context test adds wrong-height and wrong-parent negatives. The existing crate-private exact-State Kura handle becomes available outside test cfg because the production-compiled LaneProcessOwner already uses it; its Arc preserves the same storage instance, with no runner activation.

Ordinary AMX receipt projection now validates the complete typed output set before any source projection or empty-source return and follows each explicit Network input/output join. The retained certified-merge branch keeps its existing source/QC/wire checks; this is not canonical native-group receipt integration or activation. Its structural fixture now commits the real physical input root and uses the checked full-output setter. Added controls cover positive projection, stale typed-output cache, and a self-consistent changed result that cannot reuse the original executed-wire commitment/manifest. No old result accessor or fabricated Header result root is reintroduced.

After these migrations, Core full unit-test compilation 100 fails with 401 errors/104 warnings in 154.67319962498732 seconds; library check 101 fails with 64 errors/110 warnings in 73.9674907089211 seconds. Each captures 20,570 local inputs unchanged through the command. No Core test executable exists and no new Core control has run. These records are scoped local source/build evidence, not external dependency attestation, main composition, formal, workspace or network qualification. The passing SDK/shared/SCCP 850/301/197 suites belong to the earlier candidate 87 and do not qualify this policy composition.

The persistent privacy inventory failure is a decode refusal before frame comparison: its one stored Proposed activation case still contains retired activate_at_height=400 after proposed_at_height=100. The same defining owner/fixture inputs were present in candidates 63 and 90. Strict one-field lifecycle decoding and both removed-field rejection tests are preserved. A new ignored stdout-only generator calls the unchanged actual activation() fixture and existing capture helper, which decodes, compares and exactly re-encodes root/vector/option/map. Candidate 102 builds that generator; its exact retained binary emits the four replacement frames with unchanged nominal and directional identities. Only one case and its four frame strings change; the other 354 cases and 1416 frames remain byte-identical. The actual fixture digest pin and provenance note are updated together. No bytes or checksums are guessed. Candidate 103 rebuilds the include_str! fixture and its full exact-binary model suite passes 3903 tests, zero failures, ten opt-in generators ignored. All 400 block controls, nineteen policy/terminal controls, the previously failing inventory case and both strict lifecycle refusal controls pass in that same full run. All 20,570 captured local inputs remain unchanged through build and test; this is model qualification, not successful Core/Torii or network qualification. Core100/101 remain their earlier failing captured source; only model test/capture/fixture provenance files change afterward.

Only the four authoritative status/roadmap/goals/history documents are updated in MAIN; its production sources, HEAD, MERGE_HEAD and independently staged merge are preserved. The review's completed-repair temporary fix retains its separate candidate-57 evidence, including actual post-write/fsync restart, same-index retry and tampered/foreign/unowned negatives among 21 publication/repair/capacity controls. New code remains in the isolated checkout. The next producer must move the same plan into a private continuation while leaving an in-progress/poison publication gate, fit complete rows before apply, own completion journals/captures through rollback, and join ordinary sequential/parallel, Pipeline and Time execution without reminting capacity. Capacity exhaustion must not quarantine healthy callbacks or advance user-failure retry policy. The read-only rejection audit additionally identifies deliberate Network penalty/fee fragments after business rollback. The next producer needs a private disposition separate from final row representation: healthy overflow must not silently fall through generic fee eligibility, while an actual prevalidated rejection keeps its established penalty/fee policy even if a long diagnostic becomes bounded OutputLimit. This is a proposed integration decision, not an implemented or qualified policy; capacity for mandatory retained fragments and precise Network rollback wording remain open. SCCP applied-outbox authority, complete reservations, actual State output production, native Apply/runner/old-path retirement and unchanged four-/seven-validator fault/restart/final-transaction acceptance remain open. All L1–L6 and the real silent-initial-author counterexample remain open.


## Retained State output ownership and bridge consumers — checkpoints 104–108

Implementation remains in the isolated checkout. MAIN receives only these four status, roadmap, goal and history documents; its staged merge and production source remain preserved. The completed-repair temporary correction and its actual crash-cut controls retain their separate candidate-57 evidence.

State now transfers its one budget into a borrowing continuation while retaining a publication-blocking owner. It preallocates output slots, preserves canonical source order, binds actual signed/source identities, drains exact-call receipts before sizing, and fits a successful complete Network row before transaction apply. Oversized work drops State, events, captures and witness; pre-apply unwind also restores ZK deduplication. Swallowed local errors, missing phases, duplicate/foreign work and abandoned continuations cannot remint capacity or publish. Real rejected transactions and callback completions explicitly refuse pending the penalty/fee corridor and bounded journal. A completed private row collection is still not a source/witness/wire publication seal.

Fourteen new State controls include actual independent transfer execution with one applied and one rejected leg: exact complete-row capacity applies; one byte below rolls back balances, receipts, events, fragment count, FASTPQ captures and witness. Foreign receipts, competing caller receipts and unowned callback records refuse before apply. These tests are authored and compiler-diagnosed, not executed.

Bridge consumes full typed outputs and explicit Network joins, validates the complete output cache before projection, and preserves its existing finality/replay checks. All 73 prior controls remain represented, plus four new corruption/exact-finality controls. Unsupported internal/chained SCCP outbound records refuse; complete applied-outbox authority, actual execution-order replay and bounded exact-body history remain open. Structural codec mutation fixtures preserve the missing-row/stale-cache/foreign-source attacks without exposing production mutators. Parent block SCCP tests use the same canonical output contract and retain their assertions.

| Capture | Errors / warnings | Scope |
| --- | --- | --- |
| 104 | 54 / 110 | Core library check; 20,572 unchanged inputs |
| 105 | 389 / 104 | Core unit-test compilation; no executable; 20,572 unchanged inputs |
| 106 | 382 / 106 | Core unit-test compilation; no executable; 20,572 unchanged inputs |
| 107 | 54 / 110 | Core library check; 20,572 unchanged inputs |
| 108 | 376 / 105 | Core unit-test compilation; no executable; 20,572 unchanged inputs |

Captures 107/108 have identical source inventories. Earlier 101/100 had 64/401 errors; the final candidate has 54/376. Candidate 105 exposed private-field fixture access and a missing Registrable import; 106 exposed missing DecodeAll imports and two tuple projections. Those new fixture errors are corrected without dropping negative cases. Every failed capture remains immutable. No error in 108 names output_capacity, output_producer, bridge or the migrated SCCP block tests; this is not executable test qualification. Model 103 remains its separate 3903-pass/10-ignored evidence; no Core or network qualification is inferred.

After 108, one unused Decode import is removed from the bridge structural test helper; its Encode and DecodeAll imports remain. The exact before/after bytes are retained under post-check-cleanup. No Core recompile follows that import-only cleanup; static checks cover it. The failing compiler captures continue to refer to their exact original inputs.

Complete ordinary/native production, full source/trace/host admission, rejection-surviving economics, callback custody, State-owned witness closure, SCCP applied outbox, durable Apply, runner integration and old-path retirement remain open. All L1–L6 and the real four-validator silent-initial-author counterexample stay open. Four/seven-validator acceptance must use one unchanged completed candidate.

### Superseded current-state text preserved verbatim

The replacement remains inactive in production, with native State commit explicitly rejected. The isolated checkout now has an agreed genesis output envelope, an independent active Time count and one constructor-frozen terminal reservation; unfinished plans also refuse ordinary publication. Complete source/wire/host admission and actual pre-apply output production remain open. The full model suite passes 3,903 tests, with ten opt-in generators ignored (103), including all 400 block controls and 19 new policy/terminal controls. The sole earlier failure was a privacy fixture retaining a retired field; an actual one-case codec recapture fixes it without relaxing decoding. Both manual-frame configurations pass 21 tests each, with three capture generators ignored (93/99); schema passes all nine (94). After canonical AMX projection and header migration, Core library check 101 still fails with 64 errors/110 warnings, and test compilation 100 fails with 401 errors/104 warnings. Each preserves 20,570 captured inputs; Core controls remain uncompiled/unrun. The [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md) retains failed attempts and exact evidence scopes. No current Core/Torii, main-composition, formal, workspace or network qualification is claimed. State capture/producer integration, SCCP applied-outbox authority, native Apply and runner/old-path retirement remain open. All L1–L6 and the real four-validator silent-initial-author counterexample remain open.

| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner that fits full rows before economic apply, consumes the retained State-frozen terminal reservation, and establishes future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |

The isolated applying policy now has one genesis-installed finite output envelope,
separate active Time invocation count, and total Pipeline/Time registration caps.
Post-genesis envelope replacement is rejected; active Time may change within it.
Constructors capture once after their actual start lifecycle, including an invalid
restored-policy refusal. Actual disabled/depleted registrations occupy capacity;
new/replaced actions remain deferred by their existing incarnation-height guard.
The ordinary candidate count uses the worst permitted future registry/Time growth.
State reserves one terminal plan from the exact proposal/input projection before
execution; a native multi-route group contributes one Network row. An unfinished
plan refuses publication. This is logical terminal count/byte reservation only,
not complete source/routes/metadata/framing/host admission or an output producer.
The nine State controls and candidate/native extensions remain uncompiled/unrun.
TriggerUse retains bounded ID, registration height and action hash, with actual
authority authenticated inside that hash; the actual-Set rekey control is unrun.
Model candidate 90 passes all nineteen new policy/parameter/terminal controls and
all 400 block controls. Full model candidate 103 now passes 3903 tests with ten
opt-in generators ignored. The earlier single failure was a stale privacy fixture
with a retired activation-height field; actual codec recapture changes only its
four frames and preserves strict refusal of the removed layout. Both manual-frame feature
configurations pass 21 regular tests each, with three ignored generators, after
actual feature-matched capture (93/99). Schema 94 passes all nine tests.
Next consume the same non-Clone State plan through a private continuation that
keeps publication gated during execution, refusal and abandonment. Construct and
fit each full row, receipts and callback completions before economic apply, with
rollback covering captures, witness, fees, events and lifecycle state. Keep real
rejection-surviving penalty/fee fragments distinct from healthy-work capacity
failure; a bounded row must not invent misconduct or erase an actual penalty.
The explicit Network disposition and its capacity policy remain to be implemented.
Local resource refusal must not become a canonical rejection. Future-invariant complete-source and allocation admission,
the parallel apply corridor and the 32-MiB proof/256-MiB consensus ceiling remain
open. A positive input count or terminal-only byte calculation is insufficient.
Compose the canonical producer and every Core,
query, proof and SDK consumer atomically. The isolated Rust SDK/shared/SCCP
consumer suite passes 850/301/197 tests again after the descriptor change
(candidate 87, no ignored tests or build warnings). All 20,563 captured inputs stay
unchanged. Earlier standalone shared/SCCP results (78/79) remain separate evidence. Details
have one full Network output owner; real BLS tests bind exact executed wire and
reject changed source/output/cache/context claims. Earlier failed candidates,
including the SCCP fixture's missing proposal input root, remain retained.
The State producer is still a compile-fenced outside draft. Isolated Kura now
indexes only complete validated Network sources/outputs; Kaigi cursors use the
Network input index. Queries and status readers bind exact finalized wire and
charge metadata-authenticated body bytes before reading, then project one row
from an immutable carrier after complete validation. Old merge/bodyless index
promotion and the unbounded FindTransactions iterator owner are removed.
Committed Network proofs now use the same exact finalized-body authority and
explicit input-index/output joins with independently sized trees. State captures
and rechecks only the requested journal hash; no World view spans body I/O.
The raw endpoint returns original authenticated storage bytes, and complete
proof response sizing precedes large output/transcript clones. Wire and work
ceilings remain finite; complete decoded-memory reservations are still required.
Core library check 101 fails with 64 errors and 110 warnings; full unit-test
compilation 100 fails with 401 errors and 104 warnings. Each preserves its 20,570
captured inputs. Mechanical migration removes 1039 unique obsolete None header
arguments; the direct-identity test now attaches a checked full Network output.
Stripped-header guards retain real context fields; output authority cannot live
in the proposal-only Header. Ordinary AMX projection validates the complete typed
output cache and follows explicit Network joins; its certified-merge branch is
preserved and native-group receipt integration remains open. New cache-tamper and
original-commitment controls are authored, not executed. The State capacity,
authority-rekey, admission, history/proof and Torii controls also remain unrun.
Finish actual State output production and remaining consumers/fixtures before
claiming their qualification. Bound positive index selection/continuation without cloning
historical height sets; integrate decode-graph and resident-memory reservations
before claiming end-to-end query/proof bounds or enabling canonical fanout. Internal invocations require their own output-history/proof owner. Source audit
finds that the production runner leaves the SCCP header root unset; execution-then-
fill is test-only. Replace this outcome-dependent proposal field with one bounded,
rollback-safe applied outbox manifest and QC execution root/count, migrating State,
replay, archives, proofs/circuits, SDKs and formal bindings atomically. No speculative
header rewrite or native SCCP activation is qualified. Three identical OpenAPI source copies
now describe the changed header/details and finalized Network proof contracts,
including byte/work refusals and storage failures. Static document validation and
two mirror/parser checks pass; Torii runtime checks and clean-source release
metadata regeneration remain unqualified.

Native accepting/commit paths remain closed until the sole consumer, durable
Apply and atomic old-signer/Ordinary economic bypass retirement are qualified.
Native query/proof/fee observability and fair runner/transport/refresh remain
required. All L1–L6 and the real four-validator counterexample remain OPEN.
Four-/seven-validator acceptance must use one unchanged completed candidate.


## Transaction-owned callback capture and typed consumers — checkpoints 110–113

Implementation remains isolated. MAIN receives only these four authoritative documents; the staged merge and production source are preserved. The completed-repair temporary fix and candidate-57 post-write/flush/fsync/pre-rename crash-cut evidence remain separate; current MAIN composition is unqualified.

The actual execute_trigger wrapper now allocates a transaction-owned ordinal before depth/body checks, captures nested successful steps in entry order, and preserves typed early failures. This closes a trace gap where ExecuteTrigger discarded nested returned steps after effects had been staged. The DFS wrapper separately latches errors from matching/depth/gas checks before another dispatch. A successful empty NoOp is a retained invocation. No posthoc result-derived Time identity or fragment-count by-call fallback supplies ownership. Failed journal capture cannot be swallowed into success; its incomplete failure/economic corridor remains explicitly gated.

The private producer drains exact-call receipts and callback trace/completions before exact complete-row fit. Canonical unframed child bytes form a lower bound under the frozen row ceiling, excluding standalone headers so exact boundaries are not falsely rejected. Overflow drops the whole business overlay and retains its already-reserved terminal. Local allocation/ownership failure aborts the carrier. Real callback errors remain typed, are never relabelled OutputLimit, and refuse this success-only kernel. Completion events are emitted once after fit. Undrained journals prevent ordinary or consensus-effects apply and poison the parent. Full rejection/penalty/fee/accountable-work execution, host/source admission, internal Pipeline/Time producers and the common publication seal remain unfinished.

Eight journal unit tests pass in an exact-source standalone rustc harness linked to retained 108 model/crypto/Norito artifacts: nested pre-body order, move-once, overflow, real failure precedence, missing/foreign/changed identities, unfinished/missing capacity, successful NoOp, exact complete-row lower-bound boundaries and post-body scan failure. The initial six-test harness is also retained; it is not pooled into the final eight. Neither harness executes Core State or qualifies production. Five new State tests use actual registered parent/child actions and signed sources, real metadata effects/repeat debits, exact fit and one-byte-below rollback, early depth failure, real DFS cascade failure before dispatch, and undrained ordinary/consensus-effects apply. They remain unrun alongside the earlier fourteen producer controls.

The frozen telemetry/v2_apply two-file draft is now integrated. Classified counters validate complete typed output/cache structure and explicit Network source joins; sealed commitment and reveal sources each count once, and internal callbacks are excluded. Existing atomic progress/counter and finality behavior remains. Exact QC-bound body read/decode/work admission is still missing in the telemetry reader and is not inferred from its existing header/hash checks. Apply diagnostic text counts rejected Network/Pipeline/Time rows. Five new consumer tests are authored, not executed.

| Capture | Errors / warnings | Scope |
| --- | --- | --- |
| 110 | 48 / 110 | Core library check; 20,574 unchanged inputs |
| 111 | 372 / 104 | Core unit-test compilation; no executable; 20,574 unchanged inputs |
| 112 | 48 / 110 | Core library check; 20,574 unchanged inputs |
| 113 | 370 / 104 | Core unit-test compilation; no executable; 20,574 unchanged inputs |

Capture 111 exposes two new telemetry test errors (borrowed Log message and a setter-returning match arm). Both are corrected in the final candidate; no assertion is dropped. The direct-undrained State control also covers consensus-effects apply. Final 112/113 preserve identical inventories. Prior 107/108 had 54/376 errors; current counts are 48/370, without a passing Core executable. Static formatting, whitespace, codec and history checks have separate receipts. Model 103 remains separate 3903-pass/10-ignored evidence. No current formal/workspace/SDK/network qualification follows. Every L1–L6 milestone and the real four-validator silent-initial-author counterexample remains open. Acceptance still requires one unchanged completed four/seven-validator candidate.

### Superseded current-state text preserved verbatim

The replacement remains inactive in production, with native State commit explicitly rejected. The isolated State output plan now retains ownership during execution, refusal and abandonment. A private successful-Network kernel joins actual receipts, fits the full row before apply, and rolls back oversized work; all plan states still refuse publication pending the actual rejection/penalty/fee corridor, callback journal and sealing tail. Fourteen new State controls are authored, including real independent-batch receipt boundaries; none has executed. Core bridge consumers now validate complete typed outputs and explicit Network joins, with unsupported internal outbound records refused. Final Core library check 107 fails with 54 errors/110 warnings; full test compilation 108 fails with 376 errors/105 warnings. Both preserve the same 20,572 captured inputs. The earlier model 103 suite remains 3,903 passed and ten opt-in generators ignored; manual-frame suites 93/99 each pass 21 tests and schema 94 passes nine. The [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md) retains every failed attempt and exact evidence scope. No current Core executable, main-composition, formal, workspace or network qualification follows. Complete source/host admission, SCCP applied-outbox authority, native Apply and runner/old-path retirement remain open. All L1–L6 and the real four-validator silent-initial-author counterexample remain open.

| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner: compose the private pre-apply Network kernel with actual rejection/penalty/fee/gas execution, a bounded callback journal and the common sealing tail, consume the retained State-frozen reservation, and establish future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |

The isolated applying policy now has one genesis-installed finite output envelope,
separate active Time invocation count, and total Pipeline/Time registration caps.
Post-genesis envelope replacement is rejected; active Time may change within it.
Constructors capture once after their actual start lifecycle, including an invalid
restored-policy refusal. Actual disabled/depleted registrations occupy capacity;
new/replaced actions remain deferred by their existing incarnation-height guard.
The ordinary candidate count uses the worst permitted future registry/Time growth.
State reserves one terminal plan from the exact proposal/input projection before
execution; a native multi-route group contributes one Network row. An unfinished
plan refuses publication. Its private continuation now transfers the same budget
while retaining Reserved/Running/Retained/Poisoned ownership on State. Completed
rows still cannot authorize publication. This is not complete source, framing,
trace, capture or resident-memory admission.
The nine State controls and candidate/native extensions remain uncompiled/unrun.
TriggerUse retains bounded ID, registration height and action hash, with actual
authority authenticated inside that hash; the actual-Set rekey control is unrun.
Model candidate 90 passes all nineteen new policy/parameter/terminal controls and
all 400 block controls. Full model candidate 103 now passes 3903 tests with ten
opt-in generators ignored. The earlier single failure was a stale privacy fixture
with a retired activation-height field; actual codec recapture changes only its
four frames and preserves strict refusal of the removed layout. Both manual-frame feature
configurations pass 21 regular tests each, with three ignored generators, after
actual feature-matched capture (93/99). Schema 94 passes all nine tests.
The private successful-Network kernel preallocates row storage, preserves input
positions under reordered execution, joins exact-call transaction-owned receipts
and fits the full row before State apply. Oversized healthy work rolls back;
local refusal or pre-apply unwind poisons the carrier, restores ZK deduplication
and drops its witness overlay. Claimed or actual callback completions and real
Network rejection refuse until their proper owners are integrated. Fourteen new
State tests cover exact byte limits, actual independent-batch receipts and
balance/event/fragment/FASTPQ/witness rollback, source substitution, duplicate
spending, unfinished phases, swallowed errors and interruption. These tests
remain unexecuted. Next connect actual execution, accountable work and the bounded
callback journal; fit every complete row and side channel before economic apply. Keep real
rejection-surviving penalty/fee fragments distinct from healthy-work capacity
failure; a bounded row must not invent misconduct or erase an actual penalty.
The explicit Network disposition and its capacity policy remain to be implemented.
Local resource refusal must not become a canonical rejection. Future-invariant complete-source and allocation admission,
the parallel apply corridor and the 32-MiB proof/256-MiB consensus ceiling remain
open. A positive input count or terminal-only byte calculation is insufficient.
Compose the canonical producer and every Core,
query, proof and SDK consumer atomically. The isolated Rust SDK/shared/SCCP
consumer suite passes 850/301/197 tests again after the descriptor change
(candidate 87, no ignored tests or build warnings). All 20,563 captured inputs stay
unchanged. Earlier standalone shared/SCCP results (78/79) remain separate evidence. Details
have one full Network output owner; real BLS tests bind exact executed wire and
reject changed source/output/cache/context claims. Earlier failed candidates,
including the SCCP fixture's missing proposal input root, remain retained.
The earlier complete-callback producer remains an unapplied compile-fenced draft;
the private success kernel alone does not replace it. Isolated Kura now
indexes only complete validated Network sources/outputs; Kaigi cursors use the
Network input index. Queries and status readers bind exact finalized wire and
charge metadata-authenticated body bytes before reading, then project one row
from an immutable carrier after complete validation. Old merge/bodyless index
promotion and the unbounded FindTransactions iterator owner are removed.
Committed Network proofs now use the same exact finalized-body authority and
explicit input-index/output joins with independently sized trees. State captures
and rechecks only the requested journal hash; no World view spans body I/O.
The raw endpoint returns original authenticated storage bytes, and complete
proof response sizing precedes large output/transcript clones. Wire and work
ceilings remain finite; complete decoded-memory reservations are still required.
Final Core library check 107 fails with 54 errors and 110 warnings; full
unit-test compilation 108 fails with 376 errors and 105 warnings. Both preserve
the same 20,572 captured inputs. No error diagnostic targets the new output-owner modules,
bridge or migrated SCCP block fixtures, but there is no Core test executable.
Bridge now validates full output/cache structure and joins Network sources by
input index. Its 73 migrated controls and four new corruption/finality controls
are unrun. Successful internal/chained outbound records explicitly refuse until
the actual SCCP applied-outbox owner exists; replay ordering/custody remains open.
Failed intermediate candidates 104–106 remain retained. Mechanical migration removes 1039 unique obsolete None header
arguments; the direct-identity test now attaches a checked full Network output.
Stripped-header guards retain real context fields; output authority cannot live
in the proposal-only Header. Ordinary AMX projection validates the complete typed
output cache and follows explicit Network joins; its certified-merge branch is
preserved and native-group receipt integration remains open. New cache-tamper and
original-commitment controls are authored, not executed. The State capacity,
authority-rekey, admission, history/proof and Torii controls also remain unrun.
Finish actual State output production and remaining consumers/fixtures before
claiming their qualification. Bound positive index selection/continuation without cloning
historical height sets; integrate decode-graph and resident-memory reservations
before claiming end-to-end query/proof bounds or enabling canonical fanout. Internal invocations require their own output-history/proof owner. Source audit
finds that the production runner leaves the SCCP header root unset; execution-then-
fill is test-only. Replace this outcome-dependent proposal field with one bounded,
rollback-safe applied outbox manifest and QC execution root/count, migrating State,
replay, archives, proofs/circuits, SDKs and formal bindings atomically. No speculative
header rewrite or native SCCP activation is qualified. Three identical OpenAPI source copies
now describe the changed header/details and finalized Network proof contracts,
including byte/work refusals and storage failures. Static document validation and
two mirror/parser checks pass; Torii runtime checks and clean-source release
metadata regeneration remain unqualified.

Native accepting/commit paths remain closed until the sole consumer, durable
Apply and atomic old-signer/Ordinary economic bypass retirement are qualified.
Native query/proof/fee observability and fair runner/transport/refresh remain
required. All L1–L6 and the real four-validator counterexample remain OPEN.
Four-/seven-validator acceptance must use one unchanged completed candidate.


## Time success ownership, completed work and complete replay parity — checkpoints 115–120

Implementation remains isolated. MAIN receives only these four authoritative documents; its staged merge and production source remain preserved. The completed-repair temporary fix and candidate-57 real post-write/flush/fsync/pre-rename crash-cut evidence remain separate. Current MAIN composition is unqualified.

The same private State-borrowing output owner admits Time once after prior phases, performs maintenance only with the captured applying capacity and exact Running owner, uses the real event and frozen bounded matcher, and revalidates actions before each invocation. Source/index/action descriptors seed the actual callback call before body execution; Time has no signed Network identity. Root and nested capture plus exact-call receipts are measured as one complete row before effects apply. Same-generation repeat debits and retry clearing live in that transaction. Healthy overflow drops the complete business/witness overlay, preserves repeat/retry state, accounts actual completed work and emits only its bounded root failure. Typed real errors remain distinct and poison this unfinished success-only kernel. No failure/retry policy, normal/penalty fee corridor or complete publication seal is claimed. Existing direct legacy Time tests/common tail remain unmigrated and refuse without the active owner.

CompletedOutputWork retains actual gas, confidential operation/verification/proof-byte/gas counters across healthy overflow for both Network and Time; it does not double-charge StateTransaction apply. A focused control uses the actual budget APIs and verifies next-overlay block limits after rollback; it is not proof-verifier qualification. New Time controls use actual registered actions, matcher/event provenance and signed Network sources to cover nested trace/repeats/events, exact fit and one-byte-below rollback, frozen T under Network mutation, use-time removal, typed real failure and phase single ownership. Additional persisted-retry/repeated-schedule/replacement controls are source-reviewed and authored, not executed.

Replay consumers now compare complete validated output rows rather than zipping Network sources against synthetic transaction results. Exact headers/signatures, full Network sequence, typed output root/count/rows, fragments, FASTPQ, AXT and lane-finality metadata are all compared. Signed replay logging follows exact Network joins, distinguishes outer sealed source hash from inner execution call, and leaves internal failure diagnostics separate. The caller retains its independent exact stored-wire/QC/manifest, recomputed ExecutionCommitment and checkpoint gates. All thirteen pre-existing replay-validation entrypoints and old helper assertions are preserved or migrated; strict replay actual execution fixtures are not fabricated.

The six exact-source replay parity tests pass in a standalone rustc harness linked to retained 108 model/crypto/Norito/logger/test dependencies (artifact hashes captured). They cover unequal source/output counts, repeated Time occurrence identity, stale/malformed caches on either side, self-consistent altered rows/receipts/completions/action/Time descriptors, metadata outside the root, rejected sealed joins, and proposal header/signature mismatch. The fixture owns a deterministic network ID and does not require State configuration. This tests structural comparison only, not State replay, finality read authority, host resource admission or production. The earlier eight exact-source journal and model103 tests remain separate evidence and were not rerun.

| Capture | Errors / warnings | Scope |
| --- | --- | --- |
| 115 | 32 / 110 | Core library check; 20,578 unchanged inputs |
| 116 | 342 / 104 | Core unit-test compilation; no executable; 20,578 unchanged inputs |
| 117 | 29 / 110 | Core library check; 20,578 unchanged inputs |
| 118 | 335 / 104 | Core unit-test compilation; no executable; 20,578 unchanged inputs |
| 119 | 29 / 110 | Core library check; 20,578 unchanged inputs |
| 120 | 333 / 104 | Core unit-test compilation; no executable; 20,578 unchanged inputs |

115/116 retain the missing LoadedActionTrait import and three ambiguous Metadata lookups (two diagnostics each); final candidate corrects them without dropping assertions. Capture118 additionally exposes two generation lookups on SetBlock. The replacement fixture now compares the persistent registration-height/action-hash incarnation; the overlay-local generation resets per transaction, so separate transaction probes would be invalid. It retains fresh repeat/policy/retry and current-height deferral assertions. Final119/120 input inventories are identical. Formatting, whitespace, codec and history verification have separate receipts. No full Core, current formal/workspace/SDK or real-network qualification follows. Complete failure/economic execution, frozen Pipeline provenance, source/host/witness/wire admission, SCCP outbox, native Apply and runner retirement remain open. L1–L6 and the real four-validator silent-initial-author counterexample remain open; four/seven-validator acceptance requires one unchanged completed candidate.

### Superseded current-state text preserved verbatim

The replacement remains inactive in production, with native State commit explicitly rejected. The private State output owner now captures actual nested callback trace/completion ordinals before dispatch and fits them with exact-call receipts before successful Network apply. Undrained, failed, foreign or abandoned journals prevent application/publication; real errors cannot become capacity rejections. Eight exact-source isolated journal tests pass; five new actual-State callback controls and the earlier fourteen producer controls remain unexecuted. Typed telemetry and Apply diagnostics are integrated, with exact finalized-body admission still open for telemetry. Final Core library check 112 fails with 48 errors/110 warnings; full test compilation 113 fails with 370 errors/104 warnings. Both preserve identical inventories of 20,574 inputs. Model 103 remains separate evidence: 3,903 passed, ten opt-in generators ignored. The [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md) preserves failed attempts and exact scopes. No Core executable, main-composition, formal, workspace or network qualification follows. Full rejection/penalty/fee/accountable-work execution, Pipeline/Time output production, source/host admission, SCCP applied-outbox authority, common seal, native Apply and runner/old-path retirement remain open. All L1–L6 and the real four-validator silent-initial-author counterexample remain open.

| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner: compose the private pre-apply Network kernel with actual rejection/penalty/fee/gas execution, the integrated successful-callback journal, full Pipeline/Time invocation ownership and the common sealing tail, consume the retained State-frozen reservation, and establish future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |

The isolated applying policy now has one genesis-installed finite output envelope,
separate active Time invocation count, and total Pipeline/Time registration caps.
Post-genesis envelope replacement is rejected; active Time may change within it.
Constructors capture once after their actual start lifecycle, including an invalid
restored-policy refusal. Actual disabled/depleted registrations occupy capacity;
new/replaced actions remain deferred by their existing incarnation-height guard.
The ordinary candidate count uses the worst permitted future registry/Time growth.
State reserves one terminal plan from the exact proposal/input projection before
execution; a native multi-route group contributes one Network row. An unfinished
plan refuses publication. Its private continuation now transfers the same budget
while retaining Reserved/Running/Retained/Poisoned ownership on State. Completed
rows still cannot authorize publication. This is not complete source, framing,
trace, capture or resident-memory admission.
The nine State controls and candidate/native extensions remain uncompiled/unrun.
TriggerUse retains bounded ID, registration height and action hash, with actual
authority authenticated inside that hash; the actual-Set rekey control is unrun.
Model candidate 90 passes all nineteen new policy/parameter/terminal controls and
all 400 block controls. Full model candidate 103 now passes 3903 tests with ten
opt-in generators ignored. The earlier single failure was a stale privacy fixture
with a retired activation-height field; actual codec recapture changes only its
four frames and preserves strict refusal of the removed layout. Both manual-frame feature
configurations pass 21 regular tests each, with three ignored generators, after
actual feature-matched capture (93/99). Schema 94 passes all nine tests.
The private successful-Network kernel preallocates row storage, preserves input
positions under reordered execution, joins exact-call transaction-owned receipts
and fits the full row before State apply. Oversized healthy work rolls back;
local refusal or pre-apply unwind poisons the carrier, restores ZK deduplication
and drops its witness overlay. Actual callback capture now lives on each State
transaction: a non-copyable pre-body ordinal binds the original execution call,
including nested by-call work whose old returned trace was discarded. The actual
wrapper captures early errors and successful empty NoOp steps; a DFS failure
before another dispatch also latches refusal. Successful trace/completions move
once into the complete Network row before fitting and apply. Completion events
are emitted once only after fit, under the same call and actual ordinal. Missing,
foreign, failed or undrained journals block both ordinary and consensus-effects
application and poison parent publication. Child-payload lower bounds derive
from the frozen row ceiling; exact complete-row sizing remains authoritative.
Host allocation refusal is distinct from actual output overflow. Failed callback
errors remain typed and cannot be converted into a healthy OutputLimit rollback.
Eight exact-source journal tests pass in an isolated harness against retained
108 model/codec/crypto artifacts. This is not State execution qualification.
Five new real-State controls use registered actions and signed ExecuteTrigger
sources, including nested trace/repeats/events, exact fit/one-byte-below rollback,
early-depth and actual DFS pre-dispatch failures, and undrained application via
both apply methods. They and the earlier fourteen producer controls remain
unexecuted because Core has no test executable. The private kernel still does
not run the full public Network validation, economics or accountable-work path.
Next compose that corridor and actual Pipeline/Time producers with the common
source/witness/wire seal; fit every complete row and side channel before apply. Keep real
rejection-surviving penalty/fee fragments distinct from healthy-work capacity
failure; a bounded row must not invent misconduct or erase an actual penalty.
The explicit Network disposition and its capacity policy remain to be implemented.
Local resource refusal must not become a canonical rejection. Future-invariant complete-source and allocation admission,
the parallel apply corridor and the 32-MiB proof/256-MiB consensus ceiling remain
open. A positive input count or terminal-only byte calculation is insufficient.
Compose the canonical producer and every Core,
query, proof and SDK consumer atomically. The isolated Rust SDK/shared/SCCP
consumer suite passes 850/301/197 tests again after the descriptor change
(candidate 87, no ignored tests or build warnings). All 20,563 captured inputs stay
unchanged. Earlier standalone shared/SCCP results (78/79) remain separate evidence. Details
have one full Network output owner; real BLS tests bind exact executed wire and
reject changed source/output/cache/context claims. Earlier failed candidates,
including the SCCP fixture's missing proposal input root, remain retained.
The earlier complete-callback producer remains an unapplied compile-fenced draft;
the private success kernel alone does not replace it. Isolated Kura now
indexes only complete validated Network sources/outputs; Kaigi cursors use the
Network input index. Queries and status readers bind exact finalized wire and
charge metadata-authenticated body bytes before reading, then project one row
from an immutable carrier after complete validation. Old merge/bodyless index
promotion and the unbounded FindTransactions iterator owner are removed.
Committed Network proofs now use the same exact finalized-body authority and
explicit input-index/output joins with independently sized trees. State captures
and rechecks only the requested journal hash; no World view spans body I/O.
The raw endpoint returns original authenticated storage bytes, and complete
proof response sizing precedes large output/transcript clones. Wire and work
ceilings remain finite; complete decoded-memory reservations are still required.
Final Core library check 112 fails with 48 errors and 110 warnings; full
unit-test compilation 113 fails with 370 errors and 104 warnings. Both preserve
the same 20,574 captured inputs. No error diagnostic targets the new journal,
producer, callback, telemetry or Apply-diagnostic implementation/tests, but there
is no Core test executable. Capture 111 exposed two telemetry fixture mistakes;
113 corrects both without dropping assertions. All failed captures are retained.
Typed telemetry validates complete output/cache shape and counts only explicit
Network joins; it does not yet authenticate exact finalized body bytes or admit
complete body/decode/work cost. Apply diagnostics count rejected typed outputs
without changing validation or finality. Their five new tests remain unrun.
Bridge now validates full output/cache structure and joins Network sources by
input index. Its 73 migrated controls and four new corruption/finality controls
are unrun. Successful internal/chained outbound records explicitly refuse until
the actual SCCP applied-outbox owner exists; replay ordering/custody remains open.
Failed intermediate candidates 104–106 remain retained. Mechanical migration removes 1039 unique obsolete None header
arguments; the direct-identity test now attaches a checked full Network output.
Stripped-header guards retain real context fields; output authority cannot live
in the proposal-only Header. Ordinary AMX projection validates the complete typed
output cache and follows explicit Network joins; its certified-merge branch is
preserved and native-group receipt integration remains open. New cache-tamper and
original-commitment controls are authored, not executed. The State capacity,
authority-rekey, admission, history/proof and Torii controls also remain unrun.
Finish actual State output production and remaining consumers/fixtures before
claiming their qualification. Bound positive index selection/continuation without cloning
historical height sets; integrate decode-graph and resident-memory reservations
before claiming end-to-end query/proof bounds or enabling canonical fanout. Internal invocations require their own output-history/proof owner. Source audit
finds that the production runner leaves the SCCP header root unset; execution-then-
fill is test-only. Replace this outcome-dependent proposal field with one bounded,
rollback-safe applied outbox manifest and QC execution root/count, migrating State,
replay, archives, proofs/circuits, SDKs and formal bindings atomically. No speculative
header rewrite or native SCCP activation is qualified. Three identical OpenAPI source copies
now describe the changed header/details and finalized Network proof contracts,
including byte/work refusals and storage failures. Static document validation and
two mirror/parser checks pass; Torii runtime checks and clean-source release
metadata regeneration remain unqualified.

Native accepting/commit paths remain closed until the sole consumer, durable
Apply and atomic old-signer/Ordinary economic bypass retirement are qualified.
Native query/proof/fee observability and fair runner/transport/refresh remain
required. All L1–L6 and the real four-validator counterexample remain OPEN.
Four-/seven-validator acceptance must use one unchanged completed candidate.


## Actual Network output ownership and typed consumers — checkpoints 122–130

Implementation remains isolated. MAIN receives only these four authoritative documents; its staged merge and all production source remain preserved. Candidate57 completed-repair temporary recovery and post-write/flush/fsync/pre-rename crash-cut tests remain separate scoped evidence. MAIN composition is unqualified.

The private linear producer now executes actual Network sources after freezing original positions, explicit-time stateless admission, original routes and pre-execution reveal ordering. A real transaction overlay owns source/call identity, callback journal, exact-call receipts and witness/ZK rollback through the complete row fit. Healthy oversized success rolls back business effects and retains completed work. A true rejection consumes/discards callback capture even after healthy overflow, rolls back its business attempt, applies a prevalidated governance penalty independently, then settles eligible fees. Fee failure does not undo the earlier penalty; penalty failure remains Internal and forbids fee/gas handling. Confidential work remains accountable; final block-gas rejection preserves the old special no-fee/no-transaction-gas disposition. The old closure-only Network kernel is now test-only. No canonical driver or publication activation is claimed.

A rejected Network row with excessive diagnostics reuses its preallocated terminal string for `execution rejected; diagnostic omitted`. The original typed error determines economics before this projection; this fixed LimitCheck diagnostic is distinct from healthy OutputLimit and retains no business receipts/completions. Five new model controls cover both codec roundtrips, exact fit/one-byte-below, aggregate reservation, foreign origin, success/internal misuse and rejected receipt/completion retention. Five new journal controls cover actual failure, earlier overflow, successful capture followed by external rejection, poisoned pending/foreign/refused/absent-policy owners and double consumption. The fresh 13-test source harness covers only the journal, not State integration.

Real signed-source State controls exercise output-position retention, actual callback fit/rollback, actual business error after healthy callback overflow, pre-body block gas admission, future stateless rejection, actual Nexus rejected-batch fee settlement and merge-source refusal. The separate real two-ballot fixture exercises accepted lock creation followed by an actual direction-conflict rejection and 20% slash, retaining actual custody, slash ledger and events, then dropping the whole overlay afterward. These tests are authored but not run. Fee failure after penalty and complete source/reveal/QueuePlan authority qualification remain open. The read-only audit found a merge-source omission; both ordinary continuation and Network freezing now refuse that source. Full committed-context validation and control admission remain explicit prerequisites; the private contextless execution-fixture fallback grants no consensus authority.

Genesis validation now checks full typed output structure/cache and exact Network coverage. Event projection validates once and joins Network output input indices to exact source routes; internal outputs do not fabricate transaction identity, and sealed reveals use their inner signed event identity. All seven old consumer tests remain and four new controls are added. No runtime consumer pass is claimed.

Capture 124: return 101, 19 errors, 113 warnings, 100.641 seconds, unchanged local inputs.

Capture 125: return 101, 320 errors, 105 warnings, 273.599 seconds, unchanged local inputs.

Capture 130: return 101, 313 errors, 104 warnings, 188.427 seconds, unchanged local inputs.

Capture 126: return 0, model executable errors, see log warnings, 178.242 seconds, unchanged local inputs.

Capture 127: return 101, 18 errors, 110 warnings, 90.939 seconds, unchanged local inputs.

Full model126: 3908 passed, 0 failed, 10 ignored opt-in generators; exact retained binary `b7f3b0bf6cb11dcec4341ba7f2a93f5244031e4570c9c221f7b6cc3ca30ab75c`. Journal129: 13 passed, zero ignored; exact retained binary `ff0da2b8df4fb9d689fcbb95f46c51cedb17da4d9c703079efcd3817f6d18df9`. No Core executable was emitted. Earlier replay117, SDK87, model103, formal and repair57 observations retain separate scopes; no full gate or release result follows.

All L1–L6 and the real four-validator silent-initial-author counterexample remain open. Complete canonical producer/genesis, Pipeline and internal failure handling, source/host admission, applied SCCP outbox, common metadata/wire seal, native activation/old-path retirement and unchanged-candidate four/seven-validator qualification are still required.

### Superseded current statements preserved exactly

`status.md`

```text
The replacement remains inactive in production, with native and unfinished ordinary State publication explicitly rejected. The same private output owner now runs actual bounded Time matching, binds actions at use, captures root/nested callbacks and fits complete rows before applying repeat/retry or business changes. Healthy overflow rolls back those changes while retaining completed gas/confidential work. Real errors still refuse the incomplete success kernel. Replay parity now compares complete typed rows, sources and side metadata while preserving independent finality/commitment/checkpoint gates; six exact-source structural parity tests pass in isolation. New Time and work-accounting State controls remain unexecuted. Final Core library check 119 fails with 29 errors/110 warnings; full test compilation 120 fails with 333 errors/104 warnings, on identical inventories of 20,578 unchanged inputs. Eight earlier isolated journal tests and model 103's 3,903 passes/ten ignored generators remain separate evidence. The [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md) records exact scopes and failed attempts. No Core executable, main-composition, formal, workspace or network qualification follows. Full Network rejection/penalty/fee execution, internal failure/retry policy, frozen Pipeline routing, complete source/host admission, SCCP applied-outbox authority, common seal and native runtime/old-path retirement remain open. All L1–L6 and the real four-validator silent-initial-author counterexample remain open.
```

`roadmap.md`

```text
| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner: compose the private pre-apply Network kernel with actual rejection/penalty/fee/gas execution, the integrated successful-callback journal, actual Time success ownership plus full internal failure/retry policy and frozen Pipeline routing and the common sealing tail, consume the retained State-frozen reservation, and establish future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |
```

`specs/sumeragi_liveness_redesign_goals.md`

```text
Full model candidate 103 now passes 3903 tests with ten
opt-in generators ignored.
```

`specs/sumeragi_liveness_redesign_goals.md`

```text
Eight exact-source journal tests pass in an isolated harness against retained
108 model/codec/crypto artifacts. This is not State execution qualification.
```

`specs/sumeragi_liveness_redesign_goals.md`

```text
Next move actual Network execution and its economic disposition under this
owner, freezing original source index/call and all routes before any Network
work. The current tx return is already applied; wrapping it would preserve the
ownership defect. Preserve prevalidated penalty-before-fee behavior, penalty
survival on fee failure, reveal execution/source-order distinction and actual
fragment accounting. Then Pipeline consumes that completed prefix and frozen
routes before Time. Compose internal failure owners and the common source/
witness/wire seal, fitting rows before apply. Keep real
rejection-surviving penalty/fee fragments distinct from healthy-work capacity
failure; a bounded row must not invent misconduct or erase an actual penalty.
The explicit Network disposition and its capacity policy remain to be implemented.

```

`specs/sumeragi_liveness_redesign_goals.md`

```text
Final Core library check 119 fails with 29 errors and 110 warnings; full
unit-test compilation 120 fails with 333 errors and 104 warnings. Both preserve
the same 20,578 captured inputs. There is still no Core test executable.
Captures 115/116 exposed a missing Time action trait import and three ambiguous
metadata lookups in new tests; those are corrected without losing assertions.
Capture 118 also exposes two invalid block-view generation queries in a new
fixture. The correction compares the persistent action incarnation (registration
height and full action hash); transaction-local generation cannot be compared
across separate overlays. Fresh-repeat/policy and registration-height refusal
assertions remain intact. All failed captures are retained. New State controls remain unexecuted, and the
isolated parity result is not current full Core or production qualification.

```

## Actual Pipeline and internal rejection ownership — checkpoints 132–140

Implementation remains isolated. MAIN receives only these four authoritative documents; its staged merge and production sources remain preserved. Candidate57 completed-repair temporary authentication and the post-write/flush/fsync/pre-rename strict-restart control remain separate evidence. MAIN composition is unqualified.

The private producer derives Pipeline events from actual retained Network dispositions and frozen routes, preserving original source/candidate indices, then runs BlockApproved. Non-signed sources, stale/replaced/disabled/depleted matches and exhausted gas release their unused reservation. Internal invocation calls bind the actual persistent action before body execution. Pipeline and Time share the complete row-fit and State/witness/ZK rollback owner. Pipeline repeat debit precedes DFS; Time repeat debit and retry clearing follow full success. Healthy output overflow rolls back those business changes without quarantine/retry; actual rejection discards failed callback capture, accounts work and independently quarantines the same authenticated Pipeline action or preserves existing Time retry/removal behavior. Earlier successful siblings survive.

The new OmittedAfterRejection diagnostic is structurally distinct from OmittedByOutputLimit. Both full and bounded real rejections retain no business receipts and only callback-zero root Failure. The model reservation reuses its two preowned reason strings when the full row does not fit. Core uses borrowed canonical child-payload lower bounds before copying root projections and a bounded UTF-8 formatter; host/codec/journal ownership refusal remains fatal, rather than a canonical trigger failure. Root failure and later DFS failure retain distinct declared-versus-returned diagnostic provenance.

Nine added model controls all pass. Nine new Core controls and one migrated real-Time-failure assertion cover event/call order, stale match gaps, repeated phase refusal, gas skipping, exact/one-byte row fitting, quarantine with sibling retention, actual root-success/DFS failure, oversized actual rejection and actual retry advancement/exhaustion. They are authored but unrun; no Core executable exists. The gas-stop control sets the existing stop predicate after real Network work and is not a gas-meter qualification. Existing seeded retry-success/overflow and callback controls remain intact. One extracted exact-source bounded formatter control passes; it tests no State transaction.

Capture 135: return 0, model executable errors, see log warnings, 162.621 seconds, unchanged local inputs.

Capture 136: return 101, 18 errors, 110 warnings, 105.603 seconds, unchanged local inputs.

Capture 137: return 101, 314 errors, 104 warnings, 236.603 seconds, unchanged local inputs.

Capture 139: return 101, 313 errors, 104 warnings, 153.692 seconds, unchanged local inputs.

Model135: 3917 passed, 0 failed, 10 ignored generators; binary `374ceafbe0fcb353f706c46c8bd54297221037ab3fc5b2c6bc296db7a0f789cc`. Formatter138: one passed, zero ignored; binary `52972c60f56037c50ae70428dff3d404ef11063c4794ea53e9ddb243c3095c50`. Initial137 includes one stale test-only sizing accessor; final139 changes only that assertion to canonical Norito full-frame sizing. Library136 production inputs and all model inputs remain unchanged; their results are not rerun or relabeled as new State evidence. Static140 formatting/whitespace/codec checks pass.

The next read-only map identifies one narrow ordinary phase driver and consuming common seal. It must retain actual source bindings and every zero/nonzero-transcript call, reconcile applied captures without inventing execution authority, preserve fee/penalty captures for rejected Network rows, finalize AXT and lane-finality metadata before one checked output attachment, and leave a nonpublishable marker through any failed take/seal. Complete source/host/wire admission, genesis/native/merge ownership, witness-before-start-hooks, complete deterministic projection, SCCP applied-outbox redesign, old-path retirement and unchanged four/seven-validator qualification remain open. All L1–L6 and the silent-initial-author counterexample remain open.

### Exact superseded current prose

From `status.md`:

```text
The replacement remains inactive in production, with native and unfinished ordinary State publication explicitly rejected. Its private output owner now freezes actual Network routes/admission instants/reveal order and executes the real transaction path, fitting complete successful rows before apply. Actual rejection rolls back business capture, preserves penalty-before-fee ordering and uses an independent bounded diagnostic after economic eligibility is decided. Actual Time success and complete replay comparison retain their previous scope. Genesis/event consumers now use typed output ownership. Model 126 passes 3,908 tests with 10 opt-in generators ignored; the fresh exact-source callback journal harness passes 13 controls. Core library check 127 still fails with 18 errors/110 warnings; full test compilation 130 fails with 313 errors/104 warnings, on identical inventories of 20,582 unchanged inputs. New signed execution, callback, gas, Nexus-fee and real double-vote penalty controls remain unexecuted. The [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md) records scopes and failed captures. No Core executable, main-composition, formal, workspace or network qualification follows. Sole canonical producer/genesis integration, internal failure/retry policy, frozen Pipeline execution, complete source/host admission, SCCP applied-outbox authority, common seal and native runtime/old-path retirement remain open. All L1–L6 and the real four-validator silent-initial-author counterexample remain open.
```

From `roadmap.md`:

```text
| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner: integrate the private actual Network rejection/penalty/fee owner and callback journal into the sole canonical driver, finish frozen Pipeline execution and internal Time failure/retry ownership, and compose the common sealing tail, consume the retained State-frozen reservation, and establish future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |
```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Full model candidate 126 now passes 3908 tests with 10
opt-in generators ignored; five new bounded-rejection controls cover exact fit,
shared surplus, binary/JSON roundtrip and rejection of retained business capture.
```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
The same borrowing producer now admits the Time phase once after prior phases,
uses the constructor-frozen count and actual Time event/matcher, and revalidates
each action at use after earlier callbacks. A source-bound Time descriptor and
call seed precede execution; internal work never takes a signed Network identity.
The transaction journal captures root/nested trace and exact-call receipts before
full-row fit. Same-generation repeat debit and retry clearing apply only after
fit. Healthy overflow drops those changes, business writes, witness and capture,
retains actual completed work and emits only the reserved root failure completion.
Real typed errors abort this success kernel and poison publication; actual
failure/retry disposition is not implemented. Existing legacy direct Time callers
are still unmigrated and cannot bypass the Running owner. Authored Time controls
exercise exact fit/one-byte-below rollback, frozen matching, use-time revalidation,
phase ownership, real errors, persisted retry and repeated schedule identity.
They remain unrun. Pipeline still needs the actual frozen Network route/event
provenance; do not synthesize routes or rederive them after execution.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
to the unfinished canonical driver, not to this frozen execution prefix. Next
Pipeline must consume its actual completed Network events and frozen routes;
then compose internal failure policy and the common source/witness/wire seal.
```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Final Core library check 127 fails with 18 errors and 110 warnings; full
unit-test compilation 130 fails with 313 errors and 104 warnings. Both preserve
the same 20,582 captured inputs. There is still no Core test executable.
Capture 124 retained a new borrowed-admission lifetime error; capture125 retained
seven new-test path, retired Mint constructor, boxed diagnostic and shared-event
pattern errors. These are corrected without dropping assertions. The gas test
name now accurately states its pre-body admission scope. Remaining production diagnostics are in retired Time and common
producer/seal APIs. All failed captures are retained. Fresh model and private
journal passes do not qualify actual State execution or production. Earlier
six structural replay parity passes remain their separate source/artifact scope.

```

## 2026-09-17: complete actual source inventory and State seal (141–148)

The replacement remains inactive in production, with native and unfinished ordinary State publication explicitly rejected. Its private Network/Pipeline/Time owner now retains every actual invocation, frozen route and network/height context, including rejected and zero-transcript calls. FASTPQ admits only those execution calls plus typed applied protocol-purpose sources. A State driver reserves, executes and consumes the actual rows through one seal: complete proposal commitments are checked before finalization, typed errors survive, source/transcript inventory is authenticated, complete outputs attach once, and exact wire hash/length remain bound. Errors, unwind and repeated takes remain nonpublishable; post-seal transaction application is refused. Five source-inventory and ten seal controls are authored, with all 91 existing inventory tests unchanged. Core library146 fails with 19 errors/111 warnings and test compilation145 with 313 errors/105 warnings on the same 20,589 unchanged inputs; no new owner/test-path error remains, but no Core test executable exists. Initial library144 retains its corrected iterator-bound error. Static147 passes formatting, whitespace and codec checks. Canonical Block/DAG execution is unchanged: rejected non-Batch fee metering, quarantine quota/order/cycles and actual instruction/byte limits require explicit replacement; raw-IVM failure metering and witness read ownership also need work. The old tail still calls the now test-only supplied inventory API. Model135, journal129, replay117, SDK87 and repair57 remain separate earlier evidence. Complete source/host admission, canonical finalization/genesis/native integration, SCCP applied outbox and unchanged four/seven-validator qualification remain open; see the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md). All L1–L6 and the real silent-initial-author counterexample remain open.

The canonical driver still owes complete source/finality and host admission. The producer now retains a privately constructed capsule in actual output order, including rejected and zero-transcript calls, exact frozen Network routes and originating network/height. FASTPQ compares proposal and frozen context, admits only actual execution calls and typed applied ProtocolPurpose extras, and preserves transcript-content sealing, digest validation and the invalidation latch. Independently applied fees/penalties remain legitimate transcript owners for rejected Network calls. One State-only driver reserves, executes all three phases and consumes its actual rows through a seal. Complete proposal commitments are checked before the finalizer. The caller cannot supply replacement rows, sources or applying policy. Finalizer effects precede fragment reconciliation, transcript inventory and one checked output attachment; exact wire hash/length remain bound. Typed finalizer errors survive. Partial/mock/foreign sources, unowned receipts, repeated takes, errors and unwind retain Poisoned; successful attachment retains Sealed. Ordinary and consensus-only transaction application after sealing is refused. The commit gate stays closed. Five source and ten seal controls are authored and unrun; all fourteen existing inventory child files and 91 tests remain byte-exact.

Canonical Block/DAG replacement remains pending an explicit economic/resource contract. Rejected ordinary Instructions/VM work needs actual fee metering; quarantine needs quota/order/restricted-cycle ownership; non-Batch materialized effects need actual instruction/byte limits. Broadening a Batch fee predicate or deleting caps would change economics/resource admission. Raw-IVM error exits also need completed-work metering. Removed prepared-read observations need deliberate witness ownership. Strict source timestamps, route/control/finality gates, deterministic execution order and complete AXT/lane/SCCP metadata remain mandatory. The best-effort DAG diagnostic sidecar may retire with that scheduler; required authenticated source sidecars may not. Genesis/native/merge, full source/host admission and complete witness/finality publication remain unfinished.

Core library146 fails with 19 errors/111 warnings; full unit-test compilation145 fails with 313 errors/105 warnings on the same 20589 unchanged inputs. No new owner/test-path primary error remains, but there is no Core test executable or actual State qualification. Initial library144 retained 20 errors/111 warnings including a new opaque-iterator Clone mismatch, corrected by borrowing the complete physical ordinary input slice; native/merge sources remain excluded. The correction also reauthenticates auxiliary proposal bodies before finalization and tests same-header auxiliary tampering. The old parallel tail still calls the now test-only supplied inventory API, an explicit unfinished cutover diagnostic. Static147 formatting, whitespace and codec checks pass. Model135 (3917 tests/10 ignored), journal129, replay117, SDK87 and repair57 retain separate source scopes. None qualifies MAIN composition, canonical Block execution, formal/workspace gates or four/seven-validator fault/restart tests. All L1–L6 and the real silent-initial-author counterexample remain open.

Implementation stays in the isolated checkout. MAIN receives only these four authoritative documents. Root package143, owned-fastpq142, DAG audit142 and seal review144 retain exact source evidence; CANDIDATE_CLARIFICATION preserves that strict private timestamp/quarantine changes were proposed, not implemented. Block, old tail and Network production bytes remain phase139-exact. No compatibility wrapper or accepting publication path is added. The current State test callback supplying empty lane statements does not qualify settlement metadata; the canonical Block finalizer is still pending. The ten seal controls cover actual phase rows and proposal preservation, combined/repeated driver use, partial/foreign source refusal, typed finalizer error/unwind, actual extra-fragment mismatch, changed signatures, post-seal transaction application, altered auxiliary proposal bytes and unowned receipts before/after finalization. No Core test ran. No new MAIN, full model/formal/workspace/SDK/network qualification is claimed.

### Exact superseded current prose

From `status.md`:

```text
The replacement remains inactive in production, with native and unfinished ordinary State publication explicitly rejected. Its private output owner now runs actual Network, Pipeline and Time phases. Pipeline derives events from retained Network results and frozen routes, preserving source/candidate positions across skips. Pipeline and Time share full-row fitting, callback capture and business rollback; real failures preserve authenticated quarantine or retry/removal, while healthy output overflow does neither. Oversized real failure diagnostics are bounded separately, with copies sized before allocation. Model 135 passes 3,917 tests with 10 opt-in generators ignored, including nine new rejection controls; an exact-source bounded formatter harness passes one test. Core library check 136 still fails with 18 errors/110 warnings; final test compilation 139 fails with 313 errors/104 warnings. Each captures 20,586 unchanged inputs; the final capture differs only by the corrected test sizing accessor. Actual State controls remain unexecuted because Core has no test executable. Journal129 and earlier replay/SDK/repair evidence retain separate scopes. The [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md) records failed captures and limits. Sole canonical producer/genesis integration, complete source/host admission, complete FASTPQ invocation inventory, SCCP applied-outbox authority, common seal and native runtime/old-path retirement remain open. No main-composition, formal, workspace or network qualification follows. All L1–L6 and the real four-validator silent-initial-author counterexample remain open.
```

From `roadmap.md`:

```text
| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner: integrate the private actual Network/Pipeline/Time owner and its rejection/penalty/fee/quarantine/retry policies into the sole canonical driver, retain every actual zero/nonzero-transcript call with its frozen source facts, and compose one consuming common seal, consume the retained State-frozen reservation, and establish future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |
```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
admission and canonical producer/common seal integration remain unfinished.
```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
to the unfinished canonical driver, not to this frozen execution prefix. Next
retain its frozen routes and complete actual Network/Pipeline/Time call inventory
through one consuming source/witness/wire seal, including zero-transcript sources.
Applied fee/penalty captures for rejected Network rows remain legitimate; unknown
ExecutionCall captures must refuse rather than become hash-sorted extra sources.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Core library check 136 fails with 18 errors and 110 warnings; final full
unit-test compilation 139 fails with 313 errors and 104 warnings. Every capture
keeps its 20,586 inputs unchanged. Only one test sizing accessor differs between
136/137 and 139; production and model source are unchanged. Capture137 retains
the initial 314-error result, including that new fixture error; it is corrected
without dropping assertions. No new owner/test-path error remains in final139,
but there is still no Core test executable or actual State qualification.
Model135 passes all 3917 regular controls (10 ignored generators); earlier
journal129, replay117, SDK87 and repair57 results retain their separate artifacts
and source scopes. Remaining production diagnostics are retired Time and common
producer/seal APIs. All failed captures remain retained. No formal, workspace,
real-network, MAIN composition or release qualification follows.

```

## 2026-09-17: execution-owned rejection fees and raw VM work (149–158)

The private Network owner now consumes an execution-owned rejection fee record for every signed source class. Admission freezes exact source/proposal/network/route/index, Gas/Nexus policy and payload length. The actual body supplies authored or verified-replay instruction count and direct VM/ISI gas; mixed Batch contract work accumulates in the same record. The root closes before Data DFS, and triggered contract work cannot become another direct root charge. Business rollback discards its effects, while independently committed penalties precede a fresh authenticated fee overlay. Fee pricing uses the admitted snapshot and actual retained work, never a reconstructed claimed overlay or a Batch-only eligibility predicate. Settlement consumes the record once and does not add gas or test completed work against the block ceiling again. Zero charges add no synthetic applied fee fragment. Typed internal/resource and block-gas exclusions remain; healthy output overflow drops staged business and fees. Successful and rejected roots use the same direct-body pricing basis; callback work still counts against block execution resources.

Both generic and self-describing raw-IVM branches now retain consumed gas before runtime, artifact-validation or artifact-application errors can return. The old post-application root assignment is removed so nested work is not overwritten. Deployed contract calls also record their actual direct VM work in the fee owner. Seven actual State fee controls and five actual raw-runtime controls are authored, including exact block-gas boundaries, admission failure, Data callback rollback, healthy output overflow, a consumed and context-bound fee record, admitted price retention, non-genesis raw rejection fees, generic/bound runtime exhaustion, artifact validation, artifact apply rollback and successful artifact application. None has executed. Two existing executor tests now use fresh overlays for all signed attempts, preserve every original assertion and warm cache, apply successful setup/work, and drop failed attempts.

Canonical Block/DAG execution remains unchanged. Its cutover still needs actual instruction/byte limits, quarantine quota/order/restricted cycles and explicit prepared-read witness ownership. The read-only effect-budget audit identifies the existing signed policy inputs and actual HostExecutionArtifacts apply boundary; it does not implement a resource owner. Those limits cover encoded InstructionBox sums, not complete host/decoded memory. Opaque-deferred healthy NoOp must be decided before charging effects that will not apply. Full source/finality/host admission, complete deterministic projection, canonical metadata and genesis/native/merge integration, SCCP applied outbox and four/seven-validator qualification remain open. Native and unfinished ordinary publication stay rejected. All L1–L6 and the real silent-initial-author counterexample remain open.

Core library156 fails with 19 errors/111 warnings and unit-test compilation155 with 313 errors/105 warnings on the same 20,592 unchanged inputs. Diagnostics match the prior migration baseline; no new fee-owner/raw-work/test primary error remains, but there is no Core executable and none of the twelve new regressions ran. Initial library152 retained the same 19 errors. Initial test153 retained 329 errors, including sixteen new fixture type/crypto-path diagnostics; corrected source155 also migrates two repeated-attempt fixtures and adds the actual raw rejection-fee control. Static157 formatting, whitespace and codec checks pass. Source144–148, model135, journal129, replay117, SDK87 and repair57 retain separate source scopes; no MAIN composition, formal/workspace/SDK or real-network pass is claimed.

Implementation remains in the isolated checkout. MAIN receives only the four authoritative documents; HEAD, MERGE_HEAD and staged merge are preserved. The latest repair-temp review remains separately implemented in MAIN: pristine repair admission rejects unowned temporaries, while an existing authenticated repair locator can own exact canonical temporary bytes. Its actual post-write/flush/fsync/pre-rename crash cut and restart/tamper/foreign controls belong to candidate57 evidence, not these failed Core diagnostics. The final fee patch leaves the actual source inventory, State seal, canonical Block and old tail unchanged. No compatibility shim or accepting publication path is introduced. The actual fee-failure-after-committed-penalty runtime control remains required; the previous real two-ballot control is preserved.

### Exact superseded current prose

From `status.md`:

```text
The replacement remains inactive in production, with native and unfinished ordinary State publication explicitly rejected. Its private Network/Pipeline/Time owner now retains every actual invocation, frozen route and network/height context, including rejected and zero-transcript calls. FASTPQ admits only those execution calls plus typed applied protocol-purpose sources. A State driver reserves, executes and consumes the actual rows through one seal: complete proposal commitments are checked before finalization, typed errors survive, source/transcript inventory is authenticated, complete outputs attach once, and exact wire hash/length remain bound. Errors, unwind and repeated takes remain nonpublishable; post-seal transaction application is refused. Five source-inventory and ten seal controls are authored, with all 91 existing inventory tests unchanged. Core library146 fails with 19 errors/111 warnings and test compilation145 with 313 errors/105 warnings on the same 20,589 unchanged inputs; no new owner/test-path error remains, but no Core test executable exists. Initial library144 retains its corrected iterator-bound error. Static147 passes formatting, whitespace and codec checks. Canonical Block/DAG execution is unchanged: rejected non-Batch fee metering, quarantine quota/order/cycles and actual instruction/byte limits require explicit replacement; raw-IVM failure metering and witness read ownership also need work. The old tail still calls the now test-only supplied inventory API. Model135, journal129, replay117, SDK87 and repair57 remain separate earlier evidence. Complete source/host admission, canonical finalization/genesis/native integration, SCCP applied outbox and unchanged four/seven-validator qualification remain open; see the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md). All L1–L6 and the real silent-initial-author counterexample remain open.
```

From `roadmap.md`:

```text
| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner: integrate the private actual Network/Pipeline/Time owner and its rejection/penalty/fee/quarantine/retry policies into the sole canonical driver, qualify the retained complete source inventory and State consuming seal, unify rejected non-Batch fee metering, quarantine quota/order/cycles and actual instruction/byte limits before replacing the canonical Block/DAG driver, and consume the retained State-frozen reservation, and establish future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |
```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
route/DA/AMX authority, source/host-memory and common-wire admission still belong
The canonical driver still owes complete source/finality and host admission.
```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Canonical Block/DAG replacement remains pending an explicit economic/resource
contract. Rejected ordinary Instructions/VM work needs actual fee metering;
quarantine needs quota/order/restricted-cycle ownership; non-Batch materialized
effects need actual instruction/byte limits. Broadening a Batch fee predicate or
deleting caps would change economics/resource admission. Raw-IVM error exits also
need completed-work metering. Removed prepared-read observations need deliberate
witness ownership. Strict source timestamps, route/control/finality gates,
deterministic execution order and complete AXT/lane/SCCP metadata remain
mandatory. The best-effort DAG diagnostic sidecar may retire with that scheduler;
required authenticated source sidecars may not. Genesis/native/merge, full
source/host admission and complete witness/finality publication remain unfinished.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Core library146 fails with 19 errors/111 warnings; full unit-test compilation145
fails with 313 errors/105 warnings on the same 20589 unchanged inputs. No new
owner/test-path primary error remains, but there is no Core test executable or
actual State qualification. Initial library144 retained 20 errors/111 warnings
including a new opaque-iterator Clone mismatch, corrected by borrowing the
complete physical ordinary input slice; native/merge sources remain excluded. The
correction also reauthenticates auxiliary proposal bodies before finalization and
tests same-header auxiliary tampering. The old parallel tail still calls the now
test-only supplied inventory API, an explicit unfinished cutover diagnostic.
Static147 formatting, whitespace and codec checks pass. Model135 (3917 tests/10
ignored), journal129, replay117, SDK87 and repair57 retain separate source scopes.
None qualifies MAIN composition, canonical Block execution, formal/workspace gates
or four/seven-validator fault/restart tests. All L1–L6 and the real silent-
initial-author counterexample remain open.

```

## 2026-09-17: signed-root instruction admission before effects (159–177)

One private signed-root instruction budget now freezes the exact source, proposal, network, route/index and agreed overlay instruction/byte policy. Authored Instructions and Batch sets, verified-replay queues and consumed actual HostExecutionArtifacts all admit a whole group before its first effect. Mixed Batch calls debit that same cumulative owner; the late returned-instruction recheck is removed. Individual fixed V1 bare InstructionBox encodings are measured with a bounded counting writer instead of an allocated encoding vector, and the byte diagnostic reports only that the measured ceiling was exceeded. Zero retains the existing no-additional-limit meaning. This is an instruction count/encoding budget, not a complete host/decode/durable-state memory bound.

The actual Executor wrapper closes effect and fee roots on every normal result before Data callbacks. The actual synchronous trigger depth excludes both generic and bound callback bodies from the direct-root instruction budget. Healthy opaque-deferred NoOp is decided before artifact admission. Completed VM/replay work survives later cap refusal, while refused authored work and never-started queued confidential instructions are not recorded as executed work. Private Network fee exclusion now uses the retained typed cap failure instead of parsing diagnostic strings. Missing, repeated, foreign, unclosed and failed owners cannot publish staged effects; ordinary and consensus-effects apply guards poison incomplete State publication.

Sixteen new controls are authored: exact/unlimited/one-below authored and actual generic/bound/mixed VM groups; bare V1 encoding under ambient flags, atomic group counting and arithmetic failures; missing-root actual artifact refusal; actual Network fees and plain/generic-IVM callbacks; owner reuse/context substitution through both apply paths and a post-stage unwind; supplied post-verification replay count/byte refusal with retained gas. None has executed. Three existing Host artifact tests and three existing supplied-replay tests retain all original assertions with explicit signed-root ownership; one mixed byte diagnostic assertion now reflects bounded measurement. The supplied replay fixtures do not claim actual proof verification.

Canonical Block/DAG execution remains unchanged. Its cutover still needs one deterministic quarantine admission/order/quota/cycle rule and explicit prepared-read witness ownership. The read-only quarantine audit exposes old parallel/sequential and live-carrier exceptions; it recommends one frozen classification/disposition per source, separate deterministic quota ranking, and a true shared cycle owner, but implements none of them. Full source/finality/host admission, complete deterministic projection, canonical metadata and genesis/native/merge integration, SCCP applied outbox and four/seven-validator qualification remain open. Native and unfinished ordinary publication stay rejected. All L1–L6 and the real silent-initial-author counterexample remain open.

Core library175 fails with 19 errors/111 warnings and unit-test compilation174 with 313 errors/105 warnings on the same 20,595 unchanged inputs. Primary error message/file/source-text multisets match library156 and test155; no additional effect-owner/test primary error remains, but there is no Core executable and none of the sixteen new regressions ran. Initial test165 retains 315 errors, including the two corrected new-test imports. Intermediate169/170 retain baseline313/19 errors plus four single-use lifetime warnings; explicit iterator lifetime bounds remove those warnings in final174/175. Static176 formatting, whitespace and codec checks pass. Previous fee155/156, model135, journal129, replay117, SDK87 and repair57 remain separate evidence; no MAIN composition, formal/workspace/SDK or real-network pass is claimed.

Implementation remains isolated. MAIN receives only four authoritative documents; HEAD, MERGE_HEAD and staged merge remain untouched. The latest repair-temporary review remains separately fixed in MAIN and candidate57: an existing authenticated repair index can own exact canonical temporary bytes, pristine admission cannot adopt them, and the actual post-write/flush/fsync/pre-rename crash cut has strict restart and tamper/foreign controls. This phase does not requalify that MAIN composition. Neither the canonical Block/DAG nor a publication acceptance gate is changed. The actual fee-failure-after-committed-penalty control and complete runtime qualification remain required.

### Exact superseded current prose

From `status.md`:

```text
The replacement remains inactive in production, with native and unfinished ordinary State publication explicitly rejected. Its private Network/Pipeline/Time owner and consuming State seal retain exact actual sources, frozen routes, transcript inventory and final wire identity. Rejected signed sources now retain one admitted fee policy and actual direct work across business rollback; the fresh fee overlay authenticates that record and never counts completed gas twice. Both raw-IVM paths retain work before runtime/artifact failures, without overwriting nested gas. Twelve regressions are authored and two existing executor fixtures preserve their assertions using separate attempts. Core library156 fails with 19 errors/111 warnings and unit-test compilation155 with 313 errors/105 warnings on the same 20,592 unchanged inputs. Diagnostics match the prior migration baseline; no new fee-owner/raw-work/test primary error remains, but there is no Core executable and none of the twelve new regressions ran. Initial library152 retained the same 19 errors. Initial test153 retained 329 errors, including sixteen new fixture type/crypto-path diagnostics; corrected source155 also migrates two repeated-attempt fixtures and adds the actual raw rejection-fee control. Static157 formatting, whitespace and codec checks pass. Source144–148, model135, journal129, replay117, SDK87 and repair57 retain separate source scopes; no MAIN composition, formal/workspace/SDK or real-network pass is claimed. Canonical Block/DAG execution remains unchanged. Its cutover still needs actual instruction/byte limits, quarantine quota/order/restricted cycles and explicit prepared-read witness ownership. The read-only effect-budget audit identifies the existing signed policy inputs and actual HostExecutionArtifacts apply boundary; it does not implement a resource owner. Those limits cover encoded InstructionBox sums, not complete host/decoded memory. Opaque-deferred healthy NoOp must be decided before charging effects that will not apply. Full source/finality/host admission, complete deterministic projection, canonical metadata and genesis/native/merge integration, SCCP applied outbox and four/seven-validator qualification remain open. Native and unfinished ordinary publication stay rejected. All L1–L6 and the real silent-initial-author counterexample remain open. See the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).
```

From `roadmap.md`:

```text
| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner: integrate the private actual Network/Pipeline/Time owner and its rejection/penalty/fee/quarantine/retry policies into the sole canonical driver, qualify the retained complete source inventory and State consuming seal, qualify execution-owned rejected fees and raw-IVM work, establish quarantine quota/order/cycles and actual instruction/byte limits before replacing the canonical Block/DAG driver, and consume the retained State-frozen reservation, and establish future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |
```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Canonical Block/DAG execution remains unchanged. Its cutover still needs actual
instruction/byte limits, quarantine quota/order/restricted cycles and explicit
prepared-read witness ownership. The read-only effect-budget audit identifies the
existing signed policy inputs and actual HostExecutionArtifacts apply boundary; it
does not implement a resource owner. Those limits cover encoded InstructionBox
sums, not complete host/decoded memory. Opaque-deferred healthy NoOp must be
decided before charging effects that will not apply. Full source/finality/host
admission, complete deterministic projection, canonical metadata and
genesis/native/merge integration, SCCP applied outbox and four/seven-validator
qualification remain open. Native and unfinished ordinary publication stay
rejected. All L1–L6 and the real silent-initial-author counterexample remain open.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Core library156 fails with 19 errors/111 warnings and unit-test compilation155
with 313 errors/105 warnings on the same 20,592 unchanged inputs. Diagnostics
match the prior migration baseline; no new fee-owner/raw-work/test primary error
remains, but there is no Core executable and none of the twelve new regressions
ran. Initial library152 retained the same 19 errors. Initial test153 retained 329
errors, including sixteen new fixture type/crypto-path diagnostics; corrected
source155 also migrates two repeated-attempt fixtures and adds the actual raw
rejection-fee control. Static157 formatting, whitespace and codec checks pass.
Source144–148, model135, journal129, replay117, SDK87 and repair57 retain separate
source scopes; no MAIN composition, formal/workspace/SDK or real-network pass is
claimed.

```

## 2026-09-17: frozen quarantine selection and shared actual VM cycles (178–195)

Private Network admission now freezes exact signed Boolean quarantine classification once. Only stateless-admitted signed sources consume quota, ranked deterministically by outer entrypoint hash and original index; selection never changes the original effect order or separately frozen reveal order. Zero quota rejects all classified sources before business execution, gas, fees or penalties. A later selected-source business or output failure does not refill its slot. Batch and ballot shapes have no exception. Policy drift after freezing is a local ownership error. This policy remains in the private replacement owner; the old canonical Block/DAG is unchanged.

A finite non-cloneable completed-cycle owner belongs to the actual signed root and spans direct generic or bound VMs, mixed Batch segments, actual proved replay and recursive CoreHost calls. Each dispatch reserves its architectural cost before effects, holds parent syscall reservations through children and retains completed work through traps, reuse and unwind. HALT commits before trace flushing. Exact fit succeeds; a refused reservation is sticky even when a host swallows a child error. Local foreign/closed owners remain distinct from deterministic cycle exhaustion. Zero cycle limit means no additional bound. Actual trigger callbacks have their own scope. This is completed architectural-cycle accounting, not total host, proof, decode or allocation work.

Actual replay work now transfers to State and the root fee meter immediately after verification returns, including rejection after real execution. Successful replay records its actual instruction basis once before later SCCP, AXT, payer, block-cap or effect admission can fail; metered replay requires the retained gas and does not add it again. Supplied post-verification fixtures now state their supplied work handoff explicitly; they do not claim measured execution or cryptographic proof. Typed instruction/byte preflight exemptions remain separate from chargeable cycle exhaustion.

Twenty-four new regressions are authored: nine actual VM cycle tests, two real nested CoreHost tests, nine actual Network quota/fee/reveal tests, generic signed and mixed-Batch root cycle limits, authorized actual replay work and an actual generic callback under a finite quarantined root. Existing proof fixtures additionally cover cryptographic replay rejection and a successful actual proof/replay followed by block-gas refusal, preserving all prior assertions. Existing supplied-replay fixtures preserve their assertions with explicit work custody. Only the nine new VM cases have executed; the fifteen new Core cases and expanded Core fixtures have no runtime result.

Canonical Block/DAG cutover still requires explicit prepared-read witness ownership, complete deterministic State projection, whole-source admission/finality and allocation bounds, canonical metadata and genesis/native/merge integration, and the SCCP applied outbox. The private quota/cycle owner must be integrated and qualified together with actual fees, output budgets and source seals. Native and unfinished ordinary State publication remain rejected. All L1–L6, the real silent-initial-author counterexample and four/seven-validator qualification remain open.

VM186 passes 88 dispatch/cycle tests, including all nine new VM controls, on 20,598 unchanged local inputs. Initial VM182 preserves five passes/four fixture admission failures from an unknown syscall; the corrected fixture uses an admitted canonical syscall. Core test compilation192 fails with 313 errors/105 warnings; library193 fails with 19 errors/111 warnings. Error message/file/source-text multisets match test174/library175. Initial Core187 retains 315 errors, including the two corrected replay-fixture type/import errors. Core192/193 use the same complete inputs; VM186 differs only by that Core test-fixture correction. There is still no Core executable and no Core runtime qualification. Static194 formatting, whitespace and codec checks pass. Previous model135, journal129, replay117, SDK87 and repair57 remain separate evidence; no MAIN composition, full formal/workspace/SDK or real-network pass is claimed.

All implementation changes remain isolated. MAIN receives only the four authoritative documents; its HEAD, MERGE_HEAD and staged merge remain unchanged. The repair-temporary review remains separately fixed in MAIN/candidate57 with owned exact temporary recovery and an actual post-write/fsync crash cut; this phase does not qualify the combined MAIN checkout.

### Exact superseded current prose

From `status.md`:

```text
The replacement remains inactive in production, with native and unfinished ordinary State publication explicitly rejected. Its private Network/Pipeline/Time owner retains exact sources, frozen routes, actual fee work and a consuming State output seal. Signed instruction/byte admission now has one retained owner across authored, verified-replay and actual VM artifact groups; complete groups are checked before effects apply, with callback scope and typed fee exclusions. Core library175 fails with 19 errors/111 warnings and unit-test compilation174 with 313 errors/105 warnings on the same 20,595 unchanged inputs. Primary error message/file/source-text multisets match library156 and test155; no additional effect-owner/test primary error remains, but there is no Core executable and none of the sixteen new regressions ran. Initial test165 retains 315 errors, including the two corrected new-test imports. Intermediate169/170 retain baseline313/19 errors plus four single-use lifetime warnings; explicit iterator lifetime bounds remove those warnings in final174/175. Static176 formatting, whitespace and codec checks pass. Previous fee155/156, model135, journal129, replay117, SDK87 and repair57 remain separate evidence; no MAIN composition, formal/workspace/SDK or real-network pass is claimed. Canonical Block/DAG execution remains unchanged. Its cutover still needs one deterministic quarantine admission/order/quota/cycle rule and explicit prepared-read witness ownership. The read-only quarantine audit exposes old parallel/sequential and live-carrier exceptions; it recommends one frozen classification/disposition per source, separate deterministic quota ranking, and a true shared cycle owner, but implements none of them. Full source/finality/host admission, complete deterministic projection, canonical metadata and genesis/native/merge integration, SCCP applied outbox and four/seven-validator qualification remain open. Native and unfinished ordinary publication stay rejected. All L1–L6 and the real silent-initial-author counterexample remain open. See the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).
```

From `roadmap.md`:

```text
| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner: integrate the private actual Network/Pipeline/Time owner and its rejection/penalty/fee/quarantine/retry policies into the sole canonical driver, qualify the retained complete source inventory and State consuming seal, qualify execution-owned fees, raw-IVM work and actual signed instruction/byte admission, establish one deterministic quarantine quota/order/cycle owner before replacing the canonical Block/DAG driver, and consume the retained State-frozen reservation, and establish future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |
```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Canonical Block/DAG execution remains unchanged. Its cutover still needs one
deterministic quarantine admission/order/quota/cycle rule and explicit prepared-
read witness ownership. The read-only quarantine audit exposes old
parallel/sequential and live-carrier exceptions; it recommends one frozen
classification/disposition per source, separate deterministic quota ranking, and a
true shared cycle owner, but implements none of them. Full source/finality/host
admission, complete deterministic projection, canonical metadata and
genesis/native/merge integration, SCCP applied outbox and four/seven-validator
qualification remain open. Native and unfinished ordinary publication stay
rejected. All L1–L6 and the real silent-initial-author counterexample remain open.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Core library175 fails with 19 errors/111 warnings and unit-test compilation174
with 313 errors/105 warnings on the same 20,595 unchanged inputs. Primary error
message/file/source-text multisets match library156 and test155; no additional
effect-owner/test primary error remains, but there is no Core executable and none
of the sixteen new regressions ran. Initial test165 retains 315 errors, including
the two corrected new-test imports. Intermediate169/170 retain baseline313/19
errors plus four single-use lifetime warnings; explicit iterator lifetime bounds
remove those warnings in final174/175. Static176 formatting, whitespace and codec
checks pass. Previous fee155/156, model135, journal129, replay117, SDK87 and
repair57 remain separate evidence; no MAIN composition, formal/workspace/SDK or
real-network pass is claimed.

```

## 2026-09-17: actual World preimages and private net-delta seal (196–210)

Actual MV block and transaction journals now expose borrowed before/after entries and cell preimages. Applied siblings preserve the first block preimage; failed children and unwind leave no applied delta. Block replacement starts from the reverted parent. Explicit no-op and absent-to-absent touches remain distinguishable, with no additional key/value clone or change-list allocation in these accessors.

The private World net-delta fold consumes the same 278-field registry as the overlay constructors, with the trigger owner projecting its ten stores in place of one field. It includes snapshot-skipped authoritative values and derived lookup indexes. Values use streamed fixed V1 bare Norito; the fold binds field identity/type, key, presence, before/after value hashes and counts. Canonical equal values remove no-op mutation history. Trigger actions/bytecode and the executor use borrowed semantic projections that exclude their loaded runtime caches. Encoding/count failure or unwind retains an incomplete-field latch, so no partial fold can finish. Existing merge, snapshot and consensus formats are unchanged.

The actual private ordinary output seal captures its own World net delta after finalizer effects and checks the live World again during seal verification. Unchanged block wire bytes cannot hide a later changed World value. Applied State transaction guards and the unfinished publication gate remain in force. This retained delta is one private component of the future State proof; it is not supplied by the caller or exposed as an ExecutionCommitment.

The stable execution identity must come from canonical State values and an authenticated persistent baseline, independent of incidental read order, caches or scheduler hints. Existing witness roots discard pure read keys when writes exist, and the recovery snapshot retains undo history while omitting some authoritative World fields; neither currently authenticates a complete State projection. The new World delta deliberately does not bind untouched values. State-owned membership/runtime configuration, process event delivery, post-finality effects, complete semantic/cache invariance, incremental baseline/tree maintenance and agreed work/allocation limits still need explicit coverage. Actual read observations remain required where proofs or diagnosis depend on them, without becoming a competing state-root authority.

Twenty-eight regressions are authored: eleven actual MV preimage/apply/rollback/replacement/no-clone controls, seven World projection controls, seven TriggerSet semantic/rollback/history controls, one actual loaded-executor cache control and two actual output-seal controls. MV200 passes all 51 library tests with no warnings, including the eleven new cases. The seventeen new Core tests remain unexecuted.

Canonical Block/DAG cutover still requires an authenticated incremental State baseline and the complete State projection beyond this World delta, State-owned proof/read capture, whole-source admission/finality and allocation bounds, canonical metadata and genesis/native/merge integration, and the SCCP applied outbox. The private output, source, fee, quota/cycle and World-delta owners must be integrated and qualified together. Native and unfinished ordinary State publication remain rejected. All L1–L6, the real silent-initial-author counterexample and four/seven-validator qualification remain open.

MV200 passes 51 tests on 20,603 unchanged local inputs. Initial Core201 retains 325 errors/105 warnings: all 313 previous errors plus twelve new import, reference-codec, hash-slice and Option-fixture diagnostics. Those new diagnostics are corrected. Final Core test compilation205 retains 313 errors/105 warnings and library206 retains 19 errors/111 warnings; primary error and warning message/file/source-text multisets exactly match192/193. Final205/206 inputs are identical and unchanged; MV200 differs only in four corrected Core files, with MV and its dependencies unchanged. Static207 formatting, whitespace and codec checks pass. No Core executable or Core runtime qualification exists. Prior VM186, model135, journal129, replay117, SDK87 and repair57 remain separate evidence. No MAIN composition, full formal/workspace/SDK or real-network pass is claimed.

All implementation changes remain isolated. MAIN receives only the four authoritative documents and ignored evidence; its HEAD, MERGE_HEAD and staged merge remain unchanged. The authenticated repair-temporary review remains separately fixed in MAIN/candidate57 with an actual post-write/fsync crash cut. This phase does not qualify the combined MAIN checkout. The 14-file source delta has exact forward/reverse zero-fuzz reconstruction; initial errors and their corrections are retained.

### Exact superseded current prose

From `status.md`:

```text
The replacement remains inactive in production, with native and unfinished ordinary State publication explicitly rejected. Private Network admission now owns one deterministic quarantine quota selection, separate from effect/reveal order, with no failed-slot refill or Batch/ballot exception. One signed root owns finite completed VM cycles across direct, mixed, nested and proved replay runs; callbacks remain separate. Actual replay work transfers before later admission can fail. VM186 passes 88 dispatch/cycle tests, including all nine new VM controls, on 20,598 unchanged local inputs. Initial VM182 preserves five passes/four fixture admission failures from an unknown syscall; the corrected fixture uses an admitted canonical syscall. Core test compilation192 fails with 313 errors/105 warnings; library193 fails with 19 errors/111 warnings. Error message/file/source-text multisets match test174/library175. Initial Core187 retains 315 errors, including the two corrected replay-fixture type/import errors. Core192/193 use the same complete inputs; VM186 differs only by that Core test-fixture correction. There is still no Core executable and no Core runtime qualification. Static194 formatting, whitespace and codec checks pass. Previous model135, journal129, replay117, SDK87 and repair57 remain separate evidence; no MAIN composition, full formal/workspace/SDK or real-network pass is claimed. Canonical Block/DAG cutover still requires explicit prepared-read witness ownership, complete deterministic State projection, whole-source admission/finality and allocation bounds, canonical metadata and genesis/native/merge integration, and the SCCP applied outbox. The private quota/cycle owner must be integrated and qualified together with actual fees, output budgets and source seals. Native and unfinished ordinary State publication remain rejected. All L1–L6, the real silent-initial-author counterexample and four/seven-validator qualification remain open. See the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).
```

From `roadmap.md`:

```text
| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish complete deterministic State projection and one typed invocation-output owner: integrate the private actual Network/Pipeline/Time owner and its rejection/penalty/fee/quarantine/retry policies into the sole canonical driver, qualify the retained complete source inventory and State consuming seal, qualify execution-owned fees, raw-IVM work and actual signed instruction/byte admission, integrate and qualify the private deterministic quarantine quota and shared actual VM cycle owner before replacing the canonical Block/DAG driver, and consume the retained State-frozen reservation, and establish future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |
```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Canonical Block/DAG cutover still requires explicit prepared-read witness
ownership, complete deterministic State projection, whole-source
admission/finality and allocation bounds, canonical metadata and
genesis/native/merge integration, and the SCCP applied outbox. The private
quota/cycle owner must be integrated and qualified together with actual fees,
output budgets and source seals. Native and unfinished ordinary State publication
remain rejected. All L1–L6, the real silent-initial-author counterexample and
four/seven-validator qualification remain open.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
VM186 passes 88 dispatch/cycle tests, including all nine new VM controls, on
20,598 unchanged local inputs. Initial VM182 preserves five passes/four fixture
admission failures from an unknown syscall; the corrected fixture uses an admitted
canonical syscall. Core test compilation192 fails with 313 errors/105 warnings;
library193 fails with 19 errors/111 warnings. Error message/file/source-text
multisets match test174/library175. Initial Core187 retains 315 errors, including
the two corrected replay-fixture type/import errors. Core192/193 use the same
complete inputs; VM186 differs only by that Core test-fixture correction. There is
still no Core executable and no Core runtime qualification. Static194 formatting,
whitespace and codec checks pass. Previous model135, journal129, replay117, SDK87
and repair57 remain separate evidence; no MAIN composition, full
formal/workspace/SDK or real-network pass is claimed.

```

## 2026-09-17: persistent World baseline and exact lifecycle cut audit (211–219)

A private persistent World baseline now complements the actual World net delta. The shared visitor uses the same 278-field constructor registry, expanding TriggerSet into ten semantic stores for 287 field sections. Cold capture includes untouched values, snapshot-skipped authoritative stores and derived indexes. Incremental versions encode only actual touched before/after pairs and check every touched preimage, including no-ops. Cold predecessor capture reverses the actual first-preimage journal after MV replacement undo. Field names/order/kinds are bound; this does not introduce independent Norito type-schema identifiers.

The lower-level MerkleMap uses immutable shared nodes and a canonical compressed binary radix tree of domain-separated key/value hashes. Its shape/root depend on current values, not insertion, deletion or undo history. Branches bind their split bit, raw shared prefix and ordered children; absence, empty values and count remain distinct. Each update checks its preimage/count before replacing the root and copies only its bounded key path. Existing Hash supplies the hashing implementation. The map has no wire format, external node/proof input or disk durability claim. Initial capture, retained versions, final snapshot destruction and aggregate allocation still require resource admission.

The existing private output seal continues to bind the actual World net delta. The new complete World value baseline is a private component exercised by tests, not installed in the production State lifecycle, a full State root, or publication permission. It requires the same actual predecessor: touched-key comparison cannot authenticate unrelated untouched-state drift. Executor and trigger semantic encoders are shared by both projections; merge, snapshot and consensus commitment formats are unchanged.

The source-bound lifecycle audit identifies late World writes after output sealing: AXT incarnations/ratchets, replay expiry, DA quota/pin changes and lane cleanup, including live World pruning after World commit. Transaction replay membership and agreed runtime/context publication are separate owners. Replacement reverts MV owners but currently clones some live Nexus/incarnation/lineage/manifest metadata. The complete baseline therefore needs one State-owned commit-preparation capsule, exact custom replay-membership projection, authenticated runtime predecessor restoration and publication in the same State generation. Root/attestation dependency order must avoid self-reference; existing checkpoint hashes and merge hint roots retain their narrower meanings.

Twelve new regressions are authored. All six persistent-map tests pass: 120 insertion/removal orders, all 255 variable hash-bit splits and byte boundaries, 700 mixed updates against an independent sorted rebuild, stale-preimage/count failure, immutable subtree sharing and independently calculated hash vectors. Six actual World baseline tests cover untouched/skipped values, cold/incremental equivalence, commit/replacement/rollback, stale-preimage refusal, touched-only encoding, schema/presence and actual trigger stores. These six Core tests remain unrun. Independent source reviews found no concrete defect; they are not runtime qualification.

Canonical Block/DAG cutover requires coupling the private persistent World baseline to one State-owned commit-preparation capsule, completing non-World replay/runtime/context projection and authenticated predecessor restoration, State-owned proof/read capture, whole-source admission/finality and allocation bounds, canonical metadata and genesis/native/merge integration, and the SCCP applied outbox. The private output, source, fee, quota/cycle, World delta and baseline owners must be integrated and qualified together. Native and unfinished ordinary State publication remain rejected. All L1–L6, the silent-initial-author counterexample and four/seven-validator qualification remain open.

Crypto213 passes six tests with zero warnings. Core test compilation214 retains 313 errors/106 warnings; library216 retains 19 errors/112 warnings. Both primary error message/file/source-text multisets exactly match205/206. Each Core check adds one unused baseline-export warning before lifecycle integration; it is not suppressed. All three captures use the same 20,607 unchanged local inputs. Static215 formatting, whitespace and codec checks pass. The eight-file source delta reconstructs exactly forward/reverse with zero fuzz. No Core executable or new Core runtime qualification exists. Prior MV200, VM186, model135, journal129, replay117, SDK87 and repair57 are separate evidence. MAIN composition, full formal/workspace/SDK and real-network qualification remain open.

Implementation stays isolated. MAIN receives only these four authoritative documents and ignored evidence; its HEAD, MERGE_HEAD and staged merge remain unchanged. The earlier authenticated repair-temporary review remains fixed in MAIN/candidate57 with an actual post-write/fsync crash cut; this phase does not qualify combined MAIN.

### Exact superseded current prose

From `status.md`:

```text
The replacement remains inactive in production, with native and unfinished ordinary State publication explicitly rejected. The private output seal now binds exact net changes across the World registry from actual MV preimages, including trigger semantic stores, and checks for later World mutation. This is not a complete State root: untouched baseline, non-World State, lifecycle and resource coverage remain open. MV200 passes 51 tests on 20,603 unchanged local inputs. Initial Core201 retains 325 errors/105 warnings: all 313 previous errors plus twelve new import, reference-codec, hash-slice and Option-fixture diagnostics. Those new diagnostics are corrected. Final Core test compilation205 retains 313 errors/105 warnings and library206 retains 19 errors/111 warnings; primary error and warning message/file/source-text multisets exactly match192/193. Final205/206 inputs are identical and unchanged; MV200 differs only in four corrected Core files, with MV and its dependencies unchanged. Static207 formatting, whitespace and codec checks pass. No Core executable or Core runtime qualification exists. Prior VM186, model135, journal129, replay117, SDK87 and repair57 remain separate evidence. No MAIN composition, full formal/workspace/SDK or real-network pass is claimed. Canonical Block/DAG cutover still requires an authenticated incremental State baseline and the complete State projection beyond this World delta, State-owned proof/read capture, whole-source admission/finality and allocation bounds, canonical metadata and genesis/native/merge integration, and the SCCP applied outbox. The private output, source, fee, quota/cycle and World-delta owners must be integrated and qualified together. Native and unfinished ordinary State publication remain rejected. All L1–L6, the real silent-initial-author counterexample and four/seven-validator qualification remain open. See the [foundation record](docs/history/2026-09-16/sumeragi-lane-context-foundation.md).
```

From `roadmap.md`:

```text
| N0 | Sumeragi liveness redesign | Core/Sumeragi, lifecycle, P2P, Kura, formal and integration owners | Complete [L1–L6](specs/sumeragi_liveness_redesign_goals.md): replace global witness authority with State-owned capture, establish an authenticated incremental State baseline, complete the projection beyond the private World net delta, and qualify one typed invocation-output owner: integrate the private actual Network/Pipeline/Time owner and its rejection/penalty/fee/quarantine/retry policies into the sole canonical driver, qualify the retained complete source inventory and State consuming seal, qualify execution-owned fees, raw-IVM work and actual signed instruction/byte admission, integrate and qualify the private deterministic quarantine quota and shared actual VM cycle owner before replacing the canonical Block/DAG driver, and consume the retained State-frozen reservation, and establish future-invariant whole-source admission and complete allocation budgets, qualify Network admission and bounded exact-finality history/proof consumers, finish resident reservations and actual State/Kura qualification, replace outcome-dependent SCCP header commitments with one applied outbox and QC execution owner, then finish native publication/Apply and atomically retire old signers and the Ordinary economic bypass, remove competing scheduling authority, make wake events reachable, preserve results and durable obligations across generation/restart, eliminate resource cycles and permanent retry loops, and qualify one unchanged candidate on four/seven validators. First-release replacement requires no compatibility shims; existing safety, DA and release gates remain mandatory. |
```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Actual MV block and transaction journals now expose borrowed before/after entries
and cell preimages. Applied siblings preserve the first block preimage; failed
children and unwind leave no applied delta. Block replacement starts from the
reverted parent. Explicit no-op and absent-to-absent touches remain
distinguishable, with no additional key/value clone or change-list allocation in
these accessors.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
The private World net-delta fold consumes the same 278-field registry as the
overlay constructors, with the trigger owner projecting its ten stores in place of
one field. It includes snapshot-skipped authoritative values and derived lookup
indexes. Values use streamed fixed V1 bare Norito; the fold binds field
identity/type, key, presence, before/after value hashes and counts. Canonical
equal values remove no-op mutation history. Trigger actions/bytecode and the
executor use borrowed semantic projections that exclude their loaded runtime
caches. Encoding/count failure or unwind retains an incomplete-field latch, so no
partial fold can finish. Existing merge, snapshot and consensus formats are
unchanged.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
The actual private ordinary output seal captures its own World net delta after
finalizer effects and checks the live World again during seal verification.
Unchanged block wire bytes cannot hide a later changed World value. Applied State
transaction guards and the unfinished publication gate remain in force. This
retained delta is one private component of the future State proof; it is not
supplied by the caller or exposed as an ExecutionCommitment.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
The stable execution identity must come from canonical State values and an
authenticated persistent baseline, independent of incidental read order, caches or
scheduler hints. Existing witness roots discard pure read keys when writes exist,
and the recovery snapshot retains undo history while omitting some authoritative
World fields; neither currently authenticates a complete State projection. The new
World delta deliberately does not bind untouched values. State-owned
membership/runtime configuration, process event delivery, post-finality effects,
complete semantic/cache invariance, incremental baseline/tree maintenance and
agreed work/allocation limits still need explicit coverage. Actual read
observations remain required where proofs or diagnosis depend on them, without
becoming a competing state-root authority.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Twenty-eight regressions are authored: eleven actual MV
preimage/apply/rollback/replacement/no-clone controls, seven World projection
controls, seven TriggerSet semantic/rollback/history controls, one actual loaded-
executor cache control and two actual output-seal controls. MV200 passes all 51
library tests with no warnings, including the eleven new cases. The seventeen new
Core tests remain unexecuted.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
Canonical Block/DAG cutover still requires an authenticated incremental State
baseline and the complete State projection beyond this World delta, State-owned
proof/read capture, whole-source admission/finality and allocation bounds,
canonical metadata and genesis/native/merge integration, and the SCCP applied
outbox. The private output, source, fee, quota/cycle and World-delta owners must
be integrated and qualified together. Native and unfinished ordinary State
publication remain rejected. All L1–L6, the real silent-initial-author
counterexample and four/seven-validator qualification remain open.

```

From `specs/sumeragi_liveness_redesign_goals.md`:

```text
MV200 passes 51 tests on 20,603 unchanged local inputs. Initial Core201 retains
325 errors/105 warnings: all 313 previous errors plus twelve new import,
reference-codec, hash-slice and Option-fixture diagnostics. Those new diagnostics
are corrected. Final Core test compilation205 retains 313 errors/105 warnings and
library206 retains 19 errors/111 warnings; primary error and warning
message/file/source-text multisets exactly match192/193. Final205/206 inputs are
identical and unchanged; MV200 differs only in four corrected Core files, with MV
and its dependencies unchanged. Static207 formatting, whitespace and codec checks
pass. No Core executable or Core runtime qualification exists. Prior VM186,
model135, journal129, replay117, SDK87 and repair57 remain separate evidence. No
MAIN composition, full formal/workspace/SDK or real-network pass is claimed.

```

## Canonical driver and Core runtime checkpoint 220–268

September 18 isolated work continues in `/private/tmp/iroha-sumeragi-native-20260916`;
MAIN source is unchanged. Exact earlier MAIN document preimages, source overrides,
patches, all-input manifests, compiler logs, immutable Core executable and selected
runtime receipts live under ignored
`dist/sumeragi-liveness-redesign-20260916/canonical-owner-checkpoint-267/`.
This checkpoint supersedes the earlier current compilation claim, without changing
its historical results or asserting a full release pass.

The ordinary driver now consumes one complete output owner across Network,
Pipeline and Time. World preparation includes late canonical World writes and
persistent baseline checks. Replay membership has explicit committed/predecessor/
staged ownership. ABI and gas policy come from the actual acquired or reverted
World overlay. Actual native source migration and complete non-World State/runtime
publication remain outstanding; both publication guards remain closed.

Core library249 passes; full unit-test compile264 passes with zero errors and168
warnings on 20,613 unchanged local inputs. Its executable SHA256 is
`29c4c389a0c48262706a0e0fbe3e5f059b9c6aa98c9417142d0be0c386e9d84b`.
Runtime265 passes 94 output controls and one Time identity control. Runtime266
passes all125 query controls and three of nine extra trigger scenarios; six stop
at old completion-event interleaving snapshots. Explicit snapshot corrections are
being checked against all retained balance assertions, including scenarios that
were unreachable after the first assertion failure. Torii library268 fails with
68 errors/422 warnings after 256.165 seconds and no source drift. These are stale consumers
of removed output/Time APIs, including duplicated positional proof construction;
the canonical APIs are being migrated without compatibility aliases.

Earlier253 passes 44 preparation/DA/membership/genesis/fraud/time controls;254's
World projection selection passes 13. Its output fixture deadlock and four observed
failures were corrected before259 passed92 output/Time controls. Query255 passes
118/125,260 passes 123/134, then262 repairs fixture Musubi baseline and preserves the
real finalized-prune refusal plus independent unfinalized-prune cleanup check.
These different source epochs are retained separately; none is promoted into a
full Core, full workspace, formal, MAIN or network qualification. The active goal
and all L1–L6 remain open.

Runtime272 subsequently passes all nine Time/Data trigger economics scenarios
using the unchanged264 executable and exactly eight explicitly reviewed runtime
JSON fixture changes. Each before/after event multiset, Data-event subsequence and
completion subsequence is identical; only the interleaving changes to reflect
completion publication after the whole root/DFS row is accepted.270 first passed
seven and reached two further old regular-success snapshots; those corrections
are separately retained in271. No UPDATE_EXPECT invocation or assertion removal
was used. All20,613 runtime inputs and the executable were unchanged during272.
Cross-crate source migration after this checkpoint has not inherited these passes.

## Authenticated history consumers 273–281

The isolated Torii migration replaces positional input/result/proof zips and
retired Time-entrypoint readers. Core now owns bounded exact-finality carrier
acquisition, complete source/output/cache validation and immutable read custody;
its State wrapper captures/rechecks only the canonical hash journal around I/O.
No mutable Kura handle or publication permission is exposed. SoraFS drops World
views before authentication and rechecks the captured prefix/membership. Contract
and VPN lookup use exact indexed carriers. Trigger history projects persisted
completions from Network/Pipeline/Time outputs, retaining internal call identity
without inventing a Network input index; reconstruction and its option are removed.

Review279 binds cached projections to the caller's exact cache-key end hash and
moves visibility checks onto an explicit per-request bounded authenticated-body
owner. Bool filter failures are retained and checked by consuming finish on every
response path, including early success. No ambient thread-local owner or raw-body
fallback is added. The three lazy whole-chain cache builders still need bounded
resumable construction: refusing an oversized complete scan must not become a
permanent retry, and a partial cache must not masquerade as complete.

Torii274 fails one root call-identity argument with425 warnings after131.171s;
275 corrects it and passes with zero errors/422 warnings after59.786s. Both capture
all20,615 local inputs without drift. The three new warnings were unreachable
patterns after Time retirement and are removed without suppression. Core library
also typechecks the shared reader. No new runtime or full Torii test pass follows
from this library result. Core276 is compiling the full unit-test target before
executing three new real-finality reader controls and the affected query suite.
Five new root Torii controls and four routing controls are authored; three existing
completion-history controls retain filtering/scan-limit/early-stop assertions with
canonical output fixtures and genuine three-of-four finality certificates.

Exact275 source overrides, full input manifests and compiler receipts, failed274,
root migration273 and fixture280 preimages are retained under ignored
`dist/sumeragi-liveness-redesign-20260916/canonical-history-checkpoint-281/`.
MAIN receives only the four status/plan/history documents; source integration,
native/complete State ownership, full workspace/formal/SDK and real-network
qualification remain open. No production publication guard is cleared.

## History reader runtime and test migration 282–288

Core276 fails three diagnostics from one new assertion comparing SignedBlockWire
wrappers, which have no equality/debug traits. Comparing their exact framed bytes
repairs the test without changing production. Full Core unit target282 then passes
with zero errors/168 warnings in 234.134 seconds. Runtime278 executes all 137 selected
controls (22 transaction, nine Network query, 97 query, one Time and eight Data
trigger tests) with zero failures. Every one of 20,615 captured inputs remains
unchanged and matches282; a separate COW inode retains executable SHA
`4520d6f3e2b137e97931bf68cdc646a20f0309d35b7943a9bb2500ac68273069`.
The new reader controls include actual pre-I/O budget refusals and damaged exact
stored wire despite a warm body cache. This is selected runtime evidence, not the
full 15,921-test Core suite, production publication or network qualification.

Torii's full unit-test target277 fails 165 errors/424 warnings in 532.982 seconds.
Root285 and routing286 migrate the obsolete header/result fixture surface to
complete typed outputs and actual input/output proofs. Existing assertions and
all 505 routing macro test names remain. Target287 then fails only one usize/u32
Network index mismatch, with 424 warnings, in 85.077 seconds. Both builds retain
20,615 unchanged inputs. The index conversion is corrected after this failed cut.
Read-only review also identifies positive push and exact-history fixtures that
lack the finality now required before projection; their genuine parent-linked
certificate setup is being corrected before the next build. No Torii test run or
successful full test-target compile is inferred from these diagnostics.

The cold-cache design review284 also rejects merely resuming full-history scans
under reset per-request budgets: retained rows, strings and indexes need their own
finite owner. The next design must use anchored pagination with truthful total
semantics or a separately bounded durable derived index. It remains unimplemented.
Evidence and the complete282 source overrides are retained under ignored
`dist/sumeragi-liveness-redesign-20260916/canonical-history-checkpoint-288/`.
MAIN receives only these four progress/plan documents. All previously listed
State/native/witness/resource/SCCP and release qualification gaps remain open.

## Torii history and push regressions 292–300

Full Torii test compilation292 passes (zero errors/424 warnings). Its ordinary
20,615-file capture omitted one ignored but compiled signer fixture; exact prebuild
290 bytes independently authenticate that supplement. Runtime289/293/295 explicitly
capture all20,616 inputs and pass42/46 controls. The four failures are two obsolete
exact refusal strings and two push tests that dropped their sole broadcast sender
before requesting worker shutdown. The expected refusals, cursor and queue effects
already occur. Corrections294/296 retain those assertions, retain the event sender
through shutdown, gate fixture helpers with cfg(test), and move the byte-identical
signer to nonignored finality_test_support.rs. Standalone history tests retain their
own checked structural fixture setter rather than depend on the app_api feature.

Full Torii unit-test target297 passes with zero errors/424 warnings in44.338seconds.
All20,616 standard captured inputs remain unchanged. Runtime298 passes40selected
history, status/details/alias, completion, proof-delivery, explorer and native custody
controls; runtime299 passes all6push controls including missing-finality refusal,
durable backlog/gap, full-queue recovery, indivisible capacity and persistence repair.
Both use the exact297 executable SHA
`768829c24521f7568f25d0e27a3fc085d54166e3268cb18941a49c1b1db37d47`
and unchanged standard manifests. This is46selected passes, not all5,396Torii tests.
The full297 source overrides, runtime logs, executable and correction evidence are
under ignored dist/sumeragi-liveness-redesign-20260916/canonical-history-checkpoint-300.
No new State/native publication, formal, SDK, MAIN or network qualification is claimed.
The next implementation resumes sole native output ownership and actual MV runtime
predecessors; the broader goal and production publication guards remain open/closed
respectively.

## Native execution and snapshot recovery 301–360

The isolated301–319 native/common driver uses the actual carrier header and
verified native inputs, then one Network/Pipeline/Time output owner. Runtime and
lane-context current/predecessor state use actual MV cells. Capture321 acquires
one finite generation-coherent view and returns a typed busy/changed outcome;
encoding remains subject to the explicit unfinished allocation-bound TODO.
Snapshot323/330 preserves both maps for the five previously current-only special
stores, including deleted preimages, absence tombstones and manifest identities.

Full Core/MV test compilation337 passes: zero errors,174warnings,265.985seconds,
20,625 unchanged standard source/config inputs. All56MV tests338 pass. Core338
never executes tests because its module filter also matches unrelated query and
receiver snapshot modules. Corrected341 passes all192 selected native/runtime
checks. The94-test snapshot module passes its first9, then aborts at
can_read_multiple_blocks with stack overflow;85 tests do not pass/complete.
LLDB345 identifies canonical_world_field_order constructing/serializing a default
World inside the reader stack. Static derive-provided field order347 replaces
that runtime construction without changing emitted JSON bytes.

The successor requires fourteen SoraFS/SoraDNS MV envelopes342/346 previously
defaulted on restore, with exact current/undo and both-cut directory validation.
Derived UAID bindings331, account identities352, recipient/confidential-policy
indexes343 and trigger active IDs344 retain their predecessor. Native observation
fence335 discards either a result or an error only when the State generation
changes; stable invalid sources retain their deterministic rejection.

Compilation349 reports10 test-only errors on20,633 unchanged inputs: eight plain
JSON calls for binary-key maps and two nonexistent World transaction calls.353
corrects these through the exact snapshot codec and actual World transaction API.
Compilation355 reports one missing AccountDetails test import on20,634 unchanged
inputs;358 corrects it.359 passes full Core/MV unit-test compilation with zero
errors,175 warnings in209.1169 seconds on20,634 unchanged inputs. Norito350 finds
the new test is absent from the grouped harness;357 registers it.361 then passes
191 grouped codec tests and60 derive unit tests, with no changed inputs.

Core362 runs319 selected tests in154 processes:301 pass, fourteen snapshot tests
abort with stack overflow and four SCCP snapshot tests fail before their intended
assertions. All new service-state and predecessor-index tests pass, as do the192
native/runtime regressions. The executable and20,634 source inputs remain unchanged.
LLDB363 confirms static field metadata removed the earlier defaultWorld discovery,
but nested by-value State frames still exhaust the normal stack: the snapshot-map,
bundle and initializer frames occupy approximately614,718 and237 KiB respectively.
The isolated successor365 keeps restored State boxed through decode, validation,
replay and daemon handoff. No enlarged-stack workaround is added. The four SCCP
failures share fixture headers with no transaction Merkle root;364 fixes three
constructors and asserts immutable proposal identity across output installation.
Build367 reports three missed boxed test consumers. Corrected370 passes full
Core/MV unit-test compilation in117.4023 seconds, with zero errors and175 warnings
on20,634 unchanged inputs. Runtime372 then passes345/346 selected tests in181
processes, including all94 snapshot cases and all192 native/runtime controls.
There are no stack overrides, changed source inputs or altered runtime executables.
Its sole failure is the three-height SCCP fixture retaining height-zero runtime
undo.374 changes the structural fixture to publish each metadata height separately
and asserts its actual height-two predecessor. These samples do not claim block
execution; retained finality artifacts and corruption checks remain unchanged.

Daemon373 fails with35 diagnostics (28 unique) and439 warnings in309.4834 seconds
on the same unchanged inputs. Its output consumers still use removed flat-result
APIs and retired header parameters.375/376 migrate them to explicit typed Network
joins, preserving finality and immutable proposal checks, and add mixed internal
output controls.371 adds five account-scope predecessor tests and shares the live
entry derivation. Frozen377 has20,636 inputs;378 reports three Vec/VecDeque method
mismatches in the revised fixture.380 corrects those calls and removes one redundant
proposal validation already performed by the output-cache validator. Successor
Core/MV381 and daemon382 qualification is pending at this checkpoint. No descendant
result is inferred from370/372.
The codec guard and original pending-membership ledger agreement356 pass on354;
this is neither full formal nor full codec qualification.

Read-only audit340 records remaining predecessor gaps in account scope, ownership,
contract/asset alias, custody/game/VPN and other reverse indexes. Its separate
trigger clarification corrects an initial overstatement: active-ID lookup affects
queries and projection roots; Time/Pipeline matching uses typed actions directly.
The initial packet and correction are retained without erasing the original.

Evidence/source capsules are under ignored
`dist/sumeragi-liveness-redesign-20260916/canonical-snapshot-checkpoint-360/`.
MAIN HEAD, MERGE_HEAD and exact staged patch remain unchanged; only the four
previously owned status/plan/history documents differ from baseline220. No new
MAIN source integration, full State publication, full Core/workspace/formal/SDK
or real four/seven-validator qualification is claimed. All L1–L6 stay open.


## 2026-09-18: retained authority and replacement lookup (381–402)

Core/MV381 passes compilation in159.425856 seconds with zero errors and177 warnings
on20,636 unchanged inputs. Runtime383 completes191 processes and356 selected tests:
355 pass; the old account-scope catalog-pruning fixture fails before its target
logic because ordinary set_nexus correctly refuses to replace physical dataspaces.
All94 snapshot cases, all192 native/runtime cases and all five new account-scope
predecessor tests pass. There is no stack override, changed input or executable.
388 seeds canonical accounts, uses authenticated pre-genesis configuration, asserts
ordinary-setter rejection and preserves the original pruning checks.

Daemon382 passes its binary-target build in607.059795 seconds (zero errors,
541 warnings), but390 verifies that the thin launcher lists zero tests. This is
not daemon runtime qualification. The corrected library build392 fails in19.795832
seconds with one ExecutionStep-default error and440 warnings on unchanged inputs;
393 constructs the exact structural instruction list. No daemon tests ran.

386 derives domain owner, NFT owner/domain and RWA owner/status/frozen indexes from
one authoritative current/undo history per source, including redundant touched
buckets; three new tests retain replacement and second-restore cases.391 rejects
configured physical dataspace changes against retained snapshot or committed
State even when no protected runtime additions exist. Fresh H0 configuration and
description-only metadata remain admissible; three new tests cover both boundaries.

389 deletes the independently mutable process-local fee-settlement set. Canonical
World receipt and settlement markers remain the sole duplicate/replay authority;
block-local stage facts still prohibit partial fixture publication. Existing tests
inspect those canonical markers. Its new marker-only control covers snapshot undo,
discarded/committed replacement and republishing; it grants no economic or finality
qualification. A separate recovery completeness audit remains open.

397 corrects TransactionsBlock::get, which ignored replacement mode and exposed
abandoned-tip identities after other State owners had reverted. Ordinary views,
replacement lookup and membership-transition point reads share one logical-cut
function. The new test compares point reads with actual predecessor and staged
projections, including removal, historical shadowing, discard and commit.

Frozen394 has20,638 inputs. Core395 fails in64.094457 seconds with one unstable
anonymous-lifetime error and127 warnings;398 restores a stable named lifetime.
These new changes remain unqualified at this checkpoint. MAIN merge/index and
all non-document inputs remain unchanged. The complete State publication owner,
remaining derived histories, allocation bounds, witness ownership, SCCP outbox,
runner cutover and unchanged four/seven-validator acceptance remain unfinished.


## 2026-09-18: replacement controls and daemon runtime (399–407)

Frozen399 has20,638 unchanged inputs. Core/MV400 compiles in213.125269 seconds
(zero errors,175 warnings); actual daemon library401 compiles in354.953817 seconds
(zero errors,439 warnings). Core403 passes all389 selected tests in194 processes,
zero ignored, with the owned executable and source unchanged. This includes all94
snapshot tests on normal stacks, all192 native/runtime controls, ownership386,
configured authority391, fee-marker snapshot/replacement389 and all26 transaction
membership controls including397. It resolves the earlier configuration-fixture
failure without weakening the ordinary setter.

Daemon407 runs32 selected actual library tests:23 pass, nine Musubi cases fail
initial genesis admission before their intended finality assertions. The pure
Network/Pipeline/Time join, signed provider capture, provider Network-only
observation, selected fee-relay and Soracloud controls pass. The selection did not
match nested runtime-dependency or authoritative-execution module names; those
consumers remain unexecuted until explicitly selected. No failure is waived.

405 extends read-only execution completeness to settlement and source receipt
markers derived from the committed transcript, independent of current fee mode.
Emission remains owned by actual settlement; the expected-set helper cannot mint
markers. State and Kura snapshot-release proof consumers share the complete set.
406 rebuilds asset/contract alias lookups from current and retained prior sources,
validating leases, uniqueness and exact-cut definition/domain references before
assignment. Five controls cover live writes, dropped/committed replacement,
second restore, invalid priors and valid removed dependencies. These descendants
await compilation and execution; no MAIN source integration is claimed.


## 2026-09-18: complete fee and alias recovery controls (409–422)

409 freezes only the six owned Core405/406 paths over exact399; it explicitly
excludes in-progress daemon408. Core/MV410 compiles in182.834508 seconds with zero
errors and176 warnings on20,642 unchanged inputs. Recovery414 passes143/143 exact
processes, zero ignored, including ten new fee/alias regressions, all94 snapshots,
five Kura release consumers and existing alias/fee tests. Sources and the private
executable remain unchanged. The one added lifetime warning is addressed later by
417, whose helper accepts the whole execution batch; that edit is not qualified
by410/414. Daemon412 separately passes all11 previously omitted startup and
authoritative-execution tests on399.

Musubi408 investigation reaches a substantive dependency after correcting initial
admission: actual authenticated genesis publication is still refused by the output
owner. Output attachment is sealed, but complete State publication remains
unimplemented. No guard is cleared, no stateful positive is converted to an
expected failure, and no placeholder execution root is accepted as execution
proof. The pending read-only test-support projection rejoins actual witness,
source inventory, exact sealed wire and canonical manifests through the production
commitment calculation. It cannot authorize publication. This work remains
unqualified until its build and runtime checks complete.

Extensions416 and421 retain the closed builds, executions, sources and failures
under the ignored checkpoint360 directory; all copied hashes were verified. MAIN
merge/index and non-document inputs remain unchanged. Full State publication,
remaining derived histories, allocation bounds, witness ownership, SCCP outbox,
runner cutover and unchanged four/seven-validator acceptance remain outstanding.


## 2026-09-18: consuming preparation and membership (423–460)

Carrier metadata and final World preparation now execute before candidate voting,
retaining the actual source inventory, witness, sealed output and frozen context.
Ordinary merge metadata follows authoritative current/undo World cells instead of
rolling-cache contents. Replacement regression449 stores its actual parent blocks
before Kura rewind. Canonical runtime444 rejects derived lane-configuration drift;
its cache-independence and actual-history fixtures preserve both MV versions.

Core/MV451 compiles exact450 (20,649 local inputs). Runtime452 completes263 tests:
202 pass and61 fail. The original process disappears after48 completed tests;
a missing process handle and an empty process census establish termination. The
remaining215 tests resume once against the same source and private executable;
final source/executable hashes are unchanged. All six targeted444 controls and
replacement449 pass. Broader failures retain their logs and assertions, including
superseded cache-based fixtures, missing historical runtime and Kura evidence.
No failure is waived or converted into an expected failure.

Membership454 adds a move-only admitted transition holding the original writer.
State integration453 admits it before geometry and publishes it once without
reloading or revalidating membership. The unreachable second membership-failure
rollback branch is removed. Candidate453 also retains admitted event delivery and
tiered persistence plans; delivery/publication guards remain closed. Core/MV456
compiles frozen455 (20,651 inputs), zero errors and176 warnings. Runtime457 passes
160/160 selected actual candidate, membership, World, execution-prefix, tiered and
snapshot controls, zero ignored, with all inputs and the private binary unchanged.
One new unused backend-copy warning is removed in later source, not qualified by456.
The tiered plan in455 still inherits synchronous live-value reads and pending
worker overwrite;459 addresses those separately and has not been qualified here.

Daemon448 compiles exact446. Earlier daemon415 passes34/43: nine repaired real
genesis Musubi fixtures reach the unfinished output-publication guard. This is a
remaining implementation dependency, not a cryptographic fixture waiver.
The sole pending-membership formal regression passes on438; full formal structural
qualification remains open. New receipts and source captures remain under the
local `/private/tmp/iroha-sumeragi-redesign-checkpoint` workspace; no new portable
archive extension or MAIN source integration is claimed for this interval.

The publication contract now explicitly retains existing wire commitments and
uses actual journal ownership plus exact QC/durability authorization. A new full
State Merkle root is not a prerequisite. Full consuming Apply, native source
cutover, archive/geometry/resource admission, Queue-veto removal, State-owned read
capture, SCCP outbox and unchanged four/seven-validator acceptance remain required.


## Prepared archives and retained journals 461–477

On 2026-09-18, Core/MV462 compiles exact461; runtime463 passes384/431 with47 failures, zero ignored tests and unchanged source/executable. Twenty-four previous452 failures pass, with no new failure on shared selectors. This does not qualify all Core tests.

Root464 removes only the redundant generation rejection inside the owned native kernel; original source/context/predecessor checks and unowned-source observation fences remain. Core/MV466 compiles exact465; runtime467 passes56/63. The new economic manifest-cache-refresh regression passes. Five malformed-catalog controls require valid lane identity fixture rows (468); the native preflight fixture still uses a resultless admission carrier, and the tiered reference fixture lacks a storage directory. Corrections to those last two are source-only472.

Frozen469 contains the five identity-fixture fixes and prepared provider/reputation archive plans. Core/MV470 compiles in249.175 seconds, zero errors/176 warnings, on20,653 unchanged inputs. Runtime471 passes118/118: both archive unit suites plus the five lifecycle identity controls. Original archive writers reserve predecessor/capacity; publication authenticates exact durable receipts and writes/readbacks the retained bytes. Reputation retries preserve once-only policy accounting after partial I/O. Codec guard passes. Existing production Apply still calls archive capture after finality; this result does not qualify pre-decision resource ownership or retry scheduling.

Later source474 also contains ordered snapshot fixture journals, the two remaining467 fixture corrections, and root473 carrier journal capture. The latter retains original State journals, membership admission, witness/source seals, events, recovery hash, tiered payload and archive reservations with no mutable StateBlock escape or publication API. Build475 is running; none of those later changes is qualified by471. Geometry admission, complete consuming Apply and native runner cutover remain open.

Receipts and source captures for this interval are local under `/private/tmp/iroha-sumeragi-redesign-checkpoint`; no portable archive extension or MAIN source integration is claimed. MAIN's merge and staged diff remain preserved. All liveness goals and full unchanged-candidate network/formal/workspace acceptance remain open.


## Captured geometry and admission fixtures 478–494

All-route and secondary admission fixtures now finish explicit empty typed results after their final admission controls, sign that exact proposal and retain its actual zero-work runtime sample. Core/MV480 compiled on20,655 unchanged inputs. Runtime481 passed29/33; three failures were the secondary helper's stale resultless carriers and one was a missing reputation policy in the positive journal/archive fixture. Those setup failures did not establish a consensus or execution qualification.

Frozen483 adds captured geometry ownership and bounded prepared Kura journal phases. State capture binds actual MV predecessor/successor, protected runtime catalogs, exact transition plan/header, manifest policy and certified drain frontier without consulting physical caches. The Kura phase owner retains one canonical encoding, bounded byte differences proven by comparing complete encodings, and exact file operations; aggregate retained allocation is checked, and persistence does not allocate another full journal. Existing retirement/storage locks and admission remain; this owner cannot cross canonical persistence yet.

Core/MV484 compiles in211.428 seconds with zero errors/178 warnings on20,659 unchanged inputs. Runtime485 passes179/185, zero ignored, with unchanged inputs and executable. All four captured-geometry controls, four prepared-journal controls, admission/body/leader-timeout cases and stale-AXT fixture pass. The six failures are five existing geometry restart fixtures without their required initial primary storage anchor and the positive journal/archive fixture without its orderbook policy. Source488 installs exact primary anchors at those five fixture starts, before writes; source490 activates the actual reputation/orderbook/reserve policies, permissions and reserve account/asset through signed genesis. Source486 finishes autoscale DA fixtures after attachments invalidate prior results, and source487 installs malformed/future lane fixtures in their canonical runtime owner. Assertions remain intact.

Frozen491 combines these corrections; Core/MV492 is compiling. Runtime493 will include the prior431-case regression, all185 geometry/journal/admission cases and current autoscale/lifecycle/setter/snapshot controls. No result from485 qualifies these later corrections. The complete-input pending-membership formal ledger check and retired-codec guard pass on491. Full State publication, aggregate resource admission, native runner construction, complete formal binding and real four/seven-validator qualification remain open. In particular the retirement scanner still repairs, compacts and syncs evidence; candidate admission needs a pure bounded observation plus a reservation respected by every cross-route writer. MAIN source integration has not expanded, and this local evidence does not extend the portable source archive.


## Retirement maintenance and Native observation 495–507

Core/MV492 passed in132.284 seconds, zero errors/178 warnings, on20,659 unchanged inputs. Its runtime493 finished634/651, zero ignored and no input/executable drift. The five corrected initial-primary restart anchors all passed. Seventeen failures remained: seven in archive policy time, actual runtime predecessor/reset lineage, malformed-source admission, initial rollback geometry anchor, random signer comparison and authenticated future-lane merge setup; ten in configured-dataspace setter fixtures.

Frozen498 contains the exact source corrections495–497 and501 plus the geometry admission design record. Core/MV499 passed in118.999 seconds with zero errors/178 warnings and20,660 unchanged inputs. Focused500 passed196/204, zero ignored, unchanged inputs/executable. It includes every493 failure, the complete current autoscale and Nexus-setter prefixes and all carrier-preparation controls. All seven earlier non-setter failures and the routing/staking negative controls now pass. The remaining eight setters fail at initial physical configured-catalog publication; explicit State startup authority alone does not replace Kura's authenticated configured baseline. Source507 is correcting those constructions without bypassing the baseline guard or dropping assertions.

Sources502/503 split existing per-route retirement maintenance from observation at the original lock-scoped call site, preserving certified recovery, authenticated frontier compaction and seven-pair repair ordering. Native evidence has a bounded pure observer and a consuming explicit durability attester. The observer retains immutable decoded maps, file metadata/hash identities and one directory handle while borrowing the caller's inventory; it neither repairs nor syncs. Attestation checks the exact captured files and complete namespace before returning durable records. Independent source review identified and corrected mutable decoded-map exposure and duplicate retained inventory allocations. Five new Native controls cover drop/exact bytes, byte substitution, sibling namespace changes, temporaries and retention refusal; the maintenance control covers interrupted recovery ordering and retry. Frozen504 is compiling in505; no runtime qualification is yet claimed for these production changes.

The remaining historical/autonomous readers still sync, and the whole retirement scanner is not pure. Cross-route reservations, aggregate resource admission, exact consuming State publication, native runner construction, full formal bindings and real four/seven-validator fault/restart/final-transaction qualification remain open. Local Queue/resource unavailability must retain typed retry ownership and a wakeup; it must not become a durable invalid-body verdict. MAIN source integration and the portable source archive have not expanded.


Core/MV505 subsequently passed in149.435 seconds, zero errors/178 warnings on20,663 unchanged inputs. All131 prior geometry tests and all five new Native observation controls passed in506 (136/137, zero ignored, no source/executable drift). The new maintenance test's recovery-order and rejection assertions passed, but retry correctly rejected its arbitrary orphan temporary as terminal-history corruption. Source508b replaces that fixture data with the exact authenticated application receipt data/index pair, adding index preservation/removal assertions; no production recovery guard changed.

Frozen508 includes configured-startup fixture construction507 and the authenticated interruption fixture508b. Core/MV509 passed in121.947 seconds with zero errors/178 warnings on20,663 unchanged inputs. Runtime510 passed all61 selected controls in86.377 seconds, zero ignored and no input/executable drift. Every remaining500 failure, all Nexus setter controls, the maintenance ordering/retry test, existing recovery controls and five Native observation tests pass on that exact executable. The retired-codec guard also passes on508. These focused results do not replace full Core/workspace/network qualification. The corresponding formal ledger/owner contract update508a remains a separate pending source capture; it was not part of Rust509/508.


## Combined regression and historical observation 511–521

Runtime511 completes657/657 selected tests in835.992 seconds on the unchanged509 executable and508 source (20,663 inputs), zero ignored and no input/executable drift. This union includes all493 selectors plus current geometry, autoscale/lifecycle/setter/snapshot and final510 fixes. It does not qualify later514 changes, a full Core/workspace run, or network liveness.

Formal508a separately passes31 geometry ledger/semantic mutation controls. Formal-only overlay512 captures its exact six files. Source-inventory513 audits three existing omitted providers: block/carrier_preparation.rs, block/output_event_tests.rs and smartcontracts/ivm/host/shared_vm_cycle_budget_tests.rs. Its15 inventory tests pass on a44-file indexed copy whose bytes match Shared, and all31 geometry controls pass again. The strict stage-zero guard remains: untracked Shared providers must enter a coherent tracked candidate for the direct live-source gate. No MAIN index changes or broad formal/TLC/Apalache success are claimed.

Later514 extracts bounded historical recovery observation from explicit durability attestation, retains the exact canonical read identity and all existing retained-finality/lane bindings, and adds six actual signed-carrier regressions. It binds nested names/count/bytes/file identities and absence, rejects substitutions before returning attested records, and preserves temporaries for maintenance. Independent review finds no blocker. Compile517 found one missing PathBuf borrow in a test helper;519 corrects that borrow without production changes and520 is rebuilding. Runtime521 is planned for all current geometry, historical and incarnation-ABA controls; no later qualification is claimed yet. Full pure retirement, cross-route/capacity reservation, consuming State publication, native production cutover and unchanged four/seven-validator acceptance remain open. MAIN source integration and the portable source archive have not expanded.


## Historical observation qualification and storage design 522–529

Core/MV520 compiled519 in166.743 seconds, zero errors/179 warnings. Runtime521 passed173/179 controls; all six new cases failed during fixture setup because its noncanonical temporary-directory spelling differed from the canonical Kura root. The strict path guard was correct. Source523 derives the fixture path from Kura's actual root and asserts the real finality receipt; no production guard or assertion was weakened. Core/MV524 compiles523 in77.115 seconds, zero errors/178 warnings, on20,664 unchanged inputs. Runtime525 passes all six new cases in7.573 seconds, zero ignored and no source/executable drift. Together these are scoped coverage, not a single full-suite run or qualification of MAIN.

Historical formal contract515 passes32 positive/mutation controls;31 Native controls also pass. Full structural522 reports321 diagnostic occurrences/144 unique on an exact indexed mirror. Inventory526 adds four audited existing Kura test/helper include providers and passes20 checks. Its full structural run still fails with168 diagnostics/167 unique in56.163 seconds; zero inventory/digest failures remain. Removing repeated inventory refusals exposes24 previously unreachable checks. No semantic obligation was removed or passing formal/TLC/Apalache claim made. Workspace formatter528 completes successfully with exactly three formatting-only Rust regions; the524/525 executable predates those formatting edits.

Independent/root audit527 selects stable canonical storage and immutable lane-incarnation namespaces. Spec529 replaces the proposed consensus-spanning evidence freeze with authenticated catalog/reference publication and deferred physical GC. Current production still moves alias-based paths and retains the post-finality Queue veto. Pending cross-route participant work is real ownership: bytes alone cannot authorize execution after retirement. Native canonical-carrier repair has exact finality/manifest/WSV authority, but preflight, ordinary predecessor, publication, readback, cleanup and restart discovery must all retain the original instance; fresh admission remains active-only. Exact historical work authority, bounded capacity, snapshot/undo/recovery pins, GC deletion fences and the complete consuming State publisher are required before the old guards can be removed.

Evidence remains local under `/private/tmp/iroha-sumeragi-redesign-checkpoint`, including historical-observation-514, source-inventory-526, immutable-lane-storage-audit-527, format-528 and immutable-storage-design-529. MAIN source integration and the portable source archive have not expanded. Full formal/workspace/native production and unchanged four/seven-validator fault/restart/final-transaction qualification remain open; the liveness goal is active.


## Publication resource audit and formal ownership 530–544

Formal531 replaces stale transaction membership tokens with the actual consuming preparation/publish owners, including the original writer and exact predecessor. The Rust item parser now recognizes lifetime-parameterized impls without accepting lookalike self types. All38 positive/mutation controls pass. Its full structural gate still fails with164 diagnostics (four prior diagnostics removed, zero added). Formal532 binds delegated State frontier, successor and replay checks to their actual owners, preserving each former obligation. All28 new controls and38 membership controls pass together (66 total); the full structural gate still fails with140 diagnostics (24 removed, zero added). Both use exact indexed source mirrors; neither is a full formal pass or MAIN qualification.

Prototype530 retained exact Native route/ancestor directory handles through publication phases. Frozen533 compiled in534 (318.052 seconds, zero errors/178 warnings). Runtime535 completed160 controls:158 passed, zero ignored; two old merge receipt fixtures failed before their assertions because the carrier duplicated externally certified content. Inputs and executable remained unchanged. Independent review found a separate descriptor regression:530 retained a full ancestor chain for each of up to1,024 routes only after finality, with no pre-vote resource admission. Shared ancestors reduce duplication but cannot reserve leaf handles. Reopening from cached device/inode identity loses exact-object ABA protection. Withdrawal537 restores precisely the eight530 beforeimages; the prototype and its diagnostic evidence remain preserved locally. It is not an accepted liveness fix.

Read-only audit536 traces the missing handoff: PreparedCarrier computes the complete Native manifest before voting, but validate_candidate returns only its execution commitment and drops the owner. BodyStore cached/reproposal validation can bypass preparation entirely. The production worker, validation marker, retry, restart and Apply consumers must retain or rejoin exact resource ownership. The current local Queue veto becomes a durable invalid-body verdict; merely adding a new error string, dropping the veto, or attaching a reservation to a discarded object cannot close this. Spec543 records these concrete integration requirements; the full consuming publisher remains open.

Fixture538 builds an explicit reference-only merge carrier while retaining complete certified sources/results, then checks the production manifest join. Core/MV540 compiles frozen539 in305.645 seconds, zero errors/178 warnings on20,672 unchanged inputs. Focused541 passes both formerly failing receipt controls and the unfinalized restart caller (3/3, zero ignored; no source/executable drift). Source544 consolidates the equivalent multi-lane fixture and fixes the separate compaction factory to attach its merge reference before output finalization and signing. Every recovery/corruption assertion is retained. Core/MV546 then compiles frozen545 in91.577 seconds, zero errors/178 warnings on20,672 unchanged inputs. Runtime547 passes all176 selected controls, zero ignored and no source/executable drift. This includes all current Native publication tests, ordinary receipt/frontier/predecessor durability and every test in the terminal/capacity/compaction fixture owners; all completed-repair temporary crash cuts pass. The eight542 formal overlays are not included in this Rust source capture.

All evidence remains local under `/private/tmp/iroha-sumeragi-redesign-checkpoint`. No MAIN source integration, staged change, commit, portable archive extension, full workspace/formal pass or real four/seven-validator qualification is claimed. The active liveness goal remains incomplete.


## Carrier-scoped Native evidence design 549

Independent and root read-only review confirms that per-route Native manifest/receipt artifacts contain deterministic projections of canonical execution and verified finality, not independently produced application facts. The current State snapshot can nevertheless block a producer on a missing derived per-route file after the matching World frontier has applied. Select one immutable carrier-scoped authenticated proof bundle in the stable canonical namespace, plus a bounded complete route/incarnation history catalog. This refines529; it is not implemented or runtime-qualified.

A reference-only design requiring permanent full-body retention does not preserve the existing authenticated hash-only bootstrap and Native evidence retention behavior. The selected bundle must be constructed and completely authenticated before releasing the exact result-bearing carrier and, for compact autonomous carriers, its associated MergeLedgerEntry. Preserve every canonical leaf/proof/count/wire/source/result/proposal/settlement check, complete Native history across ordinary interleaving and exact global finality plus checkpoint/commit-manifest application join. Finalized-before-WSV remains pending. Pending unmerged cross-route participant work retains separate source/terminal authority and the active signing fence; a bundle cannot derive a nonexistent application carrier.

One slow or retired route pins the entire shared carrier bundle. Account its full bytes once, plus all temporary and predecessor/successor peaks, until the last exact route/snapshot/undo/recovery owner releases it. GC, complete catalog discovery, omitted/duplicate rows, same-height replacement and historical incarnation admission remain mandatory. Original per-route inode identity is local mutation protection, not protocol authority; the new immutable store enforces integrity at its own bounded boundary. Current physical guards remain until the complete first-release representation and consuming publication path replace them. Evidence is in immutable-lane-storage-audit-527/canonical-native-evidence-alternatives.md and carrier-evidence-design-549 under the local checkpoint root; no new production Rust or compatibility path was added.


## Native source delegation542/550 and canonical storage552/556

Native preparation542 binds the actual PreparedCarrier and capacity-reservation delegates without claiming a consuming publisher. Its43 new controls pass. The broader existing indexed run ended99 passed,7 failed and58 setup errors, exposing real stale fixture delegation; these are retained failures, not attributed solely to untracked source providers. Structural542 reduces140 to115 diagnostics,25 removed and zero added.

Fixture550 follows the exact HistoricalRecovery macro dispatch to its helper and the retention→genesis→archival constructor chain. All original corridor/committee/State/Native obligations remain at their actual owners, with argument and branch substitution controls. The focused28 controls pass; the broader affected selection passes187,22 unrelated deselected, in81 seconds. Structural550 reduces115 to94 diagnostics,21 removed and zero added, on20,675 unchanged indexed inputs. It remains a failing full gate.

Canonical storage552/556 implements a first isolated storage split: fixed blocks/canonical and merge_ledger/canonical.log own chain bodies and merge associations; the primary lane owns a separate sidecar pair. Alias moves no longer flush or retarget canonical handles. Existing geometry/drain/Queue guards remain; immutable incarnation/reference publication and carrier-scoped Native bundles are still open. Startup preflight refuses a missing bound canonical namespace, preserves authenticated stage recovery before component-file requirements, and retains independent lane/canonical identity barriers. Geometry bindings cannot own canonical paths or their ancestors/descendants. Existing block/merge budget roots count canonical bytes exactly once. Fresh-start provisioning, snapshot/Kagami fixtures and Taira hash readers are aligned; three local native-tool projection tests pass, including no alias fallback. Production deployment was not touched.

Compile554 exposed one missing test-path borrow and is retained as a failed diagnostic. Corrected557 compiled successfully558. Full Kura559 completed1,338 cases:1,322 passed,15 failed and one preexisting explicitly ignored optimized measurement. Namespace, inode, symlink/hardlink and relabel controls passed. Fixture564/565/567 repairs finality setup, final-context output signing, cold tampered-byte reads and actual storage-only replacement setup without weakening production guards. Bounded preflight563 removes canonical-owner whole-tree rescans and retains both parent identities. Frozen569 compiled successfully570 with zero errors. Focused572 passed31 controls and failed one obsolete combined-storage error assertion;576 requires exact missing lane-marker refusal, unchanged canonical bytes and no invented marker. The combined test-network/canonical reader577 is compiling578. No full-Kura green run is claimed yet. Main source, merge and staged changes remain unchanged; evidence is local under the existing checkpoint root. No full-workspace, full-formal or unchanged four/seven-validator qualification is claimed.


Merge-validation560 retains all12 original obligations at validate_merge_execution_batch_with_replay and requires exact wrapper delegation. Its52 controls pass; the indexed full gate decreases94→82 with12 removed and zero added. Structural575 reruns the complete frozen569 storage candidate using a private review index:82 diagnostics, zero delta,20,676 unchanged inputs and unchanged workspace index. The initial575 setup assertion accidentally counted two gitlinks as source files; it stopped before the gate, was corrected to the same regular-file inventory, and is retained as a setup diagnostic.

Kagami562 terminated with51 compile errors from removed result/query APIs (including repeated inclusion of one fixture provider). Typed-genesis/localnet568 preserves complete Network joins, all-output success and fragment coverage; it is source-only. Native scaling574 replaces obsolete external-empty merge-carrier query fixtures with current Native Decisions and typed Network proofs, including a bounded read-only anchored context-witness owner. That migration is incomplete and unqualified. Test-network571 stops taking the maximum arbitrary lane hash journal height; it uses only fixed canonical hashes and rejects partial records, while Torii applied status remains the readiness barrier. No live deployment was modified.


## Canonical storage and current consumer qualification (563–598)

Bounded canonical preflight563 and fixture564/565/567/576 repairs pass all32 focused controls582. Frozen583 compiles584; complete Kura585 passes1,339 cases with zero failures and one preexisting performance-only measurement ignored. All20,676 source inputs and the cloned executable remain unchanged. This qualifies current fixed-canonical Kura behavior, not immutable lane instances, prepared State publication, full Core/workspace execution or real-network liveness.

Typed test-network consumers581 compile584. Runtime586 passes17 and fails20 due to zero/one-validator genesis fixtures. Fixture587 uses an actual four-validator BLS/PoP committee, retains the custom staking peer, and verifies exact registered membership and PoPs; production committee guards are unchanged. Compile590 passes. Runtime591 passes36 and fails stale-signature metadata mutation. Builder592 signs the exact changed resultless proposal before strict Core preexecution and replaces the caller block only after execution succeeds. Direct strict stale-signature rejection remains tested. Compile594 passes; runtime595 passes36 and reaches invalid empty DA policy in that fixture. Correction596 uses the actual configured bundle and preserves negative, metadata and complete-output assertions. Integrated597/598 qualification is pending.

Capacity579 passes55 controls and reduces full structural diagnostics82→76, six removed and none added. Native recovery588 binds actual complete-history selection, all three pending-repair abstentions, exact route/incarnation/height checks and storage-error rejection before retained-work mutation. All21 scoped controls pass, including11 new mutations; full structural76→66, ten removed and none added,20,676 unchanged inputs and private index. The gate remains failing; these structural controls do not prove runtime liveness.

Agent574 releases15 paths for bounded read-only Native evidence and Kagami verifier/export/launcher/filesystem consumers. It retains anchored global finality, complete context-write proofs, immutable first admission, previous complete-set membership, exact Native Decisions and shared RS16 calculation. It exports no live signing/current-set/State publication capability. One-lane/eight-request fixtures require eight execution carriers; four-lane fixtures require two. No retired merged-Network proof fallback remains. Targeted format and codec checks pass. Frozen597 combines568/574,580,588 and596; compilation598/runtime remain pending. Immutable storage implementation603 starts from the connected startup/identity/reference/recovery boundary while preserving current drain, cross-route and publication guards.

MAIN advanced externally while these records were prepared. The four task document hashes stayed identical to561; packet599 captures the current HEAD, merge state and staged index immediately before editing and preserves them. No MAIN source, commit, merge or index was changed by this task. Current baseline: a33dad42ed5d8eb296db9d5f4b2c57b051685cf6.

Prior current-view text is retained verbatim:

Isolated runtime511 passes657 controls; historical observation524/525 compiles and passes all six new controls. Membership/delegated-State formal531/532 passes66 controls. Native preparation542 adds43 passing controls; fixture delegation550 passes187 affected controls. Merge-validation560 adds52 passing controls. Full structural575 on the current storage candidate still reports82 diagnostics, unchanged from560; full formal remains failing. The retained-directory prototype530 compiled, but was withdrawn after review exposed post-finality descriptor demand without pre-vote admission;535 passed158/160, with two unrelated duplicated-content fixture failures. The complete canonical terminal/compaction fixture correction compiles in546 and passes all176 selected Native, receipt, capacity, compaction and restart controls in547, zero ignored and no drift. The selected storage design uses stable canonical/immutable incarnation storage and one authenticated Native proof bundle per carrier, with bounded history references; The isolated canonical storage split552/556 and bounded preflight563 compile in570. Kura559 passed1,322 with15 fixture failures and one explicitly ignored measurement; repaired572 passed31 of32 focused controls, with one obsolete error-path assertion corrected in576 and awaiting578. immutable incarnation/reference publication, historical work authority, exact prepared-resource handoff, consuming State publication and native production integration remain unfinished. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#publication-resource-audit-and-formal-ownership-530544) does not qualify MAIN/full workspace/formal/SDK or four/seven-validator liveness. All L1–L6 remain open.


Formal531/532 passes66 controls; Native preparation542 adds43 and fixture delegation550 passes187 affected controls. Merge-validation560 adds52 passing controls; structural575 remains failing with82 diagnostics. Finish qualification of the compiled isolated canonical storage split552/556/563 and current-layout consumer repairs; lane incarnation/reference publication remains open.

Canonical storage552/556 separates chain files from lane alias moves; bounded preflight563 avoids unrelated full-tree rescans while retaining exact namespace identities. Compilation570 passes. Kura559 exposed15 stale fixture failures after1,322 passes; repaired focused572 passes31 of32, with the remaining old combined-path assertion corrected576 but not yet rerun. Preserve current drain/publication guards until immutable incarnation references and the consuming owner replace them. Formal542/550/560 preserves delegated obligations with43 preparation,187 affected fixture and52 merge-validation controls passing in their respective scoped runs. Structural575 remains failing with82 diagnostics, zero new from the storage split. Kagami562 exposes51 current-API migration compile errors;568 and574 migrate actual typed Native evidence without restoring retired proofs. This does not close an L1–L6 outcome.


## Consumer qualification and immutable storage draft (604–624)

Frozen620 compiles621 across Core, MV, test-network and Kagami library/binary test targets with zero errors;20,677 source inputs remain unchanged. Runtime608 passes128 controls and exposes three failures: a metadata mutation fixture did not actually mutate its configured DA policy; staged genesis published governed Nexus before installing its manifest/compliance registries; an evidence tamper fixture indexed a second proof despite constructing only one lane. Consumer610 makes the policy mutation valid and observable, installs both validated registries before staged Nexus publication, and uses the existing actual four-lane evidence fixture. Runtime613 passes all37 test-network/height controls and all five tamper controls; the four-validator Taira localnet generation/bootstrap control also passes. Overall613 passes47 and exposes three additional signing-fixture failures.

Signing fixtures614 retain private configuration custody and pinned signer enforcement. The checked-in deployment template is projected into an owner-only canonical temporary config with disposable fixture credentials;619 sets its pinned public key to the actual test signer. The direct-manifest assertion compares complete instruction batches after validating the derived context metadata and retains block/transaction signature checks. Generated localnet output paths are canonicalized before loading private files. Runtime617 passes six controls, including generated Nexus localnet resigning, exact signer/network context parity and private-file rejection;622 passes the remaining DA-policy/pinned-key control after619. All executables and frozen source inputs remain unchanged. These scoped runs do not constitute full Core/workspace or real-network liveness qualification.

QueuePlan604 aligns the ledger and source checker with actual defining modules, reservation journal release authority, unconditional synced-input exclusion and exact complete-input route/claim validation. Initial604 scoped execution exposed a missing reservation-journal fixture owner;609 adds the actual source file and keeps the positive-owner assertion. All18 controls pass, including new mutations; full diagnostics fall66→45 with21 removed and none added. Queue-owner618 requires strict startup reconciliation even for an empty initial receipt, binds the actual exact-key complete-input validators, and tracks the current restart/activation owner. All6 controls pass; full diagnostics fall45→39 with six removed and none added. The formal gate remains failing; token/source controls alone do not establish liveness.

Draft603 changes physical lane storage to immutable full identities and makes Apply/rollback publish references. It carries the actual serialized MV predecessor into snapshot pins and adds durable exact-instance collection ownership before GC moves. Its startup recovery/capacity boundary and remaining caller migration are unfinished and uncompiled. Root fixture624 retains all48 GC/startup controls, including symlink/no-clobber, crash/quarantine/deletion acknowledgement, accounting, merge receipt durability and corruption boundaries. Those tests now inspect retained immutable instances and only allow the exact durable collection transfer before a parent-substitution failure. Source formatting and whitespace checks pass; compilation/runtime qualification awaits the connected603 release. Carrier-scoped proof bundles, complete historical work authority, prepared resource handoff and consuming State publication remain open.

MAIN advanced externally to 4cb0be17e071bdcf55a7cd4879f815d1a2bc0229 before this update. All four task document hashes matched599. Packet623 captures and preserves this HEAD, merge state and staged index; no MAIN source or index was changed by this task.

Prior current-view text is retained verbatim:

The isolated canonical storage split552/556/563 passes1,339 Kura tests585 with zero failures and one preexisting performance-only measurement ignored; all20,676 source inputs and the owned executable are unchanged. Core/MV/test-network590/594 compile. Genesis591/595 each pass36 of37 controls: the metadata-mutation fixture exposed stale-signature ordering (fixed592), then invalid empty DA policy (corrected596, awaiting qualification). Capacity579 and Native recovery588 bindings pass55 and21 controls; full structural diagnostics fall82→76→66 with none added, and the gate remains failing. Native proof-path574 is source-released and compiling598. Immutable instance/reference publication, carrier proof bundles, historical work authority, prepared-resource handoff and consuming State publication remain unfinished. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#canonical-storage-and-current-consumer-qualification-563598) does not qualify MAIN, full workspace/formal/SDK or four/seven-validator liveness. All L1–L6 remain open.


The isolated canonical storage split passes1,339 Kura controls585 with zero failures and one preexisting measurement ignored. Capacity579 and Native recovery588 bindings pass55 and21 controls; full structural588 still reports66 diagnostics. Finish genesis/Native evidence consumer qualification. Implement immutable instance paths, authenticated bundle/reference publication and the complete consuming owner; these remain open.

Canonical storage552/556 separates chain files from lane alias moves; bounded preflight563 avoids unrelated full-tree rescans while retaining exact namespace identities. Full Kura585 passes1,339 tests with zero failures, one preexisting performance-only measurement ignored and no source/executable drift. Core/MV/test-network590/594 compile. Genesis/canonical-height591/595 controls pass36 of37: builder proposal-signing order is fixed592 and invalid empty-DA fixture corrected596, awaiting qualification. Capacity579 and Native history/recovery588 pass55 and21 controls. Full structural588 remains failing with66 diagnostics, ten removed and none added from579. Native scaling574 is source-released: current Native Decisions, complete context-write witnesses, exact first admissions, typed Network proofs and shared live RS16 calculation. Compilation598/runtime remain pending. Preserve current drain/publication guards until immutable instance references, authenticated carrier bundles and the consuming owner replace them. This does not close an L1–L6 outcome.


## Startup policy and delegated carrier bindings (625–647)

Pruning625 binds the actual planner, bounded preimage collection, intent validation, exact replay and no-clobber publication owners. Its first run passes16 controls and exposes two mutation-fixture anchors that omitted the generic function suffix;627 corrects those anchors and passes both mutations plus the positive owner. All18 selected obligations pass across the two runs. Full diagnostics fall39→31 with eight removed and none added. Delegated carrier631 preserves the candidate caller checks while binding its actual source budget and pristine execution helpers, plus the actual event preparation/authorization owners. All74 controls pass, including22 new semantic-mutation/ledger-owner controls. Full diagnostics fall31→22 with nine removed and none added. Sources and private indexes remain unchanged during validation; the full gate still fails and these structural checks are not a liveness proof.

Daemon628 makes startup policy ownership explicit: validate the effective protected catalog, freeze manifests and compliance once, install both before publishing governed lane geometry, and carry that exact snapshot through snapshot authentication, replay and queue handoff. Provisional imported snapshots still cannot publish geometry before authorization; emergency Fast still skips filesystem policy loading. Offline genesis validation and its staging fixture use the same ordering. Production/binary compilation630 passes but its thin binary has zero unit tests; actual daemon library test compilation633,637,641 and646 passes. Runtime634 passes23 of25 selected controls, including snapshot boundaries, protected catalog reconstruction and offline genesis success/rejection. Two new policy fixtures initially use blank Kura without an authenticated catalog;635 fixes that setup,639 replaces the parser fixture’s tiny example disk limit with bounded64MiB, and644 uses the production State constructor so default test geometry does not preinstall a conflicting incarnation. Runtime647 passes all four policy controls, including governed geometry, exact frozen source reuse, invalid compliance, and failure preserving prior policies/geometry. Each run captures20,677 unchanged inputs and its unchanged executable. These runs do not establish whole-workspace or real-network liveness.

Draft603’s ordinary retained auxiliary recovery could otherwise accept empty active-map scans or missing pair structure. The connected repair now enumerates exact journal references, retains original identities through repair/readback/cleanup, and introduces an explicitly authorized physical recovery pass before auxiliary consumers. Only existing durable Intent/H0 ownership may complete empty creation; completed/rolled-back pairs must already exist. Provisional snapshots defer mutation until authenticated finalization. Full network/dataspace/lane/incarnation/activation identity governs paths and replacement; aliases have no storage effect. Source review also identified unused empty per-instance base BlockStore/merge files after the canonical split. Their removal is an open first-release design obligation; any narrow empty-structure guard remains temporary and does not freeze mutable sidecar digests.

Root fixture626 retains32 transition/journal controls and updates its narrow reference-publication test hook. Fixture632 retains34 retirement/recovery controls, including zero-file and block-before-merge creation cuts, missing terminal evidence, foreign pair payloads, exact reference replay, no-clobber and parent substitution. Together with48 controls624,114 geometry controls are migrated but remain uncompiled alongside603. Additional ordinary/Native/external caller migration is ongoing;643 owns the next56-control Kura fixture file. Connected compilation and runtime qualification, carrier-scoped proof bundles, historical unmerged-work authority and consuming prepared-resource State publication remain open.

MAIN HEAD at capture: `4cb0be17e071bdcf55a7cd4879f815d1a2bc0229`. All four task document hashes matched623. Packet648 preserves current HEAD, merge state and staged index; this task changes no MAIN source or index.

Prior current-view text retained verbatim:

The isolated canonical storage split passes1,339 Kura tests585 with zero failures and one preexisting performance-only measurement ignored. Current Core/MV/test-network/Kagami test compilation621 passes with20,677 unchanged source inputs. Consumer610 fixes all three608 failures; 613 passes47 controls and exposes three broader signing fixtures. Those fixes614/619 pass all seven selected controls617/622, including generated Nexus localnet resigning and strict pinned-key/private-config checks. QueuePlan609 and queue-owner618 bindings pass18 and6 controls; full structural diagnostics fall66→45→39 with none added, and the gate remains failing. Immutable instance/reference cutover603 and its48 GC/startup fixtures624 are uncompiled drafts. Carrier proof bundles, historical work authority and prepared-resource State publication remain unfinished. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#consumer-qualification-and-immutable-storage-draft-604624) does not qualify MAIN, the full workspace or four/seven-validator liveness. All L1–L6 remain open.

The isolated canonical storage split passes1,339 Kura controls585. Current consumer compilation621 passes; signing fixtures617/622 pass seven controls including generated Nexus resigning. QueuePlan609 and queue-owner618 bindings pass18 and6 controls; full structural618 still reports39 diagnostics. Freeze, compile and qualify immutable instance/reference draft603 and48 GC/startup controls624; then finish authenticated carrier bundles, historical work authority and prepared-resource State publication.

Canonical storage552/556 and bounded preflight563 pass1,339 Kura controls585 with zero failures and one preexisting performance-only measurement ignored. Current Core/MV/test-network/Kagami test compilation621 passes with20,677 unchanged inputs. Consumer610 fixes all three608 failures; broader613 signing fixture failures are corrected614/619 and all seven selected controls pass617/622. The production genesis signer installs validated governance/DA registry policies before publishing Nexus so State cannot observe a configured lane without its manifest. QueuePlan609 passes18 controls and queue-owner618 passes6; full structural66→45→39 removes27 diagnostics with none added, but remains failing. Immutable network/dataspace/lane/incarnation/activation storage and reference-only Apply603 remain uncompiled drafts; fixture624 retains all48 GC/startup controls. Preserve current drain/publication guards until these owners, authenticated carrier bundles and consuming State publication are qualified. This does not close an L1–L6 outcome.


## Immutable instance compilation and owner bindings (650–662)

The connected603 source is frozen653 across20,680 inputs, including all56 Kura controls643, all875 State declarations650 (three renamed),40 sidecar controls651, and current/historical repair classification652. The State fixtures derive real pending/manual/static identities, capture original objects through retirement, preserve bytes across aliases, and publish exact journal references; cold sidecar tests replay the actual journal. Repair652 propagates authentication failures for an exact active route instead of classifying all errors as historical. Both a consumer corruption control and a helper identity matrix cover that distinction.

Compilation654 of Core, MV, test-network, Kagami, daemon and Torii lib/bin test targets fails with61 diagnostics on unchanged source. Caller657 and recovery658 fix the reported migrations without compatibility paths. Bootstrap temporary/quarantine recovery moves after admitted physical instance recovery and resolves exact retained network/route/incarnation/proposal context under geometry/sidecar guards. Marker656 rejects missing initial or dynamic authority during reference restoration and rejects complete/partial collection seals at active-use boundaries. New controls retain no-write assertions and positive fault restoration. These fixes are source-reviewed/formatted, not yet runtime-qualified.

Freeze660 captures all20,680 inputs including656–659; compilation661 is running. Binding659 moves staged incarnation/lineage publication and delegated drain validation to their actual owners while preserving caller, frontier, reset and watermark obligations. Its16 selected controls pass, including13 weakened-owner mutations and exact new-module closure. Twenty pytest warnings concern pre-existing temporary-directory cleanup; there are no selected test failures. Full gate655 reports28 diagnostics;660-based gate662 reports17 with no source/index drift. One remaining diagnostic is the registry checker expectation not yet matching its updated ledger; Native prepublication, in-flight lifecycle and delegated repair/size owners also remain open. These checks are source consistency evidence, not runtime or liveness proof.

MAIN changed externally since648;665 captures fresh HEAD `44f013385fb9ef56d0dcec14237776b6cf9f06d5`, preserves unrelated status/roadmap edits and changes only the four task documents. HEAD, merge state and staged index are unchanged by665. No source integration or release qualification is claimed.

Prior current-view text retained verbatim:

The isolated canonical storage split passes1,339 Kura controls585. Core/MV/test-network/Kagami test compilation621 and daemon library test compilation646 pass with20,677 unchanged inputs. Daemon628 installs one validated manifest/compliance policy snapshot before publishing geometry;634 passes23 startup controls and identifies fixture setup failures, corrected635/639/644 with all four policy controls passing647. Earlier signing617/622 passes seven controls. Pruning625/627 passes18 obligations and delegated carrier631 passes74 controls; full structural diagnostics fall39→31→22 with none added, but the gate remains failing. Immutable instance/reference cutover603, connected physical recovery, and114 migrated geometry controls624/626/632 remain uncompiled drafts. Carrier bundles, historical work authority, removal of unused per-instance base journals, consuming prepared-State publication and real four/seven-validator liveness qualification remain open. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#startup-policy-and-delegated-carrier-bindings-625647) does not qualify MAIN or close L1–L6.

The isolated canonical storage split passes1,339 Kura controls585. Consumer621 and daemon646 test compilation pass; startup628 policy controls pass647 after exact production fixture setup. Prune625/627 and delegated carrier631 controls pass18 and74 obligations; full structural631 still reports22 diagnostics. Finish caller migration, freeze and qualify connected immutable-instance/reference recovery603 and114 geometry controls624/626/632. Remove unused per-instance base journals instead of preserving empty compatibility structure; then finish authenticated carrier bundles, historical work authority and prepared-resource State publication.

Canonical storage552/556 and bounded preflight563 pass1,339 Kura controls585; consumer621 and daemon646 library test compilation pass with20,677 unchanged source inputs. Daemon628 validates and installs one immutable policy snapshot before governed geometry, reuses it for snapshot authentication/replay/handoff, and preserves provisional-import and emergency-Fast boundaries. Runtime634 passes23 controls; its two setup failures require authenticated Kura, a real bounded fixture budget, and the production State constructor635/639/644. All four policy controls pass647, including changed-source reuse and failure-before-partial-installation. Earlier signing617/622 passes seven controls. Prune625/627 passes18 obligations; delegated carrier631 passes74 with executable budget, pristine-surface and event-authorization mutations. Full diagnostics39→31→22 remove17 with none added, but remain failing. Immutable identity/reference draft603 and114 geometry controls624/626/632 remain uncompiled. Its physical recovery must complete only exact durable Intent/H0 targets after startup authorization, authenticate complete unsealed retained objects before auxiliary inventory, and refuse missing completed/rolled-back evidence. Remove unused per-instance BlockStore/merge scaffolding once the writer census confirms canonical storage and lane_artifacts own all data; a temporary empty-structure corruption guard is not design completion. Authenticated carrier bundles, complete historical work authority, consuming prepared-State publication and unchanged four/seven-validator qualification remain open. This does not close an L1–L6 outcome.


## Immutable instance runtime and startup audit (666–674)

Build661 passes all six crate lib/bin test targets in716.8s; build669 repeats that result after666/667 in146.0s, both with zero errors,664 warnings and20,680 unchanged inputs. Runtime664 passes14/21 foundational controls. Fixtures666/667 explicitly admit initial geometry, bind test networks, preserve original cold-recovery identities and distinguish read-only capacity reconstruction from complete startup repair. Runtime670 passes19/21. Identity separation, snapshot predecessors, creation intent ownership, missing/collection-sealed markers, Native foreign/tampered/unindexed temporary rejection, current-vs-historical repair classification and same-plan replacement controls pass. Source and immutable executable hashes remain unchanged.

The two670 failures are retained. Bootstrap quarantine itself succeeds, then the generation audit still asks the unpublished active catalog for the original LaneId. Source672 replaces that lookup with existing exact network/route/incarnation/activation journal authentication under geometry/sidecar guards; the real fsynced-temp regression now asserts cold startup has no active entries. Completed Native repair already promotes the exact manifest temporary and retires its original index without changing receipt/latest evidence; its fixture omits network binding before subsequent explicit geometry restoration.672 captures the authenticated original network and supplies it at that boundary. Source edits are narrowly captured and not yet runtime-qualified.

Broad Kura673 runs the remaining1,333 controls independently on unchanged668, preserving the two focused failures instead of treating this as acceptance evidence. Early failures cluster around geometry fixture networks, obsolete relocation/error expectations and checkpoint/collection assumptions; final results remain pending. Formal663 reports17→0 full structural diagnostics with unchanged20,682 inputs/index, plus51 new passing contract controls. Existing suite closure repair and final release are still in progress; no composed candidate has yet passed all runtime/model checks.

MAIN674 captures fresh HEAD `44f013385fb9ef56d0dcec14237776b6cf9f06d5`, preserves external changes and edits only the four task documents. No task source integration or goal completion is claimed.

Prior current-view text retained verbatim:

The prior canonical storage split passes 1,339 Kura controls585; consumer621/daemon646 compilation and all four startup-policy controls647 pass on their recorded source. The connected immutable-instance/reference candidate653 includes real State and sidecar fixture migration643/650/651 plus repair652, which distinguishes a historical identity from corruption in the active instance. Its first test-target compile654 fails with 61 errors on 20,680 unchanged inputs. Marker/collection authority656 and exact retained-bootstrap recovery658 close further recovery gaps; caller fixes657 and source bindings659 are frozen as660, with Core/Torii/daemon/MV/test-network/Kagami test-target compilation661 running. All 16 selected source-binding controls659 pass. Full formal diagnostics655→662 fall 28→17, including a remaining registry checker/ledger mismatch; the gate still fails. Carrier bundles, historical work authority, removal of unused per-instance base journals, consuming prepared-State publication and real four/seven-validator liveness qualification remain open. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#immutable-instance-compilation-and-owner-bindings-650662) does not qualify MAIN or close L1–L6.

Prior storage controls585 and consumer/startup qualification621/646/647 remain scoped to their source. Complete compilation and runtime qualification of frozen660: immutable references/recovery603, State and sidecar fixture migration643/650/651, corruption classification652, marker/collection guards656 and bootstrap/caller fixes657/658. First compile654 reported61 errors;661 is running after those fixes. Owner bindings659 pass16 controls; full formal662 still reports17 diagnostics. Close those actual-owner obligations, remove unused per-instance base journals, then finish carrier proof bundles, historical work authority and consuming prepared-State publication.

Prior canonical-storage, consumer and startup-policy evidence585/621/646/647 remains valid only for those recorded source inputs. Connected immutable instance603 now includes State/sidecar fixtures643/650/651 and repair652: exact current-identity corruption propagates an error, while genuinely historical identities remain distinct. First frozen653 compilation654 reports61 errors without source drift. Guards656 prohibit recreating missing completed markers or using collection-sealed objects. Bootstrap recovery658 runs after journal-owned physical authentication and resolves retained full identities;657 fixes ordinary call sites. Frozen660 contains20,680 unchanged inputs and is undergoing Core/Torii/daemon/MV/test-network/Kagami test-target compilation661. Source-binding659 passes16 controls including13 weakened-owner mutations and explicitly includes the new storage/recovery/GC modules. Full diagnostics655→662 fall28→17; remaining Native/in-flight/delegated owners and registry expectation require repair. No connected runtime pass is yet claimed. Remove the unused instance BlockStore/merge scaffolding; the temporary empty-structure guard is not design completion. Carrier-scoped proofs, complete historical authority, consuming prepared-State publication and unchanged four/seven-validator qualification remain open. This does not close an L1–L6 outcome.


## Immutable instance accounting and recovery (675–714)

Compilation699 passes all six crate library/binary test targets in450.646 seconds, zero errors, with20,682 unchanged inputs. Selected runtime700 finishes329 controls:295 pass and34 fail; source and executable remain unchanged. All geometry tests and new physical-accounting controls pass. This follows broader673 (1,192 pass,140 fail,one pre-existing measurement ignore) and690 (201/276 pass); their source differs and their counts are not a trend measurement.

Physical scanner682 accounts for every fixed immutable instance namespace, retained evidence and recovery temporaries once, with bounded entries, no-follow identity checks and no per-route open-descriptor growth. Replay binding692 retains the complete original authenticated inventory when active references change, preserving retired-path mutation detection. Retained-view repair701 carries the exact borrowed entry through startup before State. Fixtures686/687/691/697/702/704/705/706/709 use actual network, initial admission, physical paths and signed per-route incarnations; terminal inventory707 sorts full storage identities rather than hashed path names. All existing test declarations/assertions are retained except explicit contract corrections recorded in each source packet. Freeze711 carries the released corrections through709; compilation712 is running and713 is prepared to run every Kura test plus21 foundational controls. Exact-network post-WSV fixture710 is released for a later freeze.

Investigation693 identifies an unresolved certified-reset crash boundary. A durable frontier plus append journal can coexist with an older indexed prefix after a State-authorized reset. Cold preflight rejects it before State restores reset authority; merely permitting startup would expose generic append recovery that can mutate before authorization. No bypass was added. The continuation needs authenticated durable reset admission or typed retained debt with all mutating seams fenced until restored authority. Positive/negative crash-cut, read-only, retry and exact preimage controls remain required.

Formal release663 passes57 controls and a zero-diagnostic indexed structural gate on its recorded source. Gate703 against the unindexed validation tree reports480 diagnostics,479 due to missing indexed include providers and one cascaded owner check; it is preserved as a failed diagnostic. Indexed exact698 mirror708 passes with zero diagnostics in66.855 seconds, unchanged20,682 source inputs and unchanged index. These structural results do not execute TLC/TLAPS or qualify runtime liveness.

MAIN714 captures fresh HEAD `592c6e0e5adcd2ff5e0492d971bfbb179f591b53` and preserves staged index, merge state and unrelated edits. Only four task documents change. No source integration or release qualification is claimed; all six liveness goals remain open.

Superseded current-view passages, retained verbatim:

The connected immutable-instance candidate compiles all Core/Torii/daemon/MV/test-network/Kagami lib/bin test targets:661 and669 pass with zero errors and 20,680 unchanged inputs. Focused runtime670 passes19 of21 controls after fixture admission/restart corrections666/667. The remaining bootstrap failure is a generation audit that still consulted active geometry before State;672 switches it to exact retained-journal authority under geometry/sidecar guards. The Native completed-repair test now supplies its original network before explicit geometry restoration. Those changes await a rebuilt runtime. Broader Kura diagnostic673 is running and has exposed additional geometry fixture/recovery failures. Formal overlay663 reports a zero-diagnostic structural gate and51 new controls passing; existing controls and release capture remain in progress. Carrier bundles, historical work authority, removal of unused per-instance base journals, consuming prepared-State publication and real four/seven-validator liveness qualification remain open. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#immutable-instance-runtime-and-startup-audit-666674) does not qualify MAIN or close L1–L6.

All six crate lib/bin test targets now compile in661/669. Close the two focused runtime670 failures with the retained-bootstrap generation audit and exact-network fixture corrections672, then rebuild and rerun them with the broader Kura failures emerging in673. Review and freeze the final663 formal release; its structural gate and51 new controls pass on an isolated overlay. Complete connected State/snapshot runtime coverage. Remove unused per-instance base journals, then finish carrier proof bundles, historical work authority and consuming prepared-State publication.

Connected immutable instance603 and its caller/fixture corrections compile all six crate lib/bin test targets in661/669: zero errors on20,680 unchanged inputs. Focused runtime664 improved from14/21 to19/21 in670 after666/667. Bootstrap startup still reaches an active-catalog lookup in its generation audit;672 uses the exact retained journal under geometry/sidecar guards and asserts no active geometry is published during cold startup. The second failure is the repair fixture missing explicit original-network binding before State geometry restore;672 supplies it. These fixes require a rebuilt runtime. Broad Kura673 is diagnosing remaining geometry/recovery assumptions on unchanged668. Agent663 reports the complete structural gate reduced17→0 diagnostics and51 new positive/mutation controls passing on its indexed overlay; existing regression validation and final release are still in progress. No complete connected runtime pass is claimed. Remove unused instance BlockStore/merge scaffolding, then complete carrier-scoped proofs, historical authority, consuming prepared-State publication and unchanged four/seven-validator qualification. This does not close an L1–L6 outcome.


## Immutable instance recovery qualification (710–727)

Compilation712 passes in260.490 seconds with zero errors and20,682 unchanged inputs. Full Kura713 plus21 foundational controls finishes1,357 tests:1,344 pass,12 fail and one pre-existing measurement test is ignored. Source and copied executable hashes remain unchanged. The twelve failures are two post-WSV fixture networks, two certified-reset recovery cases, one completed-terminal cold network, one illegal sibling half-pair retirement setup, two active association reads before State restore, one earlier symlink diagnostic and three capacity fixtures.

Packet710 binds post-WSV fixtures to the actual signed execution network. Production715 returns optional targets only after authenticated retained-inventory absence and propagates failed marker/identity validation; two new controls cover filled/empty reservation maps, exact retry and real cold retained recovery. Packet716 restores original network/geometry before active association reads, establishes initial geometry before exact budget calculation and follows the earlier no-follow symlink error. Packet717 runs actual certified retirement, retaining the complete sibling instance and exact pending reservation while removing active admission. Packet719 charges real signed lane evidence instead of invalid arbitrary per-instance base bytes. Packet721 supplies the completed-terminal fixture's original network before State geometry restoration. Freeze723 includes these releases across nine changed paths; compilation724 passes in281.896 seconds with zero errors and20,682 unchanged inputs. Focused725 selects331 controls, including every713 failure and both new715 regressions; it is running. Reset718/726 are excluded.

The indexed720 composition gate passes with zero diagnostics in66.952 seconds;57 binding controls722 pass in215.59 pytest seconds with unchanged source/index. An initial722 precondition stopped before pytest because720 was still constructing its index; that attempt made no source changes and is recorded separately. These are structural source-binding checks, not TLC/TLAPS execution, a runtime liveness proof or MAIN qualification.

Reset693 exposed a second connected barrier: State's production startup projection rejected a pending append before its planned writer could run. Draft718 introduces authenticated retained debt and an exact State-authorized recovery admission; production passive preflight must return a repair plan while preserving bytes. Root regression726 extends the actual State reset control: interrupt its lifecycle-bound writer after the payload write at data sync, request two complete passive repair snapshots with exact QC/PoP owners and unchanged tree/status/generation, then close the debt through the State writer. It complements occupied-slot cold controls with a sparse higher-slot prepend case. Draft review also requires the common recovery fence to reject a forged certificate-type change or mismatched autonomous target. Implementation/runtime qualification remain pending.

MAIN727 captures fresh HEAD `592c6e0e5adcd2ff5e0492d971bfbb179f591b53` and preserves index, merge state and unrelated edits. Only four task documents change. All L1–L6 outcomes remain open; no source integration or release readiness is claimed.

Superseded current-view passages, retained verbatim:

The connected immutable-instance candidate compiles all Core/Torii/daemon/MV/test-network/Kagami lib/bin test targets in699 with zero errors and 20,682 unchanged inputs. Runtime700 passes295 of329 selected controls; all geometry and new physical-accounting controls pass, while34 recovery/fixture failures remain. Production682 accounts for every physical immutable instance and692 preserves the authenticated retained inventory across active-reference changes. Released701 repairs retained-view recovery before State;711 freezes it with fixture corrections for compilation712 and full Kura diagnostic713. The reset-authority recovery gap693 remains unresolved. Formal663 passes57 controls and its indexed structural gate; the independent indexed698 gate708 also has zero diagnostics. Gate703 lacked indexed include providers and is retained as a failed diagnostic. Carrier proofs, historical work authority, removal of unused instance journals, consuming prepared-State publication and real four/seven-validator qualification remain open. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#immutable-instance-accounting-and-recovery-675714) does not qualify MAIN or close L1–L6.

Six-crate test compilation699 passes; selected runtime700 passes295/329, including all geometry and new physical-accounting controls. Finish full Kura qualification713 of frozen711 after compilation712; carry the exact-network reservation fixtures710 into the next frozen candidate. Close certified-reset recovery ownership693 without inferring State authority from proposal height or allowing generic journal recovery to mutate before authorization. Formal663 passes57 controls and its indexed gate; indexed698 gate708 independently reports zero diagnostics. Complete connected State/snapshot runtime coverage, remove unused per-instance journals, then finish carrier proof bundles, historical work authority and consuming prepared-State publication.

Connected immutable instance603 and corrections through698 compile all six crate lib/bin test targets in699 with zero errors and20,682 unchanged inputs. Selected runtime700 passes295/329 controls; all geometry and new physical-accounting controls pass. Physical accounting682 visits each immutable instance once with bounded scans and identity checks; retained replay binding692 preserves the complete authenticated inventory across active-reference changes. Recovery701 passes exact retained identity into view repair before State instead of consulting the absent active catalog. Freeze711 includes701/702/704/705/706/707/709 and awaits compilation712/full Kura diagnostic713; fixture710 follows separately. Certified-reset gap693 still needs explicit durable authorization ownership across frontier/append crash cuts: a greater proposal height alone cannot authorize replacement, and unauthenticated generic recovery must not mutate old indexed state. Formal663 passes57 positive/mutation controls and a zero-diagnostic indexed gate; indexed exact698 gate708 independently passes with unchanged source/index. Failed gate703 lacked indexed include providers and is not a production failure or pass. Carrier-scoped proofs, historical work authority, removal of unused instance BlockStore/merge scaffolding, consuming prepared-State publication and unchanged four/seven-validator qualification remain open. This does not close an L1–L6 outcome.


## Certified reset and terminal rewrite qualification (728–745)

Run725 completed328/331 passes: only the two reset controls and a symlink diagnostic assertion failed. Packet730 corrects the symlink assertion to its actual early prune-inventory owner without changing production policy. Ordinary reset718 plus startup deferral735 bind authenticated retained history, the exact frontier and append journal to restored State authority; generic readers cannot resume the reset. Passive State planning now returns the pending repair without storage or cache mutation. The original726 sparse fixture did not hit the append hook:734 seeds an actual occupied height1 before the retained height5, preserving all original filtering assertions. Sparse prepend recovery remains a separate open path.

Freeze731 combines723 with718/726/730/734/735. Compilation732 finds five test-only errors: three denied trivial casts and two missing path borrows. Packet738 fixes them; freeze739 compiles all six lib/bin test targets in740 in325.764 seconds, zero errors/664 warnings,20,682 unchanged inputs. Focused741 completes336 tests:331 pass and five fail with unchanged source/executable. All three new ordinary reset crash cuts pass. The five failures are three retirement/rewrite controls, post-authentication frontier substitution failing to set the ambiguity flag, and the connected State fixture lacking admitted physical H0 geometry. Broader742 selects1,125 remaining Kura and explicitly connected State controls without repeating741; it is in progress.

Source743 distinguishes namespace-bound cleanup of unpublished data/build files from indexed mutation. A committed terminal rewrite carries its existing authenticated inventory plus held original/candidate pairs and exact frontier into the common recovery boundary. Retirement calls that same owner before frontier repair. The existing rewrite test now interrupts the real writer after both temporary-file syncs at its directory barrier; a generic read must leave all four files unchanged before authorized recovery. Source744 uses production-like authenticated startup/H0 for the connected State regression. READY729 extends the reset owner with its independently authenticated source, complete capacity reservation and both State and existing READY commit authority. These changes remain uncompiled/unqualified. Unification of sparse prepend with the append journal requires an operation-specific encoded-size budget and bounded decoding; no compatibility path is intended.

Indexed mirror736 of exact731 passes the structural gate with zero diagnostics in65.671 seconds. All57 controls737 pass in218.100 runner seconds, with source/index unchanged. These are source-binding controls, not TLC/TLAPS execution or a network liveness proof. MAIN745 changes only four task documents, preserving HEAD/index/merge state and unrelated edits. All L1–L6 outcomes remain open.

Superseded current-view passages, retained verbatim:

The connected immutable-instance candidate compiles all Core/Torii/daemon/MV/test-network/Kagami lib/bin test targets in712 and724 with zero errors and20,682 unchanged inputs. Full Kura diagnostic713 finishes1,357 controls:1,344 pass,12 fail and one existing measurement is ignored; source and executable remain unchanged. Frozen723 includes fixes for ten failures, including exact-network post-WSV fixtures710, authenticated absence versus corruption715, real retirement717 and restart/capacity fixtures716/719/721; focused331-control run725 is in progress. The two certified-reset failures require connected cold preflight, State planning and authorized writer recovery718/726, still unqualified. Indexed composition720 has zero structural diagnostics and all57 binding controls722 pass. Carrier proofs, historical work authority, removal of unused instance journals, consuming prepared-State publication and real four/seven-validator qualification remain open. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#immutable-instance-recovery-qualification-710727) does not qualify MAIN or close L1–L6.

Six-crate test compilation724 passes. Full Kura713 reports1,344 passes,12 failures and one existing measurement ignore on unchanged711. Qualify frozen723/724 in focused725, which carries ten reviewed fixture/recovery fixes including strict post-WSV error classification715. Finish certified-reset recovery718 with connected State regression726: cold startup must retain authenticated debt, passive planning must expose its repair owner without writes, and every mutating recovery must require exact State authority. Indexed720 gate and57 controls722 pass. Complete connected State/snapshot runtime coverage, remove unused per-instance journals, then finish carrier proof bundles, historical work authority and consuming prepared-State publication.

Connected immutable instance603 and corrections compile six crate lib/bin test targets in712 and724, zero errors on20,682 unchanged inputs. Full Kura713 finishes1,357 controls:1,344 pass,12 fail,one existing measurement ignore; source and executable remain unchanged. Freeze723 includes710/715/716/717/719/721; focused331-control725 is running. Strict post-WSV lookup715 distinguishes proven absence from failed current/retained authentication, preserving reservation ownership and cold reconstruction. Certified-retirement fixture717 retains the complete journal-owned sibling instance rather than moving half a pair; capacity719 uses real signed lane evidence. Reset investigation693 now has draft718 and connected State regression726: authentication must precede append mutation, and production read-only State planning must return a reachable repair obligation so startup does not stop before its writer can run. These changes remain unqualified; they must reject unauthorized, foreign, newer or tampered preimages without granting an autonomous-certificate bypass. Indexed720 structural gate passes with zero diagnostics; all57 controls722 pass on unchanged source/index. Carrier proofs, historical work authority, removal of unused instance scaffolding, consuming prepared-State publication and unchanged four/seven-validator qualification remain open. This does not close an L1–L6 outcome.

## READY reset and captured State predecessor (746–764)

749 compiles frozen748=739+743+744+729+751 across Core, MV, test-network, Kagami, daemon and Torii lib/bin test targets in327.034 seconds, with zero errors and666 warnings; all20,682 captured source/config inputs remain unchanged.750 executes357 distinct selected Core controls:349 pass, eight fail, none ignored; executable and source fingerprints remain fixed. Every729 new READY reset crash-cut, corrupted-source and successor-append control passes, together with the BLS reuse and post-validation substitution regressions. The remaining failures are three retirement ordering/crash-fixture cases, two dataspace rebind fixtures, one historical resultless-carrier fixture, and two replay calls refused by the unfinished State publisher.

742 completed the earlier739 artifact's remaining full Kura namespace plus explicitly selected connected State cases:1,107 pass,17 fail and one measurement is ignored. This is diagnostic evidence, not a full Core or network qualification.746 source binding on739+743+744 passed.753 on748 found one stale expected call arity;755 retains both explicit absent-authority arguments.756 gate on748+755 passes with zero diagnostics in63.963 seconds, and757 passes all57 binding controls in260.309 seconds with unchanged source/index. These are structural controls, not model execution.

752 derives the historical fixture's complete execution attachment through the actual validator.758 makes catalog validation use its owned MV parameter preimage and captured runtime policy rather than a separately opened live view; new controls cover a real replacement, an unauthorized copy of the discarded tip, policy-cache mutation and late key invalidation.759 limits terminal rewrite admission to actual committed rewrite markers and makes the test-only retained-window helper reach the genuine temporary-write barrier even for an unchanged window.763 supplies each structural dataspace fixture's exact static baseline. These source-only follow-ups form frozen760; six-crate761 compilation is running.747 separately unifies bounded prepend intent, exact encoding and pending-receipt admission/compaction ownership. No compatibility decoder or production publication bypass is authorized.

The complete consuming publisher, retained resource handoff across cached/recovered validation and Apply, carrier-scoped Native proofs, historical completion ownership, unused instance-scaffold deletion and real four/seven-validator fault/restart/final-transaction qualification remain open. MAIN source and index are preserved; only the four scoped documentation files change. No liveness goal is complete.

Superseded current-status statements are retained verbatim below.

The isolated immutable-instance candidate compiles all six crate lib/bin test targets in740 with zero errors on20,682 unchanged inputs. Focused741 finishes336 controls:331 pass and five fail; the three new reset crash cuts pass. Failures expose overbroad rewrite recovery fencing, a lost frontier ambiguity flag and an incomplete State fixture startup. Source fixes743/744 and READY recovery729 await qualification; broader Kura/State742 is running. Indexed731 source gate736 has zero diagnostics and all57 binding controls737 pass on unchanged source/index. Carrier proofs, historical work authority, removal of unused instance journals, consuming prepared-State publication and real four/seven-validator qualification remain open. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#certified-reset-and-terminal-rewrite-qualification-728745) does not qualify MAIN or close L1–L6.

Six-crate test compilation740 passes. Qualify the connected ordinary reset, terminal rewrite and READY source owners together:741 passes331/336 controls, with five diagnosed failures;743/744 and729 are unqualified corrections and broader742 is running. Preserve all three passing reset crash cuts and exact State-authorized continuation. Indexed731 gate736 and57 controls737 pass. Complete connected State/snapshot coverage, unify sparse prepend recovery and exact capacity accounting, remove unused per-instance journals, then finish carrier proof bundles, historical work authority and consuming prepared-State publication. Finish native production integration and unchanged four/seven-validator fault/restart/final-transaction qualification before closing any liveness goal.

Connected immutable instance603 and the ordinary reset owner compile six crate lib/bin test targets in740 with zero errors on20,682 unchanged inputs. Frozen739 focused741 passes331 of336 controls; all three ordinary reset crash cuts pass. The five failures identify overbroad fencing of pre-publication cleanup and authenticated terminal rewrites, an omitted fail-stop flag after an extra authenticated reread, and the new connected State fixture lacking the physical H0 journal. Corrections743/744 and READY reset729 are source-only; broader Kura/State742 is running. The State regression now targets a real occupied slot; sparse prepend has a distinct write protocol and remains open until unified journal and exact sizing work is qualified. Indexed731 gate736 passes with zero diagnostics and57 controls737 pass on unchanged source/index. Carrier proofs, historical work authority, removal of unused instance scaffolding, consuming prepared-State publication and unchanged four/seven-validator qualification remain open. This does not close an L1–L6 outcome.


## Carrier policy and qualification (765–776)

765 structurally checks exact760 with zero diagnostics in63.712 seconds, unchanged source and index.761 failed compilation with two fixture type errors: the actual fragment count is optional until execution attaches it.766 requires the actual attachment before checking its count; frozen767=760+766 passes six-crate lib/bin test compilation768 in208.775 seconds, with zero errors and666 warnings on20,682 unchanged inputs.

769 executes397 exact Core controls in separate processes, four at a time:393 pass, four fail, none ignored; source and retained executable remain unchanged. All selected READY reset, BLS/substitution, retirement rewrite, catalog predecessor, carrier preparation and World preparation controls pass. The remaining failures are consensus_lane_lifecycle_replay_converges_on_fresh_state and signed_lane_lifecycle_rejects_stale_catalog_after_prior_commit (ExecutionOutputCapacity at the unfinished publisher); historical_autonomous_merge_recovers_certified_carrier_before_world_replay (actual execution now reaches the competing native/ordinary source guard); and lane_lifecycle_same_shard_dataspace_rebind_hides_previous_da_indexes_after_kura_replay (its pin exists only in Kura, without the canonical World alias owner).772 seeds that exact pin owner in the fixture while retaining pre-reset visibility and reset/restart absence assertions. It changes no production hydration authority.

770 captures immutable Nexus, manifest, compliance and ZK policy at each original StateBlock constructor, including replacement. Additive merge catalog validation uses the original parameter/Cell predecessor, exact pending geometry, independently derived manifests and captured policy; it no longer opens a live World view or runs retirement I/O for an additive transition. The original policy survives journal decomposition. New tests stage a real catalog transaction, alter physical policy caches, reject copying that substitution into the overlay, and exercise actual paired World/runtime undo.771 checks indexed767+770 with zero diagnostics in64.893 seconds on unchanged source/index. These are source-binding diagnostics, not model execution or runtime qualification of770. Frozen773=767+770+772 is compiling in774.747 remains independently owned until immutable release and composition; its new receipt-window work must cover absent-pair high-first writes and Direct receipt growth as well as compaction.

Original MV journals still retain concrete writer guards. A validated hash cannot substitute for their complete retained State/resource owner; carrying locked writers across quorum waits is not the intended handoff. The consuming publisher, exact native publication/Apply, carrier-scoped proofs, historical work authority, unused instance-scaffold deletion and same-candidate four/seven-validator fault/restart/final-transaction matrix remain open. MAIN source/index are preserved; only the four scoped documentation files change. No liveness goal is complete.

Superseded current-status statements are retained verbatim below.

The isolated immutable-instance candidate compiles six crate lib/bin test targets in749 with zero errors on20,682 unchanged inputs. Focused750 finishes357 controls:349 pass and eight fail; the READY reset crash/source/successor, cached BLS and substitution controls pass. Source fixes752/758/759/763 address historical fixture execution, actual MV catalog predecessor ownership, retirement rewrite ordering and static dataspace fixtures; frozen760/761 compilation is running. Two replay failures still require the complete State publisher. The earlier broader742 finishes1,107 passes,17 failures and one ignored measurement. Indexed748+755 gate756 and all57 binding controls757 pass on unchanged source/index. Unified prepend747 remains in progress. Carrier proofs, historical work authority, removal of unused instance journals, consuming prepared-State publication and real four/seven-validator qualification remain open. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#ready-reset-and-captured-state-predecessor-746764) does not qualify MAIN or close L1–L6.

Six-crate test compilation749 passes;750 passes349/357 controls with unchanged source/executable. Qualify760, including the retirement ordering, actual historical execution and State predecessor/fixture follow-ups;761 is compiling. Preserve the passing READY/reset/cache/substitution controls. Indexed748+755 gate756 and57 controls757 pass. Complete connected State/snapshot coverage, unified prepend747 and its exact capacity/receipt-compaction pins, remove unused per-instance journals, then finish carrier proof bundles, historical work authority and consuming prepared-State publication. Two replay controls still fail at that unfinished publisher. Finish native production integration and unchanged four/seven-validator fault/restart/final-transaction qualification before closing any liveness goal.

The isolated immutable-instance candidate compiles six crate lib/bin test targets in749 with zero errors on20,682 unchanged inputs. Focused750 finishes357 controls:349 pass and eight fail; the READY reset crash/source/successor, cached BLS and substitution controls pass. Source fixes752/758/759/763 address historical fixture execution, actual MV catalog predecessor ownership, retirement rewrite ordering and static dataspace fixtures; frozen760/761 compilation is running. Two replay failures still require the complete State publisher. The earlier broader742 finishes1,107 passes,17 failures and one ignored measurement. Indexed748+755 gate756 and all57 binding controls757 pass on unchanged source/index. Unified prepend747 remains in progress. Carrier proofs, historical work authority, removal of unused instance journals, consuming prepared-State publication and real four/seven-validator qualification remain open. This source does not qualify MAIN or close L1–L6.


## Owned journal preparation and prepend qualification (774–797)

774 compiles all six selected crate lib/bin test targets on frozen773 in389.020 seconds with zero errors and663 warnings,20,682 unchanged inputs.775 runs399 exact Core controls, four isolated processes at a time:396 pass, three fail, none ignored; source and retained executable remain unchanged. The new captured-policy controls pass, and772 corrects the canonical World DA-pin fixture. The remaining failures are consensus_lane_lifecycle_replay_converges_on_fresh_state and signed_lane_lifecycle_rejects_stale_catalog_after_prior_commit (unfinished publication's ExecutionOutputCapacity refusal), plus historical_autonomous_merge_recovers_certified_carrier_before_world_replay (competing ordinary/native source ownership). No guard is bypassed.

747 releases one authenticated bound prepend intent with bounded complete old/new windows, canonical sizing, explicit decode limits, retained post-WSV receipt headroom, exact-pair compaction pins and absent-pair ordering.777 finds six stale source bindings;778 updates both ledger and exact owner contracts.779 checks exact773+747+778 with zero diagnostics in67.655 seconds;781 passes57 structural mutation/owner controls in263.34 seconds, with unchanged source/index. These are source checks, not protocol-model or runtime qualification. Frozen782 compiles in783 but fails11 diagnostics on unchanged inputs: seven test File/OpenOptions qualification errors and two File-only parser mismatches repeated across lib/test targets.789 uses the same Read+Seek parser for live files and owned authenticated images, qualifies the test paths and adds disk/Cursor header/length/checksum/position parity. Failed783 is retained; no runtime result exists for it.

780 adds lifetime-free owned Cell/Storage journals with exact private current/undo owner identities, explicit ordinary/replacement mode, and admission before copying touched final values. Original undo data moves and writers release on capture or refusal;17 new controls include actual worker handoff. All mutation/constructor paths maintain identity. It is a capture primitive: no detached publish/reattach API, no allocation-free installation claim, and complete aggregate admission remains open.785 detaches membership in actual carrier decomposition, retaining exact full-history/tip identity and immutable admitted action.787 removes its borrowed storage reference for static worker custody. Six membership controls cover release/drop, no-op/refusal, concurrent candidates, replacement/ABA, history-only change and distinct restored owners.786 exposes six raw multiline-token mismatches,787 corrects those;788 exposes one trailing-comma normalization mismatch,791 corrects the fragment while retaining normalized exact destructure, rotation and drop relations. Checker failures remain retained evidence.

Frozen792=782+780+785+787+789+791 contains20,685 inputs and is compiling in793.795 checks the exact indexed candidate;796 runs membership positive/mutation controls.790 block-hash capture remains separate parallel work, excluded from792. World/runtime/hash/archive detachment, retained resources and the aggregate publisher still need completion before actual Validate-to-Apply custody can replace the old path. No State/native/output authorization is weakened. MAIN source/index/HEAD remain untouched; only scoped progress documents change. No liveness goal is complete.

Superseded current-status statements are retained verbatim below.

Frozen767 compiles six crate lib/bin test targets in768 with zero errors on20,682 unchanged inputs. Runtime769 completes397 controls:393 pass and four fail. READY reset, retirement ordering and captured-predecessor controls pass. Two failures retain the unfinished State publication refusal; historical execution reaches the ordinary/native ownership guard, and one DA pin fixture lacks its canonical World owner. Source772 corrects that pin fixture. Source770 captures immutable carrier policy and removes live cache reads from additive merge-catalog validation; indexed767+770 gate771 passes. Frozen773=767+770+772 is compiling in774. Unified prepend747 is awaiting composition and qualification. Carrier proofs, detached retained journals, historical work authority, unused instance-journal removal, consuming prepared-State publication and real four/seven-validator qualification remain open. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#carrier-policy-and-qualification-765776) does not qualify MAIN or close L1–L6.

Frozen767 compiles six crate lib/bin test targets in768 with zero errors on20,682 unchanged inputs. Runtime769 completes397 controls:393 pass and four fail. READY reset, retirement ordering and captured-predecessor controls pass. Two failures retain the unfinished State publication refusal; historical execution reaches the ordinary/native ownership guard, and one DA pin fixture lacks its canonical World owner. Source772 corrects that pin fixture. Source770 captures immutable carrier policy and removes live cache reads from additive merge-catalog validation; indexed767+770 gate771 passes. Frozen773=767+770+772 is compiling in774. Unified prepend747 is awaiting composition and qualification. Carrier proofs, detached retained journals, historical work authority, unused instance-journal removal, consuming prepared-State publication and real four/seven-validator qualification remain open. This source does not qualify MAIN or close L1–L6.

Six-crate compilation768 passes;769 passes393/397 controls on unchanged source/executable. Preserve the READY/reset/retirement controls and qualify captured policy770 plus authoritative DA fixture772 in frozen773/774. Indexed767+770 gate771 passes; previous57 binding controls757 retain their separate scope. Complete unified prepend747 and its exact pending-receipt envelope across compaction and first-write ordering. Finish detached original-journal handoff, carrier proof bundles, historical work authority and consuming prepared-State publication; current replay and old merge fixtures still expose these incomplete boundaries. Remove unused instance journals, integrate native production ownership, then qualify one unchanged four/seven-validator fault/restart/final-transaction candidate before closing any liveness goal.


## Complete World capture and retained failures (798–816)

793 fails two denied test pointer-cast diagnostics on frozen792 after641.280 seconds; no794 runtime occurs.799 corrects the test coercions.795 passes the exact792 indexed source gate in67.038 seconds with zero diagnostics;796 passes all49 membership structural/mutation controls in126.22 pytest seconds, with unchanged source/index.804 independently executes the complete emitted MV artifact from793:73/73 pass with unchanged source/executables; the overall793 compilation still failed. These separate receipts are not one full-candidate qualification.

Frozen800=792+790+798+799 contains20,686 inputs.801 compiles all six selected crate lib/bin test targets in805.009 seconds, zero errors and674 warnings, unchanged inputs.802 stops during selector resolution before running tests because one747 selector omitted its owning module.809 records each unique selector-to-executable-name resolution, then runs528 exact single-test processes, four concurrently. All73 MV and451/455 Core tests pass, none ignored; all source inputs and both retained executables remain unchanged. The four failures are the three previously retained State publication/native execution ownership failures and post_wsv_prepend_reserves_bounded_future_window_and_exact_retry. The latter reaches the actual failed-data-sync append intent: read-only reservation reconciliation succeeds without mutation, but the writer's own second attempt rejects that unresolved intent before its recovery owner runs.815 is fixing exact authorized retry ordering; passive readers and malformed/foreign temporary refusal remain required.

803 captures all ten original TriggerSet journals after one admission.805 captures the four actual runtime Cells.806 checks exact800+803+805 indexed source in67.325 seconds with zero diagnostics and unchanged source/index.807 derives all278 World journals from the existing inventory, exhaustively destructures those plus the dataspace catalog and external events, and retains typed original deltas and exact owner/current/undo identities in a flat private owner. No World clone, Any/downcast, reconstructed read view, publisher or compatibility path is introduced. Nine new controls cover actual deltas, aborted children, replacement, every untouched owner, mode/admission refusal, writer release, moved extras and static handoff.808 uses this capture during actual PreparedCarrier decomposition and calls one required admission callback over the original StateBlock before archive projection or final-value copying. Its guard remains last on the successful owner. Independent review finds early-error local drop order releasing that guard before original owners;816 declares the guard before those owners and tests that its destructor sees the original hash writer released.

Frozen810=800+803+805+807+808+816 contains20,691 inputs.811 is compiling the six crate test targets and813 is checking exact indexed source;815 remains excluded. Runtime812 is prepared only if811 passes, including complete MV and TriggerSet suites plus retained Core and new World/carrier controls. These source changes have not completed runtime qualification at this record.

Independent receipt audit also identifies a separate post-commit liveness gap: Direct carriers return early from merge-only prepend admission but use the same bounded receipt index. A pending low merge receipt can make a later Direct receipt exceed the admitted index span; the common writer returns WouldBlock only after canonical execution exists. The lane adapter erases this into Persistence while holding a fail-stop operation; historical recovery likewise has no typed receipt-resource dependency. This is a code-path finding, not an executed real-network reproduction. Merely retrying that error or weakening the output guard is insufficient: complete Direct/Merge ownership must be established before voting, with a reachable drain wakeup, or the planned carrier proof replacement must remove the shared index obligation.

Archive captures still retain their physical index writers. Aggregate heap/installation admission, archive logical reservations, complete Validate-to-Apply custody, the sole consuming publisher, carrier proof bundles, historical authority, unused instance-journal retirement and the known initial-author failover gap remain open. MAIN receives only these four scoped documents; source, HEAD, MERGE_HEAD and index remain untouched. No liveness goal is complete.

Superseded current-status statements are retained verbatim below.

Frozen773 passes six-crate lib/bin test compilation774 and396/399 exact Core controls775 on20,682 unchanged inputs and a retained unchanged executable. Captured policy770 and authoritative DA fixture772 pass; three failures still expose unfinished State publication and competing native/ordinary execution ownership. Unified prepend747 with bindings778 passes indexed gate779 and57 structural controls781; composed782 compilation783 fails11 diagnostics, corrected by source789. Detached MV780 and membership785/787 retain exact original identities and release writers, without granting publication; checker correction791 retains full consuming relations. Frozen792=782+780+785+787+789+791 is compiling in793; indexed gate795 and membership controls796 are running. Block-hash detachment790 is parallel work. Complete journal/resource custody, prepared-State publication, carrier proofs, historical work authority, unused instance-journal removal and real four/seven-validator qualification remain open. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#owned-journal-preparation-and-prepend-qualification-774797) does not qualify MAIN or close L1–L6.

Frozen773 passes six-crate lib/bin test compilation774 and396/399 exact Core controls775 on20,682 unchanged inputs and a retained unchanged executable. Captured policy770 and authoritative DA fixture772 pass; three failures still expose unfinished State publication and competing native/ordinary execution ownership. Unified prepend747 with bindings778 passes indexed gate779 and57 structural controls781; composed782 compilation783 fails11 diagnostics, corrected by source789. Detached MV780 and membership785/787 retain exact original identities and release writers, without granting publication; checker correction791 retains full consuming relations. Frozen792=782+780+785+787+789+791 is compiling in793; indexed gate795 and membership controls796 are running. Block-hash detachment790 is parallel work. Complete journal/resource custody, prepared-State publication, carrier proofs, historical work authority, unused instance-journal removal and real four/seven-validator qualification remain open. No liveness goal is complete.

Qualify frozen792 across complete MV tests and exact Core carrier/receipt/geometry controls. Compilation774 and runtime775 establish396/399 passes only on773;779/781 establish separate structural evidence for773+747+778. Preserve783 compilation failures and786/788 checker failures alongside their789/791 corrections. Complete detached World/runtime/hash/archive owners, predecision resource admission and the sole consuming State publisher. Resolve the three retained publication/execution-owner failures, carrier proof custody and historical work authority, remove unused instance journals, and integrate the shared lane reducer. Qualify one unchanged four/seven-validator fault/restart/final-transaction candidate before closing any liveness goal.

## Work consolidated on optimizations, September 18

The user directed all further work to `/Users/takemiyamakoto/dev/iroha` on
`optimizations`. Existing task changes were imported into that working tree from
their retained source, with overlapping MAIN changes reviewed and the Git index
unchanged. Independent beacon composition, lifecycle-certificate admission and
telemetry changes remain. This composition is new; earlier isolated passes do
not qualify it. Import review and local command logs live under ignored
`dist/sumeragi-main-work/` in this checkout.

All 88 MV tests pass, including fifteen consuming-publication controls for actual
Cell/Storage changes, replacement undo, existing readers, stale/foreign owners,
writer contention, abort/retry, admission-before-copy and resource-guard custody.
The retired-codec guard passes. The combined Core/Torii/test-network/Kagami/daemon
test compilation is in progress. Actual aggregate State publication, complete
resource accounting and Validate-to-Apply custody remain unfinished. Parallel
lane work must connect the existing shared reducer through one process-lived
transport/Decision/candidate owner before retiring the old fresh signer.

The previous isolated819 runtime821 finished597/601 with unchanged sources and
executables: all new World/trigger/carrier controls passed, while the same three
State ownership failures and authenticated interrupted-receipt retry failed.
The receipt fix815 and archive custody823–825 are now in the main working tree;
they still require qualification on this combined branch. No goal is complete.

Superseded current-status and roadmap text, retained verbatim:

Frozen800 passes six-crate lib/bin test compilation801 on 20,686 unchanged inputs. Runtime809 passes 524 of 528 exact tests: all 73 MV tests and 451 of 455 Core controls, with unchanged source and retained executables. Three State publication/native ownership failures persist. One new failure identifies a receipt writer that rejects its own authenticated interrupted append before reaching recovery; source815 is in progress. Membership bindings795 and all 49 controls796 pass; exact800+803+805 source gate806 passes. World capture807 and actual carrier integration808 retain all 278 original World journals and four runtime cells after one admission; review correction816 makes the reservation outlive originals on early errors. Frozen810 contains these changes and is compiling in811; source gate813 is running. Archive writers, aggregate pre-vote resource ownership, the consuming publisher and real four/seven-validator qualification remain open. Direct receipt window exhaustion also lacks pre-vote admission and can reach fail-stop after commitment. [Scoped evidence](docs/history/2026-09-16/sumeragi-lane-context-foundation.md#complete-world-capture-and-retained-failures-798816) does not qualify MAIN or close L1–L6.

The next Sumeragi cutover must carry exact prepared State/resource ownership through validation, cached/recovered markers, voting and Apply; returning only the execution hash drops that owner. Acquire bounded descriptor/capacity resources before voting and preserve typed local deferral with a reachable retry. Prototype530 was withdrawn for post-finality descriptor growth. Retain historical Native authority, stable canonical storage and immutable incarnation directories. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded complete history references; preserve exact merge-source/finality/WSV joins before source release, and charge whole bundles while any route retains them. Defer physical GC with release, snapshot/recovery pins and an instance-local deletion fence. Preserve cross-route closure and drain/signing fences. Remove the post-finality local Queue veto only with the complete consuming path. Qualify frozen810 across complete MV/TriggerSet suites and retained Core carrier/receipt/geometry controls. Frozen800 passes compilation801 and 524/528 tests809; preserve all four failures. Complete receipt-owned interrupted append recovery815 without weakening passive authentication, and remove the Direct/Merge post-commit capacity refusal through complete pre-vote resource ownership or the planned carrier proof design. Complete archive logical reservations, aggregate admission and the sole consuming prepared-State publisher; the detached World/runtime/hash/membership owners now need that actual Validate-to-Apply custody. Preserve scoped source passes795/796/806 and qualify807/808/816 together. Resolve the three State publication/native ownership failures, carrier proof custody and historical work authority, remove unused instance journals, and integrate the shared lane reducer. Qualify one unchanged four/seven-validator fault/restart/final-transaction candidate before closing any liveness goal.


## Consuming component publication on optimizations, September 18

The checkout-local ownership contract run passes all 104 tests in 527.27 seconds. The first combined test build found one obsolete `execute_time_triggers` call and four consequent inference errors; the fixture now invokes the canonical output producer and retains the no-internal-source and tampering assertions. The next build found six denied trivial casts in new membership allocation-identity tests; these use `std::ptr::from_ref` now. Neither failed build is a runtime pass.

Detached membership reacquires the exact original identity and consumes its already admitted transition. Detached block hashes move the original complete vector without another chain copy. All ten trigger components prepare together; a late refusal aborts every earlier writer and returns original journals. Installation guards outlive physical writers and transfer to the aggregate caller on publication. Six hash, five membership and five trigger regressions cover real values/undo, original allocations, foreign/stale identities, all trigger contention positions, abort/retry and guard lifetimes. They await a passing rebuilt harness. These operations are component mechanisms; complete State authorization and resource accounting are still required before production use.

The registered native lane driver has three real-State controls for silent-author failover, global-carrier rollover retention and complete Decision-group handoff. Transport and frame-bound drafts remain separate from production activation. Current native replay refuses NPoS controls; mandatory beacon composition is an explicit activation dependency, alongside one native ingress/candidate/Apply consumer and atomic retirement of the legacy fresh signer.

Superseded current-status text, retained verbatim:

All active Sumeragi edits and validation now use this checkout on `optimizations`, as requested. The existing isolated work has been reconciled into the branch working tree with independent beacon, key-lifecycle and telemetry changes retained and the Git index unchanged. Complete World/runtime/hash/membership capture and lifetime-free provider/reputation archive custody are present. MV now prepares exact original current/undo publications under nonblocking writer acquisition, returns unchanged journals on refusal, supports abort without publication, and transfers retained resource guards. All 88 MV tests and the retired-codec guard pass here. The combined Core/Torii/test-network/Kagami/daemon test build is running. Earlier isolated runtime821 passed 597/601 controls, retaining three State ownership failures and the receipt retry failure addressed by source815; that result does not qualify this composition. The aggregate State publisher/resource policy, production Validate-to-Apply handoff, Direct/Merge pre-vote receipt capacity and live shared-lane cutover remain open. L1–L6 and real four/seven-validator qualification remain open.


## Nonblocking preparation and complete World acquisition, September 18

All work and logs remain under the mandated checkout on `optimizations`. MV preparation previously called a blocking publication-version mutex before and after its nonblocking data-writer acquisition. It now uses one typed nonblocking identity check: contention returns the same original journal as Busy, foreign/changed identities return Changed, and a poisoned publication is a local reconstruction failure. Five new controls pass with all 93 MV tests; the tests also prove that writer release precedes installation-guard release.

The successful Core test artifact emitted during failed combined build04 passes 19 of 28 scoped controls: six block-hash, five membership, five complete TriggerSet, two native envelope bounds and one authenticated receipt retry. The overall build failed on twelve stale Kagami API uses. Four native driver tests failed earlier in their common admission fixture because proposal attachments invalidated its empty result; the source now binds exact zero-work typed outputs, re-signs the final proposal and retains the actual runtime sample. Two lifecycle tests still fail the incomplete State publication gate. Historical native merge geometry and two merge/beacon composition cases remain failures. Logs and executable identity are retained in `dist/sumeragi-main-work/publication-driver-controls-build04/`.

The complete World publisher now reacquires all 278 original inventory fields, including the ten-store TriggerSet, under one required installation admission. Every earlier prepared field aborts on a late refusal; the original journals, order, catalog, events and capture guard return together. Private typed wrappers retain original accessors without Any/downcasts, reconstructed World or repeated execution. Publication still requires the enclosing State visibility and exact finality owner. Six controls cover every field being held, every Busy position, final-field identity change, full current/undo images, replacement, original extras, retry and resource guards. Core library checking passed; new test-image comparison errors were corrected after check05/build06 and await build07. The broad default-Core integration check also exposed 49 existing grouped-governance fixture/API errors, retained in check05 rather than hidden behind compatibility helpers.

Kagami now checks immutable proposal commitments and complete typed output shape/cache before projecting Network, Pipeline and Time results. Catalog integration proofs use exact finality-bound canonical execution, complete native input, first finalized admission, pinned four-member committee/PoPs, exact three Commit signatures and regenerated signed RS16. Those source migrations await the coordinated build and runtime controls. The production cutover still requires process-lived ingress/Decision custody, native source plus mandatory beacon composition, preserved Validate-to-Apply ownership and an exact consuming Apply receipt before old fresh QueuePlan signing can retire.

Superseded status and roadmap paragraphs, retained verbatim:

All active Sumeragi edits and validation use `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Integrated World/runtime/hash/membership capture and lifetime-free provider/reputation archive custody remain present. All 88 MV tests, 104 membership/native-preparation source-contract tests and the retired-codec guard pass here. Exact detached membership, block-hash and complete ten-store trigger publication now have consuming preparation/abort operations and regression tests; their new runtime execution is pending. The combined test build exposed a removed trigger-test API and six pointer-cast lint errors; both are corrected, with recompilation pending. The process-lived native driver and actual three-survivor failover controls are registered, but production transport/candidate/Apply activation and mandatory beacon composition are unfinished. Earlier runtime821 passed 597/601 on its separate source; it does not qualify this composition. Aggregate State publication/resource policy, original Validate-to-Apply custody, Direct/Merge pre-vote receipt capacity, carrier-proof custody, live shared-lane cutover and unchanged four/seven-validator qualification remain open. No L1–L6 goal is complete.

The next Sumeragi cutover must carry exact prepared State/resource ownership through validation, cached/recovered markers, voting and Apply; returning only the execution hash drops that owner. Acquire bounded descriptor/capacity resources before voting and preserve typed local deferral with a reachable retry. Prototype530 was withdrawn for post-finality descriptor growth. Retain historical Native authority, stable canonical storage and immutable incarnation directories. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded complete history references; preserve exact merge-source/finality/WSV joins before source release, and charge whole bundles while any route retains them. Defer physical GC with release, snapshot/recovery pins and an instance-local deletion fence. Preserve cross-route closure and drain/signing fences. Remove the post-finality local Queue veto only with the complete consuming path. Continue all edits and validation in `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Complete and test the aggregate consuming State publisher using the retained journals and prepared MV publications; the current checkout passes all 88 MV controls. Finish the original Validate-to-Apply custody and capacity policy, authenticated receipt retry and Direct/Merge pre-vote admission. Preserve independent beacon and key-lifecycle behavior during integration. Connect the process-lived shared lane reducer to transport, exact Decision groups and candidate application while retiring the old fresh-signing authority at one complete cutover. Resolve the retained State ownership failures, carrier proof custody and historical work authority, remove unused instance journals, and qualify one unchanged four/seven-validator fault/restart/final-transaction candidate before closing any liveness goal.


## Physical release ownership, September 19

Earlier current-status paragraphs, preserved verbatim before replacement:

All active Sumeragi edits and validation use `/Users/takemiyamakoto/dev/iroha` on `optimizations`. All 93 MV tests pass, including nonblocking publication-identity contention and poisoned-owner controls. The emitted Core artifact from combined build04 passes all 16 new membership/hash/TriggerSet publication controls, both native frame-bound controls and authenticated receipt retry. Its nine retained failures are four driver-fixture failures before opening, two lifecycle publication failures and three merge/beacon ownership failures; none are counted as passing. The driver admission fixture now reattaches exact zero-work outputs and signs its final proposal. Complete 278-field World preparation/abort and six controls are added; test-image compiler errors are corrected and combined build07 is running. Kagami and catalog/recovery proof consumers now use typed outputs and exact native carrier proofs, pending rebuilt runtime checks. Earlier 104 source-contract passes do not establish runtime progress. Aggregate State authorization/resource policy, original Validate-to-Apply custody, Direct/Merge pre-vote receipt capacity, carrier-proof custody, native/beacon production cutover and unchanged four/seven-validator qualification remain open. No L1–L6 goal is complete.

The next Sumeragi cutover must carry exact prepared State/resource ownership through validation, cached/recovered markers, voting and Apply; returning only the execution hash drops that owner. Acquire bounded descriptor/capacity resources before voting and preserve typed local deferral with a reachable retry. Prototype530 was withdrawn for post-finality descriptor growth. Retain historical Native authority, stable canonical storage and immutable incarnation directories. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded complete history references; preserve exact merge-source/finality/WSV joins before source release, and charge whole bundles while any route retains them. Defer physical GC with release, snapshot/recovery pins and an instance-local deletion fence. Preserve cross-route closure and drain/signing fences. Remove the post-finality local Queue veto only with the complete consuming path. Continue all edits and validation in `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Complete and test the aggregate consuming State publisher using the retained journals and prepared MV publications; the current checkout passes all 93 MV controls and 16 hash/membership/TriggerSet publication controls. Qualify the new complete World preparation and corrected native opening fixture, then join runtime, geometry, exact QC/Kura authority and all resource guards before State visibility. Finish the original Validate-to-Apply custody and capacity policy, authenticated receipt retry and Direct/Merge pre-vote admission. Preserve independent beacon and key-lifecycle behavior during integration. Connect the process-lived shared lane reducer to transport, exact Decision groups and candidate application while retiring the old fresh-signing authority at one complete cutover. Resolve the retained State ownership failures, carrier proof custody and historical work authority, remove unused instance journals, and qualify one unchanged four/seven-validator fault/restart/final-transaction candidate before closing any liveness goal.

## Superseded root evidence, September 19, 2026

The following root-status paragraphs are preserved verbatim from before the current build13 and original-review rerun. Their older pass counts apply only to their recorded source; the current root status carries the remaining failures.

The four complete-input review findings are corrected. Actual 800 KiB admission batching within a 2 MiB carrier, signed 160 KiB gossip within 256 KiB, deferred custody and cold recovery pass their scoped controls. Completed-publication repair also passes its real economic two-cycle regression and all 133 selected Core/61 Torii checks; actual completed-secondary cold archival subsequently passed. These results do not qualify the replacement runtime.

The retained-source/common-tail candidate builds and passes all 594 selected Core tests on 20,548 unchanged local source/config inputs, including 16 new controls and all 21 publication/repair/capacity cases. Its model executable is byte-identical to the prior 345/345 passing artifact; model tests were not rerun. The 19-path runtime/inventory change is integrated into main with independent staged changes preserved. The inventory correction passes 34 source-loader controls in an exact source mirror; full structural output returns to its prior 148 errors. Main composition and full workspace remain uncompiled/unqualified.

All active Sumeragi edits and validation use `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Combined build08 passes with 7,019 unchanged Rust/configuration inputs; 46 fresh Core/catalog controls pass, and nine Kagami controls retain the same executable identity. Combined build09 also passes on 7,022 unchanged inputs; its seven runtime-journal and three hash/membership release controls pass. Current MV passes all 103 tests and strict library/test Clippy. Complete World/runtime/hash/membership preparation now retains original journals on local refusal, and actual physical-lock release supplies async Busy wakeups; poisoned Concread writers are distinct from ordinary contention. The hash read-panic correction and complete State commit/write/lifecycle release instrumentation await combined build10. Five previously observed lifecycle/merge ownership failures remain unresolved, and the broader default Core integration feature selection still exposes stale governance fixtures. Aggregate State authorization/resource policy, original Validate-to-Apply custody, geometry/receipt pre-vote admission, carrier-proof custody, native/beacon production cutover and unchanged four/seven-validator qualification remain open. No L1–L6 goal is complete.


### Superseded build14 root status, September 19, 2026

These are the exact two root status paragraphs before the joint storage-acquisition checkpoint. Their evidence remains scoped to build14.

All active Sumeragi edits and validation use `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Combined build14 passes, including the formerly broken default Core governance target, with 7,211 unchanged Rust/configuration/formal inputs. All 96 focused Core/Kagami/catalog controls pass on unchanged executables. Current physical State-lock release, original journal retention, exact verified-decision binding and geometry admission ordering have scoped runtime evidence. Build14's 47 positive/source-inventory/release-ledger checks pass; build12's broader 158 ownership/mutation/source-inventory checks also pass on their recorded source. MV's 103 tests and strict library/test Clippy pass. These are component checks, not production cutover qualification.

All 17 original-review controls pass on build14: complete sealed inputs, actual 800 KiB admissions batched inside a 2 MiB carrier with deferred custody, signed 160 KiB certified gossip within 256 KiB and completed-repair temporary recovery/tamper checks. The carrier fixture now derives its manifest from the actual catalog and retains the exact four live BLS authorities. Current governance migration compiles and passes 25 of 26 controls, preserving all tests and assertions; the signed-ballot fixture fails at unauthenticated synthetic genesis and its coherent replacement still requires the complete output/State publisher. Five earlier lifecycle/merge ownership failures remain; historical catalog recovery now passes its execution-header/geometry join and reaches the remaining competing-Native output guard. Complete State source/durability/resource authorization, original Validate-to-Apply custody, native/beacon production cutover and unchanged four/seven-validator qualification remain open. No L1–L6 goal is complete.


### Superseded build18 root status and roadmap, September 19, 2026

These are the exact root paragraphs before the source-custody and checkpoint integration. Their evidence remains scoped to build18.

All active Sumeragi edits and validation use `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Combined build18 passes, including default Core governance compilation, with 7,215 unchanged Rust/configuration/formal inputs. All 158 selected runtime controls pass on unchanged executables: 111 composition, 17 original-review and 30 Kura storage regressions. Joint physical acquisition now retains the exact original Kura, decided carrier and all State journals, releases every earlier owner on refusal, and waits on the actual failed lock. Shared State/Kura mutexes notify after physical release. Permanent poison refuses immediately; active pruning remains distinct from abandoned-intent recovery. All 131 selected formal/source-contract checks and scoped Rust formatting pass. Earlier MV's 103 tests and strict library/test Clippy retain their recorded scope. These are component checks, not production cutover qualification.

All 17 original-review controls pass on build18: complete sealed inputs, actual 800 KiB admissions batched inside a 2 MiB carrier with deferred custody, signed 160 KiB certified gossip within 256 KiB and completed-repair temporary recovery/tamper checks. The carrier fixture derives its manifest from the actual catalog and retains the exact four live BLS authorities. Build14's governance migration compiled and passed 25 of 26 controls, preserving all tests and assertions; the signed-ballot fixture still needs authenticated genesis and the complete output/State publisher. Five earlier lifecycle/merge ownership failures remain; historical catalog recovery reaches the competing-Native output guard after its corrected geometry join. Complete source/durability/resource authorization, original Validate-to-Apply custody, native/beacon production cutover and unchanged four/seven-validator qualification remain open. The checkpoint receipt draft remains unregistered, including its unresolved platform durability boundary. No L1–L6 goal is complete.

The next Sumeragi cutover must carry exact prepared State/resource ownership through validation, cached/recovered markers, voting and Apply; returning only the execution hash drops that owner. Acquire bounded descriptor/capacity resources before voting and preserve typed local deferral with a reachable retry. Prototype530 was withdrawn for post-finality descriptor growth. Retain historical Native authority, stable canonical storage and immutable incarnation directories. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded complete history references; preserve exact merge-source/finality/WSV joins before source release, and charge whole bundles while any route retains them. Defer physical GC with release, snapshot/recovery pins and an instance-local deletion fence. Preserve cross-route closure and drain/signing fences. Remove the post-finality local Queue veto only with the complete consuming path. Continue all edits and validation in `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Complete and test the aggregate consuming State publisher using the retained journals and prepared MV publications; the current checkout passes 103 MV controls and strict MV library/test Clippy. Complete World and all four runtime journals have scoped publication evidence, and the corrected native opening/driver controls pass. Carry the passing joint Kura/State physical acquisition, exact retries, decision-binding and execution-header/geometry controls into the aggregate publisher. Join actual retained source/output authority, lease-bound checkpoint durability, geometry/retirement and all resource guards before State visibility; physical writer exclusion alone grants none of these permissions. Finish the original Validate-to-Apply custody and capacity policy, authenticated receipt retry and Direct/Merge pre-vote admission. Preserve independent beacon and key-lifecycle behavior during integration. Connect the process-lived shared lane reducer to transport, exact Decision groups and candidate application while retiring the old fresh-signing authority at one complete cutover. Resolve the retained State ownership failures, carrier proof custody and historical work authority, remove unused instance journals, and qualify one unchanged four/seven-validator fault/restart/final-transaction candidate before closing any liveness goal.


### Superseded build21 root status and roadmap, September 19, 2026

These are the exact root paragraphs before Native source custody and retirement-source-gate qualification. Their evidence remains scoped to build21.

All active Sumeragi edits and validation use `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Combined build21 passes, including default Core governance compilation, with 7,218 unchanged Rust/configuration/formal inputs. All 287 selected runtime controls pass on unchanged executables: 240 composition/output controls, 17 original-review regressions and 30 existing Kura storage regressions. Actual ordinary execution sources, inventory and witness now survive sealing and preparation in one owner; raw State and transaction writes remain closed after transfer. The decided carrier retains the actual checkpoint writer receipt and reauthenticates its original Kura, finality, captured State hash, file and ancestor handles under the joint Kura lease before acquiring State. Exact persistence retries preserve file identity; malformed temporary paths keep resource accounting unavailable until actual re-audit. All 201 selected formal/source-contract controls and formatting of 50 changed Rust files pass. MV's earlier 103 tests and strict library/test Clippy retain their recorded scope. These are component checks, not production cutover qualification.

All 17 original-review controls pass on build21: complete sealed inputs, actual 800 KiB admissions batched inside a 2 MiB carrier with deferred custody, signed 160 KiB certified gossip within 256 KiB and completed-repair temporary recovery/tamper checks. The carrier fixture derives its manifest from the actual catalog and retains the exact four live BLS authorities. Build14's governance migration compiled and passed 25 of 26 controls, preserving all tests and assertions; the signed-ballot fixture still needs authenticated genesis and the complete output/State publisher. Five earlier lifecycle/merge ownership failures remain. Build19's two test migration errors and build20's malformed-temporary regression and three default-stack fixture failures were corrected and rerun in build21 without removing assertions or increasing test stack limits. Complete Native/source, geometry/retirement and resource authorization, original Validate-to-Apply custody, production cutover and unchanged four/seven-validator qualification remain open. Native Windows namespace durability remains unqualified. No L1–L6 goal is complete.

The next Sumeragi cutover must carry exact prepared State/resource ownership through validation, cached/recovered markers, voting and Apply; returning only the execution hash drops that owner. Acquire bounded descriptor/capacity resources before voting and preserve typed local deferral with a reachable retry. Prototype530 was withdrawn for post-finality descriptor growth. Retain historical Native authority, stable canonical storage and immutable incarnation directories. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded complete history references; preserve exact merge-source/finality/WSV joins before source release, and charge whole bundles while any route retains them. Defer physical GC with release, snapshot/recovery pins and an instance-local deletion fence. Preserve cross-route closure and drain/signing fences. Remove the post-finality local Queue veto only with the complete consuming path. Continue all edits and validation in `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Complete and test the aggregate consuming State publisher using the retained journals and prepared MV publications; the current checkout passes 103 MV controls and strict MV library/test Clippy. Complete World and all four runtime journals have scoped publication evidence, and the corrected native opening/driver controls pass. Consume the now-retained ordinary source-prefix owner and exact lease-bound checkpoint receipt with the actual journals. Finish Native source/economic authority, geometry/retirement and bounded capture, file-handle and installation resources before exposing the sole State publication operation. The passing source-custody, exact retry, joint Kura/State and decision-binding controls establish component joins; they do not authorize publication or replace production handoff. Qualify native Windows namespace durability before claiming that storage target. Finish the original Validate-to-Apply custody and capacity policy, authenticated receipt retry and Direct/Merge pre-vote admission. Preserve independent beacon and key-lifecycle behavior during integration. Connect the process-lived shared lane reducer to transport, exact Decision groups and candidate application while retiring the old fresh-signing authority at one complete cutover. Resolve the retained State ownership failures, carrier proof custody and historical work authority, remove unused instance journals, and qualify one unchanged four/seven-validator fault/restart/final-transaction candidate before closing any liveness goal.

### Root views before Native recorded execution qualification (September 19, 2026)

Original `status.md` paragraph:

All active Sumeragi edits and validation use `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Combined build23 passes, including default Core governance compilation, with 7,218 unchanged Rust/configuration/formal inputs. Its unchanged executables pass all 331 selected runtime controls: 240 composition/output, 44 Native source/economic/recovery, 17 original-review and 30 Kura storage controls. The existing Native stage now retains the original verified source vector, first-carrier/body/finality, all route contexts and Decisions after execution; private construction rejoins them to the actual staged batch. Ordinary source/witness custody and exact lease-bound checkpoint reauthentication remain qualified by their existing controls. Both retirement maintenance branches now have distinct source bindings; the proof-ledger gate follows the actual maintenance-to-directory handoff. All 218 selected formal/source controls plus 45 mutation subtests, formatting of 62 changed Rust files and the codec guard pass. The prior 70 Native-preparation source controls passed on build22; its actual-owner and release-gate joins were rerun on build23 after the geometry ledger change. These are component checks; complete production publication and network qualification remain open.

Original `status.md` paragraph:

All 17 original-review controls pass on build23: complete sealed inputs, actual 800 KiB admissions batched inside a 2 MiB carrier with deferred custody, signed 160 KiB certified gossip within 256 KiB and completed-repair temporary recovery/tamper checks. Build22 compiled and passed the preceding 287 runtime controls, but the expanded Native selection exposed 42 default-stack fixture failures and one geometry mutation escaped its source checker. Build23 corrects those failures while preserving all assertions: the original heap State survives separate construction/publication fixture phases, and both authenticated retirement rewrite branches are checked explicitly. The active fixture frame falls from about 919 KiB to 527 KiB without increasing stack limits. Build14's governance migration retains its 25/26 result; the signed-ballot fixture still needs authenticated genesis and the complete publisher. Five earlier lifecycle/merge ownership failures remain. Complete Native witness/control/metadata composition, geometry/retirement and resource authorization, original Validate-to-Apply custody, production cutover and unchanged four/seven-validator qualification remain open. Native Windows namespace durability remains unqualified. No L1–L6 goal is complete.

Original `roadmap.md` paragraph:

The next Sumeragi cutover must carry exact prepared State/resource ownership through validation, cached/recovered markers, voting and Apply; returning only the execution hash drops that owner. Acquire bounded descriptor/capacity resources before voting and preserve typed local deferral with a reachable retry. Prototype530 was withdrawn for post-finality descriptor growth. Retain historical Native authority, stable canonical storage and immutable incarnation directories. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded complete history references; preserve exact merge-source/finality/WSV joins before source release, and charge whole bundles while any route retains them. Defer physical GC with release, snapshot/recovery pins and an instance-local deletion fence. Preserve cross-route closure and drain/signing fences. Remove the post-finality local Queue veto only with the complete consuming path. Continue all edits and validation in `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Complete and test the aggregate consuming State publisher using the retained journals and prepared MV publications; the current checkout passes 103 MV controls and strict MV library/test Clippy. Complete World and all four runtime journals have scoped publication evidence, and the corrected native opening/driver controls pass. Consume the now-retained ordinary source-prefix owner and exact lease-bound checkpoint receipt with the actual journals. Consume the original verified Native groups now retained by the existing stage. Join the actual canonical validator to the same Native execution kernel with one witness scope acquired after State writers; retain distinct output and metadata write cuts, execute mandatory controls and complete economic/settlement authority before opening the live header gate. Separate retirement observation from maintenance only while preserving original route-directory custody and admitting aggregate evidence/writer resources. Finish bounded capture, file-handle and installation resources before exposing the sole State publication operation. The passing source-custody, exact retry, joint Kura/State and decision-binding controls establish component joins; they do not authorize publication or replace production handoff. Qualify native Windows namespace durability before claiming that storage target. Finish the original Validate-to-Apply custody and capacity policy, authenticated receipt retry and Direct/Merge pre-vote admission. Preserve independent beacon and key-lifecycle behavior during integration. Connect the process-lived shared lane reducer to transport, exact Decision groups and candidate application while retiring the old fresh-signing authority at one complete cutover. Resolve the retained State ownership failures, carrier proof custody and historical work authority, remove unused instance journals, and qualify one unchanged four/seven-validator fault/restart/final-transaction candidate before closing any liveness goal.

### Root views before Native recorder-order and economic relay qualification (September 19, 2026)

Original `status.md` paragraph:

All active Sumeragi edits and validation use `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Combined build25 passes Core/Torii/test-network/Kagami/daemon compilation on 7,220 unchanged Rust/configuration/formal inputs. Its exact executables pass 340 distinct selected runtime controls: 240 composition/output/publication, 53 Native source/economic/recovery/recorder, 17 original-review and 30 Kura storage controls. Native recorded replay now consumes the original authenticated sources through one witness scope, actual Network/Pipeline/Time execution, common and Native metadata, output sealing, lane-context finalization and checked capture. Recorder eligibility is checked before State access; the actual recorder is acquired only after State writers. Original settlement hashes prevent metadata substitution, and receipt-free Native Decisions do not fabricate old AMX receipts. All 118 scoped source/inventory/mutation controls and three separate alternate-index closure checks pass; the main index was unchanged. The final candidate reruns the two actual-owner/release-binding joins. Formatting of 68 changed Rust files and the codec guard pass. These are component checks; the recorded owner grants no ValidBlock, resource, publication or Apply authority.

Original `status.md` paragraph:

All 17 original-review controls pass on build25: complete sealed inputs, actual 800 KiB admissions batched inside a 2 MiB carrier with deferred custody, signed 160 KiB certified gossip within 256 KiB and authenticated completed-repair temporary recovery/tamper checks. Build24 compiled and passed 52/53 Native controls; one new fixture exceeded its two-input bound. Build25 tests transfer, rejection and sealed reveal in independent single/atomic fixtures and passes all 53 without stack overrides. Earlier build23 default-stack and retirement-binding corrections remain recorded in the liveness specification. Build14 governance remains 25/26, with authenticated genesis/complete publication still required for the signed-ballot fixture; five earlier lifecycle/merge ownership failures remain. Mandatory Native controls, authenticated suffix-context opening, genuine receipt-bearing relay runtime coverage, scratch-under-recorder concurrency, geometry/retirement and aggregate resource admission, original Validate-to-Apply custody, the consuming publisher, production cutover and unchanged four/seven-validator qualification remain open. Native Windows namespace durability remains unqualified. The source-inventory live-index pytest correctly refuses the untracked new provider; equivalent reviewed closure is checked with a separate index, without relaxing the release gate. No L1–L6 goal is complete.

Original `roadmap.md` paragraph:

The next Sumeragi cutover must carry exact prepared State/resource ownership through validation, cached/recovered markers, voting and Apply; returning only the execution hash drops that owner. Acquire bounded descriptor/capacity resources before voting and preserve typed local deferral with a reachable retry. Prototype530 was withdrawn for post-finality descriptor growth. Retain historical Native authority, stable canonical storage and immutable incarnation directories. Replace mandatory per-route application files with one authenticated carrier proof bundle and bounded complete history references; preserve exact merge-source/finality/WSV joins before source release, and charge whole bundles while any route retains them. Defer physical GC with release, snapshot/recovery pins and an instance-local deletion fence. Preserve cross-route closure and drain/signing fences. Remove the post-finality local Queue veto only with the complete consuming path. Continue all edits and validation in `/Users/takemiyamakoto/dev/iroha` on `optimizations`. Complete and test the aggregate consuming State publisher using the retained journals and prepared MV publications; the current checkout passes 103 MV controls and strict MV library/test Clippy. Complete World and all four runtime journals have scoped publication evidence, and the corrected native opening/driver controls pass. Consume the now-retained ordinary source-prefix owner and exact lease-bound checkpoint receipt with the actual journals. Join the actual canonical validator and original Validate-to-Apply owner to the now-qualified recorded Native replay, retaining the original source groups, one witness and distinct output/metadata cuts. Complete mandatory control composition, authenticated suffix-context opening and genuine receipt-bearing relay qualification. Resolve concurrent scratch-under-recorder acquisition with a coherent nonblocking/refusal or migrated-callsite design; the new recorded-entry check alone is not a global lock-order proof. Preserve bounded resources and complete publication authority before opening the live Native header gate. Separate retirement observation from maintenance only while preserving original route-directory custody and admitting aggregate evidence/writer resources. Finish bounded capture, file-handle and installation resources before exposing the sole State publication operation. The passing source-custody, exact retry, joint Kura/State and decision-binding controls establish component joins; they do not authorize publication or replace production handoff. Qualify native Windows namespace durability before claiming that storage target. Finish the original Validate-to-Apply custody and capacity policy, authenticated receipt retry and Direct/Merge pre-vote admission. Preserve independent beacon and key-lifecycle behavior during integration. Connect the process-lived shared lane reducer to transport, exact Decision groups and candidate application while retiring the old fresh-signing authority at one complete cutover. Resolve the retained State ownership failures, carrier proof custody and historical work authority, remove unused instance journals, and qualify one unchanged four/seven-validator fault/restart/final-transaction candidate before closing any liveness goal.
