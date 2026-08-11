import CryptoKit
import Foundation

public enum ConfidentialNoteError: Error, Equatable, LocalizedError {
    case invalidField(String)
    case bridgeUnavailable
    case cryptographyFailed

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Invalid confidential note field: \(field)."
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "The canonical confidential V3 native bridge is required for note derivation."
            )
        case .cryptographyFailed:
            return "Confidential note cryptography failed."
        }
    }
}

public struct ConfidentialNoteOpening: Equatable, Sendable {
    public let rho: Data
    public let spendKey: Data
    public let ownerTag: Data
    public let asset: String
    public let networkId: NetworkId
    public let amount: String

    public init(
        rho: Data,
        spendKey: Data,
        ownerTag: Data,
        asset: String,
        networkId: NetworkId,
        amount: String
    ) throws {
        self.rho = try ConfidentialNoteCrypto.fixedNonZeroBytes(rho, count: 32, field: "rho")
        self.spendKey = try ConfidentialNoteCrypto.fixedNonZeroBytes(
            spendKey,
            count: 32,
            field: "spendKey"
        )
        self.ownerTag = try ConfidentialNoteCrypto.fixedScalar(ownerTag, field: "ownerTag")
        self.asset = try ConfidentialNoteCrypto.canonicalText(asset, field: "asset")
        self.networkId = networkId
        self.amount = try ConfidentialNoteCrypto.canonicalU128(amount, field: "amount")
    }

    public static func fromSpendKey(
        rho: Data,
        spendKey: Data,
        asset: String,
        networkId: NetworkId,
        amount: String
    ) throws -> ConfidentialNoteOpening {
        try ConfidentialNoteOpening(
            rho: rho,
            spendKey: spendKey,
            ownerTag: ConfidentialOwnerTag.deriveFromSpendKey(spendKey),
            asset: asset,
            networkId: networkId,
            amount: amount
        )
    }

    public static func fromSpendKeyWithDiversifier(
        rho: Data,
        spendKey: Data,
        diversifier: Data,
        asset: String,
        networkId: NetworkId,
        amount: String
    ) throws -> ConfidentialNoteOpening {
        try ConfidentialNoteOpening(
            rho: rho,
            spendKey: spendKey,
            ownerTag: ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(
                spendKey,
                diversifier: diversifier
            ),
            asset: asset,
            networkId: networkId,
            amount: amount
        )
    }
}

public enum ConfidentialOwnerTag {
    public static func defaultDiversifier() throws -> Data {
        try ConfidentialNoteNativeDerivation.defaultDiversifierV3()
    }

    public static func deriveDiversifier(_ seed: Data) throws -> Data {
        guard !seed.isEmpty, seed.count <= 4_096 else {
            throw ConfidentialNoteError.invalidField("seed")
        }
        return try ConfidentialNoteNativeDerivation.deriveDiversifierV3(seed: seed)
    }

    public static func deriveFromSpendKey(_ spendKey: Data) throws -> Data {
        try deriveFromSpendKeyWithDiversifier(
            spendKey,
            diversifier: try defaultDiversifier()
        )
    }

    public static func deriveFromSpendKeyWithDiversifier(
        _ spendKey: Data,
        diversifier: Data
    ) throws -> Data {
        let spendKey = try ConfidentialNoteCrypto.fixedNonZeroBytes(
            spendKey,
            count: 32,
            field: "spendKey"
        )
        let diversifier = try ConfidentialNoteCrypto.fixedScalar(
            diversifier,
            field: "diversifier"
        )
        return try ConfidentialNoteNativeDerivation.deriveOwnerTagV3(
            spendKey: spendKey,
            diversifier: diversifier
        )
    }
}

public enum ConfidentialNoteCommitment {
    public static func deriveFromOpening(_ opening: ConfidentialNoteOpening) throws -> Data {
        try ConfidentialNoteNativeDerivation.deriveNoteCommitmentV3(
            asset: Data(opening.asset.utf8),
            amount: Data(opening.amount.utf8),
            rho: opening.rho,
            ownerTag: opening.ownerTag
        )
    }
}

