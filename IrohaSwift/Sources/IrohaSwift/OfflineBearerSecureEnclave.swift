import Foundation

#if canImport(Security)
import Security
#endif

/// Durable storage used by the Secure Enclave Offline Bearer adapter.
///
/// Production implementations must bind this store to rollback-resistant state.
/// A normal file, keychain value, or user defaults record is not sufficient for
/// strict Offline Bearer use because restoring an older purse balance would
/// allow double spend attempts.
public protocol OfflineBearerRollbackResistantPurseStore: AnyObject {
    /// Whether the store prevents rollback to an older purse sequence.
    var rollbackResistant: Bool { get }

    /// Returns the currently installed purse certificate, if one exists.
    func currentCertificate() throws -> OfflineBearerCertificateV2?

    /// Returns the currently installed purse state, if one exists.
    func currentState() throws -> OfflineBearerPurseStateV2?

    /// Persists the active purse certificate and state atomically.
    func savePurse(certificate: OfflineBearerCertificateV2, state: OfflineBearerPurseStateV2) throws

    /// Appends a locally signed debit receipt to the durable settlement journal.
    func appendDebitReceipt(_ receipt: OfflineBearerDebitReceiptV2) throws

    /// Appends a locally signed credit receipt to the durable settlement journal.
    func appendCreditReceipt(_ receipt: OfflineBearerCreditReceiptV2) throws

    /// Exports a receiver-complete settlement batch from the durable journal.
    func exportSettlementBatch(chainId: String, purseId: String, maxReceipts: Int) throws -> OfflineBearerSettlementBatchV2

    /// Removes settlement journal entries whose transfer ids were accepted online.
    func pruneSettled(transferIds: Set<String>) throws
}

/// Strict iOS Secure Enclave P-256 implementation of `OfflineBearerSecureElement`.
///
/// This adapter never falls back to software signing. Without a rollback-resistant
/// purse store and non-empty attestation evidence it reports unsupported
/// capabilities and all mutating purse operations fail closed.
public final class SecureEnclaveOfflineBearerSecureElement: OfflineBearerSecureElement {
    private let keyTag: String
    private let purseStore: OfflineBearerRollbackResistantPurseStore?
    private let attestationEvidenceProvider: () throws -> Data
    private let createKeyIfNeeded: Bool

