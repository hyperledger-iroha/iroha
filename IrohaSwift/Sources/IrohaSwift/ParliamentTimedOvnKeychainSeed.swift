// Generation-bound Keychain custody for Parliament timed-OVN root seeds.

import CryptoKit
import Foundation
import Security

/// Fail-closed errors from the production Parliament timed-OVN seed store.
public enum ParliamentTimedOvnKeychainSeedError: Error, Equatable, Sendable {
    /// The alias is empty, noncanonical, or exceeds the fixed bound.
    case invalidAlias
    /// A live generation already occupies the requested alias.
    case aliasAlreadyExists
    /// No protected seed exists under the requested alias.
    case seedNotFound
    /// The handle names a generation that has been deleted or replaced.
    case staleHandle
    /// The protected item has a malformed version, width, binding, or seed.
    case corruptEnvelope
    /// The handle belongs to another store configuration.
    case foreignHandle
    /// Secure random generation failed.
    case randomFailure(OSStatus)
    /// A Keychain operation failed.
    case keychainFailure(OSStatus)
}

/// Opaque, generation-bound Keychain handle for one Parliament timed-OVN seed.
///
/// Equality includes the immutable random generation. Deleting and recreating
/// an alias therefore never retargets an existing handle to replacement key
/// material.
public final class ParliamentTimedOvnKeychainSeedHandle:
    ParliamentTimedOvnSeedHandle,
    @unchecked Sendable,
    Equatable,
    Hashable
{
    /// Canonical application-selected label. This is not secret material.
    public let alias: String

    fileprivate let generation: Data
    fileprivate let service: String
    fileprivate let accessGroup: String?
    fileprivate let storage: any ParliamentTimedOvnSeedStorage
    fileprivate let operationLock: NSLock

    fileprivate init(
        alias: String,
        generation: Data,
        service: String,
        accessGroup: String?,
        storage: any ParliamentTimedOvnSeedStorage,
        operationLock: NSLock
    ) {
        self.alias = alias
        self.generation = generation
        self.service = service
        self.accessGroup = accessGroup
        self.storage = storage
        self.operationLock = operationLock
    }

    /// Borrow exactly 32 bytes from the current, generation-matched Keychain
    /// item. The store lock remains held for the operation, so deletion cannot
    /// race an in-flight native proof construction.
    public func withUnsafeSeedBytes(
        _ body: (UnsafeRawBufferPointer) throws -> Data
    ) throws -> Data {
        try operationLock.withParliamentTimedOvnLock {
            guard var envelope = try storage.load(
                service: service,
                alias: alias,
                accessGroup: accessGroup
            ) else {
                throw ParliamentTimedOvnKeychainSeedError.staleHandle
            }
            defer { envelope.parliamentTimedOvnWipe() }
            _ = try ParliamentTimedOvnSeedEnvelope.validate(
                &envelope,
                service: service,
                alias: alias,
                expectedGeneration: generation
            )
            return try envelope.withUnsafeMutableBytes { bytes in
                guard let base = bytes.baseAddress else {
                    throw ParliamentTimedOvnKeychainSeedError.corruptEnvelope
                }
                let seed = UnsafeRawBufferPointer(
                    start: base.advanced(by: ParliamentTimedOvnSeedEnvelope.seedOffset),
                    count: ParliamentTimedOvnSeedEnvelope.seedBytes
                )
                return try body(seed)
            }
        }
    }

    public static func == (
        lhs: ParliamentTimedOvnKeychainSeedHandle,
        rhs: ParliamentTimedOvnKeychainSeedHandle
    ) -> Bool {
        lhs.alias == rhs.alias
            && lhs.generation == rhs.generation
            && lhs.service == rhs.service
            && lhs.accessGroup == rhs.accessGroup
    }

    public func hash(into hasher: inout Hasher) {
        hasher.combine(alias)
        hasher.combine(generation)
        hasher.combine(service)
        hasher.combine(accessGroup)
    }
}

