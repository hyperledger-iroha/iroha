import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class ZkX509SigningPreparationV1Tests: XCTestCase {
    override func tearDown() {
        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(nil)
        super.tearDown()
    }

    func testInputBoundsRefuseBeforeUnavailableNativeDispatch() {
        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(false)
        XCTAssertEqual(ZkX509SigningPreparationV1.maximumStatementBytes, 262_144)
        XCTAssertEqual(ZkX509SigningPreparationV1.maximumMessageBytes, 262_144)
        XCTAssertThrowsError(
            try ZkX509SigningPreparationV1.prepareOwnershipSigningV1(statementNorito: Data())
        ) {
            XCTAssertEqual($0 as? ZkX509SigningPreparationErrorV1, .emptyStatement)
        }
        XCTAssertThrowsError(
            try ZkX509SigningPreparationV1.prepareOwnershipSigningV1(
                statementNorito: Data(repeating: 0, count: 262_145)
            )
        ) {
            XCTAssertEqual(
                $0 as? ZkX509SigningPreparationErrorV1,
                .statementTooLarge(maximum: 262_144, actual: 262_145)
            )
        }
        XCTAssertFalse(ZkX509SigningPreparationV1.isNativeAvailable)
        for input in [Data([1]), Data(repeating: 0, count: 262_144)] {
            XCTAssertThrowsError(
                try ZkX509SigningPreparationV1.prepareOwnershipSigningV1(statementNorito: input)
            ) {
                XCTAssertEqual($0 as? ZkX509SigningPreparationErrorV1, .nativeUnavailable)
            }
        }
    }

    func testNativeResultRequiresBoundedMessageAndMatchingDigest() throws {
        // Output-contract controls only; these bytes are not an X.509 statement.
        let message = Data("public output integrity fixture".utf8)
        let digest = Data(SHA256.hash(data: message))
        let prepared = try ZkX509SigningPreparationV1.validateNativeResult(
            message: message, digest: digest
        )
        XCTAssertEqual(prepared.message, message)
        XCTAssertEqual(prepared.digest, digest)
        var substituted = digest
        substituted[0] ^= 1
        for (candidateMessage, candidateDigest) in [
            (Data(), digest),
            (Data(repeating: 1, count: 262_145), digest),
            (message, Data(digest.dropLast())),
            (message, digest + Data([0])),
            (message, substituted),
            (message + Data([0]), digest),
        ] {
            XCTAssertThrowsError(
                try ZkX509SigningPreparationV1.validateNativeResult(
                    message: candidateMessage, digest: candidateDigest
                )
            ) {
                XCTAssertEqual($0 as? ZkX509SigningPreparationErrorV1, .invalidNativeResult)
            }
        }
    }

    func testRustFixtureAdapterMatchesBothNativeOutputsExactly() throws {
        try requireNativeTestCapability(
            ZkX509SigningPreparationV1.isNativeAvailable,
            "Current native X.509 ownership signing preparation is required."
        )
        let statement = try x509Statement()
        let prepared = try ZkX509SigningPreparationV1.prepareOwnershipSigningV1(
            statementNorito: statement
        )
        let direct = try directNativePreparation(statement)
        XCTAssertEqual(prepared.message, direct.message)
        XCTAssertEqual(prepared.digest, direct.digest)
        XCTAssertEqual(prepared.digest, Data(SHA256.hash(data: prepared.message)))
        XCTAssertEqual(prepared.digest.count, 32)
        XCTAssertFalse(prepared.message.isEmpty)
        XCTAssertLessThanOrEqual(prepared.message.count, 262_144)
        XCTAssertEqual(
            try ZkX509SigningPreparationV1.prepareOwnershipSigningV1(statementNorito: statement),
            prepared
        )
    }

    func testNativeRejectsOtherProtocolsAndNoncanonicalStatementArchives() throws {
        try requireNativeTestCapability(
            ZkX509SigningPreparationV1.isNativeAvailable,
            "Current native X.509 ownership signing preparation is required."
        )
        let bundle = try fixtureBundle()
        for row in bundle.rows where row.protocolId != .irohaZkX509StarkP256V1 {
            XCTAssertThrowsError(
                try ZkX509SigningPreparationV1.prepareOwnershipSigningV1(
                    statementNorito: row.statementNorito
                )
            ) {
                XCTAssertEqual($0 as? ZkX509SigningPreparationErrorV1, .nativeRejected(-5))
            }
        }
        let row = try XCTUnwrap(bundle.rows.first { $0.protocolId == .irohaZkX509StarkP256V1 })
        let canonical = row.statementNorito
        var corrupted = canonical
        corrupted[0] ^= 0x80
        for invalid in [
            Data(canonical.dropFirst()), Data(canonical.dropLast()),
            canonical + Data([0]), corrupted, row.envelopeNorito,
            Data(repeating: 0, count: 262_144),
        ] {
            XCTAssertThrowsError(
                try ZkX509SigningPreparationV1.prepareOwnershipSigningV1(statementNorito: invalid)
            ) {
                XCTAssertEqual($0 as? ZkX509SigningPreparationErrorV1, .nativeRejected(-4))
            }
        }
    }

    func testExternalP256MessageAndPrehashSigningApplySha256ExactlyOnce() throws {
        try requireNativeTestCapability(
            ZkX509SigningPreparationV1.isNativeAvailable,
            "Current native X.509 ownership signing preparation is required."
        )
        let prepared = try ZkX509SigningPreparationV1.prepareOwnershipSigningV1(
            statementNorito: x509Statement()
        )
        // A test-owned external key demonstrates signing API semantics only.
        // No certificate ownership, low-s witness admission or proving is claimed.
        let key = try P256.Signing.PrivateKey(rawRepresentation: Data(repeating: 0x42, count: 32))
        let digest = SHA256.hash(data: prepared.message)
        XCTAssertEqual(Data(digest), prepared.digest)
        let messageSignature = try key.signature(for: prepared.message)
        let prehashSignature = try key.signature(for: digest)
        XCTAssertEqual(messageSignature.rawRepresentation.count, 64)
        XCTAssertEqual(prehashSignature.rawRepresentation.count, 64)
        XCTAssertTrue(key.publicKey.isValidSignature(messageSignature, for: digest))
        XCTAssertTrue(key.publicKey.isValidSignature(prehashSignature, for: prepared.message))
        let doubleHashed = try key.signature(for: prepared.digest)
        XCTAssertFalse(key.publicKey.isValidSignature(doubleHashed, for: prepared.message))
        var substituted = prepared.message
        substituted[0] ^= 1
        XCTAssertFalse(key.publicKey.isValidSignature(messageSignature, for: substituted))
    }

    private func x509Statement() throws -> Data {
        try XCTUnwrap(fixtureBundle().rows.first {
            $0.protocolId == .irohaZkX509StarkP256V1
        }).statementNorito
    }

    private func fixtureBundle() throws -> PrivacyExact12FixtureBundleV1 {
        var directory = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
        while directory.path != "/" {
            let fixture = directory.appendingPathComponent(
                "fixtures/privacy/exact12_typed_fixture_bundle_v1.norito.b64"
            )
            if FileManager.default.fileExists(atPath: fixture.path) {
                return try PrivacyExact12FixtureCodecV1.decodeCanonicalBase64File(
                    String(contentsOf: fixture, encoding: .utf8)
                )
            }
            directory.deleteLastPathComponent()
        }
        throw RequiredNativeTestCapabilityError.unavailable("Cannot locate the Rust Exact12 fixture.")
    }

    private func directNativePreparation(_ statement: Data) throws -> (message: Data, digest: Data) {
        #if canImport(Darwin)
        typealias Prepare = @convention(c) (
            UnsafePointer<UInt8>?, CUnsignedLong,
            UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?,
            UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?, UnsafeMutablePointer<CUnsignedLong>?
        ) -> Int32
        typealias Free = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
        let bridge = NoritoNativeBridge.shared
        let prepare = try XCTUnwrap(bridge.resolveNativeSymbol(
            "connect_norito_zk_x509_prepare_ownership_signing_v1", as: Prepare.self
        ))
        let free = try XCTUnwrap(bridge.resolveNativeSymbol("iroha_privacy_free_buffer", as: Free.self))
        var message: UnsafeMutablePointer<UInt8>?, digest: UnsafeMutablePointer<UInt8>?
        var messageLength: CUnsignedLong = 0, digestLength: CUnsignedLong = 0
        let status = statement.withUnsafeBytes { bytes in
            prepare(bytes.bindMemory(to: UInt8.self).baseAddress, CUnsignedLong(bytes.count),
                    &message, &messageLength, &digest, &digestLength)
        }
        defer {
            if let message { free(message) }
            if let digest { free(digest) }
        }
        XCTAssertEqual(status, 0)
        let messagePointer = try XCTUnwrap(message), digestPointer = try XCTUnwrap(digest)
        guard messageLength > 0, messageLength <= 262_144, digestLength == 32 else {
            throw ZkX509SigningPreparationErrorV1.invalidNativeResult
        }
        return (Data(bytes: messagePointer, count: Int(messageLength)),
                Data(bytes: digestPointer, count: Int(digestLength)))
        #else
        throw RequiredNativeTestCapabilityError.unavailable("Native bridge requires Darwin.")
        #endif
    }
}
