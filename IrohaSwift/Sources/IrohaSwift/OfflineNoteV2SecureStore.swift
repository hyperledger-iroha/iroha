import Foundation
import LocalAuthentication
import Security

public enum OfflineNoteV2WalletNoteJsonCodecError: Error, LocalizedError, Equatable {
    case invalidJson
    case invalidField(String)

    public var errorDescription: String? {
        switch self {
        case .invalidJson:
            return "Offline Note V2 wallet note JSON must be an object."
        case let .invalidField(field):
            return "Offline Note V2 wallet note field \(field) is invalid."
        }
    }
}

public enum OfflineNoteV2WalletNoteJsonCodec {
    public static let version: UInt64 = 1

    public static func encode(_ note: OfflineNoteV2WalletNote) throws -> Data {
        let payload: [String: Any] = [
            "version": NSNumber(value: version),
            "chain_id": note.chainId,
            "account_id": note.accountId,
            "asset_id": note.assetId,
            "amount": note.amount,
            "key_certificate_norito_base64": try note.keyCertificate.noritoEncoded().base64EncodedString(),
            "note_commitment_hex": note.noteCommitmentHex,
            "note_secret_base64": note.noteSecret.base64EncodedString(),
            "origin": try encodeOrigin(note.origin),
            "state": note.state.rawValue,
            "created_at_ms": NSNumber(value: note.createdAtMs),
            "updated_at_ms": NSNumber(value: note.updatedAtMs)
        ]
        return try JSONSerialization.data(withJSONObject: payload, options: [.sortedKeys])
    }

    public static func decode(_ payload: Data) throws -> OfflineNoteV2WalletNote {
        guard let object = try JSONSerialization.jsonObject(with: payload) as? [String: Any] else {
            throw OfflineNoteV2WalletNoteJsonCodecError.invalidJson
        }
        guard try uint(object["version"], field: "version") == version else {
            throw OfflineNoteV2WalletNoteJsonCodecError.invalidField("version")
        }
        guard let keyCertificateBytes = Data(base64Encoded: try string(
            object["key_certificate_norito_base64"],
            field: "key_certificate_norito_base64"
        )) else {
            throw OfflineNoteV2WalletNoteJsonCodecError.invalidField("key_certificate_norito_base64")
        }
        guard let noteCommitment = Data(hexString: try string(
            object["note_commitment_hex"],
            field: "note_commitment_hex"
        )) else {
            throw OfflineNoteV2WalletNoteJsonCodecError.invalidField("note_commitment_hex")
        }
        guard let noteSecret = Data(base64Encoded: try string(
            object["note_secret_base64"],
            field: "note_secret_base64"
        )) else {
            throw OfflineNoteV2WalletNoteJsonCodecError.invalidField("note_secret_base64")
        }
        let state = try decodeState(try string(object["state"], field: "state"))
        return try OfflineNoteV2WalletNote(
            chainId: string(object["chain_id"], field: "chain_id"),
            accountId: string(object["account_id"], field: "account_id"),
            assetId: string(object["asset_id"], field: "asset_id"),
            amount: string(object["amount"], field: "amount"),
            keyCertificate: OfflineNoteV2Decoding.decodeKeyCertificate(keyCertificateBytes),
            noteCommitment: noteCommitment,
            noteSecret: noteSecret,
            origin: decodeOrigin(dictionary(object["origin"], field: "origin")),
            state: state,
            createdAtMs: uint(object["created_at_ms"], field: "created_at_ms"),
            updatedAtMs: uint(object["updated_at_ms"], field: "updated_at_ms")
        )
    }

    private static func encodeOrigin(_ origin: OfflineNoteCommitmentOriginV2) throws -> [String: Any] {
        switch origin {
        case let .issuerLoad(value):
            return [
                "type": "issuer_load",
                "operation_id": value.operationId,
                "lineage_id": value.lineageId,
                "local_revision": NSNumber(value: value.localRevision)
            ]
        case let .p2pOutput(value):
            return [
                "type": "p2p_output",
                "payment_request_id": value.paymentRequestId,
                "output_index": NSNumber(value: value.outputIndex)
            ]
        }
    }

