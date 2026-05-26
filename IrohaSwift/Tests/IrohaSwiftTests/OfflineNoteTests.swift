import XCTest
@testable import IrohaSwift

final class OfflineNoteTests: XCTestCase {
    func testCertificateSigningBytesMatchRustVector() throws {
        let fixture = try Self.loadFixture()
        let sender = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let verifier = try Self.certificateVerifier(fixture)

        XCTAssertEqual(
            try sender.signingBytes().base64EncodedString(),
            fixture.chainVectors.certificates.senderPayloadBase64
        )
        XCTAssertEqual(
            try sender.payloadHash().hexLowercased(),
            fixture.chainVectors.certificates.senderPayloadHash
        )
        XCTAssertTrue(try verifier.verifyCertificate(sender))

        var tamperedSignature = sender.issuerSignature
        tamperedSignature[tamperedSignature.startIndex] ^= 0x01
        let tampered = try OfflineNoteKeyCertificate(
            version: sender.version,
            platform: sender.platform,
            keyId: sender.keyId,
            deviceId: sender.deviceId,
            accountId: sender.accountId,
            publicKey: sender.publicKey,
            assertionScheme: sender.assertionScheme,
            assertionKeyAlgorithm: sender.assertionKeyAlgorithm,
            assertionPublicKey: sender.assertionPublicKey,
            assertionUsageCountLimit: sender.assertionUsageCountLimit,
            oneUse: sender.oneUse,
            issuerSignature: tamperedSignature
        )
        XCTAssertFalse(try verifier.verifyCertificate(tampered))
        XCTAssertFalse(try RejectingOfflineNoteCertificateVerifier().verifyCertificate(sender))
        XCTAssertFalse(try Ed25519OfflineNoteCertificateVerifier(
            trustedIssuerPublicKeys: [Data(repeating: 0x42, count: 32)]
        ).verifyCertificate(sender))
    }

    func testOfflineNoteModelsMatchRustNoritoVectors() throws {
        let fixture = try Self.loadFixture()

        XCTAssertEqual(
            try Self.issue(fixture).noritoEncoded().base64EncodedString(),
            fixture.chainVectors.issue.noritoBase64
        )
        XCTAssertEqual(
            try Self.audit(fixture).noritoEncoded().base64EncodedString(),
            fixture.chainVectors.audit.noritoBase64
        )
        XCTAssertEqual(
            try Self.redeem(fixture).noritoEncoded().base64EncodedString(),
            fixture.chainVectors.redeem.noritoBase64
        )
    }

    func testOfflineNotePublicNoritoDecodersRoundTripFixturePayloads() throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let senderPayloadBytes = try Self.base64(fixture.chainVectors.certificates.senderPayloadBase64)
        let issueBytes = try Self.base64(fixture.chainVectors.issue.noritoBase64)
        let auditBytes = try Self.base64(fixture.chainVectors.audit.noritoBase64)
        let redeemBytes = try Self.base64(fixture.chainVectors.redeem.noritoBase64)

