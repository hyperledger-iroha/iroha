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
        request: KagemushaRecipientPaymentRequest
    ) throws
        -> KagemushaRecursiveSpendPeerPayment
    {
        let requestDigest = try request.verified(
            atMilliseconds: 1_800_000_000_500
        ).digest
        guard let fixtureDigest = Data(hexString:
            "6e62171fc4d85584f96aa4c5b49c2633e66cea2719a361ae9cdf96251c41b608"
        ) else {
            throw KagemushaRecursiveSpendError.invalidField("transportFixture.digestHex")
        }
        guard var archive = Data(hexString: canonicalRecipientBundleArchiveHex),
              let digestRange = archive.range(of: fixtureDigest),
              archive[digestRange.upperBound...].range(of: fixtureDigest) == nil else {
            throw KagemushaRecursiveSpendError.invalidField("transportFixture.bundleHex")
        }
        archive.replaceSubrange(digestRange, with: requestDigest)
        let finalRoot = fixed32(0x44)
        let bundle = try KagemushaRecursiveSpendBundle(noritoArchive: archive)
        let payment = try KagemushaRecursiveSpendPeerPayment.create(
            recipientBundle: bundle,
            recipientMembershipWitness: membershipWitness(root: finalRoot)
        )
        return try KagemushaRecursiveSpendPeerPayment.decode(payment.archive)
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

    private static func membershipWitness(
        root: Data
    ) throws -> KagemushaNoteMembershipWitness {
        let leafIndex: UInt32 = 5
        let inputPath = try PrivacyConfidentialMerklePathWitnessV2(
            siblings: (0..<16).map { fixed32(UInt8($0 + 1)) },
            directions: Data((0..<16).map {
                UInt8((UInt64(leafIndex) >> UInt64($0)) & 1)
            }),
            root: root
        )
        let dummyPath = try PrivacyConfidentialMerklePathWitnessV2(
            siblings: (0..<16).map { fixed32(UInt8($0 + 33)) },
            directions: Data(repeating: 0, count: 16),
            root: root
        )
        return try KagemushaNoteMembershipWitness(
            leafIndex: leafIndex,
            inputPath: inputPath,
            dummyInputPath: dummyPath
        )
    }

    /// Rust-generated canonical bundle template. `payment(request:)` rebinds
    /// its peer-split request digest to the concrete P-256 fixture signature.
    /// The one-byte proof is deliberately opaque but satisfies the public
    /// structural contract; no test bypasses the native bundle-validation gate.
    private static let canonicalRecipientBundleArchiveHex = [
        "4e5254300000dd08ef107254cbcf59c74170bd235bac005503000000000000698b08642ee59045020000000000000000b305",
        "1b1a1973776966742d6b6167656d757368612d7472616e73706f727420010101020103010401050106014701080189010a01",
        "0b010c010d010e010f011004020000002044444444444444444444444444444444444444444444444444444444444444444b",
        "0100000000000000422052525252525252525252525252525252525252525252525252525252525252522054545454545454",
        "545454545454545454545454545454545454545454545454540402000000040100000096011b1a1973776966742d6b616765",
        "6d757368612d7472616e73706f727420010101020103010401050106014701080189010a010b010c010d010e010f01102042",
        "4242424242424242424242424242424242424242424242424242424242424220434343434343434343434343434343434343",
        "434343434343434343434343434316107d00000000000000000000000000000004020000005701000000000000004e2c2054",
        "5454545454545454545454545454545454545454545454545454545454545401010800000000000000002018000000000000",
        "00e3c98bd553300eafa14ec05dca52bd82784dbb29cdcb807179017700000000722053535353535353535353535353535353",
        "535353535353535353535353535353530400000000206e62171fc4d85584f96aa4c5b49c2633e66cea2719a361ae9cdf9625",
        "1c41b60820515151515151515151515151515151515151515151515151515151515151515104010000000400000000371514",
        "7472616e73706f72742d666978747572652d763320a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7",
        "a7a7a757191868616c6f322f6970612d70617374612d6379636c652d76313c3b6b6167656d757368612d7265637572736976",
        "652d7370656e642d737465702d65712d74776f2d706172656e742d65786163742d73746174652d76319e0157191868616c6f",
        "322f6970612d70617374612d6379636c652d76313c3b6b6167656d757368612d7265637572736976652d7370656e642d7374",
        "65702d65712d74776f2d706172656e742d65786163742d73746174652d7631206b36dd593e0690a5b4b423e9e1337a3dc1c3",
        "2f6580ab6d002c4deac80add071124191868616c6f322f6970612d70617374612d6379636c652d7631090100000000000000",
        "b0",
    ].joined()
}