/// Production, Keychain-only custody for Parliament timed-OVN root seeds.
///
/// Items use `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`; they are neither
/// synchronized nor backed by a file/plaintext fallback. The arbitrary
/// 32-byte root seed is stored as Keychain data rather than being represented
/// as a Secure Enclave key, which would misstate the enclave's supported key
/// types.
public final class ParliamentTimedOvnKeychainSeedStore: @unchecked Sendable {
    private static let service = "org.hyperledger.iroha.parliament-timed-ovn-seed.v1"
    private static let productionLock = NSLock()
    private static let productionStorage = ParliamentTimedOvnSystemKeychainStorage()

    private let accessGroup: String?
    private let storage: any ParliamentTimedOvnSeedStorage
    private let operationLock: NSLock
    private let randomBytes: @Sendable (Int) throws -> Data

    /// Construct the production Keychain-only store.
    public init(accessGroup: String? = nil) {
        self.accessGroup = accessGroup
        self.storage = Self.productionStorage
        self.operationLock = Self.productionLock
        self.randomBytes = { count in try Self.secureRandomBytes(count: count) }
    }

    init(
        accessGroup: String? = nil,
        storage: any ParliamentTimedOvnSeedStorage,
        operationLock: NSLock = NSLock(),
        randomBytes: @escaping @Sendable (Int) throws -> Data
    ) {
        self.accessGroup = accessGroup
        self.storage = storage
        self.operationLock = operationLock
        self.randomBytes = randomBytes
    }

    /// Generate and persist one fresh 32-byte root seed.
    ///
    /// This operation never overwrites an existing alias.
    public func create(alias: String) throws -> ParliamentTimedOvnKeychainSeedHandle {
        try Self.validateAlias(alias)
        return try operationLock.withParliamentTimedOvnLock {
            if var existing = try storage.load(
                service: Self.service,
                alias: alias,
                accessGroup: accessGroup
            ) {
                existing.parliamentTimedOvnWipe()
                throw ParliamentTimedOvnKeychainSeedError.aliasAlreadyExists
            }

            var generation = try randomBytes(ParliamentTimedOvnSeedEnvelope.generationBytes)
            defer { generation.parliamentTimedOvnWipe() }
            var seed = try randomBytes(ParliamentTimedOvnSeedEnvelope.seedBytes)
            defer { seed.parliamentTimedOvnWipe() }
            guard generation.count == ParliamentTimedOvnSeedEnvelope.generationBytes,
                  generation.contains(where: { $0 != 0 }),
                  seed.count == ParliamentTimedOvnSeedEnvelope.seedBytes,
                  seed.contains(where: { $0 != 0 }) else {
                throw ParliamentTimedOvnKeychainSeedError.randomFailure(errSecParam)
            }
            var envelope = ParliamentTimedOvnSeedEnvelope.make(
                service: Self.service,
                alias: alias,
                generation: generation,
                seed: seed
            )
            defer { envelope.parliamentTimedOvnWipe() }
            try storage.insert(
                envelope,
                generation: generation,
                service: Self.service,
                alias: alias,
                accessGroup: accessGroup
            )
            return ParliamentTimedOvnKeychainSeedHandle(
                alias: alias,
                generation: generation,
                service: Self.service,
                accessGroup: accessGroup,
                storage: storage,
                operationLock: operationLock
            )
        }
    }

    /// Open the exact generation currently stored under an alias.
    public func open(alias: String) throws -> ParliamentTimedOvnKeychainSeedHandle {
        try Self.validateAlias(alias)
        return try operationLock.withParliamentTimedOvnLock {
            guard var envelope = try storage.load(
                service: Self.service,
                alias: alias,
                accessGroup: accessGroup
            ) else {
                throw ParliamentTimedOvnKeychainSeedError.seedNotFound
            }
            defer { envelope.parliamentTimedOvnWipe() }
            let generation = try ParliamentTimedOvnSeedEnvelope.validate(
                &envelope,
                service: Self.service,
                alias: alias,
                expectedGeneration: nil
            )
            return ParliamentTimedOvnKeychainSeedHandle(
                alias: alias,
                generation: generation,
                service: Self.service,
                accessGroup: accessGroup,
                storage: storage,
                operationLock: operationLock
            )
        }
    }

