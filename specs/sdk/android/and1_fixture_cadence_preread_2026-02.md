<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND1 Fixture Cadence Governance Pre-Read — February 2026

**Prepared:** 2026-02-24  
**Review window:** Feb governance sync (2026-02-24 08:30 UTC)  
**Owners:** Android Foundations TL · Release Engineering · SDK Program Lead

This packet satisfies the roadmap hot-list action (“Confirm Norito fixture sync cadence & JDK upgrade policy”) ahead of the February 2026 governance vote. It captures the Tue/Fri 09:00 UTC rotation evidence, summarizes the latest exporter attempt and failure log, and restates the JDK 21 posture so the council can sign off without waiting for ad‑hoc status updates.

> **Superseded on 2026-08-21:** this remains a dated record of the February
> window, but its Android-local generation procedure is retired. Future fixture
> refreshes use `xtask norito-rpc-fixtures --output-root` at two independent
> absent absolute external roots, require identical sealed publications, apply
> the reviewed identity-relative patch, and finish with
> `xtask norito-rpc-verify`. Android only consumes and checks its generated
> mirror.

## 1. Rotation Evidence (Feb 2026 windows)

| Date (UTC) | Owner | Window | Status | Evidence |
|------------|-------|--------|--------|----------|
| 2026-02-17 09:00 | Erika Park | Tue scheduled | No-op — no discriminant/ABI deltas since commit `ae0abecf…` (2025-11-12), so rotation recorded without running exporter. | `artifacts/android/fixture_runs/2026-02-17-noop.md` |
| 2026-02-20 09:00 | Ryo Takahashi | Fri scheduled | No-op — dry-run confirmed exporter readiness but no Norito schema merges, recorded for audit continuity. | `artifacts/android/fixture_runs/2026-02-20-noop.md` |
| 2026-02-24 09:00 | codex-agent | Tue scheduled | Blocked — the now-retired Android-local generator failed because `AccountSuffix` derived `Copy` despite storing `String`; the exporter aborted before writing new fixtures. | `artifacts/android/fixture_runs/2026-02-24-run.log` |

The cadence label (`twice-weekly-tue-fri-0900utc`) and rotation owner metadata remain unchanged in `artifacts/android_fixture_regen_state.json`. Once the upstream `iroha_data_model` fix lands, the Tue/Fri windows can resume emitting fresh timestamps.

## 2. Exporter Failure Summary

- Historical command: the retired Android-local fixture-generation target;
  this record is not a current rerun instruction.
- Failure: `error[E0204]` at `crates/iroha_data_model/src/sns/mod.rs:21` (`Copy` cannot be derived for a struct containing `String`).
- Impact: Tue 2026-02-24 rotation could not regenerate fixtures; the governance packet uses the log as evidence and tracks the fix as a blocking follow-up for the Fri 2026-02-27 window.
- Current mitigation: after correcting the canonical source, render two fresh
  absent absolute external owner publications, compare their
  paths/types/modes/manifests/bytes, apply the reviewed identity-relative patch,
  run `norito-rpc-verify`, and then run `make android-fixtures-check` as a
  consumer gate.
- **Update (2026-02-24 09:30 UTC):** Exporter rerun completed successfully. See `artifacts/android/fixture_runs/2026-02-24-success.md` for the evidence bundle and confirm the cadence state now reflects the new timestamp.

## 3. JDK 21 LTS Posture (Quarterly Review)

- The workspace stays on **JDK 21 LTS** for CI and developer laptops per the policy documented in `specs/android_fixture_changelog.md`.
- Q1 2026 CPU review: staged Azul JDK 21.0.4 in CI under
  `ANDROID_JDK_NEXT=1`; `ci/run_android_tests.sh` and the then-current,
  now-retired Android-local generator completed dry-runs without deterministic
  drift. Promotion was deferred until the exporter regression above cleared so
  governance could review one change at a time.
- Next review checkpoint: April 2026 CPU drop; revisit once Wed 2026-04-08 sync occurs.

## 4. Actions for Governance

1. **Approve cadence evidence:** Accept the Tue/Fri window records above as proof that the twice-weekly rotation process is in place even when no fixture bytes change.
2. **Track exporter fix:** Require the `iroha_data_model` derive regression to be resolved before Fri 2026-02-27; escalate if the fix misses the window.
3. **Reaffirm JDK policy:** Note that the JDK 21 LTS posture remains valid; promotion of newer builds is gated on quarterly reviewers and unrelated to the exporter fix.
4. **Status updates:** Mirror this pre-read into `status.md` and keep `specs/android_fixture_changelog.md` aligned by logging every Tue/Fri window (run, no-op, or failure) until AND7 telemetry GA.

For any current rerun, use the canonical two-root owner procedure, record both
sealed publication identities plus `norito-rpc-verify` and Android consumer-check
results, and append a new row to the changelog.
