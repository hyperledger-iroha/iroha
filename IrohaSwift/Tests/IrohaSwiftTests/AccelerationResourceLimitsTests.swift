import XCTest
@testable import IrohaSwift

final class AccelerationResourceLimitsTests: XCTestCase {
    func testEnabledDefaultsAndAllFiniteNativeCeilings() {
        let policy = AccelerationSettings()
        XCTAssertTrue(policy.enableSIMD)
        XCTAssertTrue(policy.enableMetal)
        XCTAssertTrue(policy.enableCUDA)
        let limits = policy.resourceLimits
        XCTAssertEqual(limits.hostBytes, 268_435_456)
        XCTAssertEqual(limits.pinnedBytes, 268_435_456)
        XCTAssertEqual(limits.deviceBytes, 1_073_741_824)
        XCTAssertEqual(limits.inFlight, 16)
        XCTAssertEqual(limits.metadataBytes, 16_777_216)
        XCTAssertEqual(limits.observedDevices, 16)
        XCTAssertEqual(limits.discoveryOrdinals, 64)
        XCTAssertEqual(limits.modules, 304)
        XCTAssertEqual(limits.streams, 16)
        XCTAssertEqual(limits.artifactBytes, 16_777_216)
    }

    func testEveryExplicitZeroSurvivesJSONAndNativeRoundtrip() throws {
        let limits = AccelerationResourceLimits(hostBytes: 0, pinnedBytes: 0, deviceBytes: 0,
            inFlight: 0, metadataBytes: 0, observedDevices: 0, discoveryOrdinals: 0,
            modules: 0, streams: 0, artifactBytes: 0)
        let policy = AccelerationSettings(maxGPUs: 0, merkleMinLeavesGPU: 0,
            merkleMinLeavesMetal: 0, merkleMinLeavesCUDA: 0,
            preferCpuSha2MaxLeavesAarch64: 0, preferCpuSha2MaxLeavesX86: 0,
            resourceLimits: limits)
        let decoded = try AccelerationSettings.fromJSON(JSONEncoder().encode(policy))
        XCTAssertEqual(decoded.resourceLimits, limits)
        XCTAssertEqual(decoded.maxGPUs, 0)
        XCTAssertEqual(decoded.merkleMinLeavesGPU, 0)
        XCTAssertEqual(decoded.merkleMinLeavesMetal, 0)
        XCTAssertEqual(decoded.merkleMinLeavesCUDA, 0)
        XCTAssertEqual(decoded.preferCpuSha2MaxLeavesAarch64, 0)
        XCTAssertEqual(decoded.preferCpuSha2MaxLeavesX86, 0)
        #if canImport(Darwin)
        let native = AccelerationSettings(nativeConfig: decoded.nativeConfig)
        XCTAssertEqual(native.resourceLimits, limits)
        XCTAssertEqual(native.maxGPUs, 0)
        XCTAssertEqual(native.merkleMinLeavesGPU, 0)
        #endif
    }

    func testPartialNestedTOMLUsesDefaultMissingLimits() throws {
        let policy = try AccelerationSettings.fromIrohaConfig(Data("""
        [accel]
        max_gpus = 0
        [accel.resource_limits]
        host_bytes = 1_024
        device_bytes = 0
        discovery_ordinals = 3
        """.utf8))
        XCTAssertEqual(policy.maxGPUs, 0)
        XCTAssertEqual(policy.resourceLimits.hostBytes, 1024)
        XCTAssertEqual(policy.resourceLimits.deviceBytes, 0)
        XCTAssertEqual(policy.resourceLimits.discoveryOrdinals, 3)
        XCTAssertEqual(policy.resourceLimits.pinnedBytes, AccelerationResourceLimits().pinnedBytes)
    }

    func testTOMLIntegerRadicesAndCanonicalSeparators() throws {
        let policy = try AccelerationSettings.fromIrohaConfig(Data("""
        [accel.resource_limits]
        host_bytes = 0x4_00
        pinned_bytes = 0o2_000
        device_bytes = 0b100_00000000
        in_flight = +1_6
        """.utf8))
        XCTAssertEqual(policy.resourceLimits.hostBytes, 1024)
        XCTAssertEqual(policy.resourceLimits.pinnedBytes, 1024)
        XCTAssertEqual(policy.resourceLimits.deviceBytes, 1024)
        XCTAssertEqual(policy.resourceLimits.inFlight, 16)
        for value in ["01", "_1", "1_", "0x_1", "0b102", "0o8", "1__2"] {
            XCTAssertThrowsError(try AccelerationSettings.fromIrohaConfig(
                Data("[accel.resource_limits]\nhost_bytes = \(value)".utf8)))
        }
    }

