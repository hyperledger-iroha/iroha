import CryptoKit
import Foundation
import LocalAuthentication
import Security
#if os(iOS)
import UIKit
#endif

public enum ConnectKeyStoreError: Error, LocalizedError {
    case invalidLabel(String)
    case corrupt(String)
    case integrityMismatch
    case keychainFailure(OSStatus)
    case hmacUnavailable

    public var errorDescription: String? {
        switch self {
        case .invalidLabel(let label):
            return "Connect keystore label is invalid: \(label)"
        case .corrupt(let reason):
            return "Connect keystore entry is corrupted: \(reason)"
        case .integrityMismatch:
            return "Connect keystore entry failed integrity verification"
        case .keychainFailure(let status):
            return "Connect keystore keychain operation failed with status \(status)"
        case .hmacUnavailable:
            return "Connect keystore integrity key could not be loaded"
        }
    }
}

/// Persistent keystore for Connect X25519 keypairs with lightweight attestation.
public struct ConnectKeyStore {
    public struct Configuration: Sendable {
        public let appGroup: String?
        public let requireBiometrics: Bool
        public let requireDeviceLock: Bool
        public let preferKeychain: Bool

        public init(appGroup: String? = nil,
                    requireBiometrics: Bool = false,
                    requireDeviceLock: Bool = true,
                    preferKeychain: Bool = true) {
            self.appGroup = appGroup
            self.requireBiometrics = requireBiometrics
            self.requireDeviceLock = requireDeviceLock
            self.preferKeychain = preferKeychain
        }

        public static var `default`: Configuration { Configuration() }
    }

    public struct Attestation: Codable, Equatable, Sendable {
        /// SHA-256 digest of the public key bytes.
        public let publicKeyDigest: Data
        /// Device or host identifier recorded at creation time.
        public let deviceLabel: String
        /// UTC timestamp when the keypair was created.
        public let createdAt: Date
    }

    struct StoredKey: Codable, Equatable, Sendable {
        let keyPair: ConnectKeyPair
        let attestation: Attestation
    }

    private let backing: HybridBacking

    /// Create a keystore backed by a secure keychain first, with an authenticated file fallback.
    /// The default initializer stores records under the user's application support directory.
    public init(directory: URL? = nil, configuration: Configuration = .default) {
        let baseDirectory = directory ?? FileBacking.defaultBaseDirectory()
        let integrity = IntegrityKeyProvider(baseDirectory: baseDirectory,
                                             accessGroup: configuration.appGroup,
                                             useKeychain: configuration.preferKeychain)
        let fileBacking = FileBacking(baseDirectory: baseDirectory, integrity: integrity)
        let keychainBacking = configuration.preferKeychain ? KeychainBacking(configuration: configuration) : nil
        backing = HybridBacking(file: fileBacking, keychain: keychainBacking)
    }

    /// Load an existing keypair + attestation if present; otherwise generate, attest, and persist a new pair.
    @discardableResult
    public func generateOrLoad(label rawLabel: String) throws -> (ConnectKeyPair, Attestation) {
        let label = try sanitize(label: rawLabel)
        if let existing = try backing.load(label: label, rawLabel: rawLabel) {
            try validate(attestation: existing.attestation, keyPair: existing.keyPair)
            return (existing.keyPair, existing.attestation)
        }
        let pair = try ConnectCrypto.generateKeyPair()
        let attestation = try Self.buildAttestation(publicKey: pair.publicKey)
        let stored = StoredKey(keyPair: pair, attestation: attestation)
        try backing.save(label: label, rawLabel: rawLabel, stored: stored)
        return (pair, attestation)
    }

    /// Remove a stored keypair.
    public func delete(label rawLabel: String) throws {
        let label = try sanitize(label: rawLabel)
        try backing.delete(label: label, rawLabel: rawLabel)
    }

