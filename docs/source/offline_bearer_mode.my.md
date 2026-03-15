---
lang: my
direction: ltr
source: docs/source/offline_bearer_mode.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 099558fc1dc290237cdb8803e3440a774c8fbce36e5765c06ccd19d35181a1a3
source_last_modified: "2026-01-05T09:28:12.034080+00:00"
translation_last_reviewed: 2026-02-07
title: Offline Allowance Bearer Mode Guidance
---

Status: 🈴 OA6 deliverable — apps can now declare whether offline allowances are destined for
ledger reconciliation or will circulate entirely off-ledger.

This note clarifies when operators may treat offline allowances as bearer instruments, what
disclosures are required, and how SDKs expose the new circulation mode toggles added in
`IrohaSwift.OfflineWallet` and `org.hyperledger.iroha.android.offline.OfflineWallet`.

## 1. Why circulation modes exist

- **Ledger-reconcilable mode** keeps the treasury view in sync with on-ledger commitments. Receipts
  are expected to be submitted back to Torii once devices regain connectivity, allowing the ledger to
  enforce counter monotonicity and commitment deltas.
- **Pure offline mode** is meant for short-lived pilots or tightly scoped programs (e.g., offline
  vouchers at events) where operators intentionally forgo ledger re-entry. In this case allowances
  behave like cash: once a device is lost or compromised there is no canonical record to claw funds
  back, and treasury teams must treat the issued value as outstanding bearer liability.

## 2. When each mode is acceptable

| Scenario | Recommended mode | Notes |
|----------|------------------|-------|
| Day-to-day wallet flows, regulated deployments, merchant settlements | Ledger reconcilable | Submit receipts within the operator’s defined SLA (default 7 days). Enable audit logging + WAL export so regulators can replay deposits. |
| Disaster drills where Torii is intentionally unreachable but devices will reconnect | Ledger reconcilable (with deferred uploads) | Flip to offline-only **only** if the drill explicitly documents how and when reconciliation resumes. |
| Pop-up events, prepaid vouchers, or offline loyalty campaigns with capped float | Offline only | Must ship explicit warnings to users, capture treasury approval, and document how much float is outstanding at any given time. |
| Air-gapped devices for testing QA harnesses | Offline only | Gate the mode behind internal builds; do not expose to production users. |

Bearer circulation remains the exception:

- **Short-lived excursions.** Use bearer mode only when connectivity or jurisdiction rules make
  on-ledger deposits impossible for a limited time (hours or days). Long-lived bearer deployments
  treat receipts as final cash and do not require on-ledger reconciliation; if operators later opt
  back into ledger-reconcilable mode, publish a transition plan and reconciliation window.
- **Treasury caps.** Treasury owners must cap the outstanding allowance per merchant and document the
  limit so finance teams can book the exposure just like physical cash drawers.
- **Counter telemetry stays active.** Even when deposits are deferred, wallets must keep advancing
  their hardware counters and export digests so dupes/replays are detectable between merchants and
  if reconciliation is later re-enabled.

## 3. Risk & disclosure checklist (Offline-only mode)

1. Treasury/endowment owners approve the outstanding float and understand it is booked as a liability
   until redeemed or expired.
2. UX copy explains that **lost devices cannot be recovered via Torii**, and redemption requires the
   local journal or paper wallets.
3. Merchants must provide manual verification flows (QR, short code, etc.) because Torii cannot
   authoritatively answer balance queries.
4. Operators must publish a contact channel for dispute resolution plus an expiration schedule so
   regulators know when float is cleared.
5. Monitoring hooks (`offline.float_outstanding`, `offline_only_wallets`) should alarm when float
   exceeds the authorised envelope.
6. Counter digests (`OfflineCounterSummary`) must continue to publish via
   `iroha offline summary export --output counters.json` or `/v2/offline/summaries` so merchants can
   reject duplicated `(certificate_id, counter)` tuples even while the ledger is unaware.

## 4. SDK toggles

Both SDKs now surface an explicit circulation mode so UX layers can show warnings and gate Torii
calls:

### Swift

```swift
let wallet = try OfflineWallet(
    toriiClient: torii,
    auditLoggingEnabled: true,
    circulationMode: .ledgerReconcilable) { mode, notice in
        bannerView.show(title: notice.headline, message: notice.details)
    }

// Flip into pure offline mode for a capped pilot
wallet.setCirculationMode(.offlineOnly)

if wallet.requiresLedgerReconciliation {
    try await wallet.fetchTransfers(params: .init(limit: 25))
} else {
    logger.notice("Skipping Torii calls: wallet circulating offline only")
}
```

`OfflineWalletCirculationMode` also exposes `notice.headline/details` so apps can localise banners
without hard-coding copy.

### Android

```java
OfflineWallet wallet =
    new OfflineWallet(
        toriiClient,
        auditLogger,
        OfflineWallet.CirculationMode.LEDGER_RECONCILABLE,
        (mode, notice) -> uiThread.post(() -> banner.show(notice.headline(), notice.details())));

wallet.setCirculationMode(OfflineWallet.CirculationMode.OFFLINE_ONLY);

if (wallet.requiresLedgerReconciliation()) {
  wallet.fetchTransfersWithAudit(params);
} else {
  telemetry.emit("offline_only_wallet", wallet.auditLogFile().toString());
}
```

