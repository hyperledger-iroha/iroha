@testable import IrohaSwift
import XCTest

final class AccelerationSettingsTests: XCTestCase {
    func testApplyDoesNotCrashWhenBridgeMissing() {
        let settings = AccelerationSettings(enableMetal: false,
                                            merkleMinLeavesMetal: 128,
                                            preferCpuSha2MaxLeavesAarch64: 64)
        settings.apply()
        XCTAssertTrue(true)
    }

    func testNegativeValuesAreIgnored() {
        let settings = AccelerationSettings(enableMetal: true,
                                            maxGPUs: -1,
                                            merkleMinLeavesGPU: -42,
                                            preferCpuSha2MaxLeavesAarch64: -3)
        XCTAssertNil(settings.maxGPUs)
        XCTAssertNil(settings.merkleMinLeavesGPU)
        XCTAssertNil(settings.preferCpuSha2MaxLeavesAarch64)
    }

    func testDecodesFromJSONPayload() throws {
        let json = """
        {
            "enable_metal": false,
            "enable_cuda": true,
            "max_gpus": 2,
            "merkle_min_leaves_gpu": 256,
            "merkle_min_leaves_metal": 128,
            "prefer_cpu_sha2_max_leaves_aarch64": 64
        }
        """
        let decoded = try AccelerationSettings.fromJSON(Data(json.utf8))
        XCTAssertFalse(decoded.enableMetal)
        XCTAssertTrue(decoded.enableCUDA)
        XCTAssertEqual(decoded.maxGPUs, 2)
        XCTAssertEqual(decoded.merkleMinLeavesGPU, 256)
        XCTAssertEqual(decoded.merkleMinLeavesMetal, 128)
        XCTAssertEqual(decoded.preferCpuSha2MaxLeavesAarch64, 64)
        XCTAssertNil(decoded.merkleMinLeavesCUDA)
    }

    func testEncodesToSnakeCaseKeys() throws {
        let settings = AccelerationSettings(enableMetal: false,
                                            enableCUDA: true,
                                            maxGPUs: 1,
                                            merkleMinLeavesCUDA: 512,
                                            preferCpuSha2MaxLeavesX86: 32)
        settings.apply() // ensure no crash for coverage
        let encoded = try JSONEncoder().encode(settings)
        let jsonObject = try JSONSerialization.jsonObject(with: encoded) as? [String: Any]
        XCTAssertEqual(jsonObject?["enable_metal"] as? Bool, false)
        XCTAssertEqual(jsonObject?["enable_cuda"] as? Bool, true)
        XCTAssertEqual(jsonObject?["max_gpus"] as? Int, 1)
        XCTAssertEqual(jsonObject?["merkle_min_leaves_cuda"] as? Int, 512)
        XCTAssertEqual(jsonObject?["prefer_cpu_sha2_max_leaves_x86"] as? Int, 32)
        XCTAssertNil(jsonObject?["merkleMinLeavesCUDA"])
    }

    func testLoadsFromIrohaConfigJSON() throws {
        let json = """
        {
            "torii": {
                "url": "http://127.0.0.1:8080"
            },
            "accel": {
                "enable_metal": false,
                "enable_cuda": true,
                "max_gpus": 0,
                "merkle_min_leaves_gpu": 16384,
                "prefer_cpu_sha2_max_leaves_x86": 0
            }
        }
        """
        let settings = try AccelerationSettings.fromIrohaConfig(Data(json.utf8))
        XCTAssertFalse(settings.enableMetal)
        XCTAssertTrue(settings.enableCUDA)
        XCTAssertNil(settings.maxGPUs) // 0 maps to nil/default
        XCTAssertEqual(settings.merkleMinLeavesGPU, 16_384)
        XCTAssertNil(settings.preferCpuSha2MaxLeavesX86)
    }

    func testLoadsAccelerationFromNestedJSON() throws {
        let json = """
        {
            "client": {
                "chain": "00000000-0000-0000-0000-000000000000",
                "telemetry": {},
                "advanced": {
                    "acceleration": {
                        "enable_metal": true,
                        "enable_cuda": false,
                        "merkle_min_leaves_cuda": 4096
                    }
                }
            }
        }
        """
        let settings = try AccelerationSettings.fromIrohaConfig(Data(json.utf8))
        XCTAssertTrue(settings.enableMetal)
        XCTAssertFalse(settings.enableCUDA)
        XCTAssertNil(settings.maxGPUs)
        XCTAssertEqual(settings.merkleMinLeavesCUDA, 4096)
    }

    func testLoadsFromIrohaConfigToml() throws {
        let toml = """
        chain = "00000000-0000-0000-0000-000000000000"

        [accel]
        enable_metal = false
        enable_cuda = true
        max_gpus = 2
        merkle_min_leaves_gpu = 8_192
        prefer_cpu_sha2_max_leaves_aarch64 = 0
        """
        let settings = try AccelerationSettings.fromIrohaConfig(Data(toml.utf8))
        XCTAssertFalse(settings.enableMetal)
        XCTAssertTrue(settings.enableCUDA)
        XCTAssertEqual(settings.maxGPUs, 2)
        XCTAssertEqual(settings.merkleMinLeavesGPU, 8_192)
        XCTAssertNil(settings.preferCpuSha2MaxLeavesAarch64)
    }

    func testIrohaConfigFallbacksToDefaultsWhenSectionMissing() throws {
        let config = """
        {
            "chain": "00000000-0000-0000-0000-000000000000",
            "torii": { "url": "http://127.0.0.1:8080" }
        }
        """
        let settings = try AccelerationSettings.fromIrohaConfig(Data(config.utf8))
        XCTAssertTrue(settings.enableMetal)
        XCTAssertFalse(settings.enableCUDA)
        XCTAssertNil(settings.maxGPUs)
        XCTAssertNil(settings.merkleMinLeavesMetal)
    }
}
