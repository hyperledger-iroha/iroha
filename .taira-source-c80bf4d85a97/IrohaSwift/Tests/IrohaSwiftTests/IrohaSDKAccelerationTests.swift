@testable import IrohaSwift
import XCTest

final class IrohaSDKAccelerationTests: XCTestCase {
    func testInitializationAppliesAccelerationSettings() {
        let settings = AccelerationSettings(enableMetal: false,
                                            enableCUDA: true,
                                            maxGPUs: 2)
        let sdk = IrohaSDK(baseURL: URL(string: "http://127.0.0.1")!, accelerationSettings: settings)
        XCTAssertEqual(sdk.accelerationSettings.enableMetal, false)
        XCTAssertEqual(sdk.accelerationSettings.enableCUDA, true)
        XCTAssertEqual(sdk.accelerationSettings.maxGPUs, 2)
    }

    func testChangingSettingsAppliesImmediately() {
        let sdk = IrohaSDK(baseURL: URL(string: "http://127.0.0.1")!)
        sdk.accelerationSettings = AccelerationSettings(enableMetal: false)
        XCTAssertFalse(sdk.accelerationSettings.enableMetal)
    }

    #if canImport(Darwin)
    func testDecodingNativeAccelerationConfig() {
        let native = ConnectNoritoAccelerationConfig(
            enable_simd: 1,
            enable_metal: 1,
            enable_cuda: 0,
            max_gpus: 3,
            max_gpus_present: 1,
            merkle_min_leaves_gpu: 4096,
            merkle_min_leaves_gpu_present: 1,
            merkle_min_leaves_metal: 2048,
            merkle_min_leaves_metal_present: 1,
            merkle_min_leaves_cuda: 0,
            merkle_min_leaves_cuda_present: 0,
            prefer_cpu_sha2_max_leaves_aarch64: 1024,
            prefer_cpu_sha2_max_leaves_aarch64_present: 1,
            prefer_cpu_sha2_max_leaves_x86: 0,
            prefer_cpu_sha2_max_leaves_x86_present: 0
        )
        let decoded = AccelerationSettings(nativeConfig: native)
        XCTAssertTrue(decoded.enableMetal)
        XCTAssertFalse(decoded.enableCUDA)
        XCTAssertTrue(decoded.enableSIMD)
        XCTAssertEqual(decoded.maxGPUs, 3)
        XCTAssertEqual(decoded.merkleMinLeavesGPU, 4096)
        XCTAssertEqual(decoded.merkleMinLeavesMetal, 2048)
        XCTAssertNil(decoded.merkleMinLeavesCUDA)
        XCTAssertEqual(decoded.preferCpuSha2MaxLeavesAarch64, 1024)
        XCTAssertNil(decoded.preferCpuSha2MaxLeavesX86)
    }

    func testDecodingNativeAccelerationState() {
        let errorMessage = "metal disabled by policy"
        let errorBytes = Array(errorMessage.utf8)
        let errorPtr = UnsafeMutablePointer<UInt8>.allocate(capacity: errorBytes.count)
        errorPtr.initialize(from: errorBytes, count: errorBytes.count)
        defer { errorPtr.deallocate() }

        let state = ConnectNoritoAccelerationState(
            config: ConnectNoritoAccelerationConfig(
                enable_simd: 1,
                enable_metal: 1,
                enable_cuda: 1,
                max_gpus: 1,
                max_gpus_present: 1,
                merkle_min_leaves_gpu: 0,
                merkle_min_leaves_gpu_present: 0,
                merkle_min_leaves_metal: 8192,
                merkle_min_leaves_metal_present: 1,
                merkle_min_leaves_cuda: 8192,
                merkle_min_leaves_cuda_present: 1,
                prefer_cpu_sha2_max_leaves_aarch64: 32768,
                prefer_cpu_sha2_max_leaves_aarch64_present: 1,
                prefer_cpu_sha2_max_leaves_x86: 65536,
                prefer_cpu_sha2_max_leaves_x86_present: 1
            ),
            simd: ConnectNoritoAccelerationBackendStatus(supported: 1,
                                                         configured: 1,
                                                         available: 1,
                                                         parity_ok: 1,
                                                         last_error_ptr: nil,
                                                         last_error_len: 0),
            metal: ConnectNoritoAccelerationBackendStatus(supported: 1,
                                                         configured: 1,
                                                         available: 1,
                                                         parity_ok: 1,
                                                         last_error_ptr: errorPtr,
                                                         last_error_len: UInt(errorBytes.count)),
            cuda: ConnectNoritoAccelerationBackendStatus(supported: 0,
                                                         configured: 1,
                                                         available: 0,
                                                         parity_ok: 0,
                                                         last_error_ptr: nil,
                                                         last_error_len: 0)
        )
        let decoded = AccelerationState(nativeState: state)
        XCTAssertEqual(decoded.settings.maxGPUs, 1)
        XCTAssertEqual(decoded.settings.merkleMinLeavesMetal, 8192)
        XCTAssertTrue(decoded.simd.supported)
        XCTAssertTrue(decoded.simd.available)
        XCTAssertTrue(decoded.simd.parityOK)
        XCTAssertTrue(decoded.metal.supported)
        XCTAssertTrue(decoded.metal.available)
        XCTAssertTrue(decoded.metal.parityOK)
        XCTAssertEqual(decoded.metal.lastError, errorMessage)
        XCTAssertFalse(decoded.cuda.supported)
        XCTAssertFalse(decoded.cuda.available)
        XCTAssertFalse(decoded.cuda.parityOK)
        XCTAssertNil(decoded.cuda.lastError)
    }
    #endif
}