        XCTAssertEqual(
            try OfflineNoteDecoding.decodeKeyCertificatePayload(senderPayloadBytes)
                .noritoEncoded()
                .base64EncodedString(),
            senderPayloadBytes.base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteDecoding.decodeKeyCertificate(try senderCertificate.noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try senderCertificate.noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteDecoding.decodeIssue(issueBytes).noritoEncoded().base64EncodedString(),
            issueBytes.base64EncodedString()
        )

        let decodedAudit = try OfflineNoteDecoding.decodeAudit(auditBytes)
        XCTAssertEqual(try decodedAudit.noritoEncoded().base64EncodedString(), auditBytes.base64EncodedString())
        XCTAssertEqual(
            try OfflineNoteDecoding.decodeIssuedClaim(decodedAudit.inputClaims[0].noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try decodedAudit.inputClaims[0].noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteDecoding.decodeAuditPublicInputs(decodedAudit.publicInputs().noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try decodedAudit.publicInputs().noritoEncoded().base64EncodedString()
        )

        let decodedRedeem = try OfflineNoteDecoding.decodeRedeem(redeemBytes)
        XCTAssertEqual(try decodedRedeem.noritoEncoded().base64EncodedString(), redeemBytes.base64EncodedString())
        XCTAssertEqual(
            try OfflineNoteDecoding.decodeRedeemPublicInputs(decodedRedeem.publicInputs().noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try decodedRedeem.publicInputs().noritoEncoded().base64EncodedString()
        )

        let commitmentPreimage = try OfflineNoteCommitmentPreimage(
            chainId: derivation.chainId,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.redeem.amount,
            noteSecret: Self.hex(derivation.sourceNoteSecretHex),
            origin: .issuerLoad(OfflineNoteIssuerLoadOrigin(
                operationId: derivation.issuerLoadOperationId,
                lineageId: derivation.issuerLoadLineageId,
                localRevision: derivation.issuerLoadLocalRevision
            ))
        )
        XCTAssertEqual(
            try OfflineNoteDecoding.decodeNoteCommitmentPreimage(commitmentPreimage.noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try commitmentPreimage.noritoEncoded().base64EncodedString()
        )

        let nullifierPreimage = try OfflineNoteInputNullifierPreimage(
            chainId: derivation.chainId,
            sourceNoteCommitment: Self.hex(derivation.sourceNoteCommitment),
            ownerKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            noteSecret: Self.hex(derivation.sourceNoteSecretHex)
        )
        XCTAssertEqual(
            try OfflineNoteDecoding.decodeInputNullifierPreimage(nullifierPreimage.noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try nullifierPreimage.noritoEncoded().base64EncodedString()
        )

        let tokenPreimage = try OfflineNotePaymentTokenIdPreimage(
            chainId: derivation.chainId,
            paymentRequestId: derivation.paymentRequestId,
            createdAtMs: fixture.paymentToken.createdAtMs,
            tokenNonce: Self.hex(derivation.tokenNonceHex),
            senderKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            inputNullifiers: [Self.hex(derivation.inputNullifier)],
            outputCommitments: [
                Self.hex(derivation.recipientOutputCommitment),
                Self.hex(derivation.changeOutputCommitment)
            ]
        )
        XCTAssertEqual(
            try OfflineNoteDecoding.decodePaymentTokenIdPreimage(tokenPreimage.noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try tokenPreimage.noritoEncoded().base64EncodedString()
        )
    }

    func testOfflineNotePublicNoritoInstructionDecodersReadExplorerEnvelopeBytes() throws {
        let fixture = try Self.loadFixture()
        let issue = try Self.issue(fixture)
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)

        let issueEnvelope = Self.rawInstructionPair(
            wireName: OfflineNoteTypeNames.issueInstruction,
            wirePayload: try Self.instructionWirePayload(
                typeName: OfflineNoteTypeNames.issueInstruction,
                modelPayload: OfflineNoteEncoding.encodeIssue(issue)
            )
        )
        XCTAssertEqual(
            try OfflineNoteDecoding.decodeIssueInstruction(issueEnvelope).noritoEncoded().base64EncodedString(),
            try issue.noritoEncoded().base64EncodedString()
        )

        let auditEnvelope = Self.rawInstructionPair(
            wireName: OfflineNoteTypeNames.auditInstruction,
            wirePayload: try Self.instructionWirePayload(
                typeName: OfflineNoteTypeNames.auditInstruction,
                modelPayload: OfflineNoteEncoding.encodeAudit(audit)
            )
        )
        XCTAssertEqual(
            try OfflineNoteDecoding.decodeAuditInstruction(auditEnvelope).noritoEncoded().base64EncodedString(),
            try audit.noritoEncoded().base64EncodedString()
        )

        let redeemEnvelope = Self.rawInstructionPair(
            wireName: OfflineNoteTypeNames.redeemInstruction,
            wirePayload: try Self.instructionWirePayload(
                typeName: OfflineNoteTypeNames.redeemInstruction,
                modelPayload: OfflineNoteEncoding.encodeRedeem(redeem)
            )
        )
        XCTAssertEqual(
            try OfflineNoteDecoding.decodeRedeemInstruction(redeemEnvelope).noritoEncoded().base64EncodedString(),
            try redeem.noritoEncoded().base64EncodedString()
        )
    }

    func testOfflineNotePaymentTokenCodecRoundTripsNoritoTextAndQrFrames() throws {
        let fixture = try Self.loadFixture()
        let token = OfflineNotePaymentToken(
            chainId: fixture.chainVectors.derivation.chainId,
            paymentRequestId: fixture.paymentToken.invoiceId,
            tokenNonce: try Self.hex(fixture.chainVectors.derivation.tokenNonceHex),
            tokenId: try Self.hex(fixture.paymentToken.tokenId),
            audit: try Self.audit(fixture),
            createdAtMs: fixture.paymentToken.createdAtMs
        )
        let canonicalPayload = try Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64)
        XCTAssertEqual(try OfflineNotePaymentTokenCodec.encodeNorito(token), canonicalPayload)

        let noritoDecoded = try OfflineNotePaymentTokenCodec.decodeNorito(
            OfflineNotePaymentTokenCodec.encodeNorito(token)
        )
        XCTAssertEqual(noritoDecoded.tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(noritoDecoded.paymentRequestId, token.paymentRequestId)
        XCTAssertEqual(try noritoDecoded.audit.noritoEncoded(), try token.audit.noritoEncoded())
        XCTAssertEqual(noritoDecoded.bearerAuditTrail.map(\.tokenId), [token.tokenId])
        let canonicalDecoded = try OfflineNotePaymentTokenCodec.decodeNorito(canonicalPayload)
        XCTAssertEqual(canonicalDecoded.tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(try canonicalDecoded.audit.noritoEncoded(), try token.audit.noritoEncoded())
        XCTAssertEqual(canonicalDecoded.bearerAuditTrail.map(\.tokenId), [token.tokenId])

        let text = try OfflineNotePaymentTokenCodec.encodeText(token)
        XCTAssertEqual(text, fixture.sdkInterop.paymentTokenText)
        XCTAssertTrue(text.hasPrefix(OfflineNotePaymentTokenCodec.textPrefix))
        XCTAssertEqual(OfflineNotePaymentTokenCodec.textPrefix, OfflineNoteTransferTextPayloadCodec.paymentTokenPrefix)
        XCTAssertEqual(try OfflineNotePaymentTokenCodec.decodeText(text).tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(
            try OfflineNotePaymentTokenCodec.decodeText(
                OfflineNoteTransferTextPayloadCodec.paymentTokenPrefix + String(text.split(separator: ":").last!)
            ).tokenIdHex,
            token.tokenIdHex
        )

        let frames = try OfflineNotePaymentTokenCodec.encodeQrFrameBytes(
            token,
            options: OfflineQrStreamOptions(chunkSize: 180, parityGroup: 2)
        )
        XCTAssertEqual(
            frames.map { $0.hexLowercased() },
            fixture.sdkInterop.paymentTokenQr.frames.map(\.bytesHex)
        )
        let decoder = OfflineQrStreamDecoder()
        var payload: Data?
        for frame in frames {
            let result = try decoder.ingest(frameBytes: frame)
            payload = result.payload ?? payload
        }
        let qrDecoded = try OfflineNotePaymentTokenCodec.decodeQrPayload(XCTUnwrap(payload))
        XCTAssertEqual(qrDecoded.tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(try qrDecoded.audit.noritoEncoded(), try token.audit.noritoEncoded())

        let canonicalDecoder = OfflineQrStreamDecoder()
        var canonicalQrPayload: Data?
        for frame in fixture.sdkInterop.paymentTokenQr.frames {
            let result = try canonicalDecoder.ingest(frameBytes: Self.hex(frame.bytesHex))
            canonicalQrPayload = result.payload ?? canonicalQrPayload
        }
        XCTAssertEqual(try XCTUnwrap(canonicalQrPayload), canonicalPayload)
        XCTAssertEqual(
            try OfflineNotePaymentTokenCodec.decodeQrPayload(try XCTUnwrap(canonicalQrPayload)).tokenIdHex,
            token.tokenIdHex
        )
    }

    func testOfflineNoteReceiveRequestCodecRoundTripsNoritoTextAndQrFrames() throws {
        let fixture = try Self.loadFixture()
        let output = fixture.paymentToken.outputClaims[0]
        let request = try OfflineNoteReceiveRequest(
            chainId: fixture.chainVectors.derivation.chainId,
            paymentRequestId: fixture.paymentToken.invoiceId,
            accountId: output.accountId,
            assetDefinitionId: output.assetDefinitionId,
            assetId: "\(output.assetDefinitionId)#\(output.accountId)",
            amount: output.amount,
            keyCertificate: Self.certificate(output.keyCertificate),
            outputCommitment: Self.hex(output.noteCommitment)
        )

        let noritoDecoded = try OfflineNoteReceiveRequestCodec.decodeNorito(
            OfflineNoteReceiveRequestCodec.encodeNorito(request)
        )
        XCTAssertEqual(noritoDecoded.paymentRequestId, request.paymentRequestId)
        XCTAssertEqual(noritoDecoded.accountId, request.accountId)
        XCTAssertEqual(noritoDecoded.assetId, request.assetId)
        XCTAssertEqual(noritoDecoded.amount, request.amount)
        XCTAssertEqual(noritoDecoded.outputCommitmentHex, request.outputCommitmentHex)
        XCTAssertEqual(
            try noritoDecoded.keyCertificate.payloadHash(),
            try request.keyCertificate.payloadHash()
        )

        let text = try OfflineNoteReceiveRequestCodec.encodeText(request)
        XCTAssertTrue(text.hasPrefix(OfflineNoteReceiveRequestCodec.textPrefix))
        XCTAssertEqual(
            try OfflineNoteReceiveRequestCodec.decodeText(text).outputCommitmentHex,
            request.outputCommitmentHex
        )
        XCTAssertEqual(
            try OfflineNoteTransferTextPayloadCodec.decodeReceiveRequest(text).outputCommitmentHex,
            request.outputCommitmentHex
        )

        let frames = try OfflineNoteReceiveRequestCodec.encodeQrFrameBytes(
            request,
            options: OfflineQrStreamOptions(chunkSize: 180, parityGroup: 2)
        )
        let decoder = OfflineQrStreamDecoder()
        var payload: Data?
        for frame in frames {
            let result = try decoder.ingest(frameBytes: frame)
            payload = result.payload ?? payload
        }
        let qrDecoded = try OfflineNoteReceiveRequestCodec.decodeQrPayload(XCTUnwrap(payload))
        XCTAssertEqual(qrDecoded.outputCommitmentHex, request.outputCommitmentHex)
    }

    func testAssetDefinitionAddressRoundTripsFixtureAndRejectsBadChecksum() throws {
        let fixture = try Self.loadFixture()
        let definition = Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId)
        let uuidBytes = try XCTUnwrap(AssetDefinitionAddress.decode(definition))

        XCTAssertEqual(AssetDefinitionAddress.encode(uuidBytes: uuidBytes), definition)

        var tampered = definition
        let replacement: Character = tampered.last == "1" ? "2" : "1"
        tampered.removeLast()
        tampered.append(replacement)
        XCTAssertNil(AssetDefinitionAddress.decode(tampered))
    }

    func testOfflineNoteTransferHandoffSupportsQrNfcAndNearbyPayloads() throws {
        let fixture = try Self.loadFixture()
        let token = OfflineNotePaymentToken(
            chainId: fixture.chainVectors.derivation.chainId,
            paymentRequestId: fixture.paymentToken.invoiceId,
            tokenNonce: try Self.hex(fixture.chainVectors.derivation.tokenNonceHex),
            tokenId: try Self.hex(fixture.paymentToken.tokenId),
            audit: try Self.audit(fixture),
            createdAtMs: fixture.paymentToken.createdAtMs
        )
        let canonicalPayload = try Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64)

        let capabilities = OfflineNoteTransferCapabilities.current()
        XCTAssertTrue(capabilities.supportedModalities.contains(.qrStreaming))
        XCTAssertTrue(capabilities.supportedModalities.contains(.nearby))
        XCTAssertFalse(capabilities.supportedModalities.contains(.nfc))

        let nearby = try OfflineNoteTransferHandoff.nearbyPayload(for: token)
        XCTAssertEqual(nearby.modality, .nearby)
        XCTAssertEqual(nearby.contentType, OfflineNoteTransferHandoff.paymentTokenContentType)
        XCTAssertEqual(nearby.payload, canonicalPayload)
        XCTAssertEqual(
            try OfflineNoteTransferHandoff.decodePaymentToken(from: nearby).tokenIdHex,
            token.tokenIdHex
        )

        let qrFrames = try OfflineNoteTransferHandoff.qrStreamingFrameBytes(for: token)
        XCTAssertEqual(
            qrFrames.map { $0.hexLowercased() },
            fixture.sdkInterop.paymentTokenQr.frames.map(\.bytesHex)
        )
        let qrReceiver = OfflineNoteTransferStreamReceiver()
        var qrResult: OfflineNoteTransferStreamResult?
        for frame in qrFrames {
            qrResult = try qrReceiver.ingestFrame(frame)
        }
        XCTAssertEqual(try XCTUnwrap(qrResult?.token).tokenIdHex, token.tokenIdHex)

        let nfcFrames = try OfflineNoteTransferHandoff.nfcFrameBytes(for: token)
        XCTAssertTrue(nfcFrames.allSatisfy { $0.count <= 250 })
        let nfcReceiver = OfflineNoteTransferStreamReceiver()
        var nfcResult: OfflineNoteTransferStreamResult?
        for frame in nfcFrames {
            nfcResult = try nfcReceiver.ingestFrame(frame)
        }
        XCTAssertEqual(try XCTUnwrap(nfcResult?.token).tokenIdHex, token.tokenIdHex)
    }

    func testOfflineNoteTransferHandoffRejectsAdversarialStreamsAndMetadata() throws {
        let fixture = try Self.loadFixture()
        let token = try OfflineNotePaymentTokenCodec.decodeNorito(
            Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64)
        )
        let rawPayload = try OfflineNoteTransferHandoff.rawPaymentTokenBytes(for: token)
        let payload = try OfflineNoteTransferHandoff.paymentTokenPayload(for: token, modality: .qrStreaming)
        let wrongContentType = OfflineNoteTransferPayload(
            modality: .nearby,
            contentType: OfflineNoteTransferHandoff.receiptAckContentType,
            payload: payload.payload
        )
        XCTAssertThrowsError(try OfflineNoteTransferHandoff.decodePaymentToken(from: wrongContentType))

        let frames = try OfflineNoteTransferHandoff.qrStreamingFrameBytes(
            for: token,
            options: OfflineQrStreamOptions(chunkSize: 128, parityGroup: 0)
        )
        XCTAssertGreaterThan(frames.count, 2)

        var badMagic = frames[0]
        badMagic[badMagic.startIndex] = 0x00
        XCTAssertThrowsError(try OfflineNoteTransferStreamReceiver().ingestFrame(badMagic))

        var badVersion = frames[0]
        badVersion[badVersion.startIndex + 2] = 0x7f
        XCTAssertThrowsError(try OfflineNoteTransferStreamReceiver().ingestFrame(badVersion))

        var badChecksum = frames[1]
        badChecksum[badChecksum.index(before: badChecksum.endIndex)] ^= 0x01
        XCTAssertThrowsError(try OfflineNoteTransferStreamReceiver().ingestFrame(badChecksum))

        XCTAssertThrowsError(try OfflineNoteTransferStreamReceiver().ingestFrame(Data(frames[0].prefix(8))))

        let header = try OfflineQrStreamFrame.decode(frames[0])
        var mismatchedHeaderStreamId = header.streamId
        mismatchedHeaderStreamId[mismatchedHeaderStreamId.startIndex] ^= 0x01
        let mismatchedHeader = try OfflineQrStreamFrame(
            kind: .header,
            streamId: mismatchedHeaderStreamId,
            index: header.index,
            total: header.total,
            payload: header.payload
        ).encode()
        XCTAssertThrowsError(try OfflineNoteTransferStreamReceiver().ingestFrame(mismatchedHeader))

        let firstData = try OfflineQrStreamFrame.decode(frames[1])
        var wrongStreamId = firstData.streamId
        wrongStreamId[wrongStreamId.startIndex] ^= 0x7f
        let wrongStreamFrame = try OfflineQrStreamFrame(
            kind: .data,
            streamId: wrongStreamId,
            index: firstData.index,
            total: firstData.total,
            payload: firstData.payload
        ).encode()
        let ignoreWrongStreamReceiver = OfflineNoteTransferStreamReceiver()
        XCTAssertNil(try ignoreWrongStreamReceiver.ingestFrame(frames[0]).token)
        XCTAssertNil(try ignoreWrongStreamReceiver.ingestFrame(wrongStreamFrame).token)
        var completed: OfflineNoteTransferStreamResult?
        for frame in frames.dropFirst() {
            completed = try ignoreWrongStreamReceiver.ingestFrame(frame)
        }
        XCTAssertEqual(try XCTUnwrap(completed?.token).tokenIdHex, token.tokenIdHex)

        var poisonedPayload = firstData.payload
        poisonedPayload[poisonedPayload.startIndex] ^= 0x01
        let poisonedFrame = try OfflineQrStreamFrame(
            kind: .data,
            streamId: firstData.streamId,
            index: firstData.index,
            total: firstData.total,
            payload: poisonedPayload
        ).encode()
        let poisonedReceiver = OfflineNoteTransferStreamReceiver()
        _ = try poisonedReceiver.ingestFrame(frames[0])
        _ = try poisonedReceiver.ingestFrame(poisonedFrame)
        XCTAssertThrowsError(try frames.dropFirst(2).forEach { _ = try poisonedReceiver.ingestFrame($0) })

        let wrongKindFrames = try OfflineQrStreamEncoder.encodeFrameBytes(
            payload: rawPayload,
            payloadKind: .offlineReceiptAck,
            options: OfflineQrStreamOptions(chunkSize: 512, parityGroup: 0)
        )
        let wrongKindReceiver = OfflineNoteTransferStreamReceiver()
        XCTAssertThrowsError(try wrongKindFrames.forEach { _ = try wrongKindReceiver.ingestFrame($0) })
    }

    func testOfflineNoteNfcApduProtocolSupportsAndroidSafeAndIOSFastChunks() throws {
        let fixture = try Self.loadFixture()
        let token = try OfflineNotePaymentTokenCodec.decodeNorito(Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64))
        let payload = try OfflineNoteTransferHandoff.rawPaymentTokenBytes(for: token)

        XCTAssertEqual(OfflineNoteTransferHandoff.defaultNfcAidHex, OfflineNoteNfcApduProtocol.aidHex)
        XCTAssertEqual(OfflineNoteNfcApduProtocol.parseCommand(OfflineNoteNfcApduProtocol.selectAidAPDUData()), .select)
        let customAid = Data([0xF0, 0x50, 0x4B, 0x45, 0x50, 0x4B, 0x52, 0x4E, 0x46, 0x43, 0x01])
        XCTAssertEqual(OfflineNoteNfcApduProtocol.aidHex(for: customAid), "F0504B45504B524E464301")
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(
                OfflineNoteNfcApduProtocol.selectAidAPDUData(aid: customAid),
                aid: customAid
            ),
            .select
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(
                OfflineNoteNfcApduProtocol.selectAidAPDUData(aid: customAid)
            ),
            .unsupported
        )
        XCTAssertEqual(OfflineNoteNfcApduProtocol.parseCommand(OfflineNoteNfcApduProtocol.getInfoAPDUData()), .getInfo)

        let infoBytes = try OfflineNoteNfcApduProtocol.encodeInfo(kind: .paymentToken, payloadBytes: payload)
        let info = try XCTUnwrap(OfflineNoteNfcApduProtocol.decodeInfo(infoBytes))
        XCTAssertEqual(info.kind, .paymentToken)
        XCTAssertEqual(info.payloadLength, payload.count)
        XCTAssertEqual(info.maxChunkLength, OfflineNoteNfcApduProtocol.androidSafeChunkBytes)
        XCTAssertTrue(OfflineNoteNfcApduProtocol.payloadDigestMatches(payload, expectedSha256: info.sha256))

        let androidApdus = try OfflineNoteTransferHandoff.nfcPaymentTokenWriteAPDUs(for: token)
        XCTAssertEqual(OfflineNoteNfcApduProtocol.parseCommand(androidApdus.first), .writeMeta(kind: .paymentToken, payloadLength: payload.count, sha256: info.sha256))
        for apdu in androidApdus.dropFirst().dropLast() {
            guard case let .writeChunk(_, bytes) = OfflineNoteNfcApduProtocol.parseCommand(apdu) else {
                return XCTFail("Expected write chunk APDU")
            }
            XCTAssertLessThanOrEqual(bytes.count, OfflineNoteNfcApduProtocol.androidSafeChunkBytes)
        }
        XCTAssertEqual(OfflineNoteNfcApduProtocol.parseCommand(androidApdus.last), .commit)

        let fastPayload = Data(repeating: 0x5A, count: 512)
        let fastApdu = try OfflineNoteNfcApduProtocol.writeChunkAPDUData(offset: 1_024, bytes: fastPayload)
        XCTAssertEqual(
            Data(fastApdu.prefix(7)),
            Data([0x80, 0x21, 0x04, 0x00, 0x00, 0x02, 0x00])
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(fastApdu),
            .writeChunk(offset: 1_024, bytes: fastPayload)
        )
        let fastRead = try OfflineNoteNfcApduProtocol.readChunkAPDUData(
            offset: 256,
            length: OfflineNoteNfcApduProtocol.maxExtendedReadChunkBytes
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(fastRead),
            .readChunk(offset: 256, requestedLength: OfflineNoteNfcApduProtocol.maxExtendedReadChunkBytes)
        )
    }

    func testOfflineNoteTransportWireFormatMatchesSharedFixture() throws {
        let fixture = try Self.loadFixture()
        let token = try OfflineNotePaymentTokenCodec.decodeNorito(Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64))
        let payload = try OfflineNoteTransferHandoff.rawPaymentTokenBytes(for: token)
        let writeApdus = try OfflineNoteTransferHandoff.nfcPaymentTokenWriteAPDUs(for: token)
        let readApdus = try OfflineNoteNfcApduProtocol.readPayloadAPDUs(payloadLength: payload.count)
        let nearbyBytes = try OfflineNoteTransferHandoff.nearbyPaymentEnvelopeBytes(for: token)

        XCTAssertEqual(payload.count, 4_665)
        XCTAssertEqual(OfflineNoteNfcApduProtocol.selectAidAPDUData().hexLowercased(), "00a4040007f049524f48413200")
        XCTAssertEqual(OfflineNoteNfcApduProtocol.getInfoAPDUData().hexLowercased(), "8010000000")
        XCTAssertEqual(
            try OfflineNoteNfcApduProtocol.encodeInfo(kind: .paymentToken, payloadBytes: payload).hexLowercased(),
            "020000123900f074daabfb799585d3bd938827eaf913df0d590a5b4b4da1968e3d4cdec2587abd"
        )
        XCTAssertEqual(
            try OfflineNoteNfcApduProtocol.writeMetaAPDUData(kind: .paymentToken, payloadBytes: payload).hexLowercased(),
            "8020000025020000123974daabfb799585d3bd938827eaf913df0d590a5b4b4da1968e3d4cdec2587abd"
        )
        XCTAssertEqual(writeApdus.count, 22)
        XCTAssertEqual(writeApdus[0].hexLowercased(), "8020000025020000123974daabfb799585d3bd938827eaf913df0d590a5b4b4da1968e3d4cdec2587abd")
        XCTAssertEqual(OfflineNoteNfcApduProtocol.sha256(writeApdus[1]).hexLowercased(), "67ce747103b4acbfa13aabbc6424f80b884fd303ab79c72f4b09d374268a116d")
        XCTAssertEqual(writeApdus[writeApdus.count - 2].hexLowercased(), "802111d0696117166f66666c696e652d6e6f74652d72656375727369766520699b945eaef37b763f70ce18b173caed4fe4fec9bb8110fc5231feb9f868d7a52e0a0968616c6f322f697061221a000000000000006f66666c696e652d766563746f722d61756469742d70726f6f66")
        XCTAssertEqual(writeApdus.last?.hexLowercased(), "8022000000")
        XCTAssertEqual(readApdus.count, 20)
        XCTAssertEqual(readApdus.first?.hexLowercased(), "80110000f0")
        XCTAssertEqual(nearbyBytes.count, 6_318)
        XCTAssertEqual(OfflineNoteNfcApduProtocol.sha256(nearbyBytes).hexLowercased(), "586c5562935e68b942a5ba5c1e9935cc799a49b88320942751fa05ded0c43d40")
    }

    func testOfflineNoteNfcApduProtocolRejectsAdversarialPayloadsBeforeCommit() throws {
        let payload = Data("offline-payment".utf8)
        let info = try XCTUnwrap(
            OfflineNoteNfcApduProtocol.decodeInfo(
                try OfflineNoteNfcApduProtocol.encodeInfo(kind: .receiptAck, payloadBytes: payload)
            )
        )
        let assembler = try OfflineNoteNfcPayloadAssembler(info: info)

        XCTAssertFalse(assembler.write(offset: payload.count - 2, chunk: Data(repeating: 0x01, count: 4)))
        XCTAssertTrue(assembler.write(offset: 0, chunk: Data(payload.prefix(6))))
        XCTAssertTrue(assembler.write(offset: 0, chunk: Data(payload.prefix(6))))
        XCTAssertFalse(assembler.write(offset: 0, chunk: Data("OFFLIN".utf8)))
        XCTAssertThrowsError(try assembler.commit()) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .incompletePayload)
        }
        XCTAssertTrue(assembler.write(offset: 6, chunk: Data(payload.dropFirst(6))))
        XCTAssertEqual(try assembler.commit(), payload)

        var oversizedInfo = try OfflineNoteNfcApduProtocol.encodeInfo(kind: .paymentToken, payloadBytes: payload)
        let oversized = OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes + 1
        oversizedInfo[oversizedInfo.startIndex + 1] = UInt8((oversized >> 24) & 0xff)
        oversizedInfo[oversizedInfo.startIndex + 2] = UInt8((oversized >> 16) & 0xff)
        oversizedInfo[oversizedInfo.startIndex + 3] = UInt8((oversized >> 8) & 0xff)
        oversizedInfo[oversizedInfo.startIndex + 4] = UInt8(oversized & 0xff)
        XCTAssertNil(OfflineNoteNfcApduProtocol.decodeInfo(oversizedInfo))

        let badAssembler = try OfflineNoteNfcPayloadAssembler(
            kind: .paymentToken,
            expectedLength: payload.count,
            expectedSha256: Data(repeating: 0x00, count: 32)
        )
        XCTAssertTrue(badAssembler.write(offset: 0, chunk: payload))
        XCTAssertThrowsError(try badAssembler.commit()) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .checksumMismatch)
        }
        XCTAssertThrowsError(
            try OfflineNoteNfcPayloadAssembler(
                kind: .paymentToken,
                expectedLength: OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes + 1,
                expectedSha256: Data(repeating: 0x00, count: 32)
            )
        )
    }

    func testOfflineNoteNfcApduProtocolRejectsMalformedCommandsAndBounds() throws {
        XCTAssertEqual(OfflineNoteNfcApduProtocol.parseCommand(nil), .invalid)
        XCTAssertEqual(OfflineNoteNfcApduProtocol.parseCommand(Data([0x00])), .invalid)
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x00, 0xA4, 0x04, 0x00, 0x01, 0xFF, 0x00])),
            .unsupported
        )
        var selectWithNonZeroLe = OfflineNoteNfcApduProtocol.selectAidAPDUData()
        selectWithNonZeroLe[selectWithNonZeroLe.index(before: selectWithNonZeroLe.endIndex)] = 0x01
        XCTAssertEqual(OfflineNoteNfcApduProtocol.parseCommand(selectWithNonZeroLe), .unsupported)
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x81, 0x10, 0x00, 0x00, 0x00])),
            .unsupported
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x10, 0x00, 0x01, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x10, 0x00, 0x00, 0x01])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x10, 0x00, 0x00, 0x01, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x11, 0x00, 0x00, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x20, 0x00, 0x00, 0x01, 0x01])),
            .invalid
        )
        var writeMetaWithOffset = try OfflineNoteNfcApduProtocol.writeMetaAPDUData(
            kind: .receiptAck,
            payloadBytes: Data([0x01])
        )
        writeMetaWithOffset[writeMetaWithOffset.startIndex + 3] = 0x01
        XCTAssertEqual(OfflineNoteNfcApduProtocol.parseCommand(writeMetaWithOffset), .invalid)
        let zeroLengthMeta = Data([OfflineNoteNfcPayloadKind.paymentToken.rawValue, 0x00, 0x00, 0x00, 0x00])
            + Data(repeating: 0x00, count: 32)
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x20, 0x00, 0x00, UInt8(zeroLengthMeta.count)]) + zeroLengthMeta),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x21, 0x00, 0x00, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x21, 0x00, 0x00, 0x02, 0x01])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x22, 0x00, 0x00, 0x01, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x22, 0x01, 0x00, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(Data([0x80, 0x22, 0x00, 0x00, 0x01])),
            .invalid
        )

        XCTAssertThrowsError(try OfflineNoteNfcApduProtocol.writeChunkAPDUData(offset: 0x1_0000, bytes: Data([0x01])))
        XCTAssertThrowsError(try OfflineNoteNfcApduProtocol.writeChunkAPDUData(offset: 0, bytes: Data()))
        let rangePayload = Data([0x01, 0x02])
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: -1..<1
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: 1..<3
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: 3..<4
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: Int.min..<0
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: 0..<Int.max
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: rangePayload.startIndex..<rangePayload.startIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidChunkLength)
        }
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: rangePayload.endIndex..<rangePayload.endIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidChunkLength)
        }
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: -1,
                payloadBytes: rangePayload,
                range: rangePayload.startIndex..<rangePayload.endIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: Int.min,
                payloadBytes: rangePayload,
                range: rangePayload.startIndex..<rangePayload.endIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 0x1_0000,
                payloadBytes: rangePayload,
                range: rangePayload.startIndex..<rangePayload.startIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidOffset)
        }
        let oversizedChunkPayload = Data(
            repeating: 0xA5,
            count: OfflineNoteNfcApduProtocol.maxExtendedWriteChunkBytes + 1
        )
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: oversizedChunkPayload,
                range: oversizedChunkPayload.startIndex..<oversizedChunkPayload.endIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidChunkLength)
        }
        var shiftedOversizedPayload = Data([0x00])
        shiftedOversizedPayload.append(oversizedChunkPayload)
        shiftedOversizedPayload.append(0xFF)
        let shiftedOversizedStart = shiftedOversizedPayload.index(after: shiftedOversizedPayload.startIndex)
        let shiftedOversizedEnd = shiftedOversizedPayload.index(before: shiftedOversizedPayload.endIndex)
        let shiftedOversizedRange = shiftedOversizedStart..<shiftedOversizedEnd
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: shiftedOversizedPayload,
                range: shiftedOversizedRange
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteNfcApduError, .invalidChunkLength)
        }
        let shortHeaderPayload = Data(repeating: 0x33, count: Int(UInt8.max))
        let shortHeaderApdu = try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
            offset: 0x1234,
            payloadBytes: shortHeaderPayload,
            range: shortHeaderPayload.startIndex..<shortHeaderPayload.endIndex
        )
        XCTAssertEqual(
            Data(shortHeaderApdu.prefix(5)),
            Data([0x80, 0x21, 0x12, 0x34, 0xFF])
        )
        XCTAssertEqual(shortHeaderApdu.count, 5 + Int(UInt8.max))

        var shiftedShortHeaderPayload = Data([0x00])
        shiftedShortHeaderPayload.append(shortHeaderPayload)
        shiftedShortHeaderPayload.append(0xFF)
        let shiftedShortHeaderStart = shiftedShortHeaderPayload.index(after: shiftedShortHeaderPayload.startIndex)
        let shiftedShortHeaderEnd = shiftedShortHeaderPayload.index(before: shiftedShortHeaderPayload.endIndex)
        let shiftedShortHeaderRange = shiftedShortHeaderStart..<shiftedShortHeaderEnd
        let shiftedShortHeaderApdu = try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
            offset: 0x1234,
            payloadBytes: shiftedShortHeaderPayload,
            range: shiftedShortHeaderRange
        )
        XCTAssertEqual(Data(shiftedShortHeaderApdu.prefix(5)), Data([0x80, 0x21, 0x12, 0x34, 0xFF]))
        XCTAssertEqual(shiftedShortHeaderApdu.count, 5 + Int(UInt8.max))

        let extendedBoundaryPayload = Data(repeating: 0x44, count: Int(UInt8.max) + 1)
        let extendedBoundaryApdu = try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
            offset: 0,
            payloadBytes: extendedBoundaryPayload,
            range: extendedBoundaryPayload.startIndex..<extendedBoundaryPayload.endIndex
        )
        XCTAssertEqual(
            Data(extendedBoundaryApdu.prefix(7)),
            Data([0x80, 0x21, 0x00, 0x00, 0x00, 0x01, 0x00])
        )
        XCTAssertEqual(extendedBoundaryApdu.count, 7 + Int(UInt8.max) + 1)

        let maxChunkLength = OfflineNoteNfcApduProtocol.maxExtendedWriteChunkBytes
        let maxChunkPayload = Data(repeating: 0x5C, count: maxChunkLength)
        let maxChunkApdu = try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
            offset: 0xFFFF,
            payloadBytes: maxChunkPayload,
            range: maxChunkPayload.startIndex..<maxChunkPayload.endIndex
        )
        XCTAssertEqual(
            Data(maxChunkApdu.prefix(7)),
            Data([0x80, 0x21, 0xFF, 0xFF, 0x00, 0x40, 0x00])
        )
        XCTAssertEqual(maxChunkApdu.count, 7 + maxChunkLength)
        XCTAssertEqual(maxChunkApdu.suffix(2), Data([0x5C, 0x5C]))

        var shiftedPayload = Data([0x00])
        shiftedPayload.append(maxChunkPayload)
        shiftedPayload.append(0xFF)
        let shiftedMaxChunkApdu = try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
            offset: 3,
            payloadBytes: shiftedPayload,
            range: shiftedPayload.index(after: shiftedPayload.startIndex)..<shiftedPayload.index(before: shiftedPayload.endIndex)
        )
        XCTAssertEqual(shiftedMaxChunkApdu.count, 7 + maxChunkLength)
        XCTAssertEqual(shiftedMaxChunkApdu.suffix(2), Data([0x5C, 0x5C]))
        XCTAssertEqual(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 7,
                payloadBytes: rangePayload,
                range: rangePayload.startIndex..<rangePayload.endIndex
            ),
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(offset: 7, bytes: rangePayload)
        )
        XCTAssertEqual(
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: rangePayload.index(before: rangePayload.endIndex)..<rangePayload.endIndex
            ),
            try OfflineNoteNfcApduProtocol.writeChunkAPDUData(offset: 0, bytes: Data([0x02]))
        )
        XCTAssertThrowsError(try OfflineNoteNfcApduProtocol.readChunkAPDUData(offset: 0, length: 0))
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.readChunkAPDUData(
                offset: 0,
                length: OfflineNoteNfcApduProtocol.maxExtendedReadChunkBytes + 1
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.writePayloadAPDUs(
                kind: .paymentToken,
                payloadBytes: Data([0x01]),
                maxChunkLength: 0
            )
        )
        XCTAssertThrowsError(try OfflineNoteNfcApduProtocol.readPayloadAPDUs(payloadLength: 0))
        XCTAssertThrowsError(
            try OfflineNoteNfcApduProtocol.readPayloadAPDUs(
                payloadLength: 1,
                maxChunkLength: OfflineNoteNfcApduProtocol.maxExtendedReadChunkBytes + 1
            )
        )

        let response = OfflineNoteNfcApduProtocol.response(Data([0xAA, 0xBB]))
        XCTAssertEqual(response, Data([0xAA, 0xBB, 0x90, 0x00]))
        XCTAssertEqual(OfflineNoteNfcApduProtocol.responseStatus(response), 0x9000)
        XCTAssertEqual(OfflineNoteNfcApduProtocol.responseStatus(Data([0x90])), nil)
        XCTAssertEqual(OfflineNoteNfcApduProtocol.responseData(response), Data([0xAA, 0xBB]))
        XCTAssertEqual(OfflineNoteNfcApduProtocol.responseData(Data([0x90])), Data())

        let assembler = try OfflineNoteNfcPayloadAssembler(
            kind: .receiptAck,
            expectedLength: 4,
            expectedSha256: OfflineNoteNfcApduProtocol.sha256(Data([0x01, 0x02, 0x03, 0x04]))
        )
        XCTAssertFalse(assembler.write(offset: Int.max, chunk: Data([0x01])))
        XCTAssertFalse(assembler.write(offset: 4, chunk: Data([0x01])))
        XCTAssertFalse(assembler.write(offset: -1, chunk: Data([0x01])))
        XCTAssertFalse(assembler.write(offset: 0, chunk: Data()))
        XCTAssertTrue(assembler.write(offset: 0, chunk: Data([0x01, 0x02])))
        XCTAssertTrue(assembler.write(offset: 1, chunk: Data([0x02, 0x03])))
        XCTAssertFalse(assembler.write(offset: 1, chunk: Data([0x09, 0x09])))
    }

    func testOfflineNoteNfcApduRangeWriteCopiesOnlyRequestedWindow() throws {
        let payload = Data([0xA0, 0x10, 0x20, 0x30, 0x40, 0xB0])
        let cases: [(offset: Int, range: Range<Data.Index>, expected: Data)] = [
            (
                0,
                payload.index(payload.startIndex, offsetBy: 1)..<payload.index(payload.startIndex, offsetBy: 3),
                Data([0x10, 0x20])
            ),
            (
                7,
                payload.index(payload.startIndex, offsetBy: 2)..<payload.index(payload.startIndex, offsetBy: 5),
                Data([0x20, 0x30, 0x40])
            ),
            (
                0xFFFE,
                payload.index(payload.startIndex, offsetBy: 4)..<payload.index(payload.startIndex, offsetBy: 5),
                Data([0x40])
            ),
        ]

        for testCase in cases {
            let apdu = try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                offset: testCase.offset,
                payloadBytes: payload,
                range: testCase.range
            )

            guard case let .writeChunk(offset, bytes) = OfflineNoteNfcApduProtocol.parseCommand(apdu) else {
                XCTFail("expected write chunk command")
                continue
            }
            XCTAssertEqual(offset, testCase.offset)
            XCTAssertEqual(bytes, testCase.expected)
            XCTAssertFalse(bytes.contains(0xA0))
            XCTAssertFalse(bytes.contains(0xB0))
        }
    }

    func testOfflineNoteNfcApduRangeWriteCopiesExtendedWindowWithoutSentinels() throws {
        var payload = Data([0xA0])
        payload.append(Data(repeating: 0x7B, count: Int(UInt8.max) + 1))
        payload.append(0xB0)
        let range = payload.index(after: payload.startIndex)..<payload.index(before: payload.endIndex)

        let apdu = try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
            offset: 0x100,
            payloadBytes: payload,
            range: range
        )

        XCTAssertEqual(
            Data(apdu.prefix(7)),
            Data([0x80, 0x21, 0x01, 0x00, 0x00, 0x01, 0x00])
        )
        XCTAssertEqual(
            OfflineNoteNfcApduProtocol.parseCommand(apdu),
            .writeChunk(offset: 0x100, bytes: Data(repeating: 0x7B, count: Int(UInt8.max) + 1))
        )
    }

    func testOfflineNoteNearbyEnvelopeRoundTripsPairingPaymentAndAck() throws {
        let fixture = try Self.loadFixture()
        let token = try OfflineNotePaymentTokenCodec.decodeNorito(Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64))
        let receiveOutput = try XCTUnwrap(token.audit.outputClaims.first)
        let assetDefinitionId = try XCTUnwrap(
            receiveOutput.assetId.split(separator: "#", maxSplits: 1).first
        ).description
        let receiveRequest = try OfflineNoteReceiveRequest(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            accountId: receiveOutput.keyCertificate.accountId,
            assetDefinitionId: assetDefinitionId,
            assetId: receiveOutput.assetId,
            amount: receiveOutput.amount,
            keyCertificate: receiveOutput.keyCertificate,
            outputCommitment: receiveOutput.noteCommitment
        )
        let receiptAck = try OfflineNoteReceiptAck.fromPaymentToken(
            token,
            recipientAccountId: receiveOutput.keyCertificate.accountId,
            acceptedAtMs: 1_706_000_000_333
        )
        let challenge = try OfflineNoteNearbyPairingChallenge(assetName: " nearby_pairing_bird ")
        let challengeEnvelope = try OfflineNoteNearbyEnvelope(
            kind: .receiveRequest,
            payload: OfflineNoteReceiveRequestCodec.encodeNorito(receiveRequest),
            contentType: OfflineNoteTransferHandoff.receiveRequestContentType,
            pairingChallenge: challenge
        )
        let paymentBytes = try OfflineNoteTransferHandoff.nearbyPaymentEnvelopeBytes(for: token)
        let paymentEnvelope = try OfflineNoteNearbyEnvelope.decode(paymentBytes)
        let ackEnvelope = try OfflineNoteNearbyEnvelope(
            kind: .receiptAck,
            payload: OfflineNoteTransferHandoff.rawReceiptAckBytes(for: receiptAck),
            contentType: OfflineNoteTransferHandoff.receiptAckContentType
        )
        let textChallenge = try OfflineNoteReceiveRequestCodec.encodeText(receiveRequest)
        let textPayment = try OfflineNotePaymentTokenCodec.encodeText(token)
        let textAck = try OfflineNoteReceiptAckCodec.encodeText(receiptAck)
        let textChallengeBytes = try OfflineNoteTransferHandoff.nearbyTextEnvelopeBytes(
            payload: textChallenge,
            kind: .receiveRequest,
            pairingChallenge: challenge
        )
        let textPaymentBytes = try OfflineNoteTransferHandoff.nearbyTextEnvelopeBytes(
            payload: textPayment,
            kind: .paymentToken
        )
        let textAckBytes = try OfflineNoteTransferHandoff.nearbyTextEnvelopeBytes(
            payload: textAck,
            kind: .receiptAck
        )

        XCTAssertEqual(try OfflineNoteNearbyEnvelope.decode(challengeEnvelope.encoded()).pairingChallenge, challenge)
        XCTAssertEqual(
            try OfflineNoteNearbyEnvelope.decode(challengeEnvelope.encoded()).receiveRequest().outputCommitmentHex,
            receiveRequest.outputCommitmentHex
        )
        XCTAssertEqual(paymentEnvelope.kind, .payment)
        XCTAssertEqual(try paymentEnvelope.paymentToken().tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(try OfflineNoteTransferHandoff.decodeNearbyPaymentToken(from: paymentBytes).tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(
            try OfflineNoteNearbyEnvelope.decode(ackEnvelope.encoded()).receiptAck().tokenIdHex,
            receiptAck.tokenIdHex
        )
        XCTAssertFalse(challengeEnvelope.requiresDisconnectGraceAfterSend)
        XCTAssertFalse(paymentEnvelope.requiresDisconnectGraceAfterSend)
        XCTAssertTrue(ackEnvelope.requiresDisconnectGraceAfterSend)
        XCTAssertEqual(
            ackEnvelope.recommendedDisconnectGraceNanosecondsAfterSend,
            OfflineNoteNearbyTransportPolicy.receiptAckDisconnectGraceNanoseconds
        )
        XCTAssertEqual(
            OfflineNoteNearbyTransportPolicy.disconnectGraceNanosecondsAfterSending(.payment),
            0
        )
        XCTAssertEqual(
            try OfflineNoteTransferHandoff.decodeNearbyTextPayload(
                from: textChallengeBytes,
                expectedKind: .receiveRequest
            ).payload,
            textChallenge
        )
        XCTAssertEqual(
            try OfflineNoteTransferHandoff.decodeNearbyTextPayload(
                from: textPaymentBytes,
                expectedKind: .paymentToken
            ).payload,
            textPayment
        )
        XCTAssertEqual(
            try OfflineNoteTransferHandoff.decodeNearbyTextPayload(
                from: textAckBytes,
                expectedKind: .receiptAck
            ).payload,
            textAck
        )
    }

    func testOfflineNoteNearbyEnvelopeRejectsAdversarialMessages() throws {
        let fixture = try Self.loadFixture()
        let tokenPayload = try Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64)
        let pairing = try OfflineNoteNearbyPairingChallenge(assetName: "nearby_pairing_mask")

        XCTAssertThrowsError(try OfflineNoteNearbyPairingChallenge(assetName: "nearby_pairing_mask<script>"))
        XCTAssertThrowsError(
            try OfflineNoteNearbyEnvelope(
                kind: .receiveRequest,
                payload: Data("challenge".utf8),
                contentType: OfflineNoteTransferHandoff.receiveRequestContentType
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteNearbyEnvelope(
                kind: .receiveRequest,
                payload: Data("challenge".utf8),
                contentType: OfflineNoteTransferHandoff.receiptAckContentType,
                pairingChallenge: pairing
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteNearbyEnvelope(
                kind: .payment,
                payload: tokenPayload,
                contentType: OfflineNoteTransferHandoff.paymentTokenContentType,
                pairingChallenge: pairing
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteNearbyEnvelope(
                kind: .payment,
                payload: Data(repeating: 0x01, count: OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes + 1),
                contentType: OfflineNoteTransferHandoff.paymentTokenContentType
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteNearbyEnvelope(
                kind: .receiptAck,
                payload: Data("ok".utf8),
                contentType: OfflineNoteTransferHandoff.receiveRequestContentType
            )
        )
        let invalidTextReceiveRequest = try OfflineNoteTransferTextPayloadCodec.encode(
            Data("garbage".utf8),
            kind: .receiveRequest
        )
        let invalidTextPayment = try OfflineNoteTransferTextPayloadCodec.encode(
            Data("garbage".utf8),
            kind: .paymentToken
        )
        let invalidTextAck = try OfflineNoteTransferTextPayloadCodec.encode(
            Data("garbage".utf8),
            kind: .receiptAck
        )
        XCTAssertThrowsError(
            try OfflineNoteNearbyEnvelope(
                kind: .receiveRequest,
                payload: Data(invalidTextReceiveRequest.utf8),
                contentType: OfflineNoteTransferHandoff.textReceiveRequestContentType,
                pairingChallenge: pairing
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteNearbyEnvelope(
                kind: .payment,
                payload: Data(invalidTextPayment.utf8),
                contentType: OfflineNoteTransferHandoff.textPaymentTokenContentType
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteNearbyEnvelope(
                kind: .receiptAck,
                payload: Data(invalidTextAck.utf8),
                contentType: OfflineNoteTransferHandoff.textReceiptAckContentType
            )
        )

        let unknownField = Data(
            #"{"kind":"payment","payload":"AQID","contentType":"application/vnd.iroha.offline.payment-token+norito","extra":true}"#.utf8
        )
        let challengeContentTypeDowngrade = Data(
            #"{"kind":"receive_request","payload":"YQ","contentType":"application/vnd.iroha.offline.receipt-ack+norito","pairingChallenge":"nearby_pairing_bird"}"#.utf8
        )
        let ackContentTypeDowngrade = Data(
            #"{"kind":"receipt_ack","payload":"b2s","contentType":"application/vnd.iroha.offline.receive-request+norito"}"#.utf8
        )
        let paddedPayload = Data(
            #"{"kind":"receive_request","payload":"YQ==","contentType":"application/vnd.iroha.offline.receive-request+norito","pairingChallenge":"nearby_pairing_bird"}"#.utf8
        )
        XCTAssertThrowsError(try OfflineNoteNearbyEnvelope.decode(unknownField))
        XCTAssertThrowsError(try OfflineNoteNearbyEnvelope.decode(challengeContentTypeDowngrade))
        XCTAssertThrowsError(try OfflineNoteNearbyEnvelope.decode(ackContentTypeDowngrade))
        XCTAssertThrowsError(try OfflineNoteNearbyEnvelope.decode(paddedPayload))

        let topLevelArray = Data(#"[]"#.utf8)
        let invalidBase64Payload = Data(
            #"{"kind":"receive_request","payload":"!!!!","contentType":"application/vnd.iroha.offline.receive-request+norito","pairingChallenge":"nearby_pairing_bird"}"#.utf8
        )
        let badPairingObject = Data(
            #"{"kind":"receive_request","payload":"YQ","contentType":"application/vnd.iroha.offline.receive-request+norito","pairingChallenge":{"assetName":1}}"#.utf8
        )
        let smuggledPairingObject = Data(
            #"{"kind":"receive_request","payload":"YQ","contentType":"application/vnd.iroha.offline.receive-request+norito","pairingChallenge":{"assetName":"nearby_pairing_bird","extra":true}}"#.utf8
        )
        let ackWithPairing = Data(
            #"{"kind":"receipt_ack","payload":"b2s","contentType":"application/vnd.iroha.offline.receipt-ack+norito","pairingChallenge":"nearby_pairing_bird"}"#.utf8
        )
        let decodedInvalidTextPayment = Data(
            """
            {"kind":"payment","payload":"\(Self.base64Url(Data(invalidTextPayment.utf8)))","contentType":"text/vnd.iroha.offline.payment-token"}
            """.utf8
        )
        XCTAssertThrowsError(try OfflineNoteNearbyEnvelope.decode(topLevelArray))
        XCTAssertThrowsError(try OfflineNoteNearbyEnvelope.decode(invalidBase64Payload))
        XCTAssertThrowsError(try OfflineNoteNearbyEnvelope.decode(badPairingObject))
        XCTAssertThrowsError(try OfflineNoteNearbyEnvelope.decode(smuggledPairingObject))
        XCTAssertThrowsError(try OfflineNoteNearbyEnvelope.decode(ackWithPairing))
        XCTAssertThrowsError(try OfflineNoteNearbyEnvelope.decode(decodedInvalidTextPayment))
        XCTAssertThrowsError(
            try OfflineNoteNearbyEnvelope(
                kind: .payment,
                payload: Data([0x01, 0x02, 0x03]),
                contentType: OfflineNoteTransferHandoff.paymentTokenContentType
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteNearbyEnvelope(
                kind: .receiptAck,
                payload: Data(),
                contentType: OfflineNoteTransferHandoff.receiptAckContentType
            )
        )
    }

    func testOfflineNoteWalletAcceptsCanonicalSdkInteropPaymentToken() throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let recipientStore = InMemoryOfflineNoteStore()
        let recipientWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            attestationProvider: StaticAttestationProvider(certificate: recipientCertificate),
            store: recipientStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.recipientNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_001_200 }
        )
        let receiveRequest = try recipientWallet.prepareReceive(
            assetDefinitionId: Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId),
            amount: fixture.chainVectors.redeem.amount
        )
        XCTAssertEqual(receiveRequest.outputCommitmentHex, derivation.recipientOutputCommitment)

        let token = try OfflineNotePaymentTokenCodec.decodeNorito(
            Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64)
        )
        let pending = try XCTUnwrap(
            recipientStore.findNote(noteCommitment: Self.hex(derivation.recipientOutputCommitment))
        )
        let output = token.audit.outputClaims[0]
        XCTAssertEqual(pending.assetId, output.assetId)
        XCTAssertEqual(pending.amount, output.amount)
        XCTAssertEqual(try pending.keyCertificate.payloadHash(), try output.keyCertificate.payloadHash())
        if case let .p2pOutput(origin) = pending.origin {
            XCTAssertEqual(origin.paymentRequestId, token.paymentRequestId)
            XCTAssertEqual(origin.outputIndex, 0)
        } else {
            XCTFail("expected P2P pending output origin")
        }
        let accepted = try recipientWallet.accept(token)

        XCTAssertEqual(accepted.noteCommitmentHex, derivation.recipientOutputCommitment)
        XCTAssertEqual(accepted.state, .spendable)
        XCTAssertEqual(
            try recipientStore.findNote(noteCommitment: Self.hex(derivation.recipientOutputCommitment))?.state,
            .spendable
        )
    }

    func testOfflineNoteWalletNoteJsonCodecRoundTripsFixtureNote() throws {
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)

        let decoded = try OfflineNoteWalletNoteJsonCodec.decode(
            OfflineNoteWalletNoteJsonCodec.encode(note)
        )

        XCTAssertEqual(decoded, note)
        XCTAssertEqual(try decoded.keyCertificate.noritoEncoded(), try note.keyCertificate.noritoEncoded())

        var spendPendingObject = try XCTUnwrap(
            JSONSerialization.jsonObject(with: OfflineNoteWalletNoteJsonCodec.encode(note)) as? [String: Any]
        )
        spendPendingObject["state"] = "spendPending"
        let migratedSpent = try OfflineNoteWalletNoteJsonCodec.decode(
            JSONSerialization.data(withJSONObject: spendPendingObject, options: [.sortedKeys])
        )
        XCTAssertEqual(migratedSpent.state, .spent)

        var changePendingObject = spendPendingObject
        changePendingObject["state"] = "CHANGE_PENDING"
        let migratedChange = try OfflineNoteWalletNoteJsonCodec.decode(
            JSONSerialization.data(withJSONObject: changePendingObject, options: [.sortedKeys])
        )
        XCTAssertEqual(migratedChange.state, .spendable)
    }

    func testOfflineNoteKeychainStoreRejectsInvalidLabel() {
        XCTAssertThrowsError(try OfflineNoteKeychainStore(label: "bad/label")) { error in
            XCTAssertEqual(error as? OfflineNoteKeychainStoreError, .invalidLabel("bad/label"))
        }
    }

    func testOfflineNoteKeychainStoreMovesMetadataBeforeDeletingOldCollection() throws {
        let label = "atomic-order-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteKeychainBacking()
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)

        try store.upsert(note)
        backing.operations.removeAll()
        try store.upsert(note)

        let metadataSave = try XCTUnwrap(backing.operations.firstIndex(of: "save:\(label).meta"))
        let oldCollectionDelete = try XCTUnwrap(backing.operations.firstIndex(of: "delete:\(label).rev.1"))
        XCTAssertLessThan(metadataSave, oldCollectionDelete)

        backing.operations.removeAll()
        try store.clear()

        let metadataDelete = try XCTUnwrap(backing.operations.firstIndex(of: "delete:\(label).meta"))
        let currentCollectionDelete = try XCTUnwrap(backing.operations.firstIndex(of: "delete:\(label).rev.2"))
        XCTAssertLessThan(metadataDelete, currentCollectionDelete)
    }

    func testOfflineNoteKeychainStoreKeepsOldRevisionWhenMetadataSaveFails() throws {
        let label = "metadata-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteKeychainBacking()
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)

        try store.upsert(note)
        backing.operations.removeAll()
        backing.saveFailures.insert("\(label).meta")

        let updated = try note.withState(.spent, updatedAtMs: note.updatedAtMs + 1)
        XCTAssertThrowsError(try store.upsert(updated)) { error in
            XCTAssertEqual(error as? OfflineNoteKeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertFalse(backing.operations.contains("delete:\(label).rev.1"))
        XCTAssertNotNil(backing.values["\(label).rev.2"])

        backing.saveFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spendable)
        XCTAssertEqual(loaded.updatedAtMs, note.updatedAtMs)
    }

    func testOfflineNoteKeychainStoreKeepsOldRevisionWhenCollectionSaveFails() throws {
        let label = "collection-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteKeychainBacking()
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)

        try store.upsert(note)
        backing.operations.removeAll()
        backing.saveFailures.insert("\(label).rev.2")

        let updated = try note.withState(.spent, updatedAtMs: note.updatedAtMs + 1)
        XCTAssertThrowsError(try store.upsert(updated)) { error in
            XCTAssertEqual(error as? OfflineNoteKeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertNil(backing.values["\(label).rev.2"])
        XCTAssertFalse(backing.operations.contains("save:\(label).meta"))

        backing.saveFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spendable)
    }

    func testOfflineNoteKeychainStoreKeepsLegacyCollectionWhenMetadataSaveFails() throws {
        let label = "legacy-metadata-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteKeychainBacking()
        backing.values[label] = try Self.storedCollectionData(notes: [note])
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)
        backing.saveFailures.insert("\(label).meta")

        let updated = try note.withState(.spent, updatedAtMs: note.updatedAtMs + 1)
        XCTAssertThrowsError(try store.upsert(updated)) { error in
            XCTAssertEqual(error as? OfflineNoteKeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertFalse(backing.operations.contains("delete:\(label)"))

        backing.saveFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spendable)
    }

    func testOfflineNoteKeychainStoreUsesNewRevisionWhenOldCollectionDeleteFails() throws {
        let label = "delete-old-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteKeychainBacking()
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)

        try store.upsert(note)
        backing.operations.removeAll()
        backing.deleteFailures.insert("\(label).rev.1")

        let updated = try note.withState(.spent, updatedAtMs: note.updatedAtMs + 1)
        XCTAssertThrowsError(try store.upsert(updated)) { error in
            XCTAssertEqual(error as? OfflineNoteKeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertNotNil(backing.values["\(label).rev.1"])

        backing.deleteFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spent)
    }

    func testOfflineNoteKeychainStoreRejectsMetadataPointerToMissingCollection() throws {
        let label = "missing-collection-test"
        let backing = RecordingOfflineNoteKeychainBacking()
        backing.values["\(label).meta"] = try Self.storedMetadataData(revision: 99)
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)

        XCTAssertThrowsError(try store.listNotes()) { error in
            XCTAssertEqual(
                error as? OfflineNoteKeychainStoreError,
                .corrupt("collection revision is missing")
            )
        }
    }

    func testOfflineNoteKeychainStoreDoesNotFallBackToLegacyCollectionWhenMetadataExists() throws {
        let label = "metadata-blocks-legacy-fallback-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteKeychainBacking()
        backing.values[label] = try Self.storedCollectionData(notes: [note])
        backing.values["\(label).meta"] = try Self.storedMetadataData(revision: 2)
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)

        XCTAssertThrowsError(try store.listNotes()) { error in
            XCTAssertEqual(
                error as? OfflineNoteKeychainStoreError,
                .corrupt("collection revision is missing")
            )
        }
    }

    func testOfflineNoteKeychainStoreRejectsInvalidMetadataShapes() throws {
        for (label, metadata, expected) in [
            (
                "metadata-version-test",
                try Self.storedMetadataData(version: 2, revision: 1),
                OfflineNoteKeychainStoreError.corrupt("unsupported metadata")
            ),
            (
                "metadata-zero-revision-test",
                try Self.storedMetadataData(version: 1, revision: 0),
                OfflineNoteKeychainStoreError.corrupt("unsupported metadata")
            )
        ] {
            let backing = RecordingOfflineNoteKeychainBacking()
            backing.values["\(label).meta"] = metadata
            let store = try OfflineNoteKeychainStore(label: label, backing: backing)

            XCTAssertThrowsError(try store.listNotes()) { error in
                XCTAssertEqual(error as? OfflineNoteKeychainStoreError, expected)
            }
        }
    }

    func testOfflineNoteKeychainStoreRejectsInvalidMetadataJson() throws {
        let label = "metadata-json-test"
        let backing = RecordingOfflineNoteKeychainBacking()
        backing.values["\(label).meta"] = Data("{".utf8)
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)

        XCTAssertThrowsError(try store.listNotes()) { error in
            guard case let OfflineNoteKeychainStoreError.corrupt(reason) = error else {
                return XCTFail("expected corrupt metadata, got \(error)")
            }
            XCTAssertTrue(reason.contains("failed to decode metadata"))
        }
    }

    func testOfflineNoteKeychainStoreRejectsCorruptCollections() throws {
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let validPayload = try OfflineNoteWalletNoteJsonCodec.encode(note).base64EncodedString()
        let cases: [(String, [String: Any], String)] = [
            (
                "collection-version-test",
                ["version": 2, "notes": []],
                "unsupported collection version"
            ),
            (
                "collection-base64-test",
                ["version": 1, "notes": [["commitmentHex": note.noteCommitmentHex, "payloadBase64": "%%%"]]],
                "note payload is not base64"
            ),
            (
                "collection-index-test",
                ["version": 1, "notes": [["commitmentHex": String(repeating: "0", count: 64), "payloadBase64": validPayload]]],
                "note commitment index mismatch"
            ),
            (
                "collection-duplicate-test",
                [
                    "version": 1,
                    "notes": [
                        ["commitmentHex": note.noteCommitmentHex, "payloadBase64": validPayload],
                        ["commitmentHex": note.noteCommitmentHex, "payloadBase64": validPayload]
                    ]
                ],
                "duplicate note commitment"
            )
        ]

        for (label, collection, reason) in cases {
            let backing = RecordingOfflineNoteKeychainBacking()
            backing.values[label] = try JSONSerialization.data(
                withJSONObject: collection,
                options: [.sortedKeys]
            )
            let store = try OfflineNoteKeychainStore(label: label, backing: backing)

            XCTAssertThrowsError(try store.listNotes()) { error in
                XCTAssertEqual(error as? OfflineNoteKeychainStoreError, .corrupt(reason))
            }
        }
    }

    func testOfflineNoteKeychainStoreRejectsInvalidCollectionJson() throws {
        let label = "collection-json-test"
        let backing = RecordingOfflineNoteKeychainBacking()
        backing.values[label] = Data("{".utf8)
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)

        XCTAssertThrowsError(try store.listNotes()) { error in
            guard case let OfflineNoteKeychainStoreError.corrupt(reason) = error else {
                return XCTFail("expected corrupt collection, got \(error)")
            }
            XCTAssertTrue(reason.contains("failed to decode collection"))
        }
    }

    func testOfflineNoteKeychainStoreDoesNotDeleteCollectionWhenMetadataClearFails() throws {
        let label = "clear-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteKeychainBacking()
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)

        try store.upsert(note)
        backing.operations.removeAll()
        backing.deleteFailures.insert("\(label).meta")

        XCTAssertThrowsError(try store.clear()) { error in
            XCTAssertEqual(error as? OfflineNoteKeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertFalse(backing.operations.contains("delete:\(label).rev.1"))

        backing.deleteFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spendable)
    }

    func testOfflineNoteKeychainStoreDoesNotResurrectRevisionWhenClearCollectionDeleteFails() throws {
        let label = "clear-collection-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteKeychainBacking()
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)

        try store.upsert(note)
        backing.operations.removeAll()
        backing.deleteFailures.insert("\(label).rev.1")

        XCTAssertThrowsError(try store.clear()) { error in
            XCTAssertEqual(error as? OfflineNoteKeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertNil(backing.values["\(label).meta"])
        XCTAssertNotNil(backing.values["\(label).rev.1"])

        backing.deleteFailures.removeAll()
        XCTAssertTrue(try store.listNotes().isEmpty)
    }

    func testOfflineNoteKeychainStoreUsesMetadataWhenLegacyDeleteFails() throws {
        let label = "legacy-delete-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteKeychainBacking()
        backing.values[label] = try Self.storedCollectionData(notes: [note])
        let store = try OfflineNoteKeychainStore(label: label, backing: backing)
        backing.deleteFailures.insert(label)

        let updated = try note.withState(.spent, updatedAtMs: note.updatedAtMs + 1)
        XCTAssertThrowsError(try store.upsert(updated)) { error in
            XCTAssertEqual(error as? OfflineNoteKeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertNotNil(backing.values[label])
        XCTAssertNotNil(backing.values["\(label).meta"])

        backing.deleteFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spent)
        XCTAssertEqual(loaded.updatedAtMs, updated.updatedAtMs)
    }

    func testOfflineNoteWalletDerivationsMatchRustVectors() throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let recipientOutput = fixture.paymentToken.outputClaims[0]
        let changeOutput = fixture.paymentToken.outputClaims[1]

        let sourceCommitment = try OfflineNoteCommitmentPreimage(
            chainId: derivation.chainId,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount,
            noteSecret: Self.hex(derivation.sourceNoteSecretHex),
            origin: .issuerLoad(OfflineNoteIssuerLoadOrigin(
                operationId: derivation.issuerLoadOperationId,
                lineageId: derivation.issuerLoadLineageId,
                localRevision: derivation.issuerLoadLocalRevision
            ))
        ).deriveNoteCommitment()
        XCTAssertEqual(sourceCommitment.hexLowercased(), derivation.sourceNoteCommitment)

        let inputNullifier = try OfflineNoteInputNullifierPreimage(
            chainId: derivation.chainId,
            sourceNoteCommitment: sourceCommitment,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            noteSecret: Self.hex(derivation.sourceNoteSecretHex)
        ).deriveInputNullifier()
        XCTAssertEqual(inputNullifier.hexLowercased(), derivation.inputNullifier)

        let recipientCommitment = try OfflineNoteCommitmentPreimage(
            chainId: derivation.chainId,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.recipientKeyCertificatePayloadHash),
            assetId: "\(recipientOutput.assetDefinitionId)#\(recipientOutput.accountId)",
            amount: recipientOutput.amount,
            noteSecret: Self.hex(derivation.recipientNoteSecretHex),
            origin: .p2pOutput(OfflineNoteP2pOutputOrigin(
                paymentRequestId: derivation.paymentRequestId,
                outputIndex: 0
            ))
        ).deriveNoteCommitment()
        XCTAssertEqual(recipientCommitment.hexLowercased(), derivation.recipientOutputCommitment)

        let changeCommitment = try OfflineNoteCommitmentPreimage(
            chainId: derivation.chainId,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            assetId: "\(changeOutput.assetDefinitionId)#\(changeOutput.accountId)",
            amount: changeOutput.amount,
            noteSecret: Self.hex(derivation.changeNoteSecretHex),
            origin: .p2pOutput(OfflineNoteP2pOutputOrigin(
                paymentRequestId: derivation.paymentRequestId,
                outputIndex: 1
            ))
        ).deriveNoteCommitment()
        XCTAssertEqual(changeCommitment.hexLowercased(), derivation.changeOutputCommitment)

        let tokenId = try OfflineNotePaymentTokenIdPreimage(
            chainId: derivation.chainId,
            paymentRequestId: derivation.paymentRequestId,
            createdAtMs: fixture.paymentToken.createdAtMs,
            tokenNonce: Self.hex(derivation.tokenNonceHex),
            senderKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            inputNullifiers: [inputNullifier],
            outputCommitments: [recipientCommitment, changeCommitment]
        ).derivePaymentTokenId()
        XCTAssertEqual(tokenId.hexLowercased(), derivation.paymentTokenId)

        let redeemNullifier = try OfflineNoteInputNullifierPreimage(
            chainId: derivation.chainId,
            sourceNoteCommitment: recipientCommitment,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.recipientKeyCertificatePayloadHash),
            noteSecret: Self.hex(derivation.recipientNoteSecretHex)
        ).deriveInputNullifier()
        XCTAssertEqual(redeemNullifier.hexLowercased(), derivation.redeemNullifier)
    }

    func testOfflinePaymentTokensExposeOutputCommitmentMatchingForRecipientAndChange() throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let recipientOutput = fixture.paymentToken.outputClaims[0]
        let changeOutput = fixture.paymentToken.outputClaims[1]
        let nativeToken = try OfflineNotePaymentTokenCodec.decodeNorito(
            Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64)
        )

        XCTAssertTrue(nativeToken.containsOutputNoteCommitment(hex: derivation.recipientOutputCommitment))
        XCTAssertTrue(nativeToken.containsOutputNoteCommitment(
            hex: " 0x\(derivation.changeOutputCommitment.uppercased()) "
        ))
        XCTAssertEqual(
            nativeToken.outputClaim(matchingNoteCommitment: try Self.hex(derivation.changeOutputCommitment))?.amount,
            changeOutput.amount
        )
        XCTAssertFalse(nativeToken.containsOutputNoteCommitment(Data(repeating: 0xFF, count: 32)))
        XCTAssertFalse(nativeToken.containsOutputNoteCommitment(hex: "not-hex"))

        let senderCompact = OfflineCompactKeyCertificate(
            certificate: try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        )
        let recipientCompact = OfflineCompactKeyCertificate(
            certificate: try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        )
        let compatibilityToken = OfflinePaymentToken(
            tokenId: derivation.paymentTokenId,
            invoiceId: derivation.paymentRequestId,
            senderAccountId: fixture.paymentToken.senderAccountId,
            recipientAccountId: fixture.paymentToken.recipientAccountId,
            assetDefinitionId: recipientOutput.assetDefinitionId,
            amount: recipientOutput.amount,
            changeAmount: changeOutput.amount,
            sourceNoteCommitment: derivation.sourceNoteCommitment,
            inputNullifiers: [derivation.inputNullifier],
            inputClaims: [],
            outputCommitments: fixture.chainVectors.audit.outputCommitments,
            outputClaims: [
                OfflinePaymentTokenOutputClaim(
                    noteCommitment: recipientOutput.noteCommitment,
                    keyCertificate: recipientCompact,
                    accountId: recipientOutput.accountId,
                    assetDefinitionId: recipientOutput.assetDefinitionId,
                    amount: recipientOutput.amount
                ),
                OfflinePaymentTokenOutputClaim(
                    noteCommitment: changeOutput.noteCommitment,
                    keyCertificate: senderCompact,
                    accountId: changeOutput.accountId,
                    assetDefinitionId: changeOutput.assetDefinitionId,
                    amount: changeOutput.amount
                )
            ],
            senderKeyCertificate: senderCompact,
            recipientKeyCertificate: recipientCompact,
            oneUseAssertion: OfflineOneUseAssertion(
                platform: senderCompact.platform,
                keyId: senderCompact.keyId,
                counter: 1,
                challengeHashHex: fixture.chainVectors.audit.publicInputsHash,
                assertionBase64: Data("assertion".utf8).base64EncodedString()
            ),
            recursiveProof: OfflineRecursiveProof(
                publicInputsHashHex: fixture.chainVectors.audit.publicInputsHash,
                proofBytesBase64: Data("proof".utf8).base64EncodedString()
            ),
            createdAtMs: fixture.paymentToken.createdAtMs
        )

        XCTAssertTrue(compatibilityToken.containsOutputNoteCommitment(derivation.recipientOutputCommitment))
        XCTAssertTrue(compatibilityToken.containsOutputNoteCommitment(
            " 0x\(derivation.changeOutputCommitment.uppercased()) "
        ))
        XCTAssertEqual(
            compatibilityToken.outputClaim(matchingNoteCommitment: derivation.changeOutputCommitment)?.amount,
            changeOutput.amount
        )
        XCTAssertFalse(compatibilityToken.containsOutputNoteCommitment(String(repeating: "f", count: 64)))
    }

    func testOfflineNotePublicInputHashesMatchRustVectors() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)

        XCTAssertEqual(try audit.publicInputsHash().hexLowercased(), fixture.chainVectors.audit.publicInputsHash)
        XCTAssertEqual(try redeem.publicInputsHash().hexLowercased(), fixture.chainVectors.redeem.publicInputsHash)
        XCTAssertNoThrow(try audit.validateProofBinding())
        XCTAssertNoThrow(try redeem.validateProofBinding())
    }

    func testOfflineNoteWalletLoadDerivesCommitmentBeforeIssuerSubmission() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let loadContext = OfflineNoteLoadContext(
            operationId: derivation.issuerLoadOperationId,
            lineageId: derivation.issuerLoadLineageId,
            localRevision: derivation.issuerLoadLocalRevision,
            keyCertificate: senderCertificate
        )
        let issuerClient = RecordingIssuerClient(loadContext: loadContext)
        let wallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: Self.accountId(fromAssetId: fixture.chainVectors.issue.assetId),
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            issuerClient: issuerClient,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.sourceNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_001_000 }
        )

        let note = try await wallet.load(
            assetDefinitionId: Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId),
            amount: fixture.chainVectors.issue.amount
        )

        XCTAssertEqual(note.noteCommitmentHex, derivation.sourceNoteCommitment)
        XCTAssertEqual(issuerClient.lastIssueRequest?.noteCommitment.hexLowercased(), derivation.sourceNoteCommitment)
        XCTAssertEqual(note.state, .spendable)
    }

    /// Regression: `Wallet.load(assetDefinitionId:)` must forward the
    /// asset definition id verbatim to the issuer client. An earlier
    /// revision derived the value from the SDK-internal 2-part `assetId
    /// = name#account`, which dropped any suffix after the first `#`
    /// (e.g. `someBase58Alias#extra` → `someBase58Alias`). The pass-
    /// through is verified against the Base58 form that the wallet's
    /// note-commitment encoding requires.
    func testWalletLoadForwardsAssetDefinitionIdVerbatimToIssuerClient() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let loadContext = OfflineNoteLoadContext(
            operationId: derivation.issuerLoadOperationId,
            lineageId: derivation.issuerLoadLineageId,
            localRevision: derivation.issuerLoadLocalRevision,
            keyCertificate: senderCertificate
        )
        let issuerClient = RecordingIssuerClient(loadContext: loadContext)
        let wallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: Self.accountId(fromAssetId: fixture.chainVectors.issue.assetId),
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            issuerClient: issuerClient,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.sourceNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_001_000 }
        )

        // The Base58 form is what the wallet's note-commitment encoding
        // requires (assetId = `<asset-def-base58>#<account>`); the
        // regression check is that the exact Base58 value is forwarded
        // to the issuer client, not the substring before any later `#`.
        let assetDefinitionId = Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId)
        _ = try await wallet.load(
            assetDefinitionId: assetDefinitionId,
            amount: fixture.chainVectors.issue.amount
        )

        XCTAssertEqual(
            issuerClient.lastPrepareLoadAssetDefinitionId,
            assetDefinitionId,
            "prepareLoad must receive the assetDefinitionId verbatim; an earlier revision derived it from the SDK-internal assetId which dropped suffixes"
        )
        XCTAssertEqual(
            issuerClient.lastIssueRequest?.assetDefinitionId,
            assetDefinitionId,
            "issueNote must receive the assetDefinitionId verbatim; an earlier revision derived it from the SDK-internal assetId which dropped suffixes"
        )
    }

    func testToriiIssuerClientBodySignsRefillAndIssuesWalletCommitment() async throws {
        let fixture = try Self.loadFixture()
        let certificate = fixture.paymentToken.senderKeyCertificate
        let accountId = certificate.accountId
        let assetDefinitionId = Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId)
        let offlinePublicKey = String(repeating: "a5", count: 32)
        let deviceBinding = try OfflineNoteIssuerDeviceBinding(
            deviceId: "device-1",
            offlinePublicKey: offlinePublicKey,
            deviceBinding: [
                "device_id": "device-1",
                "attestation_key_id": "attestation-key-1",
                "offline_public_key": offlinePublicKey,
                "signature_base64": "nested-device-signature-is-not-body-auth",
            ]
        )
        OfflineIssuerURLProtocol.reset()
        OfflineIssuerURLProtocol.handler = { request in
            let body = try Self.requestBody(request)
            let response: [String: Any]
            switch request.url?.path {
            case "/v1/offline/keys/refill":
                response = [
                    "operation_id": try Self.string(body, "operation_id"),
                    "lineage_state": Self.lineageState(revision: 0, balance: "0"),
                    "key_certificate": Self.certificateJSON(certificate, expiresAtMs: 1_700_000_060_000),
                    "key_certificates": [Self.certificateJSON(certificate, expiresAtMs: 1_700_000_060_000)],
                ]
            case "/v1/offline/notes/issue":
                response = [
                    "operation_id": try Self.string(body, "operation_id"),
                    "settlement": ["entry_hash": "settlement-entry-hash"],
                    "lineage_state": Self.lineageState(revision: 1, balance: "5"),
                    "local_balance": "5",
                    "locked_balance": "0",
                    "local_revision": 1,
                    "local_state_hash": "lineage-state-hash",
                    "issued_note_commitment": try Self.string(body, "note_commitment"),
                    "key_certificate": Self.certificateJSON(certificate, expiresAtMs: 1_700_000_060_000),
                    "key_certificates": [Self.certificateJSON(certificate, expiresAtMs: 1_700_000_060_000)],
                ]
            default:
                throw ToriiOfflineNoteIssuerClientError.invalidURL(request.url?.path ?? "")
            }
            return (200, try JSONSerialization.data(withJSONObject: response, options: [.sortedKeys]))
        }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [OfflineIssuerURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let client = ToriiOfflineNoteIssuerClient(
            baseURL: URL(string: "https://torii.example")!,
            session: session,
            canonicalAuth: ToriiCanonicalRequestAuth(
                accountId: accountId,
                privateKey: Data(0..<32)
            ),
            deviceBindingProvider: StaticIssuerDeviceBindingProvider(binding: deviceBinding),
            clock: { 1_700_000_000_000 },
            nonceGenerator: SequenceIdGenerator(ids: [
                "operation-refill-1",
                "auth-refill-1",
                "auth-issue-1",
            ])
        )

        let context = try await client.prepareLoad(
            chainId: "chain-1",
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            amount: "5"
        )
        XCTAssertEqual(context.operationId, "operation-refill-1")
        XCTAssertEqual(context.lineageId, "lineage-1")
        XCTAssertEqual(context.localRevision, 1)

        let commitment = Data((1...32).map(UInt8.init))
        let response = try await client.issueNote(OfflineNoteIssueRequest(
            chainId: "chain-1",
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            assetId: "\(assetDefinitionId)#\(accountId)",
            amount: "5",
            loadContext: context,
            noteCommitment: commitment
        ))

        XCTAssertEqual(response.noteCommitment, commitment)
        XCTAssertEqual(response.settlementEntryHashHex, "settlement-entry-hash")
        let requests = OfflineIssuerURLProtocol.requests
        XCTAssertEqual(requests.count, 2)
        XCTAssertEqual(requests[0].url?.path, "/v1/offline/keys/refill")
        XCTAssertEqual(requests[1].url?.path, "/v1/offline/notes/issue")
        for request in requests {
            XCTAssertFalse((request.allHTTPHeaderFields ?? [:]).keys.contains { $0.lowercased().hasPrefix("x-iroha-") })
        }
        let refillBody = try Self.requestBody(requests[0])
        XCTAssertEqual(try Self.string(refillBody, "account_id"), accountId)
        XCTAssertEqual(try Self.string(refillBody, "operation_id"), "operation-refill-1")
        XCTAssertEqual(try Self.uint64(refillBody, "local_revision"), 0)
        XCTAssertEqual(try Self.string(refillBody, "local_state_hash"), "")
        XCTAssertEqual(try Self.string(refillBody, "attestation_key_id"), "attestation-key-1")
        XCTAssertEqual(try Self.string(refillBody, "nonce"), "auth-refill-1")
        XCTAssertFalse(try Self.string(refillBody, "signature_base64").isEmpty)
        XCTAssertEqual(
            try Self.string(try Self.object(refillBody, "device_binding"), "signature_base64"),
            "nested-device-signature-is-not-body-auth"
        )

        let issueBody = try Self.requestBody(requests[1])
        XCTAssertEqual(try Self.string(issueBody, "note_commitment"), commitment.hexLowercased())
        XCTAssertEqual(try Self.uint64(issueBody, "local_revision"), 0)
        XCTAssertEqual(try Self.string(issueBody, "local_balance"), "0")
        XCTAssertEqual(try Self.string(issueBody, "nonce"), "auth-issue-1")
        _ = try Self.object(issueBody, "lineage_state")
    }

    /// Regression: the canonical body sent to Torii must NOT escape `/` as
    /// `\/`. The server reconstructs the signing bytes via `norito::json::to_vec`
    /// which never escapes slashes; if Swift's `JSONSerialization` does, the
    /// reconstructed bytes diverge and every refill / issue fails with
    /// `OFFLINE_SIGNATURE_INVALID` (403). Base64 fields routinely contain `/`,
    /// so this is hit by every real device binding in practice.
    func testRefillBodyDoesNotEscapeForwardSlashesForCanonicalSigning() async throws {
        let fixture = try Self.loadFixture()
        let certificate = fixture.paymentToken.senderKeyCertificate
        let accountId = certificate.accountId
        let assetDefinitionId = Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId)
        // Deliberately use a base64-shaped value that contains `/` so we
        // exercise the escape path. `JSONSerialization` with only
        // `[.sortedKeys]` would emit `\/` here.
        let offlinePublicKey = "AB//CD/EFGH//IJ="
        let attestationKeyId = "attestation/key//with/slash"
        let deviceBinding = try OfflineNoteIssuerDeviceBinding(
            deviceId: "device-1",
            offlinePublicKey: offlinePublicKey,
            deviceBinding: [
                "device_id": "device-1",
                "attestation_key_id": attestationKeyId,
                "offline_public_key": offlinePublicKey,
                "signature_base64": "ABC//DEF/GHI/JKL=",
            ]
        )
        OfflineIssuerURLProtocol.reset()
        OfflineIssuerURLProtocol.handler = { request in
            let body = try Self.requestBody(request)
            let response: [String: Any] = [
                "operation_id": try Self.string(body, "operation_id"),
                "lineage_state": Self.lineageState(revision: 0, balance: "0"),
                "key_certificate": Self.certificateJSON(certificate, expiresAtMs: 1_700_000_060_000),
                "key_certificates": [Self.certificateJSON(certificate, expiresAtMs: 1_700_000_060_000)],
            ]
            return (200, try JSONSerialization.data(withJSONObject: response, options: [.sortedKeys]))
        }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [OfflineIssuerURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let client = ToriiOfflineNoteIssuerClient(
            baseURL: URL(string: "https://torii.example")!,
            session: session,
            canonicalAuth: ToriiCanonicalRequestAuth(
                accountId: accountId,
                privateKey: Data(0..<32)
            ),
            deviceBindingProvider: StaticIssuerDeviceBindingProvider(binding: deviceBinding),
            clock: { 1_700_000_000_000 },
            nonceGenerator: SequenceIdGenerator(ids: [
                "operation-refill-slash",
                "auth-refill-slash",
            ])
        )

        _ = try await client.prepareLoad(
            chainId: "chain-1",
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            amount: "5"
        )

        let requests = OfflineIssuerURLProtocol.requests
        XCTAssertEqual(requests.count, 1)
        guard let rawBody = OfflineIssuerURLProtocol.body(for: requests[0]) else {
            return XCTFail("missing request body")
        }
        let rawString = String(decoding: rawBody, as: UTF8.self)
        XCTAssertFalse(
            rawString.contains("\\/"),
            "canonical body must not contain escaped slashes (\\/); server reconstructs bytes via norito::json::to_vec which never escapes /"
        )
        XCTAssertTrue(
            rawString.contains(offlinePublicKey),
            "expected the raw offline_public_key (with /) to appear unescaped in the body"
        )
        XCTAssertTrue(
            rawString.contains(attestationKeyId),
            "expected the raw attestation_key_id (with /) to appear unescaped in the body"
        )
    }

    func testOfflineNoteWalletLifecycleBuildsAuditAcceptAndRedeemTransactions() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let senderStore = InMemoryOfflineNoteStore()
        try senderStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let senderWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: Self.accountId(fromAssetId: fixture.chainVectors.issue.assetId),
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: senderStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.tokenNonceHex),
                try Self.hex(derivation.changeNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { fixture.paymentToken.createdAtMs }
        )
        let recipientSubmitter = RecordingTransactionSubmitter()
        let recipientWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            attestationProvider: StaticAttestationProvider(certificate: recipientCertificate),
            transactionSubmitter: recipientSubmitter,
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.recipientNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_001_200 }
        )

        let receiveRequest = try recipientWallet.prepareReceive(
            assetDefinitionId: Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId),
            amount: fixture.chainVectors.redeem.amount
        )
        XCTAssertEqual(receiveRequest.outputCommitmentHex, derivation.recipientOutputCommitment)

        let token = try senderWallet.pay(receiveRequest)

        XCTAssertEqual(token.tokenIdHex, derivation.paymentTokenId)
        XCTAssertEqual(try token.audit.publicInputsHash().hexLowercased(), fixture.chainVectors.audit.publicInputsHash)
        XCTAssertEqual(token.paymentRequestId, derivation.paymentRequestId)
        XCTAssertEqual(try senderStore.findNote(noteCommitment: try Self.hex(derivation.sourceNoteCommitment))?.state, .spent)
        XCTAssertEqual(try senderStore.findNote(noteCommitment: try Self.hex(derivation.changeOutputCommitment))?.state, .spendable)

        let accepted = try recipientWallet.accept(token)

        XCTAssertEqual(accepted.state, .spendable)
        XCTAssertEqual(recipientSubmitter.audits.count, 0)
        try await recipientWallet.publishAudit(token)
        XCTAssertEqual(recipientSubmitter.audits.count, 1)
        let redeeming = try await recipientWallet.redeem(accepted)
        XCTAssertEqual(redeeming.state, .redeemPending)
        XCTAssertTrue(recipientSubmitter.redemptions.isEmpty)
        XCTAssertEqual(recipientSubmitter.defunds.count, 1)
        let defund = try XCTUnwrap(recipientSubmitter.defunds.first)
        XCTAssertEqual(defund.bearerAuditTrail.map(\.tokenId), [token.tokenId])
        XCTAssertEqual(
            try defund.redemption.publicInputsHash().hexLowercased(),
            fixture.chainVectors.redeem.publicInputsHash
        )
    }

    func testOfflineNoteWalletSyncReconcilesPendingSpendChangeAndRedeemStates() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let senderStore = InMemoryOfflineNoteStore()
        try senderStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let syncResolver = RecordingSyncResolver(resolutions: [
            derivation.sourceNoteCommitment: .spent,
            derivation.changeOutputCommitment: .spendable
        ])
        let senderWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: Self.accountId(fromAssetId: fixture.chainVectors.issue.assetId),
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: senderStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            syncResolver: syncResolver,
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.tokenNonceHex),
                try Self.hex(derivation.changeNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_000 }
        )
        let recipientWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            attestationProvider: StaticAttestationProvider(certificate: recipientCertificate),
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.recipientNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_100 }
        )

        let receiveRequest = try recipientWallet.prepareReceive(
            assetDefinitionId: Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId),
            amount: fixture.chainVectors.redeem.amount
        )
        _ = try senderWallet.pay(receiveRequest)
        _ = try await senderWallet.sync()

        XCTAssertEqual(try senderStore.findNote(noteCommitment: try Self.hex(derivation.sourceNoteCommitment))?.state, .spent)
        let spendableChange = try senderStore.findNote(noteCommitment: try Self.hex(derivation.changeOutputCommitment))
        XCTAssertEqual(spendableChange?.state, .spendable)
        XCTAssertEqual(syncResolver.resolvedCommitments, [])

        syncResolver.resolutions[derivation.changeOutputCommitment] = .redeemed
        let redeeming = try await senderWallet.redeem(try XCTUnwrap(spendableChange))
        XCTAssertEqual(redeeming.state, .redeemPending)

        _ = try await senderWallet.sync()

        XCTAssertEqual(try senderStore.findNote(noteCommitment: try Self.hex(derivation.changeOutputCommitment))?.state, .redeemed)
    }

    func testOfflineNoteWalletRejectsDuplicateTokenAndAlreadyPendingInputs() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let senderStore = InMemoryOfflineNoteStore()
        try senderStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let senderWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: Self.accountId(fromAssetId: fixture.chainVectors.issue.assetId),
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: senderStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.tokenNonceHex),
                try Self.hex(derivation.changeNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_200 }
        )
        let recipientWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            attestationProvider: StaticAttestationProvider(certificate: recipientCertificate),
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.recipientNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_300 }
        )

        let receiveRequest = try recipientWallet.prepareReceive(
            assetDefinitionId: Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId),
            amount: fixture.chainVectors.redeem.amount
        )
        let token = try senderWallet.pay(receiveRequest)

        XCTAssertThrowsError(try senderWallet.pay(receiveRequest))

        let accepted = try recipientWallet.accept(token)
        XCTAssertEqual(accepted.state, .spendable)
        XCTAssertThrowsError(try recipientWallet.accept(token))
    }

    func testOfflineNoteWalletRejectsAdversarialCertificateBindings() throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let senderAccountId = Self.accountId(fromAssetId: fixture.chainVectors.issue.assetId)
        let assetDefinitionId = Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId)

        let defaultRejectingWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            attestationProvider: StaticAttestationProvider(certificate: recipientCertificate),
            proofProvider: BindingProofProvider(),
            randomSource: QueueRandomSource(values: []),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_700 }
        )
        XCTAssertThrowsError(try defaultRejectingWallet.prepareReceive(
            assetDefinitionId: assetDefinitionId,
            amount: fixture.chainVectors.redeem.amount
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .certificateVerificationFailed)
        }
        let wrongAccountReceiveWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: senderAccountId,
            attestationProvider: StaticAttestationProvider(certificate: recipientCertificate),
            proofProvider: BindingProofProvider(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: []),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_710 }
        )
        XCTAssertThrowsError(try wrongAccountReceiveWallet.prepareReceive(
            assetDefinitionId: assetDefinitionId,
            amount: fixture.chainVectors.redeem.amount
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .certificateVerificationFailed)
        }

        let senderStore = InMemoryOfflineNoteStore()
        try senderStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let senderWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: senderAccountId,
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: senderStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.tokenNonceHex),
                try Self.hex(derivation.changeNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { fixture.paymentToken.createdAtMs }
        )
        let recipientWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            attestationProvider: StaticAttestationProvider(certificate: recipientCertificate),
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.recipientNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_800 }
        )
        let receiveRequest = try recipientWallet.prepareReceive(
            assetDefinitionId: assetDefinitionId,
            amount: fixture.chainVectors.redeem.amount
        )
        let accountSubstitution = try OfflineNoteReceiveRequest(
            chainId: receiveRequest.chainId,
            paymentRequestId: receiveRequest.paymentRequestId,
            accountId: senderAccountId,
            assetDefinitionId: receiveRequest.assetDefinitionId,
            assetId: receiveRequest.assetId,
            amount: receiveRequest.amount,
            keyCertificate: receiveRequest.keyCertificate,
            outputCommitment: receiveRequest.outputCommitment
        )
        XCTAssertThrowsError(try senderWallet.pay(accountSubstitution)) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .certificateVerificationFailed)
        }
        let chainSubstitution = try OfflineNoteReceiveRequest(
            chainId: "\(receiveRequest.chainId)-evil",
            paymentRequestId: receiveRequest.paymentRequestId,
            accountId: receiveRequest.accountId,
            assetDefinitionId: receiveRequest.assetDefinitionId,
            assetId: receiveRequest.assetId,
            amount: receiveRequest.amount,
            keyCertificate: receiveRequest.keyCertificate,
            outputCommitment: receiveRequest.outputCommitment
        )
        XCTAssertThrowsError(try senderWallet.pay(chainSubstitution)) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .chainMismatch)
        }
        let assetOwnerSubstitution = try OfflineNoteReceiveRequest(
            chainId: receiveRequest.chainId,
            paymentRequestId: receiveRequest.paymentRequestId,
            accountId: receiveRequest.accountId,
            assetDefinitionId: receiveRequest.assetDefinitionId,
            assetId: "\(receiveRequest.assetDefinitionId)#\(senderAccountId)",
            amount: receiveRequest.amount,
            keyCertificate: receiveRequest.keyCertificate,
            outputCommitment: receiveRequest.outputCommitment
        )
        let assetOwnerSubstitutionStore = InMemoryOfflineNoteStore()
        try assetOwnerSubstitutionStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let assetOwnerSubstitutionSender = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: senderAccountId,
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: assetOwnerSubstitutionStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                Data(repeating: 0x21, count: 32),
                Data(repeating: 0x22, count: 32)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { fixture.paymentToken.createdAtMs + 3 }
        )
        XCTAssertThrowsError(try assetOwnerSubstitutionSender.pay(assetOwnerSubstitution)) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .certificateVerificationFailed)
        }

        let forgedInputStore = InMemoryOfflineNoteStore()
        try forgedInputStore.upsert(try Self.sourceWalletNote(
            fixture,
            certificate: Self.tamperedSignatureCertificate(senderCertificate)
        ))
        let forgedInputWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: senderAccountId,
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: forgedInputStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: []),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_900 }
        )
        XCTAssertThrowsError(try forgedInputWallet.pay(receiveRequest)) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .certificateVerificationFailed)
        }
        let wrongAccountInputStore = InMemoryOfflineNoteStore()
        try wrongAccountInputStore.upsert(try Self.sourceWalletNote(fixture, certificate: recipientCertificate))
        let wrongAccountInputWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: senderAccountId,
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: wrongAccountInputStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: []),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_910 }
        )
        XCTAssertThrowsError(try wrongAccountInputWallet.pay(receiveRequest)) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .certificateVerificationFailed)
        }
        let commitmentSubstitutionStore = InMemoryOfflineNoteStore()
        try commitmentSubstitutionStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let commitmentSubstitutionSender = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: senderAccountId,
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: commitmentSubstitutionStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                Data(repeating: 0x31, count: 32),
                Data(repeating: 0x32, count: 32)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { fixture.paymentToken.createdAtMs + 1 }
        )
        let commitmentSubstitution = try OfflineNoteReceiveRequest(
            chainId: receiveRequest.chainId,
            paymentRequestId: receiveRequest.paymentRequestId,
            accountId: receiveRequest.accountId,
            assetDefinitionId: receiveRequest.assetDefinitionId,
            assetId: receiveRequest.assetId,
            amount: receiveRequest.amount,
            keyCertificate: receiveRequest.keyCertificate,
            outputCommitment: Data(repeating: 0xA5, count: 32)
        )
        XCTAssertThrowsError(try recipientWallet.accept(
            commitmentSubstitutionSender.pay(commitmentSubstitution)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .noPendingOutput)
        }
        let forgedOutputAmount = receiveRequest.amount == "1" ? "2" : "1"
        let amountSubstitutionStore = InMemoryOfflineNoteStore()
        try amountSubstitutionStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let amountSubstitutionSender = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: senderAccountId,
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: amountSubstitutionStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                Data(repeating: 0x41, count: 32),
                Data(repeating: 0x42, count: 32)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { fixture.paymentToken.createdAtMs + 2 }
        )
        let amountSubstitution = try OfflineNoteReceiveRequest(
            chainId: receiveRequest.chainId,
            paymentRequestId: receiveRequest.paymentRequestId,
            accountId: receiveRequest.accountId,
            assetDefinitionId: receiveRequest.assetDefinitionId,
            assetId: receiveRequest.assetId,
            amount: forgedOutputAmount,
            keyCertificate: receiveRequest.keyCertificate,
            outputCommitment: receiveRequest.outputCommitment
        )
        XCTAssertThrowsError(try recipientWallet.accept(
            amountSubstitutionSender.pay(amountSubstitution)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .outputMismatch)
        }

        let token = try senderWallet.pay(receiveRequest)
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingChainId(token, chainId: "\(token.chainId)-evil")
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .chainMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingPaymentRequestId(token, paymentRequestId: "\(token.paymentRequestId)-evil")
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .outputMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingTopLevelTokenId(token)
        )) { error in
            XCTAssertEqual(error as? OfflineNotePaymentTokenCodecError, .tokenIdMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingAuditTokenId(token)
        )) { error in
            XCTAssertEqual(error as? OfflineNotePaymentTokenCodecError, .tokenIdMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingFirstOutputAmountWithoutProofRebind(token, amount: forgedOutputAmount)
        )) { error in
            guard case .proofPublicInputsHashMismatch = error as? OfflineNoteError else {
                XCTFail("expected proof public input mismatch, got \(error)")
                return
            }
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingFirstOutputAmount(token, amount: forgedOutputAmount)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .outputMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingFirstOutputAsset(
                token,
                assetId: "\(receiveRequest.assetId)#dataspace:1"
            )
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .outputMismatch)
        }
        XCTAssertGreaterThanOrEqual(token.audit.outputClaims.count, 2)
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReversingOutputs(token)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .outputMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenDroppingFirstOutput(token)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .noPendingOutput)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingFirstOutputCertificate(token, certificate: senderCertificate)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .certificateVerificationFailed)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingLastOutputCertificate(token, certificate: recipientCertificate)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .certificateVerificationFailed)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingFirstInputClaimHash(
                token,
                keyCertificatePayloadHash: try recipientCertificate.payloadHash()
            )
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .certificateVerificationFailed)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingSenderCertificate(token, certificate: recipientCertificate)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .certificateVerificationFailed)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingBearerAuditTrail(token, bearerAuditTrail: [])
        ))
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingBearerAuditTrail(
                token,
                bearerAuditTrail: [
                    try Self.auditReplacingTokenIdWithoutProofRebind(token.audit),
                ]
            )
        ))
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingBearerAuditTrail(
                token,
                bearerAuditTrail: [
                    token.audit,
                    token.audit,
                ]
            )
        ))
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingBearerAuditTrail(
                token,
                bearerAuditTrail: [
                    try Self.auditReplacingFirstOutputAmountWithoutProofRebind(token.audit, amount: forgedOutputAmount),
                    token.audit,
                ]
            )
        ))
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingBearerAuditTrail(
                token,
                bearerAuditTrail: [
                    try Self.selfConsumingAudit(token.audit),
                    token.audit,
                ]
            )
        ))

        let accepted = try recipientWallet.accept(token)
        XCTAssertEqual(accepted.state, .spendable)
        XCTAssertEqual(accepted.bearerAuditTrail.map(\.tokenId), [token.tokenId])
    }

    func testOfflineNoteWalletRedeemDefundsP2pBearerWithAuditTrail() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let senderStore = InMemoryOfflineNoteStore()
        try senderStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let senderWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: Self.accountId(fromAssetId: fixture.chainVectors.issue.assetId),
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: senderStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.tokenNonceHex),
                try Self.hex(derivation.changeNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_700 }
        )
        let recipientStore = InMemoryOfflineNoteStore()
        let recipientSubmitter = RecordingTransactionSubmitter()
        let recipientWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            attestationProvider: StaticAttestationProvider(certificate: recipientCertificate),
            store: recipientStore,
            transactionSubmitter: recipientSubmitter,
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.recipientNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_800 }
        )

        let receiveRequest = try recipientWallet.prepareReceive(
            assetDefinitionId: Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId),
            amount: fixture.chainVectors.redeem.amount
        )
        let token = try senderWallet.pay(receiveRequest)
        let accepted = try recipientWallet.accept(token)

        _ = try await recipientWallet.redeem(accepted)

        XCTAssertEqual(recipientSubmitter.defunds.count, 1)
        let defund = try XCTUnwrap(recipientSubmitter.defunds.first)
        XCTAssertEqual(defund.bearerAuditTrail.map(\.tokenId), [token.tokenId])
        XCTAssertEqual(defund.redemption.sourceNoteCommitment, accepted.noteCommitment)
        XCTAssertTrue(recipientSubmitter.redemptions.isEmpty)
    }

    func testOfflineNoteWalletRejectsP2pRedeemWithoutBearerAuditTrail() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let recipientStore = InMemoryOfflineNoteStore()
        let note = try OfflineNoteWalletNote(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            assetId: fixture.chainVectors.redeem.assetId,
            amount: fixture.chainVectors.redeem.amount,
            keyCertificate: recipientCertificate,
            noteCommitment: try Self.hex(derivation.recipientOutputCommitment),
            noteSecret: try Self.hex(derivation.recipientNoteSecretHex),
            origin: .p2pOutput(OfflineNoteP2pOutputOrigin(
                paymentRequestId: derivation.paymentRequestId,
                outputIndex: 0
            )),
            state: .spendable,
            createdAtMs: 1_700_000_002_900,
            updatedAtMs: 1_700_000_002_900
        )
        try recipientStore.upsert(note)
        let submitter = RecordingTransactionSubmitter()
        let wallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            attestationProvider: StaticAttestationProvider(certificate: recipientCertificate),
            store: recipientStore,
            transactionSubmitter: submitter,
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: []),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_003_000 }
        )

        await XCTAssertThrowsErrorAsync(try await wallet.redeem(note)) { error in
            XCTAssertEqual(error as? OfflineNoteWalletError, .missingBearerAuditTrail)
        }
        XCTAssertTrue(submitter.defunds.isEmpty)
        XCTAssertTrue(submitter.redemptions.isEmpty)
    }

    func testOfflineNoteWalletSyncReconcilesFailedAuditAndRedeemOutcomes() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let senderStore = InMemoryOfflineNoteStore()
        try senderStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let senderWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: Self.accountId(fromAssetId: fixture.chainVectors.issue.assetId),
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: senderStore,
            transactionSubmitter: RecordingTransactionSubmitter(),
            syncResolver: RecordingSyncResolver(resolutions: [
                derivation.sourceNoteCommitment: .spendable,
                derivation.changeOutputCommitment: .cancelled
            ]),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.tokenNonceHex),
                try Self.hex(derivation.changeNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_400 }
        )
        let recipientStore = InMemoryOfflineNoteStore()
        let recipientWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            attestationProvider: StaticAttestationProvider(certificate: recipientCertificate),
            store: recipientStore,
            transactionSubmitter: FailingTransactionSubmitter(),
            syncResolver: RecordingSyncResolver(resolutions: [
                derivation.recipientOutputCommitment: .cancelled
            ]),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: [
                try Self.hex(derivation.recipientNoteSecretHex)
            ]),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_500 }
        )

        let receiveRequest = try recipientWallet.prepareReceive(
            assetDefinitionId: Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId),
            amount: fixture.chainVectors.redeem.amount
        )
        let token = try senderWallet.pay(receiveRequest)

        let accepted = try recipientWallet.accept(token)
        XCTAssertEqual(accepted.state, .spendable)
        await XCTAssertThrowsErrorAsync(try await recipientWallet.publishAudit(token)) { _ in }
        XCTAssertEqual(
            try recipientStore.findNote(noteCommitment: try Self.hex(derivation.recipientOutputCommitment))?.state,
            .spendable
        )

        _ = try await senderWallet.sync()
        _ = try await recipientWallet.sync()

        XCTAssertEqual(try senderStore.findNote(noteCommitment: try Self.hex(derivation.sourceNoteCommitment))?.state, .spent)
        XCTAssertEqual(try senderStore.findNote(noteCommitment: try Self.hex(derivation.changeOutputCommitment))?.state, .spendable)
        XCTAssertEqual(
            try recipientStore.findNote(noteCommitment: try Self.hex(derivation.recipientOutputCommitment))?.state,
            .spendable
        )

        let redeemStore = InMemoryOfflineNoteStore()
        let redeemNote = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        try redeemStore.upsert(redeemNote)
        let redeemWallet = OfflineNoteWallet(
            chainId: derivation.chainId,
            accountId: Self.accountId(fromAssetId: fixture.chainVectors.issue.assetId),
            attestationProvider: StaticAttestationProvider(certificate: senderCertificate),
            store: redeemStore,
            transactionSubmitter: FailingTransactionSubmitter(),
            syncResolver: RecordingSyncResolver(resolutions: [
                derivation.sourceNoteCommitment: .spendable
            ]),
            proofProvider: BindingProofProvider(),
            proofVerifier: BindingProofVerifier(),
            certificateVerifier: try Self.certificateVerifier(fixture),
            randomSource: QueueRandomSource(values: []),
            idGenerator: FixedIdGenerator(id: derivation.paymentRequestId),
            clock: { 1_700_000_002_600 }
        )

        await XCTAssertThrowsErrorAsync(try await redeemWallet.redeem(redeemNote)) { _ in }
        XCTAssertEqual(try redeemStore.findNote(noteCommitment: try Self.hex(derivation.sourceNoteCommitment))?.state, .spendable)

        _ = try await redeemWallet.sync()

        XCTAssertEqual(try redeemStore.findNote(noteCommitment: try Self.hex(derivation.sourceNoteCommitment))?.state, .spendable)
    }

    func testOfflineNoteOutcomeIndexResolvesCommittedAndRejectedExplorerInstructions() throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)
        let redeemPending = try OfflineNoteWalletNote(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            assetId: fixture.chainVectors.redeem.assetId,
            amount: fixture.chainVectors.redeem.amount,
            keyCertificate: recipientCertificate,
            noteCommitment: redeem.sourceNoteCommitment,
            noteSecret: Self.hex(derivation.recipientNoteSecretHex),
            origin: .p2pOutput(OfflineNoteP2pOutputOrigin(
                paymentRequestId: derivation.paymentRequestId,
                outputIndex: 0
            )),
            state: .redeemPending,
            createdAtMs: 1_700_000_002_000,
            updatedAtMs: 1_700_000_003_000
        )

        let committed = try OfflineNoteOutcomeIndex.fromExplorerOutcomes([
            OfflineNoteExplorerInstructionOutcome(
                kind: OfflineNoteOutcomeIndex.kindAudit,
                transactionStatus: "Committed",
                transactionHashHex: "audit-tx",
                encodedInstruction: try Self.auditInstructionEnvelope(audit)
            ),
            OfflineNoteExplorerInstructionOutcome(
                kind: OfflineNoteOutcomeIndex.kindRedeem,
                transactionStatus: "Committed",
                transactionHashHex: "redeem-tx",
                encodedInstruction: try Self.redeemInstructionEnvelope(redeem)
            ),
        ])

        XCTAssertEqual(try committed.resolve(redeemPending), OfflineNoteSyncResolution(
            state: .redeemed,
            transactionHashHex: "redeem-tx"
        ))

        let rejected = OfflineNoteOutcomeIndex()
            .recordRejectedAudit(audit, transactionHashHex: "audit-rejected")
            .recordRejectedRedeem(redeem, transactionHashHex: "redeem-rejected")

        XCTAssertEqual(try rejected.resolve(redeemPending), OfflineNoteSyncResolution(
            state: .spendable,
            transactionHashHex: "redeem-rejected"
        ))
    }

    func testOfflineNoteTransactionBuildersProduceSignedEnvelopes() throws {
        let fixture = try Self.loadFixture()
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let chainId = "00000000-0000-0000-0000-000000000000"
        let creationTimeMs: UInt64 = 1_706_000_000_000

        let issue = try SwiftTransactionEncoder.encodeIssueOfflineNote(
            request: IssueOfflineNoteRequest(
                chainId: chainId,
                authority: authority,
                issue: Self.issue(fixture),
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )
        let audit = try SwiftTransactionEncoder.encodeAuditOfflineNote(
            request: AuditOfflineNoteRequest(
                chainId: chainId,
                authority: authority,
                audit: Self.audit(fixture),
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )
        let redeem = try SwiftTransactionEncoder.encodeRedeemOfflineNote(
            request: RedeemOfflineNoteRequest(
                chainId: chainId,
                authority: authority,
                redemption: Self.redeem(fixture),
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )
        let defund = try SwiftTransactionEncoder.encodeDefundOfflineNote(
            request: DefundOfflineNoteRequest(
                chainId: chainId,
                authority: authority,
                bearerAuditTrail: [Self.audit(fixture)],
                redemption: Self.redeem(fixture),
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )

        for envelope in [issue, audit, redeem, defund] {
            XCTAssertEqual(envelope.norito.first, 1)
            XCTAssertEqual(Data(envelope.norito.dropFirst()), envelope.signedTransaction)
            XCTAssertEqual(envelope.transactionHash.count, 32)
            XCTAssertNil(envelope.payload)
        }
        XCTAssertEqual(try Self.instructionCount(in: issue), 1)
        XCTAssertEqual(try Self.instructionCount(in: audit), 1)
        XCTAssertEqual(try Self.instructionCount(in: redeem), 1)
        XCTAssertEqual(try Self.instructionCount(in: defund), 2)
        XCTAssertNotEqual(issue.transactionHash, audit.transactionHash)
        XCTAssertNotEqual(audit.transactionHash, redeem.transactionHash)
        XCTAssertNotEqual(redeem.transactionHash, defund.transactionHash)
    }

    func testRedeemBuilderRejectsMismatchedProofBinding() throws {
        let fixture = try Self.loadFixture()
        let redeem = try Self.redeem(fixture)
        let badProof = try OfflineNoteRecursiveProof(
            publicInputsHash: IrohaHash.hash(Data("wrong-public-inputs".utf8)),
            proofBytes: Data("offline-vector-redeem-proof".utf8)
        )
        let forged = try OfflineNoteRedeem(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: badProof
        )
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        let authority = AccountId.make(publicKey: keypair.publicKey)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeRedeemOfflineNote(
                request: RedeemOfflineNoteRequest(
                    chainId: "00000000-0000-0000-0000-000000000000",
                    authority: authority,
                    redemption: forged
                ),
                keypair: keypair,
                creationTimeMs: 1
            )
        ) { error in
            guard case OfflineNoteError.proofPublicInputsHashMismatch = error else {
                return XCTFail("expected proofPublicInputsHashMismatch, got \(error)")
            }
        }
    }

    func testOfflineNoteProofAndHashValidationRejectsMalformedValues() throws {
        let fixture = try Self.loadFixture()
        let publicInputsHash = try Self.hex(fixture.chainVectors.audit.publicInputsHash)

        let trimmedProof = try OfflineNoteProofBox(
            backend: "  \(OfflineNoteConstants.recursiveBackend)  ",
            bytes: Data([0x01])
        )
        XCTAssertEqual(trimmedProof.backend, OfflineNoteConstants.recursiveBackend)

        XCTAssertThrowsError(try OfflineNoteProofBox(backend: " \n ", bytes: Data([0x01]))) { error in
            XCTAssertEqual(error as? OfflineNoteError, .emptyProofBackend)
        }
        XCTAssertThrowsError(try OfflineNoteProofBox(backend: "halo2/ipa", bytes: Data())) { error in
            XCTAssertEqual(error as? OfflineNoteError, .emptyProofBytes)
        }
        XCTAssertThrowsError(try OfflineNoteRecursiveProof(
            publicInputsHash: Data(repeating: 0x01, count: 31),
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .invalidHashLength(field: "public_inputs_hash", expected: 32, actual: 31)
            )
        }

        var nonCanonicalHash = publicInputsHash
        nonCanonicalHash[31] &= 0xfe
        XCTAssertThrowsError(try OfflineNoteRecursiveProof(
            publicInputsHash: nonCanonicalHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? OfflineNoteError, .invalidHash(field: "public_inputs_hash"))
        }
    }

    func testOfflineNoteCertificateValidationRejectsMalformedValues() throws {
        let fixture = try Self.loadFixture()
        let cert = fixture.paymentToken.senderKeyCertificate
        let publicKey = try Self.base64(cert.publicKey)
        let assertionPublicKey = try Self.base64(cert.assertionPublicKey)
        let issuerSignature = try Self.base64(cert.issuerSignatureBase64)

        XCTAssertThrowsError(try OfflineNoteKeyCertificate(
            version: 2,
            platform: cert.platform,
            keyId: cert.keyId,
            deviceId: cert.deviceId,
            accountId: cert.accountId,
            publicKey: publicKey,
            assertionScheme: cert.assertionScheme,
            assertionKeyAlgorithm: cert.assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: cert.assertionUsageCountLimit,
            oneUse: true,
            issuerSignature: issuerSignature
        )) { error in
            XCTAssertEqual(error as? OfflineNoteError, .invalidCertificateVersion(2))
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificate(
            platform: cert.platform,
            keyId: cert.keyId,
            deviceId: cert.deviceId,
            accountId: cert.accountId,
            publicKey: publicKey,
            assertionScheme: cert.assertionScheme,
            assertionKeyAlgorithm: cert.assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: cert.assertionUsageCountLimit,
            oneUse: false,
            issuerSignature: issuerSignature
        )) { error in
            XCTAssertEqual(error as? OfflineNoteError, .certificateMustBeOneUse)
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificate(
            platform: cert.platform,
            keyId: cert.keyId,
            deviceId: cert.deviceId,
            accountId: cert.accountId,
            publicKey: Data(publicKey.dropLast()),
            assertionScheme: cert.assertionScheme,
            assertionKeyAlgorithm: cert.assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: cert.assertionUsageCountLimit,
            oneUse: true,
            issuerSignature: issuerSignature
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .invalidNotePublicKeyLength(expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificate(
            platform: cert.platform,
            keyId: cert.keyId,
            deviceId: cert.deviceId,
            accountId: cert.accountId,
            publicKey: publicKey,
            assertionScheme: cert.assertionScheme,
            assertionKeyAlgorithm: cert.assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: cert.assertionUsageCountLimit,
            oneUse: true,
            issuerSignature: Data(issuerSignature.dropLast())
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .invalidIssuerSignatureLength(expected: 64, actual: 63)
            )
        }

        XCTAssertNoThrow(try OfflineNoteKeyCertificate(
            platform: cert.platform,
            keyId: cert.keyId,
            deviceId: cert.deviceId,
            accountId: cert.accountId,
            publicKey: publicKey,
            assertionScheme: cert.assertionScheme,
            assertionKeyAlgorithm: cert.assertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKey,
            assertionUsageCountLimit: cert.assertionUsageCountLimit,
            oneUse: true,
            issuerSignature: issuerSignature
        ))
    }

    func testOfflineNoteAuditBundleRejectsInvalidShapes() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)

        XCTAssertThrowsError(try OfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: [],
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteError, .emptyInputNullifiers)
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: [],
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteError, .emptyInputClaims)
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers + [audit.inputNullifiers[0]],
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .auditInputCountMismatch(nullifiers: audit.inputNullifiers.count + 1, claims: audit.inputClaims.count)
            )
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: [],
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteError, .emptyOutputCommitments)
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: [],
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteError, .emptyOutputClaims)
        }

        let uncommittedClaim = try OfflineNoteAuditOutputClaim(
            noteCommitment: Data(repeating: 0x03, count: 32),
            keyCertificate: audit.outputClaims[0].keyCertificate,
            assetId: audit.outputClaims[0].assetId,
            amount: audit.outputClaims[0].amount
        )
        XCTAssertThrowsError(try OfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: [uncommittedClaim],
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .auditOutputClaimNotCommitted(uncommittedClaim.noteCommitment.hexLowercased())
            )
        }
    }

    func testOfflineNoteIssueAndClaimValidationCoversDerivedClaimAndFailures() throws {
        let fixture = try Self.loadFixture()
        let certificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let noteCommitment = try Self.hex(fixture.chainVectors.issue.noteCommitment)
        let issue = try OfflineNoteIssue(
            noteCommitment: noteCommitment,
            keyCertificate: certificate,
            assetId: fixture.chainVectors.issue.assetId,
            amount: "5.5000"
        )

        XCTAssertEqual(issue.amount, "5.5000")
        let claim = try issue.issuedClaim()
        XCTAssertEqual(claim.domain, OfflineNoteConstants.issuedClaimDomain)
        XCTAssertEqual(claim.noteCommitment, issue.noteCommitment)
        XCTAssertEqual(claim.keyCertificatePayloadHash, try certificate.payloadHash())
        XCTAssertEqual(claim.assetId, issue.assetId)
        XCTAssertEqual(claim.amount, "5.5000")
        XCTAssertEqual(try claim.claimHash().count, 32)

        XCTAssertThrowsError(try OfflineNoteIssue(
            noteCommitment: Data(repeating: 0x01, count: 31),
            keyCertificate: certificate,
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .invalidHashLength(field: "note_commitment", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteIssue(
            noteCommitment: noteCommitment,
            keyCertificate: certificate,
            assetId: "cash#branch.sbp",
            amount: fixture.chainVectors.issue.amount
        )) { error in
            guard case OfflineNoritoError.invalidAssetId("cash#branch.sbp") = error else {
                return XCTFail("expected invalidAssetId, got \(error)")
            }
        }
        XCTAssertThrowsError(try OfflineNoteIssue(
            noteCommitment: noteCommitment,
            keyCertificate: certificate,
            assetId: fixture.chainVectors.issue.assetId,
            amount: "not-a-number"
        )) { error in
            guard case OfflineNoritoError.invalidNumeric("not-a-number") = error else {
                return XCTFail("expected invalidNumeric, got \(error)")
            }
        }
    }

    func testOfflineNoteRedeemValidationRejectsBadInputsAndDerivesIssuedClaim() throws {
        let fixture = try Self.loadFixture()
        let redeem = try Self.redeem(fixture)
        let issuedClaim = try redeem.issuedClaim()

        XCTAssertEqual(issuedClaim.noteCommitment, redeem.sourceNoteCommitment)
        XCTAssertEqual(issuedClaim.keyCertificatePayloadHash, try redeem.senderKeyCertificate.payloadHash())
        XCTAssertEqual(issuedClaim.assetId, redeem.assetId)
        XCTAssertEqual(issuedClaim.amount, redeem.amount)

        XCTAssertThrowsError(try OfflineNoteRedeem(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: [],
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteError, .emptyInputNullifiers)
        }
        XCTAssertThrowsError(try OfflineNoteRedeem(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: [Data(repeating: 0x01, count: 31)],
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .invalidHashLength(field: "input_nullifiers[0]", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteRedeem(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: "\(redeem.recipient)@bad",
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        ))
    }

    func testOfflineNoteAuditValidateProofBindingReportsExpectedAndActualHashes() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        var wrongPublicInputsHash = try audit.publicInputsHash()
        wrongPublicInputsHash[0] ^= 0x01
        let forgedProof = try OfflineNoteRecursiveProof(
            publicInputsHash: wrongPublicInputsHash,
            proofBytes: audit.recursiveProof.proof.bytes
        )
        let forgedAudit = try OfflineNoteAuditBundle(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: forgedProof
        )

        XCTAssertThrowsError(try forgedAudit.validateProofBinding()) { error in
            guard case let OfflineNoteError.proofPublicInputsHashMismatch(expected, actual) = error else {
                return XCTFail("expected proofPublicInputsHashMismatch, got \(error)")
            }
            XCTAssertEqual(expected, try? audit.publicInputsHash().hexLowercased())
            XCTAssertEqual(actual, forgedProof.publicInputsHash.hexLowercased())
        }
    }

    func testOfflineNoteTransactionBuilderCoversOptionalNonceAndInputValidation() throws {
        let fixture = try Self.loadFixture()
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let chainId = "00000000-0000-0000-0000-000000000000"
        let issue = try Self.issue(fixture)

        let defaultEnvelope = try SwiftTransactionEncoder.encodeIssueOfflineNote(
            request: IssueOfflineNoteRequest(chainId: chainId, authority: authority, issue: issue),
            keypair: keypair,
            creationTimeMs: 1_706_000_000_000
        )
        let nonceEnvelope = try SwiftTransactionEncoder.encodeIssueOfflineNote(
            request: IssueOfflineNoteRequest(
                chainId: "  \(chainId)  ",
                authority: "  \(authority)  ",
                issue: issue,
                ttlMs: nil,
                nonce: 42
            ),
            keypair: keypair,
            creationTimeMs: 1_706_000_000_000
        )

        XCTAssertNotEqual(defaultEnvelope.signedTransaction, nonceEnvelope.signedTransaction)
        XCTAssertNotEqual(defaultEnvelope.transactionHash, nonceEnvelope.transactionHash)

        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeIssueOfflineNote(
            request: IssueOfflineNoteRequest(chainId: " \n ", authority: authority, issue: issue),
            keypair: keypair,
            creationTimeMs: 1
        )) { error in
            XCTAssertEqual(error as? TransactionInputError, .emptyChainId)
        }
        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeIssueOfflineNote(
            request: IssueOfflineNoteRequest(chainId: chainId, authority: "\(authority)@bad", issue: issue),
            keypair: keypair,
            creationTimeMs: 1
        )) { error in
            XCTAssertEqual(
                error as? TransactionInputError,
                .malformedAccountId(field: "authority", value: "\(authority)@bad")
            )
        }
    }

    func testOfflineNoteRecursiveProofCoversCustomVerifierAndVerifierValidation() throws {
        let publicInputsHash = try Self.audit(Self.loadFixture()).publicInputsHash()
        let proof = try OfflineNoteRecursiveProof(
            verifierBackend: "custom_backend",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01, 0x02, 0x03]),
            proofBackend: " custom_proof_backend "
        )

        XCTAssertEqual(proof.verifierKeyId.backend, "custom_backend")
        XCTAssertEqual(proof.verifierKeyId.name, "custom_vk")
        XCTAssertEqual(proof.proof.backend, "custom_proof_backend")
        XCTAssertEqual(proof.proof.bytes, Data([0x01, 0x02, 0x03]))

        XCTAssertThrowsError(try OfflineNoteRecursiveProof(
            verifierBackend: "",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .emptyBackend)
        }
        XCTAssertThrowsError(try OfflineNoteRecursiveProof(
            verifierBackend: "custom_backend",
            verifierName: "",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .emptyName)
        }
        XCTAssertThrowsError(try OfflineNoteRecursiveProof(
            verifierBackend: "halo2:ipa",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .invalidSeparator)
        }
    }

    func testOfflineNoteCertificatePayloadValidationAndEncodingBranches() throws {
        let certificate = try Self.certificate(Self.loadFixture().paymentToken.senderKeyCertificate)
        let payload = try certificate.signingPayload()

        XCTAssertEqual(payload.domain, OfflineNoteConstants.keyCertificatePayloadDomain)
        XCTAssertEqual(payload.version, certificate.version)
        XCTAssertEqual(payload.publicKey, certificate.publicKey)
        XCTAssertEqual(payload.oneUse, true)
        XCTAssertNotEqual(try payload.noritoEncoded(), try certificate.noritoEncoded())

        let noLimitPayload = try OfflineNoteKeyCertificatePayload(
            version: 1,
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: nil,
            oneUse: true
        )
        let limitedPayload = try OfflineNoteKeyCertificatePayload(
            version: 1,
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: 7,
            oneUse: true
        )
        XCTAssertNil(noLimitPayload.assertionUsageCountLimit)
        XCTAssertEqual(limitedPayload.assertionUsageCountLimit, 7)
        XCTAssertNotEqual(try noLimitPayload.noritoEncoded(), try limitedPayload.noritoEncoded())

        XCTAssertThrowsError(try OfflineNoteKeyCertificatePayload(
            version: 1,
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: "\(certificate.accountId)@bad",
            publicKey: certificate.publicKey,
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: certificate.assertionUsageCountLimit,
            oneUse: true
        ))
        XCTAssertThrowsError(try OfflineNoteKeyCertificatePayload(
            version: 1,
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: Data(certificate.publicKey.dropLast()),
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: certificate.assertionUsageCountLimit,
            oneUse: true
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .invalidNotePublicKeyLength(expected: 32, actual: 31)
            )
        }
    }

    func testOfflineNotePublicInputConstructorsRejectMalformedInputs() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)
        let auditOutputClaims = try audit.outputClaims.map(OfflineNoteIssuedClaim.fromAuditOutput)

        XCTAssertThrowsError(try OfflineNoteRedeemPublicInputs(
            sourceNoteCommitment: Data(repeating: 0x01, count: 31),
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: try redeem.senderKeyCertificate.payloadHash(),
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .invalidHashLength(field: "source_note_commitment", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteRedeemPublicInputs(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: Data(repeating: 0x01, count: 31),
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .invalidHashLength(field: "key_certificate_payload_hash", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteRedeemPublicInputs(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: try redeem.senderKeyCertificate.payloadHash(),
            recipient: "\(redeem.recipient)@bad",
            assetId: redeem.assetId,
            amount: redeem.amount
        ))

        XCTAssertThrowsError(try OfflineNoteAuditPublicInputs(
            tokenId: Data(repeating: 0x01, count: 31),
            keyCertificatePayloadHash: try audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: auditOutputClaims
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteError,
                .invalidHashLength(field: "token_id", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteAuditPublicInputs(
            tokenId: audit.tokenId,
            keyCertificatePayloadHash: try audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: []
        )) { error in
            XCTAssertEqual(error as? OfflineNoteError, .emptyOutputClaims)
        }
    }

    private static func issue(_ fixture: OfflineInteropFixture) throws -> OfflineNoteIssue {
        try OfflineNoteIssue(
            noteCommitment: hex(fixture.chainVectors.issue.noteCommitment),
            keyCertificate: certificate(fixture.paymentToken.senderKeyCertificate),
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount
        )
    }

    private static func redeem(_ fixture: OfflineInteropFixture) throws -> OfflineNoteRedeem {
        let vector = fixture.chainVectors.redeem
        return try OfflineNoteRedeem(
            sourceNoteCommitment: hex(vector.sourceNoteCommitment),
            inputNullifiers: try vector.inputNullifiers.map(hex),
            senderKeyCertificate: certificate(fixture.paymentToken.recipientKeyCertificate),
            recipient: fixture.paymentToken.recipientAccountId,
            assetId: vector.assetId,
            amount: vector.amount,
            recursiveProof: OfflineNoteRecursiveProof(
                publicInputsHash: hex(vector.publicInputsHash),
                proofBytes: Data("offline-vector-redeem-proof".utf8)
            )
        )
    }

    private static func audit(_ fixture: OfflineInteropFixture) throws -> OfflineNoteAuditBundle {
        let vector = fixture.chainVectors.audit
        return try OfflineNoteAuditBundle(
            tokenId: hex(vector.tokenId),
            senderKeyCertificate: certificate(fixture.paymentToken.senderKeyCertificate),
            inputNullifiers: try vector.inputNullifiers.map(hex),
            inputClaims: try fixture.paymentToken.inputClaims.map(issuedClaim),
            outputCommitments: try vector.outputCommitments.map(hex),
            outputClaims: try fixture.paymentToken.outputClaims.map(auditOutputClaim),
            recursiveProof: OfflineNoteRecursiveProof(
                publicInputsHash: hex(vector.publicInputsHash),
                proofBytes: Data("offline-vector-audit-proof".utf8)
            )
        )
    }

    private static func certificate(_ json: OfflineCertificateJSON) throws -> OfflineNoteKeyCertificate {
        try OfflineNoteKeyCertificate(
            version: json.version,
            platform: json.platform,
            keyId: json.keyId,
            deviceId: json.deviceId,
            accountId: json.accountId,
            publicKey: base64(json.publicKey),
            assertionScheme: json.assertionScheme,
            assertionKeyAlgorithm: json.assertionKeyAlgorithm,
            assertionPublicKey: base64(json.assertionPublicKey),
            assertionUsageCountLimit: json.assertionUsageCountLimit,
            oneUse: json.oneUse,
            issuerSignature: base64(json.issuerSignatureBase64)
        )
    }

    private static func certificateVerifier(_ fixture: OfflineInteropFixture) throws -> Ed25519OfflineNoteCertificateVerifier {
        Ed25519OfflineNoteCertificateVerifier(
            trustedIssuerPublicKeys: [try base64(fixture.offlineFiPublicKeyBase64)]
        )
    }

    private static func tamperedSignatureCertificate(
        _ certificate: OfflineNoteKeyCertificate
    ) throws -> OfflineNoteKeyCertificate {
        var signature = certificate.issuerSignature
        signature[signature.startIndex] ^= 0x01
        return try OfflineNoteKeyCertificate(
            version: certificate.version,
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey,
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey,
            assertionUsageCountLimit: certificate.assertionUsageCountLimit,
            oneUse: certificate.oneUse,
            issuerSignature: signature
        )
    }

    private static func paymentTokenReplacingFirstOutputCertificate(
        _ token: OfflineNotePaymentToken,
        certificate: OfflineNoteKeyCertificate
    ) throws -> OfflineNotePaymentToken {
        var outputClaims = token.audit.outputClaims
        let output = try XCTUnwrap(outputClaims.first)
        outputClaims[0] = try OfflineNoteAuditOutputClaim(
            noteCommitment: output.noteCommitment,
            keyCertificate: certificate,
            assetId: output.assetId,
            amount: output.amount
        )
        return try paymentTokenReplacingAuditClaims(token, inputClaims: token.audit.inputClaims, outputClaims: outputClaims)
    }

    private static func paymentTokenReplacingFirstOutputAmount(
        _ token: OfflineNotePaymentToken,
        amount: String
    ) throws -> OfflineNotePaymentToken {
        var outputClaims = token.audit.outputClaims
        let output = try XCTUnwrap(outputClaims.first)
        outputClaims[0] = try OfflineNoteAuditOutputClaim(
            noteCommitment: output.noteCommitment,
            keyCertificate: output.keyCertificate,
            assetId: output.assetId,
            amount: amount
        )
        return try paymentTokenReplacingAuditClaims(token, inputClaims: token.audit.inputClaims, outputClaims: outputClaims)
    }

    private static func paymentTokenReplacingFirstOutputAmountWithoutProofRebind(
        _ token: OfflineNotePaymentToken,
        amount: String
    ) throws -> OfflineNotePaymentToken {
        var outputClaims = token.audit.outputClaims
        let output = try XCTUnwrap(outputClaims.first)
        outputClaims[0] = try OfflineNoteAuditOutputClaim(
            noteCommitment: output.noteCommitment,
            keyCertificate: output.keyCertificate,
            assetId: output.assetId,
            amount: amount
        )
        return OfflineNotePaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: token.tokenId,
            audit: try OfflineNoteAuditBundle(
                tokenId: token.audit.tokenId,
                senderKeyCertificate: token.audit.senderKeyCertificate,
                inputNullifiers: token.audit.inputNullifiers,
                inputClaims: token.audit.inputClaims,
                outputCommitments: token.audit.outputCommitments,
                outputClaims: outputClaims,
                recursiveProof: token.audit.recursiveProof
            ),
            createdAtMs: token.createdAtMs
        )
    }

    private static func paymentTokenReplacingFirstOutputAsset(
        _ token: OfflineNotePaymentToken,
        assetId: String
    ) throws -> OfflineNotePaymentToken {
        var outputClaims = token.audit.outputClaims
        let output = try XCTUnwrap(outputClaims.first)
        outputClaims[0] = try OfflineNoteAuditOutputClaim(
            noteCommitment: output.noteCommitment,
            keyCertificate: output.keyCertificate,
            assetId: assetId,
            amount: output.amount
        )
        return try paymentTokenReplacingAuditClaims(token, inputClaims: token.audit.inputClaims, outputClaims: outputClaims)
    }

    private static func paymentTokenReversingOutputs(
        _ token: OfflineNotePaymentToken
    ) throws -> OfflineNotePaymentToken {
        try paymentTokenReplacingOutputs(
            token,
            outputClaims: Array(token.audit.outputClaims.reversed()),
            outputCommitments: Array(token.audit.outputCommitments.reversed())
        )
    }

    private static func paymentTokenDroppingFirstOutput(
        _ token: OfflineNotePaymentToken
    ) throws -> OfflineNotePaymentToken {
        try paymentTokenReplacingOutputs(
            token,
            outputClaims: Array(token.audit.outputClaims.dropFirst()),
            outputCommitments: Array(token.audit.outputCommitments.dropFirst())
        )
    }

    private static func paymentTokenReplacingChainId(
        _ token: OfflineNotePaymentToken,
        chainId: String
    ) -> OfflineNotePaymentToken {
        OfflineNotePaymentToken(
            chainId: chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: token.tokenId,
            audit: token.audit,
            createdAtMs: token.createdAtMs
        )
    }

    private static func paymentTokenReplacingLastOutputCertificate(
        _ token: OfflineNotePaymentToken,
        certificate: OfflineNoteKeyCertificate
    ) throws -> OfflineNotePaymentToken {
        var outputClaims = token.audit.outputClaims
        let output = try XCTUnwrap(outputClaims.last)
        outputClaims[outputClaims.count - 1] = try OfflineNoteAuditOutputClaim(
            noteCommitment: output.noteCommitment,
            keyCertificate: certificate,
            assetId: output.assetId,
            amount: output.amount
        )
        return try paymentTokenReplacingAuditClaims(token, inputClaims: token.audit.inputClaims, outputClaims: outputClaims)
    }

    private static func paymentTokenReplacingFirstInputClaimHash(
        _ token: OfflineNotePaymentToken,
        keyCertificatePayloadHash: Data
    ) throws -> OfflineNotePaymentToken {
        var inputClaims = token.audit.inputClaims
        let input = try XCTUnwrap(inputClaims.first)
        inputClaims[0] = try OfflineNoteIssuedClaim(
            domain: input.domain,
            noteCommitment: input.noteCommitment,
            keyCertificatePayloadHash: keyCertificatePayloadHash,
            assetId: input.assetId,
            amount: input.amount
        )
        return try paymentTokenReplacingAuditClaims(token, inputClaims: inputClaims, outputClaims: token.audit.outputClaims)
    }

    private static func paymentTokenReplacingSenderCertificate(
        _ token: OfflineNotePaymentToken,
        certificate: OfflineNoteKeyCertificate
    ) throws -> OfflineNotePaymentToken {
        let certificateHash = try certificate.payloadHash()
        let inputClaims = try token.audit.inputClaims.map { input in
            try OfflineNoteIssuedClaim(
                domain: input.domain,
                noteCommitment: input.noteCommitment,
                keyCertificatePayloadHash: certificateHash,
                assetId: input.assetId,
                amount: input.amount
            )
        }
        let tokenId = try OfflineNotePaymentTokenIdPreimage(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            createdAtMs: token.createdAtMs,
            tokenNonce: token.tokenNonce,
            senderKeyCertificatePayloadHash: certificateHash,
            inputNullifiers: token.audit.inputNullifiers,
            outputCommitments: token.audit.outputCommitments
        ).derivePaymentTokenId()
        let draft = try OfflineNoteAuditBundle(
            tokenId: tokenId,
            senderKeyCertificate: certificate,
            inputNullifiers: token.audit.inputNullifiers,
            inputClaims: inputClaims,
            outputCommitments: token.audit.outputCommitments,
            outputClaims: token.audit.outputClaims,
            recursiveProof: token.audit.recursiveProof
        )
        let proof = try OfflineNoteRecursiveProof(
            verifierKeyId: token.audit.recursiveProof.verifierKeyId,
            publicInputsHash: draft.publicInputsHash(),
            proof: token.audit.recursiveProof.proof
        )
        return OfflineNotePaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: tokenId,
            audit: try draft.replacingRecursiveProof(proof),
            createdAtMs: token.createdAtMs
        )
    }

    private static func paymentTokenReplacingPaymentRequestId(
        _ token: OfflineNotePaymentToken,
        paymentRequestId: String
    ) throws -> OfflineNotePaymentToken {
        let tokenId = try OfflineNotePaymentTokenIdPreimage(
            chainId: token.chainId,
            paymentRequestId: paymentRequestId,
            createdAtMs: token.createdAtMs,
            tokenNonce: token.tokenNonce,
            senderKeyCertificatePayloadHash: token.audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: token.audit.inputNullifiers,
            outputCommitments: token.audit.outputCommitments
        ).derivePaymentTokenId()
        let draft = try OfflineNoteAuditBundle(
            tokenId: tokenId,
            senderKeyCertificate: token.audit.senderKeyCertificate,
            inputNullifiers: token.audit.inputNullifiers,
            inputClaims: token.audit.inputClaims,
            outputCommitments: token.audit.outputCommitments,
            outputClaims: token.audit.outputClaims,
            recursiveProof: token.audit.recursiveProof
        )
        let proof = try OfflineNoteRecursiveProof(
            verifierKeyId: token.audit.recursiveProof.verifierKeyId,
            publicInputsHash: draft.publicInputsHash(),
            proof: token.audit.recursiveProof.proof
        )
        return OfflineNotePaymentToken(
            chainId: token.chainId,
            paymentRequestId: paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: tokenId,
            audit: try draft.replacingRecursiveProof(proof),
            createdAtMs: token.createdAtMs
        )
    }

    private static func paymentTokenReplacingTopLevelTokenId(
        _ token: OfflineNotePaymentToken
    ) -> OfflineNotePaymentToken {
        OfflineNotePaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: flippedHash(token.tokenId),
            audit: token.audit,
            createdAtMs: token.createdAtMs
        )
    }

    private static func paymentTokenReplacingAuditTokenId(
        _ token: OfflineNotePaymentToken
    ) throws -> OfflineNotePaymentToken {
        let auditTokenId = flippedHash(token.audit.tokenId)
        let draft = try OfflineNoteAuditBundle(
            tokenId: auditTokenId,
            senderKeyCertificate: token.audit.senderKeyCertificate,
            inputNullifiers: token.audit.inputNullifiers,
            inputClaims: token.audit.inputClaims,
            outputCommitments: token.audit.outputCommitments,
            outputClaims: token.audit.outputClaims,
            recursiveProof: token.audit.recursiveProof
        )
        let proof = try OfflineNoteRecursiveProof(
            verifierKeyId: token.audit.recursiveProof.verifierKeyId,
            publicInputsHash: draft.publicInputsHash(),
            proof: token.audit.recursiveProof.proof
        )
        return OfflineNotePaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: token.tokenId,
            audit: try draft.replacingRecursiveProof(proof),
            createdAtMs: token.createdAtMs
        )
    }

    private static func paymentTokenReplacingBearerAuditTrail(
        _ token: OfflineNotePaymentToken,
        bearerAuditTrail: [OfflineNoteAuditBundle]
    ) -> OfflineNotePaymentToken {
        OfflineNotePaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: token.tokenId,
            audit: token.audit,
            bearerAuditTrail: bearerAuditTrail,
            createdAtMs: token.createdAtMs
        )
    }

    private static func auditReplacingTokenIdWithoutProofRebind(
        _ audit: OfflineNoteAuditBundle
    ) throws -> OfflineNoteAuditBundle {
        try OfflineNoteAuditBundle(
            tokenId: flippedHash(audit.tokenId),
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )
    }

    private static func auditReplacingFirstOutputAmountWithoutProofRebind(
        _ audit: OfflineNoteAuditBundle,
        amount: String
    ) throws -> OfflineNoteAuditBundle {
        var outputClaims = audit.outputClaims
        let output = try XCTUnwrap(outputClaims.first)
        outputClaims[0] = try OfflineNoteAuditOutputClaim(
            noteCommitment: output.noteCommitment,
            keyCertificate: output.keyCertificate,
            assetId: output.assetId,
            amount: amount
        )
        return try OfflineNoteAuditBundle(
            tokenId: flippedHash(audit.tokenId),
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers.map(flippedHash),
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: outputClaims,
            recursiveProof: audit.recursiveProof
        )
    }

    private static func selfConsumingAudit(
        _ audit: OfflineNoteAuditBundle
    ) throws -> OfflineNoteAuditBundle {
        let firstInput = try XCTUnwrap(audit.inputClaims.first)
        let firstOutput = try XCTUnwrap(audit.outputClaims.first)
        let replacementOutputCommitment = flippedHash(firstOutput.noteCommitment)
        var inputClaims = audit.inputClaims
        inputClaims[0] = try OfflineNoteIssuedClaim(
            domain: firstInput.domain,
            noteCommitment: replacementOutputCommitment,
            keyCertificatePayloadHash: firstInput.keyCertificatePayloadHash,
            assetId: firstInput.assetId,
            amount: firstInput.amount
        )
        var outputClaims = audit.outputClaims
        outputClaims[0] = try OfflineNoteAuditOutputClaim(
            noteCommitment: replacementOutputCommitment,
            keyCertificate: firstOutput.keyCertificate,
            assetId: firstOutput.assetId,
            amount: firstOutput.amount
        )
        var outputCommitments = audit.outputCommitments
        outputCommitments[0] = replacementOutputCommitment
        let draft = try OfflineNoteAuditBundle(
            tokenId: flippedHash(audit.tokenId),
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers.map(flippedHash),
            inputClaims: inputClaims,
            outputCommitments: outputCommitments,
            outputClaims: outputClaims,
            recursiveProof: audit.recursiveProof
        )
        return try draft.replacingRecursiveProof(OfflineNoteRecursiveProof(
            publicInputsHash: draft.publicInputsHash(),
            proofBytes: Data("self-consuming-audit-proof".utf8)
        ))
    }

    private static func paymentTokenReplacingOutputs(
        _ token: OfflineNotePaymentToken,
        outputClaims: [OfflineNoteAuditOutputClaim],
        outputCommitments: [Data]
    ) throws -> OfflineNotePaymentToken {
        let tokenId = try OfflineNotePaymentTokenIdPreimage(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            createdAtMs: token.createdAtMs,
            tokenNonce: token.tokenNonce,
            senderKeyCertificatePayloadHash: token.audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: token.audit.inputNullifiers,
            outputCommitments: outputCommitments
        ).derivePaymentTokenId()
        let draft = try OfflineNoteAuditBundle(
            tokenId: tokenId,
            senderKeyCertificate: token.audit.senderKeyCertificate,
            inputNullifiers: token.audit.inputNullifiers,
            inputClaims: token.audit.inputClaims,
            outputCommitments: outputCommitments,
            outputClaims: outputClaims,
            recursiveProof: token.audit.recursiveProof
        )
        let proof = try OfflineNoteRecursiveProof(
            verifierKeyId: token.audit.recursiveProof.verifierKeyId,
            publicInputsHash: draft.publicInputsHash(),
            proof: token.audit.recursiveProof.proof
        )
        return OfflineNotePaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: tokenId,
            audit: try draft.replacingRecursiveProof(proof),
            createdAtMs: token.createdAtMs
        )
    }

    private static func flippedHash(_ hash: Data) -> Data {
        var copy = hash
        copy[copy.startIndex] ^= 0x01
        return copy
    }

    private static func paymentTokenReplacingAuditClaims(
        _ token: OfflineNotePaymentToken,
        inputClaims: [OfflineNoteIssuedClaim],
        outputClaims: [OfflineNoteAuditOutputClaim]
    ) throws -> OfflineNotePaymentToken {
        let draft = try OfflineNoteAuditBundle(
            tokenId: token.audit.tokenId,
            senderKeyCertificate: token.audit.senderKeyCertificate,
            inputNullifiers: token.audit.inputNullifiers,
            inputClaims: inputClaims,
            outputCommitments: token.audit.outputCommitments,
            outputClaims: outputClaims,
            recursiveProof: token.audit.recursiveProof
        )
        let proof = try OfflineNoteRecursiveProof(
            verifierKeyId: token.audit.recursiveProof.verifierKeyId,
            publicInputsHash: draft.publicInputsHash(),
            proof: token.audit.recursiveProof.proof
        )
        return OfflineNotePaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: token.tokenId,
            audit: try draft.replacingRecursiveProof(proof),
            createdAtMs: token.createdAtMs
        )
    }

    private static func issuedClaim(_ json: OfflineInputClaimJSON) throws -> OfflineNoteIssuedClaim {
        try OfflineNoteIssuedClaim(
            domain: json.domain,
            noteCommitment: hex(json.noteCommitment),
            keyCertificatePayloadHash: hex(json.keyCertificatePayloadHash),
            assetId: json.assetId,
            amount: json.amount
        )
    }

    private static func auditOutputClaim(_ json: OfflineOutputClaimJSON) throws -> OfflineNoteAuditOutputClaim {
        try OfflineNoteAuditOutputClaim(
            noteCommitment: hex(json.noteCommitment),
            keyCertificate: certificate(json.keyCertificate),
            assetId: "\(json.assetDefinitionId)#\(json.accountId)",
            amount: json.amount
        )
    }

    private static func sourceWalletNote(
        _ fixture: OfflineInteropFixture,
        certificate: OfflineNoteKeyCertificate
    ) throws -> OfflineNoteWalletNote {
        let derivation = fixture.chainVectors.derivation
        return try OfflineNoteWalletNote(
            chainId: derivation.chainId,
            accountId: accountId(fromAssetId: fixture.chainVectors.issue.assetId),
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount,
            keyCertificate: certificate,
            noteCommitment: hex(derivation.sourceNoteCommitment),
            noteSecret: hex(derivation.sourceNoteSecretHex),
            origin: .issuerLoad(OfflineNoteIssuerLoadOrigin(
                operationId: derivation.issuerLoadOperationId,
                lineageId: derivation.issuerLoadLineageId,
                localRevision: derivation.issuerLoadLocalRevision
            )),
            state: .spendable,
            createdAtMs: 1_700_000_000_000,
            updatedAtMs: 1_700_000_000_000
        )
    }

    private static func storedCollectionData(notes: [OfflineNoteWalletNote]) throws -> Data {
        let stored = try notes.map { note in
            [
                "commitmentHex": note.noteCommitmentHex,
                "payloadBase64": try OfflineNoteWalletNoteJsonCodec.encode(note).base64EncodedString()
            ]
        }
        return try JSONSerialization.data(
            withJSONObject: ["version": 1, "notes": stored],
            options: [.sortedKeys]
        )
    }

    private static func storedMetadataData(version: Int = 1, revision: Int) throws -> Data {
        try JSONSerialization.data(
            withJSONObject: ["version": version, "revision": revision],
            options: [.sortedKeys]
        )
    }

    private final class RecordingOfflineNoteKeychainBacking: OfflineNoteKeychainBackingStore {
        var values: [String: Data] = [:]
        var operations: [String] = []
        var saveFailures: Set<String> = []
        var deleteFailures: Set<String> = []

        func load(label: String) throws -> Data? {
            values[label]
        }

        func save(label: String, data: Data) throws {
            operations.append("save:\(label)")
            if saveFailures.contains(label) {
                throw OfflineNoteKeychainStoreError.keychainFailure(-1)
            }
            values[label] = data
        }

        func delete(label: String) throws {
            operations.append("delete:\(label)")
            if deleteFailures.contains(label) {
                throw OfflineNoteKeychainStoreError.keychainFailure(-1)
            }
            values.removeValue(forKey: label)
        }
    }

    private struct StaticAttestationProvider: OfflineNoteAttestationProvider {
        let certificate: OfflineNoteKeyCertificate

        func currentKeyCertificate() throws -> OfflineNoteKeyCertificate {
            certificate
        }
    }

    private final class QueueRandomSource: OfflineNoteRandomSource {
        private let values: [Data]
        private var index = 0

        init(values: [Data]) {
            self.values = values
        }

        func nextBytes(count: Int) throws -> Data {
            guard index < values.count else {
                throw OfflineNoteFixtureError.randomSourceExhausted
            }
            let value = values[index]
            index += 1
            guard value.count == count else {
                throw OfflineNoteWalletError.randomLength(expected: count, actual: value.count)
            }
            return value
        }
    }

    private struct FixedIdGenerator: OfflineNoteIdGenerator {
        let id: String

        func nextId(prefix: String) -> String {
            id
        }
    }

    private final class SequenceIdGenerator: OfflineNoteIdGenerator {
        private let ids: [String]
        private var index = 0

        init(ids: [String]) {
            self.ids = ids
        }

        func nextId(prefix: String) -> String {
            precondition(index < ids.count, "test id generator exhausted")
            defer { index += 1 }
            return ids[index]
        }
    }

    private struct StaticIssuerDeviceBindingProvider: OfflineNoteIssuerDeviceBindingProvider {
        let binding: OfflineNoteIssuerDeviceBinding

        func currentDeviceBinding(chainId: String,
                                  accountId: String,
                                  assetDefinitionId: String) throws -> OfflineNoteIssuerDeviceBinding {
            binding
        }
    }

    private struct BindingProofProvider: OfflineNoteProofProvider {
        func proveAudit(_ audit: OfflineNoteAuditBundle) throws -> OfflineNoteRecursiveProof {
            try OfflineNoteRecursiveProof(
                publicInputsHash: audit.publicInputsHash(),
                proofBytes: Data("wallet-audit-proof".utf8)
            )
        }

        func proveRedeem(_ redemption: OfflineNoteRedeem) throws -> OfflineNoteRecursiveProof {
            try OfflineNoteRecursiveProof(
                publicInputsHash: redemption.publicInputsHash(),
                proofBytes: Data("wallet-redeem-proof".utf8)
            )
        }
    }

    private struct BindingProofVerifier: OfflineNoteProofVerifier {
        func verifyAudit(_ audit: OfflineNoteAuditBundle) throws -> Bool {
            try audit.recursiveProof.publicInputsHash == audit.publicInputsHash()
        }

        func verifyRedeem(_ redemption: OfflineNoteRedeem) throws -> Bool {
            try redemption.recursiveProof.publicInputsHash == redemption.publicInputsHash()
        }
    }

    private final class RecordingIssuerClient: OfflineNoteIssuerClient {
        let loadContext: OfflineNoteLoadContext
        var lastIssueRequest: OfflineNoteIssueRequest?
        var lastPrepareLoadAssetDefinitionId: String?

        init(loadContext: OfflineNoteLoadContext) {
            self.loadContext = loadContext
        }

        func prepareLoad(chainId: String,
                         accountId: String,
                         assetDefinitionId: String,
                         amount: String) async throws -> OfflineNoteLoadContext {
            lastPrepareLoadAssetDefinitionId = assetDefinitionId
            return loadContext
        }

        func issueNote(_ request: OfflineNoteIssueRequest) async throws -> OfflineNoteIssueResponse {
            lastIssueRequest = request
            return OfflineNoteIssueResponse(
                noteCommitment: request.noteCommitment,
                operationId: request.loadContext.operationId,
                lineageId: request.loadContext.lineageId,
                localRevision: request.loadContext.localRevision,
                keyCertificate: request.loadContext.keyCertificate,
                settlementEntryHashHex: "settlement-entry-hash"
            )
        }
    }

    private final class RecordingTransactionSubmitter: OfflineNoteTransactionSubmitter {
        private(set) var audits: [OfflineNoteAuditBundle] = []
        private(set) var redemptions: [OfflineNoteRedeem] = []
        private(set) var defunds: [(redemption: OfflineNoteRedeem, bearerAuditTrail: [OfflineNoteAuditBundle])] = []

        func submitAudit(_ audit: OfflineNoteAuditBundle) async throws {
            audits.append(audit)
        }

        func submitRedeem(_ redemption: OfflineNoteRedeem) async throws {
            redemptions.append(redemption)
        }

        func submitDefund(_ redemption: OfflineNoteRedeem,
                          bearerAuditTrail: [OfflineNoteAuditBundle]) async throws {
            defunds.append((redemption, bearerAuditTrail))
        }
    }

    private struct FailingTransactionSubmitter: OfflineNoteTransactionSubmitter {
        func submitAudit(_ audit: OfflineNoteAuditBundle) async throws {
            throw OfflineNoteWalletError.invalidState
        }

        func submitRedeem(_ redemption: OfflineNoteRedeem) async throws {
            throw OfflineNoteWalletError.invalidState
        }

        func submitDefund(_ redemption: OfflineNoteRedeem,
                          bearerAuditTrail: [OfflineNoteAuditBundle]) async throws {
            throw OfflineNoteWalletError.invalidState
        }
    }

    private final class RecordingSyncResolver: OfflineNoteSyncResolver {
        var resolutions: [String: OfflineNoteWalletNoteState]
        private(set) var resolvedCommitments: [String] = []

        init(resolutions: [String: OfflineNoteWalletNoteState]) {
            self.resolutions = resolutions
        }

        func resolvePendingNote(_ note: OfflineNoteWalletNote) async throws -> OfflineNoteSyncResolution? {
            let commitment = note.noteCommitmentHex
            resolvedCommitments.append(commitment)
            guard let state = resolutions[commitment] else {
                return nil
            }
            return OfflineNoteSyncResolution(state: state, transactionHashHex: "tx-\(commitment)")
        }
    }

    private static func issueInstructionEnvelope(_ issue: OfflineNoteIssue) throws -> Data {
        rawInstructionPair(
            wireName: OfflineNoteTypeNames.issueInstruction,
            wirePayload: try instructionWirePayload(
                typeName: OfflineNoteTypeNames.issueInstruction,
                modelPayload: OfflineNoteEncoding.encodeIssue(issue)
            )
        )
    }

    private static func instructionCount(in envelope: SignedTransactionEnvelope) throws -> UInt64 {
        var signedTransaction = OfflineNoritoReader(data: envelope.signedTransaction)
        _ = try signedTransaction.readField()
        let transactionPayload = try signedTransaction.readField()
        var transaction = OfflineNoritoReader(data: transactionPayload)
        _ = try transaction.readField()
        _ = try transaction.readField()
        _ = try transaction.readField()
        let executablePayload = try transaction.readField()
        var executable = OfflineNoritoReader(data: executablePayload)
        XCTAssertEqual(try executable.readUInt32LE(), 0)
        let instructionsPayload = try executable.readField()
        var instructions = OfflineNoritoReader(data: instructionsPayload)
        return try instructions.readUInt64LE()
    }

    private static func auditInstructionEnvelope(_ audit: OfflineNoteAuditBundle) throws -> Data {
        rawInstructionPair(
            wireName: OfflineNoteTypeNames.auditInstruction,
            wirePayload: try instructionWirePayload(
                typeName: OfflineNoteTypeNames.auditInstruction,
                modelPayload: OfflineNoteEncoding.encodeAudit(audit)
            )
        )
    }

    private static func redeemInstructionEnvelope(_ redemption: OfflineNoteRedeem) throws -> Data {
        rawInstructionPair(
            wireName: OfflineNoteTypeNames.redeemInstruction,
            wirePayload: try instructionWirePayload(
                typeName: OfflineNoteTypeNames.redeemInstruction,
                modelPayload: OfflineNoteEncoding.encodeRedeem(redemption)
            )
        )
    }

    private static func instructionWirePayload(typeName: String, modelPayload: Data) -> Data {
        var payload = OfflineNoritoWriter()
        payload.writeField(modelPayload)
        return noritoEncode(typeName: typeName, payload: payload.data, flags: 0)
    }

    private static func rawInstructionPair(wireName: String, wirePayload: Data, compact: Bool = true) -> Data {
        var data = Data()
        writeInstructionField(encodeInstructionString(wireName, compact: compact), to: &data, compact: compact)
        writeInstructionField(encodeInstructionBytesVec(wirePayload), to: &data, compact: compact)
        return data
    }

    private static func encodeInstructionString(_ value: String, compact: Bool) -> Data {
        let bytes = Data(value.utf8)
        var data = Data()
        appendInstructionLength(UInt64(bytes.count), to: &data, compact: compact)
        data.append(bytes)
        return data
    }

    private static func encodeInstructionBytesVec(_ value: Data) -> Data {
        var data = Data()
        appendInstructionLength(UInt64(value.count), to: &data, compact: false)
        data.append(value)
        return data
    }

    private static func writeInstructionField(_ payload: Data, to data: inout Data, compact: Bool) {
        appendInstructionLength(UInt64(payload.count), to: &data, compact: compact)
        data.append(payload)
    }

    private static func appendInstructionLength(_ value: UInt64, to data: inout Data, compact: Bool) {
        if compact {
            var remaining = value
            while remaining >= 0x80 {
                data.append(UInt8(remaining & 0x7f) | 0x80)
                remaining >>= 7
            }
            data.append(UInt8(remaining))
        } else {
            var littleEndian = value.littleEndian
            data.append(contentsOf: withUnsafeBytes(of: &littleEndian, Array.init))
        }
    }

    private static func loadFixture() throws -> OfflineInteropFixture {
        let testFile = URL(fileURLWithPath: #filePath)
        let fixtureURL = testFile
            .deletingLastPathComponent()
            .appendingPathComponent("../../../fixtures/offline/interop_contract.json")
            .standardizedFileURL
        let data = try Data(contentsOf: fixtureURL)
        return try JSONDecoder().decode(OfflineInteropFixture.self, from: data)
    }

    private static func hex(_ value: String) throws -> Data {
        guard let data = Data(hexString: value) else {
            throw OfflineNoteFixtureError.invalidHex(value)
        }
        return data
    }

    private static func base64(_ value: String) throws -> Data {
        guard let data = Data(base64Encoded: value) else {
            throw OfflineNoteFixtureError.invalidBase64
        }
        return data
    }

    private static func base64Url(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .trimmingCharacters(in: CharacterSet(charactersIn: "="))
    }

    private static func certificateJSON(_ certificate: OfflineCertificateJSON,
                                        expiresAtMs: UInt64) -> [String: Any] {
        [
            "version": Int(certificate.version),
            "platform": certificate.platform,
            "key_id": certificate.keyId,
            "device_id": certificate.deviceId,
            "account_id": certificate.accountId,
            "public_key": certificate.publicKey,
            "assertion_scheme": certificate.assertionScheme,
            "assertion_key_algorithm": certificate.assertionKeyAlgorithm,
            "assertion_public_key": certificate.assertionPublicKey,
            "assertion_usage_count_limit": certificate.assertionUsageCountLimit.map { Int($0) } ?? NSNull(),
            "one_use": certificate.oneUse,
            "issuer_signature_base64": certificate.issuerSignatureBase64,
            "expires_at_ms": NSNumber(value: expiresAtMs),
        ]
    }

    private static func lineageState(revision: UInt64, balance: String) -> [String: Any] {
        [
            "lineage_id": "lineage-1",
            "server_revision": NSNumber(value: revision),
            "pending_local_revision": NSNumber(value: revision),
            "balance": balance,
            "locked_balance": "0",
            "authorization": [
                "expires_at_ms": NSNumber(value: 1_700_000_060_000),
            ],
        ]
    }

    private static func requestBody(_ request: URLRequest) throws -> [String: Any] {
        guard let body = request.httpBody ?? OfflineIssuerURLProtocol.body(for: request) else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("request_body")
        }
        let parsed = try JSONSerialization.jsonObject(with: body)
        guard let object = parsed as? [String: Any] else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("request_body")
        }
        return object
    }

    private static func object(_ object: [String: Any], _ key: String) throws -> [String: Any] {
        guard let value = object[key] as? [String: Any] else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON(key)
        }
        return value
    }

    private static func string(_ object: [String: Any], _ key: String) throws -> String {
        guard let value = object[key] as? String else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON(key)
        }
        return value
    }

    private static func uint64(_ object: [String: Any], _ key: String) throws -> UInt64 {
        if let value = object[key] as? UInt64 {
            return value
        }
        if let value = object[key] as? Int {
            return UInt64(value)
        }
        if let value = object[key] as? NSNumber {
            return value.uint64Value
        }
        throw ToriiOfflineNoteIssuerClientError.invalidJSON(key)
    }

    private static func assetDefinition(fromAssetId assetId: String) -> String {
        String(assetId.split(separator: "#", maxSplits: 1)[0])
    }

    private static func accountId(fromAssetId assetId: String) -> String {
        String(assetId.split(separator: "#", maxSplits: 1)[1].split(separator: "#", maxSplits: 1)[0])
    }
}