    private static func decodeOrigin(_ payload: [String: Any]) throws -> OfflineNoteCommitmentOriginV2 {
        switch try string(payload["type"], field: "origin.type") {
        case "issuer_load":
            return try .issuerLoad(OfflineNoteIssuerLoadOriginV2(
                operationId: string(payload["operation_id"], field: "origin.operation_id"),
                lineageId: string(payload["lineage_id"], field: "origin.lineage_id"),
                localRevision: uint(payload["local_revision"], field: "origin.local_revision")
            ))
        case "p2p_output":
            let outputIndex = try uint(payload["output_index"], field: "origin.output_index")
            guard outputIndex <= UInt64(UInt32.max) else {
                throw OfflineNoteV2WalletNoteJsonCodecError.invalidField("origin.output_index")
            }
            return try .p2pOutput(OfflineNoteP2pOutputOriginV2(
                paymentRequestId: string(payload["payment_request_id"], field: "origin.payment_request_id"),
                outputIndex: UInt32(outputIndex)
            ))
        default:
            throw OfflineNoteV2WalletNoteJsonCodecError.invalidField("origin.type")
        }
    }

    private static func decodeState(_ raw: String) throws -> OfflineNoteV2WalletNoteState {
        if let state = OfflineNoteV2WalletNoteState(rawValue: raw) {
            return state
        }
        switch raw {
        case "spendPending", "SPEND_PENDING":
            return .spent
        case "changePending", "CHANGE_PENDING":
            return .spendable
        default:
            throw OfflineNoteV2WalletNoteJsonCodecError.invalidField("state")
        }
    }

    private static func dictionary(_ value: Any?, field: String) throws -> [String: Any] {
        guard let object = value as? [String: Any] else {
            throw OfflineNoteV2WalletNoteJsonCodecError.invalidField(field)
        }
        return object
    }

    private static func string(_ value: Any?, field: String) throws -> String {
        guard let string = value as? String,
              !string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        else {
            throw OfflineNoteV2WalletNoteJsonCodecError.invalidField(field)
        }
        return string
    }

    private static func uint(_ value: Any?, field: String) throws -> UInt64 {
        if let number = value as? NSNumber {
            let double = number.doubleValue
            guard double >= 0,
                  double.rounded() == double,
                  double <= Double(UInt64.max)
            else {
                throw OfflineNoteV2WalletNoteJsonCodecError.invalidField(field)
            }
            return number.uint64Value
        }
        if let string = value as? String, let parsed = UInt64(string) {
            return parsed
        }
        throw OfflineNoteV2WalletNoteJsonCodecError.invalidField(field)
    }
}

public enum OfflineNoteV2KeychainStoreError: Error, LocalizedError, Equatable {
    case invalidLabel(String)
    case corrupt(String)
    case keychainFailure(OSStatus)

    public var errorDescription: String? {
        switch self {
        case let .invalidLabel(label):
            return "Offline Note V2 keychain store label is invalid: \(label)"
        case let .corrupt(reason):
            return "Offline Note V2 keychain store is corrupted: \(reason)"
        case let .keychainFailure(status):
            return "Offline Note V2 keychain operation failed with status \(status)"
        }
    }
}

public final class OfflineNoteV2KeychainStore: OfflineNoteV2Store {
    public struct Configuration: Sendable {
        public let appGroup: String?
        public let requireBiometrics: Bool
        public let requireDeviceLock: Bool

        public init(appGroup: String? = nil,
                    requireBiometrics: Bool = false,
                    requireDeviceLock: Bool = true) {
            self.appGroup = appGroup
            self.requireBiometrics = requireBiometrics
            self.requireDeviceLock = requireDeviceLock
        }

        public static var `default`: Configuration { Configuration() }
    }

    private struct StoredNote: Codable, Equatable {
        let commitmentHex: String
        let payloadBase64: String
    }

    private struct StoredCollection: Codable, Equatable {
        let version: Int
        let notes: [StoredNote]
    }

    private struct StoredMetadata: Codable, Equatable {
        let version: Int
        let revision: Int
    }

    private let label: String
    private let backing: OfflineNoteV2KeychainBacking
    private let lock = NSLock()

