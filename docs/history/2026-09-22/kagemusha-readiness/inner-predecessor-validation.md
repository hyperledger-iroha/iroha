## Current validation boundary — 2026-09-22

Default Axiom validation now records 43 distinct passing test functions across
**two windows**, with identical captured production inputs. The first selected
42 tests and finished with 41 passes and one failed protocol-fault assertion.
The correction changed only test code: it recognizes the remaining RNG subcalls
within one field sample while requiring rejection before the next protocol step.
The second window passed that corrected test and a new satisfiable whole-PLONK
test. Each window retained unchanged source and executable bytes during its run;
the dated [opening validation note](../docs/history/2026-09-22/kagemusha-readiness/opening-validation.md)
records the original failure, source-map differences, commands, hashes and scope.
This is not one 43-test run. A durable no-multicore rerun passed all 42 selected
functions with zero failures or ignores and unchanged source/binary bytes; its
255-input map matches the default correction window. The selection includes the
new positive whole-PLONK fixture and explicitly excludes the exhaustive source-read
fault matrix already passed under default. The first no-multicore attempt stopped
without a final receipt and remains interrupted/unqualified. These receipts bind
the guarded-P candidate; later inner-IPA work requires separate validation.

The guarded-P differential fixture deliberately uses an unsatisfied constant-one
gate to exercise inverse tails. Its full byte equality and direct opening
verification are not positive whole-PLONK acceptance. The separate satisfiable
square/lookup/copy fixture passes the real whole-PLONK verifier in both Pasta
fields and direct, default and hybrid instance modes (six cases in one test),
with changed-public-input, corruption and truncation rejection. Its inner IPA
handoff remains test-only and uses the ordinary unguarded implementation.

Earlier default and no-multicore builds each passed the same 27 scalar/blind
component tests (54 executions, 27 distinct functions) on 248 matching captured
inputs; the [scalar/blind note](../docs/history/2026-09-22/kagemusha-readiness/scalar-blind-validation.md)
retains that separate candidate. These component results do not qualify the
complete guarded stored prover, authenticated Core/SDK integration, final-artifact
monetary proofs, device operation or whole-process resource limits.

The earlier active/current sections and root status row are preserved
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
replace complete proofs, workspace checks, canonical release provenance,
measured full-process resources, qualified hardware, or independent review.

