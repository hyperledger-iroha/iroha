import CryptoKit
import Foundation

enum SccpMessageProofBundleError: Error, Equatable {
    case invalid(String)
    case mismatch
    case missingSourceProof
}

struct SccpMessageProofBundleSummary: Equatable {
    let sourceDomain: UInt32
    let targetDomain: UInt32
    let messageId: String
    let payloadHash: String
    let commitmentRoot: String
    let finalityProofBytes: Data
}

@discardableResult
func requireSccpProofRequestBundleMatchesPublicInputs(
    targetDomain: UInt32,
    messageId: String,
    payloadHash: String,
    commitmentRoot: String,
    finalityHeight: UInt64,
    finalityBlockHash: String,
    bundleBytes: Data,
    sourceProofBytes: Data
) throws -> SccpMessageProofBundleSummary {
    let summary = try decodeSccpMessageProofBundleSummary(bundleBytes, field: "bundleBytes")
    guard summary.targetDomain == targetDomain,
          summary.messageId == messageId,
          summary.payloadHash == payloadHash,
          summary.commitmentRoot == commitmentRoot else {
        throw SccpMessageProofBundleError.mismatch
    }
    try requireSccpSourceProofMatchesBundle(
        sourceDomain: summary.sourceDomain,
        targetDomain: summary.targetDomain,
        messageId: summary.messageId,
        payloadHash: summary.payloadHash,
        commitmentRoot: summary.commitmentRoot,
        finalityHeight: finalityHeight,
        finalityBlockHash: finalityBlockHash,
        finalityProofBytes: summary.finalityProofBytes,
        sourceProofBytes: sourceProofBytes
    )
    return summary
}

func requireSccpSourceProofMatchesBundle(
    sourceDomain: UInt32,
    targetDomain: UInt32,
    messageId: String,
    payloadHash: String,
    commitmentRoot: String,
    finalityHeight: UInt64,
    finalityBlockHash: String,
    finalityProofBytes: Data,
    sourceProofBytes: Data
) throws {
    guard sourceDomain != sccpDomainSora || sourceProofBytes.isEmpty else {
        throw SccpMessageProofBundleError.invalid("sourceProofBytes must be empty for SORA source bundle")
    }
    guard sourceDomain == sccpDomainSora || !sourceProofBytes.isEmpty else {
        throw SccpMessageProofBundleError.missingSourceProof
    }
    guard sourceDomain != sccpDomainSora else {
        return
    }
    guard sourceProofBytes == finalityProofBytes else {
        throw SccpMessageProofBundleError.invalid("sourceProofBytes must match bundleBytes finality proof")
    }
    let sourceProof = try decodeSccpBundleSourceProofSummary(sourceProofBytes, field: "sourceProofBytes")
    let normalizedFinalityBlockHash = try requireSccpBundleNonZeroHex32(
        finalityBlockHash,
        field: "finalityBlockHash"
    )
    guard sourceProof.sourceDomain == sourceDomain,
          sourceProof.targetDomain == targetDomain,
          sourceProof.messageId == messageId,
          sourceProof.payloadHash == payloadHash,
          sourceProof.commitmentRoot == commitmentRoot,
          sourceProof.finalityHeight == finalityHeight,
          sourceProof.finalityBlockHash == normalizedFinalityBlockHash else {
        throw SccpMessageProofBundleError.invalid("sourceProofBytes must match bundleBytes and publicInputs")
    }
}

private let sccpBundleSourceChainProofEnvelopeSchema = "iroha_sccp::SccpSourceChainProofEnvelopeV1"
private let sccpBundleSourceEventDigestPrefixV1 = "sccp:source:event:v1"
private let sccpBundleMessagePrefixAssetRegisterV1 = "sccp:asset:register:v1"
private let sccpBundleMessagePrefixRouteActivateV1 = "sccp:route:activate:v1"
private let sccpBundleMessagePrefixTransferV1 = "sccp:transfer:v1"
private let sccpBundleMessagePrefixTokenAddV1 = "sccp:token:add:v1"
private let sccpBundleMessagePrefixTokenPauseV1 = "sccp:token:pause:v1"
private let sccpBundleMessagePrefixTokenResumeV1 = "sccp:token:resume:v1"
private let sccpBundleHubLeafPrefixV1 = "sccp:hub:leaf:v1"
private let sccpBundleHubNodePrefixV1 = "sccp:hub:node:v1"
private let sccpBundlePayloadHashPrefixV1 = "sccp:payload:v1"
private let sccpBundleCodecTextUtf8: UInt8 = 1
private let sccpBundleCodecEvmHex: UInt8 = 2
private let sccpBundleCodecSolanaBase58: UInt8 = 3
private let sccpBundleCodecTonRaw: UInt8 = 4
private let sccpBundleCodecTronBase58Check: UInt8 = 5
private let sccpBundleCodecSoraAssetId: UInt8 = 6
private let sccpBundleMaxSourceMerkleBranchNodes = 64
private let sccpBundleBase58Alphabet = Array("123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz")

private struct SccpBundleVecRange {
    let bytes: Data
    let nextOffset: Int
}

private struct SccpBundlePayloadSummary {
    let kind: String
    let sourceDomain: UInt32
    let targetDomain: UInt32
    let messageId: String
    let payloadHash: String
}

private struct SccpBundleCommitmentSummary {
    let kindCode: UInt8
    let targetDomain: UInt32
    let messageId: String
    let payloadHash: String
}

