import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class NativeBridgeLoaderTests: XCTestCase {
    func testExpectedBridgeAbiVersionIsTwentyThreeForPackagedArtifacts() {
        XCTAssertEqual(NoritoBridgeLoader.expectedBridgeAbiVersion(for: "macos-arm64_x86_64"), 23)
        XCTAssertEqual(NoritoBridgeLoader.expectedBridgeAbiVersion(for: "ios-arm64"), 23)
        XCTAssertEqual(NoritoBridgeLoader.expectedBridgeAbiVersion(for: "ios-arm64_x86_64-simulator"), 23)
        XCTAssertFalse(NoritoBridgeLoader.isSupportedBridgeAbiVersion(20, for: "macos-arm64_x86_64"))
        XCTAssertFalse(NoritoBridgeLoader.isSupportedBridgeAbiVersion(18, for: "macos-arm64_x86_64"))
        XCTAssertFalse(NoritoBridgeLoader.isSupportedBridgeAbiVersion(19, for: "macos-arm64_x86_64"))
        XCTAssertFalse(NoritoBridgeLoader.isSupportedBridgeAbiVersion(nil, for: "macos-arm64_x86_64"))
        XCTAssertFalse(NoritoBridgeLoader.isSupportedBridgeAbiVersion(21, for: "macos-arm64_x86_64"))
        XCTAssertFalse(NoritoBridgeLoader.isSupportedBridgeAbiVersion(22, for: "macos-arm64_x86_64"))
        XCTAssertTrue(NoritoBridgeLoader.isSupportedBridgeAbiVersion(23, for: "macos-arm64_x86_64"))
        XCTAssertTrue(KagemushaRecursiveSpend.requiredProtocolSymbols.contains(
            "connect_norito_kagemusha_recursive_spend_artifact_begin_v4"
        ))
        XCTAssertTrue(KagemushaRecursiveSpend.requiredProtocolSymbols.contains(
            "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4"
        ))
        XCTAssertTrue(KagemushaRecursiveSpend.requiredProtocolSymbols.contains(
            "connect_norito_kagemusha_offline_operation_status_validate_v2"
        ))
        XCTAssertEqual(KagemushaRecursiveSpend.nativeContractRevision, 1)
        XCTAssertTrue(KagemushaRecursiveSpend.requiredProtocolSymbols.contains(
            "connect_norito_kagemusha_native_contract_revision"
        ))
        XCTAssertEqual(
            NoritoBridgeLoader.parliamentTimedOvnWalletRequiredSymbols,
            [
                "connect_norito_parliament_timed_ovn_verify_casting_proof_page_v1",
                "connect_norito_parliament_timed_ovn_verify_casting_proof_v1",
                "connect_norito_parliament_timed_ovn_registration_from_proof_v1",
                "connect_norito_parliament_timed_ovn_ballot_from_proof_v1"
            ]
        )
    }

    func testParliamentTimedOvnWalletUsesClosedChoiceAndErrorSets() {
        XCTAssertEqual(ParliamentTimedOvnBallotChoiceV1.aye.rawValue, 0)
        XCTAssertEqual(ParliamentTimedOvnBallotChoiceV1.nay.rawValue, 1)
        XCTAssertEqual(ParliamentTimedOvnBallotChoiceV1.abstain.rawValue, 2)
        XCTAssertEqual(NativeBridgeError.fromStatus(-505), .parliamentTimedOvnWallet)
        XCTAssertEqual(
            NoritoNativeBridge.parliamentTimedOvnCastingProofPageVerificationBytes,
            41
        )
        let verification = try? ParliamentTimedOvnCastingProofPageVerificationV1(
            evaluatedBlockHeight: 70,
            evaluatedContextID: Data(repeating: 0x22, count: 32),
            moreAvailable: true
        )
        XCTAssertEqual(verification?.evaluatedBlockHeight, 70)
        XCTAssertEqual(verification?.evaluatedContextID, Data(repeating: 0x22, count: 32))
        XCTAssertEqual(verification?.moreAvailable, true)
        XCTAssertThrowsError(
            try ParliamentTimedOvnCastingProofPageVerificationV1(
                evaluatedBlockHeight: 0,
                evaluatedContextID: Data(repeating: 0x22, count: 32),
                moreAvailable: false
            )
        )
        var encoded = Data(repeating: 0, count: 41)
        encoded[7] = 70
        encoded.replaceSubrange(8..<40, with: Data(repeating: 0x22, count: 32))
        encoded[40] = 1
        XCTAssertEqual(
            try? NoritoNativeBridge.decodeParliamentTimedOvnCastingProofPageVerificationV1(
                encoded
            ),
            verification
        )
        XCTAssertThrowsError(
            try NoritoNativeBridge.decodeParliamentTimedOvnCastingProofPageVerificationV1(
                Data(encoded.dropLast())
            )
        )
        encoded[40] = 2
        XCTAssertThrowsError(
            try NoritoNativeBridge.decodeParliamentTimedOvnCastingProofPageVerificationV1(
                encoded
            )
        )
    }

    func testParliamentTimedOvnTrustAnchorSnapshotsInputsAndArchiveOnlySurfaceIsGone() throws {
        var network = Data(repeating: 1, count: 32)
        var context = Data(repeating: 3, count: 32)
        var ballot = Data(repeating: 5, count: 32)
        let anchor = try ParliamentTimedOvnCastingTrustAnchorV1(
            networkID: network,
            trustedCheckpointHeight: 7,
            trustedCheckpointContextID: context,
            expectedBallotAttemptID: ballot
        )
        network.resetBytes(in: network.startIndex..<network.endIndex)
        context.resetBytes(in: context.startIndex..<context.endIndex)
        ballot.resetBytes(in: ballot.startIndex..<ballot.endIndex)
        XCTAssertEqual(
            anchor,
            try ParliamentTimedOvnCastingTrustAnchorV1(
                networkID: Data(repeating: 1, count: 32),
                trustedCheckpointHeight: 7,
                trustedCheckpointContextID: Data(repeating: 3, count: 32),
                expectedBallotAttemptID: Data(repeating: 5, count: 32)
            )
        )
        XCTAssertThrowsError(
            try ParliamentTimedOvnCastingTrustAnchorV1(
                networkID: Data(repeating: 1, count: 31),
                trustedCheckpointHeight: 7,
                trustedCheckpointContextID: Data(repeating: 3, count: 32),
                expectedBallotAttemptID: Data(repeating: 5, count: 32)
            )
        )

        var packageRoot = URL(fileURLWithPath: #filePath)
        for _ in 0..<3 { packageRoot.deleteLastPathComponent() }
        let source = try String(
            contentsOf: packageRoot.appendingPathComponent("Sources/IrohaSwift/NativeBridge.swift"),
            encoding: .utf8
        )
        XCTAssertFalse(source.contains("registration_from_seed_v1"))
        XCTAssertFalse(source.contains("ballot_from_seed_v1"))
        let proofGate = try XCTUnwrap(source.range(of: "let verifyStatus ="))
        let seedBorrow = try XCTUnwrap(source.range(of: "return try seedHandle.withUnsafeSeedBytes"))
        XCTAssertLessThan(proofGate.lowerBound, seedBorrow.lowerBound)
    }

    #if canImport(Darwin)
    func testParliamentTimedOvnWalletFailsClosedWhenBridgeIsDisabled() throws {
        struct SeedHandle: ParliamentTimedOvnSeedHandle {
            let seed: [UInt8]

            func withUnsafeSeedBytes(
                _ body: (UnsafeRawBufferPointer) throws -> Data
            ) throws -> Data {
                try seed.withUnsafeBytes(body)
            }
        }

        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(false)
        defer { NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(nil) }
        let handle = SeedHandle(seed: [UInt8](repeating: 7, count: 32))
        let trustAnchor = try ParliamentTimedOvnCastingTrustAnchorV1(
            networkID: Data(repeating: 1, count: 32),
            trustedCheckpointHeight: 7,
            trustedCheckpointContextID: Data(repeating: 3, count: 32),
            expectedBallotAttemptID: Data(repeating: 5, count: 32)
        )
        XCTAssertThrowsError(
            try NoritoNativeBridge.shared.parliamentTimedOvnRegistrationFromProofV1(
                castingProofResponseNorito: Data([1]),
                trustAnchor: trustAnchor,
                authority: "ed0120" + String(repeating: "00", count: 32),
                seedHandle: handle
            )
        ) { error in
            XCTAssertEqual(
                error as? ParliamentTimedOvnNativeWalletError,
                .bridgeUnavailable
            )
        }
    }
    #endif

    #if os(macOS)
    func testMacOSLoaderSelectsTheUniversalSlice() {
        XCTAssertEqual(NoritoBridgeLoader.currentIdentifier(), "macos-arm64_x86_64")
    }
    #endif

    func testMissingBridgeIsReported() {
        let status = NoritoBridgeLoader.validateForTests(at: "/tmp/does/not/exist", allowUntrustedLocation: true)
        XCTAssertEqual(status, .missing(path: "/tmp/does/not/exist"))
    }

    func testZkAssetPolicyStatusUsesTheCurrentFirstReleaseName() {
        XCTAssertEqual(NativeBridgeError.fromStatus(-404), .zkAssetPolicy)
    }

    func testExplicitChainDiscriminantRequiresNativeScope() throws {
        XCTAssertNoThrow(try NoritoNativeBridge.validateChainDiscriminantContext(
            nil,
            scopeAvailable: false
        ))
        XCTAssertNoThrow(try NoritoNativeBridge.validateChainDiscriminantContext(
            SccpV1.tairaI105DiscriminantV1,
            scopeAvailable: true
        ))
        XCTAssertThrowsError(try NoritoNativeBridge.validateChainDiscriminantContext(
            SccpV1.tairaI105DiscriminantV1,
            scopeAvailable: false
        )) { error in
            XCTAssertEqual(error as? NativeBridgeError, .bridgeUnavailable)
        }
    }

    func testTamperedBridgeFailsHashCheck() throws {
        let original = try bundledBridgeBinary()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let tampered = stagedBridgeURL(root: tempDir, identifier: original.identifier)
        try FileManager.default.createDirectory(
            at: tampered.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try FileManager.default.copyItem(at: original.url, to: tampered)

        var data = try Data(contentsOf: tampered)
        if !data.isEmpty {
            data[0] ^= 0xFF
        }
        try data.write(to: tampered, options: .atomic)

        let status = NoritoBridgeLoader.validateForTests(at: tampered.path, allowUntrustedLocation: true)
        switch status {
        case .hashMismatch(let path, _, _):
            XCTAssertEqual(path, tampered.path)
        default:
            XCTFail("expected hash mismatch, got \(status)")
        }
    }

    func testManifestlessBridgeUsesPinnedFallbackHash() throws {
        // `dist` is an ignored local build output, so use controlled bytes to test the
        // fallback policy independently of whichever XCFramework a developer materialized.
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let identifier = NoritoBridgeLoader.currentIdentifier()
        let bridgeURL = stagedBridgeURL(root: tempDir, identifier: identifier)
        try FileManager.default.createDirectory(
            at: bridgeURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        let fixture = Data("manifestless-native-bridge-fixture-v1".utf8)
        try fixture.write(to: bridgeURL, options: .atomic)
        let pinnedHash = "6a917bf15f2d25afa0100a8d2b320eb1ec55c48b99ccb1cef3cf43d92b2af4f2"
        XCTAssertEqual(
            SHA256.hash(data: fixture).map { String(format: "%02x", $0) }.joined(),
            pinnedHash
        )

        let status = NoritoBridgeLoader.validateForTests(
            at: bridgeURL.path,
            allowUntrustedLocation: true,
            pinnedHashesForTests: [identifier: pinnedHash]
        )
        XCTAssertEqual(status, .valid(path: bridgeURL.path, identifier: identifier))
    }

    func testManifestlessBridgeRejectsBinaryThatDoesNotMatchPinnedFallbackHash() throws {
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let identifier = NoritoBridgeLoader.currentIdentifier()
        let bridgeURL = stagedBridgeURL(root: tempDir, identifier: identifier)
        try FileManager.default.createDirectory(
            at: bridgeURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        let fixture = Data("manifestless-native-bridge-fixture-v1".utf8)
        try fixture.write(to: bridgeURL, options: .atomic)
        let actualHash = SHA256.hash(data: fixture)
            .map { String(format: "%02x", $0) }
            .joined()
        let incorrectPin = String(repeating: "0", count: 64)

        let status = NoritoBridgeLoader.validateForTests(
            at: bridgeURL.path,
            allowUntrustedLocation: true,
            pinnedHashesForTests: [identifier: incorrectPin]
        )
        XCTAssertEqual(
            status,
            .hashMismatch(path: bridgeURL.path, expected: incorrectPin, actual: actualHash)
        )
    }

    func testUntrustedPathIsDeniedWhenOverridesDisabled() throws {
        let original = try bundledBridgeBinary()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: tempDir, withIntermediateDirectories: true)
        let target = tempDir.appendingPathComponent("NoritoBridge")
        try FileManager.default.copyItem(at: original.url, to: target)

        let status = NoritoBridgeLoader.validateForTests(at: target.path, allowUntrustedLocation: false)
        XCTAssertEqual(status, .pathDenied(path: target.path))
    }

    func testArtifactManifestHashOverridesPinnedHashForLocalBridge() throws {
        let original = try bundledBridgeBinary()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let bridgeURL = stagedBridgeURL(root: tempDir, identifier: original.identifier)
        try FileManager.default.createDirectory(
            at: bridgeURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try FileManager.default.copyItem(at: original.url, to: bridgeURL)

        var tamperedData = try Data(contentsOf: bridgeURL)
        if !tamperedData.isEmpty {
            tamperedData[0] ^= 0xA5
        }
        try tamperedData.write(to: bridgeURL, options: .atomic)

        let hashHex = SHA256.hash(data: tamperedData).map { String(format: "%02x", $0) }.joined()
        let manifestURL = tempDir.appendingPathComponent("NoritoBridge.artifacts.json")
        let manifest = """
        {
          "version": "\(NoritoBridgeLoader.expectedVersion)",
          "native_bridge_abi_version": 23,
          "hashes": {
            "\(original.identifier)": "\(hashHex)"
          }
        }
        """
        try manifest.write(to: manifestURL, atomically: true, encoding: .utf8)

        let status = NoritoBridgeLoader.validateForTests(at: bridgeURL.path, allowUntrustedLocation: true)
        XCTAssertEqual(status, .valid(path: bridgeURL.path, identifier: original.identifier))
    }

    func testArtifactManifestRejectsStaleAbiNineteenBeforeHashAcceptance() throws {
        let identifier = NoritoBridgeLoader.currentIdentifier()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let bridgeURL = stagedBridgeURL(root: tempDir, identifier: identifier)
        try FileManager.default.createDirectory(
            at: bridgeURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        let bytes = Data("stale-abi-19-bridge".utf8)
        try bytes.write(to: bridgeURL, options: .atomic)
        let hashHex = SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
        let manifestURL = tempDir.appendingPathComponent("NoritoBridge.artifacts.json")
        let manifest = """
        {
          "version": "\(NoritoBridgeLoader.expectedVersion)",
          "native_bridge_abi_version": 19,
          "hashes": {
            "\(identifier)": "\(hashHex)"
          }
        }
        """
        try manifest.write(to: manifestURL, atomically: true, encoding: .utf8)

        let status = NoritoBridgeLoader.validateForTests(
            at: bridgeURL.path,
            allowUntrustedLocation: true
        )
        XCTAssertEqual(
            status,
            .abiMismatch(path: bridgeURL.path, expected: 23, actual: 19)
        )
    }

    func testArtifactManifestAtDistRootOverridesPinnedHashForXcframeworkLayout() throws {
        let original = try bundledBridgeBinary()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let target = tempDir
            .appendingPathComponent("NoritoBridge.xcframework", isDirectory: true)
        let bridgeURL = stagedBridgeURL(root: target, identifier: original.identifier)
        try FileManager.default.createDirectory(
            at: bridgeURL.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try FileManager.default.copyItem(at: original.url, to: bridgeURL)

        var tamperedData = try Data(contentsOf: bridgeURL)
        if !tamperedData.isEmpty {
            tamperedData[0] ^= 0x5A
        }
        try tamperedData.write(to: bridgeURL, options: .atomic)

        let hashHex = SHA256.hash(data: tamperedData).map { String(format: "%02x", $0) }.joined()
        let manifestURL = tempDir.appendingPathComponent("NoritoBridge.artifacts.json")
        let manifest = """
        {
          "version": "\(NoritoBridgeLoader.expectedVersion)",
          "native_bridge_abi_version": 23,
          "hashes": {
            "\(original.identifier)": "\(hashHex)"
          }
        }
        """
        try manifest.write(to: manifestURL, atomically: true, encoding: .utf8)

        let status = NoritoBridgeLoader.validateForTests(at: bridgeURL.path, allowUntrustedLocation: true)
        XCTAssertEqual(status, .valid(path: bridgeURL.path, identifier: original.identifier))
    }

    func testArtifactManifestCandidateSearchHonorsAscentLimit() {
        let binaryURL = URL(fileURLWithPath:
            "/tmp/NoritoBridge.xcframework/ios-arm64/Library/Frameworks/libNoritoBridge.a")
        let xcframeworkManifest = URL(fileURLWithPath:
            "/tmp/NoritoBridge.xcframework/NoritoBridge.artifacts.json")
        let siblingManifest = URL(fileURLWithPath: "/tmp/NoritoBridge.artifacts.json")

        let limited = NoritoBridgeLoader.candidateArtifactManifestURLsForTests(
            near: binaryURL,
            maxAscents: 1
        )
        XCTAssertFalse(limited.contains(xcframeworkManifest))
        XCTAssertFalse(limited.contains(siblingManifest))

        let full = NoritoBridgeLoader.candidateArtifactManifestURLsForTests(
            near: binaryURL,
            maxAscents: 4
        )
        XCTAssertTrue(full.contains(xcframeworkManifest))
        XCTAssertTrue(full.contains(siblingManifest))
    }

    private func bundledBridgeBinary() throws -> (url: URL, identifier: String) {
        #if os(macOS)
        let identifier = "macos-arm64_x86_64"
        #else
        #if targetEnvironment(simulator)
        let identifier = "ios-arm64_x86_64-simulator"
        #else
        let identifier = "ios-arm64"
        #endif
        #endif

        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { root.deleteLastPathComponent() }
        let url = stagedBridgeURL(
            root: root.appendingPathComponent("dist/NoritoBridge.xcframework"),
            identifier: identifier
        )
        try requireNativeTestCapability(
            FileManager.default.fileExists(atPath: url.path),
            "NoritoBridge.xcframework missing at \(url.path)"
        )
        return (url, identifier)
    }

    private func stagedBridgeURL(root: URL, identifier: String) -> URL {
        root
            .appendingPathComponent(identifier, isDirectory: true)
            .appendingPathComponent("libNoritoBridge.a")
    }
}

final class BridgePolicyHintTests: XCTestCase {
    func testBridgeRequirementHintReferencesPath() {
        let hint = NoritoNativeBridge.bridgeRequirementHint
        XCTAssertTrue(hint.contains("NoritoBridge.xcframework"))
        XCTAssertTrue(hint.contains("../dist/NoritoBridge.xcframework"))
    }
}

#if canImport(Darwin)
final class BridgeAvailabilitySurfaceTests: XCTestCase {
    func testTransferEncodingFailsWhenBridgeUnavailable() throws {
        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(false)
        defer { NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(nil) }

        let keypair = try Keypair(privateKeyBytes: Data(repeating: 7, count: 32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = TransferRequest(networkId: TestNetworkIds.canonical,
                                      authority: authority,
                                      assetDefinitionId: "66owaQmAQMuHxPzxUN3bqZ6FJfDa",
                                      quantity: "1",
                                      destination: authority,
                                      description: nil,
                                      feePayment: .authority(chargeLimits: [], gasLimit: nil),
                                      ttlMs: nil)

        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeTransfer(transfer: request,
                                                                        keypair: keypair,
                                                                        creationTimeMs: 0)) { error in
            guard case SwiftTransactionEncoderError.nativeBridgeUnavailable = error else {
                XCTFail("expected nativeBridgeUnavailable, got \(error)")
                return
            }
            XCTAssertTrue(error.localizedDescription.contains("NoritoBridge.xcframework"))
        }
    }

    func testConnectCodecUnavailableWhenBridgeDisabled() throws {
        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(false)
        NoritoNativeBridge.shared.overrideConnectCodecAvailabilityForTests(false)
        defer {
            NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(nil)
            NoritoNativeBridge.shared.overrideConnectCodecAvailabilityForTests(nil)
        }

        let frame = ConnectFrame(
            sessionID: Data([0x01]),
            direction: .appToWallet,
            sequence: 0,
            kind: .ciphertext(.init(payload: Data([0x02])))
        )

        XCTAssertThrowsError(try ConnectCodec.encode(frame)) { error in
            guard case ConnectCodecError.bridgeUnavailable = error else {
                XCTFail("expected ConnectCodecError.bridgeUnavailable, got \(error)")
                return
            }
            XCTAssertTrue(error.localizedDescription.contains("NoritoBridge.xcframework"))
        }
    }
}
#endif