public enum ConfidentialNoteNullifier {
    public static func deriveFromOpening(_ opening: ConfidentialNoteOpening) throws -> Data {
        try ConfidentialNoteNativeDerivation.deriveNullifierV3(
            networkID: opening.networkId.bytes,
            asset: Data(opening.asset.utf8),
            spendKey: opening.spendKey,
            rho: opening.rho
        )
    }
}

public enum ConfidentialNoteTags {
    public static func deriveAssetTag(_ asset: String) throws -> Data {
        let canonical = try ConfidentialNoteCrypto.canonicalText(asset, field: "asset")
        return try ConfidentialNoteNativeDerivation.deriveAssetTagV3(asset: Data(canonical.utf8))
    }

    public static func deriveNetworkTag(_ networkId: NetworkId) throws -> Data {
        try ConfidentialNoteNativeDerivation.deriveNetworkTagV3(networkID: networkId.bytes)
    }
}

public enum ConfidentialNoteEncryption {
    public static func publicKeyFromPrivateKey(_ privateKey: Data) throws -> Data {
        let privateBytes = try ConfidentialNoteCrypto.fixedNonZeroBytes(
            privateKey,
            count: 32,
            field: "privateKey"
        )
        do {
            let key = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: privateBytes)
            return key.publicKey.rawRepresentation
        } catch {
            throw ConfidentialNoteError.invalidField("privateKey")
        }
    }

    public static func encryptNote(
        opening: ConfidentialNoteOpening,
        recipientPublicKey: Data
    ) throws -> ConfidentialEncryptedPayload {
        let ephemeralPrivateKey = Curve25519.KeyAgreement.PrivateKey().rawRepresentation
        var nonce = Data()
        nonce.reserveCapacity(24)
        for _ in 0..<24 {
            nonce.append(UInt8.random(in: UInt8.min...UInt8.max))
        }
        return try encryptNote(
            opening: opening,
            recipientPublicKey: recipientPublicKey,
            ephemeralPrivateKey: ephemeralPrivateKey,
            nonce: nonce
        )
    }

    public static func encryptNote(
        opening: ConfidentialNoteOpening,
        recipientPublicKey: Data,
        ephemeralPrivateKey: Data,
        nonce: Data
    ) throws -> ConfidentialEncryptedPayload {
        let recipientPublic = try ConfidentialNoteCrypto.fixedBytes(
            recipientPublicKey,
            count: 32,
            field: "recipientPublicKey"
        )
        let ephemeralPrivate = try ConfidentialNoteCrypto.fixedNonZeroBytes(
            ephemeralPrivateKey,
            count: 32,
            field: "ephemeralPrivateKey"
        )
        let nonceBytes = try ConfidentialNoteCrypto.fixedBytes(nonce, count: 24, field: "nonce")
        let ephemeralPublic = try publicKeyFromPrivateKey(ephemeralPrivate)
        let key = try derivePayloadKey(
            localPrivateKey: ephemeralPrivate,
            peerPublicKey: recipientPublic,
            ephemeralPublicKey: ephemeralPublic,
            recipientPublicKey: recipientPublic
        )
        let plaintext = try encodePlaintext(opening)
        let ciphertext = try xChaCha20Poly1305Seal(
            key: key,
            nonce: nonceBytes,
            aad: payloadAad(ephemeralPublicKey: ephemeralPublic, recipientPublicKey: recipientPublic),
            plaintext: plaintext
        )
        return try ConfidentialEncryptedPayload(
            ephemeralPublicKey: ephemeralPublic,
            nonce: nonceBytes,
            ciphertext: ciphertext
        )
    }

    static let plaintextVersion: UInt8 = 1
    static let textMaxBytes = 4096
    private static let kdfSalt = Data("iroha:confidential-note:v1:x25519".utf8)
    private static let kdfInfoPrefix = Data("iroha:confidential-note:v1:xchacha20poly1305".utf8)
    private static let aadPrefix = Data("iroha:confidential-note:v1".utf8)

    private static func encodePlaintext(_ opening: ConfidentialNoteOpening) throws -> Data {
        let assetBytes = Data(opening.asset.utf8)
        let amountBytes = Data(opening.amount.utf8)
        guard (1...textMaxBytes).contains(assetBytes.count) else {
            throw ConfidentialNoteError.invalidField("asset")
        }
        guard (1...textMaxBytes).contains(amountBytes.count) else {
            throw ConfidentialNoteError.invalidField("amount")
        }

        var out = Data()
        out.reserveCapacity(1 + 96 + assetBytes.count + amountBytes.count + 8)
        out.append(plaintextVersion)
        out.append(opening.rho)
        out.append(opening.ownerTag)
        appendVarint(UInt64(assetBytes.count), to: &out)
        out.append(assetBytes)
        out.append(opening.networkId.bytes)
        appendVarint(UInt64(amountBytes.count), to: &out)
        out.append(amountBytes)
        return out
    }

    static func derivePayloadKey(
        localPrivateKey: Data,
        peerPublicKey: Data,
        ephemeralPublicKey: Data,
        recipientPublicKey: Data
    ) throws -> SymmetricKey {
        do {
            let local = try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: localPrivateKey)
            let peer = try Curve25519.KeyAgreement.PublicKey(rawRepresentation: peerPublicKey)
            let shared = try local.sharedSecretFromKeyAgreement(with: peer)
            let allZero = shared.withUnsafeBytes { bytes in
                bytes.allSatisfy { $0 == 0 }
            }
            guard !allZero else {
                throw ConfidentialNoteError.invalidField("peerPublicKey")
            }
            let sharedKey = SymmetricKey(data: shared.withUnsafeBytes { Data($0) })
            return HKDF<SHA256>.deriveKey(
                inputKeyMaterial: sharedKey,
                salt: kdfSalt,
                info: payloadKdfInfo(
                    ephemeralPublicKey: ephemeralPublicKey,
                    recipientPublicKey: recipientPublicKey
                ),
                outputByteCount: 32
            )
        } catch let error as ConfidentialNoteError {
            throw error
        } catch {
            throw ConfidentialNoteError.invalidField("peerPublicKey")
        }
    }

    private static func payloadKdfInfo(ephemeralPublicKey: Data, recipientPublicKey: Data) -> Data {
        var out = kdfInfoPrefix
        out.append(ephemeralPublicKey)
        out.append(recipientPublicKey)
        return out
    }

    static func payloadAad(ephemeralPublicKey: Data, recipientPublicKey: Data) -> Data {
        var out = aadPrefix
        out.append(plaintextVersion)
        out.append(ephemeralPublicKey)
        out.append(recipientPublicKey)
        return out
    }

    static func xChaCha20Poly1305Seal(
        key: SymmetricKey,
        nonce: Data,
        aad: Data,
        plaintext: Data
    ) throws -> Data {
        let keyBytes = key.withUnsafeBytes { Data($0) }
        let subkey = try hChaCha20(key: keyBytes, nonce16: Data(nonce.prefix(16)))
        var ietfNonce = Data(repeating: 0, count: 4)
        ietfNonce.append(nonce.suffix(8))
        do {
            let sealed = try ChaChaPoly.seal(
                plaintext,
                using: SymmetricKey(data: subkey),
                nonce: ChaChaPoly.Nonce(data: ietfNonce),
                authenticating: aad
            )
            var out = sealed.ciphertext
            out.append(sealed.tag)
            return out
        } catch {
            throw ConfidentialNoteError.cryptographyFailed
        }
    }

    static func xChaCha20Poly1305Open(
        key: SymmetricKey,
        nonce: Data,
        aad: Data,
        ciphertext: Data
    ) throws -> Data {
        guard ciphertext.count >= 16 else {
            throw ConfidentialNoteError.cryptographyFailed
        }
        let keyBytes = key.withUnsafeBytes { Data($0) }
        let subkey = try hChaCha20(key: keyBytes, nonce16: Data(nonce.prefix(16)))
        var ietfNonce = Data(repeating: 0, count: 4)
        ietfNonce.append(nonce.suffix(8))
        let encrypted = ciphertext.prefix(ciphertext.count - 16)
        let tag = ciphertext.suffix(16)
        do {
            let sealed = try ChaChaPoly.SealedBox(
                nonce: ChaChaPoly.Nonce(data: ietfNonce),
                ciphertext: encrypted,
                tag: tag
            )
            return try ChaChaPoly.open(
                sealed,
                using: SymmetricKey(data: subkey),
                authenticating: aad
            )
        } catch {
            throw ConfidentialNoteError.cryptographyFailed
        }
    }

    private static func hChaCha20(key: Data, nonce16: Data) throws -> Data {
        guard key.count == 32 else {
            throw ConfidentialNoteError.invalidField("key")
        }
        guard nonce16.count == 16 else {
            throw ConfidentialNoteError.invalidField("nonce16")
        }
        var state: [UInt32] = [
            0x6170_7865, 0x3320_646e, 0x7962_2d32, 0x6b20_6574,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]
        for index in 0..<8 {
            state[4 + index] = loadUInt32LE(key, offset: index * 4)
        }
        for index in 0..<4 {
            state[12 + index] = loadUInt32LE(nonce16, offset: index * 4)
        }
        for _ in 0..<10 {
            quarterRound(&state, 0, 4, 8, 12)
            quarterRound(&state, 1, 5, 9, 13)
            quarterRound(&state, 2, 6, 10, 14)
            quarterRound(&state, 3, 7, 11, 15)
            quarterRound(&state, 0, 5, 10, 15)
            quarterRound(&state, 1, 6, 11, 12)
            quarterRound(&state, 2, 7, 8, 13)
            quarterRound(&state, 3, 4, 9, 14)
        }
        var out = Data()
        out.reserveCapacity(32)
        for index in [0, 1, 2, 3, 12, 13, 14, 15] {
            appendUInt32LE(state[index], to: &out)
        }
        return out
    }

    private static func quarterRound(
        _ state: inout [UInt32],
        _ a: Int,
        _ b: Int,
        _ c: Int,
        _ d: Int
    ) {
        state[a] = state[a] &+ state[b]
        state[d] = (state[d] ^ state[a]).rotatedLeft(16)
        state[c] = state[c] &+ state[d]
        state[b] = (state[b] ^ state[c]).rotatedLeft(12)
        state[a] = state[a] &+ state[b]
        state[d] = (state[d] ^ state[a]).rotatedLeft(8)
        state[c] = state[c] &+ state[d]
        state[b] = (state[b] ^ state[c]).rotatedLeft(7)
    }

    private static func loadUInt32LE(_ bytes: Data, offset: Int) -> UInt32 {
        UInt32(bytes[offset])
            | (UInt32(bytes[offset + 1]) << 8)
            | (UInt32(bytes[offset + 2]) << 16)
            | (UInt32(bytes[offset + 3]) << 24)
    }

    private static func appendUInt32LE(_ value: UInt32, to data: inout Data) {
        data.append(UInt8(value & 0xff))
        data.append(UInt8((value >> 8) & 0xff))
        data.append(UInt8((value >> 16) & 0xff))
        data.append(UInt8((value >> 24) & 0xff))
    }

    private static func appendVarint(_ value: UInt64, to data: inout Data) {
        var remaining = value
        while remaining >= 0x80 {
            data.append(UInt8(remaining & 0x7f) | 0x80)
            remaining >>= 7
        }
        data.append(UInt8(remaining))
    }
}