private final class OfflineIssuerURLProtocol: URLProtocol {
    static var handler: ((URLRequest) throws -> (Int, Data))?
    private(set) static var requests: [URLRequest] = []

    static func reset() {
        handler = nil
        requests = []
    }

    override class func canInit(with request: URLRequest) -> Bool {
        true
    }

    override class func canonicalRequest(for request: URLRequest) -> URLRequest {
        request
    }

    override func startLoading() {
        do {
            Self.requests.append(request)
            guard let handler = Self.handler else {
                throw ToriiOfflineNoteIssuerClientError.invalidURL(request.url?.absoluteString ?? "")
            }
            let (status, body) = try handler(request)
            let response = HTTPURLResponse(
                url: request.url!,
                statusCode: status,
                httpVersion: nil,
                headerFields: ["Content-Type": "application/json"]
            )!
            client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: body)
            client?.urlProtocolDidFinishLoading(self)
        } catch {
            client?.urlProtocol(self, didFailWithError: error)
        }
    }

    override func stopLoading() {}

    static func body(for request: URLRequest) -> Data? {
        if let body = request.httpBody {
            return body
        }
        guard let stream = request.httpBodyStream else {
            return nil
        }
        stream.open()
        defer { stream.close() }
        var data = Data()
        let bufferSize = 4096
        let buffer = UnsafeMutablePointer<UInt8>.allocate(capacity: bufferSize)
        defer { buffer.deallocate() }
        while stream.hasBytesAvailable {
            let read = stream.read(buffer, maxLength: bufferSize)
            if read > 0 {
                data.append(buffer, count: read)
            } else {
                break
            }
        }
        return data
    }
}

