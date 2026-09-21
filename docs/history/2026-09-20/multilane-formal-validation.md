# Multilane bounded formal validation and source-binding repair

The unchanged canonical in-flight first-release TLC corpus passed on
2026-09-20. Its fixed configuration checked all 20 invariants through complete
finite exploration: 3,251,669 generated states, 280,818 distinct states, no
remaining states, and depth 36. All 25 required mutation configurations produced
their exact expected invariant violations. The complete 26-case runner exited
zero in 92.67 seconds; its recorder accepted every case with no consistency
errors and all captured inputs unchanged.

The command was `bash scripts/formal/run_sumeragi_v2_inflight_first_release.sh`,
using the pinned TLA2Tools 1.7.4 JAR with SHA-256
`936a262061c914694dfd669a543be24573c45d5aa0ff20a8b96b23d01e050e88`
and the local OpenJDK 21.0.12.1 executable. The canonical one-worker runner,
fixed model/configuration, invariant list and mutation inventory were unchanged.
The retained packet contains the actual executed model/configuration/JAR copies,
tool identity, commands, raw output, counterexamples and acceptance records:

`target/first-release-tlc-canonical-20260920/sumeragi-v2-tlc-754m2i_e/`

This is bounded abstract in-flight layout evidence. It does not establish Rust
transition refinement, Native application evidence, TLAPS, Verus, Apalache or
release readiness.

## Source-binding correction

An independently captured structural preflight first failed with exit one in
144.77 seconds. Its original stdout/stderr, command, input hashes and result
remain under `target/first-release-tlc-canonical-20260920/preflight-*`.
Two source-binding records were stale:

- Rustfmt split the retained descriptor's existing `pop().expect(...)` across
  lines. The exact token in the Native preparation contract and binding ledger
  now matches that formatting. The full operation, vacant-subject/owner checks,
  explicit validation-error return and original-owner installation remain bound.
- `host/contract_state_namespace.rs` became the production include owner of four
  namespace methods. All four definitions are byte-identical to their former
  definitions in `host.rs` at `9f9cfcc61f^`. The include inventory adds precisely
  that provider; every other inventory entry and prior host provider retains
  its order. The inventory digest was recomputed only after checking this exact
  one-entry delta.

The method extraction changes no modeled observable. Separately, current
`host.rs` adds the SoraFS final-promotion authority and account-custody prefixes
to its opaque list. Those are separate behavior changes, not part of the
extraction-equivalence claim. Every prior opaque prefix, including QueuePlan's
protected markers, remains unchanged and ordered; the read-only list is
byte-identical. This repair changes no production Rust, TLA model, invariant or
configuration.

The new current-owner test row checks all four functions' unique ownership in
the expanded source. Existing missing-file, removed-edge, duplicate-edge and
extra-provider negatives also exercise this provider. The retained descriptor's
`pop()` to `first()` negative remains intact.

The focused run initially passed 22 controls and failed one stale inventory
reference to the account-profile regression. The actual test is now named
`account_profile_validation_rejects_foreign_permission_payloads`; its native
`NotPermitted`, unchanged metadata and equal serial/worker-result assertions
remain present. Only the Python inventory's symbol reference was corrected.
The repeated selection passed **all 23 tests**, zero skipped, in 51.01 seconds.
An earlier broad 426-case Native-preparation selection was interrupted after
28 completed passes when narrowed to this repair; it is not a full-suite pass.

`python3 -I -S scripts/formal/check_sumeragi_v2_multilane_models.py` then has
a captured zero exit in 148.75 seconds, with empty stderr and unchanged captured
checker, ledger and production-owner inputs. Its inputs are unaffected by the
subsequent test-inventory name correction. `git diff --check` also passed for
the scoped repair. Exact focused selectors, before/after source hashes,
method-equivalence comparison, inventory delta, patch, both failed runs and
successful results remain under:

`target/first-release-source-binding-drift-20260920/`

## Outstanding qualification

The full 17-configuration TLC matrix and separate 106-case refinement mutation
corpus were not executed by this scoped run. Strict TLAPS against pinned commit
`3ab43c7ff31db4ced850619d4746fa4c841a7681`, pinned Verus
`0.2026.05.31.5dd6d83`, derived cross-tool evidence and final-source production
traces remain required by `ci/check_sumeragi_formal.sh`.

The canonical Apalache 0.52.2 gate remains unpassed, including the unchanged
18-step in-flight bound and all 20 named invariants with `after` scheduling.
The earlier canonical attempt terminated with exit 143 after 411,980 seconds;
the bounded quantified and fixed-domain diagnostics hit their declared CPU
limits, and the experimental arrays diagnostic returned RuntimeError/exit 12.
None supplies positive evidence. Original failures and troubleshooting packets
are indexed by
`target/first-release-apalache-diagnostics-20260920/summary.json`. No additional
Apalache attempt or canonical model rewrite was made during this validation.
