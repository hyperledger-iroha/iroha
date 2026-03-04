import Foundation

/// Describes how an offline allowance is expected to circulate.
public enum OfflineWalletCirculationMode: String, Codable, CaseIterable {
    case ledgerReconcilable
    case offlineOnly

    /// Returns `true` when the wallet must submit bundles back to the ledger.
    public var requiresLedgerReconciliation: Bool {
        self == .ledgerReconcilable
    }

    /// Human-readable notice that SDKs can surface whenever the mode flips.
    public var notice: OfflineWalletCirculationNotice {
        switch self {
        case .ledgerReconcilable:
            return OfflineWalletCirculationNotice(
                headline: "Ledger reconciliation required",
                details: "This wallet is expected to submit offline receipts back to Torii within the policy window. " +
                         "Keep audit logging enabled and plan for controller treasury checks so allowances never exceed registered commitments.")
        case .offlineOnly:
            return OfflineWalletCirculationNotice(
                headline: "Pure offline circulation",
                details: "Allowances in this mode behave as bearer instruments; receipts are final between participants " +
                         "and will not be re-entered on-ledger. Operators must accept treasury risk, warn users that " +
                         "recovery depends on the local journal, and disclose that lost devices cannot be reconciled " +
                         "via Torii.")
        }
    }
}

/// Friendly text that wallets can display when a circulation mode is selected.
public struct OfflineWalletCirculationNotice: Equatable {
    public let headline: String
    public let details: String

    public init(headline: String, details: String) {
        self.headline = headline
        self.details = details
    }
}

public typealias OfflineWalletModeChangeHandler = (OfflineWalletCirculationMode, OfflineWalletCirculationNotice) -> Void

/// High-level helper that talks to Torii's offline endpoints and manages optional audit logging.
public final class OfflineWallet {
    public let toriiClient: ToriiClient
    private let auditLogger: OfflineAuditLogger
    private var mode: OfflineWalletCirculationMode
    private let modeChangeHandler: OfflineWalletModeChangeHandler?
    private let verdictJournal: OfflineVerdictJournal
    private let revocationJournal: OfflineRevocationJournal
    private let counterJournal: OfflineCounterJournal

    public init(toriiClient: ToriiClient,
                auditLoggingEnabled: Bool = false,
                auditStorageURL: URL? = nil,
                auditLogger: OfflineAuditLogger? = nil,
                circulationMode: OfflineWalletCirculationMode = .ledgerReconcilable,
                onCirculationModeChange modeChangeHandler: OfflineWalletModeChangeHandler? = nil,
                verdictJournal: OfflineVerdictJournal? = nil,
                revocationJournal: OfflineRevocationJournal? = nil,
                counterJournal: OfflineCounterJournal? = nil) throws {
        self.toriiClient = toriiClient
        self.mode = circulationMode
        self.modeChangeHandler = modeChangeHandler
        if let logger = auditLogger {
            self.auditLogger = logger
            logger.setEnabled(auditLoggingEnabled)
        } else {
            self.auditLogger = try OfflineAuditLogger(storageURL: auditStorageURL,
                                                      isEnabled: auditLoggingEnabled)
        }
        self.verdictJournal = try verdictJournal ?? OfflineVerdictJournal()
        self.revocationJournal = try revocationJournal ?? OfflineRevocationJournal()
        self.counterJournal = try counterJournal ?? OfflineCounterJournal()
        notifyModeChangeIfNeeded()
    }

    // MARK: Audit Logging

    public var isAuditLoggingEnabled: Bool {
        auditLogger.currentEnabled()
    }

    public var auditLogURL: URL {
        auditLogger.storageURL
    }

    public func setAuditLoggingEnabled(_ enabled: Bool) {
        auditLogger.setEnabled(enabled)
    }

    public func recordAuditEntry(txId: String,
                                 senderId: String,
                                 receiverId: String,
                                 assetId: String,
                                 amount: String,
                                 timestampMs: UInt64? = nil) {
        let recordedAt = timestampMs ?? OfflineWallet.currentTimestampMs()
        let entry = OfflineAuditEntry(txId: txId,
                                      senderId: senderId,
                                      receiverId: receiverId,
                                      assetId: assetId,
                                      amount: amount,
                                      timestampMs: recordedAt)
        auditLogger.record(entry: entry)
    }

