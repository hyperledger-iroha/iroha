import XCTest
@testable import IrohaSwift

final class PrivacyConfidentialWitnessTests: XCTestCase {
    func testEncodesPathBasedTransferWitnessWithoutWholeTree() throws {
        let witness = try Self.transferWitnessV2()
        let witnessArchive = try PrivacyConfidentialWitnessCodecs.encodeTransferWitnessV2(witness)
        let witnessFrame = try XCTUnwrap(noritoDecodeFrame(witnessArchive))
        XCTAssertEqual(witnessFrame.paddingLength, 8)
        XCTAssertEqual(
            witnessFrame.header.schema,
            noritoSchemaHash(
                forTypeName: PrivacyConfidentialWitnessCodecs.privacyConfidentialWitnessV2WireName
            )
        )

        let fields = try compactFields(witnessFrame.payload)
        XCTAssertEqual(fields.count, 9)
        let paths = try compactSequence(fields[3])
        XCTAssertEqual(paths.count, 2)
        for path in paths {
            let pathFields = try compactFields(path)
            XCTAssertEqual(pathFields.count, 4)
            XCTAssertEqual(
                try compactSequence(pathFields[0]).count,
                PrivacyConfidentialWitnessCodecs.confidentialTreeDepthV2
            )
            XCTAssertEqual(
                try byteVec(pathFields[1]).count,
                PrivacyConfidentialWitnessCodecs.confidentialTreeDepthV2
            )
            XCTAssertEqual(try compactSequence(pathFields[2]), [])
            XCTAssertEqual(try byteVec(pathFields[3]), Self.fixed32(0x77))
        }

        let request = try PrivacyConfidentialWitnessCodecs
            .buildConfidentialTransferProofRequestV2(witness: witness)
        let requestFields = try compactFields(try XCTUnwrap(noritoDecodeFrame(request)).payload)
        XCTAssertEqual(try byteVec(requestFields[4]), witnessArchive)
    }

    func testEncodesTransferWitnessAndProofRequestWireShape() throws {
        let witness = try Self.transferWitness()
        let witnessArchive = try PrivacyConfidentialWitnessCodecs.encodeTransferWitness(witness)
        let witnessFrame = try XCTUnwrap(noritoDecodeFrame(witnessArchive))

        XCTAssertEqual(witnessFrame.paddingLength, 8)
        XCTAssertEqual(witnessFrame.header.flags, NoritoHeader.compactLen)
        XCTAssertEqual(
            witnessFrame.header.schema,
            noritoSchemaHash(forTypeName: PrivacyConfidentialWitnessCodecs.privacyConfidentialWitnessV1WireName)
        )

        let request = try PrivacyConfidentialWitnessCodecs.buildConfidentialTransferProofRequestV1(
            witness: witness
        )
        let requestFrame = try XCTUnwrap(noritoDecodeFrame(request))
        XCTAssertEqual(requestFrame.header.schema, Array(repeating: UInt8(0x52), count: 16))
        XCTAssertEqual(requestFrame.header.flags, NoritoHeader.compactLen)

        let fields = try compactFields(requestFrame.payload)
        XCTAssertEqual(fields.count, 6)
        XCTAssertEqual(try compactString(fields[0]), PrivacyConfidentialWitnessCodecs.confidentialTransferV2AlgorithmId)
        XCTAssertEqual(try compactString(fields[1]), PrivacyConfidentialWitnessCodecs.confidentialTransferV2Entrypoint)
        XCTAssertEqual(try compactString(fields[2]), PrivacyConfidentialWitnessCodecs.confidentialTransferV2VerifierRef)
        XCTAssertEqual(try byteVec(fields[3]), PrivacyConfidentialWitnessCodecs.confidentialTransferPublicInputsSchema())
        XCTAssertEqual(try byteVec(fields[4]), witnessArchive)
        XCTAssertEqual(try byteVec(fields[5]), Data())
    }