private struct SccpBundleSourceProofSummary {
    let sourceDomain: UInt32
    let targetDomain: UInt32
    let sourceChain: String
    let sourceProofPlan: UInt32
    let finalityModel: UInt32
    let messageId: String
    let payloadHash: String
    let sourceEventDigest: String
    let commitmentRoot: String
    let finalityHeight: UInt64
    let finalityBlockHash: String
    let finalizedHeaderHash: String
    let receiptOrMessageRoot: String
    let consensusProofBytes: Data
    let messageInclusionProofBytes: Data
    let inclusionBranch: [Data]
}

private func decodeSccpMessageProofBundleSummary(
    _ bundleBytes: Data,
    field: String
) throws -> SccpMessageProofBundleSummary {
    var offset = 0
    let version = try readSccpBundleU8(bundleBytes, offset: offset, field: "\(field).version")
    offset += 1
    guard version == 1 else {
        throw SccpMessageProofBundleError.invalid("\(field).version")
    }
    guard offset + 32 <= bundleBytes.count else {
        throw SccpMessageProofBundleError.invalid("\(field).commitment_root")
    }
    let commitmentRoot = "0x" + Data(bundleBytes[offset..<(offset + 32)]).hexEncodedString()
    offset += 32
    var range = try readSccpBundleCanonicalVec(bundleBytes, offset: offset, field: "\(field).commitment")
    let commitmentBytes = range.bytes
    offset = range.nextOffset
    range = try readSccpBundleCanonicalVec(bundleBytes, offset: offset, field: "\(field).merkle_proof")
    let merkleProofBytes = range.bytes
    offset = range.nextOffset
    range = try readSccpBundleCanonicalVec(bundleBytes, offset: offset, field: "\(field).payload")
    let payloadBytes = range.bytes
    offset = range.nextOffset
    range = try readSccpBundleCanonicalVec(bundleBytes, offset: offset, field: "\(field).finality_proof")
    let finalityProofBytes = range.bytes
    offset = range.nextOffset
    try requireSccpBundleExactEnd(offset, bundleBytes, field: field)

    let payload = try decodeSccpBundlePayloadSummary(payloadBytes, field: "\(field).payload")
    let expectedCommitmentBytes = try canonicalSccpBundleCommitmentBytes(
        kind: payload.kind,
        targetDomain: payload.targetDomain,
        messageId: payload.messageId,
        payloadHash: payload.payloadHash
    )
    guard commitmentBytes == expectedCommitmentBytes else {
        throw SccpMessageProofBundleError.invalid("\(field).commitment")
    }
    let commitment = try decodeSccpBundleCommitmentSummary(commitmentBytes, field: field)
    let expectedKindCode = try sccpBundleMessageKindCode(payload.kind)
    guard commitment.kindCode == expectedKindCode else {
        throw SccpMessageProofBundleError.invalid("\(field).commitment")
    }
    let expectedRoot = try merkleRootFromCanonicalSccpBundleCommitmentBytes(
        commitmentBytes,
        merkleProofBytes: merkleProofBytes,
        field: "\(field).merkle_proof"
    )
    guard commitmentRoot == expectedRoot else {
        throw SccpMessageProofBundleError.invalid("\(field).commitment_root")
    }
    return SccpMessageProofBundleSummary(
        sourceDomain: payload.sourceDomain,
        targetDomain: commitment.targetDomain,
        messageId: commitment.messageId,
        payloadHash: commitment.payloadHash,
        commitmentRoot: commitmentRoot,
        finalityProofBytes: finalityProofBytes
    )
}

