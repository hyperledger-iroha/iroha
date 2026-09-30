import XCTest
@testable import NoritoDemoXcode

final class AccelerationConfigLoaderTests: XCTestCase {
  func testLoadsSettingsFromExplicitJSONFile() throws {
    let tmpURL = FileManager.default.temporaryDirectory
      .appendingPathComponent(UUID().uuidString)
      .appendingPathExtension("json")
    let contents = """
    {"accel":{"enable_metal":false,"prefer_cpu_sha2_max_leaves_aarch64":42}}
    """
    try contents.data(using: .utf8)?.write(to: tmpURL)
    defer { try? FileManager.default.removeItem(at: tmpURL) }
    let settings = try DemoAccelerationConfig.load(configurationURL: tmpURL, bundle: nil)

    XCTAssertFalse(settings.enableMetal)
    XCTAssertEqual(settings.preferCpuSha2MaxLeavesAarch64, 42)
  }

  func testDefaultsWhenNoConfigAvailable() throws {
    let settings = try DemoAccelerationConfig.load(bundle: nil)
    XCTAssertTrue(settings.enableMetal)
    XCTAssertNil(settings.maxGPUs)
  }

  func testInvalidSelectedFileCannotFallBackToEnabledDefaults() throws {
    let url = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try Data(#"{"accel":{"enable_metal":"invalid"}}"#.utf8).write(to: url)
    defer { try? FileManager.default.removeItem(at: url) }
    XCTAssertThrowsError(try DemoAccelerationConfig.load(configurationURL: url, bundle: nil))
  }
}