    private func sanitize(label: String) throws -> String {
        let trimmed = label.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ConnectKeyStoreError.invalidLabel(label)
        }
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "-_."))
        guard trimmed.rangeOfCharacter(from: allowed.inverted) == nil else {
            throw ConnectKeyStoreError.invalidLabel(label)
        }
        guard trimmed.count <= 64 else {
            throw ConnectKeyStoreError.invalidLabel(label)
        }
        return trimmed
    }

    private static func buildAttestation(publicKey: Data) throws -> Attestation {
        let digest = Data(SHA256.hash(data: publicKey))
        let now = Date()
        let createdAt = Date(timeIntervalSince1970: floor(now.timeIntervalSince1970))
        #if os(iOS)
        let deviceLabel = UIDevice.current.name
        #else
        let deviceLabel = ProcessInfo.processInfo.hostName
        #endif
        return Attestation(publicKeyDigest: digest, deviceLabel: deviceLabel, createdAt: createdAt)
    }

    private func validate(attestation: Attestation, keyPair: ConnectKeyPair) throws {
        let expectedDigest = Data(SHA256.hash(data: keyPair.publicKey))
        guard attestation.publicKeyDigest == expectedDigest else {
            throw ConnectKeyStoreError.integrityMismatch
        }
    }
}

private protocol Backing {
    func load(label: String, rawLabel: String) throws -> ConnectKeyStore.StoredKey?
    func save(label: String, rawLabel: String, stored: ConnectKeyStore.StoredKey) throws
    func delete(label: String, rawLabel: String) throws
}

private struct HybridBacking: Backing {
    private let file: FileBacking
    private let keychain: KeychainBacking?

    init(file: FileBacking, keychain: KeychainBacking?) {
        self.file = file
        self.keychain = keychain
    }

    func load(label: String, rawLabel: String) throws -> ConnectKeyStore.StoredKey? {
        if let keychain {
            do {
                if let stored = try keychain.load(label: label, rawLabel: rawLabel) {
                    return stored
                }
            } catch let error as ConnectKeyStoreError {
                if !shouldFallback(status: error) {
                    throw error
                }
            }
        }
        return try file.load(label: label, rawLabel: rawLabel)
    }

    func save(label: String, rawLabel: String, stored: ConnectKeyStore.StoredKey) throws {
        if let keychain {
            do {
                try keychain.save(label: label, rawLabel: rawLabel, stored: stored)
                return
            } catch let error as ConnectKeyStoreError {
                if !shouldFallback(status: error) {
                    throw error
                }
            }
        }
        try file.save(label: label, rawLabel: rawLabel, stored: stored)
    }

    func delete(label: String, rawLabel: String) throws {
        try keychain?.delete(label: label, rawLabel: rawLabel)
        try file.delete(label: label, rawLabel: rawLabel)
    }

    private func shouldFallback(status: ConnectKeyStoreError) -> Bool {
        guard case .keychainFailure(let code) = status else {
            return false
        }
        return code == errSecMissingEntitlement
            || code == errSecNotAvailable
            || code == errSecInteractionNotAllowed
            || code == errSecUnimplemented
            || code == errSecParam
    }
}

private struct KeychainBacking: Backing {
    private let service = "org.hyperledger.iroha.connect-keystore"
    private let accessGroup: String?
    private let accessControl: SecAccessControl?

    init?(configuration: ConnectKeyStore.Configuration) {
        accessGroup = configuration.appGroup
        accessControl = KeychainBacking.buildAccessControl(requireBiometrics: configuration.requireBiometrics,
                                                           requireDeviceLock: configuration.requireDeviceLock)
    }

