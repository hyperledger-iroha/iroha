import Foundation
import XCTest
@testable import IrohaSwift

final class OfflineDeviceEligibilityV1Tests: XCTestCase {
    private func be<T: FixedWidthInteger>(_ value: T) -> [UInt8] {
        withUnsafeBytes(of: value.bigEndian, Array.init)
    }

    private func eligibleProjection(registrationHash: Data) -> Data {
        let issuer = Data([0x91, 0x01])
        let credential = Data([0x92, 0x02, 0x03])
        let policy = Data([0x93, 0x04])
        let account = Data("account@test".utf8)
        let device = Data("device-1".utf8)
        let key = Data("attestation-key-1".utf8)
        let devicePublicKey = Data([0x04] + Array(repeating: 0x21, count: 64))
        let assertionPublicKey = Data([0x04] + Array(repeating: 0x31, count: 64))
        let claimSections = [account, device, key, devicePublicKey, assertionPublicKey]
        let claims = claimSections.reduce(into: Data(), { $0.append($1) })

        var result = OfflineDeviceEligibilityResponseV1.projectionMagic
        result.append(contentsOf: [0, 0, 1, 0])
        result.append(contentsOf: be(UInt64(42)))
        result.append(registrationHash)
        result.append(Data(repeating: 0x43, count: 32))
        result.append(Data(repeating: 0x45, count: 32))
        result.append(contentsOf: be(UInt16(0)))
        result.append(contentsOf: [0, 0])
        result.append(contentsOf: be(UInt32(0)))
        result.append(contentsOf: be(UInt32(issuer.count)))
        result.append(contentsOf: be(UInt32(credential.count)))
        result.append(contentsOf: be(UInt32(policy.count)))
        result.append(contentsOf: be(UInt64(7)))
        result.append(Data(repeating: 0x47, count: 32))
        result.append(contentsOf: be(UInt64(2_000_000)))
        result.append(contentsOf: be(UInt64(44)))
        result.append(Data(repeating: 0x49, count: 32))
        result.append(contentsOf: be(UInt64(1_000_000)))
        result.append(Data(repeating: 0x4b, count: 32))
        result.append(contentsOf: be(UInt64(1_100_000)))
        result.append(contentsOf: be(UInt64(1_200_000)))
        for section in claimSections {
            result.append(contentsOf: be(UInt16(section.count)))
        }
        result.append(contentsOf: [0, 0])
        result.append(contentsOf: be(UInt32(claims.count)))
        result.append(issuer)
        result.append(credential)
        result.append(policy)
        result.append(claims)
        return result
    }

    func testTypedEligibleProjectionOwnsPublicArchivesAndClaims() throws {
        let network = try NetworkId(bytes: Data(repeating: 0x11, count: 32))
        let trust = try OfflineDeviceFinalityTrustAnchorV1(
            networkId: network,
            trustedHeightContextId: Data(repeating: 0x23, count: 32)
        )
        let registration = Data(repeating: 0x41, count: 32)
        let response = try OfflineDeviceEligibilityResponseV1(
            nativeProjection: eligibleProjection(registrationHash: registration),
            responseArchive: Data([0x90]),
            expectedRegistrationHash: registration,
            trustAnchor: trust
        )
        XCTAssertEqual(response.decision.outcome, .eligible)
        XCTAssertEqual(response.decision.reason, .policySatisfied)
        XCTAssertNotNil(response.credential)
        XCTAssertEqual(response.credentialClaims?.accountId, "account@test")
        XCTAssertEqual(response.credentialClaims?.deviceId, "device-1")
        XCTAssertEqual(response.credentialClaims?.devicePublicKey.count, 65)
        XCTAssertEqual(response.policyClaims.policyEpoch, 7)
        XCTAssertEqual(response.policyClaims.finality.finalizedBlockHeight, 44)
        XCTAssertEqual(response.admission.admissionHeight, 42)
        XCTAssertEqual(response.noritoArchive, Data([0x90]))

        XCTAssertThrowsError(try OfflineDeviceEligibilityResponseV1(
            nativeProjection: eligibleProjection(registrationHash: registration),
            responseArchive: Data([0x90]),
            expectedRegistrationHash: Data(repeating: 0x51, count: 32),
            trustAnchor: trust
        ))
    }

    func testRequestAndProductionSurfaceFailClosed() throws {
        let request = try OfflineDeviceEligibilityRequestV1(
            registrationHash: Data(repeating: 0x41, count: 32),
            deviceId: "device-1",
            attestationKeyId: "attestation-key-1",
            requestedTtlMilliseconds: 60_000
        )
        XCTAssertEqual(request.requestedTtlMilliseconds, 60_000)
        XCTAssertThrowsError(try OfflineDeviceEligibilityRequestV1(
            registrationHash: Data(repeating: 0, count: 32),
            deviceId: "device-1",
            attestationKeyId: "attestation-key-1",
            requestedTtlMilliseconds: 60_000
        ))
        XCTAssertThrowsError(try OfflineDeviceEligibilityRequestV1(
            registrationHash: Data(repeating: 0x41, count: 32),
            deviceId: "device\u{0007}-1",
            attestationKeyId: "attestation-key-1",
            requestedTtlMilliseconds: 60_000
        ))
        XCTAssertThrowsError(try OfflineDeviceEligibilityRequestV1(
            registrationHash: Data(repeating: 0x41, count: 32),
            deviceId: "device-1",
            attestationKeyId: "attestation\u{0000}-key-1",
            requestedTtlMilliseconds: 60_000
        ))

        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let torii = try String(
            contentsOf: root.appendingPathComponent("Sources/IrohaSwift/ToriiClient.swift"),
            encoding: .utf8
        )
        let native = try String(
            contentsOf: root.appendingPathComponent(
                "Sources/IrohaSwift/KagemushaRecursiveSpendV2Native.swift"
            ),
            encoding: .utf8
        )
        XCTAssertTrue(torii.contains("postOfflineDeviceEligibilityV1("))
        XCTAssertTrue(torii.contains("expectedIssuer:"))
        XCTAssertTrue(torii.contains("/v1/offline/device-eligibility"))
        XCTAssertTrue(torii.contains("makeCanonicalAccountRequest("))
        XCTAssertTrue(native.contains(
            "connect_norito_offline_device_eligibility_response_verify_v1"
        ))
        XCTAssertTrue(native.contains(
            "connect_norito_offline_device_eligibility_peer_certificate_verify_v1"
        ))
    }
}
