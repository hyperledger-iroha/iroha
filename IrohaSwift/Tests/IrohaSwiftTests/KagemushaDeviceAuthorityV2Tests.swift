import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaDeviceAuthorityV2Tests: XCTestCase {
    private static let order = Data([
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84,
        0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
    ])
    private static let halfOrder = Data([
        0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00,
        0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42,
        0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa8,
    ])
    private static let generator = Data([
        0x04,
        0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47,
        0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40, 0xf2,
        0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0,
        0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98, 0xc2, 0x96,
        0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b,
        0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16,
        0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce,
        0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5,
    ])

    func testDeviceKeyAcceptsOnlyCanonicalUncompressedP256() throws {
        let key = try KagemushaDevicePublicKeyV2(sec1Bytes: Self.generator)
        XCTAssertEqual(key.sec1Bytes, Self.generator)

        var wrongPrefix = Self.generator
        wrongPrefix[0] = 0x06
        var offCurve = Self.generator
        offCurve.replaceSubrange(33..<65, with: Data(repeating: 0, count: 32))
        var compressed = Data([0x03]) // The generator's y coordinate is odd.
        compressed.append(Self.generator[1..<33])
        for malformed in [
            Data(),
            Data(Self.generator.prefix(64)),
            Self.generator + Data([0]),
            compressed,
            Data(repeating: 0, count: 65),
            wrongPrefix,
            offCurve,
        ] {
            XCTAssertThrowsError(
                try KagemushaDevicePublicKeyV2(sec1Bytes: malformed),
                "accepted malformed key of length \(malformed.count)"
            )
        }
    }

    func testRawSignatureRejectsWidthsDERScalarsAndHighS() throws {
        let one = scalar(1)
        var highS = Self.halfOrder
        highS[31] += 1
        let minimalDER = Data([0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01])
        var derPaddedAsRaw = Data(repeating: 0, count: 64)
        derPaddedAsRaw.replaceSubrange(0..<minimalDER.count, with: minimalDER)

        for malformed in [
            Data(repeating: 0, count: 63),
            Data(repeating: 0, count: 65),
            scalarPair(r: Data(repeating: 0, count: 32), s: one),
            scalarPair(r: one, s: Data(repeating: 0, count: 32)),
            scalarPair(r: Self.order, s: one),
            scalarPair(r: one, s: Self.order),
            scalarPair(r: one, s: highS),
            derPaddedAsRaw,
        ] {
            XCTAssertThrowsError(
                try KagemushaDeviceSignatureV2(rawBytes: malformed),
                "accepted malformed raw signature"
            )
        }
    }

    func testStrictDERConversionRoundTripsAndCanonicalizesHighS() throws {
        let privateKey = P256.Signing.PrivateKey()
        let message = Data("fixed kagemusha p256 profile".utf8)
        let generated = try privateKey.signature(for: message)
        let canonical = try KagemushaDeviceSignatureV2(
            derBytes: generated.derRepresentation
        )
        let publicKey = try KagemushaDevicePublicKeyV2(
            sec1Bytes: privateKey.publicKey.x963Representation
        )
        XCTAssertTrue(publicKey.isValidSignature(canonical, for: message))
        XCTAssertFalse(
            publicKey.isValidSignature(
                canonical,
                for: Data("substituted message".utf8)
            )
        )

        let strictDER = try canonical.strictDERBytes()
        XCTAssertEqual(
            try KagemushaDeviceSignatureV2(derBytes: strictDER),
            canonical
        )

        let lowS = Data(canonical.rawBytes.suffix(32))
        let highS = subtract(lowS, from: Self.order)
        XCTAssertTrue(Self.halfOrder.lexicographicallyPrecedes(highS))
        let highRaw = scalarPair(
            r: Data(canonical.rawBytes.prefix(32)),
            s: highS
        )
        XCTAssertThrowsError(try KagemushaDeviceSignatureV2(rawBytes: highRaw))
        let highDER = try P256.Signing.ECDSASignature(
            rawRepresentation: highRaw
        ).derRepresentation
        XCTAssertEqual(
            try KagemushaDeviceSignatureV2(derBytes: highDER),
            canonical
        )
    }

    func testDERParserRejectsTrailingLongFormAndNonminimalIntegers() throws {
        let strict = try KagemushaDeviceSignatureV2(
            rawBytes: scalarPair(r: scalar(1), s: scalar(1))
        ).strictDERBytes()
        XCTAssertEqual(strict, Data([0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x01]))

        let trailing = strict + Data([0])
        let longFormLength = Data([0x30, 0x81, 0x06]) + strict.dropFirst(2)
        let nonminimalInteger = Data([
            0x30, 0x07,
            0x02, 0x02, 0x00, 0x01,
            0x02, 0x01, 0x01,
        ])
        let indefiniteLength = Data([
            0x30, 0x80,
            0x02, 0x01, 0x01,
            0x02, 0x01, 0x01,
            0x00, 0x00,
        ])
        for malformed in [trailing, longFormLength, nonminimalInteger, indefiniteLength] {
            XCTAssertThrowsError(
                try KagemushaDeviceSignatureV2(derBytes: malformed),
                "accepted non-canonical DER"
            )
        }
    }

    private func scalar(_ value: UInt8) -> Data {
        var result = Data(repeating: 0, count: 32)
        result[31] = value
        return result
    }

    private func scalarPair(r: Data, s: Data) -> Data {
        r + s
    }

    private func subtract(_ value: Data, from minuend: Data) -> Data {
        let lhs = [UInt8](minuend)
        let rhs = [UInt8](value)
        var result = [UInt8](repeating: 0, count: 32)
        var borrow = 0
        for index in stride(from: 31, through: 0, by: -1) {
            var difference = Int(lhs[index]) - Int(rhs[index]) - borrow
            if difference < 0 {
                difference += 256
                borrow = 1
            } else {
                borrow = 0
            }
            result[index] = UInt8(difference)
        }
        return Data(result)
    }
}
