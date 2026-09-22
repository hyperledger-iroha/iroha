Fresh default and no-multicore Axiom builds each passed the same 27 selected
tests with zero failed or ignored tests (54 executions, not 54 distinct tests):
four scalar/opening-source tests, two new blind-fold
tests with ten retained retirement checks, one quotient-commitment differential,
five selector checks and five indexed-key scanner checks. The selected tests
cover ordinary scalar bytes/order and source/blind equivalence in both Pasta
fields, bounded copying, failure/unwind cleanup and preserved owner identity.
All 248 captured source/build inputs and each compiled executable remained
unchanged during its compilation and execution. See the
[dated validation note](../docs/history/2026-09-22/kagemusha-readiness/scalar-blind-validation.md)
for the exact commands, hashes and scope. Both modes used the same captured
source map. The later P-continuation and verifier edge-case repair are outside
this tested candidate. These checks do not execute a complete stored proof,
Core/SDK integration, device qualification or whole-process resource measurement.
