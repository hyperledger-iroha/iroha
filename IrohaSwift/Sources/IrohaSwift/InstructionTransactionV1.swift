import CryptoKit
import Foundation

/// Ordinary `Executable::Instructions` transactions whose signature is produced by the caller.
///
/// Apps that keep the account key behind their own user-presence gate build the exact payload,
/// sign `signingMessage(payload)` with that key, then assemble the versioned signed wire. The
/// same payload bytes are retained for exact-byte retries; nothing here re-signs or refreshes TTL.
public enum InstructionTransactionV1 {
    /// Canonical unsigned payload for one or more dynamic instruction frames.
    public static func payload(
        networkId: NetworkId,
        authority: String,
        creationTimeMs: UInt64,
        ttlMs: UInt64 = 100_000,
        frames: [TransactionInstructionFrame],
        feePayment: FeePaymentIntent
    ) throws -> Data {
        guard creationTimeMs > 0, ttlMs > 0 else { throw ExecutableBatchInputError.zeroTimeToLive }
        return try SingleInstructionSwiftNoritoEncoder.instructionsPayload(
            networkId: networkId, authority: authority, creationTimeMs: creationTimeMs,
            ttlMs: ttlMs, frames: frames, feePayment: feePayment)
    }

    /// The 32-byte Iroha prehash the account key signs.
    public static func signingMessage(payload: Data) -> Data { IrohaHash.hash(payload) }

    /// Assemble the versioned signed wire. An Ed25519 public key, when supplied, must verify
    /// the detached signature over this exact payload.
    public static func envelope(payload: Data, signature: Data, ed25519PublicKey: Data? = nil) throws
        -> SignedTransactionEnvelope {
        guard !payload.isEmpty, signature.count == 64 else { throw ExecutableBatchInputError.invalidInstructionFrame }
        if let ed25519PublicKey {
            guard let key = try? Curve25519.Signing.PublicKey(rawRepresentation: ed25519PublicKey),
                  key.isValidSignature(signature, for: signingMessage(payload: payload)) else {
                throw SigningKeyError.publicKeyUnavailable
            }
        }
        return SingleInstructionSwiftNoritoEncoder.signedEnvelope(transactionPayload: payload, signature: signature)
    }

    /// The Native ABI-bound decoder's JSON form of the envelope's unsigned payload, in the
    /// exact shape `ToriiClient.quoteFees(unsignedPayload:)` accepts. The selected chain
    /// discriminant controls every account literal; transaction wire itself carries no display prefix.
    /// Nil when strict native decoding or its thread-local chain scope is unavailable.
    public static func unsignedPayloadJSON(
        _ envelope: SignedTransactionEnvelope, chainDiscriminant: UInt16
    ) -> [String: ToriiJSONValue]? {
        guard let json = try? NoritoNativeBridge.shared.withChainDiscriminant(chainDiscriminant, {
            NoritoNativeBridge.shared.decodeSignedTransaction(envelope.norito)
        }),
              let data = json.data(using: .utf8),
              case let .object(root)? = try? JSONDecoder().decode(ToriiJSONValue.self, from: data),
              case let .object(payload)? = root["payload"] else { return nil }
        return payload
    }

    /// Quote the network fee for the payload built by `build`, then rebuild it with the quoted
    /// intent. The rebuilt payload must decode to exactly the payload Torii quoted.
    public static func quotedPayload(
        draftFeePayment: FeePaymentIntent,
        client: ToriiClient,
        canonicalAuth: ToriiCanonicalRequestAuth,
        chainDiscriminant: UInt16,
        build: (FeePaymentIntent) throws -> Data
    ) async throws -> (payload: Data, feePayment: FeePaymentIntent) {
        // Canonical signed-wire decoding rejects an absent/all-zero signature. Use the
        // same account credential that authenticates the quote to sign a local draft;
        // neither draft envelope is dispatched. The caller still confirms the final payload.
        let key = try SigningKey.ed25519(privateKey: canonicalAuth.privateKey)
        func decodeUnsigned(_ payload: Data) throws -> [String: ToriiJSONValue]? {
            let signature = try key.sign(signingMessage(payload: payload))
            return unsignedPayloadJSON(try envelope(payload: payload, signature: signature),
                chainDiscriminant: chainDiscriminant)
        }
        let draft = try build(draftFeePayment)
        guard let unsigned = try decodeUnsigned(draft) else {
            throw SwiftTransactionEncoderError.nativeBridgeUnavailable
        }
        let quoted = try await client.quoteAndApplyFees(unsignedPayload: unsigned, canonicalAuth: canonicalAuth)
        let payload = try build(quoted.quote.intent)
        guard try decodeUnsigned(payload) == quoted.payload else {
            throw ToriiClientError.invalidPayload("The rebuilt transaction differs from the quoted payload.")
        }
        return (payload, quoted.quote.intent)
    }
}

