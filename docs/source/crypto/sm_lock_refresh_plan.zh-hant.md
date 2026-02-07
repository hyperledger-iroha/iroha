---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_lock_refresh_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3065571b34a226a5871c4fb68063f9419e48074b20096de215f440bdf54a4e59
source_last_modified: "2025-12-29T18:16:35.943236+00:00"
translation_last_reviewed: 2026-02-07
---

//! Procedure for scheduling the Cargo.lock refresh required by the SM spike.

# SM Feature `Cargo.lock` Refresh Plan

The `sm` feature spike for `iroha_crypto` originally could not complete `cargo check` while `--locked` was enforced. This note records the coordination steps for a sanctioned `Cargo.lock` update and tracks the current status of that need.

> **2026-02-12 update:** Recent validation shows the optional `sm` feature now builds with the existing lockfile (`cargo check -p iroha_crypto --features sm --locked` succeeds in 7.9 s cold/0.23 s warm). The dependency set already contains `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `primeorder`, `sm2`, `sm3`, `sm4`, and `sm4-gcm`, so no immediate lock refresh is required. Keep the procedure below on standby for future dependency bumps or new optional crates.

## Why the refresh is needed
- Earlier iterations of the spike required adding optional crates that were missing from the lockfile. Current lock snapshots already include the RustCrypto stack (`sm2`, `sm3`, `sm4`, supporting codecs, and AES helpers).
- Repository policy still blocks opportunistic lockfile edits; if a future dependency upgrade is necessary, the procedure below remains applicable.
- Retain this plan so the team can execute a controlled refresh when new SM-related dependencies are introduced or existing ones need version bumps.

## Proposed coordination steps
1. **Raise request in Crypto WG + Release Eng sync (owner: @crypto-wg lead).**
   - Reference `docs/source/crypto/sm_program.md` and note the optional nature of the feature.
   - Confirm there are no concurrent lockfile change windows (e.g., dependency freezes).
2. **Prepare patch with lock diff (owner: @release-eng).**
   - Execute `scripts/sm_lock_refresh.sh` (after approval) to update only the required crates.
   - Capture `cargo tree -p iroha_crypto --features sm` output (script emits `target/sm_dep_tree.txt`).
3. **Security review (owner: @security-reviews).**
   - Verify new crates/versions match the audit register and licensing expectations.
   - Record hashes in supply-chain tracker.
4. **Merge window execution.**
   - Submit PR containing only the lockfile delta, dependency tree snapshot (attached as artifact), and updated audit notes.
   - Ensure CI runs with `cargo check -p iroha_crypto --features sm` before merge.
5. **Follow-up tasks.**
   - Update `docs/source/crypto/sm_program.md` action item checklist.
   - Notify SDK team that the feature can be compiled locally with `--features sm`.

## Timeline & owners
| Step | Target | Owner | Status |
|------|--------|-------|--------|
| Request agenda slot in next Crypto WG call | 2025-01-22 | Crypto WG lead | ✅ Completed (review concluded spike can proceed without refresh) |
| Draft selective `cargo update` command + sanity diff | 2025-01-24 | Release Engineering | ⚪ On standby (reactivate if new crates appear) |
| Security review of new crates | 2025-01-27 | Security Reviews | ⚪ On standby (reuse audit checklist when refresh resumes) |
| Merge lockfile update PR | 2025-01-29 | Release Engineering | ⚪ On standby |
| Update SM program doc checklist | After merge | Crypto WG lead | ✅ Addressed via `docs/source/crypto/sm_program.md` entry (2026-02-12) |

## Notes
- Keep any future refresh restricted to the SM-related crates listed above (and supporting helpers like `rfc6979`), avoiding workspace-wide `cargo update`.
- If any transitive dependencies introduce MSRV drift, surface it before merge.
- Once merged, enable an ephemeral CI job to monitor build times for the `sm` feature.
