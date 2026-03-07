import Foundation

public enum AccountAddressError: Error, Equatable {
    case unsupportedAlgorithm(String)
    case keyPayloadTooLong(Int)
    case invalidHeaderVersion(UInt8)
    case invalidNormVersion(UInt8)
    case invalidIh58Prefix(UInt16)
    case hashFailure
    case invalidIh58Encoding
    case invalidLength
    case checksumMismatch
    case invalidHexAddress
    case domainMismatch
    case invalidDomainLabel(String)
    case unexpectedNetworkPrefix(expected: UInt16, found: UInt16)
    case unknownAddressClass(UInt8)
    case unknownDomainTag(UInt8)
    case unexpectedExtensionFlag
    case unknownControllerTag(UInt8)
    case invalidPublicKey
    case unknownCurve(UInt8)
    case unexpectedTrailingBytes
    case invalidIh58PrefixEncoding(UInt8)
    case missingCompressedSentinel
    case compressedTooShort
    case invalidCompressedChar(Character)
    case invalidCompressedBase
    case invalidCompressedDigit(Int)
    case unsupportedAddressFormat
    case multisigMemberOverflow(Int)
    case invalidMultisigPolicy(String)

    /// Stable Norito error code (`ERR_*`) that mirrors the Rust data model.
    public var code: String {
        switch self {
        case .unsupportedAlgorithm:
            return "ERR_UNSUPPORTED_ALGORITHM"
        case .keyPayloadTooLong:
            return "ERR_KEY_PAYLOAD_TOO_LONG"
        case .invalidHeaderVersion:
            return "ERR_INVALID_HEADER_VERSION"
        case .invalidNormVersion:
            return "ERR_INVALID_NORM_VERSION"
        case .invalidIh58Prefix:
            return "ERR_INVALID_IH58_PREFIX"
        case .hashFailure:
            return "ERR_CANONICAL_HASH_FAILURE"
        case .invalidIh58Encoding:
            return "ERR_INVALID_IH58_ENCODING"
        case .invalidLength:
            return "ERR_INVALID_LENGTH"
        case .checksumMismatch:
            return "ERR_CHECKSUM_MISMATCH"
        case .invalidHexAddress:
            return "ERR_INVALID_HEX_ADDRESS"
        case .domainMismatch:
            return "ERR_DOMAIN_MISMATCH"
        case .invalidDomainLabel:
            return "ERR_INVALID_DOMAIN_LABEL"
        case .unexpectedNetworkPrefix:
            return "ERR_UNEXPECTED_NETWORK_PREFIX"
        case .unknownAddressClass:
            return "ERR_UNKNOWN_ADDRESS_CLASS"
        case .unknownDomainTag:
            return "ERR_UNKNOWN_DOMAIN_TAG"
        case .unexpectedExtensionFlag:
            return "ERR_UNEXPECTED_EXTENSION_FLAG"
        case .unknownControllerTag:
            return "ERR_UNKNOWN_CONTROLLER_TAG"
        case .invalidPublicKey:
            return "ERR_INVALID_PUBLIC_KEY"
        case .unknownCurve:
            return "ERR_UNKNOWN_CURVE"
        case .unexpectedTrailingBytes:
            return "ERR_UNEXPECTED_TRAILING_BYTES"
        case .invalidIh58PrefixEncoding:
            return "ERR_INVALID_IH58_PREFIX_ENCODING"
        case .missingCompressedSentinel:
            return "ERR_MISSING_COMPRESSED_SENTINEL"
        case .compressedTooShort:
            return "ERR_COMPRESSED_TOO_SHORT"
        case .invalidCompressedChar:
            return "ERR_INVALID_COMPRESSED_CHAR"
        case .invalidCompressedBase:
            return "ERR_INVALID_COMPRESSED_BASE"
        case .invalidCompressedDigit:
            return "ERR_INVALID_COMPRESSED_DIGIT"
        case .unsupportedAddressFormat:
            return "ERR_UNSUPPORTED_ADDRESS_FORMAT"
        case .multisigMemberOverflow:
            return "ERR_MULTISIG_MEMBER_OVERFLOW"
        case .invalidMultisigPolicy:
            return "ERR_INVALID_MULTISIG_POLICY"
        }
    }
}

public enum AccountAddressFormat: Equatable, Sendable {
    case ih58
    case compressed
    case canonicalHex
}

/// Structured representation of IH58 (preferred)/sora (second-best) outputs used by wallet/explorer UX.
public struct AccountAddressDisplayFormats: Equatable {
    public let ih58: String
    public let compressed: String
    public let networkPrefix: UInt16
    public let compressedWarning: String
}

public struct AccountAddress {
    private let header: AddressHeader
    private let domain: DomainSelector
    private let controller: ControllerPayload

    struct ControllerInfo {
        let algorithm: SigningAlgorithm
        let publicKey: Data
    }

    public struct MultisigPolicyInfo {
        public struct Member {
            public let algorithm: String
            public let weight: UInt16
            public let publicKeyHex: String
        }

        public let version: UInt8
        public let threshold: UInt16
        public let totalWeight: UInt32
        public let members: [Member]
        public let ctap2CborHex: String
        public let digestBlake2b256Hex: String
    }

    public static let defaultDomainName = "default"

    public static func fromAccount(domain: String, publicKey: Data, algorithm: String = "ed25519") throws -> AccountAddress {
        let header = try AddressHeader.new(version: 0, classId: .singleKey, normVersion: 1)
        let selector = try DomainSelector.from(domain: domain)
        let controller = try ControllerPayload.singleKey(publicKey: publicKey, algorithm: algorithm)
        return AccountAddress(header: header, domain: selector, controller: controller)
    }

    public static func fromCanonicalBytes(_ bytes: Data) throws -> AccountAddress {
        guard !bytes.isEmpty else { throw AccountAddressError.invalidLength }
        let header = try AddressHeader.decode(bytes[0])
        var canonicalError: AccountAddressError?
        do {
            let (controller, cursor) = try ControllerPayload.decode(bytes: bytes, cursor: 1)
            if cursor == bytes.count {
                return AccountAddress(header: header, domain: .default, controller: controller)
            }
            canonicalError = .unexpectedTrailingBytes
        } catch let error as AccountAddressError {
            canonicalError = error
        }

        // Backward compatibility: legacy payloads may include explicit domain selector bytes.
        do {
            var cursor = 1
            let (domain, domainCursor) = try DomainSelector.decode(bytes: bytes, cursor: cursor)
            cursor = domainCursor
            let (controller, controllerCursor) = try ControllerPayload.decode(bytes: bytes, cursor: cursor)
            cursor = controllerCursor
            if cursor != bytes.count {
                throw AccountAddressError.unexpectedTrailingBytes
            }
            return AccountAddress(header: header, domain: domain, controller: controller)
        } catch let error as AccountAddressError {
            throw canonicalError ?? error
        }
    }

