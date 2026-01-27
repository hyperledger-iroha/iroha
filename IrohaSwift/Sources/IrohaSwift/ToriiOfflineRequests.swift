import Foundation

public extension ToriiOfflineSpendReceiptsSubmitRequest {
    init(receipts: [OfflineSpendReceipt]) throws {
        self.init(receipts: try receipts.map { try $0.toriiJSON() })
    }
}

public extension ToriiOfflineSettlementSubmitRequest {
    init(authority: String, privateKey: String, transfer: OfflineToOnlineTransfer) throws {
        self.init(authority: authority,
                  privateKey: privateKey,
                  transfer: try transfer.toriiJSON())
    }
}

public extension ToriiOfflineCertificateIssueRequest {
    init(certificate: OfflineWalletCertificateDraft) throws {
        self.init(certificate: try certificate.toriiJSON())
    }
}

public extension ToriiOfflineAllowanceRegisterRequest {
    init(authority: String, privateKey: String, certificate: OfflineWalletCertificate) throws {
        self.init(authority: authority,
                  privateKey: privateKey,
                  certificate: try certificate.toriiJSON())
    }
}
