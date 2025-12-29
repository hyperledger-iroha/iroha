import XCTest
@testable import NoritoDemo

final class PosManifestTests: XCTestCase {
  private func manifestData() throws -> Data {
    let thisFile = URL(fileURLWithPath: #filePath)
    let projectRoot = thisFile
      .deletingLastPathComponent() // Tests
      .deletingLastPathComponent() // NoritoDemo
    let manifestUrl = projectRoot
      .appendingPathComponent("Sources")
      .appendingPathComponent("Resources")
      .appendingPathComponent("pos_manifest.json")
    return try Data(contentsOf: manifestUrl)
  }

  func testManifestParsesAndVerifies() throws {
    let manifest = try PosManifestLoader.parse(data: try manifestData())
    XCTAssertEqual(manifest.manifestId, "pos-retail-v1")
    XCTAssertEqual(manifest.sequence, 7)
    XCTAssertEqual(manifest.backendRoots.count, 2)
  }

  func testTamperedSignatureFails() throws {
    let original = try manifestData()
    guard var manifestJson = String(data: original, encoding: .utf8) else {
      XCTFail("failed to load manifest JSON")
      return
    }
    manifestJson = manifestJson.replacingOccurrences(
      of: "D32F71FF",
      with: "FFFFFFFF",
      options: [.caseInsensitive]
    )
    let tampered = Data(manifestJson.utf8)
    XCTAssertThrowsError(try PosManifestLoader.parse(data: tampered)) { error in
      let description = (error as NSError).localizedDescription.lowercased()
      XCTAssertTrue(description.contains("signature"))
    }
  }
}
