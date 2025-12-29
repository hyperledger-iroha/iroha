import XCTest
@testable import IrohaSwift

final class ComputeSimulatorTests: XCTestCase {
    private func fixtureURL(_ relative: String) -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift package root
            .deletingLastPathComponent() // workspace root
            .appendingPathComponent(relative)
    }

    private func fixtures() throws -> (manifest: [String: Any], call: [String: Any], payload: Data) {
        try ComputeSimulator.loadFixtures(
            manifestPath: fixtureURL("fixtures/compute/manifest_compute_payments.json"),
            callPath: fixtureURL("fixtures/compute/call_compute_payments.json"),
            payloadPath: fixtureURL("fixtures/compute/payload_compute_payments.json")
        )
    }

    func testSimulateEchoFixture() throws {
        let fixtures = try fixtures()
        let result = try ComputeSimulator.simulate(
            manifest: fixtures.manifest,
            call: fixtures.call,
            payload: fixtures.payload
        )
        XCTAssertEqual(result.responseBase64, fixtures.payload.base64EncodedString())
        if let expected = (fixtures.call["request"] as? [String: Any])?["payload_hash"] as? String {
            XCTAssertEqual(result.payloadHashLiteral, expected)
        } else {
            XCTFail("missing payload_hash in call fixture")
        }
    }

    func testPayloadHashMismatchFails() throws {
        let fixtures = try fixtures()
        let badPayload = Data(repeating: 0xFF, count: 4)
        XCTAssertThrowsError(
            try ComputeSimulator.simulate(
                manifest: fixtures.manifest,
                call: fixtures.call,
                payload: badPayload
            )
        ) { error in
            guard let simError = error as? ComputeSimulatorError else {
                return XCTFail("expected ComputeSimulatorError, got \(error)")
            }
            guard case let .invalidPayloadHash(expected, actual) = simError else {
                return XCTFail("expected invalidPayloadHash, got \(simError)")
            }
            XCTAssertNotEqual(expected, actual)
        }
    }
}