public enum ConfidentialNoteDecryption {
    public static func decryptNote(
        encryptedPayload: ConfidentialEncryptedPayload,
        recipientPrivateKey: Data,
        spendKey: Data,
        expectedNetworkId: NetworkId
    ) throws -> ConfidentialNoteOpening {
        try decryptNoteWithOwnerTag(
            encryptedPayload: encryptedPayload,
            recipientPrivateKey: recipientPrivateKey,
            spendKey: spendKey,
            expectedOwnerTag: ConfidentialOwnerTag.deriveFromSpendKey(spendKey),
            expectedNetworkId: expectedNetworkId
        )
    }

    public static func decryptNoteWithOwnerTag(
        encryptedPayload: ConfidentialEncryptedPayload,
        recipientPrivateKey: Data,
        spendKey: Data,
        expectedOwnerTag: Data,
        expectedNetworkId: NetworkId
    ) throws -> ConfidentialNoteOpening {
        let recipientPrivate = try ConfidentialNoteCrypto.fixedNonZeroBytes(
            recipientPrivateKey,
            count: 32,
            field: "recipientPrivateKey"
        )
        let spendKeyBytes = try ConfidentialNoteCrypto.fixedNonZeroBytes(
            spendKey,
            count: 32,
            field: "spendKey"
        )
        let expectedOwnerTagBytes = try ConfidentialNoteCrypto.fixedScalar(
            expectedOwnerTag,
            field: "expectedOwnerTag"
        )
        let recipientPublic = try ConfidentialNoteEncryption.publicKeyFromPrivateKey(recipientPrivate)
        let key = try ConfidentialNoteEncryption.derivePayloadKey(
            localPrivateKey: recipientPrivate,
            peerPublicKey: encryptedPayload.ephemeralPublicKey,
            ephemeralPublicKey: encryptedPayload.ephemeralPublicKey,
            recipientPublicKey: recipientPublic
        )
        let plaintext = try ConfidentialNoteEncryption.xChaCha20Poly1305Open(
            key: key,
            nonce: encryptedPayload.nonce,
            aad: ConfidentialNoteEncryption.payloadAad(
                ephemeralPublicKey: encryptedPayload.ephemeralPublicKey,
                recipientPublicKey: recipientPublic
            ),
            ciphertext: encryptedPayload.ciphertext
        )
        let decoded = try decodePlaintext(plaintext)
        guard decoded.networkId == expectedNetworkId else {
            throw ConfidentialNoteError.invalidField("expectedNetworkId")
        }
        guard decoded.ownerTag == expectedOwnerTagBytes else {
            throw ConfidentialNoteError.invalidField("expectedOwnerTag")
        }
        return try ConfidentialNoteOpening(
            rho: decoded.rho,
            spendKey: spendKeyBytes,
            ownerTag: decoded.ownerTag,
            asset: decoded.asset,
            networkId: decoded.networkId,
            amount: decoded.amount
        )
    }

