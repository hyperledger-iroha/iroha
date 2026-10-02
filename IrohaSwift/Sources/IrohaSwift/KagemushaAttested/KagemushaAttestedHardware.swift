import CryptoKit
import Foundation
import Security
#if canImport(DeviceCheck)
import DeviceCheck
#endif

/// A non-exportable device transfer key. There is no raw-sign or key-export surface outside
/// the suite: callers only sign canonical suite messages.
protocol KagemushaAttestedDeviceKey: Sendable {
    /// 65-byte uncompressed SEC1 public key.
    var publicKey: Data { get }
    /// ECDSA P-256/SHA-256 over `message`, normalized to low-S `r || s`.
    func sign(_ message: Data) throws -> Data
}

/// Persistent storage for device keys, keyed by a per-scheme, per-account label.
protocol KagemushaAttestedKeyStore: Sendable {
    /// `nil` when keys can be created here, otherwise why not.
    func unsupportedReason() -> KagemushaUnsupportedReason?
    func exists(label: String) -> Bool
    func load(label: String) throws -> (any KagemushaAttestedDeviceKey)?
    func create(label: String) throws -> any KagemushaAttestedDeviceKey
    func delete(label: String) throws
}

/// Vendor attestation of the app instance that owns the device key.
protocol KagemushaAttestedAttestationProvider: Sendable {
    /// Platform recorded in the certificate.
    var platform: KagemushaPlatform { get }
    /// Wire name of the attestation format sent to the issuer.
    var format: String { get }
    func unsupportedReason() -> KagemushaUnsupportedReason?
    /// Create an attested app key and return its identifier (raw bytes).
    func generateKey() async throws -> Data
    /// Attest `keyId` with `clientDataHash`.
    func attestKey(_ keyId: Data, clientDataHash: Data) async throws -> Data
    /// Produce an online assertion for `keyId` over `clientDataHash`.
    func generateAssertion(_ keyId: Data, clientDataHash: Data) async throws -> Data
}

#if canImport(CryptoKit) && !os(watchOS)
/// Secure Enclave transfer key. The key blob is an SE-wrapped handle stored in the Keychain as
/// `WhenUnlockedThisDeviceOnly`, never synchronized, and usable only by this app.
final class KagemushaSecureEnclaveKeyStore: KagemushaAttestedKeyStore, @unchecked Sendable {
    static let service = "org.hyperledger.iroha.kagemusha.attested.device-key"

    private struct SecureEnclaveKey: KagemushaAttestedDeviceKey, @unchecked Sendable {
        let key: SecureEnclave.P256.Signing.PrivateKey
        var publicKey: Data { key.publicKey.x963Representation }

        func sign(_ message: Data) throws -> Data {
            do {
                return KagemushaAttestedCrypto.normalizeLowS(try key.signature(for: message).rawRepresentation)
            } catch {
                throw KagemushaError.keyStore("secure enclave signing failed: \(error.localizedDescription)")
            }
        }
    }

    init() {}

    func unsupportedReason() -> KagemushaUnsupportedReason? {
        SecureEnclave.isAvailable ? nil : .secureEnclaveUnavailable
    }

    func exists(label: String) -> Bool {
        (try? readBlob(label: label)) != nil
    }

    func load(label: String) throws -> (any KagemushaAttestedDeviceKey)? {
        guard let blob = try readBlob(label: label) else { return nil }
        do {
            return SecureEnclaveKey(key: try SecureEnclave.P256.Signing.PrivateKey(dataRepresentation: blob))
        } catch {
            // A restored blob from another device's enclave cannot be opened here.
            throw KagemushaError.keyLost
        }
    }