    public static func fromCanonicalHex(_ encoded: String) throws -> AccountAddress {
        let body: String
        if encoded.lowercased().hasPrefix("0x") {
            body = String(encoded.dropFirst(2))
        } else {
            body = encoded
        }
        guard let data = Data(hexString: body) else {
            throw AccountAddressError.invalidHexAddress
        }
        return try AccountAddress.fromCanonicalBytes(data)
    }

    public static func fromIH58(_ encoded: String, expectedPrefix: UInt16? = nil) throws -> AccountAddress {
        let trimmed = encoded.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { throw AccountAddressError.invalidLength }
        if let bridged = try? NoritoNativeBridge.shared.parseAccountAddress(
            literal: trimmed,
            expectedPrefix: expectedPrefix
        ) {
            guard bridged.format == .ih58 else {
                throw AccountAddressError.unsupportedAddressFormat
            }
            return try AccountAddress.fromCanonicalBytes(bridged.canonicalBytes)
        }
        let (prefix, canonical) = try decodeIh58String(trimmed)
        if let expectedPrefix, prefix != expectedPrefix {
            throw AccountAddressError.unexpectedNetworkPrefix(expected: expectedPrefix, found: prefix)
        }
        return try AccountAddress.fromCanonicalBytes(canonical)
    }

    public static func fromCompressedSora(_ encoded: String) throws -> AccountAddress {
        let trimmed = encoded.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { throw AccountAddressError.invalidLength }
        guard trimmed.hasPrefix(compressedSentinel) else {
            throw AccountAddressError.missingCompressedSentinel
        }
        let canonical = try decodeCompressedString(trimmed)
        return try AccountAddress.fromCanonicalBytes(canonical)
    }

    public static func parseAny(_ input: String, expectedPrefix: UInt16? = nil) throws -> (AccountAddress, AccountAddressFormat) {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { throw AccountAddressError.invalidLength }
        let lowercased = trimmed.lowercased()
        if lowercased.hasPrefix(compressedSentinel) {
            return (try AccountAddress.fromCompressedSora(trimmed), .compressed)
        }
        if containsCompressedAlphabetBeyondIh58(trimmed) {
            throw AccountAddressError.missingCompressedSentinel
        }
        if let bridged = try? NoritoNativeBridge.shared.parseAccountAddress(
            literal: trimmed,
            expectedPrefix: expectedPrefix
        ) {
            let address = try AccountAddress.fromCanonicalBytes(bridged.canonicalBytes)
            return (address, bridged.format)
        }
        if lowercased.hasPrefix("0x") {
            let address = try fromCanonicalHex(trimmed)
            return (address, .canonicalHex)
        }
        let address = try AccountAddress.fromIH58(trimmed, expectedPrefix: expectedPrefix)
        return (address, .ih58)
    }

    static func canonicalizeDomainLabel(_ raw: String) throws -> String {
        try DomainSelector.canonicalizeLabel(raw)
    }

    func matchesDomainLabel(_ raw: String) -> Bool {
        guard let canonical = try? DomainSelector.canonicalizeLabel(raw) else {
            return false
        }
        switch domain {
        case .default:
            return !canonical.isEmpty
        case .local12(let digest):
            return computeLocalDigest(label: canonical) == digest
        case .global:
            return true
        }
    }

    /// This helper is retained for compatibility, but selector-free canonical payloads no longer rebase domain bytes.
    public func rebasedFromDefaultDomain(to domainLabel: String) throws -> AccountAddress {
        _ = try DomainSelector.canonicalizeLabel(domainLabel)
        return self
    }

    private static func containsCompressedAlphabetBeyondIh58(_ literal: String) -> Bool {
        for character in literal {
            let symbol = String(character)
            guard compressedIndex[symbol] != nil else { continue }
            if ih58Index[symbol] == nil {
                return true
            }
        }
        return false
    }

    public func canonicalBytes() throws -> Data {
        var bytes = Data()
        bytes.append(header.encode())
        try controller.encode(into: &bytes)
        return bytes
    }

    public func canonicalHex() throws -> String {
        let canonical = try canonicalBytes()
        return "0x" + canonical.map { String(format: "%02x", $0) }.joined()
    }

    public func toIH58(networkPrefix: UInt16) throws -> String {
        let canonical = try canonicalBytes()
        if let render = try? NoritoNativeBridge.shared.renderAccountAddress(
            canonicalBytes: canonical,
            networkPrefix: networkPrefix
        ) {
            return render.ih58
        }
        return try encodeIh58String(prefix: networkPrefix, canonical: canonical)
    }

    public func toCompressedSora() throws -> String {
        let canonical = try canonicalBytes()
        return try encodeCompressedString(canonical: canonical)
    }

    public func toCompressedSoraFullWidth() throws -> String {
        let canonical = try canonicalBytes()
        return try encodeCompressedString(canonical: canonical, fullWidth: true)
    }

    /// Returns both IH58 (preferred) and compressed (`sora`, second-best) representations plus the UX warning required by
    /// `docs/source/sns/address_display_guidelines.md`.
    public func displayFormats(networkPrefix: UInt16 = 753) throws -> AccountAddressDisplayFormats {
        let canonical = try canonicalBytes()
        if let render = try? NoritoNativeBridge.shared.renderAccountAddress(
            canonicalBytes: canonical,
            networkPrefix: networkPrefix
        ) {
            return AccountAddressDisplayFormats(
                ih58: render.ih58,
                compressed: render.compressed,
                networkPrefix: networkPrefix,
                compressedWarning: AccountAddress.compressedWarningMessage
            )
        }
        return AccountAddressDisplayFormats(
            ih58: try encodeIh58String(prefix: networkPrefix, canonical: canonical),
            compressed: try encodeCompressedString(canonical: canonical),
            networkPrefix: networkPrefix,
            compressedWarning: AccountAddress.compressedWarningMessage
        )
    }

    public func multisigPolicyInfo() throws -> MultisigPolicyInfo? {
        guard case let .multiSig(version, threshold, members) = controller else {
            return nil
        }
        let ctap2 = encodeMultisigPolicyCTAP2(version: version, threshold: threshold, members: members)
        let digest = blake2bMac256(ctap2, personal: AccountAddress.multisigPersonalisation)
        let infoMembers = members.map { member -> MultisigPolicyInfo.Member in
            MultisigPolicyInfo.Member(
                algorithm: member.curve.algorithmIdentifier,
                weight: member.weight,
                publicKeyHex: "0x\(member.publicKey.hexUppercased())"
            )
        }
        let totalWeight = members.reduce(UInt32(0)) { partial, member in
            partial &+ UInt32(member.weight)
        }
        return MultisigPolicyInfo(
            version: version,
            threshold: threshold,
            totalWeight: totalWeight,
            members: infoMembers,
            ctap2CborHex: "0x\(ctap2.hexUppercased())",
            digestBlake2b256Hex: "0x\(digest.hexUppercased())"
        )
    }