private func decodeSccpBundleSourceProofSummary(
    _ sourceProofBytes: Data,
    field: String
) throws -> SccpBundleSourceProofSummary {
    guard let frame = noritoDecodeFrame(sourceProofBytes),
          frame.header.compression == .none,
          frame.header.schema == noritoSchemaHash(forTypeName: sccpBundleSourceChainProofEnvelopeSchema) else {
        throw SccpMessageProofBundleError.invalid("\(field) must decode as SccpSourceChainProofEnvelopeV1")
    }
    let compactLen = (frame.header.flags & NoritoHeader.compactLen) != 0
    var reader = SccpBundleNoritoReader(data: frame.payload)
    let proof: SccpBundleSourceProofSummary
    do {
        let version = try readSccpBundleNoritoField(&reader, compactLen: compactLen, field: "\(field).version") { child in
            try child.readUInt8(field: "\(field).version")
        }
        guard version == 1 else {
            throw SccpMessageProofBundleError.invalid("\(field).version")
        }
        let sourceDomain = try readSccpBundleNoritoU32Field(
            &reader,
            compactLen: compactLen,
            field: "\(field).source_domain"
        )
        let targetDomain = try readSccpBundleNoritoU32Field(
            &reader,
            compactLen: compactLen,
            field: "\(field).target_domain"
        )
        let sourceChain = try readSccpBundleNoritoField(
            &reader,
            compactLen: compactLen,
            field: "\(field).source_chain"
        ) { child in
            try readSccpBundleNoritoString(&child, compactLen: compactLen, field: "\(field).source_chain")
        }
        let sourceProofPlan = try readSccpBundleNoritoU32Field(
            &reader,
            compactLen: compactLen,
            field: "\(field).source_proof_plan"
        )
        let finalityModel = try readSccpBundleNoritoU32Field(
            &reader,
            compactLen: compactLen,
            field: "\(field).finality_model"
        )
        let messageId = try readSccpBundleNoritoHex32Field(
            &reader,
            compactLen: compactLen,
            field: "\(field).message_id"
        )
        let payloadHash = try readSccpBundleNoritoHex32Field(
            &reader,
            compactLen: compactLen,
            field: "\(field).payload_hash"
        )
        let sourceEventDigest = try readSccpBundleNoritoHex32Field(
            &reader,
            compactLen: compactLen,
            field: "\(field).source_event_digest"
        )
        let commitmentRoot = try readSccpBundleNoritoHex32Field(
            &reader,
            compactLen: compactLen,
            field: "\(field).commitment_root"
        )
        let finalityHeight = try readSccpBundleNoritoField(
            &reader,
            compactLen: compactLen,
            field: "\(field).finality_height"
        ) { child in
            try child.readUInt64LE(field: "\(field).finality_height")
        }
        let finalityBlockHash = try readSccpBundleNoritoHex32Field(
            &reader,
            compactLen: compactLen,
            field: "\(field).finality_block_hash"
        )
        let finalizedHeaderHash = try readSccpBundleNoritoHex32Field(
            &reader,
            compactLen: compactLen,
            field: "\(field).finalized_header_hash"
        )
        let receiptOrMessageRoot = try readSccpBundleNoritoHex32Field(
            &reader,
            compactLen: compactLen,
            field: "\(field).receipt_or_message_root"
        )
        let consensusProofBytes = try readSccpBundleNoritoField(
            &reader,
            compactLen: compactLen,
            field: "\(field).consensus_proof"
        ) { child in
            try readSccpBundleNoritoRawByteVec(&child, field: "\(field).consensus_proof")
        }
        let messageInclusionProofBytes = try readSccpBundleNoritoField(
            &reader,
            compactLen: compactLen,
            field: "\(field).message_inclusion_proof"
        ) { child in
            try readSccpBundleNoritoRawByteVec(&child, field: "\(field).message_inclusion_proof")
        }
        let inclusionBranch = try readSccpBundleNoritoField(
            &reader,
            compactLen: compactLen,
            field: "\(field).inclusion_branch"
        ) { child in
            try readSccpBundleNoritoRawByteVecSequence(
                &child,
                compactLen: compactLen,
                field: "\(field).inclusion_branch"
            )
        }
        try requireSccpBundleExactEnd(reader.offset, frame.payload, field: field)
        proof = SccpBundleSourceProofSummary(
            sourceDomain: sourceDomain,
            targetDomain: targetDomain,
            sourceChain: sourceChain,
            sourceProofPlan: sourceProofPlan,
            finalityModel: finalityModel,
            messageId: messageId,
            payloadHash: payloadHash,
            sourceEventDigest: sourceEventDigest,
            commitmentRoot: commitmentRoot,
            finalityHeight: finalityHeight,
            finalityBlockHash: finalityBlockHash,
            finalizedHeaderHash: finalizedHeaderHash,
            receiptOrMessageRoot: receiptOrMessageRoot,
            consensusProofBytes: consensusProofBytes,
            messageInclusionProofBytes: messageInclusionProofBytes,
            inclusionBranch: inclusionBranch
        )
    } catch let error as SccpMessageProofBundleError {
        throw error
    } catch {
        throw SccpMessageProofBundleError.invalid("\(field) must decode as SccpSourceChainProofEnvelopeV1")
    }

    guard proof.sourceDomain != sccpDomainSora else {
        throw SccpMessageProofBundleError.invalid("\(field).source_domain")
    }
    try requireSupportedSccpBundleDomain(proof.sourceDomain, field: "\(field).source_domain")
    try requireSupportedSccpBundleDomain(proof.targetDomain, field: "\(field).target_domain")
    let expectedSourceChain = try sccpBundleSourceChainKeyForDomain(proof.sourceDomain)
    let expectedSourceProofPlan = try sccpBundleSourceProofPlanDiscriminantForDomain(proof.sourceDomain)
    let expectedFinalityModel = try sccpBundleFinalityModelDiscriminantForDomain(proof.sourceDomain)
    guard proof.sourceDomain != proof.targetDomain,
          proof.sourceChain == expectedSourceChain,
          proof.sourceProofPlan == expectedSourceProofPlan,
          proof.finalityModel == expectedFinalityModel,
          proof.finalityHeight != 0,
          !proof.consensusProofBytes.isEmpty,
          !proof.messageInclusionProofBytes.isEmpty,
          !proof.inclusionBranch.isEmpty,
          proof.inclusionBranch.count <= sccpBundleMaxSourceMerkleBranchNodes else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    for (index, sibling) in proof.inclusionBranch.enumerated() {
        guard sibling.count == 32 else {
            throw SccpMessageProofBundleError.invalid("\(field).inclusion_branch[\(index)]")
        }
    }
    _ = try requireSccpBundleNonZeroHex32(proof.messageId, field: "\(field).message_id")
    _ = try requireSccpBundleNonZeroHex32(proof.payloadHash, field: "\(field).payload_hash")
    _ = try requireSccpBundleNonZeroHex32(proof.sourceEventDigest, field: "\(field).source_event_digest")
    _ = try requireSccpBundleNonZeroHex32(proof.commitmentRoot, field: "\(field).commitment_root")
    _ = try requireSccpBundleNonZeroHex32(proof.finalityBlockHash, field: "\(field).finality_block_hash")
    _ = try requireSccpBundleNonZeroHex32(proof.finalizedHeaderHash, field: "\(field).finalized_header_hash")
    _ = try requireSccpBundleNonZeroHex32(proof.receiptOrMessageRoot, field: "\(field).receipt_or_message_root")
    let expectedSourceEventDigest = try sccpBundleSourceEventDigest(
        sourceDomain: proof.sourceDomain,
        targetDomain: proof.targetDomain,
        messageId: proof.messageId,
        payloadHash: proof.payloadHash
    )
    guard proof.sourceEventDigest == expectedSourceEventDigest else {
        throw SccpMessageProofBundleError.invalid("\(field).source_event_digest")
    }
    return proof
}

