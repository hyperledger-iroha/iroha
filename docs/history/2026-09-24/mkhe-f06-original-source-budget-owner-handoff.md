# F06 original source-budget owner handoff, 2026-09-24

This `optimizations` cut keeps the original `GlobalLookupCommitmentSessionLiveV1`
resource ledger under the retained source-opening session. A private
`RnsNativeSourceWithOriginalOpeningsV1` type carries a repeatable source
snapshot together with the complete `GlobalLookupSourceOpeningMaterialV1`;
neither the single opening inventory nor its budget is extracted or copied.
Its read and structural-receipt failures poison the combined owner. Budget
borrowing requires a valid opening record and the completed Q-mask phase.

The claimed-qPCS transition now requires a distinct move-only
`RnsNativeQpcsInitialWorkAdmittedStartedV2`. Its admission method debits the
existing original ledger for the conservative 320-leaf initial Merkle-opening
hash work before the public read or qPCS authentication. Capacity refusal
returns the same started owner and unchanged ledger for local retry. Once the
qPCS transition begins, downstream failures are terminal for that owner; the
code does not expose a re-entrant authenticated-read path. No independent
resource ledger, receipt, authority, limit, or fallback was added.

Focused validation on the combined source:

- `scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_zkp_halo2 --lib qpcs_work_refusal_keeps_original_completed_source_and_opening_inventory -- --nocapture`: 1 passed. The test checks early-phase refusal, exact one-over work refusal without a counter change, retained Q-mask owner, and identity with the original U15-table reservation.
- `scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_zkp_halo2 --lib initial_opening_admission_precedes_public_read_on_the_same_source_owner -- --nocapture`: 1 passed. The source-bound transition test checks the admission typestate and that no public read or qPCS authentication occurs in admission.
- The changed `iroha_zkp_halo2` library compiled from the corrected source in the focused test run. Scoped formatting and `git diff --check` passed.

The combined owner deliberately has **no production constructor**. The live
source/publication correspondence remains uninhabited, so this type cannot
authenticate source-to-opening identity or admit a production composite.
The helper charges only the initial opening hashes. The current initial-tree
construction exceeds the unchanged 128-billion-work whole-proof limit, and
FRI, transcript, source replay, memory, spool, I/O, and verifier-ingress
accounting remain incomplete. TODO: establish the authenticated common source
lifecycle, redesign the source-bound qPCS commitment/evaluation relation,
meter the complete proof before effects, and qualify the consuming composite.
F06 and the release gate remain open.
