import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaRedemptionChangeV4Tests: XCTestCase {
    func testPrepareRequestUsesExactBridgeSchemaFieldOrderAndAlignment() throws {
        let bundle = try opaqueBundle()
        let opening = try noteOpening(seed: 0x20)
        let amount = try KagemushaScaledAmount(atomicUnits: "25", scale: 2)
        let operationID = fixed32(0x61)
        let entropy = fixed32(0x62)
        let request = try KagemushaRecursiveSpendRedemptionChangePrepareRequestV4(
            bundle: bundle,
            inputOpening: opening,
            changeAmount: amount,
            operationID: operationID,
            entropy: entropy
        )

        let archive = try KagemushaRecursiveSpendCodecsV4
            .encodeRedemptionChangePrepareRequest(request)
        let frame = try XCTUnwrap(noritoDecodeFrame(archive))
        XCTAssertEqual(
            frame.header.schema,
            noritoSchemaHash(
                forTypeName: KagemushaRecursiveSpend
                    .redemptionChangePrepareRequestWireNameV4
            )
        )
        XCTAssertEqual(
            KagemushaRecursiveSpend.archivedPayloadAlignment(
                forWireName: KagemushaRecursiveSpend
                    .redemptionChangePrepareRequestWireNameV4
            ),
            16
        )
        XCTAssertEqual(frame.paddingLength, 8)

        var reader = CanonicalNoritoReader(data: frame.payload)
        XCTAssertEqual(try reader.readCompactField(), uint16(4))
        XCTAssertEqual(
            try reader.readCompactField(),
            try XCTUnwrap(noritoDecodeFrame(bundle.noritoArchive)).payload
        )
        XCTAssertEqual(
            try reader.readCompactField(),
            try XCTUnwrap(noritoDecodeFrame(opening.noritoEncoded())).payload
        )
        var expectedAtomicUnits = Data([25])
        expectedAtomicUnits.append(contentsOf: repeatElement(UInt8(0), count: 15))
        XCTAssertEqual(
            try reader.readCompactField(),
            fields([expectedAtomicUnits, uint32(2)])
        )
        XCTAssertEqual(try reader.readCompactField(), operationID)
        XCTAssertEqual(try reader.readCompactField(), entropy)
        XCTAssertEqual(reader.remaining(), 0)
    }

    func testPrepareRequestRejectsZeroOrReusedDerivationIdentifiers() throws {
        let bundle = try opaqueBundle()
        let opening = try noteOpening(seed: 0x20)
        let amount = try KagemushaScaledAmount(atomicUnits: "25", scale: 2)
        let operationID = fixed32(0x61)

        XCTAssertThrowsError(try KagemushaRecursiveSpendRedemptionChangePrepareRequestV4(
            bundle: bundle,
            inputOpening: opening,
            changeAmount: amount,
            operationID: Data(repeating: 0, count: 32),
            entropy: fixed32(0x62)
        ))
        XCTAssertThrowsError(try KagemushaRecursiveSpendRedemptionChangePrepareRequestV4(
            bundle: bundle,
            inputOpening: opening,
            changeAmount: amount,
            operationID: operationID,
            entropy: operationID
        ))
    }

    func testPrepareResultStrictlyDecodesReencodesAndBindsCompleteOutput() throws {
        let inputOpening = try noteOpening(seed: 0x20)
        let summary = try inputSummary(amount: "100")
        let changeAmount = try KagemushaScaledAmount(atomicUnits: "25", scale: 2)
        let preparation = try preparation(
            inputOpening: inputOpening,
            summary: summary,
            changeAmount: changeAmount
        )
        let archive = try KagemushaRecursiveSpendCodecs
            .encodeRedemptionChangePrepareResultV4(preparation)

        let frame = try XCTUnwrap(noritoDecodeFrame(archive))
        XCTAssertEqual(
            frame.header.schema,
            noritoSchemaHash(
                forTypeName: KagemushaRecursiveSpend
                    .redemptionChangePrepareResultWireNameV4
            )
        )
        XCTAssertEqual(frame.paddingLength, 8)
        var reader = CanonicalNoritoReader(data: frame.payload)
        XCTAssertEqual(try reader.readCompactField(), uint16(4))
        XCTAssertEqual(
            try reader.readCompactField(),
            try XCTUnwrap(noritoDecodeFrame(preparation.opening.noritoEncoded())).payload
        )
        XCTAssertFalse(try reader.readCompactField().isEmpty)
        XCTAssertEqual(reader.remaining(), 0)

        let decoded = try KagemushaRecursiveSpendCodecs
            .decodeRedemptionChangePrepareResultV4(
                archive,
                inputOpening: inputOpening,
                inputSummary: summary,
                changeAmount: changeAmount
            )
        XCTAssertEqual(decoded, preparation)
        XCTAssertEqual(decoded.output.networkID, TestNetworkIds.canonical)
        XCTAssertEqual(decoded.output.assetDefinitionID, summary.assetDefinitionID)
        XCTAssertEqual(decoded.output.amount, changeAmount)
        XCTAssertEqual(
            decoded.publicAmount,
            try KagemushaScaledAmount(atomicUnits: "75", scale: 2)
        )
        XCTAssertEqual(
            try decoded.publicAmount.adding(decoded.output.amount),
            summary.amount
        )
        XCTAssertEqual(decoded.opening.spendKey, inputOpening.spendKey)
        XCTAssertNotEqual(decoded.opening.rho, inputOpening.rho)
        XCTAssertEqual(
            decoded.opening.diversifier,
            try ConfidentialOwnerTag.defaultDiversifier()
        )

        var extended = archive
        extended.append(0)
        XCTAssertThrowsError(try KagemushaRecursiveSpendCodecs
            .decodeRedemptionChangePrepareResultV4(
                extended,
                inputOpening: inputOpening,
                inputSummary: summary,
                changeAmount: changeAmount
            ))

        var canonicalReader = CanonicalNoritoReader(data: frame.payload)
        _ = try canonicalReader.readCompactField()
        let openingField = try canonicalReader.readCompactField()
        let outputField = try canonicalReader.readCompactField()
        let wrongVersion = KagemushaRecursiveSpend.frameArchive(
            schema: KagemushaRecursiveSpend.redemptionChangePrepareResultWireNameV4,
            payload: fields([uint16(3), openingField, outputField])
        )
        XCTAssertThrowsError(try KagemushaRecursiveSpendCodecs
            .decodeRedemptionChangePrepareResultV4(
                wrongVersion,
                inputOpening: inputOpening,
                inputSummary: summary,
                changeAmount: changeAmount
            ))
    }

    func testPrepareResultRejectsAmountAndInputOpeningSubstitution() throws {
        let inputOpening = try noteOpening(seed: 0x20)
        let summary = try inputSummary(amount: "100")
        let encodedAmount = try KagemushaScaledAmount(atomicUnits: "30", scale: 2)
        let encoded = try KagemushaRecursiveSpendCodecs
            .encodeRedemptionChangePrepareResultV4(try preparation(
                inputOpening: inputOpening,
                summary: summary,
                changeAmount: encodedAmount
            ))

        XCTAssertThrowsError(try KagemushaRecursiveSpendCodecs
            .decodeRedemptionChangePrepareResultV4(
                encoded,
                inputOpening: inputOpening,
                inputSummary: summary,
                changeAmount: KagemushaScaledAmount(atomicUnits: "25", scale: 2)
            ))
        XCTAssertThrowsError(try KagemushaRecursiveSpendCodecs
            .decodeRedemptionChangePrepareResultV4(
                encoded,
                inputOpening: noteOpening(seed: 0x40),
                inputSummary: summary,
                changeAmount: encodedAmount
            ))
    }

    func testWorkflowValidationRequiresSmallerSameScaleChangeAndFreshIDs() throws {
        let summary = try inputSummary(amount: "100")
        let operationID = fixed32(0x61)
        let entropy = fixed32(0x62)
        XCTAssertNoThrow(try KagemushaRecursiveSpend.validateRedemptionChangeV4(
            inputSummary: summary,
            changeAmount: KagemushaScaledAmount(atomicUnits: "99", scale: 2),
            operationID: operationID,
            entropy: entropy
        ))
        for amount in ["100", "101"] {
            XCTAssertThrowsError(try KagemushaRecursiveSpend.validateRedemptionChangeV4(
                inputSummary: summary,
                changeAmount: KagemushaScaledAmount(atomicUnits: amount, scale: 2),
                operationID: operationID,
                entropy: entropy
            ))
        }
        XCTAssertThrowsError(try KagemushaRecursiveSpend.validateRedemptionChangeV4(
            inputSummary: summary,
            changeAmount: KagemushaScaledAmount(atomicUnits: "99", scale: 3),
            operationID: operationID,
            entropy: entropy
        ))
        XCTAssertThrowsError(try KagemushaRecursiveSpend.validateRedemptionChangeV4(
            inputSummary: summary,
            changeAmount: KagemushaScaledAmount(atomicUnits: "99", scale: 2),
            operationID: operationID,
            entropy: operationID
        ))
        XCTAssertThrowsError(try KagemushaRecursiveSpend.validateRedemptionChangeV4(
            inputSummary: summary,
            changeAmount: KagemushaScaledAmount(atomicUnits: "99", scale: 2),
            operationID: Data(repeating: 0, count: 32),
            entropy: entropy
        ))
    }

    func testRequiredNativeInventoryIncludesPrepareAndSecretFree() {
        XCTAssertEqual(KagemushaRecursiveSpend.requiredProofSymbols.count, 4)
        XCTAssertEqual(KagemushaRecursiveSpend.requiredProtocolSymbols.count, 44)
        XCTAssertEqual(KagemushaRecursiveSpend.requiredNativeSymbols.count, 48)
        XCTAssertTrue(KagemushaRecursiveSpend.requiredProtocolSymbols.contains(
            "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4"
        ))
        XCTAssertTrue(KagemushaRecursiveSpend.requiredProtocolSymbols.contains(
            "connect_norito_kagemusha_secret_free_buffer"
        ))
        XCTAssertFalse(KagemushaRecursiveSpend.requiredProtocolSymbols.contains(
            "connect_norito_kagemusha_recipient_registration_lineage_verify_v1"
        ))
        XCTAssertFalse(KagemushaRecursiveSpend.requiredProtocolSymbols.contains(
            "connect_norito_kagemusha_request_authorization_create_v2"
        ))
    }

    #if canImport(Darwin)
    func testSecretNativeOutputIsCopiedBeforeSecureDeallocation() throws {
        let expected = Data([0x11, 0x22, 0x33, 0x44])
        let pointer = UnsafeMutablePointer<UInt8>.allocate(capacity: expected.count)
        expected.copyBytes(to: pointer, count: expected.count)
        var freeCount = 0
        let copied = try NoritoNativeBridge.copyKagemushaNativeSecretArchiveOutput(
            pointer: pointer,
            length: CUnsignedLong(expected.count),
            secureFree: { released in
                freeCount += 1
                released?.initialize(repeating: 0, count: expected.count)
                released?.deallocate()
            }
        )
        XCTAssertEqual(copied, expected)
        XCTAssertEqual(freeCount, 1)

        var nullSecureFreeCount = 0
        XCTAssertThrowsError(try NoritoNativeBridge.copyKagemushaNativeSecretArchiveOutput(
            pointer: nil,
            length: 1,
            secureFree: { _ in nullSecureFreeCount += 1 }
        ))
        XCTAssertEqual(nullSecureFreeCount, 0)

        let empty = UnsafeMutablePointer<UInt8>.allocate(capacity: 1)
        empty.initialize(to: 0xEE)
        var emptySecureFreeCount = 0
        XCTAssertThrowsError(try NoritoNativeBridge.copyKagemushaNativeSecretArchiveOutput(
            pointer: empty,
            length: 0,
            secureFree: { released in
                emptySecureFreeCount += 1
                released?.initialize(to: 0)
                released?.deallocate()
            }
        ))
        XCTAssertEqual(emptySecureFreeCount, 1)

        let rejected = UnsafeMutablePointer<UInt8>.allocate(capacity: 1)
        rejected.initialize(to: 0xFF)
        var rejectedSecureFreeCount = 0
        XCTAssertThrowsError(try NoritoNativeBridge.copyKagemushaNativeSecretArchiveOutput(
            pointer: rejected,
            length: CUnsignedLong(
                KagemushaRecursiveSpend.maximumRedemptionChangePreparationArchiveBytesV4 + 1
            ),
            secureFree: { released in
                rejectedSecureFreeCount += 1
                released?.initialize(to: 0)
                released?.deallocate()
            }
        ))
        XCTAssertEqual(rejectedSecureFreeCount, 1)
    }
    #endif

    private func preparation(
        inputOpening: KagemushaNoteOpening,
        summary: KagemushaRecursiveSpendBundleSummaryV4,
        changeAmount: KagemushaScaledAmount
    ) throws -> KagemushaRecursiveSpendRedemptionChangePreparationV4 {
        let opening = try KagemushaNoteOpening(
            spendKey: inputOpening.spendKey,
            rho: fixed32(0x71),
            diversifier: try ConfidentialOwnerTag.defaultDiversifier()
        )
        let output = try KagemushaSpendableNoteDescriptor(
            networkID: TestNetworkIds.canonical,
            assetDefinitionID: summary.assetDefinitionID,
            noteCommitment: fixed32(0x73),
            spendNullifier: fixed32(0x74),
            amount: changeAmount
        )
        return try KagemushaRecursiveSpendRedemptionChangePreparationV4(
            opening: opening,
            output: output,
            inputOpening: inputOpening,
            inputSummary: summary,
            changeAmount: changeAmount
        )
    }

    private func inputSummary(
        amount: String
    ) throws -> KagemushaRecursiveSpendBundleSummaryV4 {
        KagemushaRecursiveSpendBundleSummaryV4(
            assetDefinitionID: assetDefinitionID(),
            amount: try KagemushaScaledAmount(atomicUnits: amount, scale: 2),
            noteCommitment: fixed32(0x51),
            spendNullifier: fixed32(0x52),
            hopCount: 1,
            proofStepCount: 1,
            branchClaims: [try KagemushaRecursiveSpendBranchClaim.root(
                lineageRoot: fixed32(0x53)
            )],
            artifactBinding: try KagemushaRecursiveSpendArtifactBindingV4(
                generation: "swift-redemption-change-test",
                manifestSHA256: fixed32(0x54)
            ),
            verifierKeyID: "halo2/ipa:test",
            bundleDigest: fixed32(0x55)
        )
    }

    private func opaqueBundle() throws -> KagemushaRecursiveSpendBundleV4 {
        try KagemushaRecursiveSpendBundleV4(
            noritoArchive: KagemushaRecursiveSpend.frameArchive(
                schema: KagemushaRecursiveSpend.bundleWireNameV4,
                payload: Data([0xA5])
            )
        )
    }

    private func noteOpening(seed: UInt8) throws -> KagemushaNoteOpening {
        try KagemushaNoteOpening(
            spendKey: fixed32(seed),
            rho: fixed32(seed &+ 1),
            diversifier: fixed32(seed &+ 2)
        )
    }

    private func assetDefinitionID() -> String {
        var bytes = Data((0..<16).map { UInt8($0 + 1) })
        bytes[6] = (bytes[6] & 0x0f) | 0x40
        bytes[8] = (bytes[8] & 0x3f) | 0x80
        return AssetDefinitionAddress.encode(uuidBytes: bytes)!
    }

    private func fields(_ values: [Data]) -> Data {
        var writer = CompactNoritoWriter()
        values.forEach { writer.writeField($0) }
        return writer.data
    }

    private func uint16(_ value: UInt16) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt16LE(value)
        return writer.data
    }

    private func uint32(_ value: UInt32) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt32LE(value)
        return writer.data
    }

    private func fixed32(_ byte: UInt8) -> Data {
        Data(repeating: byte, count: 32)
    }
}
