import Foundation

/// Minimal Norito struct-field framing for negative wire tests: each field is a
/// canonical unsigned LEB128 length followed by exactly that many bytes.
enum NoritoFieldFrames {
    static let maximumFieldBytes = 32 * 1024 * 1024
    struct Failure: Error {}

    /// Returns the canonical (shortest) LEB128 encoding of `value`.
    static func length(_ value: Int) -> Data {
        var n = value, out = Data()
        repeat { let byte = UInt8(n & 127); n >>= 7; out.append(byte | (n == 0 ? 0 : 128)) } while n != 0
        return out
    }

    /// Frames every field with its canonical length prefix, in order.
    static func record(_ fields: [Data]) -> Data {
        var out = Data()
        for field in fields { out.append(length(field.count)); out.append(field) }
        return out
    }

    /// Strict cursor over length-framed fields; non-canonical prefixes and overruns fail.
    struct Reader {
        private let bytes: Data
        private var cursor = 0
        init(_ bytes: Data) { self.bytes = Data(bytes) }
        private var remaining: Int { bytes.count - cursor }

        /// Takes exactly `size` unframed bytes.
        mutating func raw(_ size: Int) throws -> Data {
            guard size >= 0, size <= remaining else { throw Failure() }
            defer { cursor += size }; return Data(bytes[cursor..<(cursor + size)])
        }

        /// Requires that every byte was consumed.
        func finish() throws { guard remaining == 0 else { throw Failure() } }

        /// Takes one canonical length-framed field.
        mutating func field() throws -> Data {
            var value = 0, shift = 0, count = 0
            while true {
                guard shift <= 28 else { throw Failure() }
                let byte = try raw(1)[0]
                guard shift < 28 || byte <= 7 else { throw Failure() }
                value |= Int(byte & 127) << shift; count += 1
                if byte & 128 == 0 { break }; shift += 7
            }
            guard value <= maximumFieldBytes, length(value).count == count else { throw Failure() }
            return try raw(value)
        }
    }
}