    private struct DecodedPlaintext {
        let rho: Data
        let ownerTag: Data
        let asset: String
        let networkId: NetworkId
        let amount: String
    }

    private static func decodePlaintext(_ bytes: Data) throws -> DecodedPlaintext {
        guard !bytes.isEmpty,
              bytes[bytes.startIndex] == ConfidentialNoteEncryption.plaintextVersion else {
            throw ConfidentialNoteError.invalidField("plaintext.version")
        }
        var offset = bytes.startIndex + 1
        guard bytes.count >= 97 else {
            throw ConfidentialNoteError.invalidField("plaintext")
        }
        let rho = Data(bytes[offset..<offset + 32])
        offset += 32
        let ownerTag = try ConfidentialNoteCrypto.fixedScalar(
            Data(bytes[offset..<offset + 32]),
            field: "ownerTag"
        )
        offset += 32
        let asset = try readText(bytes, offset: &offset, field: "asset")
        guard bytes.count - offset >= 32 else {
            throw ConfidentialNoteError.invalidField("networkId")
        }
        let networkId: NetworkId
        do {
            networkId = try NetworkId(bytes: Data(bytes[offset..<offset + 32]))
        } catch {
            throw ConfidentialNoteError.invalidField("networkId")
        }
        offset += 32
        let amount = try ConfidentialNoteCrypto.canonicalU128(
            readText(bytes, offset: &offset, field: "amount"),
            field: "amount"
        )
        guard offset == bytes.endIndex else {
            throw ConfidentialNoteError.invalidField("plaintext.trailing")
        }
        return DecodedPlaintext(
            rho: rho,
            ownerTag: ownerTag,
            asset: asset,
            networkId: networkId,
            amount: amount
        )
    }

