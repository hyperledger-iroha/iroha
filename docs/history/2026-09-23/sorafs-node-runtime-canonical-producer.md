# Node 24 canonical runtime input producer

The pure Darwin arm64 Node 24 input owner now derives its canonical manifest
from supplied original image bytes, integer mode claims and literal alias
targets. `produce_node_runtime_manifest` takes no edge list. It sorts original
paths, computes SHA-256/size claims, derives alias resolutions and every
direct or inherited `@rpath` candidate through the verifier's shared bounded
graph routine, and re-parses the canonical JSON. The producer performs no
filesystem lookup, process launch or mapped-image observation. Its computed
manifest digest identifies content but is not an independent approval pin.

The verifier uses that same graph routine before comparing the exact declared
edge list. A stale manifest with omitted inherited slots still fails. For the
recorded 20-image local bundle under
`target/first-release-node24-runtime-inputs-20260922`, the original manifest
digest `6ffb821bec56ed303329c35b841a2c01fe2e1db1afe6d51ab9eec15e8e29cf92`
remains rejected. Supplying its same 20 original byte strings and mode claims
to the pure producer yields diagnostic manifest digest
`8a5f68d1ca035abf3de72010cdd6d906f2eec05b5047af43723535cfd9e7e51b`:
both Brotli shared `@rpath/libbrotlicommon.1.dylib` edges acquire the two
missing null-resolved Node ancestor slots. Reframing those bytes with the
derived manifest passes only the pure 20-image, 74-edge relation. This
diagnostic does not certify the four absent leaves, physical aliases, actual
dyld loadability/cache choice, mapped images, installed package, child process
or candidate provenance.

Focused validation: `python3 -m pytest
scripts/tests/sorafs_javascript_runtime_inputs_test.py
scripts/tests/sorafs_javascript_runtime_custody_test.py -q` passed 146/146;
`python3 -m py_compile` passed for the changed Python source and tests. A
broader adjacent JavaScript Python run initially passed 191 and failed nine
`sorafs_javascript_parent_input_test.py` cases at the fixed child tool digest
check. At that point `sorafs_javascript_child_session.mjs` hashed to
`7d171155645f220e94fc3d313e22e539048ba6f6dd9e0e383a63412a8e5e5f00`, while
`CHILD_TOOL_SHA256` still expected
`1bef3d1fe4a765a4d401c550bb82d545a509d9fc45a151cb9fcee7f8f19e79d0`.
Those nine failures preceded the runtime-input owner.
The single source difference from the reviewed `1bef3d...` copy retained under
`target/first-release-javascript-parent-root-review-20260922/pure-parser/tool-originals`
is a rejection-only retired native export prefix: the old literal
`connect_norito_offline_cash_` became the equivalent `retiredPrefix` expression
used by the sole ABI23 checker. The current child ABI contract explicitly
requires that expression and rejects mutating its order or removing its check;
the focused child/ABI23 policy suites passed 21/21. All other seven selected
child-tool files are byte-identical to that reviewed copy and match their
existing hashes. This was the only change in the eight-file selection.

Under the subsequent source-tool repair, only the fixed
`sorafs_javascript_child_session.mjs` hash in
`scripts/sorafs_javascript_child_tools.py` was updated to the current bytes.
All eight fixed child-tool source hashes now match their selected files. The
nine formerly failing parent-input cases pass, as do all 33/33 parent-input
tests and 214/214 combined Python parent, ABI23 policy, runtime-input,
runtime-custody and child-process tests. Source-only Node checks in
`sorafs_javascript_child_contract_test.mjs` and
`sorafs_javascript_child_input_test.mjs` pass 48/48 under the available Node
26.9.0 host. A wider Node 26 run produced 53 passes and 147 failures, mostly
at intentional selected-Node24 guards; it is not Node24 qualification. No
runtime-image, candidate-approval, release or operator pin was changed. The
child tool row changes the derived private child-input bytes and digest;
historical receipts remain historical, and this check does not authenticate a
future final candidate while concurrent integration continues.

The recorded old bundle must remain rejected. Production still requires an
independently reviewed complete pin, physical original/absence/alias custody,
same-process mapped-image observation, installed native/SDK parity and signed
release evidence on one candidate.
