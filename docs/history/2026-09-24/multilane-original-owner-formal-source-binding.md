# Multilane original-owner formal source binding, 2026-09-24

Scope: the existing `optimizations` checkout. This is a source-to-model
correspondence repair for current Core code, not a proof of the full Native
protocol or a release-candidate receipt.

The canonical structural checker now binds the signed ordinary carrier through
its pre-funded original State owner, including pristine/revert and normal
execution starts. It also binds the funded committed-evidence prune-key
construction and its order of allocation before a borrowed State scan. The
pristine NPoS preparation/application, candidate attachment, and peer-rejection
classifiers preserve typed local capacity refusals. The whole-item pins for
`State::construct_acquired_block`, its field carrier, and configured lane
geometry match the current source after the admission cuts. The signed
original-owner rewind relation has mutation controls for the delegated
wrappers, sole acquired State, and charged carrier handoff.

Validation on this source:

- `python3 scripts/formal/check_sumeragi_v2_multilane_models.py` passed the
  five refinement kernels and composed in-flight relation as a **structural
  source binding**. Output: `target/formal-check-after-evidence-helper-final.log`.
- The focused pristine local-refusal selector passed **8/8**, including
  mutations that replace local capacity classification with candidate or peer
  rejection. Output: `target/formal-pristine-local-refusal-tests-fixed.log`.
- The reserve-before-State-scan mutation passed **1/1**. Output:
  `target/formal-pristine-reserve-before-scan.log`.
- `python3 -m json.tool`, `python3 -m py_compile`, and `git diff --check`
  passed for the edited contract and test inputs.

The checker does not extract a production execution trace, complete the
unfinished 18-step Apalache run, prove TLC/TLAPS obligations, or supply
four-validator fault/restart and resource qualification. F02, F03, and F13
remain open.
