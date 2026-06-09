import Foundation

public enum OfflineCashConfigurationSnapshotError: Error, Equatable, LocalizedError {
    case offlinePaymentsDisabled
    case missingIssuerPublicKey
    case expired(expiresAtMs: UInt64, nowMs: UInt64)
    case unsupportedBridgeAbi(required: UInt32, actual: UInt32?)

    public var errorDescription: String? {
        switch self {
        case .offlinePaymentsDisabled:
            return "Offline cash is disabled in the cached configuration snapshot."
        case .missingIssuerPublicKey:
            return "Offline cash requires a cached issuer public key before offline exchange."
        case let .expired(expiresAtMs, nowMs):
            return "Offline cash configuration snapshot expired at \(expiresAtMs), current time is \(nowMs)."
        case let .unsupportedBridgeAbi(required, actual):
            return "Offline cash requires bridge ABI \(required), cached ABI is \(actual.map(String.init) ?? "missing")."
        }
    }
}

public struct OfflineCashConfigurationSnapshot: Codable, Equatable, Sendable {
    public let chainId: String
    public let assetDefinitionId: String
    public let offlinePaymentsEnabled: Bool
    public let issuerPublicKeyBase64: String?
    public let bridgeAbiVersion: UInt32?
    public let artifactSetId: String?
    public let circuitId: String?
    public let createdAtMs: UInt64
    public let expiresAtMs: UInt64?

    public init(
        chainId: String,
        assetDefinitionId: String,
        offlinePaymentsEnabled: Bool,
        issuerPublicKeyBase64: String?,
        bridgeAbiVersion: UInt32? = nil,
        artifactSetId: String? = nil,
        circuitId: String? = nil,
        createdAtMs: UInt64,
        expiresAtMs: UInt64? = nil
    ) {
        self.chainId = chainId
        self.assetDefinitionId = assetDefinitionId
        self.offlinePaymentsEnabled = offlinePaymentsEnabled
        self.issuerPublicKeyBase64 = issuerPublicKeyBase64
        self.bridgeAbiVersion = bridgeAbiVersion
        self.artifactSetId = artifactSetId
        self.circuitId = circuitId
        self.createdAtMs = createdAtMs
        self.expiresAtMs = expiresAtMs
    }

    public func requireUsableForOfflineExchange(
        nowMs: UInt64,
        requiredBridgeAbiVersion: UInt32? = nil
    ) throws {
        guard offlinePaymentsEnabled else {
            throw OfflineCashConfigurationSnapshotError.offlinePaymentsDisabled
        }
        guard issuerPublicKeyBase64?.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty == false else {
            throw OfflineCashConfigurationSnapshotError.missingIssuerPublicKey
        }
        if let expiresAtMs, expiresAtMs <= nowMs {
            throw OfflineCashConfigurationSnapshotError.expired(expiresAtMs: expiresAtMs, nowMs: nowMs)
        }
        if let requiredBridgeAbiVersion,
           bridgeAbiVersion.map({ $0 >= requiredBridgeAbiVersion }) != true {
            throw OfflineCashConfigurationSnapshotError.unsupportedBridgeAbi(
                required: requiredBridgeAbiVersion,
                actual: bridgeAbiVersion
            )
        }
    }
}

public protocol OfflineCashConfigurationSnapshotStore: AnyObject {
    func loadOfflineCashConfigurationSnapshot() throws -> OfflineCashConfigurationSnapshot?
    func saveOfflineCashConfigurationSnapshot(_ snapshot: OfflineCashConfigurationSnapshot) throws
}

public protocol OfflineCashAuditReceiptSynchronizing: AnyObject {
    func hasPendingAuditReceipts() async throws -> Bool
    func syncPendingAuditReceipts() async throws
}

public final class OfflineCashLifecycleController {
    private let listNotesOperation: () throws -> [OfflineNoteWalletNote]
    private let loadOperation: (String, String) async throws -> OfflineNoteWalletNote
    private let prepareReceiveOperation: (String, String) throws -> OfflineNoteReceiveRequest
    private let createPaymentOperation: (OfflineNoteReceiveRequest) throws -> OfflineNotePaymentToken
    private let acceptPaymentOperation: (OfflineNotePaymentToken) throws -> OfflineNoteWalletNote
    private let publishAuditOperation: (OfflineNotePaymentToken) async throws -> Void
    private let redeemOperation: (OfflineNoteWalletNote, String?) async throws -> OfflineNoteWalletNote
    private let syncNotesOperation: () async throws -> [OfflineNoteWalletNote]
    private let auditReceiptSynchronizer: OfflineCashAuditReceiptSynchronizing?