private func decodeSccpBundlePayloadSummary(
    _ payloadBytes: Data,
    field: String
) throws -> SccpBundlePayloadSummary {
    let discriminant = try readSccpBundleU8(payloadBytes, offset: 0, field: "\(field).kind")
    let body = Data(payloadBytes.dropFirst())
    let version = try readSccpBundleU8(body, offset: 0, field: "\(field).version")
    guard version == 1 else {
        throw SccpMessageProofBundleError.invalid("\(field).version")
    }
    var cursor = 1

    func readDomain(_ name: String) throws -> UInt32 {
        let domain = try readSccpBundleU32Le(body, offset: cursor, field: "\(field).\(name)")
        cursor += 4
        try requireSupportedSccpBundleDomain(domain, field: "\(field).\(name)")
        return domain
    }

    func readU64(_ name: String) throws {
        _ = try readSccpBundleU64Le(body, offset: cursor, field: "\(field).\(name)")
        cursor += 8
    }

    func readCodec(_ name: String) throws -> UInt8 {
        let codec = try readSccpBundleU8(body, offset: cursor, field: "\(field).\(name)")
        cursor += 1
        return try normalizeSccpBundleCodecId(codec, field: "\(field).\(name)")
    }

    func readCodecValue(_ codec: UInt8, _ name: String) throws {
        let range = try readSccpBundleCanonicalVec(body, offset: cursor, field: "\(field).\(name)")
        cursor = range.nextOffset
        try validateCanonicalSccpBundleCodecBytes(codec, range.bytes, field: "\(field).\(name)")
    }

    func readFixed(_ byteCount: Int, _ name: String) throws -> Data {
        guard cursor + byteCount <= body.count else {
            throw SccpMessageProofBundleError.invalid("\(field).\(name)")
        }
        let value = Data(body[cursor..<(cursor + byteCount)])
        cursor += byteCount
        return value
    }

    func summary(
        kind: String,
        sourceDomain: UInt32,
        targetDomain: UInt32,
        prefix: String
    ) -> SccpBundlePayloadSummary {
        SccpBundlePayloadSummary(
            kind: kind,
            sourceDomain: sourceDomain,
            targetDomain: targetDomain,
            messageId: sccpBundlePrefixedKeccakHex(prefix: prefix, payload: body),
            payloadHash: sccpBundleHashHex(prefix: sccpBundlePayloadHashPrefixV1, payload: payloadBytes)
        )
    }

    switch discriminant {
    case 0:
        let targetDomain = try readDomain("target_domain")
        let sourceDomain = try readDomain("home_domain")
        try readU64("nonce")
        let assetIdCodec = try readCodec("asset_id_codec")
        try readCodecValue(assetIdCodec, "asset_id")
        _ = try readSccpBundleU8(body, offset: cursor, field: "\(field).decimals")
        cursor += 1
        try requireSccpBundleExactEnd(cursor, body, field: field)
        return summary(
            kind: "AssetRegister",
            sourceDomain: sourceDomain,
            targetDomain: targetDomain,
            prefix: sccpBundleMessagePrefixAssetRegisterV1
        )
    case 1:
        let sourceDomain = try readDomain("source_domain")
        let targetDomain = try readDomain("target_domain")
        guard sourceDomain != targetDomain else {
            throw SccpMessageProofBundleError.invalid("\(field).target_domain")
        }
        try readU64("nonce")
        let assetIdCodec = try readCodec("asset_id_codec")
        try readCodecValue(assetIdCodec, "asset_id")
        let routeIdCodec = try readCodec("route_id_codec")
        try readCodecValue(routeIdCodec, "route_id")
        try requireSccpBundleExactEnd(cursor, body, field: field)
        return summary(
            kind: "RouteActivate",
            sourceDomain: sourceDomain,
            targetDomain: targetDomain,
            prefix: sccpBundleMessagePrefixRouteActivateV1
        )
    case 2:
        let sourceDomain = try readDomain("source_domain")
        let targetDomain = try readDomain("dest_domain")
        guard sourceDomain != targetDomain else {
            throw SccpMessageProofBundleError.invalid("\(field).dest_domain")
        }
        try readU64("nonce")
        _ = try readDomain("asset_home_domain")
        let assetIdCodec = try readCodec("asset_id_codec")
        try readCodecValue(assetIdCodec, "asset_id")
        let amount = try readFixed(16, "amount")
        guard amount.contains(where: { $0 != 0 }) else {
            throw SccpMessageProofBundleError.invalid("\(field).amount")
        }
        let senderCodec = try readCodec("sender_codec")
        let expectedSenderCodec = try sccpBundleCounterpartyAccountCodec(sourceDomain)
        guard senderCodec == expectedSenderCodec else {
            throw SccpMessageProofBundleError.invalid("\(field).sender_codec")
        }
        try readCodecValue(senderCodec, "sender")
        let recipientCodec = try readCodec("recipient_codec")
        let expectedRecipientCodec = try sccpBundleCounterpartyAccountCodec(targetDomain)
        guard recipientCodec == expectedRecipientCodec else {
            throw SccpMessageProofBundleError.invalid("\(field).recipient_codec")
        }
        try readCodecValue(recipientCodec, "recipient")
        let routeIdCodec = try readCodec("route_id_codec")
        try readCodecValue(routeIdCodec, "route_id")
        try requireSccpBundleExactEnd(cursor, body, field: field)
        return summary(
            kind: "Transfer",
            sourceDomain: sourceDomain,
            targetDomain: targetDomain,
            prefix: sccpBundleMessagePrefixTransferV1
        )
    case 3:
        let targetDomain = try readDomain("target_domain")
        try readU64("nonce")
        let assetId = try readFixed(32, "sora_asset_id")
        guard assetId.contains(where: { $0 != 0 }) else {
            throw SccpMessageProofBundleError.invalid("\(field).sora_asset_id")
        }
        _ = try readSccpBundleU8(body, offset: cursor, field: "\(field).decimals")
        cursor += 1
        let name = try readFixed(32, "name")
        guard fixedAsciiFieldIsNonEmpty(name) else {
            throw SccpMessageProofBundleError.invalid("\(field).name")
        }
        let symbol = try readFixed(32, "symbol")
        guard fixedAsciiFieldIsNonEmpty(symbol) else {
            throw SccpMessageProofBundleError.invalid("\(field).symbol")
        }
        try requireSccpBundleExactEnd(cursor, body, field: field)
        return summary(
            kind: "TokenAdd",
            sourceDomain: sccpDomainSora,
            targetDomain: targetDomain,
            prefix: sccpBundleMessagePrefixTokenAddV1
        )
    case 4, 5:
        let targetDomain = try readDomain("target_domain")
        try readU64("nonce")
        let assetId = try readFixed(32, "sora_asset_id")
        guard assetId.contains(where: { $0 != 0 }) else {
            throw SccpMessageProofBundleError.invalid("\(field).sora_asset_id")
        }
        try requireSccpBundleExactEnd(cursor, body, field: field)
        let isPause = discriminant == 4
        return summary(
            kind: isPause ? "TokenPause" : "TokenResume",
            sourceDomain: sccpDomainSora,
            targetDomain: targetDomain,
            prefix: isPause ? sccpBundleMessagePrefixTokenPauseV1 : sccpBundleMessagePrefixTokenResumeV1
        )
    default:
        throw SccpMessageProofBundleError.invalid(field)
    }
}