    func testEncodesUnshieldWitnessAndProofRequestWireShape() throws {
        let witness = try Self.unshieldWitness()
        let witnessArchive = try PrivacyConfidentialWitnessCodecs.encodeUnshieldWitness(witness)
        let witnessFrame = try XCTUnwrap(noritoDecodeFrame(witnessArchive))

        XCTAssertEqual(witnessFrame.paddingLength, 8)
        XCTAssertEqual(witnessFrame.header.flags, NoritoHeader.compactLen)
        XCTAssertEqual(
            witnessFrame.header.schema,
            noritoSchemaHash(forTypeName: PrivacyConfidentialWitnessCodecs.privacyConfidentialWitnessV1WireName)
        )

        let witnessFields = try compactFields(witnessFrame.payload)
        XCTAssertEqual(witnessFields.count, 9)
        XCTAssertEqual(try compactString(witnessFields[0]), "chain")
        XCTAssertEqual(try compactString(witnessFields[1]), "asset")
        XCTAssertEqual(try byteVec(witnessFields[2]), Self.fixed32(0x11))

        let treeCommitments = try compactSequence(witnessFields[3])
        XCTAssertEqual(treeCommitments.count, 1)
        XCTAssertEqual(try byteVec(treeCommitments[0]), Self.fixed32(0x66))

        let inputs = try compactSequence(witnessFields[4])
        XCTAssertEqual(inputs.count, 1)
        let inputFields = try compactFields(inputs[0])
        XCTAssertEqual(inputFields.count, 4)
        XCTAssertEqual(inputFields[0], Self.u128LittleEndian(9))
        XCTAssertEqual(try byteVec(inputFields[1]), Self.fixed32(0x22))
        XCTAssertEqual(try byteVec(inputFields[2]), Self.fixed32(0x01))
        XCTAssertEqual(try uint64(inputFields[3]), 0)

        XCTAssertEqual(try compactSequence(witnessFields[5]), [])
        let unshieldChange = try compactSequence(witnessFields[6])
        XCTAssertEqual(unshieldChange.count, 1)
        let changeFields = try compactFields(unshieldChange[0])
        XCTAssertEqual(changeFields.count, 2)
        XCTAssertEqual(changeFields[0], Self.u128LittleEndian(4))
        XCTAssertEqual(try byteVec(changeFields[1]), Self.fixed32(0x55))
        XCTAssertEqual(witnessFields[7], Self.u128LittleEndian(5))
        XCTAssertEqual(try byteVec(witnessFields[8]), Self.fixed32(0x77))

        let expectedSchema = Data(
            (
                "{\"schema\":\"confidential_unshield_v3\",\"public_inputs\":[\"input_commitment_0\"," +
                    "\"input_commitment_1\",\"nullifier_0\",\"nullifier_1\",\"change_commitment_0\"," +
                    "\"root\",\"public_amount\",\"asset_tag\",\"chain_tag\"]}"
            ).utf8
        )
        XCTAssertEqual(
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema(),
            expectedSchema
        )

        let request = try PrivacyConfidentialWitnessCodecs.buildConfidentialUnshieldProofRequestV1(
            witness: witness
        )
        let requestFrame = try XCTUnwrap(noritoDecodeFrame(request))
        XCTAssertEqual(requestFrame.header.schema, Array(repeating: UInt8(0x52), count: 16))
        XCTAssertEqual(requestFrame.header.flags, NoritoHeader.compactLen)

        let requestFields = try compactFields(requestFrame.payload)
        XCTAssertEqual(requestFields.count, 6)
        XCTAssertEqual(
            try compactString(requestFields[0]),
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3AlgorithmId
        )
        XCTAssertEqual(
            try compactString(requestFields[1]),
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3Entrypoint
        )
        XCTAssertEqual(
            try compactString(requestFields[2]),
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef
        )
        XCTAssertEqual(try byteVec(requestFields[3]), expectedSchema)
        XCTAssertEqual(try byteVec(requestFields[4]), witnessArchive)
        XCTAssertEqual(try byteVec(requestFields[5]), Data())
    }

    func testUnshieldWitnessAcceptsZeroOrOneChangeAndCanonicalPublicAmount() throws {
        let zeroChange = try Self.unshieldWitness(
            unshieldChange: [],
            publicAmount: "0"
        )
        XCTAssertNoThrow(try PrivacyConfidentialWitnessCodecs.encodeUnshieldWitness(zeroChange))

        let oneChange = try Self.unshieldWitness(publicAmount: "5")
        XCTAssertNoThrow(try PrivacyConfidentialWitnessCodecs.encodeUnshieldWitness(oneChange))

        let maximumPublicAmount = try Self.unshieldWitness(
            unshieldChange: [],
            publicAmount: "340282366920938463463374607431768211455"
        )
        XCTAssertNoThrow(
            try PrivacyConfidentialWitnessCodecs.buildConfidentialUnshieldProofRequestV1(
                witness: maximumPublicAmount
            )
        )
    }