    func load(label: String, rawLabel: String) throws -> ConnectKeyStore.StoredKey? {
        let context = LAContext()
        context.interactionNotAllowed = true
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: label,
            kSecAttrService as String: service,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecUseAuthenticationContext as String: context,
        ]
        if let accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        switch status {
        case errSecSuccess:
            guard let data = item as? Data else {
                throw ConnectKeyStoreError.keychainFailure(status)
            }
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601
            return try decoder.decode(ConnectKeyStore.StoredKey.self, from: data)
        case errSecItemNotFound:
            return nil
        default:
            throw ConnectKeyStoreError.keychainFailure(status)
        }
    }

    func save(label: String, rawLabel: String, stored: ConnectKeyStore.StoredKey) throws {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let data = try encoder.encode(stored)

        let context = LAContext()
        context.interactionNotAllowed = true
        var attributes: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: label,
            kSecAttrService as String: service,
            kSecValueData as String: data,
            kSecUseAuthenticationContext as String: context,
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
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrAccount as String: label,
                kSecAttrService as String: service,
            ]
            let update: [String: Any] = [kSecValueData as String: data]
            let updateStatus = SecItemUpdate(query as CFDictionary, update as CFDictionary)
            guard updateStatus == errSecSuccess else {
                throw ConnectKeyStoreError.keychainFailure(updateStatus)
            }
            return
        }
        guard status == errSecSuccess else {
            throw ConnectKeyStoreError.keychainFailure(status)
        }
    }

    func delete(label: String, rawLabel: String) throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: label,
            kSecAttrService as String: service,
        ]
        SecItemDelete(query as CFDictionary)
    }

    private static func buildAccessControl(requireBiometrics: Bool, requireDeviceLock: Bool) -> SecAccessControl? {
        var flags = SecAccessControlCreateFlags()
        if requireBiometrics {
            flags.insert(.userPresence)
        }

        let baseAccessibility: CFTypeRef = requireDeviceLock ? kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly : kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
        var error: Unmanaged<CFError>?
        if let control = SecAccessControlCreateWithFlags(nil, baseAccessibility, flags, &error) {
            return control
        }

        error?.release()
        return nil
    }
}

private struct IntegrityKeyProvider {
    private let keyFilename = ".integrity.key"
    private let baseDirectory: URL
    private let accessGroup: String?
    private let useKeychain: Bool
    private static let cacheLock = NSLock()
    private static nonisolated(unsafe) var cachedKeys: [String: SymmetricKey] = [:]

    init(baseDirectory: URL, accessGroup: String?, useKeychain: Bool) {
        self.baseDirectory = baseDirectory
        self.accessGroup = accessGroup
        self.useKeychain = useKeychain
    }

    func key() throws -> SymmetricKey {
        let cacheKey = Self.cacheKey(baseDirectory: baseDirectory,
                                     accessGroup: accessGroup,
                                     useKeychain: useKeychain)
        if let cached = Self.cachedKey(for: cacheKey) {
            return cached
        }

        let keyData = try (useKeychain ? loadFromKeychain() : nil) ?? loadFromFile()
        if let keyData {
            let key = SymmetricKey(data: keyData)
            Self.storeCached(key, for: cacheKey)
            return key
        }

        var bytes = [UInt8](repeating: 0, count: 32)
        let status = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        guard status == errSecSuccess else {
            throw ConnectKeyStoreError.hmacUnavailable
        }
        let data = Data(bytes)

        if !(useKeychain && storeInKeychain(data: data)) {
            try storeInFile(data: data)
        }

        let key = SymmetricKey(data: data)
        Self.storeCached(key, for: cacheKey)
        return key
    }

    private static func cacheKey(baseDirectory: URL, accessGroup: String?, useKeychain: Bool) -> String {
        "\(baseDirectory.standardizedFileURL.path)|\(accessGroup ?? "")|\(useKeychain)"
    }

    private static func cachedKey(for key: String) -> SymmetricKey? {
        cacheLock.lock()
        defer { cacheLock.unlock() }
        return cachedKeys[key]
    }

    private static func storeCached(_ key: SymmetricKey, for cacheKey: String) {
        cacheLock.lock()
        cachedKeys[cacheKey] = key
        cacheLock.unlock()
    }