    public init(keyTag: String = "org.hyperledger.iroha.offline-bearer.secure-enclave.p256",
                purseStore: OfflineBearerRollbackResistantPurseStore?,
                attestationEvidenceProvider: @escaping () throws -> Data = { Data() },
                createKeyIfNeeded: Bool = true) throws {
        guard !keyTag.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw OfflineBearerPolicyError("keyTag must not be blank")
        }
        self.keyTag = keyTag
        self.purseStore = purseStore
        self.attestationEvidenceProvider = attestationEvidenceProvider
        self.createKeyIfNeeded = createKeyIfNeeded
    }

    public func capabilities() throws -> OfflineBearerSecureElementCapabilities {
        let evidence = (try? attestationEvidenceProvider()) ?? Data()
        let stateful = purseStore?.rollbackResistant == true
        let keyAvailable = (try? publicKeyRepresentation())?.isEmpty == false
        return try OfflineBearerSecureElementCapabilities(
            hardwareBacked: keyAvailable,
            statefulPurse: stateful,
            hardwareClass: "ios-secure-enclave-p256",
            attestationKeyId: keyAvailable ? keyTag : nil,
            signatureAlgorithm: OfflineBearerV2Crypto.ecdsaP256SHA256,
            publicKeyEncoding: OfflineBearerV2Crypto.x963P256PublicKey,
            rollbackResistantState: stateful,
            attestationEvidence: evidence
        )
    }

    public func currentCertificate() throws -> OfflineBearerCertificateV2? {
        try purseStore?.currentCertificate()
    }

    public func currentState() throws -> OfflineBearerPurseStateV2? {
        try purseStore?.currentState()
    }

    public func installPurse(certificate: OfflineBearerCertificateV2, state: OfflineBearerPurseStateV2) throws {
        let store = try requireStrictStore()
        let publicKey = try publicKeyRepresentation()
        guard certificate.signatureAlgorithm == OfflineBearerV2Crypto.ecdsaP256SHA256,
              certificate.publicKeyEncoding == OfflineBearerV2Crypto.x963P256PublicKey,
              certificate.publicKey == publicKey
        else {
            throw OfflineBearerPolicyError("Secure Enclave Offline Bearer certificate does not match local P-256 key")
        }
        try store.savePurse(certificate: certificate, state: state)
    }

    public func createReceiveRequest(paymentRequestId: String,
                                     amount: String,
                                     createdAtMs: UInt64,
                                     expiresAtMs: UInt64,
                                     policyHashHex: String) throws -> OfflineBearerReceiveRequestV2 {
        let (_, certificate, state) = try requireInstalledPurse()
        let unsigned = try OfflineBearerReceiveRequestV2(
            chainId: state.chainId,
            paymentRequestId: paymentRequestId,
            recipientCertificate: certificate,
            assetDefinitionId: state.assetDefinitionId,
            amount: amount,
            createdAtMs: createdAtMs,
            expiresAtMs: expiresAtMs,
            policyHashHex: policyHashHex,
            signatureAlgorithm: OfflineBearerV2Crypto.ecdsaP256SHA256,
            challengeSignature: Data([0])
        )
        let signature = try sign(OfflineBearerV2Payloads.receiveRequestUnsignedPayload(unsigned))
        return try OfflineBearerReceiveRequestV2(
            chainId: unsigned.chainId,
            paymentRequestId: unsigned.paymentRequestId,
            recipientCertificate: unsigned.recipientCertificate,
            assetDefinitionId: unsigned.assetDefinitionId,
            amount: unsigned.amount,
            createdAtMs: unsigned.createdAtMs,
            expiresAtMs: unsigned.expiresAtMs,
            policyHashHex: unsigned.policyHashHex,
            signatureAlgorithm: unsigned.signatureAlgorithm,
            challengeSignature: signature
        )
    }

    public func debit(request: OfflineBearerReceiveRequestV2,
                      transferId: String,
                      createdAtMs: UInt64,
                      expiresAtMs: UInt64) throws -> OfflineBearerDebitReceiptV2 {
        let (store, certificate, state) = try requireInstalledPurse()
        let postBalance = try ToriiOfflineCashCodec.subtractAmounts(state.balance, request.amount)
        let nextState = try OfflineBearerPurseStateV2(
            chainId: state.chainId,
            accountId: state.accountId,
            assetDefinitionId: state.assetDefinitionId,
            purseId: state.purseId,
            balance: postBalance,
            sequence: state.sequence + 1,
            policyHashHex: state.policyHashHex,
            updatedAtMs: createdAtMs
        )
        let unsigned = try OfflineBearerDebitReceiptV2(
            transferId: transferId,
            chainId: state.chainId,
            paymentRequestId: request.paymentRequestId,
            senderCertificate: certificate,
            recipientCertificate: request.recipientCertificate,
            assetDefinitionId: state.assetDefinitionId,
            amount: request.amount,
            senderPreBalance: state.balance,
            senderPostBalance: postBalance,
            senderSequence: nextState.sequence,
            createdAtMs: createdAtMs,
            expiresAtMs: expiresAtMs,
            policyHashHex: state.policyHashHex,
            receiveChallengeSignature: request.challengeSignature,
            signatureAlgorithm: OfflineBearerV2Crypto.ecdsaP256SHA256,
            debitSignature: Data([0])
        )
        let signature = try sign(OfflineBearerV2Payloads.debitReceiptUnsignedPayload(unsigned))
        let receipt = try OfflineBearerDebitReceiptV2(
            transferId: unsigned.transferId,
            chainId: unsigned.chainId,
            paymentRequestId: unsigned.paymentRequestId,
            senderCertificate: unsigned.senderCertificate,
            recipientCertificate: unsigned.recipientCertificate,
            assetDefinitionId: unsigned.assetDefinitionId,
            amount: unsigned.amount,
            senderPreBalance: unsigned.senderPreBalance,
            senderPostBalance: unsigned.senderPostBalance,
            senderSequence: unsigned.senderSequence,
            createdAtMs: unsigned.createdAtMs,
            expiresAtMs: unsigned.expiresAtMs,
            policyHashHex: unsigned.policyHashHex,
            receiveChallengeSignature: unsigned.receiveChallengeSignature,
            signatureAlgorithm: unsigned.signatureAlgorithm,
            debitSignature: signature
        )
        try store.savePurse(certificate: certificate, state: nextState)
        try store.appendDebitReceipt(receipt)
        return receipt
    }

    public func credit(receipt: OfflineBearerDebitReceiptV2,
                       acceptedAtMs: UInt64) throws -> OfflineBearerCreditReceiptV2 {
        let (store, certificate, state) = try requireInstalledPurse()
        let postBalance = try ToriiOfflineCashCodec.addAmounts(state.balance, receipt.amount)
        let nextState = try OfflineBearerPurseStateV2(
            chainId: state.chainId,
            accountId: state.accountId,
            assetDefinitionId: state.assetDefinitionId,
            purseId: state.purseId,
            balance: postBalance,
            sequence: state.sequence + 1,
            policyHashHex: state.policyHashHex,
            updatedAtMs: acceptedAtMs
        )
        let unsigned = try OfflineBearerCreditReceiptV2(
            transferId: receipt.transferId,
            chainId: state.chainId,
            recipientCertificate: certificate,
            amount: receipt.amount,
            recipientPreBalance: state.balance,
            recipientPostBalance: postBalance,
            recipientSequence: nextState.sequence,
            acceptedAtMs: acceptedAtMs,
            signatureAlgorithm: OfflineBearerV2Crypto.ecdsaP256SHA256,
            creditSignature: Data([0])
        )
        let signature = try sign(OfflineBearerV2Payloads.creditReceiptUnsignedPayload(unsigned))
        let credit = try OfflineBearerCreditReceiptV2(
            transferId: unsigned.transferId,
            chainId: unsigned.chainId,
            recipientCertificate: unsigned.recipientCertificate,
            amount: unsigned.amount,
            recipientPreBalance: unsigned.recipientPreBalance,
            recipientPostBalance: unsigned.recipientPostBalance,
            recipientSequence: unsigned.recipientSequence,
            acceptedAtMs: unsigned.acceptedAtMs,
            signatureAlgorithm: unsigned.signatureAlgorithm,
            creditSignature: signature
        )
        try store.savePurse(certificate: certificate, state: nextState)
        try store.appendCreditReceipt(credit)
        return credit
    }

    public func exportSettlementBatch(maxReceipts: Int) throws -> OfflineBearerSettlementBatchV2 {
        let (store, certificate, state) = try requireInstalledPurse()
        return try store.exportSettlementBatch(
            chainId: state.chainId,
            purseId: certificate.purseId,
            maxReceipts: maxReceipts
        )
    }

    public func pruneSettled(transferIds: Set<String>) throws {
        try requireStrictStore().pruneSettled(transferIds: transferIds)
    }

    private func requireInstalledPurse() throws -> (
        OfflineBearerRollbackResistantPurseStore,
        OfflineBearerCertificateV2,
        OfflineBearerPurseStateV2
    ) {
        let store = try requireStrictStore()
        guard let certificate = try store.currentCertificate(),
              let state = try store.currentState() else {
            throw OfflineBearerPolicyError("Secure Enclave Offline Bearer purse is not installed")
        }
        return (store, certificate, state)
    }

    private func requireStrictStore() throws -> OfflineBearerRollbackResistantPurseStore {
        guard let store = purseStore, store.rollbackResistant else {
            throw OfflineBearerPolicyError("Secure Enclave Offline Bearer requires rollback-resistant purse state")
        }
        let evidence = try attestationEvidenceProvider()
        guard !evidence.isEmpty else {
            throw OfflineBearerPolicyError("Secure Enclave Offline Bearer requires attestation evidence")
        }
        _ = try privateKey()
        return store
    }

    private func publicKeyRepresentation() throws -> Data {
        #if canImport(Security)
        let key = try privateKey()
        guard let publicKey = SecKeyCopyPublicKey(key) else {
            throw OfflineBearerPolicyError("Secure Enclave Offline Bearer public key is unavailable")
        }
        var error: Unmanaged<CFError>?
        guard let data = SecKeyCopyExternalRepresentation(publicKey, &error) as Data? else {
            throw OfflineBearerPolicyError("Secure Enclave Offline Bearer public key export failed")
        }
        return data
        #else
        throw OfflineBearerPolicyError("Secure Enclave Offline Bearer is unavailable on this platform")
        #endif
    }

    private func sign(_ payload: Data) throws -> Data {
        #if canImport(Security)
        let key = try privateKey()
        let algorithm = SecKeyAlgorithm.ecdsaSignatureMessageX962SHA256
        guard SecKeyIsAlgorithmSupported(key, .sign, algorithm) else {
            throw OfflineBearerPolicyError("Secure Enclave Offline Bearer key does not support P-256 SHA-256 signing")
        }
        var error: Unmanaged<CFError>?
        guard let signature = SecKeyCreateSignature(key, algorithm, payload as CFData, &error) as Data? else {
            throw OfflineBearerPolicyError("Secure Enclave Offline Bearer signing failed")
        }
        return signature
        #else
        throw OfflineBearerPolicyError("Secure Enclave Offline Bearer is unavailable on this platform")
        #endif
    }

    #if canImport(Security)
    private func privateKey() throws -> SecKey {
        if let existing = loadPrivateKey() {
            return existing
        }
        guard createKeyIfNeeded else {
            throw OfflineBearerPolicyError("Secure Enclave Offline Bearer key is not installed")
        }
        guard let access = SecAccessControlCreateWithFlags(
            nil,
            kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
            [.privateKeyUsage],
            nil
        ) else {
            throw OfflineBearerPolicyError("Secure Enclave Offline Bearer access control is unavailable")
        }
        let tag = Data(keyTag.utf8)
        let attributes: [String: Any] = [
            kSecAttrKeyType as String: kSecAttrKeyTypeECSECPrimeRandom,
            kSecAttrKeySizeInBits as String: 256,
            kSecAttrTokenID as String: kSecAttrTokenIDSecureEnclave,
            kSecPrivateKeyAttrs as String: [
                kSecAttrIsPermanent as String: true,
                kSecAttrApplicationTag as String: tag,
                kSecAttrAccessControl as String: access,
            ],
        ]
        var error: Unmanaged<CFError>?
        guard let key = SecKeyCreateRandomKey(attributes as CFDictionary, &error) else {
            throw OfflineBearerPolicyError("Secure Enclave Offline Bearer key creation failed")
        }
        return key
    }

    private func loadPrivateKey() -> SecKey? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassKey,
            kSecAttrApplicationTag as String: Data(keyTag.utf8),
            kSecAttrKeyType as String: kSecAttrKeyTypeECSECPrimeRandom,
            kSecReturnRef as String: true,
        ]
        var item: CFTypeRef?
        guard SecItemCopyMatching(query as CFDictionary, &item) == errSecSuccess,
              let item,
              CFGetTypeID(item) == SecKeyGetTypeID() else {
            return nil
        }
        return unsafeBitCast(item, to: SecKey.self)
    }
    #endif
}
