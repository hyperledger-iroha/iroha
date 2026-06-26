import Foundation
import XCTest
@testable import IrohaSwift

final class OfflineIssuerPublicKeyTests: XCTestCase {
    func testAcceptsRawEd25519PublicKeysInBase64AndBase64URL() throws {
        let raw = Data((0..<OfflineIssuerPublicKey.rawEd25519ByteCount).map(UInt8.init))
        let paddedBase64 = raw.base64EncodedString()
        let unpaddedBase64 = paddedBase64.replacingOccurrences(of: "=", with: "")
        let paddedBase64URL = paddedBase64
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
        let unpaddedBase64URL = paddedBase64URL
            .replacingOccurrences(of: "=", with: "")

        let base64 = try OfflineIssuerPublicKey(unpaddedBase64)
        XCTAssertEqual(base64.rawRepresentation, raw)
        XCTAssertEqual(base64.encoded, unpaddedBase64)
        XCTAssertTrue(OfflineIssuerPublicKey.isValid(unpaddedBase64))
        XCTAssertNil(OfflineIssuerPublicKey.sanitized(" \n\(unpaddedBase64)\t "))

        let unpadded = try OfflineIssuerPublicKey(unpaddedBase64URL)
        XCTAssertEqual(unpadded.rawRepresentation, raw)
        XCTAssertEqual(unpadded.encoded, unpaddedBase64URL)
        XCTAssertTrue(OfflineIssuerPublicKey.isValid(unpaddedBase64URL))
        XCTAssertEqual(OfflineIssuerPublicKey.sanitized(unpaddedBase64URL), unpaddedBase64URL)
    }

    func testRejectsPaddedBase64BeforeDecoding() {
        let raw = Data((0..<OfflineIssuerPublicKey.rawEd25519ByteCount).map(UInt8.init))
        let paddedBase64 = raw.base64EncodedString()
        let paddedBase64URL = paddedBase64
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")

        XCTAssertThrowsError(try OfflineIssuerPublicKey(paddedBase64)) { error in
            XCTAssertEqual(error as? OfflineIssuerPublicKeyError, .invalidBase64)
        }
        XCTAssertThrowsError(try OfflineIssuerPublicKey(paddedBase64URL)) { error in
            XCTAssertEqual(error as? OfflineIssuerPublicKeyError, .invalidBase64)
        }
        XCTAssertFalse(OfflineIssuerPublicKey.isValid(paddedBase64))
        XCTAssertFalse(OfflineIssuerPublicKey.isValid(paddedBase64URL))
        XCTAssertNil(OfflineIssuerPublicKey.sanitized(paddedBase64))
        XCTAssertNil(OfflineIssuerPublicKey.sanitized(paddedBase64URL))
    }

    func testRejectsMissingMalformedAndWrongLengthKeys() {
        let wrongLengthCases = [
            Data(repeating: 0xAB, count: 31).base64EncodedString().replacingOccurrences(of: "=", with: ""): 31,
            Data(repeating: 0xCD, count: 33).base64EncodedString().replacingOccurrences(of: "=", with: ""): 33,
            Data(repeating: 0xEF, count: 44).base64EncodedString().replacingOccurrences(of: "=", with: ""): 44
        ]

        XCTAssertThrowsError(try OfflineIssuerPublicKey("")) { error in
            XCTAssertEqual(error as? OfflineIssuerPublicKeyError, .missing)
        }
        XCTAssertThrowsError(try OfflineIssuerPublicKey(" \n\t ")) { error in
            XCTAssertEqual(error as? OfflineIssuerPublicKeyError, .missing)
        }
        XCTAssertThrowsError(try OfflineIssuerPublicKey(" \(Data(repeating: 0xAA, count: 32).base64EncodedString())")) { error in
            XCTAssertEqual(error as? OfflineIssuerPublicKeyError, .surroundingWhitespace)
        }
        XCTAssertThrowsError(try OfflineIssuerPublicKey("\(Data(repeating: 0xAA, count: 32).base64EncodedString()) ")) { error in
            XCTAssertEqual(error as? OfflineIssuerPublicKeyError, .surroundingWhitespace)
        }
        XCTAssertThrowsError(try OfflineIssuerPublicKey("not base64")) { error in
            XCTAssertEqual(error as? OfflineIssuerPublicKeyError, .invalidBase64)
        }
        XCTAssertThrowsError(try OfflineIssuerPublicKey("!!!!")) { error in
            XCTAssertEqual(error as? OfflineIssuerPublicKeyError, .invalidBase64)
        }

        for (encoded, actualLength) in wrongLengthCases {
            XCTAssertThrowsError(try OfflineIssuerPublicKey(encoded)) { error in
                XCTAssertEqual(
                    error as? OfflineIssuerPublicKeyError,
                    .invalidLength(expected: OfflineIssuerPublicKey.rawEd25519ByteCount, actual: actualLength)
                )
            }
            XCTAssertFalse(OfflineIssuerPublicKey.isValid(encoded))
            XCTAssertNil(OfflineIssuerPublicKey.sanitized(encoded))
        }
    }
}