    func testMalformedOrUnknownResourcePolicyRejectsBeforeDefaults() {
        for config in [#"{"accel":{"resource_limits":{"host_byte":1}}}"#,
                       #"{"accel":{"resource_limits":false}}"#,
                       "[accel.resource_limits]\nmodules = 1__2",
                       "[accel.resource_limits]\nstreams = 1\nstreams = 2",
                       "[accel.resource_limits]\nmetadata_bytes = -1",
                       #"{"accel":{"enable_cuda":false}"#,
                       "[accel\nenable_cuda = false", "[accel.unknown]\nfoo = 1"] {
            XCTAssertThrowsError(try AccelerationSettings.fromIrohaConfig(Data(config.utf8)), config)
        }
    }

    func testCanonicalQuotedAndWhitespaceTableComponents() throws {
        for header in ["[ accel ]", "[\"accel\"]", "['accel']", #"["ac\u0063el"]"#] {
            let policy = try AccelerationSettings.fromIrohaConfig(Data("\(header) # policy\n'enable_cuda' = false # opt-out".utf8))
            XCTAssertFalse(policy.enableCUDA, header)
        }
        for header in ["[ accel . resource_limits ]", "['accel'.\"resource_limits\"]", #"[accel . "resource_\U0000006cimits"]"#] {
            let policy = try AccelerationSettings.fromIrohaConfig(Data("\(header)\n\"device_bytes\" = 0".utf8))
            XCTAssertEqual(policy.resourceLimits.deviceBytes, 0, header)
        }
    }

    func testRootDottedInlineRetiredAndDuplicatePolicyCannotBecomeDefaults() {
        for config in ["accel.enable_cuda = false", "accel = { enable_cuda = false }",
                       "'accel'.resource_limits.device_bytes = 0", "'acceleration'.enable_cuda = false",
                       "[ 'acceleration' ]\nenable_cuda = false", #"["accel\u0065ration"]"#,
                       "[accel]\nenable_cuda = false\n['accel']\nenable_metal = false",
                       "[accel.resource_limits]\nmodules = 0\n[accel . 'resource_limits']\nstreams = 0",
                       "[accel]\nresource_limits.device_bytes = 0", "[[accel]]\nenable_cuda = false",
                       "[accel .]\nenable_cuda = false", "[accel..resource_limits]\ndevice_bytes = 0",
                       "[accel resource_limits]\ndevice_bytes = 0", "[accel]\n'enable_cuda' trailing = false",
                       "[accel]\n'enable_cuda' = false\nenable_cuda = true", "enable_cuda = false"] {
            XCTAssertThrowsError(try AccelerationSettings.fromIrohaConfig(Data(config.utf8)), config)
        }
    }

    func testUnrelatedKeysCommentsAndMultilineStringsRemainUnrelated() throws {
        let config = #"""
        name = "# [accel]"
        note = '''
        [acceleration]
        enable_cuda = false
        '''
        [other]
        enable_cuda = false
        resource_limits = { device_bytes = 0 }
        [other.acceleration]
        enable_cuda = false
        [ accel ] # actual policy
        enable_metal = false
        """#
        let policy = try AccelerationSettings.fromIrohaConfig(Data(config.utf8))
        XCTAssertTrue(policy.enableCUDA)
        XCTAssertFalse(policy.enableMetal)
        XCTAssertEqual(policy.resourceLimits.deviceBytes, AccelerationResourceLimits().deviceBytes)
    }

    func testImplicitParentMayFollowResourceTableOnce() throws {
        let policy = try AccelerationSettings.fromIrohaConfig(Data("[accel.resource_limits]\ndevice_bytes = 0\n[accel]\nenable_cuda = false".utf8))
        XCTAssertFalse(policy.enableCUDA)
        XCTAssertEqual(policy.resourceLimits.deviceBytes, 0)
    }

    #if canImport(Darwin)
    func testSingleNativeLayoutAndFullUnsignedWidthMapping() throws {
        XCTAssertEqual(MemoryLayout<ConnectNoritoAccelerationResourceLimits>.size, 80)
        XCTAssertEqual(MemoryLayout<ConnectNoritoAccelerationConfig>.size, 184)
        XCTAssertEqual(MemoryLayout<ConnectNoritoAccelerationState>.size, 256)
        XCTAssertEqual(MemoryLayout<ConnectNoritoAccelerationConfig>.offset(of: \.resource_limits), 104)
        let limits = AccelerationResourceLimits(hostBytes: .max, pinnedBytes: 2, deviceBytes: 3,
            inFlight: 4, metadataBytes: 5, observedDevices: 6, discoveryOrdinals: .max,
            modules: 8, streams: 9, artifactBytes: 10)
        let policy = AccelerationSettings(maxGPUs: .max, resourceLimits: limits)
        let decoded = AccelerationSettings(nativeConfig: policy.nativeConfig)
        XCTAssertEqual(decoded.resourceLimits, limits)
        XCTAssertEqual(decoded.maxGPUs, UInt64.max)
        XCTAssertNil(decoded.merkleMinLeavesGPU)
        let json = try AccelerationSettings.fromJSON(JSONEncoder().encode(policy))
        XCTAssertEqual(json.resourceLimits, limits)
        XCTAssertEqual(json.maxGPUs, UInt64.max)
    }
    #endif
}
