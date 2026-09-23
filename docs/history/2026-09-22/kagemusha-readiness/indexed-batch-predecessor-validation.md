## Current validation boundary — 2026-09-22

The fresh Kotlin/JVM KAGEMUSHA selection executes **57 tests: 33 pass, 24 fail,
zero skipped**, with all 699 captured SDK/build/fixture inputs unchanged. Every
failure reports unavailable ABI-23 native account-address validation; these are
not evidence of 24 distinct product assertion defects. The
[Kotlin validation note](../docs/history/2026-09-22/kagemusha-readiness/kotlin-core-validation.md)
records the actual JDK 21 command, all JUnit results and exact hashes. A
current-source native bridge rebuild and rerun are required; this snapshot does
not bind the Rust native dependency graph or qualify JNI/device execution.

The later indexed-reader candidate passes the same **18 functions in default and
no-multicore builds (36 executions)**: four new reader tests and fourteen structured
codec regressions. All 262 captured inputs match across both windows and the
0.311 s non-test library check, which reuses a fresh Cargo artifact. The
[indexed-reader note](../docs/history/2026-09-22/kagemusha-readiness/indexed-reader-validation.md)
records this separate candidate. A consecutive-target regression across the
n−1→n permutation-coset boundary is staged under the shared vendor-source hold;
it is not applied or qualified by these tests.

The preceding guarded-inner candidate passes the same **27 distinct test functions in
default and no-multicore builds: 54 executions, zero failures or ignored tests**.
All 260 captured source/build inputs match across both windows and the separate
non-test vendor-library check, which passes in 1.124 s; each test executable remains
unchanged during its window. The selection includes guarded arithmetic, owner
failure/erasure controls, ordinary IPA/multiopening regressions, zero-challenge
rejection, empty/singleton FFT identities and the k0 IPA basis empty product.

The satisfiable square/lookup/copy fixture reaches real whole-PLONK acceptance
through guarded inner IPA, with dense proof-byte, transcript and next-RNG agreement
in both Pasta fields and three instance modes (six generic cases). This does not
qualify final KAGEMUSHA recursive monetary proofs. These checks use the standalone
vendor manifest/lockfile, not the root-workspace/Core dependency graph.

The [inner validation note](../docs/history/2026-09-22/kagemusha-readiness/inner-validation.md)
retains every earlier failure: the first window executed 11 functions (10 passed,
one failed), the accidental unchanged-candidate second window executed 24 (23
passed, one failed), and the third executed 26 (25 passed, one failed). The third
window installed the FFT fix but exposed a separate k0 empty-product assertion in
`compute_s`; the final candidate fixes that assertion and adds its regression.
The earlier selector/setup failures remain recorded; they are not overall passes.

The earlier guarded-P candidate records 43 distinct passing functions across two
default windows and 42 passing no-multicore functions, retaining the original
failed assertion and interrupted first no-multicore attempt. Its positive fixture
uses a test-only ordinary inner IPA; the new guarded-inner positive fixture above
is separate. Its deliberately unsatisfied inverse fixture's byte equality and
opening verification are not positive whole-PLONK acceptance. The
[opening note](../docs/history/2026-09-22/kagemusha-readiness/opening-validation.md)
preserves that candidate's exact scope. Earlier default and no-multicore builds
each passed the same 27 scalar/blind functions (54 executions, 27 distinct); the
[scalar/blind note](../docs/history/2026-09-22/kagemusha-readiness/scalar-blind-validation.md)
retains that separate candidate. These earlier receipts do not qualify later
inner-IPA source changes or final recursive monetary proofs.

The replaced active/current sections and root status row are preserved
byte-for-byte in the
[dated archive](../docs/history/2026-09-22/kagemusha-readiness/README.md).
The previously cited `target/kagemusha-validation/stored-prover-next-window-20260912`,
`target/kagemusha-validation/20260912-source-window`,
`target/kagemusha-main-native-jvm-validation-r5` and
`target/kagemusha-sdk-security-parity-validation-r1` directories are absent.
Their recorded historical test totals cannot be independently rechecked here or
used to qualify today's source. Finding entries and test counts below preserve
earlier scoped work records; they are not fresh validation of this source.

Fresh qualification must retain the exact candidate, dependency/lock inputs,
compiled artifacts, commands and results. Focused vendor or host tests cannot
replace authenticated Core/SDK integration, final monetary proofs, workspace
checks, canonical release provenance, measured full-process resources, qualified
hardware, or independent review.
