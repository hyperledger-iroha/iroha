import XCTest
@testable import IrohaSwift

final class AccelerationSettingsLoaderTests: XCTestCase {
    func testLoadsSettingsFromExplicitFile() throws {
        let url = try makeTemporaryConfig("""
        {"accel":{"enable_metal":false,"max_gpus":2}}
        """)
        defer { try? FileManager.default.removeItem(at: url) }

        let settings = try AccelerationSettingsLoader.load(
            configurationURL: url,
            bundle: nil
        )

        XCTAssertFalse(settings.enableMetal)
        XCTAssertEqual(settings.maxGPUs, 2)
    }

    func testExplicitFileKeepsLiteralZeroLimits() throws {
        let url = try makeTemporaryConfig("""
        {"accel":{"enable_metal":true,"merkle_min_leaves_metal":0,"resource_limits":{"device_bytes":0}}}
        """)
        defer { try? FileManager.default.removeItem(at: url) }

        let settings = try AccelerationSettingsLoader.load(
            configurationURL: url,
            bundle: nil
        )

        XCTAssertTrue(settings.enableMetal)
        XCTAssertEqual(settings.merkleMinLeavesMetal, 0)
        XCTAssertEqual(settings.resourceLimits.deviceBytes, 0)
    }

    func testLoadsBundleResourceWhenNoExplicitFile() throws {
        let settings = try AccelerationSettingsLoader.load(bundle: Bundle.module)
        XCTAssertFalse(settings.enableMetal)
        XCTAssertEqual(settings.merkleMinLeavesGPU, 128)
    }

    func testDefaultsWhenNoConfigFound() throws {
        let settings = try AccelerationSettingsLoader.load(bundle: nil)
        XCTAssertTrue(settings.enableMetal)
        XCTAssertNil(settings.merkleMinLeavesGPU)
    }

    func testInvalidSelectedFileCannotFallBackToEnabledDefaults() throws {
        let url = try makeTemporaryConfig(#"{"accel":{"resource_limits":{"device_bytes":-1}}}"#)
        defer { try? FileManager.default.removeItem(at: url) }
        XCTAssertThrowsError(try AccelerationSettingsLoader.load(configurationURL: url, bundle: Bundle.module))
    }

    private func makeTemporaryConfig(_ contents: String) throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathExtension("json")
        try contents.data(using: .utf8)?.write(to: url)
        return url
    }
}