private func decodeSccpBundleCommitmentSummary(
    _ commitmentBytes: Data,
    field: String
) throws -> SccpBundleCommitmentSummary {
    guard commitmentBytes.count == 70 else {
        throw SccpMessageProofBundleError.invalid("\(field).commitment")
    }
    let version = try readSccpBundleU8(commitmentBytes, offset: 0, field: "\(field).commitment.version")
    guard version == 1 else {
        throw SccpMessageProofBundleError.invalid("\(field).commitment.version")
    }
    return SccpBundleCommitmentSummary(
        kindCode: try readSccpBundleU8(commitmentBytes, offset: 1, field: "\(field).commitment.kind"),
        targetDomain: try readSccpBundleU32Le(
            commitmentBytes,
            offset: 2,
            field: "\(field).commitment.target_domain"
        ),
        messageId: "0x" + Data(commitmentBytes[6..<38]).hexEncodedString(),
        payloadHash: "0x" + Data(commitmentBytes[38..<70]).hexEncodedString()
    )
}

private func merkleRootFromCanonicalSccpBundleCommitmentBytes(
    _ commitmentBytes: Data,
    merkleProofBytes: Data,
    field: String
) throws -> String {
    var offset = 0
    let stepCount = try readSccpBundleU32Le(merkleProofBytes, offset: offset, field: "\(field).steps")
    offset += 4
    var current = sccpBundleHashBytes(prefix: sccpBundleHubLeafPrefixV1, payload: commitmentBytes)
    for index in 0..<Int(stepCount) {
        guard offset + 33 <= merkleProofBytes.count else {
            throw SccpMessageProofBundleError.invalid("\(field).steps[\(index)]")
        }
        let sibling = Data(merkleProofBytes[offset..<(offset + 32)])
        offset += 32
        let siblingIsLeft = try readSccpBundleU8(
            merkleProofBytes,
            offset: offset,
            field: "\(field).steps[\(index)].sibling_is_left"
        )
        offset += 1
        guard siblingIsLeft == 0 || siblingIsLeft == 1 else {
            throw SccpMessageProofBundleError.invalid("\(field).steps[\(index)].sibling_is_left")
        }
        var payload = Data()
        if siblingIsLeft == 1 {
            payload.append(sibling)
            payload.append(current)
        } else {
            payload.append(current)
            payload.append(sibling)
        }
        current = sccpBundleHashBytes(prefix: sccpBundleHubNodePrefixV1, payload: payload)
    }
    try requireSccpBundleExactEnd(offset, merkleProofBytes, field: field)
    return "0x" + current.hexEncodedString()
}

private func canonicalSccpBundleCommitmentBytes(
    kind: String,
    targetDomain: UInt32,
    messageId: String,
    payloadHash: String
) throws -> Data {
    var out = Data()
    out.append(1)
    out.append(try sccpBundleMessageKindCode(kind))
    appendSccpBundleU32Le(targetDomain, to: &out)
    guard let messageIdBytes = Data(hexString: String(messageId.dropFirst(2))),
          let payloadHashBytes = Data(hexString: String(payloadHash.dropFirst(2))),
          messageIdBytes.count == 32,
          payloadHashBytes.count == 32 else {
        throw SccpMessageProofBundleError.invalid("commitment")
    }
    out.append(messageIdBytes)
    out.append(payloadHashBytes)
    return out
}

private func sccpBundleMessageKindCode(_ kind: String) throws -> UInt8 {
    switch kind {
    case "Burn":
        return 0
    case "TokenAdd":
        return 1
    case "TokenPause":
        return 2
    case "TokenResume":
        return 3
    case "AssetRegister":
        return 4
    case "RouteActivate":
        return 5
    case "Transfer":
        return 6
    default:
        throw SccpMessageProofBundleError.invalid("messageKind")
    }
}

private func requireSupportedSccpBundleDomain(_ domain: UInt32, field: String) throws {
    guard domain == sccpDomainSora ||
          domain == sccpDomainEthereum ||
          domain == sccpDomainBsc ||
          domain == sccpDomainSolana ||
          domain == sccpDomainTon ||
          domain == sccpDomainTron else {
        throw SccpMessageProofBundleError.invalid(field)
    }
}