private enum OfflineNoteFixtureError: Error {
    case invalidHex(String)
    case invalidBase64
    case randomSourceExhausted
}

private struct OfflineInteropFixture: Decodable {
    let offlineFiPublicKeyBase64: String
    let chainVectors: OfflineChainVectors
    let paymentToken: OfflinePaymentTokenJSON
    let sdkInterop: OfflineSdkInterop

    private enum CodingKeys: String, CodingKey {
        case offlineFiPublicKeyBase64 = "offline_fi_public_key_base64"
        case chainVectors = "chain_vectors"
        case paymentToken = "payment_token"
        case sdkInterop = "sdk_interop"
    }
}

private struct OfflineSdkInterop: Decodable {
    let paymentTokenNoritoBase64: String
    let paymentTokenText: String
    let paymentTokenQr: OfflineQrFixture

    private enum CodingKeys: String, CodingKey {
        case paymentTokenNoritoBase64 = "payment_token_norito_base64"
        case paymentTokenText = "payment_token_text"
        case paymentTokenQr = "payment_token_qr"
    }
}

private struct OfflineQrFixture: Decodable {
    let frames: [OfflineQrFrameFixture]
}

private struct OfflineQrFrameFixture: Decodable {
    let bytesHex: String

    private enum CodingKeys: String, CodingKey {
        case bytesHex = "bytes_hex"
    }
}