    /// Delete only the generation named by `handle`.
    ///
    /// A stale handle can never delete a replacement seed that reused its
    /// human-readable alias.
    public func delete(_ handle: ParliamentTimedOvnKeychainSeedHandle) throws {
        guard handle.service == Self.service,
              handle.accessGroup == accessGroup,
              (handle.storage as AnyObject) === (storage as AnyObject),
              handle.operationLock === operationLock else {
            throw ParliamentTimedOvnKeychainSeedError.foreignHandle
        }
        try operationLock.withParliamentTimedOvnLock {
            guard var envelope = try storage.load(
                service: Self.service,
                alias: handle.alias,
                accessGroup: accessGroup
            ) else {
                throw ParliamentTimedOvnKeychainSeedError.staleHandle
            }
            defer { envelope.parliamentTimedOvnWipe() }
            _ = try ParliamentTimedOvnSeedEnvelope.validate(
                &envelope,
                service: Self.service,
                alias: handle.alias,
                expectedGeneration: handle.generation
            )
            try storage.delete(
                service: Self.service,
                alias: handle.alias,
                accessGroup: accessGroup,
                generation: handle.generation
            )
        }
    }

    private static func validateAlias(_ alias: String) throws {
        guard !alias.isEmpty,
              alias.count <= 64,
              alias == alias.trimmingCharacters(in: .whitespacesAndNewlines),
              alias.unicodeScalars.allSatisfy({ scalar in
                  let value = scalar.value
                  return (0x30 ... 0x39).contains(value)
                      || (0x41 ... 0x5A).contains(value)
                      || (0x61 ... 0x7A).contains(value)
                      || value == 0x2D
                      || value == 0x2E
                      || value == 0x5F
              }) else {
            throw ParliamentTimedOvnKeychainSeedError.invalidAlias
        }
    }

    private static func secureRandomBytes(count: Int) throws -> Data {
        var bytes = Data(count: count)
        let status = bytes.withUnsafeMutableBytes { buffer -> OSStatus in
            guard let base = buffer.baseAddress else { return errSecAllocate }
            return SecRandomCopyBytes(kSecRandomDefault, count, base)
        }
        guard status == errSecSuccess else {
            bytes.parliamentTimedOvnWipe()
            throw ParliamentTimedOvnKeychainSeedError.randomFailure(status)
        }
        return bytes
    }
}

protocol ParliamentTimedOvnSeedStorage: AnyObject, Sendable {
    func load(service: String, alias: String, accessGroup: String?) throws -> Data?
    func insert(
        _ data: Data,
        generation: Data,
        service: String,
        alias: String,
        accessGroup: String?
    ) throws
    func delete(
        service: String,
        alias: String,
        accessGroup: String?,
        generation: Data
    ) throws
}

private final class ParliamentTimedOvnSystemKeychainStorage:
    ParliamentTimedOvnSeedStorage,
    @unchecked Sendable
{
    func load(service: String, alias: String, accessGroup: String?) throws -> Data? {
        var query = baseQuery(service: service, alias: alias, accessGroup: accessGroup)
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &item)
        switch status {
        case errSecSuccess:
            guard let data = item as? Data else {
                throw ParliamentTimedOvnKeychainSeedError.keychainFailure(errSecDecode)
            }
            return data
        case errSecItemNotFound:
            return nil
        default:
            throw ParliamentTimedOvnKeychainSeedError.keychainFailure(status)
        }
    }

    func insert(
        _ data: Data,
        generation: Data,
        service: String,
        alias: String,
        accessGroup: String?
    ) throws {
        var attributes = baseQuery(service: service, alias: alias, accessGroup: accessGroup)
        attributes[kSecValueData as String] = data
        attributes[kSecAttrGeneric as String] = generation
        attributes[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        let status = SecItemAdd(attributes as CFDictionary, nil)
        if status == errSecDuplicateItem {
            throw ParliamentTimedOvnKeychainSeedError.aliasAlreadyExists
        }
        guard status == errSecSuccess else {
            throw ParliamentTimedOvnKeychainSeedError.keychainFailure(status)
        }
    }

    func delete(
        service: String,
        alias: String,
        accessGroup: String?,
        generation: Data
    ) throws {
        var query = baseQuery(service: service, alias: alias, accessGroup: accessGroup)
        // Make deletion generation-conditional in Keychain itself. This closes
        // the load/delete race with another process in the same access group.
        query[kSecAttrGeneric as String] = generation
        let status = SecItemDelete(query as CFDictionary)
        if status == errSecItemNotFound {
            throw ParliamentTimedOvnKeychainSeedError.staleHandle
        }
        guard status == errSecSuccess else {
            throw ParliamentTimedOvnKeychainSeedError.keychainFailure(status)
        }
    }

    private func baseQuery(
        service: String,
        alias: String,
        accessGroup: String?
    ) -> [String: Any] {
        var query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: alias,
            kSecAttrSynchronizable as String: kCFBooleanFalse as Any,
        ]
        if let accessGroup {
            query[kSecAttrAccessGroup as String] = accessGroup
        }
        return query
    }
}

