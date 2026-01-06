import XCTest
@testable import IrohaSwift

final class ConnectErrorTests: XCTestCase {
    func testClientErrorMapping() {
        let closed = ConnectClient.ClientError.closed.asConnectError()
        XCTAssertEqual(closed.category, .transport)
        XCTAssertEqual(closed.code, "client.closed")

        let unknownPayload = ConnectClient.ClientError.unknownPayload.asConnectError()
        XCTAssertEqual(unknownPayload.category, .codec)
        XCTAssertEqual(unknownPayload.code, "client.unknown_payload")
    }

    func testSessionErrorMapping() {
        let missingKeys = ConnectSessionError.missingDecryptionKeys.asConnectError()
        XCTAssertEqual(missingKeys.category, .internalError)
        XCTAssertEqual(missingKeys.code, "session.missing_direction_keys")
    }

    func testEnvelopeErrorMapping() {
        let invalidPayload = ConnectEnvelopeError.invalidPayload.asConnectError()
        XCTAssertEqual(invalidPayload.category, .codec)
        XCTAssertEqual(invalidPayload.code, "envelope.invalid_payload")

        let unknownKind = ConnectEnvelopeError.unknownPayloadKind("Foo").asConnectError()
        XCTAssertEqual(unknownKind.category, .codec)
        XCTAssertEqual(unknownKind.code, "envelope.unknown_payload_kind")
        XCTAssertTrue(unknownKind.message.contains("Foo"))
    }

    func testCryptoErrorMapping() {
        let invalidPrivateKey = ConnectCryptoError.invalidPrivateKeyLength(expected: 32, actual: 10).asConnectError()
        XCTAssertEqual(invalidPrivateKey.category, .internalError)
        XCTAssertEqual(invalidPrivateKey.code, "crypto.invalid_private_key_length")
    }

    func testCodecErrorMapping() {
        let unavailable = ConnectCodecError.bridgeUnavailable.asConnectError()
        XCTAssertEqual(unavailable.category, .codec)
        XCTAssertEqual(unavailable.code, "codec.bridge_unavailable")

        let encodeFailed = ConnectCodecError.encodeFailed.asConnectError()
        XCTAssertEqual(encodeFailed.category, .codec)
        XCTAssertEqual(encodeFailed.code, "codec.encode_failed")
    }

    func testQueueErrorMapping() {
        let overflow = ConnectQueueError.overflow(limit: 10).asConnectError()
        XCTAssertEqual(overflow.category, .queueOverflow)
        XCTAssertEqual(overflow.code, "queue.overflow")
        XCTAssertTrue(overflow.message.contains("10"))

        let invalidCount = ConnectQueueError.invalidCount(0).asConnectError()
        XCTAssertEqual(invalidCount.category, .internalError)
        XCTAssertEqual(invalidCount.code, "queue.invalid_count")
        XCTAssertTrue(invalidCount.message.contains("0"))
    }

    func testURLErrorMapping() {
        let error = URLError(.timedOut).asConnectError()
        XCTAssertEqual(error.category, .timeout)
        XCTAssertEqual(error.code, "network.timeout")

        let tlsError = URLError(.secureConnectionFailed).asConnectError()
        XCTAssertEqual(tlsError.category, .authorization)
        XCTAssertEqual(tlsError.code, "network.tls_failure")
    }

    func testDecodingErrorMapping() {
        let context = DecodingError.Context(codingPath: [], debugDescription: "boom")
        let error = DecodingError.dataCorrupted(context).asConnectError()
        XCTAssertEqual(error.category, .codec)
        XCTAssertEqual(error.code, "codec.decoding_failed")
        XCTAssertTrue(error.message.contains("boom"))
    }

    func testDefaultFallbackMapping() {
        struct SampleError: Error {}
        let fallback = SampleError().asConnectError()
        XCTAssertEqual(fallback.category, .internalError)
        XCTAssertEqual(fallback.code, "unknown_error")
    }

    func testTelemetryAttributesIncludeCategoryAndCode() {
        let attributes = ConnectClient.ClientError.closed.asConnectError().telemetryAttributes(fatal: true, httpStatus: 400)
        XCTAssertEqual(attributes["category"], ConnectErrorCategory.transport.rawValue)
        XCTAssertEqual(attributes["code"], "client.closed")
        XCTAssertEqual(attributes["fatal"], "true")
        XCTAssertEqual(attributes["http_status"], "400")
    }
}
