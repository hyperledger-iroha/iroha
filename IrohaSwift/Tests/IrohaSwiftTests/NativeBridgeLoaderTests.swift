import Foundation
import CryptoKit
import XCTest
@testable import IrohaSwift

final class NativeBridgeLoaderTests: XCTestCase {
    func testMissingBridgeIsReported() {
        let status = NoritoBridgeLoader.validateForTests(at: "/tmp/does/not/exist", allowUntrustedLocation: true)
        XCTAssertEqual(status, .missing(path: "/tmp/does/not/exist"))
    }

    func testTamperedBridgeFailsHashCheck() throws {
        let original = try bundledBridgeBinary()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let target = tempDir
            .appendingPathComponent(original.identifier, isDirectory: true)
            .appendingPathComponent("NoritoBridge.framework", isDirectory: true)
        try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
        let tampered = target.appendingPathComponent("NoritoBridge")
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
        let target = tempDir
            .appendingPathComponent(original.identifier, isDirectory: true)
            .appendingPathComponent("NoritoBridge.framework", isDirectory: true)
        try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
        let bridgeURL = target.appendingPathComponent("NoritoBridge")
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
          "hashes": {
            "\(original.identifier)": "\(hashHex)"
          }
        }
        """
        try manifest.write(to: manifestURL, atomically: true, encoding: .utf8)

        let status = NoritoBridgeLoader.validateForTests(at: bridgeURL.path, allowUntrustedLocation: true)
        XCTAssertEqual(status, .valid(path: bridgeURL.path, identifier: original.identifier))
    }

    func testArtifactManifestAtDistRootOverridesPinnedHashForXcframeworkLayout() throws {
        let original = try bundledBridgeBinary()
        let tempDir = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        let target = tempDir
            .appendingPathComponent("NoritoBridge.xcframework", isDirectory: true)
            .appendingPathComponent(original.identifier, isDirectory: true)
            .appendingPathComponent("NoritoBridge.framework", isDirectory: true)
        try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
        let bridgeURL = target.appendingPathComponent("NoritoBridge")
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
          "hashes": {
            "\(original.identifier)": "\(hashHex)"
          }
        }
        """
        try manifest.write(to: manifestURL, atomically: true, encoding: .utf8)

        let status = NoritoBridgeLoader.validateForTests(at: bridgeURL.path, allowUntrustedLocation: true)
        XCTAssertEqual(status, .valid(path: bridgeURL.path, identifier: original.identifier))
    }

    private func bundledBridgeBinary() throws -> (url: URL, identifier: String) {
        #if os(macOS)
        let identifier = "macos-arm64"
        #else
        #if targetEnvironment(simulator)
        let identifier = "ios-arm64_x86_64-simulator"
        #else
        let identifier = "ios-arm64"
        #endif
        #endif

        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 { root.deleteLastPathComponent() }
        let url = root
            .appendingPathComponent("dist/NoritoBridge.xcframework")
            .appendingPathComponent(identifier)
            .appendingPathComponent("NoritoBridge.framework/NoritoBridge")
        guard FileManager.default.fileExists(atPath: url.path) else {
            throw XCTSkip("NoritoBridge.xcframework missing at \(url.path)")
        }
        return (url, identifier)
    }
}

final class BridgePolicyHintTests: XCTestCase {
    func testBridgeRequirementHintReferencesPath() {
        let hint = NoritoNativeBridge.bridgeRequirementHint
        XCTAssertTrue(hint.contains("NoritoBridge.xcframework"))
        XCTAssertTrue(hint.contains("../dist/NoritoBridge.xcframework"))
    }
}

final class BridgeAvailabilitySurfaceTests: XCTestCase {
    func testTransferEncodingFailsWhenBridgeUnavailable() throws {
        #if canImport(Darwin)
        guard #available(macOS 10.15, iOS 13.0, *) else {
            throw XCTSkip("CryptoKit is unavailable on this platform.")
        }

        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(false)
        defer { NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(nil) }

        let keypair = try Keypair(privateKeyBytes: Data(repeating: 7, count: 32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let request = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                      authority: authority,
                                      assetDefinitionId: "aid:2f17c72466f84a4bb8a8e24884fdcd2f",
                                      quantity: "1",
                                      destination: authority,
                                      description: nil,
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
        #else
        throw XCTSkip("Bridge availability is only meaningful on Darwin targets.")
        #endif
    }

    func testConnectCodecUnavailableWhenBridgeDisabled() throws {
        #if canImport(Darwin)
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
        #else
        throw XCTSkip("Bridge availability is only meaningful on Darwin targets.")
        #endif
    }
}
