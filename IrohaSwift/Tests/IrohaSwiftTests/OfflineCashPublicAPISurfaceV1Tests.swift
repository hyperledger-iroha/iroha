import Foundation
import XCTest
@testable import IrohaSwift

/// Locks the first-release product boundary: `kgm2:` Offline Cash V1 is public;
/// the recursive Kagemusha implementation and retired PKK2 rails are package-only.
final class OfflineCashPublicAPISurfaceV1Tests: XCTestCase {
    private static let legacySourceFiles = [
        "IrohaPeerKagemushaAdapterV1.swift",
        "KagemushaArtifactCoordinator.swift",
        "KagemushaNFC.swift",
        "KagemushaNearby.swift",
        "KagemushaOperationFinalityCoordinator.swift",
        "KagemushaPeerTransport.swift",
        "KagemushaQRStream.swift",
        "KagemushaRecursiveSpendV2.swift",
        "KagemushaRecursiveSpendV2Codecs.swift",
        "KagemushaRecursiveSpendV4.swift",
        "KagemushaScaledAmount.swift",
        "OfflineDeviceAttestation.swift",
        "ToriiKagemushaAPIModels.swift",
    ]

    func testLegacyKagemushaSubstrateHasNoPublicTopLevelDeclarations() throws {
        let sources = Self.packageRoot
            .appendingPathComponent("Sources/IrohaSwift", isDirectory: true)
        let forbidden = try NSRegularExpression(
            pattern: #"(?m)^(?:public|open)\s+(?:class|final\s+class|struct|enum|protocol|typealias|extension)\b"#
        )

        for name in Self.legacySourceFiles {
            let source = try String(
                contentsOf: sources.appendingPathComponent(name),
                encoding: .utf8
            )
            let range = NSRange(source.startIndex..<source.endIndex, in: source)
            XCTAssertNil(
                forbidden.firstMatch(in: source, range: range),
                "\(name) must remain package-only"
            )
        }
    }

    func testOfflineCashV1RemainsThePublicPeerContract() throws {
        let source = try String(
            contentsOf: Self.packageRoot
                .appendingPathComponent("Sources/IrohaSwift/OfflineCashV1.swift"),
            encoding: .utf8
        )
        for declaration in [
            "public struct OfflineCashPaymentRequestV1",
            "public struct OfflineCashPaymentV1",
            "public struct OfflineCashAcknowledgementV1",
            "public struct OfflineCashReleaseStatusV1",
            "public final class OfflineCashVerificationSessionV1",
            "public final class OfflineCashWalletSessionV1",
            "public struct OfflineCashPeerAdapterV1",
        ] {
            XCTAssertTrue(source.contains(declaration), "missing \(declaration)")
        }
        XCTAssertEqual(OfflineCashPeerAdapterV1.textPrefix, "kgm2:")
        XCTAssertFalse(source.contains("PKK2"))
        XCTAssertFalse(source.contains("PKKQ1"))
    }

    func testVerifierAndWalletFacadesAreTruthfullySeparated() throws {
        let source = try String(
            contentsOf: Self.packageRoot
                .appendingPathComponent("Sources/IrohaSwift/OfflineCashV1.swift"),
            encoding: .utf8
        )
        let wallet = try XCTUnwrap(
            source.components(separatedBy: "public final class OfflineCashWalletSessionV1 {").last?
                .components(separatedBy: "/// Strict `kgm2:` peer adapter").first
        )
        XCTAssertTrue(wallet.contains("private init()"))
        XCTAssertTrue(wallet.contains("throw OfflineCashWalletSessionErrorV1.unavailable"))
        for forbidden in ["Data", "UInt64", "Date", "nativeHandle", "canonicalNorito"] {
            XCTAssertFalse(wallet.contains(forbidden), "wallet facade exposes \(forbidden)")
        }

        XCTAssertTrue(source.contains("OfflineCashVerificationSessionStateV1"))
        XCTAssertTrue(source.contains("connect_norito_offline_cash_verification_session_"))
        XCTAssertFalse(source.contains("connect_norito_offline_cash_wallet_session_"))
    }

    func testOfflineCashV1ExposesTheExactPublicToriiFacade() throws {
        let source = try String(
            contentsOf: Self.packageRoot
                .appendingPathComponent("Sources/IrohaSwift/OfflineCashToriiV1.swift"),
            encoding: .utf8
        )
        for declaration in [
            "public struct OfflineCashTopUpRequestV1",
            "public struct OfflineCashRedeemRequestV1",
            "public struct OfflineCashOperationReferenceV1",
            "public struct OfflineCashOperationStatusV1",
            "public struct OfflineCashReadinessV1",
            "public final class OfflineCashToriiClientV1",
        ] {
            XCTAssertTrue(source.contains(declaration), "missing \(declaration)")
        }
        XCTAssertEqual(OfflineCashToriiClientV1.readinessPath, "/v1/offline/readiness")
        XCTAssertEqual(OfflineCashToriiClientV1.topUpPath, "/v1/offline/top-up")
        XCTAssertEqual(OfflineCashToriiClientV1.redeemPath, "/v1/offline/redeem")
        XCTAssertEqual(OfflineCashToriiClientV1.operationsPath, "/v1/offline/operations")
        XCTAssertEqual(OfflineCashToriiClientV1.jsonMediaType, "application/json")
        XCTAssertEqual(OfflineCashToriiClientV1.noritoMediaType, "application/x-norito")

        let forbiddenPublicSubstrate = try NSRegularExpression(
            pattern: #"(?m)^public\s+(?:final\s+class|class|struct|enum|protocol|typealias)\s+Kagemusha"#
        )
        let range = NSRange(source.startIndex..<source.endIndex, in: source)
        XCTAssertNil(forbiddenPublicSubstrate.firstMatch(in: source, range: range))
    }

    private static var packageRoot: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }
}