    private static func readText(_ bytes: Data, offset: inout Int, field: String) throws -> String {
        let length = try readVarint(bytes, offset: &offset)
        guard (1...UInt64(ConfidentialNoteEncryption.textMaxBytes)).contains(length),
              length <= UInt64(Int.max),
              bytes.count - offset >= Int(length) else {
            throw ConfidentialNoteError.invalidField(field)
        }
        let end = offset + Int(length)
        guard let text = String(data: Data(bytes[offset..<end]), encoding: .utf8) else {
            throw ConfidentialNoteError.invalidField(field)
        }
        offset = end
        return try ConfidentialNoteCrypto.canonicalText(text, field: field)
    }

    private static func readVarint(_ bytes: Data, offset: inout Int) throws -> UInt64 {
        let startOffset = offset
        var shift = 0
        var value: UInt64 = 0
        while offset < bytes.endIndex {
            let byte = bytes[offset]
            offset += 1
            guard shift < 64 else {
                throw ConfidentialNoteError.invalidField("varint")
            }
            let part = UInt64(byte & 0x7f)
            guard shift != 63 || part <= 1 else {
                throw ConfidentialNoteError.invalidField("varint")
            }
            value |= part << UInt64(shift)
            if byte & 0x80 == 0 {
                let width = offset - startOffset
                if width > 1 {
                    let minimumCanonical = UInt64(1) << UInt64(7 * (width - 1))
                    guard value >= minimumCanonical else {
                        throw ConfidentialNoteError.invalidField("varint")
                    }
                }
                return value
            }
            guard shift < 63 else {
                throw ConfidentialNoteError.invalidField("varint")
            }
            shift += 7
        }
        throw ConfidentialNoteError.invalidField("varint")
    }
}