    func singleControllerInfo() -> ControllerInfo? {
        switch controller {
        case .singleKey(let curve, let publicKey):
            guard let algorithm = curve.signingAlgorithm else { return nil }
            return ControllerInfo(algorithm: algorithm, publicKey: publicKey)
        case .multiSig:
            return nil
        }
    }

    static let multisigPersonalisation = Data("iroha-ms-policy".utf8)
    private static let compressedWarningMessage =
        "Compressed Sora addresses rely on half-width kana and are only interoperable inside Sora-aware apps. " +
        "Prefer IH58 when sharing with explorers, wallets, or QR codes. See docs/source/sns/address_display_guidelines.md."
}

// MARK: - Internal components

private struct AddressHeader {
    let version: UInt8
    let classId: AddressClass
    let normVersion: UInt8
    let extFlag: Bool

    static func new(version: UInt8, classId: AddressClass, normVersion: UInt8) throws -> AddressHeader {
        guard version <= 0b111 else { throw AccountAddressError.invalidHeaderVersion(version) }
        guard normVersion <= 0b11 else { throw AccountAddressError.invalidNormVersion(normVersion) }
        return AddressHeader(version: version, classId: classId, normVersion: normVersion, extFlag: false)
    }

    func encode() -> UInt8 {
        var byte: UInt8 = (version & 0b111) << 5
        byte |= (classId.rawValue & 0b11) << 3
        byte |= (normVersion & 0b11) << 1
        byte |= extFlag ? 1 : 0
        return byte
    }

    static func decode(_ byte: UInt8) throws -> AddressHeader {
        let version = (byte >> 5) & 0b111
        let classBits = (byte >> 3) & 0b11
        let normVersion = (byte >> 1) & 0b11
        let extFlag = (byte & 1) == 1
        if extFlag {
            throw AccountAddressError.unexpectedExtensionFlag
        }
        guard let classId = AddressClass(rawValue: classBits) else {
            throw AccountAddressError.unknownAddressClass(classBits)
        }
        return try AddressHeader.new(version: version, classId: classId, normVersion: normVersion)
    }
}

private extension CurveId {
    var algorithmIdentifier: String {
        switch self {
        case .ed25519:
            return "ed25519"
        #if IROHASWIFT_ENABLE_SECP256K1
        case .secp256k1:
            return "secp256k1"
        #endif
        #if IROHASWIFT_ENABLE_MLDSA
        case .mldsa:
            return "mldsa"
        #endif
        #if IROHASWIFT_ENABLE_GOST
        case .gost256A:
            return "gost3410_2012_256_paramset_a"
        case .gost256B:
            return "gost3410_2012_256_paramset_b"
        case .gost256C:
            return "gost3410_2012_256_paramset_c"
        case .gost512A:
            return "gost3410_2012_512_paramset_a"
        case .gost512B:
            return "gost3410_2012_512_paramset_b"
        #endif
        #if IROHASWIFT_ENABLE_SM
        case .sm2:
            return "sm2"
        #endif
        }
    }

    var signingAlgorithm: SigningAlgorithm? {
        switch self {
        case .ed25519:
            return .ed25519
        #if IROHASWIFT_ENABLE_SECP256K1
        case .secp256k1:
            return .secp256k1
        #endif
        #if IROHASWIFT_ENABLE_MLDSA
        case .mldsa:
            return .mlDsa
        #endif
        #if IROHASWIFT_ENABLE_GOST
        case .gost256A, .gost256B, .gost256C, .gost512A, .gost512B:
            return nil
        #endif
        #if IROHASWIFT_ENABLE_SM
        case .sm2:
            return .sm2
        #endif
        }
    }
}

private enum AddressClass: UInt8 {
    case singleKey = 0
    case multiSig = 1
}

private enum DomainSelector {
    case `default`
    case local12(Data)
    case global(UInt32)

    fileprivate static func canonicalizeLabel(_ raw: String) throws -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw AccountAddressError.invalidDomainLabel(raw)
        }
        if trimmed != raw || trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw AccountAddressError.invalidDomainLabel(raw)
        }
        if trimmed.contains("@") || trimmed.contains("#") || trimmed.contains("$") {
            throw AccountAddressError.invalidDomainLabel(raw)
        }
        let lowered = trimmed.lowercased()
        for scalar in lowered.unicodeScalars {
            guard scalar.isASCII else {
                throw AccountAddressError.invalidDomainLabel(raw)
            }
            let value = scalar.value
            let isAlphaNum = (value >= 48 && value <= 57) || (value >= 97 && value <= 122)
            if !isAlphaNum && value != 45 && value != 95 && value != 46 {
                throw AccountAddressError.invalidDomainLabel(raw)
            }
        }
        return lowered
    }

    static func from(domain: String) throws -> DomainSelector {
        _ = try canonicalizeLabel(domain)
        // Canonical payloads are globally scoped and no longer encode domain selectors.
        return .default
    }

    func encode(into buffer: inout Data) {
        switch self {
        case .default:
            buffer.append(0x00)
        case .local12(let digest):
            buffer.append(0x01)
            buffer.append(digest)
        case .global(let id):
            buffer.append(0x02)
            var be = id.bigEndian
            withUnsafeBytes(of: &be) { buffer.append(contentsOf: $0) }
        }
    }

    static func decode(bytes: Data, cursor: Int) throws -> (DomainSelector, Int) {
        guard cursor < bytes.count else { throw AccountAddressError.invalidLength }
        let tag = bytes[cursor]
        let cursor = cursor + 1
        switch tag {
        case 0x00:
            return (.default, cursor)
        case 0x01:
            let end = cursor + 12
            guard end <= bytes.count else { throw AccountAddressError.invalidLength }
            let digest = bytes[cursor..<end]
            return (.local12(Data(digest)), end)
        case 0x02:
            let end = cursor + 4
            guard end <= bytes.count else { throw AccountAddressError.invalidLength }
            let raw = bytes[cursor..<end]
            let value = raw.reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
            return (.global(value), end)
        default:
            throw AccountAddressError.unknownDomainTag(tag)
        }
    }
}

private enum ControllerPayload {
    case singleKey(curve: CurveId, publicKey: Data)
    case multiSig(version: UInt8, threshold: UInt16, members: [MultisigMember])

    struct MultisigMember {
        let curve: CurveId
        let weight: UInt16
        let publicKey: Data
    }

    static func singleKey(publicKey: Data, algorithm: String) throws -> ControllerPayload {
        let curve = try CurveId.from(algorithm: algorithm)
        guard !publicKey.isEmpty else {
            throw AccountAddressError.invalidPublicKey
        }
        if let expected = curve.expectedPublicKeyLength, publicKey.count != expected {
            throw AccountAddressError.invalidPublicKey
        }
        guard publicKey.count <= 0xFF else {
            throw AccountAddressError.keyPayloadTooLong(publicKey.count)
        }
        return .singleKey(curve: curve, publicKey: publicKey)
    }

