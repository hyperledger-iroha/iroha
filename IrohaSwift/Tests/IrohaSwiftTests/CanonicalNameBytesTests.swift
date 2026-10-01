import Foundation
import XCTest
@testable import IrohaSwift

/// Canonical wire names must compare bytes, since Swift String equality normalizes Unicode.
final class CanonicalNameBytesTests: XCTestCase {
    private let composed = "caf\u{e9}"
    private let decomposed = "cafe\u{301}"

    func testMetadataEncoderRejectsAlternateNFCBytes() throws {
        XCTAssertEqual(composed, decomposed)
        XCTAssertNotEqual(Data(composed.utf8), Data(decomposed.utf8))
        XCTAssertNoThrow(try CanonicalNorito.encodeCompactMetadata([composed: .bool(true)]))
        XCTAssertThrowsError(try CanonicalNorito.encodeCompactMetadata([decomposed: .bool(true)]))
    }

    func testDraftDecoderRejectsNoncanonicalMetadataNames() throws {
        let authority = try Keypair(privateKeyBytes: Data(repeating: 0x41, count: 32))
            .accountId(networkPrefix: AccountId.defaultNetworkPrefix)
        var instructions = CompactNoritoWriter()
        instructions.writeUInt64LE(0)
        var executable = CompactNoritoWriter()
        executable.writeUInt32LE(0)
        executable.writeField(instructions.data)
        func decode(_ key: String) throws -> ToriiCanonicalTransactionDraft {
            let payload = try CanonicalUnsignedTransactionTestSupport.transactionPayload(
                networkId: TestNetworkIds.canonical, authority: authority, creationTimeMs: 123,
                executable: executable.data, timeToLiveMs: nil,
                feePayment: .authority(chargeLimits: [], gasLimit: nil),
                metadata: [key: .bool(true)]
            )
            return try ToriiCanonicalTransactionDraft.decode(
                transactionPayloadB64: payload.base64EncodedString(),
                signingMessageB64: IrohaHash.hash(payload).base64EncodedString(),
                context: "canonical metadata name test"
            )
        }
        for valid in [composed, String(repeating: "a", count: 255)] {
            XCTAssertNoThrow(try decode(valid))
        }
        for invalid in [decomposed, String(repeating: "a", count: 256), "a\u{01}", "a\u{202e}"] {
            XCTAssertThrowsError(try decode(invalid), invalid.debugDescription)
        }
    }

    func testKaigiNamesRejectAlternateNFCBytes() throws {
        XCTAssertNoThrow(try KaigiIdV1(domainID: "meetings.universal", callName: composed))
        XCTAssertThrowsError(try KaigiIdV1(domainID: "meetings.universal", callName: decomposed))
    }

    func testMusubiNamesRejectAlternateNFCBytes() throws {
        XCTAssertNoThrow(try musubiRequireName(composed, field: "name"))
        XCTAssertThrowsError(try musubiRequireName(decomposed, field: "name"))
    }

    func testManifestScopeRejectsAlternateNFCBytes() throws {
        for field in ["program", "method"] {
            let canonical = try JSONEncoder().encode([field: composed])
            XCTAssertNoThrow(try JSONDecoder().decode(ToriiUaidManifestScope.self, from: canonical))
            let alternate = try JSONEncoder().encode([field: decomposed])
            XCTAssertThrowsError(try JSONDecoder().decode(ToriiUaidManifestScope.self, from: alternate))
        }
    }
}
