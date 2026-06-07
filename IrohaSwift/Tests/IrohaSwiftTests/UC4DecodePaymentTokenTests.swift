import Foundation
import XCTest
@testable import IrohaSwift

final class UC4DecodePaymentTokenTests: XCTestCase {
    private static let paymentTokenEnvelopeTypeName =
        "iroha_data_model::offline::model::OfflineNotePaymentTokenEnvelope"

    func testDecodeUC4PaymentTokenRejectsMalformedCompactPayload() throws {
        let malformedPayload = Data("ios-compact-v1:not-a-norito-payment-token".utf8)
        let malformedText = OfflineNotePaymentTokenCodec.textPrefix + Self.base64Url(malformedPayload)

        XCTAssertThrowsError(try OfflineNotePaymentTokenCodec.decodeText(malformedText)) { error in
            XCTAssertEqual(error as? OfflineNotePaymentTokenCodecError, .invalidField("payload"))
        }

        if ProcessInfo.processInfo.environment["UC4_TOKEN_PATH"] != nil {
            let tokenText = try Self.loadUC4TokenText()
            XCTAssertThrowsError(try OfflineNotePaymentTokenCodec.decodeText(tokenText))
        }
    }

    func testDecodeUC4PaymentTokenRejectsWrongCompactMarkerThroughCanonicalDecoder() throws {
        let markerPayload = Data("ios-compact-v1:payment-token".utf8)
        XCTAssertThrowsError(try OfflineNotePaymentTokenCodec.decodeNorito(markerPayload)) { error in
            XCTAssertEqual(error as? OfflineNotePaymentTokenCodecError, .invalidField("payload"))
        }

        let missingCompactFlag = noritoEncode(
            typeName: Self.paymentTokenEnvelopeTypeName,
            payload: Data(),
            flags: 0
        )
        XCTAssertThrowsError(try OfflineNotePaymentTokenCodec.decodeNorito(missingCompactFlag)) { error in
            XCTAssertEqual(error as? OfflineNotePaymentTokenCodecError, .invalidField("layout"))
        }
    }

    private static func loadUC4TokenText() throws -> String {
        guard let path = ProcessInfo.processInfo.environment["UC4_TOKEN_PATH"], !path.isEmpty else {
            throw XCTSkip("Set UC4_TOKEN_PATH to a UC4 compact payment-token fixture.")
        }
        return try String(contentsOfFile: path, encoding: .utf8)
            .trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private static func base64Url(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}
