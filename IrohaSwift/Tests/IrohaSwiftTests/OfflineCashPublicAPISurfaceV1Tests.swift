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
            "public final class OfflineCashWalletSessionV1",
            "public struct OfflineCashPeerAdapterV1",
        ] {
            XCTAssertTrue(source.contains(declaration), "missing \(declaration)")
        }
        XCTAssertEqual(OfflineCashPeerAdapterV1.textPrefix, "kgm2:")
        XCTAssertFalse(source.contains("PKK2"))
        XCTAssertFalse(source.contains("PKKQ1"))
    }

    private static var packageRoot: URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
    }
}
