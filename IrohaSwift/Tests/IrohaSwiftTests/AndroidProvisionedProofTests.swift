import XCTest
@testable import IrohaSwift

final class AndroidProvisionedProofTests: XCTestCase {

    func testLoadsFixtureAndNormalizesFields() throws {
        let proof = try AndroidProvisionedProof.load(from: fixtureURL())
        XCTAssertEqual(proof.manifestSchema, "offline_provisioning_v1")
        XCTAssertEqual(proof.manifestVersion, 1)
        XCTAssertEqual(proof.manifestIssuedAtMs, 1_730_314_876_000)
        XCTAssertEqual(proof.counter, 7)
        XCTAssertEqual(proof.deviceId, "retail-device-001")
        XCTAssertEqual(proof.challengeHashHex,
                       "E8A8D90BF72F280BBB4AB6D1F759521D29A08DA83CCFBB3E2EE0EDE22606FB9B")
        XCTAssertEqual(proof.challengeHashLiteral,
                       "hash:E8A8D90BF72F280BBB4AB6D1F759521D29A08DA83CCFBB3E2EE0EDE22606FB9B#E0A4")
        XCTAssertEqual(proof.inspectorSignatureHex,
                       "FEFCA2274314E692ED7244C613DA012E5D8AF7BF4B2AB236D1FF91A90F300935EC96EF0090B324C79A3F68AD513E17567242C42D50DC2377494A8071FDA49F0E")
        XCTAssertNotNil(proof.challengeHashData)
        XCTAssertNotNil(proof.inspectorSignatureData)
    }

    func testRoundTripEncodingMatchesOriginalPayload() throws {
        let proof = try AndroidProvisionedProof.load(from: fixtureURL())
        let encoded = try proof.encodedData(prettyPrinted: false)
        let decoded = try JSONDecoder().decode(AndroidProvisionedProof.self, from: encoded)
        XCTAssertEqual(decoded, proof)
    }

    func testNoritoEncodingMatchesFixture() throws {
        let proof = try AndroidProvisionedProof.load(from: fixtureURL())
        let encoded = try proof.noritoEncoded()
        let expected = try Data(contentsOf: fixtureNoritoURL())
        XCTAssertEqual(encoded, expected)
    }

    func testRejectsInvalidHashLiteral() {
        XCTAssertThrowsError(
            try AndroidProvisionedProof(manifestSchema: "offline_provisioning_v1",
                                        manifestVersion: nil,
                                        manifestIssuedAtMs: 0,
                                        challengeHashLiteral: "deadbeef",
                                        counter: 1,
                                        deviceManifest: ["android.provisioned.device_id": .string("dev")],
                                        inspectorSignatureHex: String(repeating: "A", count: 128))
        ) { error in
            guard case AndroidProvisionedProofError.invalidHashLiteral = error else {
                return XCTFail("Expected invalidHashLiteral error")
            }
        }
    }

    // MARK: - Helpers

    private func fixtureURL() -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_provision/kiosk-demo/proof.json")
    }

    private func fixtureNoritoURL() -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_provision/kiosk-demo/proof.norito")
    }
}