    func encode(into buffer: inout Data) throws {
        switch self {
        case .singleKey(let curve, let key):
            buffer.append(ControllerPayloadTag.singleKey.rawValue)
            buffer.append(curve.rawValue)
            guard key.count <= 0xFF else {
                throw AccountAddressError.keyPayloadTooLong(key.count)
            }
            buffer.append(UInt8(key.count))
            buffer.append(key)
        case .multiSig(let version, let threshold, let members):
            guard members.count <= multisigMemberMax else {
                throw AccountAddressError.multisigMemberOverflow(members.count)
            }
            buffer.append(ControllerPayloadTag.multiSig.rawValue)
            buffer.append(version)
            var thresholdBE = threshold.bigEndian
            withUnsafeBytes(of: &thresholdBE) { buffer.append(contentsOf: $0) }
            buffer.append(UInt8(members.count))
            for member in members {
                buffer.append(member.curve.rawValue)
                var weightBE = member.weight.bigEndian
                withUnsafeBytes(of: &weightBE) { buffer.append(contentsOf: $0) }
                guard member.publicKey.count <= 0xFFFF else {
                    throw AccountAddressError.keyPayloadTooLong(member.publicKey.count)
                }
                var lengthBE = UInt16(member.publicKey.count).bigEndian
                withUnsafeBytes(of: &lengthBE) { buffer.append(contentsOf: $0) }
                buffer.append(member.publicKey)
            }
        }
    }

    static func decode(bytes: Data, cursor: Int) throws -> (ControllerPayload, Int) {
        guard cursor < bytes.count else { throw AccountAddressError.invalidLength }
        let tagValue = bytes[cursor]
        guard let tag = ControllerPayloadTag(rawValue: tagValue) else {
            throw AccountAddressError.unknownControllerTag(tagValue)
        }
        var cursor = cursor + 1
        switch tag {
        case .singleKey:
            guard cursor < bytes.count else { throw AccountAddressError.invalidLength }
            let curveRaw = bytes[cursor]
            cursor += 1
            let curve = try CurveId.decode(rawValue: curveRaw)
            guard cursor < bytes.count else { throw AccountAddressError.invalidLength }
            let length = Int(bytes[cursor])
            cursor += 1
            let end = cursor + length
            guard end <= bytes.count else { throw AccountAddressError.invalidLength }
            let key = bytes[cursor..<end]
            return (.singleKey(curve: curve, publicKey: Data(key)), end)
        case .multiSig:
            guard cursor < bytes.count else { throw AccountAddressError.invalidLength }
            let version = bytes[cursor]
            cursor += 1
            guard cursor + 1 < bytes.count else { throw AccountAddressError.invalidLength }
            let threshold = (UInt16(bytes[cursor]) << 8) | UInt16(bytes[cursor + 1])
            cursor += 2
            guard cursor < bytes.count else { throw AccountAddressError.invalidLength }
            let memberCount = Int(bytes[cursor])
            cursor += 1
            guard memberCount <= multisigMemberMax else {
                throw AccountAddressError.multisigMemberOverflow(memberCount)
            }
            var members: [MultisigMember] = []
            members.reserveCapacity(memberCount)
            for _ in 0..<memberCount {
                guard cursor < bytes.count else { throw AccountAddressError.invalidLength }
                let curveRaw = bytes[cursor]
                cursor += 1
                let curve = try CurveId.decode(rawValue: curveRaw)
                guard cursor + 1 < bytes.count else { throw AccountAddressError.invalidLength }
                let weight = (UInt16(bytes[cursor]) << 8) | UInt16(bytes[cursor + 1])
                cursor += 2
                guard cursor + 1 < bytes.count else { throw AccountAddressError.invalidLength }
                let keyLength = Int((UInt16(bytes[cursor]) << 8) | UInt16(bytes[cursor + 1]))
                cursor += 2
                let end = cursor + keyLength
                guard end <= bytes.count else { throw AccountAddressError.invalidLength }
                let key = Data(bytes[cursor..<end])
                cursor = end
                members.append(MultisigMember(curve: curve, weight: weight, publicKey: key))
            }
            guard threshold > 0 else {
                throw AccountAddressError.invalidMultisigPolicy("ZeroThreshold")
            }
            guard !members.isEmpty else {
                throw AccountAddressError.invalidMultisigPolicy("EmptyMembers")
            }
            return (.multiSig(version: version, threshold: threshold, members: members), cursor)
        }
    }

    private enum ControllerPayloadTag: UInt8 {
        case singleKey = 0x00
        case multiSig = 0x01
    }
}

private func encodeMultisigPolicyCTAP2(
    version: UInt8,
    threshold: UInt16,
    members: [ControllerPayload.MultisigMember]
) -> Data {
    var buffer = Data()
    cborAppendLength(into: &buffer, major: 0b101, length: 3)
    cborAppendUnsigned(into: &buffer, value: 0x01)
    cborAppendUnsigned(into: &buffer, value: UInt64(version))
    cborAppendUnsigned(into: &buffer, value: 0x02)
    cborAppendUnsigned(into: &buffer, value: UInt64(threshold))
    cborAppendUnsigned(into: &buffer, value: 0x03)
    cborAppendLength(into: &buffer, major: 0b100, length: members.count)
    for member in members {
        cborAppendLength(into: &buffer, major: 0b101, length: 3)
        cborAppendUnsigned(into: &buffer, value: 0x01)
        cborAppendUnsigned(into: &buffer, value: UInt64(member.curve.rawValue))
        cborAppendUnsigned(into: &buffer, value: 0x02)
        cborAppendUnsigned(into: &buffer, value: UInt64(member.weight))
        cborAppendUnsigned(into: &buffer, value: 0x03)
        cborAppendBytes(into: &buffer, bytes: member.publicKey)
    }
    return buffer
}

private func blake2bMac256(_ data: Data, personal: Data) -> Data {
    // Blake2bMac with an empty key still emits a zero-padded key block.
    var prefixed = Data(repeating: 0, count: blake2bBlockLength)
    prefixed.append(data)
    return Blake2b.hash256(prefixed, personal: personal)
}

private func cborAppendUnsigned(into buffer: inout Data, value: UInt64) {
    switch value {
    case 0...23:
        buffer.append(UInt8(value))
    case 24...0xFF:
        buffer.append(0x18)
        buffer.append(UInt8(value))
    case 0x100...0xFFFF:
        buffer.append(0x19)
        var be = UInt16(value).bigEndian
        withUnsafeBytes(of: &be) { buffer.append(contentsOf: $0) }
    case 0x1_0000...0xFFFF_FFFF:
        buffer.append(0x1A)
        var be = UInt32(value).bigEndian
        withUnsafeBytes(of: &be) { buffer.append(contentsOf: $0) }
    default:
        buffer.append(0x1B)
        var be = value.bigEndian
        withUnsafeBytes(of: &be) { buffer.append(contentsOf: $0) }
    }
}