Apps that do not provide a listener can still call `wallet.circulationModeNotice()` when rendering
settings screens.

### Wallet configuration requirements

1. **Explicit toggle.** Wallet UX must surface a "Skip ledger reconciliation (bearer mode)" switch
   that describes the risk (missing fraud checks, treasury write-offs, potential clawbacks). Keep the
   default **off** and gate production builds behind policy checks.
2. **Persistent journaling.** `iroha::offline::OfflineJournal` (Rust reference client),
   `IrohaSwift.OfflineJournal`, and the Android `OfflineJournal` must stay enabled so every spend is
   chained/HMACed even when the device never posts a bundle.【crates/iroha/src/offline/mod.rs:1】【IrohaSwift/Sources/IrohaSwift/OfflineJournal.swift:19】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineWallet.java:19】
3. **Audit logging.** Keep `OfflineAuditLogger` turned on whenever regulation permits so support
   teams can export `{sender,receiver,asset,amount,timestamp}` rows and reconcile bearer receipts with
   treasury ledgers.【IrohaSwift/Sources/IrohaSwift/OfflineAuditLogger.swift:31】【java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineAuditLogger.java:19】
4. **Counter digests.** Schedule `iroha offline summary export --output counters.json` (or query
   `/v2/offline/summaries`) to publish the latest `OfflineCounterSummary` digests for every wallet so
   merchants can verify `(certificate_id, counter)` pairs before accepting receipts.【crates/iroha_cli/src/offline.rs:110】

## 5. Treasury & accounting implications

- Record outstanding float per controller in the treasury ledger along with the note “offline bearer
  liability”. The balance should never exceed the cap authorised during governance review.
- Capture attestations from finance/compliance leads before enabling offline-only mode in production.
- If the mode is later switched back to ledger-reconcilable, schedule an explicit reconciliation
  window (e.g., “all receipts must be uploaded within 72 hours”) and re-enable audit logging
  defaults.
- **Balance sheets:** Treat outstanding offline allowances as cash-on-hand; the exported counter
  digests prove how much of each allowance has been consumed even before the ledger learns about the
  delta.
- **Write-offs:** If a bearer-mode deployment loses devices or detects tampering, treasury must
  decide whether to claw back the impacted allowances or write them off just like lost cash drawers.
- **Regulator evidence:** Store the WAL + audit log exports in the same evidence bucket as Torii
  transcripts so investigators can replay the spend history if reconciliation resumes.

## 6. UX copy recommendations

| Location | Copy (English baseline) |
|----------|-------------------------|
| Mode toggle warning | “Offline-only circulation disables ledger reconciliation. Lost or stolen devices cannot be recovered through Torii.” |
| Treasury dashboard | “Offline bearer float: <value>. Confirm risk approvals before increasing the cap.” |
| Merchant receipt | “Redeem before <date>. Verification is manual; retain this receipt as proof of payment.” |

Translate copy per locale but retain the explicit “cannot be recovered via Torii” clause. Embed deep
links to the operator’s dispute policy in both Swift and Android settings panes.

## 7. Evidence & next steps

- SDK coverage: `IrohaSwift/Sources/IrohaSwift/OfflineWallet.swift`,
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/offline/OfflineWallet.java`.
- Tests enforce the mode toggles (`IrohaSwift/Tests/.../OfflineWalletModeTests.swift`,
  `java/iroha_android/src/test/.../OfflineWalletTest.java`).
- Link this note from `docs/source/offline_allowance.md` and surface the toggles in the Swift/Android
  SDK guides so auditors can trace OA6 to live code.

## 8. Returning to ledger reconciliation

If operators choose to resume ledger-reconcilable mode:

1. Flip the wallet toggle back to “submit deposits online”.
2. Export the pending WAL entries and audit logs so the reconciliation window has a deterministic
   transcript.
3. Use the SDK bundle builders to construct `OfflineToOnlineTransfer` payloads that cover the entire
   delta accumulated during bearer mode, then submit them through Torii. The ledger validates the
   counters against the digests that were previously published, so any tampering detected during the
   offline window leads to an immediate rejection.
4. Archive the exported digests/logs alongside the block heights that recorded the reconciliation
   bundles.

## 9. Operational checklist

- [ ] Merchants understand bearer-mode risk and can flip the toggle themselves.
- [ ] Counter digest exports are automated (CLI job or Torii query) and distributed to every receiver.
- [ ] WAL + audit log exports are collected at least once per shift and uploaded to the regulator
      evidence bucket.
- [ ] Treasury knows the outstanding allowance ceiling per merchant and records it daily.
- [ ] Run `scripts/check_norito_bindings_sync.py` and `cargo test -p iroha_cli offline` when updating
      wallet tooling so the CLI export + SDK helpers stay aligned with ledger enforcement.