private struct OfflineChainVectors: Decodable {
    let derivation: OfflineDerivationVector
    let certificates: OfflineCertificateVectors
    let issue: OfflineIssueVector
    let audit: OfflineAuditVector
    let redeem: OfflineRedeemVector
}

private struct OfflineDerivationVector: Decodable {
    let chainId: String
    let issuerLoadOperationId: String
    let issuerLoadLineageId: String
    let issuerLoadLocalRevision: UInt64
    let paymentRequestId: String
    let sourceNoteSecretHex: String
    let recipientNoteSecretHex: String
    let changeNoteSecretHex: String
    let tokenNonceHex: String
    let senderKeyCertificatePayloadHash: String
    let recipientKeyCertificatePayloadHash: String
    let sourceNoteCommitment: String
    let inputNullifier: String
    let recipientOutputCommitment: String
    let changeOutputCommitment: String
    let paymentTokenId: String
    let redeemNullifier: String

    private enum CodingKeys: String, CodingKey {
        case chainId = "chain_id"
        case issuerLoadOperationId = "issuer_load_operation_id"
        case issuerLoadLineageId = "issuer_load_lineage_id"
        case issuerLoadLocalRevision = "issuer_load_local_revision"
        case paymentRequestId = "payment_request_id"
        case sourceNoteSecretHex = "source_note_secret_hex"
        case recipientNoteSecretHex = "recipient_note_secret_hex"
        case changeNoteSecretHex = "change_note_secret_hex"
        case tokenNonceHex = "token_nonce_hex"
        case senderKeyCertificatePayloadHash = "sender_key_certificate_payload_hash"
        case recipientKeyCertificatePayloadHash = "recipient_key_certificate_payload_hash"
        case sourceNoteCommitment = "source_note_commitment"
        case inputNullifier = "input_nullifier"
        case recipientOutputCommitment = "recipient_output_commitment"
        case changeOutputCommitment = "change_output_commitment"
        case paymentTokenId = "payment_token_id"
        case redeemNullifier = "redeem_nullifier"
    }
}

