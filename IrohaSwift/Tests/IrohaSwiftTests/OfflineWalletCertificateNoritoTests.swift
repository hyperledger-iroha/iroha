import Foundation
import XCTest
@testable import IrohaSwift

final class OfflineWalletCertificateNoritoTests: XCTestCase {
    func testCertificateNoritoMatchesFixture() throws {
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let expected = try Data(contentsOf: fixtureURL("certificate.norito"))
        let encoded = try certificate.noritoEncoded()
        XCTAssertEqual(encoded, expected)
    }

    func testCertificateIdMatchesSummaryFixture() throws {
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let summaryData = try Data(contentsOf: fixtureURL("summary.json"))
        let summary = try JSONSerialization.jsonObject(with: summaryData, options: []) as? [String: Any]
        let expected = summary?["certificate_id_hex"] as? String
        XCTAssertNotNil(expected)
        XCTAssertEqual(try certificate.certificateIdHex(), expected)
    }

    // MARK: - Helpers

    private func fixtureURL(_ name: String) -> URL {
        var url = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            url.deleteLastPathComponent()
        }
        return url.appendingPathComponent("fixtures/offline_allowance/ios-demo/\(name)")
    }
}
