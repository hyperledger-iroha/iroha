import XCTest
@testable import IrohaSwift

final class AndroidProvisionedProofTests: XCTestCase {
    func testManifestVersionRejectsValuesOutsideUInt32() {
        for value in [-1, Int(UInt32.max) + 1] {
            XCTAssertThrowsError(try makeProof(manifestVersion: value)) { error in
                guard case AndroidProvisionedProofError.invalidManifestVersion = error else {
                    return XCTFail("unexpected error for manifest version \(value): \(error)")
                }
            }
        }
    }

    func testManifestVersionAcceptsUInt32MaximumForBothNoritoPayloads() throws {
        let proof = try makeProof(manifestVersion: Int(UInt32.max))

        XCTAssertFalse(try proof.noritoEncoded().isEmpty)
        XCTAssertFalse(try proof.manifestSigningBytes().isEmpty)
    }

    func testDecodedValidationErrorsIdentifyTheirSourceFields() throws {
        try assertDecodeFailure(codingKey: "manifest_schema") { payload in
            payload["manifest_schema"] = "   "
        }
        try assertDecodeFailure(codingKey: "manifest_version") { payload in
            payload["manifest_version"] = Int(UInt32.max) + 1
        }
        try assertDecodeFailure(codingKey: "device_manifest") { payload in
            payload["device_manifest"] = [:]
        }
        try assertDecodeFailure(codingKey: "challenge_hash") { payload in
            payload["challenge_hash"] = "invalid"
        }
        try assertDecodeFailure(codingKey: "inspector_signature") { payload in
            payload["inspector_signature"] = "invalid"
        }
    }

    private func makeProof(manifestVersion: Int?) throws -> AndroidProvisionedProof {
        try AndroidProvisionedProof(
            manifestSchema: "android.provisioned.v1",
            manifestVersion: manifestVersion,
            manifestIssuedAtMs: 1,
            challengeHashLiteral: String(repeating: "00", count: 31) + "01",
            counter: 1,
            deviceManifest: ["android.provisioned.device_id": .string("device-1")],
            inspectorSignatureHex: String(repeating: "00", count: 64)
        )
    }

    private func assertDecodeFailure(
        codingKey: String,
        mutate: (inout [String: Any]) -> Void,
        file: StaticString = #filePath,
        line: UInt = #line
    ) throws {
        var payload: [String: Any] = [
            "manifest_schema": "android.provisioned.v1",
            "manifest_version": 1,
            "manifest_issued_at_ms": 1,
            "challenge_hash": String(repeating: "00", count: 31) + "01",
            "counter": 1,
            "device_manifest": ["android.provisioned.device_id": "device-1"],
            "inspector_signature": String(repeating: "00", count: 64),
        ]
        mutate(&payload)
        let data = try JSONSerialization.data(withJSONObject: payload)

        XCTAssertThrowsError(
            try JSONDecoder().decode(AndroidProvisionedProof.self, from: data),
            file: file,
            line: line
        ) { error in
            guard case DecodingError.dataCorrupted(let context) = error else {
                return XCTFail("unexpected decoding error: \(error)", file: file, line: line)
            }
            XCTAssertEqual(context.codingPath.last?.stringValue, codingKey, file: file, line: line)
        }
    }
}
