import Foundation
import XCTest
@testable import IrohaSwift

/// Consumes the Rust-owned direct instruction golden, not a signed-transaction fixture.
final class UpdatePlainConvictionRustGoldenTests: XCTestCase {
    private static let wireID =
        "iroha.instruction.v1::governance::UpdatePlainConviction"
    private static let concreteSchema =
        "iroha_data_model::isi::governance::UpdatePlainConviction"
    private static let instructionBoxSchema =
        "(alloc::string::String, alloc::vec::Vec<u8>)"

    func testSwiftDirectInstructionAndBoxMatchRustGolden() throws {
        let fixture = try loadFixture()
        XCTAssertEqual(
            Set(fixture.keys),
            [
                "version", "inputs", "wire_id", "concrete_schema_name",
                "concrete_schema_hash", "header_flags", "framed_instruction_base64",
                "framed_instruction_len", "bare_payload_hex", "concrete_frame_hex",
                "instruction_box_pair_hex", "standalone_instruction_box_frame_hex",
            ]
        )
        XCTAssertEqual(fixture["version"] as? Int, 1)
        let inputs = try XCTUnwrap(fixture["inputs"] as? [String: Any])
        XCTAssertEqual(
            Set(inputs.keys),
            ["referendum_id", "owner", "amount", "duration_blocks"]
        )
        let referendumID = try XCTUnwrap(inputs["referendum_id"] as? String)
        let owner = try XCTUnwrap(inputs["owner"] as? String)
        let amount = try XCTUnwrap(inputs["amount"] as? String)
        let durationNumber = try XCTUnwrap(inputs["duration_blocks"] as? NSNumber)
        let durationBlocks = try XCTUnwrap(UInt64(durationNumber.stringValue))
        XCTAssertEqual(fixture["wire_id"] as? String, Self.wireID)
        XCTAssertEqual(fixture["concrete_schema_name"] as? String, Self.concreteSchema)
        XCTAssertEqual(
            try fixtureHex(fixture, "concrete_schema_hash"),
            Data(noritoSchemaHash(forTypeName: Self.concreteSchema))
        )
        XCTAssertEqual(fixture["header_flags"] as? Int, Int(NoritoHeader.compactLen))

        var payload = CompactNoritoWriter()
        payload.writeField(CompactNorito.encodeString(referendumID))
        payload.writeField(try CanonicalNorito.encodeCompactAccountId(owner))
        payload.writeField(try CanonicalNorito.encodeCompactQuantity(amount))
        payload.writeField(CompactNorito.encodeUInt64(durationBlocks))
        XCTAssertEqual(payload.data, try fixtureHex(fixture, "bare_payload_hex"))

        let concrete = noritoEncode(
            typeName: Self.concreteSchema,
            payload: payload.data,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 8
        )
        XCTAssertEqual(concrete, try fixtureHex(fixture, "concrete_frame_hex"))
        XCTAssertEqual(concrete.count, fixture["framed_instruction_len"] as? Int)
        XCTAssertEqual(concrete.base64EncodedString(), fixture["framed_instruction_base64"] as? String)
        let concreteFrame = try XCTUnwrap(noritoDecodeFrame(concrete))
        XCTAssertEqual(concreteFrame.header.schema, noritoSchemaHash(forTypeName: Self.concreteSchema))
        XCTAssertEqual(concreteFrame.header.flags, NoritoHeader.compactLen)
        XCTAssertEqual(concreteFrame.paddingLength, 0)
        XCTAssertEqual(concreteFrame.payload, payload.data)

        let instruction = try TransactionInstructionFrame(
            wireName: Self.wireID,
            framedPayload: concrete
        )
        let pair = try instruction.compactInstructionBoxPayload()
        XCTAssertEqual(pair, try fixtureHex(fixture, "instruction_box_pair_hex"))
        var pairReader = CanonicalNoritoReader(data: pair)
        XCTAssertEqual(
            try pairReader.readCompactField(),
            CompactNorito.encodeString(Self.wireID)
        )
        var framedBytesReader = CanonicalNoritoReader(
            data: try pairReader.readCompactField()
        )
        XCTAssertEqual(try framedBytesReader.readUInt64LE(), UInt64(concrete.count))
        XCTAssertEqual(try framedBytesReader.readBytes(concrete.count), concrete)
        XCTAssertEqual(framedBytesReader.remaining(), 0)
        XCTAssertEqual(pairReader.remaining(), 0)

        let standaloneBox = noritoEncode(
            typeName: Self.instructionBoxSchema,
            payload: pair,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 8
        )
        XCTAssertEqual(
            standaloneBox,
            try fixtureHex(fixture, "standalone_instruction_box_frame_hex")
        )
        let boxFrame = try XCTUnwrap(noritoDecodeFrame(standaloneBox))
        XCTAssertEqual(boxFrame.header.schema, noritoSchemaHash(forTypeName: Self.instructionBoxSchema))
        XCTAssertEqual(boxFrame.header.flags, NoritoHeader.compactLen)
        XCTAssertEqual(boxFrame.paddingLength, 0)
        XCTAssertEqual(boxFrame.payload, pair)
    }