enum ParliamentTimedOvnSeedEnvelope {
    static let version: UInt8 = 1
    static let generationBytes = 16
    static let bindingBytes = 32
    static let seedBytes = 32
    static let generationOffset = 1
    static let bindingOffset = generationOffset + generationBytes
    static let seedOffset = bindingOffset + bindingBytes
    static let encodedBytes = seedOffset + seedBytes
    private static let bindingDomain = Data(
        "iroha.swift.parliament.timed-ovn.keychain-seed.binding.v1\0".utf8
    )

    static func make(
        service: String,
        alias: String,
        generation: Data,
        seed: Data
    ) -> Data {
        var envelope = Data(capacity: encodedBytes)
        envelope.append(version)
        envelope.append(generation)
        envelope.append(binding(service: service, alias: alias, generation: generation))
        envelope.append(seed)
        return envelope
    }

    static func validate(
        _ envelope: inout Data,
        service: String,
        alias: String,
        expectedGeneration: Data?
    ) throws -> Data {
        guard envelope.count == encodedBytes,
              envelope.first == version else {
            throw ParliamentTimedOvnKeychainSeedError.corruptEnvelope
        }
        let generation = Data(
            envelope[generationOffset ..< generationOffset + generationBytes]
        )
        guard generation.contains(where: { $0 != 0 }) else {
            throw ParliamentTimedOvnKeychainSeedError.corruptEnvelope
        }
        if let expectedGeneration, generation != expectedGeneration {
            throw ParliamentTimedOvnKeychainSeedError.staleHandle
        }
        let observedBinding = Data(
            envelope[bindingOffset ..< bindingOffset + bindingBytes]
        )
        guard observedBinding == binding(
            service: service,
            alias: alias,
            generation: generation
        ), envelope[seedOffset ..< seedOffset + seedBytes].contains(where: { $0 != 0 }) else {
            throw ParliamentTimedOvnKeychainSeedError.corruptEnvelope
        }
        return generation
    }

    private static func binding(service: String, alias: String, generation: Data) -> Data {
        var material = bindingDomain
        appendLengthDelimited(Data(service.utf8), to: &material)
        appendLengthDelimited(Data(alias.utf8), to: &material)
        appendLengthDelimited(generation, to: &material)
        defer { material.parliamentTimedOvnWipe() }
        return Data(SHA256.hash(data: material))
    }

    private static func appendLengthDelimited(_ value: Data, to output: inout Data) {
        var length = UInt64(value.count).bigEndian
        withUnsafeBytes(of: &length) { output.append(contentsOf: $0) }
        output.append(value)
    }
}

private extension NSLock {
    func withParliamentTimedOvnLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}

private extension Data {
    mutating func parliamentTimedOvnWipe() {
        withUnsafeMutableBytes { bytes in
            _ = bytes.initializeMemory(as: UInt8.self, repeating: 0)
        }
        removeAll(keepingCapacity: false)
    }
}