private struct OfflineCertificateVectors: Decodable {
    let senderPayloadBase64: String
    let senderPayloadHash: String

    private enum CodingKeys: String, CodingKey {
        case senderPayloadBase64 = "sender_payload_base64"
        case senderPayloadHash = "sender_payload_hash"
    }
}

private struct OfflineIssueVector: Decodable {
    let noteCommitment: String
    let assetId: String
    let amount: String
    let noritoBase64: String

    private enum CodingKeys: String, CodingKey {
        case noteCommitment = "note_commitment"
        case assetId = "asset_id"
        case amount
        case noritoBase64 = "norito_base64"
    }
}

private struct OfflineAuditVector: Decodable {
    let tokenId: String
    let inputNullifiers: [String]
    let outputCommitments: [String]
    let publicInputsHash: String
    let noritoBase64: String

    private enum CodingKeys: String, CodingKey {
        case tokenId = "token_id"
        case inputNullifiers = "input_nullifiers"
        case outputCommitments = "output_commitments"
        case publicInputsHash = "public_inputs_hash"
        case noritoBase64 = "norito_base64"
    }
}

private struct OfflineRedeemVector: Decodable {
    let sourceNoteCommitment: String
    let inputNullifiers: [String]
    let assetId: String
    let amount: String
    let publicInputsHash: String
    let noritoBase64: String

