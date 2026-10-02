import Foundation
import XCTest
import zlib
@testable import IrohaSwift

/// Shared helpers of the Petal Stream tests: fixture loading, hex, zlib and
/// small image transforms.
enum PetalTestSupport {
    enum Failure: Error {
        case missingFixture(String)
        case malformed(String)
    }

    /// Locates `fixtures/petal/<name>` by walking up from this source file.
    static func fixtureURL(_ name: String) throws -> URL {
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<8 {
            let candidate = directory.appendingPathComponent("fixtures/petal/" + name)
            if FileManager.default.fileExists(atPath: candidate.path) { return candidate }
            directory.deleteLastPathComponent()
        }
        throw Failure.missingFixture(name)
    }

    static func loadJSON(_ name: String) throws -> [String: Any] {
        let data = try Data(contentsOf: fixtureURL(name))
        guard let object = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw Failure.malformed(name)
        }
        return object
    }

    static let streamFixture: [String: Any]? = try? loadJSON("petal_stream_v1.json")
    static let capturesFixture: [String: Any]? = try? loadJSON("petal_captures_v1.json")

    static func object(_ value: [String: Any], _ key: String) throws -> [String: Any] {
        guard let object = value[key] as? [String: Any] else { throw Failure.malformed(key) }
        return object
    }

    static func array(_ value: [String: Any], _ key: String) throws -> [Any] {
        guard let array = value[key] as? [Any] else { throw Failure.malformed(key) }
        return array
    }

    static func objects(_ value: [String: Any], _ key: String) throws -> [[String: Any]] {
        guard let array = value[key] as? [[String: Any]] else { throw Failure.malformed(key) }
        return array
    }

    static func integer(_ value: [String: Any], _ key: String) throws -> Int {
        guard let number = value[key] as? NSNumber else { throw Failure.malformed(key) }
        return number.intValue
    }

    static func integers(_ value: [String: Any], _ key: String) throws -> [Int] {
        guard let numbers = value[key] as? [NSNumber] else { throw Failure.malformed(key) }
        return numbers.map(\.intValue)
    }

    static func string(_ value: [String: Any], _ key: String) throws -> String {
        guard let text = value[key] as? String else { throw Failure.malformed(key) }
        return text
    }

    static func bytes(_ value: [String: Any], _ key: String) throws -> [UInt8] {
        try hex(string(value, key))
    }

    static func hex(_ text: String) throws -> [UInt8] {
        let digits = Array(text.utf8)
        guard digits.count % 2 == 0 else { throw Failure.malformed("odd hex") }
        func nibble(_ c: UInt8) throws -> UInt8 {
            switch c {
            case 48...57: return c - 48
            case 97...102: return c - 87
            case 65...70: return c - 55
            default: throw Failure.malformed("hex digit")
            }
        }
        return try stride(from: 0, to: digits.count, by: 2).map {
            try nibble(digits[$0]) << 4 | nibble(digits[$0 + 1])
        }
    }

    static func hexString(_ bytes: [UInt8]) -> String {
        bytes.map { String(format: "%02x", $0) }.joined()
    }

    /// Inflates a zlib (RFC 1950) stream of exactly `expectedLength` bytes.
    static func inflate(_ compressed: Data, expectedLength: Int) throws -> [UInt8] {
        var stream = z_stream()
        guard inflateInit_(&stream, ZLIB_VERSION, Int32(MemoryLayout<z_stream>.size)) == Z_OK else {
            throw Failure.malformed("inflateInit")
        }
        defer { inflateEnd(&stream) }
        var output = [UInt8](repeating: 0, count: expectedLength + 1)
        let capacity = output.count
        var status = Int32(Z_STREAM_ERROR)
        var input = [UInt8](compressed)
        input.withUnsafeMutableBufferPointer { inputBuffer in
            output.withUnsafeMutableBufferPointer { outputBuffer in
                stream.next_in = inputBuffer.baseAddress
                stream.avail_in = uInt(inputBuffer.count)
                stream.next_out = outputBuffer.baseAddress
                stream.avail_out = uInt(capacity)
                status = zlib.inflate(&stream, Z_FINISH)
            }
        }
        guard status == Z_STREAM_END, stream.total_out == uLong(expectedLength) else {
            throw Failure.malformed("inflate status \(status)")
        }
        return Array(output.prefix(expectedLength))
    }

    /// Decodes a capture or negative entry of `petal_captures_v1.json`.
    static func luma(of entry: [String: Any]) throws -> PetalLuma {
        let width = try integer(entry, "width")
        let height = try integer(entry, "height")
        guard let compressed = Data(base64Encoded: try string(entry, "luma_zlib_base64")) else {
            throw Failure.malformed("base64")
        }
        let pixels = try inflate(compressed, expectedLength: width * height)
        return try PetalLuma(width: width, height: height, pixels: pixels)
    }

    /// Deterministic payload bytes (xorshift32 top bytes, like the reference tests).
    static func payload(_ length: Int, seed: UInt32) -> [UInt8] {
        var rng = PetalXorshift32(seed: seed)
        return (0..<length).map { _ in rng.nextByte() }
    }

    /// Rebuilds a square image with `source(x, y)` giving the source pixel of
    /// each output pixel.
    static func transform(_ image: PetalLuma, _ source: (Int, Int) -> (Int, Int)) throws -> PetalLuma {
        var pixels = [UInt8](repeating: 0, count: image.pixels.count)
        for y in 0..<image.height {
            for x in 0..<image.width {
                let (sx, sy) = source(x, y)
                pixels[y * image.width + x] = image.pixels[sy * image.width + sx]
            }
        }
        return try PetalLuma(width: image.width, height: image.height, pixels: pixels)
    }

    /// Horizontally mirrored copy.
    static func mirror(_ image: PetalLuma) throws -> PetalLuma {
        try transform(image) { x, y in (image.width - 1 - x, y) }
    }

    /// The software render of `frame` as luma.
    static func render(
        _ encoder: PetalStreamEncoder,
        frame: UInt16,
        size: Int,
        supersample: Int
    ) throws -> PetalLuma {
        try PetalRenderer.render(
            encoder.cells(frame: frame),
            options: PetalRenderOptions(size: size, supersample: supersample)
        ).luma()
    }

    /// Feeds clean lanes of `frame` straight into an assembler.
    static func feed(
        _ assembler: inout PetalStreamAssembler,
        _ encoder: PetalStreamEncoder,
        frame: UInt16,
        lanes: [PetalLane]
    ) throws {
        let words = encoder.words(frame: frame)
        for lane in lanes {
            switch lane {
            case .p:
                let data = try PetalLane.p.decode(words.p)
                assembler.push(atoms: try XCTUnwrap(PetalStream.parseAtomLane(.p, data: data)))
            case .k:
                let data = try PetalLane.k.decode(words.k)
                assembler.push(atoms: try XCTUnwrap(PetalStream.parseAtomLane(.k, data: data)))
            case .d:
                let data = try PetalLane.d.decode(words.d)
                assembler.push(dLane: try XCTUnwrap(PetalStream.parseDLane(data)))
            }
        }
    }
}

/// Tiny deterministic generator for test vectors (the reference tests' LCG).
struct PetalTestLCG {
    var state: UInt64

    mutating func next() -> UInt32 {
        state = state &* 6_364_136_223_846_793_005 &+ 1_442_695_040_888_963_407
        return UInt32(truncatingIfNeeded: state >> 33)
    }

    mutating func byte() -> UInt8 { UInt8(truncatingIfNeeded: next()) }

    mutating func below(_ bound: Int) -> Int { Int(next()) % bound }
}
