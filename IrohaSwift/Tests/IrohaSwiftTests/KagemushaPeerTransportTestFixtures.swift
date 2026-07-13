import CryptoKit
import Foundation
@testable import IrohaSwift

enum KagemushaPeerTransportTestFixtures {
    static func receiveRequest(seed: UInt8 = 0x41) throws
        -> KagemushaRecipientPaymentRequest
    {
        let signingKey = try P256.Signing.PrivateKey(
            rawRepresentation: fixed32(seed)
        )
        let publicKey = try KagemushaDevicePublicKeyV2(
            sec1Bytes: signingKey.publicKey.x963Representation
        )
        let recipient = try AccountAddress
            .fromAccount(publicKey: fixed32(seed))
            .toI105(networkPrefix: 0x02F1)
        let amount = try KagemushaScaledAmount(atomicUnits: "125", scale: 2)
        let note = try KagemushaSpendableNoteDescriptor(
            chainID: "swift-kagemusha-transport",
            assetDefinitionID: assetDefinitionID(),
            noteCommitment: fixed32(seed &+ 1),
            spendNullifier: fixed32(seed &+ 2),
            amount: amount
        )
        let payload = try KagemushaRecipientPaymentRequestSigningPayload(
            chainID: note.chainID,
            assetDefinitionID: note.assetDefinitionID,
            amount: amount,
            recipient: recipient,
            recipientKeyReference: try publicKey.receiverKeyReference(),
            receiverDeviceID: "ios-transport-fixture",
            receiverPublicKey: publicKey,
            requestID: fixed32(seed &+ 4),
            issuedAtMilliseconds: 1_800_000_000_000,
            expiresAtMilliseconds: 1_800_000_060_000,
            recipientOutput: note,
            senderOutputProverMaterial: Data([seed, seed &+ 1, seed &+ 2])
        )
        let signature = try signingKey.signature(for: payload.signingBytes())
        return try payload.signed(
            signature: KagemushaDeviceSignatureV2(
                derBytes: signature.derRepresentation
            )
        )
    }

    static func payment(
        request: KagemushaRecipientPaymentRequest? = nil,
        seed: UInt8 = 0x51
    ) throws
        -> KagemushaRecursiveSpendPeerPayment
    {
        let operationID = fixed32(seed)
        let requestDigest = try request?.verified(
            atMilliseconds: 1_800_000_000_500
        ).digest ?? fixed32(seed &+ 1)
        let peerSplit = fields([
            fixed32(seed &+ 2),
            uint32(KagemushaRecursiveSpendBranch.recipient.rawValue),
            requestDigest,
            operationID,
            uint32(1),
            uint32(0),
        ])
        var transition = CompactNoritoWriter()
        transition.writeUInt32LE(0)
        transition.writeField(peerSplit)
        let finalRoot = fixed32(0x44)
        var statementPrefix = (0..<9).map { Data([UInt8($0 + 1)]) }
        statementPrefix[3] = finalRoot
        let statement = fields(
            statementPrefix
                + [
                    option(transition.data),
                    Data([0xA1]),
                    Data([0xA2]),
                ]
        )
        let archive = noritoEncode(
            typeName: KagemushaRecursiveSpend.bundleWireName,
            payload: fields([
                statement,
                Data([0xB0]),
            ]),
            flags: NoritoHeader.compactLen
        )
        let claim = try KagemushaRecursiveSpendBranchClaim.root(
            lineageRoot: fixed32(seed &+ 4)
        )
        let summary = KagemushaRecursiveSpendBundleSummary(
            assetDefinitionID: assetDefinitionID(),
            amount: try KagemushaScaledAmount(atomicUnits: "125", scale: 2),
            noteCommitment: fixed32(seed &+ 5),
            spendNullifier: fixed32(seed &+ 6),
            hopCount: 1,
            proofStepCount: 1,
            branchClaims: [claim],
            artifactBinding: try KagemushaRecursiveSpendArtifactBinding(
                generation: "transport-fixture-v3",
                manifestSHA256: fixed32(0xA7)
            ),
            verifierKeyID: "\(KagemushaRecursiveSpend.pastaCycleBackend):\(KagemushaRecursiveSpend.stepEqCircuitID)",
            bundleDigest: fixed32(seed &+ 7)
        )
        let bundle = KagemushaRecursiveSpendBundle(archive: archive, summary: summary)
        return try KagemushaRecursiveSpendPeerPayment.create(
            recipientBundle: bundle,
            recipientMembershipWitness: membershipWitness(root: finalRoot)
        )
    }

    static func acknowledgement(
        request: KagemushaRecipientPaymentRequest,
        payment: KagemushaRecursiveSpendPeerPayment,
        receiverSigningSeed: UInt8 = 0x41
    ) throws -> KagemushaReceiverAcknowledgement {
        let payload = try KagemushaReceiverAcknowledgement.prepare(
            request: request,
            payment: payment,
            acceptedAtMilliseconds: 1_800_000_001_000
        )
        let signingKey = try P256.Signing.PrivateKey(
            rawRepresentation: fixed32(receiverSigningSeed)
        )
        let signature = try signingKey.signature(for: payload.signingBytes())
        return try KagemushaReceiverAcknowledgement.create(
            payload: payload,
            signature: KagemushaDeviceSignatureV2(
                derBytes: signature.derRepresentation
            ),
            request: request,
            payment: payment
        )
    }

    static func assetDefinitionID() -> String {
        var bytes = Data((0..<16).map { UInt8($0 + 1) })
        bytes[6] = (bytes[6] & 0x0F) | 0x40
        bytes[8] = (bytes[8] & 0x3F) | 0x80
        return AssetDefinitionAddress.encode(uuidBytes: bytes)!
    }

    static func fixed32(_ byte: UInt8) -> Data {
        Data(repeating: byte == 0 ? 1 : byte, count: 32)
    }

    private static func fields(_ values: [Data]) -> Data {
        var writer = CompactNoritoWriter()
        values.forEach { writer.writeField($0) }
        return writer.data
    }

    private static func option(_ value: Data?) -> Data {
        var writer = CompactNoritoWriter()
        guard let value else {
            writer.writeUInt8(0)
            return writer.data
        }
        writer.writeUInt8(1)
        writer.writeLength(UInt64(value.count))
        writer.writeBytes(value)
        return writer.data
    }

    private static func uint32(_ value: UInt32) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt32LE(value)
        return writer.data
    }

    private static func membershipWitness(
        root: Data,
        leafIndex: UInt32 = 5
    ) throws -> KagemushaNoteMembershipWitness {
        let inputDirections = Data((0..<16).map {
            UInt8((UInt64(leafIndex) >> UInt64($0)) & 1)
        })
        return try KagemushaNoteMembershipWitness(
            leafIndex: leafIndex,
            inputPath: PrivacyConfidentialMerklePathWitnessV2(
                siblings: (0..<16).map { fixed32(UInt8($0 + 1)) },
                directions: inputDirections,
                root: root
            ),
            dummyInputPath: PrivacyConfidentialMerklePathWitnessV2(
                siblings: (0..<16).map { fixed32(UInt8($0 + 33)) },
                directions: Data(repeating: 0, count: 16),
                root: root
            )
        )
    }
}