    func create(label: String) throws -> any KagemushaAttestedDeviceKey {
        guard SecureEnclave.isAvailable else { throw KagemushaError.unsupported(.secureEnclaveUnavailable) }
        var error: Unmanaged<CFError>?
        guard let access = SecAccessControlCreateWithFlags(
            nil, kSecAttrAccessibleWhenUnlockedThisDeviceOnly, .privateKeyUsage, &error)
        else {
            throw KagemushaError.keyStore("access control: \(String(describing: error?.takeRetainedValue()))")
        }
        let key: SecureEnclave.P256.Signing.PrivateKey
        do {
            key = try SecureEnclave.P256.Signing.PrivateKey(compactRepresentable: false, accessControl: access)
        } catch {
            throw KagemushaError.keyStore("secure enclave key generation failed: \(error.localizedDescription)")
        }
        try writeBlob(key.dataRepresentation, label: label)
        return SecureEnclaveKey(key: key)
    }

    func delete(label: String) throws {
        let status = SecItemDelete(baseQuery(label: label) as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else {
            throw KagemushaError.keyStore("keychain delete failed: \(status)")
        }
    }

    private func baseQuery(label: String) -> [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: Self.service,
            kSecAttrAccount as String: label,
            kSecAttrSynchronizable as String: kCFBooleanFalse as Any,
            kSecUseDataProtectionKeychain as String: true,
        ]
    }

    private func readBlob(label: String) throws -> Data? {
        var query = baseQuery(label: label)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        switch status {
        case errSecSuccess:
            guard let data = result as? Data else { throw KagemushaError.keyStore("keychain item is not data") }
            return data
        case errSecItemNotFound:
            return nil
        default:
            throw KagemushaError.keyStore("keychain read failed: \(status)")
        }
    }

    private func writeBlob(_ blob: Data, label: String) throws {
        var query = baseQuery(label: label)
        query[kSecValueData as String] = blob
        query[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KagemushaError.keyStore("keychain write failed: \(status)")
        }
    }
}
#endif

#if canImport(DeviceCheck) && (os(iOS) || os(macOS) || os(tvOS) || os(visionOS))
/// Apple App Attest. Production attestation of the app instance that holds the transfer key.
@available(iOS 14.0, macOS 11.0, tvOS 15.0, *)
struct KagemushaAppAttestProvider: KagemushaAttestedAttestationProvider {
    let platform = KagemushaPlatform.appleSecureEnclave
    let format = "apple-app-attest"

    func unsupportedReason() -> KagemushaUnsupportedReason? {
        DCAppAttestService.shared.isSupported ? nil : .appAttestUnavailable
    }

    func generateKey() async throws -> Data {
        do {
            let identifier = try await DCAppAttestService.shared.generateKey()
            guard let keyId = Data(base64Encoded: identifier), keyId.count == 32 else {
                throw KagemushaError.attestation("App Attest returned a malformed key identifier")
            }
            return keyId
        } catch let error as KagemushaError {
            throw error
        } catch {
            throw KagemushaError.attestation("App Attest key generation failed: \(error.localizedDescription)")
        }
    }

    func attestKey(_ keyId: Data, clientDataHash: Data) async throws -> Data {
        do {
            return try await DCAppAttestService.shared.attestKey(
                keyId.base64EncodedString(), clientDataHash: clientDataHash)
        } catch {
            throw KagemushaError.attestation("App Attest attestation failed: \(error.localizedDescription)")
        }
    }

    func generateAssertion(_ keyId: Data, clientDataHash: Data) async throws -> Data {
        do {
            return try await DCAppAttestService.shared.generateAssertion(
                keyId.base64EncodedString(), clientDataHash: clientDataHash)
        } catch {
            throw KagemushaError.attestation("App Attest assertion failed: \(error.localizedDescription)")
        }
    }
}
#endif

/// Production hardware selection. There is never a software fallback: when either the Secure
/// Enclave or App Attest is missing the wallet reports ``KagemushaStatus/unsupported(_:)``.
enum KagemushaAttestedProductionHardware {
    static func keyStore() -> any KagemushaAttestedKeyStore {
        #if canImport(CryptoKit) && !os(watchOS)
        return KagemushaSecureEnclaveKeyStore()
        #else
        return KagemushaUnavailableHardware()
        #endif
    }