    public func exportAuditEntries() -> [OfflineAuditEntry] {
        auditLogger.exportEntries()
    }

    public func exportAuditJSON(prettyPrinted: Bool = true) throws -> Data {
        try auditLogger.exportJSON(prettyPrinted: prettyPrinted)
    }

    public func clearAuditLog() throws {
        try auditLogger.clear()
    }

    // MARK: Circulation mode

    /// Returns the current circulation mode.
    public var circulationMode: OfflineWalletCirculationMode {
        mode
    }

    /// Returns the notice tied to the current circulation mode.
    public func circulationModeNotice() -> OfflineWalletCirculationNotice {
        mode.notice
    }

    /// Indicates whether the wallet must reconcile receipts back to the ledger.
    public var requiresLedgerReconciliation: Bool {
        mode.requiresLedgerReconciliation
    }

    /// Updates the circulation mode and notifies observers if it changes.
    public func setCirculationMode(_ newMode: OfflineWalletCirculationMode) {
        guard mode != newMode else { return }
        mode = newMode
        notifyModeChangeIfNeeded()
    }

    private func notifyModeChangeIfNeeded() {
        guard let handler = modeChangeHandler else { return }
        handler(mode, mode.notice)
    }

    // MARK: Torii helpers

    public func fetchAllowances(params: ToriiOfflineListParams? = nil) async throws -> ToriiOfflineAllowanceList {
        try await toriiClient.listOfflineAllowances(params: params)
    }

    /// Fetches allowances and records the returned verdict metadata inside the local journal.
    @discardableResult
    public func fetchAllowancesRecordingVerdicts(params: ToriiOfflineListParams? = nil,
                                                 recordedAt timestampMs: UInt64? = nil) async throws
        -> ToriiOfflineAllowanceList {
        let result = try await fetchAllowances(params: params)
        try recordVerdictMetadata(from: result, recordedAt: timestampMs)
        return result
    }

    /// Issues and registers an offline allowance. When `recordVerdict` is `true`, the issued
    /// certificate is cached in the verdict journal using `recordedAt` (or now when omitted).
    /// For iOS App Attest flows, pass the top-up `challengeHash` as `attestationNonce`.
    @discardableResult
    public func topUpAllowance(draft: OfflineWalletCertificateDraft,
                               authority: String,
                               privateKey: String,
                               attestationNonce: Data? = nil,
                               recordVerdict: Bool = true,
                               recordedAt timestampMs: UInt64? = nil) async throws -> ToriiOfflineTopUpResponse {
        let response = try await toriiClient.topUpOfflineAllowance(draft: draft,
                                                                   authority: authority,
                                                                   privateKey: privateKey,
                                                                   attestationNonce: attestationNonce)
        if recordVerdict {
            try recordVerdictMetadata(from: response.certificate, recordedAt: timestampMs)
        }
        return response
    }

    /// Issues and registers a renewed offline allowance. When `recordVerdict` is `true`, the issued
    /// certificate is cached in the verdict journal using `recordedAt` (or now when omitted).
    /// For iOS App Attest flows, pass the renewal `challengeHash` as `attestationNonce`.
    @discardableResult
    public func topUpAllowanceRenewal(certificateIdHex: String,
                                      draft: OfflineWalletCertificateDraft,
                                      authority: String,
                                      privateKey: String,
                                      attestationNonce: Data? = nil,
                                      recordVerdict: Bool = true,
                                      recordedAt timestampMs: UInt64? = nil) async throws -> ToriiOfflineTopUpResponse {
        let response = try await toriiClient.topUpOfflineAllowanceRenewal(certificateIdHex: certificateIdHex,
                                                                          draft: draft,
                                                                          authority: authority,
                                                                          privateKey: privateKey,
                                                                          attestationNonce: attestationNonce)
        if recordVerdict {
            try recordVerdictMetadata(from: response.certificate, recordedAt: timestampMs)
        }
        return response
    }

