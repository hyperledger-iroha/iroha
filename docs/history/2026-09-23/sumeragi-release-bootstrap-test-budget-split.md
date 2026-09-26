# Sumeragi release-bootstrap test budget split — 2026-09-23

On the existing `optimizations` checkout, the five direct bootstrap test
functions were initially moved verbatim to
`pytests/scripts/sumeragi_v2_release_bootstrap_direct_cases.py`. The canonical
`sumeragi_v2_release_bootstrap_test.py` module loads that component through its
existing lexical test-component mechanism at the original definition point.
This initially preserved its fixtures, assertions, parametrization, and
collection order. The source file fell from 3,063 to 2,875 lines, below the
3,000-line test limit; the new component began at 192 lines. The before/after
split collection contained the same 255 node IDs in the same order.

The shared fixture was then updated to supply the current parser's three
protected scaling source files and SHA-256 commitments, original BLAKE3
dependency source, machine/storage labels, observation overhead, and explicit
protected Cargo home. The plan and budget are copied from the existing
canonical preflight fixture files; the handoff helper is copied from the
production source. The retired five scaling runner-environment variables were
removed from fixture arguments, and their allowlist test now asserts rejection.
No production validation or compatibility path changed.

The canonical test module is now 2,891 lines, the direct-cases component
273 lines, and the environment-cases component 73 lines. The initial split
preserved all 255 pytest node IDs. Fixture modernization replaced one
obsolete positive environment-forwarding case with five exact retired-name
rejection cases, four protected-source cases, and one static production-runner
source case. The zero-timeout case
now uses a valid one-second bound to test natural completion after expiry, and
a separate case proves zero rejects before spawn. The current collection has
265 node IDs; 253 of the original 255 remain. The two changed IDs are the
retired positive forwarding case and the corrected timeout parametrization.
The combined source-budget checker reports 234 findings
across 12,863 files and 166 exceptions, compared with 235 findings across
12,859 files before this split. Neither bootstrap test file appears in the
findings. No baseline or limit was raised, and the overall gate remains open.

Before fixture repair, the six focused cases supplied by the five moved
functions reported three passes and three failures. Running the same selectors
against an unmodified `HEAD` copy in the same test directory reproduced those
exact three failures: the old allowlist expected retired scaling environment
variables, and the two zero-timeout cases expected a runtime failure where
current validation rejects nonpositive bounds first. The allowlist assertion
and the bounded-helper setup now match the first-release contract; their four
focused cases pass 4/4, including the new explicit zero-bound rejection case.
The new fixture-source success and three protected-digest rejection tests pass
4/4. Another 20 early negative cases, 13 boundary-rejection cases, and all
12 exact-retired-name/lookalike environment cases pass with the required parser
inputs present. These tests stop before scaling handoff or Cargo execution.

Before fixture repair, the full bootstrap test module ran with **49 passed and
206 failed** in 255.55 seconds. A representative shared-fixture failure is
`test_success_authenticates_then_launches_exactly_once`: the current bootstrap
parser requires ten scaling-plan/budget/handoff/source/machine arguments, while
`Fixture.arguments()` supplies none of them. The subprocess exits with argparse
status 2 before authenticating or launching. That same case and exact missing
argument list fail against unmodified `HEAD` test source. The parser-input
failure class is repaired in the fixture; the original 206 failing cases were
not all classified. A complete rerun is deferred while the shared Cargo build
slot is active: the updated fixture can reach a real scaling handoff, and the old
terminal-success fixture does not yet provide a complete selected source or
scaling preflight. No synthetic execution evidence is accepted. The complete
release gate remains unqualified.

The fixture can install the exact current production runner bytes and rebind
the candidate approvals to that runner identity. Its static handoff-source
test passes 1/1, but the old `test_success_authenticates_then_launches_exactly_once`
still installs the synthetic runner, which writes a terminal-looking receipt
without invoking the inherited scaling gate. It must not be treated as a
production success test. Current production selected-source and handoff seams
pass 7/7 focused tests each without invoking the production runner or Cargo.
The focused complete-preflight owner selection reports 13 passes and one
failure: `preflight source inventory changed`, before spawning a child. The
[separate diagnostic](sumeragi-preflight-source-inventory-drift.md) records the
33 exact path/digest differences among 269 protected source rows. The inventory
must be regenerated only after a candidate is frozen, followed by the actual
preflight and scaling run against that same candidate.
