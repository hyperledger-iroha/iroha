import Foundation
import XCTest
@testable import IrohaSwift

final class OfflineWalletCertificateNoritoTests: XCTestCase {
    func testCertificateNoritoMatchesFixture() throws {
        let certificate = try OfflineWalletCertificate.load(from: fixtureURL("certificate.json"))
        let expected = try Data(contentsOf: fixtureURL("certificate.norito"))
        let encoded = try certificate.noritoEncoded()

        if encoded != expected {
            print("=== NORITO ENCODING MISMATCH ===")
            print("Swift: \(encoded.count) bytes")
            print("Rust:  \(expected.count) bytes")

            // Find first difference
            let minLen = min(encoded.count, expected.count)
            for i in 0..<minLen {
                if encoded[i] != expected[i] {
                    let sStart = max(0, i - 8)
                    let sEnd = min(encoded.count, i + 24)
                    let rEnd = min(expected.count, i + 24)
                    let swiftHex = encoded[sStart..<sEnd].map { String(format: "%02x", $0) }.joined(separator: " ")
                    let rustHex = expected[sStart..<rEnd].map { String(format: "%02x", $0) }.joined(separator: " ")
                    print("First diff at byte \(i):")
                    print("  Swift[\(sStart)..\(sEnd)]: \(swiftHex)")
                    print("  Rust [\(sStart)..\(rEnd)]: \(rustHex)")
                    break
                }
            }

            // Dump field-by-field comparison of first 400 bytes
            print()
            print("Header (40 bytes):")
            let hdrLen = min(40, min(encoded.count, expected.count))
            print("  Swift: \(encoded.prefix(hdrLen).map { String(format: "%02x", $0) }.joined(separator: " "))")
            print("  Rust:  \(expected.prefix(hdrLen).map { String(format: "%02x", $0) }.joined(separator: " "))")

            // Decode field lengths starting at offset 40
            print()
            print("Payload field lengths:")
            var swiftOff = 40
            var rustOff = 40
            for fieldIdx in 0..<13 {
                let sFieldLen: UInt64
                let rFieldLen: UInt64
                if swiftOff + 8 <= encoded.count {
                    var val: UInt64 = 0
                    for b in (0..<8).reversed() { val = val << 8 | UInt64(encoded[swiftOff + b]) }
                    sFieldLen = val
                } else {
                    sFieldLen = UInt64.max
                }
                if rustOff + 8 <= expected.count {
                    var val: UInt64 = 0
                    for b in (0..<8).reversed() { val = val << 8 | UInt64(expected[rustOff + b]) }
                    rFieldLen = val
                } else {
                    rFieldLen = UInt64.max
                }
                let marker = sFieldLen == rFieldLen ? "✓" : "✗"
                print("  Field \(fieldIdx): Swift=\(sFieldLen) (off=\(swiftOff))  Rust=\(rFieldLen) (off=\(rustOff))  \(marker)")

                if sFieldLen != UInt64.max { swiftOff += 8 + Int(sFieldLen) }
                if rFieldLen != UInt64.max { rustOff += 8 + Int(rFieldLen) }
            }
        }

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
