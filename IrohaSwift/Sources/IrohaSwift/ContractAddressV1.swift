import Foundation

public enum ContractAddressV1Error: Error, LocalizedError, Equatable {
    case invalidLiteral
    case nativeAddressCodecUnavailable
    case subjectDerivationExhausted

    public var errorDescription: String? {
        switch self {
        case .invalidLiteral:
            return "Contract address is not a canonical ABI V1 literal."
        case .nativeAddressCodecUnavailable:
            return "The native account-address codec required for contract subject derivation is unavailable."
        case .subjectDerivationExhausted:
            return "Contract subject hash-to-point retry counter was exhausted."
        }
    }
}

/// Parser for the canonical V1 contract-address wire literal.
public enum ContractAddressV1 {
    private static let canonicalHrp = Array("irohac".utf8)
    private static let subjectHashToPointTagV1 =
        Data("iroha:contract-subject:hash-to-point:v1:".utf8)
    private static let bech32mConstant: UInt32 = 0x2bc8_30a3
    private static let charset = Array("qpzry9x8gf2tvdw0s3jn54khce6mua7l".utf8)
    private static let generators: [UInt32] = [
        0x3b6a_57b2,
        0x2650_8e6d,
        0x1ea1_19fa,
        0x3d42_33dd,
        0x2a14_62b3,
    ]

    public static func isCanonical(_ literal: String) -> Bool {
        let bytes = Array(literal.utf8)
        guard !bytes.isEmpty,
              bytes.count <= 90,
              bytes.allSatisfy({ (33 ... 126).contains(Int($0)) }),
              !bytes.contains(where: { (65 ... 90).contains(Int($0)) }),
              let separator = bytes.lastIndex(of: Character("1").asciiValue!),
              separator > bytes.startIndex,
              separator <= 83,
              bytes.distance(from: bytes.index(after: separator), to: bytes.endIndex) >= 6 else {
            return false
        }

        let hrp = Array(bytes[..<separator])
        guard hrp == canonicalHrp else {
            return false
        }
        var values = [UInt8]()
        values.reserveCapacity(bytes.distance(from: bytes.index(after: separator), to: bytes.endIndex))
        for character in bytes[bytes.index(after: separator)...] {
            guard let value = charset.firstIndex(of: character) else {
                return false
            }
            values.append(UInt8(value))
        }

        guard polymod(hrpExpand(hrp) + values) == bech32mConstant else {
            return false
        }
        guard let payload = decodePayload(values.dropLast(6)),
              payload.count == 29,
              payload.first == 1 else {
            return false
        }
        return true
    }

    /// Derive the canonical non-signable account subject for an ABI V1 contract.
    ///
    /// The native account-address decoder performs the same strict prime-order
    /// Ed25519 point admission as Rust `ContractAddress::subject_id()`. CryptoKit
    /// construction alone is deliberately insufficient because it accepts
    /// arbitrary 32-byte compressed-key material.
    public static func subjectAccountId(
        _ literal: String,
        networkPrefix: UInt16 = AccountId.defaultNetworkPrefix
    ) throws -> String {
        guard isCanonical(literal) else {
            throw ContractAddressV1Error.invalidLiteral
        }
        let bridge = NoritoNativeBridge.shared
        guard bridge.isAccountAddressCodecAvailable else {
            throw ContractAddressV1Error.nativeAddressCodecUnavailable
        }

        let address = Data(literal.utf8)
        var counter: UInt32 = 0
        while true {
            var counterBE = counter.bigEndian
            var payload = subjectHashToPointTagV1
            payload.append(address)
            withUnsafeBytes(of: &counterBE) { payload.append(contentsOf: $0) }
            let candidate = IrohaHash.hash(payload)
            let account = try AccountAddress.fromAccount(
                publicKey: candidate,
                algorithm: "ed25519"
            )
            do {
                guard let rendered = try bridge.renderAccountAddress(
                    canonicalBytes: account.canonicalBytes(),
                    networkPrefix: networkPrefix
                ) else {
                    throw ContractAddressV1Error.nativeAddressCodecUnavailable
                }
                return rendered.i105
            } catch AccountAddressError.invalidPublicKey {
                if counter == UInt32.max {
                    throw ContractAddressV1Error.subjectDerivationExhausted
                }
                counter += 1
            }
        }
    }

    private static func hrpExpand(_ hrp: [UInt8]) -> [UInt8] {
        hrp.map { $0 >> 5 } + [0] + hrp.map { $0 & 0x1f }
    }

    private static func polymod(_ values: [UInt8]) -> UInt32 {
        var checksum: UInt32 = 1
        for value in values {
            let top = checksum >> 25
            checksum = ((checksum & 0x01ff_ffff) << 5) ^ UInt32(value)
            for (index, generator) in generators.enumerated()
                where ((top >> UInt32(index)) & 1) != 0 {
                checksum ^= generator
            }
        }
        return checksum
    }

    private static func decodePayload(_ values: ArraySlice<UInt8>) -> [UInt8]? {
        var output = [UInt8]()
        var accumulator: UInt32 = 0
        var bits = 0
        for value in values {
            accumulator = (accumulator << 5) | UInt32(value)
            bits += 5
            while bits >= 8 {
                bits -= 8
                output.append(UInt8((accumulator >> UInt32(bits)) & 0xff))
            }
            accumulator &= bits == 0 ? 0 : (1 << UInt32(bits)) - 1
        }
        guard bits < 5, bits == 0 || accumulator == 0 else {
            return nil
        }
        return output
    }
}