    func testNativeSignedUpdateContainsTheRustGoldenDirectInstruction() throws {
        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAvailable,
            "same-source NoritoBridge native conviction signer is required"
        )
        let fixture = try loadFixture()
        let inputs = try XCTUnwrap(fixture["inputs"] as? [String: Any])
        let owner = try XCTUnwrap(inputs["owner"] as? String)
        let durationNumber = try XCTUnwrap(inputs["duration_blocks"] as? NSNumber)
        let durationBlocks = try XCTUnwrap(UInt64(durationNumber.stringValue))
        let signingKey = try SigningKey.ed25519(
            privateKey: Data(repeating: 1, count: 32)
        )
        let signingAccount = try AccountAddress
            .fromAccount(publicKey: signingKey.publicKey())
            .toI105(networkPrefix: AccountId.defaultNetworkPrefix)
        XCTAssertEqual(signingAccount, owner)
        let request = UpdatePlainConvictionRequest(
            networkId: TestNetworkIds.canonical,
            authority: owner,
            referendumId: try XCTUnwrap(inputs["referendum_id"] as? String),
            owner: owner,
            amount: try XCTUnwrap(inputs["amount"] as? String),
            durationBlocks: durationBlocks,
            feePayment: .authority(chargeLimits: [], gasLimit: nil),
            ttlMs: nil
        )
        let signed = try SwiftTransactionEncoder.encodeUpdatePlainConviction(
            request: request,
            signingKey: signingKey,
            creationTimeMs: 1
        )

        var signedReader = CanonicalNoritoReader(data: signed.signedTransaction)
        _ = try signedReader.readCompactField() // SignatureSet
        let transactionPayload = try signedReader.readCompactField()
        XCTAssertEqual(try signedReader.readCompactField(), Data([0]))
        XCTAssertEqual(signedReader.remaining(), 0)
        var payloadReader = CanonicalNoritoReader(data: transactionPayload)
        var fields: [Data] = []
        for _ in 0..<10 {
            fields.append(try payloadReader.readCompactField())
        }
        XCTAssertEqual(payloadReader.remaining(), 0)
        var admissionReader = CanonicalNoritoReader(data: fields[7])
        XCTAssertEqual(
            try admissionReader.readUInt32LE(),
            TransactionAdmissionIntentV1.queuePlanSynced.rawValue
        )
        XCTAssertEqual(admissionReader.remaining(), 0)

        var executable = CanonicalNoritoReader(data: fields[3])
        XCTAssertEqual(try executable.readUInt32LE(), 0) // Native Instructions
        var instructions = CanonicalNoritoReader(data: try executable.readCompactField())
        XCTAssertEqual(executable.remaining(), 0)
        XCTAssertEqual(try instructions.readUInt64LE(), 1)
        XCTAssertEqual(
            try instructions.readCompactField(),
            try fixtureHex(fixture, "instruction_box_pair_hex")
        )
        XCTAssertEqual(instructions.remaining(), 0)
    }

    private func loadFixture() throws -> [String: Any] {
        let fixtureURL = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift
            .deletingLastPathComponent() // repository
            .appendingPathComponent(
                "fixtures/governance/plain_v1/update_plain_conviction_instruction_v1.json"
            )
        let data = try Data(contentsOf: fixtureURL)
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private func fixtureHex(_ fixture: [String: Any], _ key: String) throws -> Data {
        let text = try XCTUnwrap(fixture[key] as? String)
        let data = try XCTUnwrap(Data(hexString: text))
        XCTAssertEqual(data.hexEncodedString(), text)
        return data
    }
}