    static func attestation() -> any KagemushaAttestedAttestationProvider {
        #if canImport(DeviceCheck) && (os(iOS) || os(macOS) || os(tvOS) || os(visionOS))
        if #available(iOS 14.0, macOS 11.0, tvOS 15.0, *) {
            return KagemushaAppAttestProvider()
        }
        #endif
        return KagemushaUnavailableHardware()
    }
}

/// Reports honest unavailability on platforms without the required hardware services.
struct KagemushaUnavailableHardware: KagemushaAttestedKeyStore, KagemushaAttestedAttestationProvider {
    let platform = KagemushaPlatform.appleSecureEnclave
    let format = "unavailable"

    func unsupportedReason() -> KagemushaUnsupportedReason? { .platformUnsupported }
    func exists(label: String) -> Bool { false }
    func load(label: String) throws -> (any KagemushaAttestedDeviceKey)? { nil }
    func create(label: String) throws -> any KagemushaAttestedDeviceKey {
        throw KagemushaError.unsupported(.platformUnsupported)
    }
    func delete(label: String) throws {}
    func generateKey() async throws -> Data { throw KagemushaError.unsupported(.platformUnsupported) }
    func attestKey(_ keyId: Data, clientDataHash: Data) async throws -> Data {
        throw KagemushaError.unsupported(.platformUnsupported)
    }
    func generateAssertion(_ keyId: Data, clientDataHash: Data) async throws -> Data {
        throw KagemushaError.unsupported(.platformUnsupported)
    }
}

#if DEBUG
/// Debug-only software keys for `KagemushaWallet.forTesting`. Usable only with a descriptor
/// that sets `allow_test_devices` and a configuration that sets `testing`; production issuers
/// refuse test enrollments.
public final class KagemushaTestKeyStore: KagemushaAttestedKeyStore, @unchecked Sendable {
    private struct SoftwareKey: KagemushaAttestedDeviceKey, @unchecked Sendable {
        let key: P256.Signing.PrivateKey
        var publicKey: Data { key.publicKey.x963Representation }
        func sign(_ message: Data) throws -> Data {
            try KagemushaAttestedCrypto.softwareSign(message, key: key)
        }
    }

    private let lock = NSLock()
    private var keys: [String: P256.Signing.PrivateKey] = [:]

    public init() {}

    func unsupportedReason() -> KagemushaUnsupportedReason? { nil }

    func exists(label: String) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return keys[label] != nil
    }

    func load(label: String) throws -> (any KagemushaAttestedDeviceKey)? {
        lock.lock()
        defer { lock.unlock() }
        return keys[label].map(SoftwareKey.init)
    }

    func create(label: String) throws -> any KagemushaAttestedDeviceKey {
        lock.lock()
        defer { lock.unlock() }
        let key = P256.Signing.PrivateKey()
        keys[label] = key
        return SoftwareKey(key: key)
    }

    func delete(label: String) throws {
        lock.lock()
        defer { lock.unlock() }
        keys[label] = nil
    }

    /// Simulate loss of every stored key (for drills).
    public func removeAll() {
        lock.lock()
        defer { lock.unlock() }
        keys.removeAll()
    }
}

/// Debug-only attestation stand-in for test issuers. The evidence is a fixed marker over the
/// client data hash and proves nothing about hardware.
public struct KagemushaTestAttestationProvider: KagemushaAttestedAttestationProvider {
    public static let marker = Data("iroha:kagemusha:v1:attested-app:test-attestation\0".utf8)
    let platform = KagemushaPlatform.appleSecureEnclave
    let format = "test"

    public init() {}

    func unsupportedReason() -> KagemushaUnsupportedReason? { nil }
    func generateKey() async throws -> Data { KagemushaAttestedCrypto.randomBytes(32) }
    func attestKey(_ keyId: Data, clientDataHash: Data) async throws -> Data {
        Self.marker + keyId + clientDataHash
    }
    func generateAssertion(_ keyId: Data, clientDataHash: Data) async throws -> Data {
        Self.marker + keyId + clientDataHash
    }
}
#endif
