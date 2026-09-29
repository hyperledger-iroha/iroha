# ZK runtime boundary follow-up — 2026-09-29

Scope: the first-release ZK remediation in the shared `optimizations` checkout.
The overall goal remains active. This record does not establish release readiness.

## RAM-LFE direct verifier

The internal receipt-proof helper still delegated to a generic native verifier
once envelope metadata and four payload-hash limbs matched. Policy registration,
activation and stateless receipt checks already refused proof mode. No accepting
bypass was demonstrated against the current compiled relation inventory, but a
direct identifier caller should not depend on every outer caller retaining that
refusal.

The shared helper now returns the unavailable program-execution relation error
before reading proof/key bytes. Its generic verifier and four-limb accepting path
are removed. Signed receipts retain their existing signer trust contract.
The [implementation contract](../../../specs/ram_lfe_execution_proof.md) remains
open: this correction does not implement a private execution circuit.

Existing malformed/backend/key/schema/layout/limit negative cases remain at both
RAM-LFE and identifier boundaries, now requiring the unavailable-relation error.
New controls first verify a real native replay-binding proof and then require
RAM-LFE and identifier rejection. Substantive envelope metadata and node-limit
assertions also move to the typed supported-relation verifier's existing native
positive test. Independent source review and scoped Rust formatting/diff checks
pass. **Native execution of this follow-up is pending.**

Exact predecessor/postimage hashes, patch and source-review record are retained
under `dist/zk-remediation/2026-09-29/ram-lfe-direct-refusal`. The frozen network
candidate is unchanged until the complete reviewed amendment is applied.

## Additional checks

- Six retained native Core controls pass for unsigned replay-binding derivation,
  registered-contract entrypoint dispatch, code binding, mandatory STARK replay
  and fee/replay gas limits. The combined selection records 66 passes, two stale
  X509 resource-expectation failures and one explicitly ignored optimized
  diagnostic. This executable predates the RAM-LFE helper follow-up above;
  unrelated SCCP source drift is separately retained. It is component evidence,
  not the pending fixed-candidate Torii/network qualification.
- The CLI release-graph Python module passes all 10 tests using
  `/opt/homebrew/bin/python3.12 -m unittest discover -s pytests/scripts -p test_taira_release_check_cli_graph.py`.
  The first invocation by module path failed to import its sibling helper; the
  discovery invocation fixes test loading without changing production code.
- A cross-SDK/current-source search finds retired `ivm-execution-v1` labels only
  in rejection tests, explicit retired-label documentation and rejection logic.
  It finds no remaining positive producer/registration path under that label.
  The five SDK READMEs now distinguish the backend label from the full canonical
  circuit ID and explain the validator's mandatory execution replay.
- The normal Core debug build completes after correcting the stale derived fixed
  column count to 210. Its subsequent maximum assembly check stops at the
  intentionally stale compiled-profile pin before witness assembly. The blanket
  error conversion calls this `Registration`; this failure alone is not evidence
  of a witness-construction defect. Native profile reconstruction precedes pin
  replacement and a fresh assembly run.