    private func loadFromKeychain() throws -> Data? {
        let context = LAContext()
        context.interactionNotAllowed = true
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: "connect-keystore-integrity",
            kSecAttrService as String: "org.hyperledger.iroha.connect-keystore",
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
            kSecUseAuthenticationContext as String: context,
        ]
        if let accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }

        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        switch status {
        case errSecSuccess:
            guard let data = item as? Data else {
                throw ConnectKeyStoreError.keychainFailure(status)
            }
            guard data.count == 32 else {
                throw ConnectKeyStoreError.corrupt("integrity key length invalid")
            }
            return data
        case errSecItemNotFound, errSecMissingEntitlement, errSecNotAvailable, errSecInteractionNotAllowed, errSecUnimplemented, errSecParam:
            return nil
        default:
            throw ConnectKeyStoreError.keychainFailure(status)
        }
    }

    private func storeInKeychain(data: Data) -> Bool {
        let context = LAContext()
        context.interactionNotAllowed = true
        var attributes: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: "connect-keystore-integrity",
            kSecAttrService as String: "org.hyperledger.iroha.connect-keystore",
            kSecValueData as String: data,
            kSecUseAuthenticationContext as String: context,
            kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
        ]
        if let accessGroup {
            attributes[kSecAttrAccessGroup as String] = accessGroup
        }

        let status = SecItemAdd(attributes as CFDictionary, nil)
        if status == errSecDuplicateItem {
            let query: [String: Any] = [
                kSecClass as String: kSecClassGenericPassword,
                kSecAttrAccount as String: "connect-keystore-integrity",
                kSecAttrService as String: "org.hyperledger.iroha.connect-keystore",
            ]
            let update: [String: Any] = [kSecValueData as String: data]
            let updateStatus = SecItemUpdate(query as CFDictionary, update as CFDictionary)
            return updateStatus == errSecSuccess
        }
        return status == errSecSuccess
    }

    private func loadFromFile() throws -> Data? {
        let path = baseDirectory.appendingPathComponent(keyFilename, isDirectory: false)
        guard FileManager.default.fileExists(atPath: path.path) else {
            return nil
        }
        let data = try Data(contentsOf: path)
        guard data.count == 32 else {
            throw ConnectKeyStoreError.corrupt("integrity key file malformed")
        }
        return data
    }

    private func storeInFile(data: Data) throws {
        let path = baseDirectory.appendingPathComponent(keyFilename, isDirectory: false)
        try FileManager.default.createDirectory(at: baseDirectory, withIntermediateDirectories: true)
        #if os(iOS)
        let options: Data.WritingOptions = [.atomic, .completeFileProtectionUntilFirstUserAuthentication]
        #else
        let options: Data.WritingOptions = [.atomic]
        #endif
        try data.write(to: path, options: options)
        #if os(iOS)
        try FileManager.default.setAttributes([.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
                                              ofItemAtPath: path.path)
        #endif
    }
}

private struct StoredEnvelope: Codable, Equatable {
    let version: Int
    let payload: ConnectKeyStore.StoredKey
    let hmac: String
}

private func permutations<T>(_ items: [T]) -> [[T]] {
    guard items.count > 1 else {
        return [items]
    }
    var result: [[T]] = []
    for index in items.indices {
        var remaining = items
        let head = remaining.remove(at: index)
        for tail in permutations(remaining) {
            result.append([head] + tail)
        }
    }
    return result
}

private struct FileBacking: Backing {
    private let baseDirectory: URL
    private let integrity: IntegrityKeyProvider
    private let queue = DispatchQueue(label: "org.hyperledger.iroha.connect-keystore")

    private enum PayloadKey: String {
        case attestation
        case keyPair
    }

    private enum KeyPairKey: String {
        case publicKey
        case privateKey
    }

    private enum AttestationKey: String {
        case publicKeyDigest
        case deviceLabel
        case createdAt
    }

    private struct HmacPayloadValues {
        let publicKey: String
        let privateKey: String
        let publicKeyDigest: String
        let deviceLabel: String
        let createdAt: String

        init(stored: ConnectKeyStore.StoredKey) throws {
            publicKey = try FileBacking.encodeJSONString(stored.keyPair.publicKey.base64EncodedString())
            privateKey = try FileBacking.encodeJSONString(stored.keyPair.privateKey.base64EncodedString())
            publicKeyDigest = try FileBacking.encodeJSONString(stored.attestation.publicKeyDigest.base64EncodedString())
            deviceLabel = try FileBacking.encodeJSONString(stored.attestation.deviceLabel)
            createdAt = try FileBacking.encodeDateJSONString(stored.attestation.createdAt)
        }
    }