    func testUnshieldProofRequestRejectsTransferOutputsAndCrossWiredVerifierRef() throws {
        let transferWitness = try Self.transferWitness()
        XCTAssertInvalidField("transferOutputs") {
            _ = try PrivacyConfidentialWitnessCodecs.encodeUnshieldWitness(transferWitness)
        }
        XCTAssertInvalidField("transferOutputs") {
            _ = try PrivacyConfidentialWitnessCodecs.buildConfidentialUnshieldProofRequestV1(
                witness: transferWitness
            )
        }

        let unshieldWitness = try Self.unshieldWitness()
        XCTAssertInvalidField("vkRef") {
            _ = try PrivacyConfidentialWitnessCodecs.buildConfidentialUnshieldProofRequestV1(
                witness: unshieldWitness,
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialTransferV2VerifierRef
            )
        }
    }

    func testBuildsCanonicalUnshieldVerifyRequestAndRejectsInvalidProofInputs() throws {
        let proof = Data([0x01, 0x02, 0x03, 0x04])
        let request = try PrivacyConfidentialWitnessCodecs
            .buildConfidentialUnshieldVerifyRequestV1(proof: proof)
        let frame = try XCTUnwrap(noritoDecodeFrame(request))
        XCTAssertEqual(frame.header.schema, Array(repeating: UInt8(0x52), count: 16))
        XCTAssertEqual(frame.header.flags, NoritoHeader.compactLen)
        XCTAssertEqual(frame.paddingLength, 0)

        let fields = try compactFields(frame.payload)
        XCTAssertEqual(fields.count, 6)
        XCTAssertEqual(
            try compactString(fields[0]),
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3AlgorithmId
        )
        XCTAssertEqual(
            try compactString(fields[1]),
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3Entrypoint
        )
        XCTAssertEqual(
            try compactString(fields[2]),
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldV3VerifierRef
        )
        XCTAssertEqual(
            try byteVec(fields[3]),
            PrivacyConfidentialWitnessCodecs.confidentialUnshieldPublicInputsSchema()
        )
        XCTAssertEqual(try byteVec(fields[4]), Data())
        XCTAssertEqual(try byteVec(fields[5]), proof)

        XCTAssertInvalidField("proof") {
            _ = try PrivacyConfidentialWitnessCodecs
                .buildConfidentialUnshieldVerifyRequestV1(proof: Data())
        }
        XCTAssertInvalidField("proof") {
            _ = try PrivacyConfidentialWitnessCodecs.buildConfidentialUnshieldVerifyRequestV1(
                proof: Data(repeating: 0x7f, count: 32 * 1024 * 1024 + 1)
            )
        }
        XCTAssertInvalidField("vkRef") {
            _ = try PrivacyConfidentialWitnessCodecs.buildConfidentialUnshieldVerifyRequestV1(
                proof: proof,
                vkRef: PrivacyConfidentialWitnessCodecs.confidentialTransferV2VerifierRef
            )
        }
    }

    func testTransferWitnessValidationRejectsUnshieldAndPublicAmount() throws {
        let input = try PrivacyConfidentialNoteWitnessV1(
            amount: "7",
            rho: Data(repeating: 0x22, count: 32),
            diversifier: Data(repeating: 0x01, count: 32),
            leafIndex: 0
        )
        let output = try PrivacyConfidentialTransferOutputWitnessV1(
            amount: "7",
            rho: Data(repeating: 0x33, count: 32),
            ownerTag: Data(repeating: 0x44, count: 32)
        )
        let change = try PrivacyConfidentialUnshieldChangeWitnessV1(
            amount: "1",
            rho: Data(repeating: 0x55, count: 32)
        )
        XCTAssertThrowsError(try PrivacyConfidentialWitnessV1(
            chainId: "chain",
            assetDefinitionId: "asset",
            spendKey: Data(repeating: 0x11, count: 32),
            treeCommitments: [Data(repeating: 0x66, count: 32)],
            inputs: [input],
            transferOutputs: [output],
            unshieldChange: [change],
            publicAmount: "0",
            rootHint: Data(repeating: 0x77, count: 32)
        ))

        let publicAmountWitness = try PrivacyConfidentialWitnessV1(
            chainId: "chain",
            assetDefinitionId: "asset",
            spendKey: Data(repeating: 0x11, count: 32),
            treeCommitments: [Data(repeating: 0x66, count: 32)],
            inputs: [input],
            transferOutputs: [],
            unshieldChange: [],
            publicAmount: "1",
            rootHint: Data(repeating: 0x77, count: 32)
        )
        XCTAssertThrowsError(try PrivacyConfidentialWitnessCodecs.encodeTransferWitness(publicAmountWitness))
    }

