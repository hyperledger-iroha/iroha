# Python governance tally integration

Both Python clients now use the same `GovernanceTally` model and response owner
in `python/iroha_torii_client/governance_tally.py`. The low-level client's former
four-field summary and coercing decoder are removed. The full SDK preserves its
raw/typed method split; its typed result is the same class exported by the
low-level package. Neither path fabricates zero votes for an HTTP 404.

The source contract is Torii's `TallyGetResponse` and
`governance_tally_from_view` in `crates/iroha_torii/src/gov.rs`. The six fields
retain exact unquoted u128 counts, their u128 aggregate bound, the exact
referendum selector and the evaluated block's u64 height/lowercase hash pair.
Norito's unsigned JSON writer emits decimal integer tokens. The 128-byte ASCII
selector bound makes a 4 KiB response cap sufficient for maximum-width fields.

`RuntimeGovernanceAuthMixin._governance_tally_payload` owns the shared request.
It uses the existing account authentication and prepared-request signing path,
one-shot transport, no redirects, identity encoding and original streamed bytes.
The prior exact JSON decoder, bounded reader and status-without-body helper were
moved from the full SDK to `_strict_json_response.py`; existing SoraFS callers
retain their previous explicit limits. There is one implementation of each
moved helper and no old tally model alias. Duplicate object keys, numeric
coercion, float/exponent/negative tokens, malformed bodies, surplus/missing
fields and inconsistent anchors reject. Responses close on success, rejection
and stream failure.

The current low-layer response suite passes **91 tests** in 0.41s through normal
package imports, with zero skips/failures:

```sh
PYTHONPATH=python PYTHONDONTWRITEBYTECODE=1 target/first-release-python-governance-host/venv/bin/python -B -m pytest python/iroha_torii_client/tests/governance_tally_response_test.py -q
```

The log is `target/first-release-python-governance-tally-shared-response-tests-2.log`.
These are parser/model and actual Requests-response stream ownership controls,
including late read failures and closure. They perform no native identity or
signer operation. The shared fixtures are hand-authored source-contract vectors;
their native producer comparison and cross-SDK qualification remain outstanding.

The full SDK suite now checks all 14 shared tally vectors through its raw/typed
methods and the low-level typed method, plus identical prepared-request signing
ownership and 404 semantics. It still requires the authentic rebuilt native
wheel. Its earlier normal invocation stopped during collection because
`iroha_native._crypto` was absent; no test passed in that invocation. The original
failure remains in `target/first-release-python-governance-tally-tests.log` and
the original source identity packet remains an earlier source observation.
The source-only README control passed one test before the shared-owner change;
syntax and source audits accompany the superseding packet.

Both suites are included in `ci/check_privacy_python_sdk.sh`. That release gate
retains its authenticated compiler, sealed wheel/installed-file checks and
whole-repository clean-source ABI evidence requirement. The packaged loader,
native ABI and build guards were not changed. A fresh development venv with
hash-pinned dependencies is prepared at
`target/first-release-python-governance-host/venv`; its setup log and identity
record confirm that no native build or installation occurred during preparation.

Development rebuild/installation commands are recorded in
`target/first-release-python-governance-native-rebuild.md`. A later development
wheel test will be scoped execution evidence; it will not establish a signed
release, clean-source provenance, native fixture parity or completed SDK
qualification. The external ABI evidence binds all tracked and nonignored
source paths, so canonical release collection must freeze the whole candidate,
not only these Python files.

## Rebuilt host package execution

The documented native development wheel builds successfully in 66m22s. The
actual native and pure SDK wheels are sealed, preflighted and installed together
in the isolated Python 3.12 environment. Initial installed verification exposed
a verifier bug: its source-only importer concealed the already authenticated
extension from the package's canonical loader. The verifier now returns that
exact retained native module spec and permits only source loaders for other
package children. It does not discover or admit arbitrary native siblings.
The complete existing archive/origin/loader/tamper fixture passes with explicit
suffix-shadow and uninspected-extension rejection controls.

The first installed test run passes 502 tests and rejects 21 stale fixtures.
The fixtures now mark already-buffered Requests bodies as consumed, use the
current canonical ballot-draft response, and derive a real curve public key.
Production response decoding and public-key admission were not relaxed.
The corrected run passes **524 tests**, followed by a second successful strict
installed-package verification. Three transport variants now sign with the real
native Ed25519 implementation and verify the exact request message; altered
methods and referendum selectors reject.

Exact commands, wheel identities and all seven successful stages are under
`target/first-release-python-governance-host/installed-validation-3`. Its
42-file SDK/test observation is
`target/first-release-python-governance-tally-shared-source-3.sha256.txt`
(SHA-256 `a3d13d8cfc9b9e816f612f46e369979e3d026b8d201c7c878573fcd9439fef5f`).
The observed 56-package native dependency inputs remain unchanged through the
build and these tests. Earlier failures are retained in the first two installed
validation packets. Subsequent shared Rust integration requires another native
package build for that later source; this host result does not qualify a final
candidate, other platforms, full proofs or deployment.

The full Cargo/wheel CI self-test separately exposed an outdated canonical graph
pin. The pinned-to-current dependency diff has been reviewed against the local
manifest owners and locked offline metadata. It introduces local crate boundaries
and the licensed local `concread` fork, with no new registry package or changed
registry source/version/checksum. The single accepted graph is now the existing
root lock SHA-256
`398cd15f1b51bc25d673acc766f98c8910446246a2ba33b0e97f17332bf57d40`;
the superseded graph remains a rejection fixture. Its exact installed-test
transcript now includes both governance tally files. A reported successful
self-test was investigated because the actual checkout's HEAD and index still
differ. macOS Bash 3.2 does not terminate on a failed compound test under
`set -e`; bare negated grep checks also cannot rely on errexit. The assertions
now exit explicitly and distinguish a missing pattern from an unreadable input.
Eight new regression methods execute the actual shell guards: all 34 focused
materialization/guard tests pass. The exact footer against real Git correctly
rejects this dirty checkout. The repaired full self-test finishes with exit 1
at that same HEAD/index/worktree equality gate, after its 34 controls and bounded
two-wheel archive/origin/loader/tamper checks pass. Its complete log is
`target/first-release-sdk-lock-owner-review-20260921/full-suite-3.log`. Neither
the earlier apparent success nor these fixture controls establish a clean signed
candidate. Original diagnostics and regression results are under
`target/first-release-sdk-lock-owner-review-20260921/bash-compound-errexit`.
