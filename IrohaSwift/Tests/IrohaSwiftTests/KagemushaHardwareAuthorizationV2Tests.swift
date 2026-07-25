import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaHardwareAuthorizationV2Tests: XCTestCase {
    func testHardwareAuthorizationAbiInventoryIsExactAndHasNoCompatibilityFinalizer() {
        let strictSymbols = [
            "connect_norito_kagemusha_request_authorization_finalize_hardware_v2",
            "connect_norito_kagemusha_request_authorization_finalize_ios_app_attest_v2",
        ]
        for symbol in strictSymbols {
            XCTAssertTrue(KagemushaRecursiveSpend.requiredProtocolSymbols.contains(symbol))
        }
        XCTAssertFalse(KagemushaRecursiveSpend.requiredProtocolSymbols.contains(
            "connect_norito_kagemusha_request_authorization_create_v2"
        ))
    }

    func testPreparationArchiveIsNotAnAuthorizationAndBindsPlatform() throws {
        let android = try fields(platform: .androidKeyMint)
        let ios = try fields(platform: .iosAppAttest)
        let vector = try hardwareVector()
        let androidArchive = try KagemushaRecursiveSpendCodecs
            .encodeAuthorizationPreparation(android)
        let iosArchive = try KagemushaRecursiveSpendCodecs
            .encodeAuthorizationPreparation(ios)

        XCTAssertEqual(
            androidArchive,
            try hex(try XCTUnwrap(vector["android_preparation"]))
        )
        XCTAssertEqual(
            iosArchive,
            try hex(try XCTUnwrap(vector["ios_preparation"]))
        )
        XCTAssertNotEqual(androidArchive, iosArchive)
        XCTAssertNoThrow(try KagemushaRecursiveSpend.requireArchive(
            androidArchive,
            schema: KagemushaRecursiveSpend.authorizationPreparationWireName,
            field: "authorizationPreparation"
        ))
        XCTAssertThrowsError(try KagemushaRecursiveSpend.requireArchive(
            androidArchive,
            schema: KagemushaRecursiveSpend.authorizationWireName,
            field: "authorization"
        ))
    }

    func testFieldsRejectRetiredOrUnboundAuthorizationInputs() throws {
        let valid = try fields(platform: .androidKeyMint)
        XCTAssertEqual(valid.assetDefinitionID, assetDefinitionID())
        XCTAssertEqual(valid.registrationHash, try registrationHash())

        XCTAssertThrowsError(try KagemushaRequestAuthorizationFields(
            authority: valid.authority,
            deviceID: valid.deviceID,
            assetDefinitionID: "not-an-asset-definition",
            operationID: valid.operationID,
            issuedAtMilliseconds: valid.issuedAtMilliseconds,
            expiresAtMilliseconds: valid.expiresAtMilliseconds,
            nonce: valid.nonce,
            payloadDigest: valid.payloadDigest,
            registrationHash: valid.registrationHash,
            platform: valid.platform
        ))
        XCTAssertThrowsError(try KagemushaRequestAuthorizationFields(
            authority: valid.authority,
            deviceID: valid.deviceID,
            assetDefinitionID: valid.assetDefinitionID,
            operationID: valid.operationID,
            issuedAtMilliseconds: valid.issuedAtMilliseconds,
            expiresAtMilliseconds: valid.expiresAtMilliseconds,
            nonce: valid.nonce,
            payloadDigest: valid.payloadDigest,
            registrationHash: Data(repeating: 0, count: 32),
            platform: valid.platform
        ))
    }

    func testPreparationRejectsCrossPlatformFinalizeAndMalformedIosAuthData() throws {
        let android = try preparation(platform: .androidKeyMint)
        let ios = try preparation(platform: .iosAppAttest)
        let der = try P256.Signing.PrivateKey()
            .signature(for: Data("authorization".utf8))
            .derRepresentation

        XCTAssertThrowsError(try android.finalizeIosAppAttest(
            authenticatorData: legacyAuthenticatorData(counter: 1),
            derSignature: der
        ))
        XCTAssertThrowsError(try ios.finalizeAndroidKeyMint(derSignature: der))
        XCTAssertThrowsError(try android.finalizeIosAppAttest(
            assertionObject: Data([0xa0])
        ))
        XCTAssertThrowsError(try ios.finalizeIosAppAttest(assertionObject: Data()))
        XCTAssertThrowsError(try ios.finalizeIosAppAttest(
            assertionObject: Data(
                repeating: 0,
                count: KagemushaRecursiveSpend
                    .maximumIosAppAttestAssertionObjectBytesV2 + 1
            )
        ))

        for authenticatorData in [
            Data(repeating: 0, count: 36),
            Data(repeating: 0, count: 38),
            authenticatorData(flags: 0x01, counter: 1),
            authenticatorData(flags: 0x80, counter: 1),
        ] {
            XCTAssertThrowsError(try ios.finalizeIosAppAttest(
                authenticatorData: authenticatorData,
                derSignature: der
            ))
        }
    }

    func testTypedAssertionRetainsCanonicalLowSSignature() throws {
        let privateKey = P256.Signing.PrivateKey()
        let generated = try privateKey.signature(for: Data("hardware assertion".utf8))
        let signature = try KagemushaDeviceSignatureV2(
            derBytes: generated.derRepresentation
        )
        let android = KagemushaOnlineHardwareAssertion.androidKeyMint(
            signature: signature
        )
        let ios = KagemushaOnlineHardwareAssertion.iosAppAttest(
            authenticatorData: legacyAuthenticatorData(counter: 1),
            signature: signature
        )

        XCTAssertEqual(android.platform, .androidKeyMint)
        XCTAssertNil(android.authenticatorData)
        XCTAssertEqual(android.signature, signature)
        XCTAssertEqual(ios.platform, .iosAppAttest)
        XCTAssertEqual(ios.authenticatorData, legacyAuthenticatorData(counter: 1))
        XCTAssertEqual(ios.signature, signature)
    }

    #if canImport(DeviceCheck)
    @available(iOS 15.0, macOS 12.0, *)
    func testPhysicalAppAttestEntryPointRejectsSubstitutionBeforeHardwareCall() async throws {
        let android = try preparation(platform: .androidKeyMint)
        let ios = try preparation(platform: .iosAppAttest)

        do {
            _ = try await android.authorizeWithIosAppAttest(keyId: "YQ==")
            XCTFail("Android preparation reached the iOS hardware service")
        } catch let error as KagemushaRecursiveSpendError {
            XCTAssertEqual(error, .invalidField("authorization.platform"))
        }
        do {
            _ = try await ios.authorizeWithIosAppAttest(keyId: "not canonical base64")
            XCTFail("Malformed App Attest key id reached the hardware service")
        } catch let error as KagemushaRecursiveSpendError {
            XCTAssertEqual(
                error,
                .invalidField("authorization.appAttest.keyId")
            )
        }
    }
    #endif

    private func fields(
        platform: KagemushaOnlineHardwareAssertionPlatform
    ) throws -> KagemushaRequestAuthorizationFields {
        let authority = try AccountAddress
            .fromAccount(publicKey: authorityPublicKey())
            .toI105(networkPrefix: 0x02F1)
        return try KagemushaRequestAuthorizationFields(
            authority: authority,
            deviceID: "physical-device-1",
            assetDefinitionID: assetDefinitionID(),
            operationID: fixed32(0x31),
            issuedAtMilliseconds: 1_000,
            expiresAtMilliseconds: 2_000,
            nonce: fixed32(0x32),
            payloadDigest: fixed32(0x33),
            registrationHash: registrationHash(),
            platform: platform
        )
    }

    private func preparation(
        platform: KagemushaOnlineHardwareAssertionPlatform
    ) throws -> KagemushaRequestAuthorizationPreparation {
        let fields = try fields(platform: platform)
        let vector = try hardwareVector()
        return try KagemushaRequestAuthorizationPreparation(
            fields: fields,
            preparationArchive: KagemushaRecursiveSpendCodecs
                .encodeAuthorizationPreparation(fields),
            signingBytes: platform == .iosAppAttest
                ? hex(try XCTUnwrap(vector["ios_client_data_hash"]))
                : hex(try XCTUnwrap(vector["android_signing_preimage"]))
        )
    }

    private func hardwareVector() throws -> [String: String] {
        let repositoryRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let path = repositoryRoot
            .appendingPathComponent("crates/connect_norito_bridge/tests/fixtures")
            .appendingPathComponent("kagemusha_request_authorization_v2_hardware.hex")
        let raw = try String(contentsOf: path, encoding: .utf8)
        var values: [String: String] = [:]
        for line in raw.split(separator: "\n") {
            if line.isEmpty || line.hasPrefix("#") {
                continue
            }
            let parts = line.split(separator: "=", maxSplits: 1)
            guard parts.count == 2, !parts[0].isEmpty, !parts[1].isEmpty else {
                throw KagemushaRecursiveSpendError.invalidArchive(
                    "hardwareAuthorizationVector"
                )
            }
            values[String(parts[0])] = String(parts[1])
        }
        XCTAssertEqual(Set(values.keys), [
            "authority_public_key",
            "registration_hash",
            "android_preparation",
            "android_signing_preimage",
            "ios_preparation",
            "ios_client_data_hash",
        ])
        XCTAssertEqual(
            try hex(try XCTUnwrap(values["authority_public_key"])),
            try authorityPublicKey()
        )
        XCTAssertEqual(
            try hex(try XCTUnwrap(values["registration_hash"])),
            try registrationHash()
        )
        return values
    }

    private func authorityPublicKey() throws -> Data {
        try hex("a09aa5f47a6759802ff955f8dc2d2a14a5c99d23be97f864127ff9383455a4f0")
    }

    private func registrationHash() throws -> Data {
        try hex("289ab8f0dcaad32e86ab947b6bd48a3a63385b4d52b85f09f54260ad106d00c3")
    }

    private func hex(_ value: String) throws -> Data {
        try XCTUnwrap(Data(hexString: value))
    }

    private func legacyAuthenticatorData(counter: UInt32) -> Data {
        authenticatorData(flags: 0, counter: counter)
    }

    private func authenticatorData(flags: UInt8, counter: UInt32) -> Data {
        var value = fixed32(0x77)
        value.append(flags)
        withUnsafeBytes(of: counter.bigEndian) { value.append(contentsOf: $0) }
        return value
    }

    private func assetDefinitionID() -> String {
        var bytes = Data((0..<16).map { UInt8($0 + 1) })
        bytes[6] = (bytes[6] & 0x0f) | 0x40
        bytes[8] = (bytes[8] & 0x3f) | 0x80
        return AssetDefinitionAddress.encode(uuidBytes: bytes)!
    }

    private func fixed32(_ byte: UInt8) -> Data {
        Data(repeating: byte, count: 32)
    }
}