extension KagemushaWalletLedgerTransportV1 {
    /// Sole registered dynamic wire identity of the KAGEMUSHA ordinary ledger instruction.
    public static let instructionWireName = "iroha.kagemusha.wallet.ledger.v1"

    /// Exact Native-produced ledger frame (Activate, Load, Unload or CloseLoads) as an instruction.
    public static func instructionFrame(_ nativeFrame: Data) throws -> TransactionInstructionFrame {
        guard (1...65_536).contains(nativeFrame.count) else { throw ExecutableBatchInputError.invalidInstructionFrame }
        return try TransactionInstructionFrame(wireName: instructionWireName, framedPayload: nativeFrame)
    }
}

extension TransactionInstructionFrame {
    /// Construct the funding instruction for an active ledger wallet, using its expected ordinal and stable request identity.
    /// The optional signed online charge is independent of the amount credited offline.
    public static func kagemushaWalletIssueLoad(
        scheme: Data, wallet: Data, asset: Data, ordinal: KagemushaWalletUInt128V1, requestId: Data,
        amount: KagemushaWalletUInt128V1, charge: (quote: Data, beneficiary: String)? = nil
    ) throws -> Self {
        guard [scheme, wallet, asset, requestId].allSatisfy({ $0.count == 32 && $0.contains(where: { $0 != 0 }) }),
              amount.low != 0 || amount.high != 0 else { throw ExecutableBatchInputError.invalidInstructionFrame }
        func scalar(_ value: KagemushaWalletUInt128V1) -> Data {
            var field = CompactNoritoWriter()
            field.writeUInt64LE(value.low)
            field.writeUInt64LE(value.high)
            return field.data
        }
        var optional = CompactNoritoWriter()
        if let charge {
            guard (1...1_024).contains(charge.quote.count) else { throw ExecutableBatchInputError.invalidInstructionFrame }
            var quote = CompactNoritoWriter()
            quote.writeUInt64LE(UInt64(charge.quote.count))
            quote.writeBytes(charge.quote)
            var fields = CompactNoritoWriter()
            fields.writeField(quote.data)
            fields.writeField(try exactToriiAccountIdentityPayload(charge.beneficiary))
            optional.writeUInt8(1)
            optional.writeField(fields.data)
        } else {
            optional.writeUInt8(0)
        }
        var action = CompactNoritoWriter()
        action.writeUInt32LE(5)
        for field in [wallet, asset, scalar(ordinal), requestId, scalar(amount), optional.data] { action.writeField(field) }
        var root = CompactNoritoWriter()
        root.writeField(scheme)
        root.writeField(action.data)
        return try KagemushaWalletLedgerTransportV1.instructionFrame(noritoEncode(
            typeName: "iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLedgerV1", payload: root.data,
            flags: NoritoHeader.compactLen, payloadAlignment: 16))
    }

    /// Account-signed ledger activation for a Native-exported, hardware-authorized Bootstrap package.
    public static func kagemushaSpartanActivate(
        scheme: Data, asset: Data, manifestDigest: Data, wallet: Data, activation: Data
    ) throws -> Self {
        try spartanControl(wireName: "iroha.kagemusha.spartan.activate.v1", schema: "iroha.kagemusha.spartan.Activate.v1",
                           scopes: [scheme, asset, manifestDigest, wallet], original: activation, maximum: 32_768)
    }

    /// The completed Native Control claim and World-selected pins for an ordinary signed transaction.
    public static func kagemushaSpartanUnload(
        scheme: Data, asset: Data, manifestDigest: Data, claim: Data
    ) throws -> Self {
        try spartanControl(wireName: "iroha.kagemusha.spartan.unload.v1", schema: "iroha.kagemusha.spartan.Unload.v1",
                           scopes: [scheme, asset, manifestDigest], original: claim, maximum: 16_384)
    }

    private static func spartanControl(wireName: String, schema: String, scopes: [Data], original claim: Data, maximum: Int) throws -> Self {
        guard scopes.allSatisfy({ $0.count == 32 && $0.contains(where: { $0 != 0 }) }),
              (1...maximum).contains(claim.count) else {
            throw ExecutableBatchInputError.invalidInstructionFrame
        }
        var body = CompactNoritoWriter()
        for scope in scopes { body.writeField(scope) }
        var original = CompactNoritoWriter()
        original.writeUInt64LE(UInt64(claim.count))
        original.writeBytes(claim)
        body.writeField(original.data)
        return try Self(
            wireName: wireName,
            framedPayload: noritoEncode(typeName: schema,
                                         payload: body.data, flags: NoritoHeader.compactLen, payloadAlignment: 8)
        )
    }
}