    public init(label rawLabel: String = "default", configuration: Configuration = .default) throws {
        label = try Self.sanitize(label: rawLabel)
        backing = OfflineNoteV2KeychainBacking(configuration: configuration)
    }

    public func mutateNotes<T>(_ body: (inout [String: OfflineNoteV2WalletNote]) throws -> T) throws -> T {
        try lock.withLock {
            var notes = try loadNotes()
            let result = try body(&notes)
            try saveNotes(notes)
            return result
        }
    }

    public func listNotes() throws -> [OfflineNoteV2WalletNote] {
        try lock.withLock {
            try loadNotes().values.sorted {
                if $0.createdAtMs == $1.createdAtMs {
                    return $0.noteCommitmentHex < $1.noteCommitmentHex
                }
                return $0.createdAtMs < $1.createdAtMs
            }
        }
    }

    public func findNote(noteCommitment: Data) throws -> OfflineNoteV2WalletNote? {
        try lock.withLock {
            try loadNotes()[noteCommitment.hexLowercased()]
        }
    }

    public func upsert(_ note: OfflineNoteV2WalletNote) throws {
        try mutateNotes { notes in
            notes[note.noteCommitmentHex] = note
        }
    }

    public func delete(noteCommitment: Data) throws {
        try lock.withLock {
            var notes = try loadNotes()
            notes.removeValue(forKey: noteCommitment.hexLowercased())
            try saveNotes(notes)
        }
    }

    public func clear() throws {
        try lock.withLock {
            if let metadata = try loadMetadata() {
                try backing.delete(label: collectionLabel(revision: metadata.revision))
            }
            try backing.delete(label: metadataLabel)
            try backing.delete(label: label)
        }
    }

    private func loadNotes() throws -> [String: OfflineNoteV2WalletNote] {
        if let metadata = try loadMetadata() {
            guard let data = try backing.load(label: collectionLabel(revision: metadata.revision)) else {
                throw OfflineNoteV2KeychainStoreError.corrupt("collection revision is missing")
            }
            return try decodeCollection(data)
        }
        guard let data = try backing.load(label: label) else {
            return [:]
        }
        return try decodeCollection(data)
    }

    private func decodeCollection(_ data: Data) throws -> [String: OfflineNoteV2WalletNote] {
        do {
            let collection = try JSONDecoder().decode(StoredCollection.self, from: data)
            guard collection.version == 1 else {
                throw OfflineNoteV2KeychainStoreError.corrupt("unsupported collection version")
            }
            var notes: [String: OfflineNoteV2WalletNote] = [:]
            for stored in collection.notes {
                guard let payload = Data(base64Encoded: stored.payloadBase64) else {
                    throw OfflineNoteV2KeychainStoreError.corrupt("note payload is not base64")
                }
                let note = try OfflineNoteV2WalletNoteJsonCodec.decode(payload)
                guard note.noteCommitmentHex == stored.commitmentHex else {
                    throw OfflineNoteV2KeychainStoreError.corrupt("note commitment index mismatch")
                }
                notes[stored.commitmentHex] = note
            }
            return notes
        } catch let storeError as OfflineNoteV2KeychainStoreError {
            throw storeError
        } catch {
            throw OfflineNoteV2KeychainStoreError.corrupt("failed to decode collection: \(error)")
        }
    }

    private func saveNotes(_ notes: [String: OfflineNoteV2WalletNote]) throws {
        let stored = try notes.values.sorted {
            if $0.createdAtMs == $1.createdAtMs {
                return $0.noteCommitmentHex < $1.noteCommitmentHex
            }
            return $0.createdAtMs < $1.createdAtMs
        }.map { note in
            StoredNote(
                commitmentHex: note.noteCommitmentHex,
                payloadBase64: try OfflineNoteV2WalletNoteJsonCodec.encode(note).base64EncodedString()
            )
        }
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let previousRevision = try loadMetadata()?.revision
        let revision = (previousRevision ?? 0) + 1
        try backing.save(
            label: collectionLabel(revision: revision),
            data: encoder.encode(StoredCollection(version: 1, notes: stored))
        )
        if let previousRevision {
            try backing.delete(label: collectionLabel(revision: previousRevision))
        }
        try backing.save(
            label: metadataLabel,
            data: encoder.encode(StoredMetadata(version: 1, revision: revision))
        )
        try backing.delete(label: label)
    }

