# JavaScript runtime-input integration checkpoint

Work remained in `/Users/takemiyamakoto/devstuff/iroha` on `optimizations` at
`1b8e5f92b6dadb1dcdd16f945286560e31b347ff`. The reviewed ten-file change
adds a bounded, byte-exact Node 24 runtime input graph and registers its checks
in both SoraFS release workflows. Application receipt:
`target/first-release-node24-runtime-application-20260922.json`, SHA-256
`967bbd6039faae11fae8fca74bebb6f65001a6c263dc5e73cf6ec78788056ee8`.

The input verifier requires an independently supplied expected manifest digest,
strict canonical JSON and a complete thin arm64 Mach-O image and load graph.
It checks exact image bytes, install names, ordered candidate outcomes, aliases,
reachability and bounded parser work. It rejects unsupported loader cases rather
than inferring runtime authority from the presence of a file. The new checks
are registered alongside the existing automation, workflow, parent-input,
file-input and ABI controls. No release-source seal was repinned.

The integrated run passed 1,735 distinct tests and all 5,205 setup, call and
teardown phases, with no errors, failures, skips or deselections. Automation,
shell syntax and the scoped diff check also passed. It observed 1,173 source
files, 4,373 tool files and 331 actually loaded Python modules with no drift.
The source-observed receipt is
`target/first-release-node24-runtime-integrated-validation-20260922/receipt.json`,
SHA-256 `9e65582be51c46e636f31749e791cc0db5c2696f7f63910bdfaf35ddac89ed84`.

The observed local 20-image Node runtime still fails this strict contract on
two Brotli shared `@rpath` loads. Actual loader resolution, process custody,
native addon execution, installed SDK parity and release qualification remain
open. The passing pure-input tests do not turn that rejected runtime into an
approved artifact.