    public convenience init(
        wallet: OfflineNoteWallet,
        auditReceiptSynchronizer: OfflineCashAuditReceiptSynchronizing? = nil
    ) {
        self.init(
            auditReceiptSynchronizer: auditReceiptSynchronizer,
            listNotes: { try wallet.listNotes() },
            load: { assetDefinitionId, amount in
                try await wallet.load(assetDefinitionId: assetDefinitionId, amount: amount)
            },
            prepareReceive: { assetDefinitionId, amount in
                try wallet.prepareReceive(assetDefinitionId: assetDefinitionId, amount: amount)
            },
            createPayment: { receiveRequest in
                try wallet.pay(receiveRequest)
            },
            acceptPayment: { paymentToken in
                try wallet.accept(paymentToken)
            },
            publishAudit: { paymentToken in
                try await wallet.publishAudit(paymentToken)
            },
            redeem: { note, recipient in
                try await wallet.redeem(note, recipient: recipient)
            },
            syncNotes: {
                try await wallet.sync()
            }
        )
    }

    init(
        auditReceiptSynchronizer: OfflineCashAuditReceiptSynchronizing? = nil,
        listNotes: @escaping () throws -> [OfflineNoteWalletNote],
        load: @escaping (String, String) async throws -> OfflineNoteWalletNote,
        prepareReceive: @escaping (String, String) throws -> OfflineNoteReceiveRequest,
        createPayment: @escaping (OfflineNoteReceiveRequest) throws -> OfflineNotePaymentToken,
        acceptPayment: @escaping (OfflineNotePaymentToken) throws -> OfflineNoteWalletNote,
        publishAudit: @escaping (OfflineNotePaymentToken) async throws -> Void,
        redeem: @escaping (OfflineNoteWalletNote, String?) async throws -> OfflineNoteWalletNote,
        syncNotes: @escaping () async throws -> [OfflineNoteWalletNote]
    ) {
        self.auditReceiptSynchronizer = auditReceiptSynchronizer
        self.listNotesOperation = listNotes
        self.loadOperation = load
        self.prepareReceiveOperation = prepareReceive
        self.createPaymentOperation = createPayment
        self.acceptPaymentOperation = acceptPayment
        self.publishAuditOperation = publishAudit
        self.redeemOperation = redeem
        self.syncNotesOperation = syncNotes
    }

    public func listNotes() throws -> [OfflineNoteWalletNote] {
        try listNotesOperation()
    }

    public func syncPendingAuditReceiptsIfNeeded() async throws {
        guard let auditReceiptSynchronizer else { return }
        guard try await auditReceiptSynchronizer.hasPendingAuditReceipts() else { return }
        try await auditReceiptSynchronizer.syncPendingAuditReceipts()
    }

    public func load(assetDefinitionId: String, amount: String) async throws -> OfflineNoteWalletNote {
        try await syncPendingAuditReceiptsIfNeeded()
        return try await loadOperation(assetDefinitionId, amount)
    }

    public func prepareReceive(assetDefinitionId: String, amount: String) throws -> OfflineNoteReceiveRequest {
        try prepareReceiveOperation(assetDefinitionId, amount)
    }

    public func createPayment(for receiveRequest: OfflineNoteReceiveRequest) throws -> OfflineNotePaymentToken {
        try createPaymentOperation(receiveRequest)
    }

    public func acceptPayment(_ paymentToken: OfflineNotePaymentToken) throws -> OfflineNoteWalletNote {
        try acceptPaymentOperation(paymentToken)
    }

    public func publishAudit(_ paymentToken: OfflineNotePaymentToken) async throws {
        try await publishAuditOperation(paymentToken)
    }

    public func redeem(_ note: OfflineNoteWalletNote, recipient: String? = nil) async throws -> OfflineNoteWalletNote {
        try await redeemOperation(note, recipient)
    }

    public func syncNotes() async throws -> [OfflineNoteWalletNote] {
        try await syncNotesOperation()
    }
}