    /// Reprovisions an existing allowance by rotating the Pedersen commitment while
    /// preserving the current allowance amount and policy.
    @discardableResult
    public func reprovisionAllowance(certificateIdHex: String,
                                     currentCertificateRecordData: Data,
                                     newCommitment: Data,
                                     authority: String,
                                     privateKey: String,
                                     lineage: OfflineCertificateLineage,
                                     recordVerdict: Bool = true,
                                     recordedAt timestampMs: UInt64? = nil) async throws -> ToriiOfflineTopUpResponse {
        let response = try await toriiClient.reprovisionOfflineAllowance(certificateIdHex: certificateIdHex,
                                                                         currentCertificateRecordData: currentCertificateRecordData,
                                                                         newCommitment: newCommitment,
                                                                         authority: authority,
                                                                         privateKey: privateKey,
                                                                         lineage: lineage)
        if recordVerdict {
            try recordVerdictMetadata(from: response.certificate, recordedAt: timestampMs)
        }
        return response
    }

    public func fetchTransfers(params: ToriiOfflineListParams? = nil) async throws -> ToriiOfflineTransferList {
        try await toriiClient.listOfflineTransfers(params: params)
    }

    public func fetchSummaries(params: ToriiOfflineListParams? = nil) async throws -> ToriiOfflineSummaryList {
        try await toriiClient.listOfflineSummaries(params: params)
    }

    /// Fetches summaries and records the returned counter checkpoints inside the local journal.
    @discardableResult
    public func fetchSummariesRecordingCounters(params: ToriiOfflineListParams? = nil,
                                                recordedAt timestampMs: UInt64? = nil) async throws
        -> ToriiOfflineSummaryList {
        let result = try await fetchSummaries(params: params)
        try recordCounterSummaries(from: result, recordedAt: timestampMs)
        return result
    }

    /// Fetches offline transfers and records each bundle in the audit log when enabled.
    public func fetchTransfersWithAudit(params: ToriiOfflineListParams? = nil) async throws -> ToriiOfflineTransferList {
        let list = try await fetchTransfers(params: params)
        recordTransfersForAudit(list)
        return list
    }

    public func fetchRevocations(params: ToriiOfflineRevocationListParams? = nil) async throws
        -> ToriiOfflineRevocationList {
        try await toriiClient.listOfflineRevocations(params: params)
    }

    /// Fetches revocations and records them inside the local deny list.
    @discardableResult
    public func fetchRevocationsRecordingDenyList(params: ToriiOfflineRevocationListParams? = nil,
                                                  recordedAt timestampMs: UInt64? = nil) async throws
        -> ToriiOfflineRevocationList {
        let list = try await fetchRevocations(params: params)
        try recordRevocations(from: list, recordedAt: timestampMs)
        return list
    }

    public func fetchOfflineState() async throws -> ToriiOfflineStateResponse {
        try await toriiClient.getOfflineState()
    }

    /// Fetches the full offline state and refreshes local revocations, counters, and verdict metadata.
    @discardableResult
    public func syncOfflineState(recordedAt timestampMs: UInt64? = nil) async throws
        -> ToriiOfflineStateResponse {
        let state = try await fetchOfflineState()
        try recordRevocations(from: state, recordedAt: timestampMs)
        try recordCounterSummaries(from: state, recordedAt: timestampMs)
        try recordVerdictMetadata(from: state, recordedAt: timestampMs)
        return state
    }

    // MARK: Receipt helpers