    func testWitnessConstructorsRejectNoncanonicalScalarsTextAndFixedFields() throws {
        XCTAssertInvalidField("amount") {
            _ = try PrivacyConfidentialNoteWitnessV1(
                amount: "01",
                rho: Self.fixed32(0x22),
                diversifier: Self.fixed32(0x01),
                leafIndex: 0
            )
        }
        XCTAssertInvalidField("amount") {
            _ = try PrivacyConfidentialNoteWitnessV1(
                amount: "340282366920938463463374607431768211456",
                rho: Self.fixed32(0x22),
                diversifier: Self.fixed32(0x01),
                leafIndex: 0
            )
        }
        XCTAssertInvalidField("chainId") {
            _ = try Self.transferWitness(chainId: " chain")
        }
        XCTAssertInvalidField("assetDefinitionId") {
            _ = try Self.transferWitness(assetDefinitionId: "asset\0id")
        }
        XCTAssertInvalidField("spendKey") {
            _ = try Self.transferWitness(spendKey: Data(repeating: 0x11, count: 31))
        }
        XCTAssertInvalidField("rootHint") {
            _ = try Self.transferWitness(rootHint: Data(repeating: 0x77, count: 31))
        }
        XCTAssertInvalidField("treeCommitments[0]") {
            _ = try Self.transferWitness(treeCommitments: [Data(repeating: 0x66, count: 31)])
        }
    }

    func testWitnessRejectsAdversarialTreeAndOutputTopology() throws {
        let input0 = try Self.noteWitness(leafIndex: 0, rhoSeed: 0x22)
        let input1 = try Self.noteWitness(leafIndex: 1, rhoSeed: 0x23)
        let input1DuplicateRho = try Self.noteWitness(leafIndex: 1, rhoSeed: 0x22)
        let input2 = try Self.noteWitness(leafIndex: 2, rhoSeed: 0x24)
        let output = try Self.transferOutput(amount: "7", rhoSeed: 0x33)
        let change = try PrivacyConfidentialUnshieldChangeWitnessV1(
            amount: "1",
            rho: Self.fixed32(0x55)
        )

        XCTAssertInvalidField("treeCommitments") {
            _ = try Self.transferWitness(treeCommitments: [], inputs: [input0])
        }
        XCTAssertInvalidField("inputs") {
            _ = try Self.transferWitness(inputs: [])
        }
        XCTAssertInvalidField("inputs") {
            _ = try Self.transferWitness(
                treeCommitments: [Self.fixed32(0x66), Self.fixed32(0x67), Self.fixed32(0x68)],
                inputs: [input0, input1, input2]
            )
        }
        XCTAssertInvalidField("inputs[0].leafIndex") {
            _ = try Self.transferWitness(
                treeCommitments: [Self.fixed32(0x66)],
                inputs: [input1]
            )
        }
        XCTAssertInvalidField("inputs[1].leafIndex") {
            _ = try Self.transferWitness(
                treeCommitments: [Self.fixed32(0x66), Self.fixed32(0x67)],
                inputs: [input0, input0]
            )
        }
        XCTAssertInvalidField("inputs[1].rho") {
            _ = try Self.transferWitness(
                treeCommitments: [Self.fixed32(0x66), Self.fixed32(0x67)],
                inputs: [input0, input1DuplicateRho]
            )
        }
        XCTAssertInvalidField("transferOutputs") {
            _ = try Self.transferWitness(
                transferOutputs: [
                    try Self.transferOutput(amount: "1", rhoSeed: 0x31),
                    try Self.transferOutput(amount: "2", rhoSeed: 0x32),
                    try Self.transferOutput(amount: "3", rhoSeed: 0x33)
                ]
            )
        }
        XCTAssertInvalidField("unshieldChange") {
            _ = try Self.transferWitness(
                transferOutputs: [],
                unshieldChange: [
                    change,
                    try PrivacyConfidentialUnshieldChangeWitnessV1(amount: "2", rho: Self.fixed32(0x56))
                ]
            )
        }
        XCTAssertInvalidField("transferOutputs") {
            _ = try Self.transferWitness(
                transferOutputs: [output],
                unshieldChange: [change]
            )
        }
        XCTAssertInvalidField("publicAmount") {
            _ = try Self.transferWitness(
                transferOutputs: [output],
                publicAmount: "1"
            )
        }
    }

