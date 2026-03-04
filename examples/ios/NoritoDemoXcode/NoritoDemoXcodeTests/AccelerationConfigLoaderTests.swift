#if canImport(IrohaSwift)
import XCTest
@testable import NoritoDemoXcode

final class AccelerationConfigLoaderTests: XCTestCase {
  private var originalPath: String?

  override func setUp() {
    super.setUp()
    originalPath = ProcessInfo.processInfo.environment[DemoAccelerationConfig.environmentKey]
  }

  override func tearDown() {
    if let originalPath {
      setenv(DemoAccelerationConfig.environmentKey, originalPath, 1)
    } else {
      unsetenv(DemoAccelerationConfig.environmentKey)
    }
    super.tearDown()
  }

  func testLoadsSettingsFromEnvironmentJSON() throws {
    let tmpURL = FileManager.default.temporaryDirectory
      .appendingPathComponent(UUID().uuidString)
      .appendingPathExtension("json")
    let contents = """
    {"accel":{"enable_metal":false,"prefer_cpu_sha2_max_leaves_aarch64":42}}
    """
    try contents.data(using: .utf8)?.write(to: tmpURL)

    setenv(DemoAccelerationConfig.environmentKey, tmpURL.path, 1)

    let settings = DemoAccelerationConfig.load()

    XCTAssertFalse(settings.enableMetal)
    XCTAssertEqual(settings.preferCpuSha2MaxLeavesAarch64, 42)

    try? FileManager.default.removeItem(at: tmpURL)
  }

  func testDefaultsWhenNoConfigAvailable() {
    unsetenv(DemoAccelerationConfig.environmentKey)
    let settings = DemoAccelerationConfig.load()
    XCTAssertTrue(settings.enableMetal)
    XCTAssertNil(settings.maxGPUs)
  }
}
#endif
