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
        guard let digest = blake3Hash(data: body),
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
        guard let digest = blake3Hash(data: Data(body)),
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

    private static func blake3Hash(data: Data) -> Data? {
        NoritoNativeBridge.shared.blake3Hash(data: data) ?? AssetDefinitionAddressBlake3.hashSmallInput(data)
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

public enum AssetDefinitionAddressCodec {
    public static func canonicalDefinitionLiteral(_ rawLiteral: String?) -> String? {
        let trimmed = rawLiteral?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !trimmed.isEmpty,
              trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) == nil,
              AssetDefinitionAddress.decode(trimmed) != nil else {
            return nil
        }
        return trimmed
    }
}

private enum AssetDefinitionAddressBlake3 {
    private static let iv: [UInt32] = [
        0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
        0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
    ]
    private static let permutation = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]
    private static let chunkStart: UInt32 = 1
    private static let chunkEnd: UInt32 = 2
    private static let root: UInt32 = 8

    static func hashSmallInput(_ data: Data) -> Data? {
        guard data.count <= 64 else {
            return nil
        }
        var block = [UInt8](repeating: 0, count: 64)
        block.replaceSubrange(0..<data.count, with: data)
        var words = [UInt32](repeating: 0, count: 16)
        for index in 0..<16 {
            let start = index * 4
            words[index] = UInt32(block[start])
                | (UInt32(block[start + 1]) << 8)
                | (UInt32(block[start + 2]) << 16)
                | (UInt32(block[start + 3]) << 24)
        }

        var state = [UInt32](repeating: 0, count: 16)
        for index in 0..<8 {
            state[index] = iv[index]
        }
        for index in 0..<4 {
            state[index + 8] = iv[index]
        }
        state[12] = 0
        state[13] = 0
        state[14] = UInt32(data.count)
        state[15] = chunkStart | chunkEnd | root

        var message = words
        for _ in 0..<7 {
            round(&state, message)
            message = permutation.map { message[$0] }
        }

        var output = Data(capacity: 32)
        for index in 0..<8 {
            var word = (state[index] ^ state[index + 8]).littleEndian
            withUnsafeBytes(of: &word) { output.append(contentsOf: $0) }
        }
        return output
    }

    private static func round(_ state: inout [UInt32], _ message: [UInt32]) {
        mix(&state, 0, 4, 8, 12, message[0], message[1])
        mix(&state, 1, 5, 9, 13, message[2], message[3])
        mix(&state, 2, 6, 10, 14, message[4], message[5])
        mix(&state, 3, 7, 11, 15, message[6], message[7])
        mix(&state, 0, 5, 10, 15, message[8], message[9])
        mix(&state, 1, 6, 11, 12, message[10], message[11])
        mix(&state, 2, 7, 8, 13, message[12], message[13])
        mix(&state, 3, 4, 9, 14, message[14], message[15])
    }

    private static func mix(
        _ state: inout [UInt32],
        _ a: Int,
        _ b: Int,
        _ c: Int,
        _ d: Int,
        _ x: UInt32,
        _ y: UInt32
    ) {
        state[a] = state[a] &+ state[b] &+ x
        state[d] = rotateRight(state[d] ^ state[a], by: 16)
        state[c] = state[c] &+ state[d]
        state[b] = rotateRight(state[b] ^ state[c], by: 12)
        state[a] = state[a] &+ state[b] &+ y
        state[d] = rotateRight(state[d] ^ state[a], by: 8)
        state[c] = state[c] &+ state[d]
        state[b] = rotateRight(state[b] ^ state[c], by: 7)
    }

    private static func rotateRight(_ value: UInt32, by amount: UInt32) -> UInt32 {
        (value >> amount) | (value << (32 - amount))
    }
}