enum ConfidentialNoteCrypto {
    static func fixedBytes(_ value: Data, count: Int, field: String) throws -> Data {
        guard value.count == count else {
            throw ConfidentialNoteError.invalidField(field)
        }
        return Data(value)
    }

    static func fixedNonZeroBytes(_ value: Data, count: Int, field: String) throws -> Data {
        let bytes = try fixedBytes(value, count: count, field: field)
        guard bytes.contains(where: { $0 != 0 }) else {
            throw ConfidentialNoteError.invalidField(field)
        }
        return bytes
    }

    static func fixedScalar(_ value: Data, field: String) throws -> Data {
        let bytes = try fixedBytes(value, count: 32, field: field)
        _ = try scalar(bytes, field: field)
        guard bytes.contains(where: { $0 != 0 }) else {
            throw ConfidentialNoteError.invalidField(field)
        }
        return bytes
    }

    static func scalar(_ bytes: Data, field: String) throws -> PastaFp {
        guard let scalar = PastaFp.fromCanonicalBytes(bytes) else {
            throw ConfidentialNoteError.invalidField(field)
        }
        return scalar
    }

    static func canonicalText(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed == value, !trimmed.contains("\0") else {
            throw ConfidentialNoteError.invalidField(field)
        }
        return trimmed
    }

    static func canonicalU128(_ value: String, field: String) throws -> String {
        let text = try canonicalText(value, field: field)
        guard text.allSatisfy({ $0 >= "0" && $0 <= "9" }) else {
            throw ConfidentialNoteError.invalidField(field)
        }
        guard text != "0", !text.hasPrefix("0") else {
            throw ConfidentialNoteError.invalidField(field)
        }
        let max = "340282366920938463463374607431768211455"
        guard text.count < max.count || (text.count == max.count && text <= max) else {
            throw ConfidentialNoteError.invalidField(field)
        }
        return text
    }

    static func u128LittleEndianBytes(_ value: String) throws -> Data {
        var digits = value.compactMap(\.wholeNumberValue)
        var output = Data()
        while !(digits.count == 1 && digits[0] == 0) {
            var quotient: [Int] = []
            var remainder = 0
            for digit in digits {
                let current = remainder * 10 + digit
                let q = current / 256
                remainder = current % 256
                if !quotient.isEmpty || q != 0 {
                    quotient.append(q)
                }
            }
            output.append(UInt8(remainder))
            digits = quotient.isEmpty ? [0] : quotient
        }
        guard output.count <= 16 else {
            throw ConfidentialNoteError.invalidField("u128")
        }
        while output.count < 16 {
            output.append(0)
        }
        return output
    }

}

private extension UInt32 {
    func rotatedLeft(_ amount: UInt32) -> UInt32 {
        (self << amount) | (self >> (32 - amount))
    }
}