    private func loadMetadata() throws -> StoredMetadata? {
        guard let data = try backing.load(label: metadataLabel) else {
            return nil
        }
        do {
            let metadata = try JSONDecoder().decode(StoredMetadata.self, from: data)
            guard metadata.version == 1, metadata.revision > 0 else {
                throw OfflineNoteV2KeychainStoreError.corrupt("unsupported metadata")
            }
            return metadata
        } catch let storeError as OfflineNoteV2KeychainStoreError {
            throw storeError
        } catch {
            throw OfflineNoteV2KeychainStoreError.corrupt("failed to decode metadata: \(error)")
        }
    }

    private var metadataLabel: String {
        "\(label).meta"
    }

    private func collectionLabel(revision: Int) -> String {
        "\(label).rev.\(revision)"
    }

    private static func sanitize(label: String) throws -> String {
        let trimmed = label.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineNoteV2KeychainStoreError.invalidLabel(label)
        }
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-_."))
        guard trimmed.rangeOfCharacter(from: allowed.inverted) == nil,
              trimmed.count <= 64
        else {
            throw OfflineNoteV2KeychainStoreError.invalidLabel(label)
        }
        return trimmed
    }
}

private struct OfflineNoteV2KeychainBacking {
    private let service = "org.hyperledger.iroha.offline-note-v2-store"
    private let accessGroup: String?
    private let accessControl: SecAccessControl?

    init(configuration: OfflineNoteV2KeychainStore.Configuration) {
        accessGroup = configuration.appGroup
        accessControl = Self.buildAccessControl(
            requireBiometrics: configuration.requireBiometrics,
            requireDeviceLock: configuration.requireDeviceLock
        )
    }

    func load(label: String) throws -> Data? {
        let context = LAContext()
        context.interactionNotAllowed = true
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: label,
            kSecAttrService as String: service,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecUseAuthenticationContext as String: context
        ]
        if let accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        switch status {
        case errSecSuccess:
            guard let data = item as? Data else {
                throw OfflineNoteV2KeychainStoreError.keychainFailure(status)
            }
            return data
        case errSecItemNotFound:
            return nil
        default:
            throw OfflineNoteV2KeychainStoreError.keychainFailure(status)
        }
    }

    func save(label: String, data: Data) throws {
        let context = LAContext()
        context.interactionNotAllowed = true
        var attributes: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: label,
            kSecAttrService as String: service,
            kSecValueData as String: data,
            kSecUseAuthenticationContext as String: context
        ]
        if let accessControl {
            attributes[kSecAttrAccessControl as String] = accessControl
        } else {
            attributes[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        }
        if let accessGroup {
            attributes[kSecAttrAccessGroup as String] = accessGroup
        }

        let status = SecItemAdd(attributes as CFDictionary, nil)
        if status == errSecDuplicateItem {
            var query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrAccount as String: label,
                kSecAttrService as String: service
            ]
            if let accessGroup {
                query[kSecAttrAccessGroup as String] = accessGroup
            }
            let update: [String: Any] = [kSecValueData as String: data]
            let updateStatus = SecItemUpdate(query as CFDictionary, update as CFDictionary)
            guard updateStatus == errSecSuccess else {
                throw OfflineNoteV2KeychainStoreError.keychainFailure(updateStatus)
            }
            return
        }
        guard status == errSecSuccess else {
            throw OfflineNoteV2KeychainStoreError.keychainFailure(status)
        }
    }

    func delete(label: String) throws {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: label,
            kSecAttrService as String: service
        ]
        if let accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }
        SecItemDelete(query as CFDictionary)
    }

    private static func buildAccessControl(requireBiometrics: Bool, requireDeviceLock: Bool) -> SecAccessControl? {
        var flags = SecAccessControlCreateFlags()
        if requireBiometrics {
            flags.insert(.userPresence)
        }

        let baseAccessibility: CFTypeRef = requireDeviceLock
            ? kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly
            : kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        var error: Unmanaged<CFError>?
        if let control = SecAccessControlCreateWithFlags(nil, baseAccessibility, flags, &error) {
            return control
        }

        error?.release()
        return nil
    }
}
