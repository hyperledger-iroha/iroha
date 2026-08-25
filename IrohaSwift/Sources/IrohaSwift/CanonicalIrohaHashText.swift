import Foundation

/// Shared validation for the canonical text form of an Iroha `Hash`.
///
/// Text is the exact 32 marked bytes rendered as 64 lowercase hexadecimal
/// characters. The textual representation never changes the underlying bytes.
enum CanonicalIrohaHashText {
    static let byteCount = 32
    static let characterCount = byteCount * 2

    static func decode(_ literal: String) -> Data? {
        let input = Array(literal.utf8)
        guard input.count == characterCount,
              input.allSatisfy(isLowercaseHex) else {
            return nil
        }

        var bytes = Data(capacity: byteCount)
        for index in stride(from: 0, to: input.count, by: 2) {
            guard let high = hexNibble(input[index]),
                  let low = hexNibble(input[index + 1]) else {
                return nil
            }
            bytes.append((high << 4) | low)
        }
        guard bytes.count == byteCount,
              bytes.last.map({ $0 & 1 == 1 }) == true else {
            return nil
        }
        return bytes
    }

    static func encode(_ bytes: Data) -> String? {
        guard bytes.count == byteCount,
              bytes.last.map({ $0 & 1 == 1 }) == true else {
            return nil
        }
        return bytes.map { String(format: "%02x", $0) }.joined()
    }

    private static func isLowercaseHex(_ byte: UInt8) -> Bool {
        (0x30...0x39).contains(byte) || (0x61...0x66).contains(byte)
    }

    private static func hexNibble(_ byte: UInt8) -> UInt8? {
        switch byte {
        case 0x30...0x39:
            byte - 0x30
        case 0x61...0x66:
            byte - 0x61 + 10
        default:
            nil
        }
    }
}

/// The tagged representation is a Norito JSON contract, not Iroha `Display` text.
enum CanonicalIrohaHashJSONLiteral {
    private static let characterCount = 74

    static func decode(_ literal: String) -> Data? {
        let input = Array(literal.utf8)
        guard input.count == characterCount,
              Array(input[0..<5]) == Array("hash:".utf8),
              input[69] == 0x23,
              input[5..<69].allSatisfy(isUppercaseHex),
              input[70..<74].allSatisfy(isUppercaseHex),
              let suppliedChecksum = UInt16(
                  String(decoding: input[70..<74], as: UTF8.self),
                  radix: 16
              ),
              suppliedChecksum == crc16(input[0..<69]) else {
            return nil
        }
        let body = String(decoding: input[5..<69], as: UTF8.self).lowercased()
        return CanonicalIrohaHashText.decode(body)
    }

    static func encode(_ bytes: Data) -> String? {
        guard let raw = CanonicalIrohaHashText.encode(bytes) else {
            return nil
        }
        let prefix = "hash:\(raw.uppercased())"
        return "\(prefix)#\(String(format: "%04X", crc16(prefix.utf8)))"
    }

    private static func isUppercaseHex(_ byte: UInt8) -> Bool {
        (0x30...0x39).contains(byte) || (0x41...0x46).contains(byte)
    }

    private static func crc16<S: Sequence>(_ bytes: S) -> UInt16 where S.Element == UInt8 {
        var crc: UInt16 = 0xFFFF
        for byte in bytes {
            crc ^= UInt16(byte) << 8
            for _ in 0..<8 {
                crc = (crc & 0x8000) != 0 ? (crc << 1) ^ 0x1021 : crc << 1
            }
        }
        return crc
    }
}