    private enum CodingKeys: String, CodingKey {
        case sourceNoteCommitment = "source_note_commitment"
        case inputNullifiers = "input_nullifiers"
        case assetId = "asset_id"
        case amount
        case publicInputsHash = "public_inputs_hash"
        case noritoBase64 = "norito_base64"
    }
}

private struct OfflinePaymentTokenJSON: Decodable {
    let tokenId: String
    let invoiceId: String
    let createdAtMs: UInt64
    let senderAccountId: String
    let recipientAccountId: String
    let senderKeyCertificate: OfflineCertificateJSON
    let recipientKeyCertificate: OfflineCertificateJSON
    let inputClaims: [OfflineInputClaimJSON]
    let outputClaims: [OfflineOutputClaimJSON]

    private enum CodingKeys: String, CodingKey {
        case tokenId = "token_id"
        case invoiceId = "invoice_id"
        case createdAtMs = "created_at_ms"
        case senderAccountId = "sender_account_id"
        case recipientAccountId = "recipient_account_id"
        case senderKeyCertificate = "sender_key_certificate"
        case recipientKeyCertificate = "recipient_key_certificate"
        case inputClaims = "input_claims"
        case outputClaims = "output_claims"
    }
}

private struct OfflineCertificateJSON: Decodable {
    let version: UInt16
    let platform: String
    let keyId: String
    let deviceId: String
    let accountId: String
    let publicKey: String
    let assertionScheme: String
    let assertionKeyAlgorithm: String
    let assertionPublicKey: String
    let assertionUsageCountLimit: UInt32?
    let oneUse: Bool
    let issuerSignatureBase64: String

