## Current validation boundary — 2026-09-22

The first guarded-inner default window compiled successfully but is **incomplete**:
11 test functions executed, 10 passed and one failed, with the 259 captured inputs
and executable unchanged. All seven inner tests passed, including a satisfiable
square/lookup/copy fixture with real whole-PLONK acceptance, dense bytes and RNG
agreement in both Pasta fields and three instance modes (six cases). The zero-u
verifier rejection and two ordinary prover controls also passed. The verifier's
k0/nonzero acceptance control exposed a singleton recursive-FFT empty-stage panic;
a subsequent overbroad selector stopped the remaining groups. No overall 24-test
pass is claimed. A separate production vendor-library check passed in 9.112 s.
See the [inner validation note](../docs/history/2026-09-22/kagemusha-readiness/inner-validation.md)
for exact evidence, scope and retained failure.

The FFT identity fix and its two tests remain staged, not installed or qualified.
The next selection narrows the ordinary commitment filter to IPA and contains 26
functions; corrected default and no-multicore qualification remain pending. An
accidental second worker started against the unchanged candidate after the
installation guard rejected the staged replacement; its setup-error receipt is
retained and duplicate executions are not new corrected-candidate qualification.
These are standalone vendor-library checks, not root-workspace/Core integration.

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