private func cborAppendLength(into buffer: inout Data, major: UInt8, length: Int) {
    precondition(major <= 0b111)
    let base = major << 5
    let value = UInt64(length)
    switch value {
    case 0...23:
        buffer.append(base | UInt8(value))
    case 24...0xFF:
        buffer.append(base | 24)
        buffer.append(UInt8(value))
    case 0x100...0xFFFF:
        buffer.append(base | 25)
        var be = UInt16(value).bigEndian
        withUnsafeBytes(of: &be) { buffer.append(contentsOf: $0) }
    case 0x1_0000...0xFFFF_FFFF:
        buffer.append(base | 26)
        var be = UInt32(value).bigEndian
        withUnsafeBytes(of: &be) { buffer.append(contentsOf: $0) }
    default:
        buffer.append(base | 27)
        var be = value.bigEndian
        withUnsafeBytes(of: &be) { buffer.append(contentsOf: $0) }
    }
}

private func cborAppendBytes(into buffer: inout Data, bytes: Data) {
    cborAppendLength(into: &buffer, major: 0b010, length: bytes.count)
    buffer.append(bytes)
}

/// Supported curve identifiers mirroring the registry in `docs/account_structure.md`.
/// Additional cases compile only when the corresponding feature flag is enabled so preview
/// curves fail closed by default.
private enum CurveId: UInt8 {
    case ed25519 = 1
    #if IROHASWIFT_ENABLE_SECP256K1
    case secp256k1 = 4
    #endif
    #if IROHASWIFT_ENABLE_MLDSA
    case mldsa = 2
    #endif
    #if IROHASWIFT_ENABLE_GOST
    case gost256A = 10
    case gost256B = 11
    case gost256C = 12
    case gost512A = 13
    case gost512B = 14
    #endif
    #if IROHASWIFT_ENABLE_SM
    case sm2 = 15
    #endif

    static func decode(rawValue: UInt8) throws -> CurveId {
        switch rawValue {
        case CurveId.ed25519.rawValue:
            return .ed25519
        #if IROHASWIFT_ENABLE_SECP256K1
        case CurveId.secp256k1.rawValue:
            return .secp256k1
        #endif
        #if IROHASWIFT_ENABLE_MLDSA
        case CurveId.mldsa.rawValue:
            return .mldsa
        #endif
        #if IROHASWIFT_ENABLE_GOST
        case CurveId.gost256A.rawValue:
            return .gost256A
        case CurveId.gost256B.rawValue:
            return .gost256B
        case CurveId.gost256C.rawValue:
            return .gost256C
        case CurveId.gost512A.rawValue:
            return .gost512A
        case CurveId.gost512B.rawValue:
            return .gost512B
        #endif
        #if IROHASWIFT_ENABLE_SM
        case CurveId.sm2.rawValue:
            return .sm2
        #endif
        default:
            throw AccountAddressError.unknownCurve(rawValue)
        }
    }

    static func from(algorithm: String) throws -> CurveId {
        let normalized = algorithm.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        switch normalized {
        case "ed25519", "ed":
            return .ed25519
        #if IROHASWIFT_ENABLE_SECP256K1
        case "secp256k1", "secp":
            return .secp256k1
        #endif
        #if IROHASWIFT_ENABLE_MLDSA
        case "ml-dsa", "mldsa", "ml_dsa":
            return .mldsa
        #endif
        #if IROHASWIFT_ENABLE_GOST
        case "gost256a", "gost-256-a":
            return .gost256A
        case "gost256b", "gost-256-b":
            return .gost256B
        case "gost256c", "gost-256-c":
            return .gost256C
        case "gost512a", "gost-512-a":
            return .gost512A
        case "gost512b", "gost-512-b":
            return .gost512B
        #endif
        default:
            throw AccountAddressError.unsupportedAlgorithm(algorithm)
        }
    }

    var expectedPublicKeyLength: Int? {
        switch self {
        case .ed25519:
            return 32
        #if IROHASWIFT_ENABLE_SECP256K1
        case .secp256k1:
            return 33
        #endif
        default:
            return nil
        }
    }
}

// MARK: - Encoding helpers

private let ih58Alphabet: [String] = [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F", "G", "H", "J", "K",
    "L", "M", "N", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "a", "b", "c", "d", "e",
    "f", "g", "h", "i", "j", "k", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y",
    "z",
]

private let soraKana: [String] = [
    "ｲ", "ﾛ", "ﾊ", "ﾆ", "ﾎ", "ﾍ", "ﾄ", "ﾁ", "ﾘ", "ﾇ", "ﾙ", "ｦ", "ﾜ", "ｶ", "ﾖ", "ﾀ", "ﾚ", "ｿ", "ﾂ",
    "ﾈ", "ﾅ", "ﾗ", "ﾑ", "ｳ", "ヰ", "ﾉ", "ｵ", "ｸ", "ﾔ", "ﾏ", "ｹ", "ﾌ", "ｺ", "ｴ", "ﾃ", "ｱ", "ｻ", "ｷ",
    "ﾕ", "ﾒ", "ﾐ", "ｼ", "ヱ", "ﾋ", "ﾓ", "ｾ", "ｽ",
]

private let soraKanaFullwidth: [String] = [
    "イ", "ロ", "ハ", "ニ", "ホ", "ヘ", "ト", "チ", "リ", "ヌ", "ル", "ヲ", "ワ", "カ", "ヨ", "タ",
    "レ", "ソ", "ツ", "ネ", "ナ", "ラ", "ム", "ウ", "ヰ", "ノ", "オ", "ク", "ヤ", "マ", "ケ", "フ",
    "コ", "エ", "テ", "ア", "サ", "キ", "ユ", "メ", "ミ", "シ", "ヱ", "ヒ", "モ", "セ", "ス",
]

private let compressedAlphabet: [String] = ih58Alphabet + soraKana
private let compressedAlphabetFullwidth: [String] = ih58Alphabet + soraKanaFullwidth
private let compressedIndex: [String: Int] = {
    var mapping = Dictionary(uniqueKeysWithValues: compressedAlphabet.enumerated().map { ($1, $0) })
    for (index, symbol) in compressedAlphabetFullwidth.enumerated() {
        mapping[symbol] = index
    }
    return mapping
}()
private let ih58Index: [String: Int] = Dictionary(uniqueKeysWithValues: ih58Alphabet.enumerated().map { ($1, $0) })

private let compressedSentinel = "sora"
private let compressedChecksumLength = 6
private let compressedBase = compressedAlphabet.count
private let localDomainKey = Data("SORA-LOCAL-K:v1".utf8)
private let ih58ChecksumPrefix = Data("IH58PRE".utf8)
private let multisigMemberMax = 0xFF
private let blake2bBlockLength = 128

private func encodeIh58Prefix(_ prefix: UInt16) throws -> [UInt8] {
    if prefix <= 63 {
        return [UInt8(prefix)]
    }
    if prefix <= 16_383 {
        let lower = UInt8((prefix & 0b0011_1111) | 0b0100_0000)
        let upper = UInt8(prefix >> 6)
        return [lower, upper]
    }
    throw AccountAddressError.invalidIh58Prefix(prefix)
}