    func testProofRequestBuilderRejectsAmbiguousVerifierRefsAndTransferShape() throws {
        let witness = try Self.transferWitness()
        XCTAssertInvalidField("vkRef") {
            _ = try PrivacyConfidentialWitnessCodecs.buildConfidentialTransferProofRequestV1(
                witness: witness,
                vkRef: "halo2-ipa-pasta:confidential_unshield_v3"
            )
        }

        let noOutputWitness = try Self.transferWitness(transferOutputs: [])
        XCTAssertInvalidField("transferOutputs") {
            _ = try PrivacyConfidentialWitnessCodecs.encodeTransferWitness(noOutputWitness)
        }
        XCTAssertInvalidField("transferOutputs") {
            _ = try PrivacyConfidentialWitnessCodecs.buildConfidentialTransferProofRequestV1(
                witness: noOutputWitness
            )
        }
    }

    private static func transferWitness(
        chainId: String = "chain",
        assetDefinitionId: String = "asset",
        spendKey: Data = Data(repeating: 0x11, count: 32),
        treeCommitments: [Data] = [Data(repeating: 0x66, count: 32)],
        inputs: [PrivacyConfidentialNoteWitnessV1]? = nil,
        transferOutputs: [PrivacyConfidentialTransferOutputWitnessV1]? = nil,
        unshieldChange: [PrivacyConfidentialUnshieldChangeWitnessV1] = [],
        publicAmount: String = "0",
        rootHint: Data = Data(repeating: 0x77, count: 32)
    ) throws -> PrivacyConfidentialWitnessV1 {
        let checkedInputs = try inputs ?? [noteWitness(leafIndex: 0, rhoSeed: 0x22)]
        let checkedOutputs = try transferOutputs ?? [transferOutput(amount: "7", rhoSeed: 0x33)]
        return try PrivacyConfidentialWitnessV1(
            chainId: chainId,
            assetDefinitionId: assetDefinitionId,
            spendKey: spendKey,
            treeCommitments: treeCommitments,
            inputs: checkedInputs,
            transferOutputs: checkedOutputs,
            unshieldChange: unshieldChange,
            publicAmount: publicAmount,
            rootHint: rootHint
        )
    }

    private static func transferWitnessV2() throws -> PrivacyConfidentialWitnessV2 {
        try PrivacyConfidentialWitnessV2(
            chainId: "chain",
            assetDefinitionId: "asset",
            spendKey: fixed32(0x11),
            inputPaths: [try pathV2(directionSeed: 0), try pathV2(directionSeed: 1)],
            inputs: [try noteWitness(leafIndex: 0, rhoSeed: 0x22)],
            transferOutputs: [try transferOutput(amount: "7", rhoSeed: 0x33)],
            unshieldChange: [],
            publicAmount: "0",
            rootHint: fixed32(0x77)
        )
    }

    private static func pathV2(
        directionSeed: UInt8
    ) throws -> PrivacyConfidentialMerklePathWitnessV2 {
        let depth = PrivacyConfidentialWitnessCodecs.confidentialTreeDepthV2
        return try PrivacyConfidentialMerklePathWitnessV2(
            siblings: (0..<depth).map { fixed32(UInt8($0 + 1)) },
            directions: Data((0..<depth).map { UInt8($0) & 1 ^ directionSeed }),
            root: fixed32(0x77)
        )
    }

    private static func unshieldWitness(
        transferOutputs: [PrivacyConfidentialTransferOutputWitnessV1] = [],
        unshieldChange: [PrivacyConfidentialUnshieldChangeWitnessV1]? = nil,
        publicAmount: String = "5"
    ) throws -> PrivacyConfidentialWitnessV1 {
        let checkedChange: [PrivacyConfidentialUnshieldChangeWitnessV1]
        if let unshieldChange {
            checkedChange = unshieldChange
        } else {
            checkedChange = [
                try PrivacyConfidentialUnshieldChangeWitnessV1(
                    amount: "4",
                    rho: fixed32(0x55)
                )
            ]
        }
        return try PrivacyConfidentialWitnessV1(
            chainId: "chain",
            assetDefinitionId: "asset",
            spendKey: fixed32(0x11),
            treeCommitments: [fixed32(0x66)],
            inputs: [try noteWitness(leafIndex: 0, rhoSeed: 0x22, amount: "9")],
            transferOutputs: transferOutputs,
            unshieldChange: checkedChange,
            publicAmount: publicAmount,
            rootHint: fixed32(0x77)
        )
    }