    private enum CodingKeys: String, CodingKey {
        case version
        case platform
        case keyId = "key_id"
        case deviceId = "device_id"
        case accountId = "account_id"
        case publicKey = "public_key"
        case assertionScheme = "assertion_scheme"
        case assertionKeyAlgorithm = "assertion_key_algorithm"
        case assertionPublicKey = "assertion_public_key"
        case assertionUsageCountLimit = "assertion_usage_count_limit"
        case oneUse = "one_use"
        case issuerSignatureBase64 = "issuer_signature_base64"
    }
}

private struct OfflineInputClaimJSON: Decodable {
    let domain: String
    let noteCommitment: String
    let keyCertificatePayloadHash: String
    let assetId: String
    let amount: String

    private enum CodingKeys: String, CodingKey {
        case domain
        case noteCommitment = "note_commitment"
        case keyCertificatePayloadHash = "key_certificate_payload_hash"
        case assetId = "asset_id"
        case amount
    }
}

private struct OfflineOutputClaimJSON: Decodable {
    let accountId: String
    let noteCommitment: String
    let keyCertificate: OfflineCertificateJSON
    let assetDefinitionId: String
    let amount: String

    private enum CodingKeys: String, CodingKey {
        case accountId = "account_id"
        case noteCommitment = "note_commitment"
        case keyCertificate = "key_certificate"
        case assetDefinitionId = "asset_definition_id"
        case amount
    }
}
