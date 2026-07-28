import Foundation
import XCTest
@testable import IrohaSwift

final class PrivacyNativeBridgeTests: XCTestCase {
    private let expected = [
        "zk-ace-pq-authorization-v0",
        "anonymous-pgc-k-out-of-n-v1",
        "verange-transparent-range-v1",
        "iroha-zk-ams-v1",
        "vega-existing-credential-zk-v0",
        "iroha-zk-x509-stark-p256-v0",
        "iroha-jindo-polynomial-commitment-v0",
        "iroha-bootle-lantern-anoncred-v1",
        "orchard-halo2-actions-v1",
        "monero-fcmp-plus-plus-v1",
        "iroha-ivm-private-note-stark-v1",
        "pq-masp-stark-v0",
    ]

    func testExactClosedRegistryIsStable() throws {
        XCTAssertEqual(PrivacyNativeBridge.requiredBridgeABIVersion, 21)
        XCTAssertEqual(PrivacyNativeBridge.protocolsV1.map(\.rawValue), expected)
        XCTAssertEqual(PrivacyNativeBridge.protocolsV1.count, 12)
        for (index, label) in expected.enumerated() {
            XCTAssertEqual(
                try PrivacyProtocolIdV1(canonicalLabel: label),
                PrivacyNativeBridge.protocolsV1[index]
            )
        }
    }

    func testAliasesAndNonCanonicalSpellingsAreRejected() {
        for rejected in [
            "jindo-lattice-pcs-zk-v0",
            "sis-hints-anoncred-pq-v0",
            "silent-threshold-anoncred-v0",
            "zk-ams-recursive-admission-v0",
            "iroha-zk-ams-v1 ",
            "Iroha-Zk-Ams-V1",
            "",
            "unknown-privacy-protocol-v1",
        ] {
            XCTAssertThrowsError(try PrivacyProtocolIdV1(canonicalLabel: rejected)) {
                XCTAssertEqual($0 as? PrivacyCapabilityBridgeError, .unknownProtocol)
            }
        }
    }

    func testCapabilityArchiveValidationFailsClosed() {
        XCTAssertThrowsError(
            try PrivacyNativeBridge.requireCapabilitiesArchiveV1(Data(repeating: 0, count: 39))
        )

        var badMagic = capabilityArchive()
        badMagic[0] = 0x58
        XCTAssertThrowsError(try PrivacyNativeBridge.requireCapabilitiesArchiveV1(badMagic))

        var badSchema = capabilityArchive()
        badSchema[13] = 0x51
        XCTAssertThrowsError(try PrivacyNativeBridge.requireCapabilitiesArchiveV1(badSchema))
    }

    func testCapabilityArchiveReturnsIndependentData() throws {
        var archive = capabilityArchive()
        let accepted = try PrivacyNativeBridge.requireCapabilitiesArchiveV1(archive)
        archive[0] = 0x58
        XCTAssertEqual(accepted[0], 0x4E)
    }

    private func capabilityArchive() -> Data {
        var archive = Data(repeating: 0, count: 41)
        archive[0] = 0x4E
        archive[1] = 0x52
        archive[2] = 0x54
        archive[3] = 0x30
        for index in 6..<22 {
            archive[index] = 0x50
        }
        archive[23] = 0x01
        archive[40] = 0x01
        var crc = crc64(Data([archive[40]]))
        for index in 31..<39 {
            archive[index] = UInt8(truncatingIfNeeded: crc)
            crc >>= 8
        }
        return archive
    }

    private func crc64(_ bytes: Data) -> UInt64 {
        var crc = UInt64.max
        for byte in bytes {
            crc ^= UInt64(byte)
            for _ in 0..<8 {
                crc = (crc & 1) != 0
                    ? (crc >> 1) ^ 0xC96C_5795_D787_0F42
                    : crc >> 1
            }
        }
        return crc ^ UInt64.max
    }
}
