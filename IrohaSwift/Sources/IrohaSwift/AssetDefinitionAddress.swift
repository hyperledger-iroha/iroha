import Foundation

enum AssetDefinitionAddress {
    private static let version: UInt8 = 1
    private static let alphabet = Array("123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz")
    private static let alphabetSet = CharacterSet(charactersIn: String(alphabet))
    private static let alphabetIndex: [Character: Int] = {
        Dictionary(uniqueKeysWithValues: alphabet.enumerated().map { ($0.element, $0.offset) })
    }()

    static func looksCanonical(_ literal: String) -> Bool {
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed == literal else {
            return false
        }
        guard !trimmed.contains(":"), !trimmed.contains("#"), !trimmed.contains("@"), !trimmed.contains("$") else {
            return false
        }
        return trimmed.unicodeScalars.allSatisfy { alphabetSet.contains($0) }
    }

    static func encode(uuidBytes: Data) -> String? {
        guard uuidBytes.count == 16 else {
            return nil
        }
        var body = Data([version])
        body.append(uuidBytes)
        guard let digest = NoritoNativeBridge.shared.blake3Hash(data: body),
              digest.count >= 4 else {
            return nil
        }
        var payload = body
        payload.append(digest.prefix(4))
        return encodeBase58(payload)
    }

    static func decode(_ literal: String) -> Data? {
        guard looksCanonical(literal) else {
            return nil
        }
        let payload = decodeBase58(literal)
        guard payload.count == 21, payload.first == version else {
            return nil
        }
        let body = payload.prefix(17)
        let checksum = payload.suffix(4)
        guard let digest = NoritoNativeBridge.shared.blake3Hash(data: Data(body)),
              digest.prefix(4) == checksum else {
            return nil
        }
        let uuidBytes = Data(payload[1..<17])
        guard uuidBytes.count == 16 else {
            return nil
        }
        let bytes = [UInt8](uuidBytes)
        guard bytes[6] >> 4 == 0x4, (bytes[8] & 0xC0) == 0x80 else {
            return nil
        }
        return uuidBytes
    }

    private static func encodeBase58(_ data: Data) -> String {
        let input = [UInt8](data)
        let zeroCount = input.prefix(while: { $0 == 0 }).count
        var digits = [Int](repeating: 0, count: 1)

        for byte in input {
            var carry = Int(byte)
            for index in 0..<digits.count {
                carry += digits[index] << 8
                digits[index] = carry % 58
                carry /= 58
            }
            while carry > 0 {
                digits.append(carry % 58)
                carry /= 58
            }
        }

        var encoded = String(repeating: "1", count: zeroCount)
        for digit in digits.reversed() {
            encoded.append(alphabet[digit])
        }
        return encoded
    }

    private static func decodeBase58(_ literal: String) -> Data {
        let zeroCount = literal.prefix(while: { $0 == "1" }).count
        var bytes = [UInt8](repeating: 0, count: 1)

        for character in literal {
            guard let value = alphabetIndex[character] else {
                return Data()
            }
            var carry = value
            for index in 0..<bytes.count {
                let total = Int(bytes[index]) * 58 + carry
                bytes[index] = UInt8(total & 0xff)
                carry = total >> 8
            }
            while carry > 0 {
                bytes.append(UInt8(carry & 0xff))
                carry >>= 8
            }
        }

        var decoded = Data(repeating: 0, count: zeroCount)
        decoded.append(contentsOf: bytes.reversed())
        return decoded
    }
}