    /// Builds a signed receipt and optionally records it in the journal and audit log.
    /// The `chainId` is bound into the platform challenge hash for replay protection.
    @available(macOS 10.15, iOS 13.0, *)
    @discardableResult
    public func buildSignedReceipt(
        txId: Data? = nil,
        chainId: String,
        receiverAccountId: String,
        amount: String,
        invoiceId: String,
        platformProof: OfflinePlatformProof,
        platformSnapshot: OfflinePlatformTokenSnapshot? = nil,
        senderCertificate: OfflineWalletCertificate,
        signingKey: SigningKey,
        journal: OfflineJournal? = nil,
        recordedAt timestampMs: UInt64? = nil
    ) throws -> OfflineSpendReceipt {
        let recordedAt = timestampMs ?? OfflineWallet.currentTimestampMs()
        let receipt = try OfflineReceiptBuilder.buildSignedReceipt(
            txId: txId,
            chainId: chainId,
            receiverAccountId: receiverAccountId,
            amount: amount,
            invoiceId: invoiceId,
            platformProof: platformProof,
            platformSnapshot: platformSnapshot,
            senderCertificate: senderCertificate,
            signingKey: signingKey,
            issuedAtMs: recordedAt
        )
        _ = try counterJournal.advanceCounter(certificate: senderCertificate,
                                              platformProof: platformProof,
                                              recordedAtMs: recordedAt)
        if let journal {
            try journal.appendPending(receipt: receipt, timestampMs: recordedAt)
        }
        recordReceiptAudit(receipt, timestampMs: recordedAt)
        return receipt
    }

    /// Records a receipt in the audit log when enabled.
    public func recordReceiptAudit(_ receipt: OfflineSpendReceipt,
                                   timestampMs: UInt64? = nil) {
        guard isAuditLoggingEnabled else { return }
        let recordedAt = timestampMs ?? OfflineWallet.currentTimestampMs()
        recordAuditEntry(txId: receipt.txId.hexUppercased(),
                         senderId: receipt.from,
                         receiverId: receipt.to,
                         assetId: receipt.assetId,
                         amount: receipt.amount,
                         timestampMs: recordedAt)
    }

    /// Records a single transfer bundle in the audit log when enabled.
    public func recordTransferAudit(_ transfer: ToriiOfflineTransferItem) {
        guard isAuditLoggingEnabled else { return }
        let summary = transfer.firstReceiptSummary()
        let senderId = summary?.senderId.irohaNonEmptyTrimmed ?? transfer.receiverId
        let receiverId = summary?.receiverId.irohaNonEmptyTrimmed ?? transfer.depositAccountId
        let assetId = summary?.assetId?.irohaNonEmptyTrimmed ?? transfer.assetId.irohaNonEmptyTrimmed ?? ""
        let amount = summary?.amount.irohaNonEmptyTrimmed
            ?? transfer.claimedDelta.irohaNonEmptyTrimmed
            ?? transfer.totalAmount

        recordAuditEntry(
            txId: transfer.bundleIdHex,
            senderId: senderId,
            receiverId: receiverId,
            assetId: assetId,
            amount: amount
        )
    }

    private func recordTransfersForAudit(_ list: ToriiOfflineTransferList) {
        guard isAuditLoggingEnabled else { return }
        for item in list.items {
            recordTransferAudit(item)
        }
    }

    // MARK: Verdict metadata

    public var verdictJournalURL: URL {
        verdictJournal.storageURL
    }

    @discardableResult
    public func recordVerdictMetadata(from allowances: ToriiOfflineAllowanceList,
                                      recordedAt timestampMs: UInt64? = nil) throws
        -> [OfflineVerdictMetadata] {
        let recordedAt = timestampMs ?? OfflineWallet.currentTimestampMs()
        return try verdictJournal.upsert(allowances: allowances.items, recordedAtMs: recordedAt)
    }

    @discardableResult
    public func recordVerdictMetadata(from state: ToriiOfflineStateResponse,
                                      recordedAt timestampMs: UInt64? = nil) throws
        -> [OfflineVerdictMetadata] {
        let recordedAt = timestampMs ?? state.nowMs
        return try verdictJournal.upsert(rawAllowances: state.allowances, recordedAtMs: recordedAt)
    }

    /// Records verdict metadata from a certificate-issue response (for flows that issue without listing allowances).
    @discardableResult
    public func recordVerdictMetadata(from issued: ToriiOfflineCertificateIssueResponse,
                                      recordedAt timestampMs: UInt64? = nil) throws
        -> [OfflineVerdictMetadata] {
        let recordedAt = timestampMs ?? OfflineWallet.currentTimestampMs()
        let record = try Self.makeRawAllowanceRecord(from: issued)
        return try verdictJournal.upsert(rawAllowances: [record], recordedAtMs: recordedAt)
    }