private func normalizeSccpBundleCodecId(_ value: UInt8, field: String) throws -> UInt8 {
    guard value == sccpBundleCodecTextUtf8 ||
          value == sccpBundleCodecEvmHex ||
          value == sccpBundleCodecSolanaBase58 ||
          value == sccpBundleCodecTonRaw ||
          value == sccpBundleCodecTronBase58Check ||
          value == sccpBundleCodecSoraAssetId else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    return value
}

private func sccpBundleCounterpartyAccountCodec(_ domain: UInt32) throws -> UInt8 {
    switch domain {
    case sccpDomainSora:
        return sccpBundleCodecTextUtf8
    case sccpDomainEthereum, sccpDomainBsc:
        return sccpBundleCodecEvmHex
    case sccpDomainSolana:
        return sccpBundleCodecSolanaBase58
    case sccpDomainTon:
        return sccpBundleCodecTonRaw
    case sccpDomainTron:
        return sccpBundleCodecTronBase58Check
    default:
        throw SccpMessageProofBundleError.invalid("domain")
    }
}

private func validateCanonicalSccpBundleCodecBytes(
    _ codec: UInt8,
    _ raw: Data,
    field: String
) throws {
    switch codec {
    case sccpBundleCodecTextUtf8:
        guard let text = String(data: raw, encoding: .utf8),
              !text.isEmpty,
              Data(text.utf8) == raw else {
            throw SccpMessageProofBundleError.invalid(field)
        }
    case sccpBundleCodecEvmHex:
        guard let text = String(data: raw, encoding: .utf8),
              Data(text.utf8) == raw else {
            throw SccpMessageProofBundleError.invalid(field)
        }
        try validateCanonicalSccpBundleEvmHexAddress(text, field: field)
    case sccpBundleCodecSolanaBase58:
        guard let text = String(data: raw, encoding: .utf8) else {
            throw SccpMessageProofBundleError.invalid(field)
        }
        _ = try decodeSccpBundleBase58Fixed(text, field: field, byteCount: 32)
    case sccpBundleCodecTonRaw:
        guard let text = String(data: raw, encoding: .utf8) else {
            throw SccpMessageProofBundleError.invalid(field)
        }
        try validateSccpBundleTonRawAddress(text, field: field)
    case sccpBundleCodecTronBase58Check:
        guard let text = String(data: raw, encoding: .utf8) else {
            throw SccpMessageProofBundleError.invalid(field)
        }
        _ = try sccpBundleTronBase58CheckPayload(text, field: field)
    case sccpBundleCodecSoraAssetId:
        guard raw.count == 32 else {
            throw SccpMessageProofBundleError.invalid(field)
        }
    default:
        throw SccpMessageProofBundleError.invalid(field)
    }
}

private func readSccpBundleCanonicalVec(
    _ bytes: Data,
    offset: Int,
    field: String
) throws -> SccpBundleVecRange {
    let length = Int(try readSccpBundleU32Le(bytes, offset: offset, field: "\(field).length"))
    let start = offset + 4
    let end = start + length
    guard offset >= 0, start >= 4, end >= start, end <= bytes.count else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    return SccpBundleVecRange(bytes: Data(bytes[start..<end]), nextOffset: end)
}

private func readSccpBundleU8(_ data: Data, offset: Int, field: String) throws -> UInt8 {
    guard offset >= 0, offset + 1 <= data.count else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    return data[offset]
}

private func readSccpBundleU32Le(_ data: Data, offset: Int, field: String) throws -> UInt32 {
    guard offset >= 0, offset + 4 <= data.count else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    return UInt32(data[offset])
        | (UInt32(data[offset + 1]) << 8)
        | (UInt32(data[offset + 2]) << 16)
        | (UInt32(data[offset + 3]) << 24)
}

private func readSccpBundleU64Le(_ data: Data, offset: Int, field: String) throws -> UInt64 {
    guard offset >= 0, offset + 8 <= data.count else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    var value: UInt64 = 0
    for index in 0..<8 {
        value |= UInt64(data[offset + index]) << UInt64(index * 8)
    }
    return value
}

private struct SccpBundleNoritoReader {
    let data: Data
    var offset = 0

    mutating func readUInt8(field: String) throws -> UInt8 {
        guard offset < data.count else {
            throw SccpMessageProofBundleError.invalid(field)
        }
        let value = data[data.startIndex + offset]
        offset += 1
        return value
    }

    mutating func readUInt32LE(field: String) throws -> UInt32 {
        let bytes = try readBytes(4, field: field)
        var value: UInt32 = 0
        for (index, byte) in bytes.enumerated() {
            value |= UInt32(byte) << UInt32(index * 8)
        }
        return value
    }

    mutating func readUInt64LE(field: String) throws -> UInt64 {
        let bytes = try readBytes(8, field: field)
        var value: UInt64 = 0
        for (index, byte) in bytes.enumerated() {
            value |= UInt64(byte) << UInt64(index * 8)
        }
        return value
    }

    mutating func readBytes(_ count: Int, field: String) throws -> Data {
        guard count >= 0, offset + count <= data.count else {
            throw SccpMessageProofBundleError.invalid(field)
        }
        let start = data.startIndex + offset
        let out = Data(data[start..<(start + count)])
        offset += count
        return out
    }

    mutating func readVarint(field: String) throws -> UInt64 {
        var shift: UInt64 = 0
        var value: UInt64 = 0
        while true {
            let byte = try readUInt8(field: field)
            guard shift < 64 else {
                throw SccpMessageProofBundleError.invalid(field)
            }
            value |= UInt64(byte & 0x7f) << shift
            if (byte & 0x80) == 0 {
                return value
            }
            shift += 7
        }
    }
}