private func decodeIh58Prefix(_ bytes: [UInt8]) throws -> (UInt16, Int) {
    guard let first = bytes.first else { throw AccountAddressError.invalidLength }
    if first <= 63 {
        return (UInt16(first), 1)
    }
    if (first & 0b0100_0000) != 0 {
        guard bytes.count >= 2 else { throw AccountAddressError.invalidIh58PrefixEncoding(first) }
        let value = (UInt16(bytes[1]) << 6) | UInt16(first & 0b0011_1111)
        return (value, 2)
    }
    throw AccountAddressError.invalidIh58PrefixEncoding(first)
}

private func encodeIh58String(prefix: UInt16, canonical: Data) throws -> String {
    let prefixBytes = try encodeIh58Prefix(prefix)
    var body = prefixBytes
    body.append(contentsOf: canonical)
    var checksumInput = Data(ih58ChecksumPrefix)
    checksumInput.append(contentsOf: body)
    let checksum = Blake2b.hash512(checksumInput)
    body.append(contentsOf: checksum.prefix(2))
    let digits = try encodeBaseN(bytes: body, base: 58)
    return digits.map { ih58Alphabet[$0] }.joined()
}

private func decodeIh58String(_ encoded: String) throws -> (UInt16, Data) {
    var digits: [Int] = []
    for character in encoded {
        let symbol = String(character)
        guard let index = ih58Index[symbol] else {
            throw AccountAddressError.invalidIh58Encoding
        }
        digits.append(index)
    }
    let bodyBytes = try decodeBaseN(digits: digits, base: 58)
    guard bodyBytes.count >= 3 else { throw AccountAddressError.invalidLength }
    let payload = Array(bodyBytes.dropLast(2))
    let checksumBytes = Array(bodyBytes.suffix(2))
    let (prefix, prefixLen) = try decodeIh58Prefix(payload)
    var checksumInput = Data(ih58ChecksumPrefix)
    checksumInput.append(contentsOf: payload)
    let expected = Array(Blake2b.hash512(checksumInput).prefix(2))
    guard checksumBytes.elementsEqual(expected) else {
        throw AccountAddressError.checksumMismatch
    }
    let canonical = Data(payload.dropFirst(prefixLen))
    return (prefix, canonical)
}

private func encodeCompressedString(canonical: Data, fullWidth: Bool = false) throws -> String {
    let digits = try encodeBaseN(bytes: Array(canonical), base: compressedBase)
    let checksum = compressedChecksumDigits(canonical: canonical)
    let alphabet = fullWidth ? compressedAlphabetFullwidth : compressedAlphabet
    var parts = [compressedSentinel]
    parts.append(contentsOf: digits.map { alphabet[$0] })
    parts.append(contentsOf: checksum.map { alphabet[$0] })
    return parts.joined()
}

private func decodeCompressedString(_ encoded: String) throws -> Data {
    guard encoded.hasPrefix(compressedSentinel) else {
        throw AccountAddressError.missingCompressedSentinel
    }
    let payload = encoded.dropFirst(compressedSentinel.count)
    guard payload.count > compressedChecksumLength else {
        throw AccountAddressError.compressedTooShort
    }
    var digits: [Int] = []
    for symbol in payload {
        let key = String(symbol)
        guard let value = compressedIndex[key] else {
            throw AccountAddressError.invalidCompressedChar(symbol)
        }
        digits.append(value)
    }
    let dataDigits = Array(digits.dropLast(compressedChecksumLength))
    let checksumDigits = Array(digits.suffix(compressedChecksumLength))
    let canonicalBytes = try decodeBaseN(digits: dataDigits, base: compressedBase)
    let expected = compressedChecksumDigits(canonical: Data(canonicalBytes))
    guard checksumDigits.elementsEqual(expected) else {
        throw AccountAddressError.checksumMismatch
    }
    return Data(canonicalBytes)
}

private func encodeBaseN(bytes: [UInt8], base: Int) throws -> [Int] {
    guard base >= 2 else { throw AccountAddressError.invalidCompressedBase }
    if bytes.isEmpty { return [0] }
    var value = bytes
    var leadingZeros = 0
    while leadingZeros < value.count && value[leadingZeros] == 0 {
        leadingZeros += 1
    }
    var digits: [Int] = []
    var start = leadingZeros
    while start < value.count {
        var remainder = 0
        for idx in start..<value.count {
            let acc = (remainder << 8) | Int(value[idx])
            value[idx] = UInt8(acc / base)
            remainder = acc % base
        }
        digits.append(remainder)
        while start < value.count && value[start] == 0 {
            start += 1
        }
    }
    digits.append(contentsOf: Array(repeating: 0, count: leadingZeros))
    if digits.isEmpty { digits.append(0) }
    return digits.reversed()
}

private func decodeBaseN(digits: [Int], base: Int) throws -> [UInt8] {
    guard base >= 2 else { throw AccountAddressError.invalidCompressedBase }
    guard !digits.isEmpty else { throw AccountAddressError.invalidLength }
    for digit in digits where digit < 0 || digit >= base {
        throw AccountAddressError.invalidCompressedDigit(digit)
    }
    var value = digits
    var leadingZeros = 0
    while leadingZeros < value.count && value[leadingZeros] == 0 {
        leadingZeros += 1
    }
    var bytes: [UInt8] = []
    var start = leadingZeros
    while start < value.count {
        var remainder = 0
        for idx in start..<value.count {
            let acc = remainder * base + value[idx]
            value[idx] = acc / 256
            remainder = acc % 256
        }
        bytes.append(UInt8(remainder))
        while start < value.count && value[start] == 0 {
            start += 1
        }
    }
    bytes.append(contentsOf: Array(repeating: 0, count: leadingZeros))
    if bytes.isEmpty { bytes.append(0) }
    return bytes.reversed()
}

private func convertToBase32(data: Data) -> [Int] {
    var acc = 0
    var bits = 0
    var out: [Int] = []
    for byte in data {
        acc = (acc << 8) | Int(byte)
        bits += 8
        while bits >= 5 {
            bits -= 5
            out.append((acc >> bits) & 0x1F)
        }
    }
    if bits > 0 {
        out.append((acc << (5 - bits)) & 0x1F)
    }
    return out
}

private func bech32Polymod(values: [Int]) -> UInt32 {
    let generators: [UInt32] = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
    var chk: UInt32 = 1
    for value in values {
        let top = chk >> 25
        chk = ((chk & 0x1ff_ffff) << 5) ^ UInt32(value)
        for (index, generator) in generators.enumerated() where ((top >> index) & 1) == 1 {
            chk ^= generator
        }
    }
    return chk
}

private func expandHrp(_ hrp: String) -> [Int] {
    var out: [Int] = []
    for character in hrp {
        let code = Int(character.unicodeScalars.first!.value)
        out.append(code >> 5)
    }
    out.append(0)
    for character in hrp {
        let code = Int(character.unicodeScalars.first!.value)
        out.append(code & 0x1F)
    }
    return out
}