    private static func makeRawAllowanceRecord(from issued: ToriiOfflineCertificateIssueResponse) throws -> ToriiJSONValue {
        let certificate = try issued.decodeCertificate()
        let canonicalValue = try Self.canonicalCertificateValue(from: certificate)
        var object: [String: ToriiJSONValue] = [
            "certificate": canonicalValue,
            "remaining_amount": .string(certificate.allowance.amount),
            "controller_display": .string(certificate.controller)
        ]
        if let verdictId = certificate.verdictId {
            object["verdict_id_hex"] = .string(verdictId.hexUppercased())
        }
        if let attestationNonce = certificate.attestationNonce {
            object["attestation_nonce_hex"] = .string(attestationNonce.hexUppercased())
        }
        if let refreshAtMs = certificate.refreshAtMs {
            object["refresh_at_ms"] = .string(String(refreshAtMs))
        }
        return .object(object)
    }

    private static func canonicalCertificateValue(from certificate: OfflineWalletCertificate) throws -> ToriiJSONValue {
        let data = try JSONEncoder().encode(certificate)
        return try JSONDecoder().decode(ToriiJSONValue.self, from: data)
    }

    public func verdictWarnings(warningThresholdMs: UInt64 = OfflineVerdictJournal.defaultWarningThresholdMs,
                                now: Date = Date()) -> [OfflineVerdictWarning] {
        verdictJournal.warnings(nowMs: OfflineWallet.timestampMs(for: now),
                                warningThresholdMs: warningThresholdMs)
    }

    public func verdictWarning(for certificateIdHex: String,
                               warningThresholdMs: UInt64 = OfflineVerdictJournal.defaultWarningThresholdMs,
                               now: Date = Date()) -> OfflineVerdictWarning? {
        verdictJournal.warning(for: certificateIdHex,
                               nowMs: OfflineWallet.timestampMs(for: now),
                               warningThresholdMs: warningThresholdMs)
    }

    public func verdictMetadata(for certificateIdHex: String) -> OfflineVerdictMetadata? {
        verdictJournal.metadata(for: certificateIdHex)
    }

    // MARK: Counter checkpoints

    public var counterJournalURL: URL {
        counterJournal.storageURL
    }

    @discardableResult
    public func recordCounterSummaries(from summaries: ToriiOfflineSummaryList,
                                       recordedAt timestampMs: UInt64? = nil) throws
        -> [OfflineCounterCheckpoint] {
        let recordedAt = timestampMs ?? OfflineWallet.currentTimestampMs()
        return try counterJournal.upsert(summaries: summaries.items, recordedAtMs: recordedAt)
    }

    @discardableResult
    public func recordCounterSummaries(from state: ToriiOfflineStateResponse,
                                       recordedAt timestampMs: UInt64? = nil) throws
        -> [OfflineCounterCheckpoint] {
        let recordedAt = timestampMs ?? state.nowMs
        return try counterJournal.upsert(rawSummaries: state.summaries, recordedAtMs: recordedAt)
    }

    public func counterCheckpoint(for certificateIdHex: String) -> OfflineCounterCheckpoint? {
        counterJournal.checkpoint(for: certificateIdHex)
    }

    public func counterSnapshot() -> [OfflineCounterCheckpoint] {
        counterJournal.snapshot()
    }

    @discardableResult
    public func ensureFreshVerdict(for certificateIdHex: String,
                                   attestationNonceHex: String? = nil,
                                   now: Date = Date()) throws -> OfflineVerdictMetadata {
        let normalized = certificateIdHex.lowercased()
        guard let metadata = verdictJournal.metadata(for: normalized) else {
            throw OfflineVerdictError.metadataMissing(certificateIdHex: normalized)
        }
        try validateNonce(expected: metadata.attestationNonceHex,
                          provided: attestationNonceHex,
                          certificateIdHex: normalized)
        try validateDeadlines(for: metadata,
                              nowMs: OfflineWallet.timestampMs(for: now),
                              certificateIdHex: normalized)
        return metadata
    }

