# Kaigi F10 historical reporter source audit

2026-09-24, `optimizations`. This is a source-backed release blocker, not a
qualification receipt. The existing focused Kaigi checks validate live
admission, relay key retention, and feedback lifecycle. They do not authenticate
who signed a retained report after restart.

The live [`ReportKaigiRelayHealth` handler](../../../crates/iroha_core/src/smartcontracts/isi/kaigi.rs)
admits only an authority sharing the current call host's active, typed
account-ID rekey lineage, requires an active call and a relay in its manifest,
and writes `reported_by` from that authority. The
[`AccountRekeyRecord`](../../../crates/iroha_data_model/src/account/rekey.rs)
separates `AccountIdRekey` from `AliasReassignment`, and the
[`persisted_kaigi_rekey_graph`](../../../crates/iroha_core/src/smartcontracts/isi/kaigi.rs)
uses only the former as continuity edges. These are live-state checks at the
admitting block; the persisted rekey record has an ordered provenance vector
but no transition height or block hash.

At restore, [`KaigiRelayFeedback`](../../../crates/iroha_data_model/src/kaigi.rs)
has only relay, call, reporter, status, observation time and notes. The
[`collect_kaigi_account_dependencies_from_domains`](../../../crates/iroha_core/src/smartcontracts/isi/kaigi.rs)
scan checks the feedback key, registered relay home, duplicate relay,
authenticated ledger-time ceiling, referenced call, creation time and end time.
It never compares `reported_by` with the call host or an authenticated report
operation. The scan receives domain metadata, not a Kura block source, and it
runs for both the current and latest undo domain layer. A substituted reporter
with all other fields unchanged therefore passes these structural checks.
The emitted [`KaigiRelayHealthSummary`](../../../crates/iroha_data_model/src/events/data/events.rs)
also omits reporter, notes, signed transaction identity and instruction position;
it cannot independently authenticate the retained row.

Checking today's rekey graph against the stored reporter would be unsound as a
release fix. A later valid rekey can make an earlier unauthorized account appear
continuous; an alias reassignment must not confer authority; and after an ended
call, account and alias cleanup can discard lineage that was valid at report
time. The signed transaction alone proves a submitter, not that the exact
instruction succeeded or that its authority was the host at that block. The
existing [role-11 historical execution verifier](../../../crates/iroha_core/src/query/stream_token_authority/historical_execution.rs)
and [same-State finality reader](../../../crates/iroha_core/src/query/signer_finality.rs)
show reusable finality and result-proof boundaries, but there is no Kaigi
operation row or direct instruction coordinate for them to verify today.

The V1 implementation cut should make the retained feedback and an exact source
reference one atomic state mutation. A direct-only signed instruction is the
smallest auditable interface: bind network, finalized block identity, entrypoint
index, direct instruction index, signed authority, complete feedback fields and
the resulting row digest. The executor must provide that coordinate from the
same authenticated source it executes. A State-level verifier must use the
canonical `SignedBlockWire`, matching Kura/QC finality, successful ordered
output, and bounded historical host/rekey state at that height. It must reject
missing, forked, failed, stale and mismatched source evidence in both current
and undo layers. If IVM-nested reports remain admitted, an authenticated
deterministic execution trace is additionally required; a self-declared source
field cannot substitute for it. Since this is the first release, retire the
old unbound row layout rather than accepting both representations.

Adversarial tests needed for that cut include a legitimate pre-rotation report,
an authorized successor after rotation, a reporter authorized only by a *later*
rotation, alias reassignment, changed notes/status/relay/call, a signed but
failed report, duplicate instruction positions, absent or forked finalized
blocks, and restart with different current/undo rows. All cases must retain the
unchanged resource ceilings and deterministic per-row replay bounds. No HSM is
required for the source or signer; signatures, finality and exact successful
execution are required. This audit changed no runtime path and ran no Cargo
tests. Historical reporter authorization and Kaigi release admission remain
open.