private func bech32mChecksum(data: Data) -> [Int] {
    var values = expandHrp("snx")
    values.append(contentsOf: convertToBase32(data: data))
    values.append(contentsOf: Array(repeating: 0, count: compressedChecksumLength))
    let polymod = bech32Polymod(values: values) ^ 0x2bc830a3
    var checksum: [Int] = []
    for i in 0..<compressedChecksumLength {
        let shift = 5 * (compressedChecksumLength - 1 - i)
        checksum.append(Int((polymod >> shift) & 0x1F))
    }
    return checksum
}

private func compressedChecksumDigits(canonical: Data) -> [Int] {
    bech32mChecksum(data: canonical)
}

private func computeLocalDigest(label: String) -> Data {
    let digest = Blake2s.hash(data: Data(label.utf8), key: localDomainKey, outputLength: 32)
    return Data(digest.prefix(12))
}

extension AccountAddressFormat {
    init?(bridgeCode: UInt8) {
        switch bridgeCode {
        case 0:
            self = .ih58
        case 1:
            self = .compressed
        case 2:
            self = .canonicalHex
        default:
            return nil
        }
    }
}

extension AccountAddressError {
    struct BridgePayload {
        let code: String
        let message: String
        let fields: [String: Any]
    }

    static func bridgePayload(from data: Data) -> BridgePayload? {
        guard
            let raw = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let code = raw["code"] as? String
        else {
            return nil
        }
        let message = raw["message"] as? String ?? code
        let fields = raw["fields"] as? [String: Any] ?? [:]
        return BridgePayload(code: code, message: message, fields: fields)
    }

    static func fromBridgePayload(_ payload: BridgePayload) -> AccountAddressError? {
        let fields = payload.fields
        switch payload.code {
        case "ERR_UNSUPPORTED_ALGORITHM":
            if let algorithm = fields["algorithm"] as? String {
                return AccountAddressError.unsupportedAlgorithm(algorithm)
            }
        case "ERR_KEY_PAYLOAD_TOO_LONG":
            if let length = intField("length", fields: fields) {
                return AccountAddressError.keyPayloadTooLong(length)
            }
        case "ERR_INVALID_HEADER_VERSION":
            if let value = uInt8Field("value", fields: fields) {
                return AccountAddressError.invalidHeaderVersion(value)
            }
        case "ERR_INVALID_NORM_VERSION":
            if let value = uInt8Field("value", fields: fields) {
                return AccountAddressError.invalidNormVersion(value)
            }
        case "ERR_INVALID_IH58_PREFIX":
            if let prefix = uInt16Field("prefix", fields: fields) {
                return AccountAddressError.invalidIh58Prefix(prefix)
            }
        case "ERR_CANONICAL_HASH_FAILURE":
            return AccountAddressError.hashFailure
        case "ERR_INVALID_IH58_ENCODING":
            return AccountAddressError.invalidIh58Encoding
        case "ERR_INVALID_LENGTH":
            return AccountAddressError.invalidLength
        case "ERR_CHECKSUM_MISMATCH":
            return AccountAddressError.checksumMismatch
        case "ERR_INVALID_HEX_ADDRESS":
            return AccountAddressError.invalidHexAddress
        case "ERR_DOMAIN_MISMATCH":
            return AccountAddressError.domainMismatch
        case "ERR_INVALID_DOMAIN_LABEL":
            if let label = fields["label"] as? String {
                return AccountAddressError.invalidDomainLabel(label)
            }
        case "ERR_UNEXPECTED_NETWORK_PREFIX":
            if let expected = uInt16Field("expected", fields: fields),
               let found = uInt16Field("found", fields: fields) {
                return AccountAddressError.unexpectedNetworkPrefix(expected: expected, found: found)
            }
        case "ERR_UNKNOWN_ADDRESS_CLASS":
            if let value = uInt8Field("value", fields: fields) {
                return AccountAddressError.unknownAddressClass(value)
            }
        case "ERR_UNKNOWN_DOMAIN_TAG":
            if let value = uInt8Field("value", fields: fields) {
                return AccountAddressError.unknownDomainTag(value)
            }
        case "ERR_UNEXPECTED_EXTENSION_FLAG":
            return AccountAddressError.unexpectedExtensionFlag
        case "ERR_UNKNOWN_CONTROLLER_TAG":
            if let value = uInt8Field("value", fields: fields) {
                return AccountAddressError.unknownControllerTag(value)
            }
        case "ERR_INVALID_PUBLIC_KEY":
            return AccountAddressError.invalidPublicKey
        case "ERR_UNKNOWN_CURVE":
            if let value = uInt8Field("value", fields: fields) {
                return AccountAddressError.unknownCurve(value)
            }
        case "ERR_UNEXPECTED_TRAILING_BYTES":
            return AccountAddressError.unexpectedTrailingBytes
        case "ERR_INVALID_IH58_PREFIX_ENCODING":
            if let value = uInt8Field("value", fields: fields) {
                return AccountAddressError.invalidIh58PrefixEncoding(value)
            }
        case "ERR_MISSING_COMPRESSED_SENTINEL":
            return AccountAddressError.missingCompressedSentinel
        case "ERR_COMPRESSED_TOO_SHORT":
            return AccountAddressError.compressedTooShort
        case "ERR_INVALID_COMPRESSED_CHAR":
            if let value = fields["char"] as? String, let character = value.first {
                return AccountAddressError.invalidCompressedChar(character)
            }
        case "ERR_INVALID_COMPRESSED_BASE":
            return AccountAddressError.invalidCompressedBase
        case "ERR_INVALID_COMPRESSED_DIGIT":
            if let digit = intField("digit", fields: fields) {
                return AccountAddressError.invalidCompressedDigit(digit)
            }
        case "ERR_LOCAL8_DEPRECATED":
            return AccountAddressError.unsupportedAddressFormat
        case "ERR_UNSUPPORTED_ADDRESS_FORMAT":
            return AccountAddressError.unsupportedAddressFormat
        case "ERR_MULTISIG_MEMBER_OVERFLOW":
            if let count = intField("count", fields: fields) {
                return AccountAddressError.multisigMemberOverflow(count)
            }
        case "ERR_INVALID_MULTISIG_POLICY":
            if let detail = fields["policy_error"] as? String {
                return AccountAddressError.invalidMultisigPolicy(detail)
            }
        default:
            return nil
        }
        return nil
    }

    private static func intField(_ name: String, fields: [String: Any]) -> Int? {
        StrictJSONNumber.int(from: fields[name])
    }

    private static func uInt8Field(_ name: String, fields: [String: Any]) -> UInt8? {
        guard let value = intField(name, fields: fields) else { return nil }
        guard value >= 0, value <= Int(UInt8.max) else { return nil }
        return UInt8(value)
    }

