import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Exact Rust/Swift golden coverage for the sole ABI-19 device-registration model.
final class OfflineDeviceAttestationABI19ParityTests: XCTestCase {
    func testRegistrationAndChallengeMatchRustCurrentModel() throws {
        let rust = try rustFixture()
        XCTAssertEqual(rust.count, 4)
        let assertionPublicKey = try XCTUnwrap(Data(hexString: Self.p256Generator))
        let signingCertificate = Data(
            SHA256.hash(data: Data("abi19-unit-test-signing-certificate".utf8))
        )
        let registration = try KagemushaDeviceAttestationRegistration(
            version: 1,
            platform: KagemushaDeviceAttestation.androidKeyMintPlatform,
            keyId: Data(SHA256.hash(data: assertionPublicKey)).hexLowercased(),
            deviceId: "abi19-android-unit-test-device",
            accountId: rust[3],
            assetDefinitionId: nil,
            iosTeamId: nil,
            iosBundleId: nil,
            iosEnvironment: nil,
            androidPackageName: "org.hyperledger.iroha.abi19.fixture",
            androidSigningCertificateSha256: signingCertificate,
            publicKey: Data(repeating: 0x44, count: 32),
            assertionScheme: KagemushaDeviceAttestation.androidKeyMintAssertionScheme,
            assertionKeyAlgorithm:
                KagemushaDeviceAttestation.androidKeyMintAssertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: 1,
            oneUse: true,
            attestationReport:
                Data("abi19-unit-test-not-physical-attestation-evidence".utf8),
            recentBlockHeight: 42,
            recentBlockHash: IrohaHash.hash(Data("abi19-unit-test-block".utf8)),
            expiresAtMs: 2_000_000_000_000
        )

        XCTAssertEqual(try registration.noritoEncoded(), try XCTUnwrap(Data(hexString: rust[0])))
        XCTAssertEqual(registration.challengeHash, try XCTUnwrap(Data(hexString: rust[2])))
    }

    private func rustFixture() throws -> [String] {
        let source = URL(fileURLWithPath: #filePath)
        let repoRoot = source
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let process = Process()
        process.currentDirectoryURL = repoRoot
        process.executableURL = URL(fileURLWithPath: "/usr/bin/env")
        process.arguments = [
            "CARGO_TARGET_DIR=\(repoRoot.appendingPathComponent("target/kotlin-fixture-gen-test").path)",
            "cargo",
            "run",
            "-q",
            "-p",
            "kotlin-fixture-gen",
            "--",
            "offline-device-attestation"
        ]
        let stdout = Pipe()
        let stderr = Pipe()
        process.standardOutput = stdout
        process.standardError = stderr
        try process.run()
        let output = stdout.fileHandleForReading.readDataToEndOfFile()
        let error = stderr.fileHandleForReading.readDataToEndOfFile()
        process.waitUntilExit()
        guard process.terminationStatus == 0 else {
            throw NSError(
                domain: "OfflineDeviceAttestationABI19ParityTests",
                code: Int(process.terminationStatus),
                userInfo: [
                    NSLocalizedDescriptionKey:
                        String(data: error, encoding: .utf8) ?? "Rust fixture generator failed"
                ]
            )
        }
        return try XCTUnwrap(String(data: output, encoding: .utf8))
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .split(separator: "\n")
            .map(String.init)
    }

    private static let p256Generator =
        "04"
        + "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
        + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"
}
