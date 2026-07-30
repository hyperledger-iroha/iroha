import CryptoKit
import Foundation
@testable import IrohaSwift

enum KagemushaPeerTransportTestFixtures {
    static func receiveOfferArchive() throws -> Data {
        try rustFixtureData("offline_recipient_receive_offer_v2.hex")
    }

    static func recipientRequestArchive() throws -> Data {
        try rustFixtureData("offline_recipient_payment_request_v2.hex")
    }

    static func recipientRegistrationLineageArchive() throws -> Data {
        try rustFixtureData("offline_recipient_registration_lineage_v2.hex")
    }

    static func publisherCheckpointEnvelope() throws -> Data {
        try rustFixtureData("offline_recipient_checkpoint_envelope.hex")
    }

    static func receiveRequest(seed: UInt8 = 0x41) throws
        -> KagemushaRecipientReceiveOfferV2
    {
        let exact = try receiveOfferArchive()
        if seed == 0x41 {
            return try KagemushaRecipientReceiveOfferV2(
                noritoArchive: exact,
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
        }
        let request = try KagemushaRecursiveSpendCodecs.decodeRecipientRequest(
            rustFixtureData("offline_recipient_payment_request_v2.hex"),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
        return try KagemushaRecipientReceiveOfferV2(
            request: request,
            lineageArchive: rustFixtureData(
                "offline_recipient_registration_lineage_v2.hex"
            ),
            publisherCheckpointEnvelope: Data(repeating: seed, count: 2_048),
            chainDiscriminant: SccpV1.tairaI105DiscriminantV1
        )
    }

    static func paymentRequest(seed: UInt8 = 0x41) throws
        -> KagemushaRecipientPaymentRequest
    {
        if seed == 0x41 {
            return try KagemushaRecursiveSpendCodecs.decodeRecipientRequest(
                recipientRequestArchive(),
                chainDiscriminant: SccpV1.tairaI105DiscriminantV1
            )
        }
        let signingKey = try P256.Signing.PrivateKey(
            rawRepresentation: fixed32(seed)
        )
        let publicKey = try KagemushaDevicePublicKeyV2(
            sec1Bytes: signingKey.publicKey.x963Representation
        )
        let recipient = try AccountAddress
            .fromAccount(publicKey: fixed32(seed))
            .toI105(networkPrefix: SccpV1.tairaI105DiscriminantV1)
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
            recipientKeyReference: fixed32(seed &+ 3),
            receiverDeviceID: "ios-transport-fixture",
            receiverPublicKey: publicKey,
            requestID: fixed32(seed &+ 4),
            issuedAtMilliseconds: 1_800_000_000_000,
            expiresAtMilliseconds: 1_800_000_060_000,
            recipientOutput: note,
            senderOutputProverMaterial: Data([seed, seed &+ 1, seed &+ 2])
        )
        let payloadArchive = try KagemushaRecursiveSpendCodecs
            .encodeRecipientRequestPayload(payload)
        let signature = try signingKey.signature(for: payloadArchive)
        let deviceSignature = try KagemushaDeviceSignatureV2(
            derBytes: signature.derRepresentation
        )
        guard let payloadBytes = noritoDecodeFrame(payloadArchive)?.payload else {
            throw KagemushaRecursiveSpendError.invalidArchive("transportFixture.requestPayload")
        }
        var requestWriter = CompactNoritoWriter()
        requestWriter.writeBytes(payloadBytes)
        requestWriter.writeField(deviceSignature.rawBytes)
        let requestArchive = KagemushaRecursiveSpend.frameArchive(
            schema: KagemushaRecursiveSpend.recipientRequestWireName,
            payload: requestWriter.data
        )
        return try KagemushaRecipientPaymentRequest(
            payload: payload,
            signature: deviceSignature,
            archive: requestArchive
        )
    }

    static func payment(
        request: KagemushaRecipientPaymentRequest
    ) throws
        -> KagemushaRecursiveSpendPeerPaymentV4
    {
        guard request.archive == (try recipientRequestArchive()) else {
            throw KagemushaRecursiveSpendError.invalidField(
                "transportFixture.recipientRequest"
            )
        }
        let payment = try KagemushaRecursiveSpendPeerPaymentV4(
            noritoArchive: rustFixtureData("offline_peer_payment_v4.hex")
        )
        _ = try KagemushaReceiverAcknowledgement.prepare(
            request: request,
            payment: payment,
            acceptedAtMilliseconds: 1_900_000_001_000
        )
        return payment
    }

    static func acknowledgement(
        request: KagemushaRecipientPaymentRequest,
        payment: KagemushaRecursiveSpendPeerPaymentV4,
        receiverSigningSeed: UInt8 = 0x46
    ) throws -> KagemushaReceiverAcknowledgement {
        let payload = try KagemushaReceiverAcknowledgement.prepare(
            request: request,
            payment: payment,
            acceptedAtMilliseconds: 1_900_000_001_000
        )
        let signingKey = try P256.Signing.PrivateKey(
            rawRepresentation: fixed32(receiverSigningSeed)
        )
        let signature = try signingKey.signature(for: payload.signingBytes())
        let deviceSignature = try KagemushaDeviceSignatureV2(
            derBytes: signature.derRepresentation
        )
        return try KagemushaReceiverAcknowledgement.create(
            payload: payload,
            signature: deviceSignature,
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

    private static func rustFixtureData(_ name: String) throws -> Data {
        var root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        for _ in 0..<3 { root.deleteLastPathComponent() }
        let url = root
            .appendingPathComponent("crates/connect_norito_bridge/tests/fixtures")
            .appendingPathComponent(name)
        let text = try String(contentsOf: url, encoding: .utf8)
        let compact = text.unicodeScalars.filter {
            !CharacterSet.whitespacesAndNewlines.contains($0)
        }.map(String.init).joined()
        guard let bytes = Data(hexString: compact) else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "transportFixture.\(name)"
            )
        }
        return bytes
    }

}