    private static func uInt16Field(_ name: String, fields: [String: Any]) -> UInt16? {
        guard let value = intField(name, fields: fields) else { return nil }
        guard value >= 0, value <= Int(UInt16.max) else { return nil }
        return UInt16(value)
    }
}

extension AccountAddress {
    func noritoAccountControllerPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        switch controller {
        case .singleKey(let curve, let publicKey):
            let keyPayload = try noritoPublicKeyPayload(curve: curve, publicKey: publicKey)
            writer.writeUInt32LE(0)
            writer.writeLength(UInt64(keyPayload.count))
            writer.writeBytes(keyPayload)
        case .multiSig(let version, let threshold, let members):
            let policyPayload = try noritoMultisigPolicyPayload(version: version,
                                                                threshold: threshold,
                                                                members: members)
            writer.writeUInt32LE(1)
            writer.writeLength(UInt64(policyPayload.count))
            writer.writeBytes(policyPayload)
        }
        return writer.data
    }

    private func noritoPublicKeyPayload(curve: CurveId, publicKey: Data) throws -> Data {
        let algorithm = try noritoSigningAlgorithm(for: curve)
        let multihash = OfflineNorito.publicKeyMultihash(algorithm: algorithm, payload: publicKey)
        return OfflineNorito.encodeString(multihash)
    }

    private func noritoMultisigPolicyPayload(
        version: UInt8,
        threshold: UInt16,
        members: [ControllerPayload.MultisigMember]
    ) throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeUInt8(version))
        writer.writeField(OfflineNorito.encodeUInt16(threshold))
        let membersPayload = try OfflineNorito.encodeVec(members) { member in
            var memberWriter = OfflineNoritoWriter()
            let keyPayload = try noritoPublicKeyPayload(curve: member.curve, publicKey: member.publicKey)
            memberWriter.writeField(keyPayload)
            memberWriter.writeField(OfflineNorito.encodeUInt16(member.weight))
            return memberWriter.data
        }
        writer.writeField(membersPayload)
        return writer.data
    }

    private func noritoSigningAlgorithm(for curve: CurveId) throws -> SigningAlgorithm {
        switch curve {
        case .ed25519:
            return .ed25519
        #if IROHASWIFT_ENABLE_SECP256K1
        case .secp256k1:
            return .secp256k1
        #endif
        #if IROHASWIFT_ENABLE_MLDSA
        case .mldsa:
            return .mlDsa
        #endif
        #if IROHASWIFT_ENABLE_GOST
        case .gost256A, .gost256B, .gost256C, .gost512A, .gost512B:
            throw OfflineNoritoError.invalidAccountId("unsupported GOST account controller")
        #endif
        #if IROHASWIFT_ENABLE_SM
        case .sm2:
            return .sm2
        #endif
        }
    }
}

// MARK: - Multisig builder (IOS4 scaffolding)

public enum MultisigBuilderError: Error, LocalizedError {
    case thresholdNotSet
    case noMembers
    case memberOverflow(Int)
    case unsupportedAlgorithm(SigningAlgorithm)

    public var errorDescription: String? {
        switch self {
        case .thresholdNotSet:
            return "Multisig threshold must be configured before building a policy."
        case .noMembers:
            return "Multisig policies require at least one member."
        case let .memberOverflow(count):
            return "Multisig member count \(count) exceeds the supported maximum."
        case let .unsupportedAlgorithm(algorithm):
            return "Algorithm \(algorithm) is not available in this build."
        }
    }
}

public struct MultisigMemberDescriptor: Sendable {
    public let algorithm: SigningAlgorithm
    public let weight: UInt16
    public let publicKey: Data

    public init(algorithm: SigningAlgorithm, weight: UInt16, publicKey: Data) {
        self.algorithm = algorithm
        self.weight = weight
        self.publicKey = publicKey
    }
}

public struct MultisigPolicy: Sendable {
    public let version: UInt8
    public let threshold: UInt16
    public let members: [MultisigMemberDescriptor]
    public let ctap2Cbor: Data
    public let digestBlake2b256: Data

    public var ctap2CborHex: String { "0x\(ctap2Cbor.hexUppercased())" }
    public var digestHex: String { "0x\(digestBlake2b256.hexUppercased())" }
}

public final class MultisigPolicyBuilder {
    private var version: UInt8 = 1
    private var threshold: UInt16?
    private var members: [MultisigMemberDescriptor] = []

    public init() {}

    @discardableResult
    public func setVersion(_ version: UInt8) -> MultisigPolicyBuilder {
        self.version = version
        return self
    }

    @discardableResult
    public func setThreshold(_ threshold: UInt16) -> MultisigPolicyBuilder {
        self.threshold = threshold
        return self
    }

    @discardableResult
    public func addMember(_ descriptor: MultisigMemberDescriptor) -> MultisigPolicyBuilder {
        members.append(descriptor)
        return self
    }

    @discardableResult
    public func addMember(algorithm: SigningAlgorithm,
                          weight: UInt16,
                          publicKey: Data) -> MultisigPolicyBuilder {
        addMember(MultisigMemberDescriptor(algorithm: algorithm,
                                           weight: weight,
                                           publicKey: publicKey))
    }

    public func build() throws -> MultisigPolicy {
        guard let resolvedThreshold = threshold else {
            throw MultisigBuilderError.thresholdNotSet
        }
        guard !members.isEmpty else {
            throw MultisigBuilderError.noMembers
        }
        guard members.count <= multisigMemberMax else {
            throw MultisigBuilderError.memberOverflow(members.count)
        }

        let payloadMembers = try members.map { descriptor -> ControllerPayload.MultisigMember in
            let curve = try curveId(for: descriptor.algorithm)
            return ControllerPayload.MultisigMember(curve: curve,
                                                    weight: descriptor.weight,
                                                    publicKey: descriptor.publicKey)
        }
        let cbor = encodeMultisigPolicyCTAP2(version: version,
                                             threshold: resolvedThreshold,
                                             members: payloadMembers)
        let digest = blake2bMac256(cbor, personal: AccountAddress.multisigPersonalisation)
        return MultisigPolicy(version: version,
                              threshold: resolvedThreshold,
                              members: members,
                              ctap2Cbor: cbor,
                              digestBlake2b256: digest)
    }

    private func curveId(for algorithm: SigningAlgorithm) throws -> CurveId {
        switch algorithm {
        case .ed25519:
            return .ed25519
        case .mlDsa:
            #if IROHASWIFT_ENABLE_MLDSA
            return .mldsa
            #else
            throw MultisigBuilderError.unsupportedAlgorithm(algorithm)
            #endif
        case .sm2:
            #if IROHASWIFT_ENABLE_SM
            return .sm2
            #else
            throw MultisigBuilderError.unsupportedAlgorithm(algorithm)
            #endif
        case .secp256k1:
            #if IROHASWIFT_ENABLE_SECP256K1
            return .secp256k1
            #else
            throw MultisigBuilderError.unsupportedAlgorithm(algorithm)
            #endif
        }
    }
}