    @discardableResult
    public func ensureFreshVerdict(for allowance: ToriiOfflineAllowanceItem,
                                   now: Date = Date()) throws -> OfflineVerdictMetadata {
        try ensureFreshVerdict(for: allowance.certificateIdHex,
                               attestationNonceHex: allowance.attestationNonceHex,
                               now: now)
    }

    private func validateNonce(expected: String?,
                               provided: String?,
                               certificateIdHex: String) throws {
        guard let provided = provided?.lowercased() else {
            return
        }
        guard let expected = expected?.lowercased() else {
            throw OfflineVerdictError.nonceMismatch(certificateIdHex: certificateIdHex,
                                                    expectedNonceHex: nil,
                                                    providedNonceHex: provided)
        }
        if expected != provided {
            throw OfflineVerdictError.nonceMismatch(certificateIdHex: certificateIdHex,
                                                    expectedNonceHex: expected,
                                                    providedNonceHex: provided)
        }
    }

    private func validateDeadlines(for metadata: OfflineVerdictMetadata,
                                   nowMs: UInt64,
                                   certificateIdHex: String) throws {
        if let refresh = metadata.refreshAtMs, refresh > 0, nowMs >= refresh {
            throw OfflineVerdictError.expired(certificateIdHex: certificateIdHex,
                                              deadlineKind: .refresh,
                                              deadlineMs: refresh)
        }
        if metadata.policyExpiresAtMs > 0, nowMs >= metadata.policyExpiresAtMs {
            throw OfflineVerdictError.expired(certificateIdHex: certificateIdHex,
                                              deadlineKind: .policy,
                                              deadlineMs: metadata.policyExpiresAtMs)
        }
        if metadata.expiresAtMs > 0, nowMs >= metadata.expiresAtMs {
            throw OfflineVerdictError.expired(certificateIdHex: certificateIdHex,
                                              deadlineKind: .certificate,
                                              deadlineMs: metadata.expiresAtMs)
        }
    }

    // MARK: Revocations

    public var revocationJournalURL: URL {
        revocationJournal.storageURL
    }

    @discardableResult
    public func recordRevocations(from revocations: ToriiOfflineRevocationList,
                                  recordedAt timestampMs: UInt64? = nil) throws
        -> [OfflineVerdictRevocationEntry] {
        let recordedAt = timestampMs ?? OfflineWallet.currentTimestampMs()
        return try revocationJournal.upsert(revocations: revocations.items, recordedAtMs: recordedAt)
    }

    @discardableResult
    public func recordRevocations(from state: ToriiOfflineStateResponse,
                                  recordedAt timestampMs: UInt64? = nil) throws
        -> [OfflineVerdictRevocationEntry] {
        let recordedAt = timestampMs ?? state.nowMs
        return try revocationJournal.upsert(rawRevocations: state.revocations, recordedAtMs: recordedAt)
    }

    public func isVerdictRevoked(verdictIdHex: String) -> Bool {
        revocationJournal.isRevoked(verdictIdHex: verdictIdHex)
    }

    public func revocation(for verdictIdHex: String) -> OfflineVerdictRevocationEntry? {
        revocationJournal.revocation(for: verdictIdHex)
    }

    public func revocationSnapshot() -> [OfflineVerdictRevocationEntry] {
        revocationJournal.snapshot()
    }

    // MARK: Utils

    private static func currentTimestampMs() -> UInt64 {
        timestampMs(for: Date())
    }

    private static func timestampMs(for date: Date) -> UInt64 {
        UInt64(date.timeIntervalSince1970 * 1_000)
    }
}

private extension String {
    var irohaNonEmptyTrimmed: String? {
        let trimmed = trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }
}

private extension Optional where Wrapped == String {
    var irohaNonEmptyTrimmed: String? {
        guard let value = self else { return nil }
        return value.irohaNonEmptyTrimmed
    }
}
