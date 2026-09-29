import Foundation
import XCTest
@testable import IrohaSwift

final class RamLfeProgramProfileTests: XCTestCase {
    private let descriptor = String(repeating: "ab", count: 31) + "01"

    private func profileData(descriptor: String?, overrides: [String: Any] = [:]) throws -> Data {
        var profile: [String: Any] = [
            "profile_version": 1,
            "register_count": 4,
            "memory_lane_count": 32,
            "ciphertext_mul_per_step": 16,
            "encrypted_input_mode": "encrypted_envelope_v1",
            "min_ciphertext_modulus": UInt64(1) << 52
        ]
        if let descriptor {
            profile["initializer_descriptor_hash"] = descriptor
        }
        profile.merge(overrides) { _, supplied in supplied }
        return try JSONSerialization.data(withJSONObject: profile)
    }

    func testRequiredInitializerDescriptorSurvivesRoundtrip() throws {
        let profile = try JSONDecoder().decode(
            ToriiIdentifierRamFheProfile.self,
            from: profileData(descriptor: descriptor)
        )
        XCTAssertEqual(profile.initializerDescriptorHash, descriptor)
        XCTAssertEqual(profile.registerCount, 4)
        XCTAssertEqual(profile.memoryLaneCount, 32)
        let encoded = try JSONEncoder().encode(profile)
        let decoded = try JSONDecoder().decode(ToriiIdentifierRamFheProfile.self, from: encoded)
        XCTAssertEqual(decoded.initializerDescriptorHash, descriptor)
        XCTAssertEqual(decoded.minCiphertextModulus, UInt64(1) << 52)
    }

    func testMissingOrNoncanonicalInitializerDescriptorIsRejected() throws {
        let invalid: [String?] = [
            nil,
            "",
            String(descriptor.dropLast()),
            descriptor.uppercased(),
            " " + descriptor,
            "0x" + descriptor,
            "hash:" + descriptor,
            String(repeating: "ab", count: 31) + "00"
        ]
        for value in invalid {
            XCTAssertThrowsError(try JSONDecoder().decode(
                ToriiIdentifierRamFheProfile.self,
                from: profileData(descriptor: value)
            ), "must reject \(String(describing: value))")
        }
    }

    func testDimensionsAndSoleInputModeAreStrict() throws {
        let invalid: [(String, Any)] = [
            ("profile_version", 0), ("profile_version", 256),
            ("register_count", 0), ("register_count", 65536),
            ("memory_lane_count", 0), ("memory_lane_count", 65536),
            ("ciphertext_mul_per_step", 0), ("ciphertext_mul_per_step", 256),
            ("min_ciphertext_modulus", 0), ("min_ciphertext_modulus", -1),
            ("encrypted_input_mode", "EncryptedEnvelopeV1"),
            ("encrypted_input_mode", " encrypted_envelope_v1"),
            ("encrypted_input_mode", "resolver_canonicalized_envelope_v1"),
            ("encrypted_input_mode", ["mode": "encrypted_envelope_v1"]),
            ("retired_initializer", "chacha20")
        ]
        for (field, value) in invalid {
            XCTAssertThrowsError(try JSONDecoder().decode(
                ToriiIdentifierRamFheProfile.self,
                from: profileData(descriptor: descriptor, overrides: [field: value])
            ), "must reject invalid \(field)")
        }
    }
}