private func readSccpBundleNoritoLength(
    _ reader: inout SccpBundleNoritoReader,
    compactLen: Bool,
    field: String
) throws -> Int {
    let length = compactLen
        ? try reader.readVarint(field: field)
        : try reader.readUInt64LE(field: field)
    guard length <= UInt64(Int.max) else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    return Int(length)
}

private func readSccpBundleNoritoField<T>(
    _ reader: inout SccpBundleNoritoReader,
    compactLen: Bool,
    field: String,
    _ decode: (inout SccpBundleNoritoReader) throws -> T
) throws -> T {
    let length = try readSccpBundleNoritoLength(&reader, compactLen: compactLen, field: field)
    var child = SccpBundleNoritoReader(data: try reader.readBytes(length, field: field))
    let value = try decode(&child)
    guard child.offset == child.data.count else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    return value
}

private func readSccpBundleNoritoU32Field(
    _ reader: inout SccpBundleNoritoReader,
    compactLen: Bool,
    field: String
) throws -> UInt32 {
    try readSccpBundleNoritoField(&reader, compactLen: compactLen, field: field) {
        try $0.readUInt32LE(field: field)
    }
}

private func readSccpBundleNoritoHex32Field(
    _ reader: inout SccpBundleNoritoReader,
    compactLen: Bool,
    field: String
) throws -> String {
    try readSccpBundleNoritoField(&reader, compactLen: compactLen, field: field) {
        "0x" + (try $0.readBytes(32, field: field)).hexEncodedString()
    }
}

private func readSccpBundleNoritoString(
    _ reader: inout SccpBundleNoritoReader,
    compactLen: Bool,
    field: String
) throws -> String {
    let length = try readSccpBundleNoritoLength(&reader, compactLen: compactLen, field: field)
    let bytes = try reader.readBytes(length, field: field)
    guard let value = String(data: bytes, encoding: .utf8),
          Data(value.utf8) == bytes else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    return value
}

private func readSccpBundleNoritoRawByteVec(
    _ reader: inout SccpBundleNoritoReader,
    field: String
) throws -> Data {
    let length = try readSccpBundleNoritoLength(&reader, compactLen: false, field: field)
    return try reader.readBytes(length, field: field)
}

private func readSccpBundleNoritoRawByteVecSequence(
    _ reader: inout SccpBundleNoritoReader,
    compactLen: Bool,
    field: String
) throws -> [Data] {
    let count = try readSccpBundleNoritoLength(&reader, compactLen: false, field: field)
    var out: [Data] = []
    out.reserveCapacity(count)
    for index in 0..<count {
        let elementLength = try readSccpBundleNoritoLength(
            &reader,
            compactLen: compactLen,
            field: "\(field)[\(index)]"
        )
        var child = SccpBundleNoritoReader(
            data: try reader.readBytes(elementLength, field: "\(field)[\(index)]")
        )
        let value = try readSccpBundleNoritoRawByteVec(&child, field: "\(field)[\(index)]")
        guard child.offset == child.data.count else {
            throw SccpMessageProofBundleError.invalid("\(field)[\(index)]")
        }
        out.append(value)
    }
    return out
}

private func requireSccpBundleExactEnd(_ offset: Int, _ data: Data, field: String) throws {
    guard offset == data.count else {
        throw SccpMessageProofBundleError.invalid(field)
    }
}

private func appendSccpBundleU32Le(_ value: UInt32, to out: inout Data) {
    out.append(UInt8(value & 0xff))
    out.append(UInt8((value >> 8) & 0xff))
    out.append(UInt8((value >> 16) & 0xff))
    out.append(UInt8((value >> 24) & 0xff))
}

private func sccpBundleSourceChainKeyForDomain(_ domain: UInt32) throws -> String {
    switch domain {
    case sccpDomainSora:
        return "sora"
    case sccpDomainEthereum:
        return "eth"
    case sccpDomainBsc:
        return "bsc"
    case sccpDomainSolana:
        return "sol"
    case sccpDomainTon:
        return "ton"
    case sccpDomainTron:
        return "tron"
    default:
        throw SccpMessageProofBundleError.invalid("source_domain")
    }
}

private func sccpBundleSourceProofPlanDiscriminantForDomain(_ domain: UInt32) throws -> UInt32 {
    switch domain {
    case sccpDomainEthereum:
        return 1
    case sccpDomainBsc:
        return 2
    case sccpDomainSolana:
        return 3
    case sccpDomainTon:
        return 4
    case sccpDomainTron:
        return 5
    default:
        throw SccpMessageProofBundleError.invalid("source_proof_plan")
    }
}

private func sccpBundleFinalityModelDiscriminantForDomain(_ domain: UInt32) throws -> UInt32 {
    switch domain {
    case sccpDomainEthereum:
        return 0
    case sccpDomainBsc:
        return 1
    case sccpDomainSolana:
        return 2
    case sccpDomainTon:
        return 3
    case sccpDomainTron:
        return 4
    default:
        throw SccpMessageProofBundleError.invalid("finality_model")
    }
}

private func normalizeSccpBundleHex32(_ value: String, field: String) throws -> String {
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    var hex = value
    if hex.lowercased().hasPrefix("0x") {
        hex.removeFirst(2)
    }
    guard hex.unicodeScalars.allSatisfy({ !CharacterSet.whitespacesAndNewlines.contains($0) }) else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    hex = hex.lowercased()
    guard hex.count == 64, let bytes = Data(hexString: hex), bytes.count == 32 else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    return "0x" + bytes.hexEncodedString()
}