    private static let canonicalPayloadOrder: [PayloadKey] = [.attestation, .keyPair]
    private static let canonicalKeyPairOrder: [KeyPairKey] = [.privateKey, .publicKey]
    private static let canonicalAttestationOrder: [AttestationKey] = [.createdAt, .deviceLabel, .publicKeyDigest]

    private static let legacyPayloadOrders = permutations([PayloadKey.attestation, .keyPair])
    private static let legacyKeyPairOrders = permutations([KeyPairKey.publicKey, .privateKey])
    private static let legacyAttestationOrders = permutations([AttestationKey.publicKeyDigest, .deviceLabel, .createdAt])

    init(baseDirectory: URL, integrity: IntegrityKeyProvider) {
        self.baseDirectory = baseDirectory
        self.integrity = integrity
    }

    static func defaultBaseDirectory() -> URL {
        let root = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSTemporaryDirectory(), isDirectory: true)
        return root.appendingPathComponent("iroha_connect_keystore", isDirectory: true)
    }

    func load(label: String, rawLabel _: String) throws -> ConnectKeyStore.StoredKey? {
        try queue.sync {
            let url = try canonicalPath(for: label)
            guard FileManager.default.fileExists(atPath: url.path) else {
                return nil
            }
            let data = try Data(contentsOf: url)
            let decoder = JSONDecoder()
            decoder.dateDecodingStrategy = .iso8601

            do {
                let envelope = try decoder.decode(StoredEnvelope.self, from: data)
                try verifyEnvelope(envelope, label: label)
                return envelope.payload
            } catch let storeError as ConnectKeyStoreError {
                throw storeError
            } catch {
                throw ConnectKeyStoreError.corrupt("failed to decode stored entry: \(error)")
            }
        }
    }

    func save(label: String, rawLabel _: String, stored: ConnectKeyStore.StoredKey) throws {
        try queue.sync {
            let url = try canonicalPath(for: label)
            try FileManager.default.createDirectory(at: baseDirectory, withIntermediateDirectories: true)
            let envelope = try envelopeForSave(stored: stored, label: label)
            let encoded = try encoder().encode(envelope)
            try writeProtected(data: encoded, to: url)
        }
    }

    func delete(label: String, rawLabel _: String) throws {
        try queue.sync {
            let url = try canonicalPath(for: label)
            if FileManager.default.fileExists(atPath: url.path) {
                try FileManager.default.removeItem(at: url)
            }
        }
    }

    private func verifyEnvelope(_ envelope: StoredEnvelope, label: String) throws {
        let key = try integrity.key()
        guard let decoded = Data(base64Encoded: envelope.hmac) else {
            throw ConnectKeyStoreError.corrupt("stored hmac is not valid base64")
        }
        let values = try HmacPayloadValues(stored: envelope.payload)
        let canonicalPayload = payloadData(values: values,
                                           payloadOrder: Self.canonicalPayloadOrder,
                                           keyPairOrder: Self.canonicalKeyPairOrder,
                                           attestationOrder: Self.canonicalAttestationOrder)
        let expected = computeHmac(label: label, key: key, payload: canonicalPayload)
        if decoded == expected {
            try validateAttestation(for: envelope.payload)
            return
        }
        if matchesLegacyHmac(decoded: decoded, label: label, key: key, values: values) {
            try validateAttestation(for: envelope.payload)
            return
        }
        throw ConnectKeyStoreError.integrityMismatch
    }

    private func envelopeForSave(stored: ConnectKeyStore.StoredKey, label: String) throws -> StoredEnvelope {
        let values = try HmacPayloadValues(stored: stored)
        let payload = payloadData(values: values,
                                  payloadOrder: Self.canonicalPayloadOrder,
                                  keyPairOrder: Self.canonicalKeyPairOrder,
                                  attestationOrder: Self.canonicalAttestationOrder)
        let key = try integrity.key()
        let digest = computeHmac(label: label, key: key, payload: payload).base64EncodedString()
        return StoredEnvelope(version: 1, payload: stored, hmac: digest)
    }

    private func validateAttestation(for stored: ConnectKeyStore.StoredKey) throws {
        let digest = Data(SHA256.hash(data: stored.keyPair.publicKey))
        guard stored.attestation.publicKeyDigest == digest else {
            throw ConnectKeyStoreError.integrityMismatch
        }
    }

    private func canonicalPath(for label: String) throws -> URL {
        try FileManager.default.createDirectory(at: baseDirectory, withIntermediateDirectories: true)
        let sanitized = path(for: label)
        return sanitized
    }

    private func path(for label: String) -> URL {
        baseDirectory.appendingPathComponent("\(label).json", isDirectory: false)
    }

    private func matchesLegacyHmac(decoded: Data,
                                   label: String,
                                   key: SymmetricKey,
                                   values: HmacPayloadValues) -> Bool {
        for payloadOrder in Self.legacyPayloadOrders {
            for keyPairOrder in Self.legacyKeyPairOrders {
                for attestationOrder in Self.legacyAttestationOrders {
                    let payload = payloadData(values: values,
                                              payloadOrder: payloadOrder,
                                              keyPairOrder: keyPairOrder,
                                              attestationOrder: attestationOrder)
                    let candidate = computeHmac(label: label, key: key, payload: payload)
                    if candidate == decoded {
                        return true
                    }
                }
            }
        }
        return false
    }

    private func payloadData(values: HmacPayloadValues,
                             payloadOrder: [PayloadKey],
                             keyPairOrder: [KeyPairKey],
                             attestationOrder: [AttestationKey]) -> Data {
        let keyPair = jsonObject(keys: keyPairOrder) { key in
            switch key {
            case .publicKey:
                values.publicKey
            case .privateKey:
                values.privateKey
            }
        }
        let attestation = jsonObject(keys: attestationOrder) { key in
            switch key {
            case .publicKeyDigest:
                values.publicKeyDigest
            case .deviceLabel:
                values.deviceLabel
            case .createdAt:
                values.createdAt
            }
        }
        let payload = jsonObject(keys: payloadOrder) { key in
            switch key {
            case .attestation:
                attestation
            case .keyPair:
                keyPair
            }
        }
        return Data(payload.utf8)
    }

    private func jsonObject<Key>(keys: [Key], value: (Key) -> String) -> String
        where Key: RawRepresentable, Key.RawValue == String {
        let parts = keys.map { key in
            "\"\(key.rawValue)\":\(value(key))"
        }
        return "{\(parts.joined(separator: ","))}"
    }

    private func computeHmac(label: String, key: SymmetricKey, payload: Data) -> Data {
        var hmac = HMAC<SHA256>(key: key)
        hmac.update(data: Data(label.utf8))
        hmac.update(data: payload)
        return Data(hmac.finalize())
    }

    private static func encodeJSONString(_ value: String) throws -> String {
        let encoder = JSONEncoder()
        let data = try encoder.encode(value)
        guard let string = String(data: data, encoding: .utf8) else {
            throw ConnectKeyStoreError.corrupt("failed to encode hmac payload")
        }
        return string
    }

    private static func encodeDateJSONString(_ date: Date) throws -> String {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        let data = try encoder.encode(date)
        guard let string = String(data: data, encoding: .utf8) else {
            throw ConnectKeyStoreError.corrupt("failed to encode hmac payload")
        }
        return string
    }

    private func encoder() -> JSONEncoder {
        let encoder = JSONEncoder()
        encoder.dateEncodingStrategy = .iso8601
        return encoder
    }

    private func writeProtected(data: Data, to url: URL) throws {
        #if os(iOS)
        let options: Data.WritingOptions = [.atomic, .completeFileProtectionUntilFirstUserAuthentication]
        #else
        let options: Data.WritingOptions = [.atomic]
        #endif
        try data.write(to: url, options: options)
        #if os(iOS)
        try FileManager.default.setAttributes([.protectionKey: FileProtectionType.completeUntilFirstUserAuthentication],
                                              ofItemAtPath: url.path)
        #endif
    }
}