    private static func noteWitness(
        leafIndex: UInt64,
        rhoSeed: UInt8,
        amount: String = "7"
    ) throws -> PrivacyConfidentialNoteWitnessV1 {
        try PrivacyConfidentialNoteWitnessV1(
            amount: amount,
            rho: Self.fixed32(rhoSeed),
            diversifier: Self.fixed32(0x01),
            leafIndex: leafIndex
        )
    }

    private static func transferOutput(
        amount: String,
        rhoSeed: UInt8
    ) throws -> PrivacyConfidentialTransferOutputWitnessV1 {
        try PrivacyConfidentialTransferOutputWitnessV1(
            amount: amount,
            rho: Self.fixed32(rhoSeed),
            ownerTag: Self.fixed32(0x44)
        )
    }

    private static func fixed32(_ seed: UInt8) -> Data {
        Data(repeating: seed, count: 32)
    }

    private static func u128LittleEndian(_ value: UInt8) -> Data {
        Data([value] + Array(repeating: UInt8(0), count: 15))
    }

    private func XCTAssertInvalidField(
        _ field: String,
        file: StaticString = #filePath,
        line: UInt = #line,
        _ body: () throws -> Void
    ) {
        XCTAssertThrowsError(try body(), file: file, line: line) { error in
            XCTAssertEqual(
                error as? PrivacyConfidentialWitnessError,
                PrivacyConfidentialWitnessError.invalidField(field),
                file: file,
                line: line
            )
        }
    }

    private func compactFields(_ payload: Data) throws -> [Data] {
        var reader = TestCompactReader(payload)
        var fields: [Data] = []
        while reader.remaining > 0 {
            fields.append(try reader.readField())
        }
        return fields
    }

    private func compactString(_ payload: Data) throws -> String {
        var reader = TestCompactReader(payload)
        let bytes = try reader.readBytes(Int(reader.readLength()))
        XCTAssertEqual(reader.remaining, 0)
        return try XCTUnwrap(String(data: bytes, encoding: .utf8))
    }

    private func byteVec(_ payload: Data) throws -> Data {
        var reader = TestCompactReader(payload)
        let count = try reader.readUInt64LE()
        let bytes = try reader.readBytes(Int(count))
        XCTAssertEqual(reader.remaining, 0)
        return bytes
    }

    private func compactSequence(_ payload: Data) throws -> [Data] {
        var reader = TestCompactReader(payload)
        let count = try reader.readUInt64LE()
        var values: [Data] = []
        values.reserveCapacity(Int(count))
        for _ in 0..<count {
            values.append(try reader.readField())
        }
        XCTAssertEqual(reader.remaining, 0)
        return values
    }

    private func uint64(_ payload: Data) throws -> UInt64 {
        var reader = TestCompactReader(payload)
        let value = try reader.readUInt64LE()
        XCTAssertEqual(reader.remaining, 0)
        return value
    }
}

private struct TestCompactReader {
    private let data: Data
    private var offset: Int = 0

    init(_ data: Data) {
        self.data = data
    }

    var remaining: Int {
        data.count - offset
    }

    mutating func readUInt64LE() throws -> UInt64 {
        let bytes = try readBytes(8)
        return bytes.enumerated().reduce(UInt64(0)) { partial, element in
            partial | (UInt64(element.element) << UInt64(element.offset * 8))
        }
    }

    mutating func readLength() throws -> UInt64 {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        while true {
            let byte = try readByte()
            value |= UInt64(byte & 0x7f) << shift
            if (byte & 0x80) == 0 {
                return value
            }
            shift += 7
        }
    }

    mutating func readField() throws -> Data {
        try readBytes(Int(readLength()))
    }

    mutating func readBytes(_ count: Int) throws -> Data {
        guard offset + count <= data.count else {
            throw PrivacyConfidentialWitnessError.invalidArchive("testReader")
        }
        defer { offset += count }
        return Data(data[offset..<(offset + count)])
    }

    private mutating func readByte() throws -> UInt8 {
        guard offset < data.count else {
            throw PrivacyConfidentialWitnessError.invalidArchive("testReader")
        }
        defer { offset += 1 }
        return data[offset]
    }
}