private func requireSccpBundleNonZeroHex32(_ value: String, field: String) throws -> String {
    let normalized = try normalizeSccpBundleHex32(value, field: field)
    let bytes = Data(hexString: String(normalized.dropFirst(2))) ?? Data()
    guard bytes.contains(where: { $0 != 0 }) else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    return normalized
}

private func sccpBundleSourceEventDigest(
    sourceDomain: UInt32,
    targetDomain: UInt32,
    messageId: String,
    payloadHash: String
) throws -> String {
    guard let messageIdBytes = Data(hexString: String(messageId.dropFirst(2))),
          let payloadHashBytes = Data(hexString: String(payloadHash.dropFirst(2))),
          messageIdBytes.count == 32,
          payloadHashBytes.count == 32 else {
        throw SccpMessageProofBundleError.invalid("source_event_digest")
    }
    var payload = Data([1])
    appendSccpBundleU32Le(sourceDomain, to: &payload)
    appendSccpBundleU32Le(targetDomain, to: &payload)
    payload.append(messageIdBytes)
    payload.append(payloadHashBytes)
    return sccpBundleHashHex(prefix: sccpBundleSourceEventDigestPrefixV1, payload: payload)
}

private func fixedAsciiFieldIsNonEmpty(_ raw: Data) -> Bool {
    let prefix = raw[..<(raw.firstIndex(of: 0) ?? raw.endIndex)]
    return prefix.contains { $0 != 0 }
}

private func validateCanonicalSccpBundleEvmHexAddress(_ text: String, field: String) throws {
    guard text.hasPrefix("0x") else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    let payload = Array(text.utf8.dropFirst(2))
    guard text.utf8.count == 42,
          payload.count == 40,
          payload.allSatisfy({ byte in
              (byte >= 0x30 && byte <= 0x39) ||
              (byte >= 0x41 && byte <= 0x46) ||
              (byte >= 0x61 && byte <= 0x66)
          }) else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    let lowercasePayload = payload.map { byte -> UInt8 in
        if byte >= 0x41 && byte <= 0x46 {
            return byte + 0x20
        }
        return byte
    }
    let checksum = irohaKeccak256(Data(lowercasePayload))
    for (index, byte) in payload.enumerated() {
        if byte >= 0x30 && byte <= 0x39 {
            continue
        }
        let checksumByte = checksum[index / 2]
        let checksumNibble = index % 2 == 0 ? checksumByte >> 4 : checksumByte & 0x0f
        let shouldBeUppercase = checksumNibble >= 8
        if shouldBeUppercase {
            guard byte >= 0x41 && byte <= 0x46 else {
                throw SccpMessageProofBundleError.invalid(field)
            }
        } else {
            guard byte >= 0x61 && byte <= 0x66 else {
                throw SccpMessageProofBundleError.invalid(field)
            }
        }
    }
}

private func decodeSccpBundleBase58Fixed(_ value: String, field: String, byteCount: Int) throws -> Data {
    let decoded = try decodeSccpBundleBase58(value, field: field)
    guard decoded.count == byteCount else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    return decoded
}

private func decodeSccpBundleBase58(_ value: String, field: String) throws -> Data {
    guard !value.isEmpty, value.trimmingCharacters(in: .whitespacesAndNewlines) == value else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    var bytes = [UInt8]()
    for character in value {
        guard let alphabetIndex = sccpBundleBase58Alphabet.firstIndex(of: character) else {
            throw SccpMessageProofBundleError.invalid(field)
        }
        var carry = alphabetIndex
        if !bytes.isEmpty {
            for index in stride(from: bytes.count - 1, through: 0, by: -1) {
                let next = Int(bytes[index]) * 58 + carry
                bytes[index] = UInt8(next & 0xff)
                carry = next >> 8
            }
        }
        while carry > 0 {
            bytes.insert(UInt8(carry & 0xff), at: 0)
            carry >>= 8
        }
    }
    let leadingZeros = value.prefix { $0 == "1" }.count
    var decoded = Data(repeating: 0, count: leadingZeros)
    decoded.append(Data(bytes))
    return decoded
}

private func sccpBundleTronBase58CheckPayload(_ value: String, field: String) throws -> Data {
    let raw = try decodeSccpBundleBase58(value, field: field)
    guard raw.count == 25, raw.first == 0x41 else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    let payload = Data(raw.prefix(21))
    let checksum = Data(raw.suffix(4))
    let expected = Data(Data(SHA256.hash(data: Data(SHA256.hash(data: payload)))).prefix(4))
    guard checksum == expected else {
        throw SccpMessageProofBundleError.invalid(field)
    }
    return payload
}

private func validateSccpBundleTonRawAddress(_ value: String, field: String) throws {
    let parts = value.split(separator: ":", omittingEmptySubsequences: false)
    guard value.trimmingCharacters(in: .whitespacesAndNewlines) == value,
          parts.count == 2,
          parts[0] == "0",
          parts[1].count == 64,
          parts[1].allSatisfy({ $0.isHexDigit }),
          let account = Data(hexString: String(parts[1])),
          account.count == 32,
          account.contains(where: { $0 != 0 }) else {
        throw SccpMessageProofBundleError.invalid(field)
    }
}

private func sccpBundlePrefixedKeccakHex(prefix: String, payload: Data) -> String {
    var preimage = Data(prefix.utf8)
    preimage.append(payload)
    return "0x" + irohaKeccak256(preimage).hexEncodedString()
}

private func sccpBundleHashHex(prefix: String, payload: Data) -> String {
    "0x" + sccpBundleHashBytes(prefix: prefix, payload: payload).hexEncodedString()
}

private func sccpBundleHashBytes(prefix: String, payload: Data) -> Data {
    var preimage = Data(prefix.utf8)
    preimage.append(payload)
    return Blake2b.hash256(preimage)
}
