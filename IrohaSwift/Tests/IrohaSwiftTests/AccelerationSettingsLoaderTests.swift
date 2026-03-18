import XCTest
@testable import IrohaSwift

final class AccelerationSettingsLoaderTests: XCTestCase {
    func testLoadsSettingsFromEnvironmentPath() throws {
        let url = try makeTemporaryConfig("""
        {"accel":{"enable_metal":false,"max_gpus":2}}
        """)
        defer { try? FileManager.default.removeItem(at: url) }

        let settings = AccelerationSettingsLoader.load(
            environmentKey: "CUSTOM_ACCEL",
            environment: ["CUSTOM_ACCEL": url.path],
            bundle: nil
        )

        XCTAssertFalse(settings.enableMetal)
        XCTAssertEqual(settings.maxGPUs, 2)
    }

    func testLoadsSettingsFromEnvironmentFileURL() throws {
        let url = try makeTemporaryConfig("""
        {"accel":{"enable_metal":true,"merkle_min_leaves_metal":512}}
        """)
        defer { try? FileManager.default.removeItem(at: url) }

        let settings = AccelerationSettingsLoader.load(
            environmentKey: "CUSTOM_ACCEL",
            environment: ["CUSTOM_ACCEL": url.absoluteString],
            bundle: nil
        )

        XCTAssertTrue(settings.enableMetal)
        XCTAssertEqual(settings.merkleMinLeavesMetal, 512)
    }

    func testFallsBackToBundleResource() {
        let settings = AccelerationSettingsLoader.load(environment: [:], bundle: Bundle.module)
        XCTAssertFalse(settings.enableMetal)
        XCTAssertEqual(settings.merkleMinLeavesGPU, 128)
    }

    func testDefaultsWhenNoConfigFound() {
        let settings = AccelerationSettingsLoader.load(environment: [:], bundle: nil)
        XCTAssertTrue(settings.enableMetal)
        XCTAssertNil(settings.merkleMinLeavesGPU)
    }

    private func makeTemporaryConfig(_ contents: String) throws -> URL {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathExtension("json")
        try contents.data(using: .utf8)?.write(to: url)
        return url
    }
}
