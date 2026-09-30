# Compile-bloat owner graph review

Continuation of Claude session `72f9ffb9-ff7e-4c95-85b5-4f004d06c958`.
The four new workspace units are the state-free privacy and timed-OVN owners,
plus `irohad_lib` and `iroha_cli_lib`. The existing `irohad` and `iroha_cli`
package names now select thin executable packages; consumers select their
libraries directly. Compiler ownership and Core ZK work predates this census.

The reviewed manifest-source graph counts declarations, including repeated
dependencies now explicitly owned by separate units. It does not measure
compiled size or build time. Every limit is reset to the exact current
observation, without headroom. Previous ceilings are preserved in
`dependency_budget.before.json.txt`; they are not a reconstructed measurement
of the previous feature-resolved graph.

| Scope | Previous required local ceiling | Current required locals | Previous required edge ceiling | Current required edges |
| --- | --- | --- | --- | --- |
| CLI shipping | 66 | 69 | 758 | 808 |
| Daemon shipping | 66 | 68 | 813 | 852 |
| All workspace targets | 110 | 114 | 1,689 | 1,780 |

All model, model-base and Rust SDK scope metrics remain unchanged. All external
package counts remain unchanged. Comparing the current lock to the starting
commit also finds no added or removed registry/Git package identity. Privacy
retains its actual IVM execution-proof and FASTPQ transform dependencies;
the extraction does not establish an IVM-free graph. Native/JS/Python
production consumers drop full `iroha_core`, while Core keeps committed-state
authority. Core's direct Orchard dependencies are optional and selected by
its existing `privacy-release-evidence` feature because their remaining Core
uses are evidence-only; the privacy owner still ships Orchard verification.

All package denies and configured architecture selections are preserved.
`irohad_lib` is added to the existing node-execution layer so renaming the
runtime library cannot evade those boundaries. `ownership-review.json.txt`
records every changed metric and every current local manifest hash; the full
source reports before and after the exact refresh are retained separately.
The reviewed manifest fingerprint is
`sha256:012887fb3aa8c8cdcc5dc938a809169ac01c2b3cefb58ece9c7d4139b70f143c`.

Compiling the moved privacy tests exposed their allocation-fixture dependency.
The owner now declares the existing `iroha_allocation` package as a direct
dev-only dependency. Reversing that one declaration reproduces the preceding
privacy manifest hash exactly. No shipping scope, package count, or external
edge changes; all-target required/declared edges become 1,781/1,950. The
preceding exact budget, both reports, and this isolated delta are retained in
the `fixture-owner` records. The final reviewed fingerprint is
`sha256:0231fd1c0ff91e67e08f81a634d0014e2867862e6275530f5797e3f50f4cb30c`.

This audit establishes source ownership and budget accounting. It does not
qualify release readiness or compiler-memory limits.
