import XCTest
@testable import IrohaSwift

final class OfflineNoteV2Tests: XCTestCase {
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
        let tampered = try OfflineNoteKeyCertificateV2(
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
        XCTAssertFalse(try RejectingOfflineNoteV2CertificateVerifier().verifyCertificate(sender))
        XCTAssertFalse(try Ed25519OfflineNoteV2CertificateVerifier(
            trustedIssuerPublicKeys: [Data(repeating: 0x42, count: 32)]
        ).verifyCertificate(sender))
    }

    func testOfflineNoteV2ModelsMatchRustNoritoVectors() throws {
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

    func testOfflineNoteV2PublicNoritoDecodersRoundTripFixturePayloads() throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let senderPayloadBytes = try Self.base64(fixture.chainVectors.certificates.senderPayloadBase64)
        let issueBytes = try Self.base64(fixture.chainVectors.issue.noritoBase64)
        let auditBytes = try Self.base64(fixture.chainVectors.audit.noritoBase64)
        let redeemBytes = try Self.base64(fixture.chainVectors.redeem.noritoBase64)

        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeKeyCertificatePayload(senderPayloadBytes)
                .noritoEncoded()
                .base64EncodedString(),
            senderPayloadBytes.base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeKeyCertificate(try senderCertificate.noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try senderCertificate.noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeIssue(issueBytes).noritoEncoded().base64EncodedString(),
            issueBytes.base64EncodedString()
        )

        let decodedAudit = try OfflineNoteV2Decoding.decodeAudit(auditBytes)
        XCTAssertEqual(try decodedAudit.noritoEncoded().base64EncodedString(), auditBytes.base64EncodedString())
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeIssuedClaim(decodedAudit.inputClaims[0].noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try decodedAudit.inputClaims[0].noritoEncoded().base64EncodedString()
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeAuditPublicInputs(decodedAudit.publicInputs().noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try decodedAudit.publicInputs().noritoEncoded().base64EncodedString()
        )

        let decodedRedeem = try OfflineNoteV2Decoding.decodeRedeem(redeemBytes)
        XCTAssertEqual(try decodedRedeem.noritoEncoded().base64EncodedString(), redeemBytes.base64EncodedString())
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeRedeemPublicInputs(decodedRedeem.publicInputs().noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try decodedRedeem.publicInputs().noritoEncoded().base64EncodedString()
        )

        let commitmentPreimage = try OfflineNoteCommitmentPreimageV2(
            chainId: derivation.chainId,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.redeem.amount,
            noteSecret: Self.hex(derivation.sourceNoteSecretHex),
            origin: .issuerLoad(OfflineNoteIssuerLoadOriginV2(
                operationId: derivation.issuerLoadOperationId,
                lineageId: derivation.issuerLoadLineageId,
                localRevision: derivation.issuerLoadLocalRevision
            ))
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeNoteCommitmentPreimage(commitmentPreimage.noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try commitmentPreimage.noritoEncoded().base64EncodedString()
        )

        let nullifierPreimage = try OfflineNoteInputNullifierPreimageV2(
            chainId: derivation.chainId,
            sourceNoteCommitment: Self.hex(derivation.sourceNoteCommitment),
            ownerKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            noteSecret: Self.hex(derivation.sourceNoteSecretHex)
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeInputNullifierPreimage(nullifierPreimage.noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try nullifierPreimage.noritoEncoded().base64EncodedString()
        )

        let tokenPreimage = try OfflineNotePaymentTokenIdPreimageV2(
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
            try OfflineNoteV2Decoding.decodePaymentTokenIdPreimage(tokenPreimage.noritoEncoded())
                .noritoEncoded()
                .base64EncodedString(),
            try tokenPreimage.noritoEncoded().base64EncodedString()
        )
    }

    func testOfflineNoteV2PublicNoritoInstructionDecodersReadExplorerEnvelopeBytes() throws {
        let fixture = try Self.loadFixture()
        let issue = try Self.issue(fixture)
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)

        let issueEnvelope = Self.rawInstructionPair(
            wireName: OfflineNoteV2TypeNames.issueInstruction,
            wirePayload: try Self.instructionWirePayload(
                typeName: OfflineNoteV2TypeNames.issueInstruction,
                modelPayload: OfflineNoteV2Encoding.encodeIssue(issue)
            )
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeIssueInstruction(issueEnvelope).noritoEncoded().base64EncodedString(),
            try issue.noritoEncoded().base64EncodedString()
        )

        let auditEnvelope = Self.rawInstructionPair(
            wireName: OfflineNoteV2TypeNames.auditInstruction,
            wirePayload: try Self.instructionWirePayload(
                typeName: OfflineNoteV2TypeNames.auditInstruction,
                modelPayload: OfflineNoteV2Encoding.encodeAudit(audit)
            )
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeAuditInstruction(auditEnvelope).noritoEncoded().base64EncodedString(),
            try audit.noritoEncoded().base64EncodedString()
        )

        let redeemEnvelope = Self.rawInstructionPair(
            wireName: OfflineNoteV2TypeNames.redeemInstruction,
            wirePayload: try Self.instructionWirePayload(
                typeName: OfflineNoteV2TypeNames.redeemInstruction,
                modelPayload: OfflineNoteV2Encoding.encodeRedeem(redeem)
            )
        )
        XCTAssertEqual(
            try OfflineNoteV2Decoding.decodeRedeemInstruction(redeemEnvelope).noritoEncoded().base64EncodedString(),
            try redeem.noritoEncoded().base64EncodedString()
        )
    }

    func testOfflineNoteV2PaymentTokenCodecRoundTripsNoritoTextAndQrFrames() throws {
        let fixture = try Self.loadFixture()
        let token = OfflineNoteV2PaymentToken(
            chainId: fixture.chainVectors.derivation.chainId,
            paymentRequestId: fixture.paymentToken.invoiceId,
            tokenNonce: try Self.hex(fixture.chainVectors.derivation.tokenNonceHex),
            tokenId: try Self.hex(fixture.paymentToken.tokenId),
            audit: try Self.audit(fixture),
            createdAtMs: fixture.paymentToken.createdAtMs
        )
        let canonicalPayload = try Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64)
        XCTAssertEqual(try OfflineNoteV2PaymentTokenCodec.encodeNorito(token), canonicalPayload)

        let noritoDecoded = try OfflineNoteV2PaymentTokenCodec.decodeNorito(
            OfflineNoteV2PaymentTokenCodec.encodeNorito(token)
        )
        XCTAssertEqual(noritoDecoded.tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(noritoDecoded.paymentRequestId, token.paymentRequestId)
        XCTAssertEqual(try noritoDecoded.audit.noritoEncoded(), try token.audit.noritoEncoded())
        let canonicalDecoded = try OfflineNoteV2PaymentTokenCodec.decodeNorito(canonicalPayload)
        XCTAssertEqual(canonicalDecoded.tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(try canonicalDecoded.audit.noritoEncoded(), try token.audit.noritoEncoded())

        let text = try OfflineNoteV2PaymentTokenCodec.encodeText(token)
        XCTAssertEqual(text, fixture.sdkInterop.paymentTokenText)
        XCTAssertTrue(text.hasPrefix(OfflineNoteV2PaymentTokenCodec.textPrefix))
        XCTAssertEqual(try OfflineNoteV2PaymentTokenCodec.decodeText(text).tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(
            try OfflineNoteV2PaymentTokenCodec.decodeText(fixture.sdkInterop.paymentTokenText).tokenIdHex,
            token.tokenIdHex
        )

        let frames = try OfflineNoteV2PaymentTokenCodec.encodeQrFrameBytes(
            token,
            options: OfflineQrStreamOptions(chunkSize: 180, parityGroup: 2)
        )
        XCTAssertEqual(
            frames.map { $0.hexLowercased() },
            fixture.sdkInterop.paymentTokenQrV1.frames.map(\.bytesHex)
        )
        let decoder = OfflineQrStreamDecoder()
        var payload: Data?
        for frame in frames {
            let result = try decoder.ingest(frameBytes: frame)
            payload = result.payload ?? payload
        }
        let qrDecoded = try OfflineNoteV2PaymentTokenCodec.decodeQrPayload(XCTUnwrap(payload))
        XCTAssertEqual(qrDecoded.tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(try qrDecoded.audit.noritoEncoded(), try token.audit.noritoEncoded())

        let canonicalDecoder = OfflineQrStreamDecoder()
        var canonicalQrPayload: Data?
        for frame in fixture.sdkInterop.paymentTokenQrV1.frames {
            let result = try canonicalDecoder.ingest(frameBytes: Self.hex(frame.bytesHex))
            canonicalQrPayload = result.payload ?? canonicalQrPayload
        }
        XCTAssertEqual(try XCTUnwrap(canonicalQrPayload), canonicalPayload)
        XCTAssertEqual(
            try OfflineNoteV2PaymentTokenCodec.decodeQrPayload(try XCTUnwrap(canonicalQrPayload)).tokenIdHex,
            token.tokenIdHex
        )
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

    func testOfflineNoteV2TransferHandoffSupportsQrNfcAndNearbyPayloads() throws {
        let fixture = try Self.loadFixture()
        let token = OfflineNoteV2PaymentToken(
            chainId: fixture.chainVectors.derivation.chainId,
            paymentRequestId: fixture.paymentToken.invoiceId,
            tokenNonce: try Self.hex(fixture.chainVectors.derivation.tokenNonceHex),
            tokenId: try Self.hex(fixture.paymentToken.tokenId),
            audit: try Self.audit(fixture),
            createdAtMs: fixture.paymentToken.createdAtMs
        )
        let canonicalPayload = try Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64)

        let capabilities = OfflineNoteV2TransferCapabilities.current()
        XCTAssertTrue(capabilities.supportedModalities.contains(.qrStreaming))
        XCTAssertTrue(capabilities.supportedModalities.contains(.nearby))
        XCTAssertFalse(capabilities.supportedModalities.contains(.nfc))

        let nearby = try OfflineNoteV2TransferHandoff.nearbyPayload(for: token)
        XCTAssertEqual(nearby.modality, .nearby)
        XCTAssertEqual(nearby.contentType, OfflineNoteV2TransferHandoff.paymentTokenContentType)
        XCTAssertEqual(nearby.payload, canonicalPayload)
        XCTAssertEqual(
            try OfflineNoteV2TransferHandoff.decodePaymentToken(from: nearby).tokenIdHex,
            token.tokenIdHex
        )

        let qrFrames = try OfflineNoteV2TransferHandoff.qrStreamingFrameBytes(for: token)
        XCTAssertEqual(
            qrFrames.map { $0.hexLowercased() },
            fixture.sdkInterop.paymentTokenQrV1.frames.map(\.bytesHex)
        )
        let qrReceiver = OfflineNoteV2TransferStreamReceiver()
        var qrResult: OfflineNoteV2TransferStreamResult?
        for frame in qrFrames {
            qrResult = try qrReceiver.ingestFrame(frame)
        }
        XCTAssertEqual(try XCTUnwrap(qrResult?.token).tokenIdHex, token.tokenIdHex)

        let nfcFrames = try OfflineNoteV2TransferHandoff.nfcFrameBytes(for: token)
        XCTAssertTrue(nfcFrames.allSatisfy { $0.count <= 250 })
        let nfcReceiver = OfflineNoteV2TransferStreamReceiver()
        var nfcResult: OfflineNoteV2TransferStreamResult?
        for frame in nfcFrames {
            nfcResult = try nfcReceiver.ingestFrame(frame)
        }
        XCTAssertEqual(try XCTUnwrap(nfcResult?.token).tokenIdHex, token.tokenIdHex)
    }

    func testOfflineNoteV2TransferHandoffRejectsAdversarialStreamsAndMetadata() throws {
        let fixture = try Self.loadFixture()
        let token = try OfflineNoteV2PaymentTokenCodec.decodeNorito(
            Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64)
        )
        let rawPayload = try OfflineNoteV2TransferHandoff.rawPaymentTokenBytes(for: token)
        let payload = try OfflineNoteV2TransferHandoff.paymentTokenPayload(for: token, modality: .qrStreaming)
        let wrongContentType = OfflineNoteV2TransferPayload(
            modality: .nearby,
            contentType: OfflineNoteV2TransferHandoff.receiptAckContentType,
            payload: payload.payload
        )
        XCTAssertThrowsError(try OfflineNoteV2TransferHandoff.decodePaymentToken(from: wrongContentType))

        let frames = try OfflineNoteV2TransferHandoff.qrStreamingFrameBytes(
            for: token,
            options: OfflineQrStreamOptions(chunkSize: 128, parityGroup: 0)
        )
        XCTAssertGreaterThan(frames.count, 2)

        var badMagic = frames[0]
        badMagic[badMagic.startIndex] = 0x00
        XCTAssertThrowsError(try OfflineNoteV2TransferStreamReceiver().ingestFrame(badMagic))

        var badVersion = frames[0]
        badVersion[badVersion.startIndex + 2] = 0x7f
        XCTAssertThrowsError(try OfflineNoteV2TransferStreamReceiver().ingestFrame(badVersion))

        var badChecksum = frames[1]
        badChecksum[badChecksum.index(before: badChecksum.endIndex)] ^= 0x01
        XCTAssertThrowsError(try OfflineNoteV2TransferStreamReceiver().ingestFrame(badChecksum))

        XCTAssertThrowsError(try OfflineNoteV2TransferStreamReceiver().ingestFrame(Data(frames[0].prefix(8))))

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
        XCTAssertThrowsError(try OfflineNoteV2TransferStreamReceiver().ingestFrame(mismatchedHeader))

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
        let ignoreWrongStreamReceiver = OfflineNoteV2TransferStreamReceiver()
        XCTAssertNil(try ignoreWrongStreamReceiver.ingestFrame(frames[0]).token)
        XCTAssertNil(try ignoreWrongStreamReceiver.ingestFrame(wrongStreamFrame).token)
        var completed: OfflineNoteV2TransferStreamResult?
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
        let poisonedReceiver = OfflineNoteV2TransferStreamReceiver()
        _ = try poisonedReceiver.ingestFrame(frames[0])
        _ = try poisonedReceiver.ingestFrame(poisonedFrame)
        XCTAssertThrowsError(try frames.dropFirst(2).forEach { _ = try poisonedReceiver.ingestFrame($0) })

        let wrongKindFrames = try OfflineQrStreamEncoder.encodeFrameBytes(
            payload: rawPayload,
            payloadKind: .offlineReceiptAckV2,
            options: OfflineQrStreamOptions(chunkSize: 512, parityGroup: 0)
        )
        let wrongKindReceiver = OfflineNoteV2TransferStreamReceiver()
        XCTAssertThrowsError(try wrongKindFrames.forEach { _ = try wrongKindReceiver.ingestFrame($0) })
    }

    func testOfflineNoteV2NfcApduProtocolSupportsAndroidSafeAndIOSFastChunks() throws {
        let fixture = try Self.loadFixture()
        let token = try OfflineNoteV2PaymentTokenCodec.decodeNorito(Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64))
        let payload = try OfflineNoteV2TransferHandoff.rawPaymentTokenBytes(for: token)

        XCTAssertEqual(OfflineNoteV2TransferHandoff.defaultNfcAidHex, OfflineNoteV2NfcApduProtocol.aidHex)
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.parseCommand(OfflineNoteV2NfcApduProtocol.selectAidAPDUData()), .select)
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.parseCommand(OfflineNoteV2NfcApduProtocol.getInfoAPDUData()), .getInfo)

        let infoBytes = try OfflineNoteV2NfcApduProtocol.encodeInfo(kind: .paymentToken, payloadBytes: payload)
        let info = try XCTUnwrap(OfflineNoteV2NfcApduProtocol.decodeInfo(infoBytes))
        XCTAssertEqual(info.kind, .paymentToken)
        XCTAssertEqual(info.payloadLength, payload.count)
        XCTAssertEqual(info.maxChunkLength, OfflineNoteV2NfcApduProtocol.androidSafeChunkBytes)
        XCTAssertTrue(OfflineNoteV2NfcApduProtocol.payloadDigestMatches(payload, expectedSha256: info.sha256))

        let androidApdus = try OfflineNoteV2TransferHandoff.nfcPaymentTokenWriteAPDUs(for: token)
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.parseCommand(androidApdus.first), .writeMeta(kind: .paymentToken, payloadLength: payload.count, sha256: info.sha256))
        for apdu in androidApdus.dropFirst().dropLast() {
            guard case let .writeChunk(_, bytes) = OfflineNoteV2NfcApduProtocol.parseCommand(apdu) else {
                return XCTFail("Expected write chunk APDU")
            }
            XCTAssertLessThanOrEqual(bytes.count, OfflineNoteV2NfcApduProtocol.androidSafeChunkBytes)
        }
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.parseCommand(androidApdus.last), .commit)

        let fastPayload = Data(repeating: 0x5A, count: 512)
        let fastApdu = try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(offset: 1_024, bytes: fastPayload)
        XCTAssertEqual(
            Data(fastApdu.prefix(7)),
            Data([0x80, 0x21, 0x04, 0x00, 0x00, 0x02, 0x00])
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(fastApdu),
            .writeChunk(offset: 1_024, bytes: fastPayload)
        )
        let fastRead = try OfflineNoteV2NfcApduProtocol.readChunkAPDUData(
            offset: 256,
            length: OfflineNoteV2NfcApduProtocol.maxExtendedReadChunkBytes
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(fastRead),
            .readChunk(offset: 256, requestedLength: OfflineNoteV2NfcApduProtocol.maxExtendedReadChunkBytes)
        )
    }

    func testOfflineNoteV2TransportWireFormatMatchesSharedFixture() throws {
        let fixture = try Self.loadFixture()
        let token = try OfflineNoteV2PaymentTokenCodec.decodeNorito(Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64))
        let payload = try OfflineNoteV2TransferHandoff.rawPaymentTokenBytes(for: token)
        let writeApdus = try OfflineNoteV2TransferHandoff.nfcPaymentTokenWriteAPDUs(for: token)
        let readApdus = try OfflineNoteV2NfcApduProtocol.readPayloadAPDUs(payloadLength: payload.count)
        let nearbyBytes = try OfflineNoteV2TransferHandoff.nearbyPaymentEnvelopeBytes(for: token)

        XCTAssertEqual(payload.count, 2_416)
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.selectAidAPDUData().hexLowercased(), "00a4040007f049524f48413200")
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.getInfoAPDUData().hexLowercased(), "8010000000")
        XCTAssertEqual(
            try OfflineNoteV2NfcApduProtocol.encodeInfo(kind: .paymentToken, payloadBytes: payload).hexLowercased(),
            "01020000097000f044c7349a978489568f9e4de6035df214b471571646fb8a6dec4d2c026aca1a5c"
        )
        XCTAssertEqual(
            try OfflineNoteV2NfcApduProtocol.writeMetaAPDUData(kind: .paymentToken, payloadBytes: payload).hexLowercased(),
            "802000002601020000097044c7349a978489568f9e4de6035df214b471571646fb8a6dec4d2c026aca1a5c"
        )
        XCTAssertEqual(writeApdus.count, 13)
        XCTAssertEqual(writeApdus[0].hexLowercased(), "802000002601020000097044c7349a978489568f9e4de6035df214b471571646fb8a6dec4d2c026aca1a5c")
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.sha256(writeApdus[1]).hexLowercased(), "4037d861f58cb4820507bd2fe905e395dfc326e93613eb2dd885ba0235cfd053")
        XCTAssertEqual(writeApdus[writeApdus.count - 2].hexLowercased(), "802109601063746f722d61756469742d70726f6f66")
        XCTAssertEqual(writeApdus.last?.hexLowercased(), "8022000000")
        XCTAssertEqual(readApdus.count, 11)
        XCTAssertEqual(readApdus.first?.hexLowercased(), "80110000f0")
        XCTAssertEqual(nearbyBytes.count, 3_335)
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.sha256(nearbyBytes).hexLowercased(), "ce3207d3c55c3d89fc91012bb96546ea7ed71617545bc90b266a3c7bd67aec5c")
    }

    func testOfflineNoteV2NfcApduProtocolRejectsAdversarialPayloadsBeforeCommit() throws {
        let payload = Data("offline-payment".utf8)
        let info = try XCTUnwrap(
            OfflineNoteV2NfcApduProtocol.decodeInfo(
                try OfflineNoteV2NfcApduProtocol.encodeInfo(kind: .receiptAck, payloadBytes: payload)
            )
        )
        let assembler = try OfflineNoteV2NfcPayloadAssembler(info: info)

        XCTAssertFalse(assembler.write(offset: payload.count - 2, chunk: Data(repeating: 0x01, count: 4)))
        XCTAssertTrue(assembler.write(offset: 0, chunk: Data(payload.prefix(6))))
        XCTAssertTrue(assembler.write(offset: 0, chunk: Data(payload.prefix(6))))
        XCTAssertFalse(assembler.write(offset: 0, chunk: Data("OFFLIN".utf8)))
        XCTAssertThrowsError(try assembler.commit()) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .incompletePayload)
        }
        XCTAssertTrue(assembler.write(offset: 6, chunk: Data(payload.dropFirst(6))))
        XCTAssertEqual(try assembler.commit(), payload)

        var oversizedInfo = try OfflineNoteV2NfcApduProtocol.encodeInfo(kind: .paymentToken, payloadBytes: payload)
        let oversized = OfflineNoteV2NfcApduProtocol.maxIncomingPayloadBytes + 1
        oversizedInfo[oversizedInfo.startIndex + 2] = UInt8((oversized >> 24) & 0xff)
        oversizedInfo[oversizedInfo.startIndex + 3] = UInt8((oversized >> 16) & 0xff)
        oversizedInfo[oversizedInfo.startIndex + 4] = UInt8((oversized >> 8) & 0xff)
        oversizedInfo[oversizedInfo.startIndex + 5] = UInt8(oversized & 0xff)
        XCTAssertNil(OfflineNoteV2NfcApduProtocol.decodeInfo(oversizedInfo))

        let badAssembler = try OfflineNoteV2NfcPayloadAssembler(
            kind: .paymentToken,
            expectedLength: payload.count,
            expectedSha256: Data(repeating: 0x00, count: 32)
        )
        XCTAssertTrue(badAssembler.write(offset: 0, chunk: payload))
        XCTAssertThrowsError(try badAssembler.commit()) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .checksumMismatch)
        }
        XCTAssertThrowsError(
            try OfflineNoteV2NfcPayloadAssembler(
                kind: .paymentToken,
                expectedLength: OfflineNoteV2NfcApduProtocol.maxIncomingPayloadBytes + 1,
                expectedSha256: Data(repeating: 0x00, count: 32)
            )
        )
    }

    func testOfflineNoteV2NfcApduProtocolRejectsMalformedCommandsAndBounds() throws {
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.parseCommand(nil), .invalid)
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x00])), .invalid)
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x00, 0xA4, 0x04, 0x00, 0x01, 0xFF, 0x00])),
            .unsupported
        )
        var selectWithNonZeroLe = OfflineNoteV2NfcApduProtocol.selectAidAPDUData()
        selectWithNonZeroLe[selectWithNonZeroLe.index(before: selectWithNonZeroLe.endIndex)] = 0x01
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.parseCommand(selectWithNonZeroLe), .unsupported)
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x81, 0x10, 0x00, 0x00, 0x00])),
            .unsupported
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x10, 0x00, 0x01, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x10, 0x00, 0x00, 0x01])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x10, 0x00, 0x00, 0x01, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x11, 0x00, 0x00, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x11, 0x00, 0x00, 0x00, 0x00, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x20, 0x00, 0x00, 0x01, 0x01])),
            .invalid
        )
        var writeMetaWithOffset = try OfflineNoteV2NfcApduProtocol.writeMetaAPDUData(
            kind: .receiptAck,
            payloadBytes: Data([0x01])
        )
        writeMetaWithOffset[writeMetaWithOffset.startIndex + 3] = 0x01
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.parseCommand(writeMetaWithOffset), .invalid)
        let zeroLengthMeta = Data([0x01, OfflineNoteV2NfcPayloadKind.paymentToken.rawValue, 0x00, 0x00, 0x00, 0x00])
            + Data(repeating: 0x00, count: 32)
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x20, 0x00, 0x00, UInt8(zeroLengthMeta.count)]) + zeroLengthMeta),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x21, 0x00, 0x00, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x21, 0x00, 0x00, 0x02, 0x01])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x22, 0x00, 0x00, 0x01, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x22, 0x01, 0x00, 0x00])),
            .invalid
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(Data([0x80, 0x22, 0x00, 0x00, 0x01])),
            .invalid
        )

        XCTAssertThrowsError(try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(offset: 0x1_0000, bytes: Data([0x01])))
        XCTAssertThrowsError(try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(offset: 0, bytes: Data()))
        let rangePayload = Data([0x01, 0x02])
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: -1..<1
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: 1..<3
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: 3..<4
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: Int.min..<0
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: 0..<Int.max
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: rangePayload.startIndex..<rangePayload.startIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidChunkLength)
        }
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: rangePayload.endIndex..<rangePayload.endIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidChunkLength)
        }
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: -1,
                payloadBytes: rangePayload,
                range: rangePayload.startIndex..<rangePayload.endIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: Int.min,
                payloadBytes: rangePayload,
                range: rangePayload.startIndex..<rangePayload.endIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidOffset)
        }
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 0x1_0000,
                payloadBytes: rangePayload,
                range: rangePayload.startIndex..<rangePayload.startIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidOffset)
        }
        let oversizedChunkPayload = Data(
            repeating: 0xA5,
            count: OfflineNoteV2NfcApduProtocol.maxExtendedWriteChunkBytes + 1
        )
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: oversizedChunkPayload,
                range: oversizedChunkPayload.startIndex..<oversizedChunkPayload.endIndex
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidChunkLength)
        }
        var shiftedOversizedPayload = Data([0x00])
        shiftedOversizedPayload.append(oversizedChunkPayload)
        shiftedOversizedPayload.append(0xFF)
        let shiftedOversizedStart = shiftedOversizedPayload.index(after: shiftedOversizedPayload.startIndex)
        let shiftedOversizedEnd = shiftedOversizedPayload.index(before: shiftedOversizedPayload.endIndex)
        let shiftedOversizedRange = shiftedOversizedStart..<shiftedOversizedEnd
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: shiftedOversizedPayload,
                range: shiftedOversizedRange
            )
        ) { error in
            XCTAssertEqual(error as? OfflineNoteV2NfcApduError, .invalidChunkLength)
        }
        let shortHeaderPayload = Data(repeating: 0x33, count: Int(UInt8.max))
        let shortHeaderApdu = try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
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
        let shiftedShortHeaderApdu = try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
            offset: 0x1234,
            payloadBytes: shiftedShortHeaderPayload,
            range: shiftedShortHeaderRange
        )
        XCTAssertEqual(Data(shiftedShortHeaderApdu.prefix(5)), Data([0x80, 0x21, 0x12, 0x34, 0xFF]))
        XCTAssertEqual(shiftedShortHeaderApdu.count, 5 + Int(UInt8.max))

        let extendedBoundaryPayload = Data(repeating: 0x44, count: Int(UInt8.max) + 1)
        let extendedBoundaryApdu = try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
            offset: 0,
            payloadBytes: extendedBoundaryPayload,
            range: extendedBoundaryPayload.startIndex..<extendedBoundaryPayload.endIndex
        )
        XCTAssertEqual(
            Data(extendedBoundaryApdu.prefix(7)),
            Data([0x80, 0x21, 0x00, 0x00, 0x00, 0x01, 0x00])
        )
        XCTAssertEqual(extendedBoundaryApdu.count, 7 + Int(UInt8.max) + 1)

        let maxChunkLength = OfflineNoteV2NfcApduProtocol.maxExtendedWriteChunkBytes
        let maxChunkPayload = Data(repeating: 0x5C, count: maxChunkLength)
        let maxChunkApdu = try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
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
        let shiftedMaxChunkApdu = try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
            offset: 3,
            payloadBytes: shiftedPayload,
            range: shiftedPayload.index(after: shiftedPayload.startIndex)..<shiftedPayload.index(before: shiftedPayload.endIndex)
        )
        XCTAssertEqual(shiftedMaxChunkApdu.count, 7 + maxChunkLength)
        XCTAssertEqual(shiftedMaxChunkApdu.suffix(2), Data([0x5C, 0x5C]))
        XCTAssertEqual(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 7,
                payloadBytes: rangePayload,
                range: rangePayload.startIndex..<rangePayload.endIndex
            ),
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(offset: 7, bytes: rangePayload)
        )
        XCTAssertEqual(
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: 0,
                payloadBytes: rangePayload,
                range: rangePayload.index(before: rangePayload.endIndex)..<rangePayload.endIndex
            ),
            try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(offset: 0, bytes: Data([0x02]))
        )
        XCTAssertThrowsError(try OfflineNoteV2NfcApduProtocol.readChunkAPDUData(offset: 0, length: 0))
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.readChunkAPDUData(
                offset: 0,
                length: OfflineNoteV2NfcApduProtocol.maxExtendedReadChunkBytes + 1
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.writePayloadAPDUs(
                kind: .paymentToken,
                payloadBytes: Data([0x01]),
                maxChunkLength: 0
            )
        )
        XCTAssertThrowsError(try OfflineNoteV2NfcApduProtocol.readPayloadAPDUs(payloadLength: 0))
        XCTAssertThrowsError(
            try OfflineNoteV2NfcApduProtocol.readPayloadAPDUs(
                payloadLength: 1,
                maxChunkLength: OfflineNoteV2NfcApduProtocol.maxExtendedReadChunkBytes + 1
            )
        )

        let response = OfflineNoteV2NfcApduProtocol.response(Data([0xAA, 0xBB]))
        XCTAssertEqual(response, Data([0xAA, 0xBB, 0x90, 0x00]))
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.responseStatus(response), 0x9000)
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.responseStatus(Data([0x90])), nil)
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.responseData(response), Data([0xAA, 0xBB]))
        XCTAssertEqual(OfflineNoteV2NfcApduProtocol.responseData(Data([0x90])), Data())

        let assembler = try OfflineNoteV2NfcPayloadAssembler(
            kind: .receiptAck,
            expectedLength: 4,
            expectedSha256: OfflineNoteV2NfcApduProtocol.sha256(Data([0x01, 0x02, 0x03, 0x04]))
        )
        XCTAssertFalse(assembler.write(offset: Int.max, chunk: Data([0x01])))
        XCTAssertFalse(assembler.write(offset: 4, chunk: Data([0x01])))
        XCTAssertFalse(assembler.write(offset: -1, chunk: Data([0x01])))
        XCTAssertFalse(assembler.write(offset: 0, chunk: Data()))
        XCTAssertTrue(assembler.write(offset: 0, chunk: Data([0x01, 0x02])))
        XCTAssertTrue(assembler.write(offset: 1, chunk: Data([0x02, 0x03])))
        XCTAssertFalse(assembler.write(offset: 1, chunk: Data([0x09, 0x09])))
    }

    func testOfflineNoteV2NfcApduRangeWriteCopiesOnlyRequestedWindow() throws {
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
            let apdu = try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
                offset: testCase.offset,
                payloadBytes: payload,
                range: testCase.range
            )

            guard case let .writeChunk(offset, bytes) = OfflineNoteV2NfcApduProtocol.parseCommand(apdu) else {
                XCTFail("expected write chunk command")
                continue
            }
            XCTAssertEqual(offset, testCase.offset)
            XCTAssertEqual(bytes, testCase.expected)
            XCTAssertFalse(bytes.contains(0xA0))
            XCTAssertFalse(bytes.contains(0xB0))
        }
    }

    func testOfflineNoteV2NfcApduRangeWriteCopiesExtendedWindowWithoutSentinels() throws {
        var payload = Data([0xA0])
        payload.append(Data(repeating: 0x7B, count: Int(UInt8.max) + 1))
        payload.append(0xB0)
        let range = payload.index(after: payload.startIndex)..<payload.index(before: payload.endIndex)

        let apdu = try OfflineNoteV2NfcApduProtocol.writeChunkAPDUData(
            offset: 0x100,
            payloadBytes: payload,
            range: range
        )

        XCTAssertEqual(
            Data(apdu.prefix(7)),
            Data([0x80, 0x21, 0x01, 0x00, 0x00, 0x01, 0x00])
        )
        XCTAssertEqual(
            OfflineNoteV2NfcApduProtocol.parseCommand(apdu),
            .writeChunk(offset: 0x100, bytes: Data(repeating: 0x7B, count: Int(UInt8.max) + 1))
        )
    }

    func testOfflineNoteV2NearbyEnvelopeRoundTripsPairingPaymentAndAck() throws {
        let fixture = try Self.loadFixture()
        let token = try OfflineNoteV2PaymentTokenCodec.decodeNorito(Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64))
        let challenge = try OfflineNoteV2NearbyPairingChallenge(assetName: " nearby_pairing_bird ")
        let challengeEnvelope = try OfflineNoteV2NearbyEnvelope(
            kind: .challenge,
            payload: Data("receive-challenge".utf8),
            contentType: OfflineNoteV2TransferHandoff.receiveChallengeContentType,
            pairingChallenge: challenge
        )
        let paymentBytes = try OfflineNoteV2TransferHandoff.nearbyPaymentEnvelopeBytes(for: token)
        let paymentEnvelope = try OfflineNoteV2NearbyEnvelope.decode(paymentBytes)
        let ackEnvelope = try OfflineNoteV2NearbyEnvelope(
            kind: .receiptAck,
            payload: Data("accepted-locally".utf8),
            contentType: OfflineNoteV2TransferHandoff.receiptAckContentType
        )

        XCTAssertEqual(try OfflineNoteV2NearbyEnvelope.decode(challengeEnvelope.encoded()).pairingChallenge, challenge)
        XCTAssertEqual(paymentEnvelope.kind, .payment)
        XCTAssertEqual(try paymentEnvelope.paymentToken().tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(try OfflineNoteV2TransferHandoff.decodeNearbyPaymentToken(from: paymentBytes).tokenIdHex, token.tokenIdHex)
        XCTAssertEqual(try OfflineNoteV2NearbyEnvelope.decode(ackEnvelope.encoded()).payload, Data("accepted-locally".utf8))

        let legacyPairing = Data(
            #"{"version":1,"kind":"challenge","payload":"cmVjZWl2ZS1jaGFsbGVuZ2U","contentType":"application/vnd.iroha.offline.receive-challenge-v1+octet-stream","pairingChallenge":{"assetName":"nearby_pairing_bird"}}"#.utf8
        )
        XCTAssertEqual(try OfflineNoteV2NearbyEnvelope.decode(legacyPairing).pairingChallenge, challenge)
    }

    func testOfflineNoteV2NearbyEnvelopeRejectsAdversarialMessages() throws {
        let fixture = try Self.loadFixture()
        let tokenPayload = try Self.base64(fixture.sdkInterop.paymentTokenNoritoBase64)
        let pairing = try OfflineNoteV2NearbyPairingChallenge(assetName: "nearby_pairing_mask")

        XCTAssertThrowsError(try OfflineNoteV2NearbyPairingChallenge(assetName: "nearby_pairing_mask<script>"))
        XCTAssertThrowsError(
            try OfflineNoteV2NearbyEnvelope(
                kind: .challenge,
                payload: Data("challenge".utf8),
                contentType: OfflineNoteV2TransferHandoff.receiveChallengeContentType
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteV2NearbyEnvelope(
                kind: .challenge,
                payload: Data("challenge".utf8),
                contentType: OfflineNoteV2TransferHandoff.receiptAckContentType,
                pairingChallenge: pairing
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteV2NearbyEnvelope(
                kind: .payment,
                payload: tokenPayload,
                contentType: OfflineNoteV2TransferHandoff.paymentTokenContentType,
                pairingChallenge: pairing
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteV2NearbyEnvelope(
                kind: .payment,
                payload: Data(repeating: 0x01, count: OfflineNoteV2NfcApduProtocol.maxIncomingPayloadBytes + 1),
                contentType: OfflineNoteV2TransferHandoff.paymentTokenContentType
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteV2NearbyEnvelope(
                kind: .receiptAck,
                payload: Data("ok".utf8),
                contentType: OfflineNoteV2TransferHandoff.receiveChallengeContentType
            )
        )

        let unsupportedVersion = Data(
            #"{"version":2,"kind":"payment","payload":"AQID","contentType":"application/vnd.iroha.offline.payment-token-v2+norito"}"#.utf8
        )
        let fractionalVersion = Data(
            #"{"version":1.5,"kind":"challenge","payload":"YQ","contentType":"application/vnd.iroha.offline.receive-challenge-v1+octet-stream","pairingChallenge":"nearby_pairing_bird"}"#.utf8
        )
        let unknownField = Data(
            #"{"version":1,"kind":"payment","payload":"AQID","contentType":"application/vnd.iroha.offline.payment-token-v2+norito","extra":true}"#.utf8
        )
        let challengeContentTypeDowngrade = Data(
            #"{"version":1,"kind":"challenge","payload":"YQ","contentType":"application/vnd.iroha.offline.receipt-ack-v1+octet-stream","pairingChallenge":"nearby_pairing_bird"}"#.utf8
        )
        let ackContentTypeDowngrade = Data(
            #"{"version":1,"kind":"receipt_ack","payload":"b2s","contentType":"application/vnd.iroha.offline.receive-challenge-v1+octet-stream"}"#.utf8
        )
        let paddedPayload = Data(
            #"{"version":1,"kind":"challenge","payload":"YQ==","contentType":"application/vnd.iroha.offline.receive-challenge-v1+octet-stream","pairingChallenge":"nearby_pairing_bird"}"#.utf8
        )
        XCTAssertThrowsError(try OfflineNoteV2NearbyEnvelope.decode(unsupportedVersion))
        XCTAssertThrowsError(try OfflineNoteV2NearbyEnvelope.decode(fractionalVersion))
        XCTAssertThrowsError(try OfflineNoteV2NearbyEnvelope.decode(unknownField))
        XCTAssertThrowsError(try OfflineNoteV2NearbyEnvelope.decode(challengeContentTypeDowngrade))
        XCTAssertThrowsError(try OfflineNoteV2NearbyEnvelope.decode(ackContentTypeDowngrade))
        XCTAssertThrowsError(try OfflineNoteV2NearbyEnvelope.decode(paddedPayload))

        let topLevelArray = Data(#"[]"#.utf8)
        let invalidBase64Payload = Data(
            #"{"version":1,"kind":"challenge","payload":"!!!!","contentType":"application/vnd.iroha.offline.receive-challenge-v1+octet-stream","pairingChallenge":"nearby_pairing_bird"}"#.utf8
        )
        let badPairingObject = Data(
            #"{"version":1,"kind":"challenge","payload":"YQ","contentType":"application/vnd.iroha.offline.receive-challenge-v1+octet-stream","pairingChallenge":{"assetName":1}}"#.utf8
        )
        let smuggledPairingObject = Data(
            #"{"version":1,"kind":"challenge","payload":"YQ","contentType":"application/vnd.iroha.offline.receive-challenge-v1+octet-stream","pairingChallenge":{"assetName":"nearby_pairing_bird","extra":true}}"#.utf8
        )
        let ackWithPairing = Data(
            #"{"version":1,"kind":"receipt_ack","payload":"b2s","contentType":"application/vnd.iroha.offline.receipt-ack-v1+octet-stream","pairingChallenge":"nearby_pairing_bird"}"#.utf8
        )
        XCTAssertThrowsError(try OfflineNoteV2NearbyEnvelope.decode(topLevelArray))
        XCTAssertThrowsError(try OfflineNoteV2NearbyEnvelope.decode(invalidBase64Payload))
        XCTAssertThrowsError(try OfflineNoteV2NearbyEnvelope.decode(badPairingObject))
        XCTAssertThrowsError(try OfflineNoteV2NearbyEnvelope.decode(smuggledPairingObject))
        XCTAssertThrowsError(try OfflineNoteV2NearbyEnvelope.decode(ackWithPairing))
        XCTAssertThrowsError(
            try OfflineNoteV2NearbyEnvelope(
                kind: .payment,
                payload: Data([0x01, 0x02, 0x03]),
                contentType: OfflineNoteV2TransferHandoff.paymentTokenContentType
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteV2NearbyEnvelope(
                kind: .receiptAck,
                payload: Data(),
                contentType: OfflineNoteV2TransferHandoff.receiptAckContentType
            )
        )
    }

    func testOfflineNoteV2WalletAcceptsCanonicalSdkInteropPaymentToken() throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let recipientStore = InMemoryOfflineNoteV2Store()
        let recipientWallet = OfflineNoteV2Wallet(
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

        let token = try OfflineNoteV2PaymentTokenCodec.decodeNorito(
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

    func testOfflineNoteV2WalletNoteJsonCodecRoundTripsFixtureNote() throws {
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)

        let decoded = try OfflineNoteV2WalletNoteJsonCodec.decode(
            OfflineNoteV2WalletNoteJsonCodec.encode(note)
        )

        XCTAssertEqual(decoded, note)
        XCTAssertEqual(try decoded.keyCertificate.noritoEncoded(), try note.keyCertificate.noritoEncoded())

        var spendPendingObject = try XCTUnwrap(
            JSONSerialization.jsonObject(with: OfflineNoteV2WalletNoteJsonCodec.encode(note)) as? [String: Any]
        )
        spendPendingObject["state"] = "spendPending"
        let migratedSpent = try OfflineNoteV2WalletNoteJsonCodec.decode(
            JSONSerialization.data(withJSONObject: spendPendingObject, options: [.sortedKeys])
        )
        XCTAssertEqual(migratedSpent.state, .spent)

        var changePendingObject = spendPendingObject
        changePendingObject["state"] = "CHANGE_PENDING"
        let migratedChange = try OfflineNoteV2WalletNoteJsonCodec.decode(
            JSONSerialization.data(withJSONObject: changePendingObject, options: [.sortedKeys])
        )
        XCTAssertEqual(migratedChange.state, .spendable)
    }

    func testOfflineNoteV2KeychainStoreRejectsInvalidLabel() {
        XCTAssertThrowsError(try OfflineNoteV2KeychainStore(label: "bad/label")) { error in
            XCTAssertEqual(error as? OfflineNoteV2KeychainStoreError, .invalidLabel("bad/label"))
        }
    }

    func testOfflineNoteV2KeychainStoreMovesMetadataBeforeDeletingOldCollection() throws {
        let label = "atomic-order-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteV2KeychainBacking()
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

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

    func testOfflineNoteV2KeychainStoreKeepsOldRevisionWhenMetadataSaveFails() throws {
        let label = "metadata-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteV2KeychainBacking()
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

        try store.upsert(note)
        backing.operations.removeAll()
        backing.saveFailures.insert("\(label).meta")

        let updated = try note.withState(.spent, updatedAtMs: note.updatedAtMs + 1)
        XCTAssertThrowsError(try store.upsert(updated)) { error in
            XCTAssertEqual(error as? OfflineNoteV2KeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertFalse(backing.operations.contains("delete:\(label).rev.1"))
        XCTAssertNotNil(backing.values["\(label).rev.2"])

        backing.saveFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spendable)
        XCTAssertEqual(loaded.updatedAtMs, note.updatedAtMs)
    }

    func testOfflineNoteV2KeychainStoreKeepsOldRevisionWhenCollectionSaveFails() throws {
        let label = "collection-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteV2KeychainBacking()
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

        try store.upsert(note)
        backing.operations.removeAll()
        backing.saveFailures.insert("\(label).rev.2")

        let updated = try note.withState(.spent, updatedAtMs: note.updatedAtMs + 1)
        XCTAssertThrowsError(try store.upsert(updated)) { error in
            XCTAssertEqual(error as? OfflineNoteV2KeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertNil(backing.values["\(label).rev.2"])
        XCTAssertFalse(backing.operations.contains("save:\(label).meta"))

        backing.saveFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spendable)
    }

    func testOfflineNoteV2KeychainStoreKeepsLegacyCollectionWhenMetadataSaveFails() throws {
        let label = "legacy-metadata-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteV2KeychainBacking()
        backing.values[label] = try Self.storedCollectionData(notes: [note])
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)
        backing.saveFailures.insert("\(label).meta")

        let updated = try note.withState(.spent, updatedAtMs: note.updatedAtMs + 1)
        XCTAssertThrowsError(try store.upsert(updated)) { error in
            XCTAssertEqual(error as? OfflineNoteV2KeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertFalse(backing.operations.contains("delete:\(label)"))

        backing.saveFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spendable)
    }

    func testOfflineNoteV2KeychainStoreUsesNewRevisionWhenOldCollectionDeleteFails() throws {
        let label = "delete-old-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteV2KeychainBacking()
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

        try store.upsert(note)
        backing.operations.removeAll()
        backing.deleteFailures.insert("\(label).rev.1")

        let updated = try note.withState(.spent, updatedAtMs: note.updatedAtMs + 1)
        XCTAssertThrowsError(try store.upsert(updated)) { error in
            XCTAssertEqual(error as? OfflineNoteV2KeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertNotNil(backing.values["\(label).rev.1"])

        backing.deleteFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spent)
    }

    func testOfflineNoteV2KeychainStoreRejectsMetadataPointerToMissingCollection() throws {
        let label = "missing-collection-test"
        let backing = RecordingOfflineNoteV2KeychainBacking()
        backing.values["\(label).meta"] = try Self.storedMetadataData(revision: 99)
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

        XCTAssertThrowsError(try store.listNotes()) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2KeychainStoreError,
                .corrupt("collection revision is missing")
            )
        }
    }

    func testOfflineNoteV2KeychainStoreDoesNotFallBackToLegacyCollectionWhenMetadataExists() throws {
        let label = "metadata-blocks-legacy-fallback-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteV2KeychainBacking()
        backing.values[label] = try Self.storedCollectionData(notes: [note])
        backing.values["\(label).meta"] = try Self.storedMetadataData(revision: 2)
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

        XCTAssertThrowsError(try store.listNotes()) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2KeychainStoreError,
                .corrupt("collection revision is missing")
            )
        }
    }

    func testOfflineNoteV2KeychainStoreRejectsInvalidMetadataShapes() throws {
        for (label, metadata, expected) in [
            (
                "metadata-version-test",
                try Self.storedMetadataData(version: 2, revision: 1),
                OfflineNoteV2KeychainStoreError.corrupt("unsupported metadata")
            ),
            (
                "metadata-zero-revision-test",
                try Self.storedMetadataData(version: 1, revision: 0),
                OfflineNoteV2KeychainStoreError.corrupt("unsupported metadata")
            )
        ] {
            let backing = RecordingOfflineNoteV2KeychainBacking()
            backing.values["\(label).meta"] = metadata
            let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

            XCTAssertThrowsError(try store.listNotes()) { error in
                XCTAssertEqual(error as? OfflineNoteV2KeychainStoreError, expected)
            }
        }
    }

    func testOfflineNoteV2KeychainStoreRejectsInvalidMetadataJson() throws {
        let label = "metadata-json-test"
        let backing = RecordingOfflineNoteV2KeychainBacking()
        backing.values["\(label).meta"] = Data("{".utf8)
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

        XCTAssertThrowsError(try store.listNotes()) { error in
            guard case let OfflineNoteV2KeychainStoreError.corrupt(reason) = error else {
                return XCTFail("expected corrupt metadata, got \(error)")
            }
            XCTAssertTrue(reason.contains("failed to decode metadata"))
        }
    }

    func testOfflineNoteV2KeychainStoreRejectsCorruptCollections() throws {
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let validPayload = try OfflineNoteV2WalletNoteJsonCodec.encode(note).base64EncodedString()
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
            let backing = RecordingOfflineNoteV2KeychainBacking()
            backing.values[label] = try JSONSerialization.data(
                withJSONObject: collection,
                options: [.sortedKeys]
            )
            let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

            XCTAssertThrowsError(try store.listNotes()) { error in
                XCTAssertEqual(error as? OfflineNoteV2KeychainStoreError, .corrupt(reason))
            }
        }
    }

    func testOfflineNoteV2KeychainStoreRejectsInvalidCollectionJson() throws {
        let label = "collection-json-test"
        let backing = RecordingOfflineNoteV2KeychainBacking()
        backing.values[label] = Data("{".utf8)
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

        XCTAssertThrowsError(try store.listNotes()) { error in
            guard case let OfflineNoteV2KeychainStoreError.corrupt(reason) = error else {
                return XCTFail("expected corrupt collection, got \(error)")
            }
            XCTAssertTrue(reason.contains("failed to decode collection"))
        }
    }

    func testOfflineNoteV2KeychainStoreDoesNotDeleteCollectionWhenMetadataClearFails() throws {
        let label = "clear-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteV2KeychainBacking()
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

        try store.upsert(note)
        backing.operations.removeAll()
        backing.deleteFailures.insert("\(label).meta")

        XCTAssertThrowsError(try store.clear()) { error in
            XCTAssertEqual(error as? OfflineNoteV2KeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertFalse(backing.operations.contains("delete:\(label).rev.1"))

        backing.deleteFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spendable)
    }

    func testOfflineNoteV2KeychainStoreDoesNotResurrectRevisionWhenClearCollectionDeleteFails() throws {
        let label = "clear-collection-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteV2KeychainBacking()
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)

        try store.upsert(note)
        backing.operations.removeAll()
        backing.deleteFailures.insert("\(label).rev.1")

        XCTAssertThrowsError(try store.clear()) { error in
            XCTAssertEqual(error as? OfflineNoteV2KeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertNil(backing.values["\(label).meta"])
        XCTAssertNotNil(backing.values["\(label).rev.1"])

        backing.deleteFailures.removeAll()
        XCTAssertTrue(try store.listNotes().isEmpty)
    }

    func testOfflineNoteV2KeychainStoreUsesMetadataWhenLegacyDeleteFails() throws {
        let label = "legacy-delete-failure-test"
        let fixture = try Self.loadFixture()
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let note = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        let backing = RecordingOfflineNoteV2KeychainBacking()
        backing.values[label] = try Self.storedCollectionData(notes: [note])
        let store = try OfflineNoteV2KeychainStore(label: label, backing: backing)
        backing.deleteFailures.insert(label)

        let updated = try note.withState(.spent, updatedAtMs: note.updatedAtMs + 1)
        XCTAssertThrowsError(try store.upsert(updated)) { error in
            XCTAssertEqual(error as? OfflineNoteV2KeychainStoreError, .keychainFailure(-1))
        }
        XCTAssertNotNil(backing.values[label])
        XCTAssertNotNil(backing.values["\(label).meta"])

        backing.deleteFailures.removeAll()
        let loaded = try XCTUnwrap(try store.findNote(noteCommitment: note.noteCommitment))
        XCTAssertEqual(loaded.state, .spent)
        XCTAssertEqual(loaded.updatedAtMs, updated.updatedAtMs)
    }

    func testOfflineNoteV2WalletDerivationsMatchRustVectors() throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let recipientOutput = fixture.paymentToken.outputClaims[0]
        let changeOutput = fixture.paymentToken.outputClaims[1]

        let sourceCommitment = try OfflineNoteCommitmentPreimageV2(
            chainId: derivation.chainId,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount,
            noteSecret: Self.hex(derivation.sourceNoteSecretHex),
            origin: .issuerLoad(OfflineNoteIssuerLoadOriginV2(
                operationId: derivation.issuerLoadOperationId,
                lineageId: derivation.issuerLoadLineageId,
                localRevision: derivation.issuerLoadLocalRevision
            ))
        ).deriveNoteCommitment()
        XCTAssertEqual(sourceCommitment.hexLowercased(), derivation.sourceNoteCommitment)

        let inputNullifier = try OfflineNoteInputNullifierPreimageV2(
            chainId: derivation.chainId,
            sourceNoteCommitment: sourceCommitment,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            noteSecret: Self.hex(derivation.sourceNoteSecretHex)
        ).deriveInputNullifier()
        XCTAssertEqual(inputNullifier.hexLowercased(), derivation.inputNullifier)

        let recipientCommitment = try OfflineNoteCommitmentPreimageV2(
            chainId: derivation.chainId,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.recipientKeyCertificatePayloadHash),
            assetId: "\(recipientOutput.assetDefinitionId)#\(recipientOutput.accountId)",
            amount: recipientOutput.amount,
            noteSecret: Self.hex(derivation.recipientNoteSecretHex),
            origin: .p2pOutput(OfflineNoteP2pOutputOriginV2(
                paymentRequestId: derivation.paymentRequestId,
                outputIndex: 0
            ))
        ).deriveNoteCommitment()
        XCTAssertEqual(recipientCommitment.hexLowercased(), derivation.recipientOutputCommitment)

        let changeCommitment = try OfflineNoteCommitmentPreimageV2(
            chainId: derivation.chainId,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            assetId: "\(changeOutput.assetDefinitionId)#\(changeOutput.accountId)",
            amount: changeOutput.amount,
            noteSecret: Self.hex(derivation.changeNoteSecretHex),
            origin: .p2pOutput(OfflineNoteP2pOutputOriginV2(
                paymentRequestId: derivation.paymentRequestId,
                outputIndex: 1
            ))
        ).deriveNoteCommitment()
        XCTAssertEqual(changeCommitment.hexLowercased(), derivation.changeOutputCommitment)

        let tokenId = try OfflineNotePaymentTokenIdPreimageV2(
            chainId: derivation.chainId,
            paymentRequestId: derivation.paymentRequestId,
            createdAtMs: fixture.paymentToken.createdAtMs,
            tokenNonce: Self.hex(derivation.tokenNonceHex),
            senderKeyCertificatePayloadHash: Self.hex(derivation.senderKeyCertificatePayloadHash),
            inputNullifiers: [inputNullifier],
            outputCommitments: [recipientCommitment, changeCommitment]
        ).derivePaymentTokenId()
        XCTAssertEqual(tokenId.hexLowercased(), derivation.paymentTokenId)

        let redeemNullifier = try OfflineNoteInputNullifierPreimageV2(
            chainId: derivation.chainId,
            sourceNoteCommitment: recipientCommitment,
            ownerKeyCertificatePayloadHash: Self.hex(derivation.recipientKeyCertificatePayloadHash),
            noteSecret: Self.hex(derivation.recipientNoteSecretHex)
        ).deriveInputNullifier()
        XCTAssertEqual(redeemNullifier.hexLowercased(), derivation.redeemNullifier)
    }

    func testOfflineNoteV2PublicInputHashesMatchRustVectors() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)

        XCTAssertEqual(try audit.publicInputsHash().hexLowercased(), fixture.chainVectors.audit.publicInputsHash)
        XCTAssertEqual(try redeem.publicInputsHash().hexLowercased(), fixture.chainVectors.redeem.publicInputsHash)
        XCTAssertNoThrow(try audit.validateProofBinding())
        XCTAssertNoThrow(try redeem.validateProofBinding())
    }

    func testOfflineNoteV2WalletLoadDerivesCommitmentBeforeIssuerSubmission() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let loadContext = OfflineNoteV2LoadContext(
            operationId: derivation.issuerLoadOperationId,
            lineageId: derivation.issuerLoadLineageId,
            localRevision: derivation.issuerLoadLocalRevision,
            keyCertificate: senderCertificate
        )
        let issuerClient = RecordingIssuerClient(loadContext: loadContext)
        let wallet = OfflineNoteV2Wallet(
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
        let loadContext = OfflineNoteV2LoadContext(
            operationId: derivation.issuerLoadOperationId,
            lineageId: derivation.issuerLoadLineageId,
            localRevision: derivation.issuerLoadLocalRevision,
            keyCertificate: senderCertificate
        )
        let issuerClient = RecordingIssuerClient(loadContext: loadContext)
        let wallet = OfflineNoteV2Wallet(
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
        let deviceBinding = try OfflineNoteV2IssuerDeviceBinding(
            deviceId: "device-1",
            offlinePublicKey: offlinePublicKey,
            deviceBinding: [
                "device_id": "device-1",
                "offline_public_key": offlinePublicKey,
                "signature_base64": "nested-device-signature-is-not-body-auth",
            ]
        )
        OfflineIssuerURLProtocol.reset()
        OfflineIssuerURLProtocol.handler = { request in
            let body = try Self.requestBody(request)
            let response: [String: Any]
            switch request.url?.path {
            case "/v1/offline/v2/keys/refill":
                response = [
                    "operation_id": try Self.string(body, "operation_id"),
                    "lineage_state": Self.lineageState(revision: 0, balance: "0"),
                    "key_certificate": Self.certificateJSON(certificate, expiresAtMs: 1_700_000_060_000),
                    "key_certificates": [Self.certificateJSON(certificate, expiresAtMs: 1_700_000_060_000)],
                ]
            case "/v1/offline/v2/notes/issue":
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
                throw ToriiOfflineNoteV2IssuerClientError.invalidURL(request.url?.path ?? "")
            }
            return (200, try JSONSerialization.data(withJSONObject: response, options: [.sortedKeys]))
        }
        let configuration = URLSessionConfiguration.ephemeral
        configuration.protocolClasses = [OfflineIssuerURLProtocol.self]
        let session = URLSession(configuration: configuration)
        let client = ToriiOfflineNoteV2IssuerClient(
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
        let response = try await client.issueNote(OfflineNoteV2IssueRequest(
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
        XCTAssertEqual(requests[0].url?.path, "/v1/offline/v2/keys/refill")
        XCTAssertEqual(requests[1].url?.path, "/v1/offline/v2/notes/issue")
        for request in requests {
            XCTAssertFalse((request.allHTTPHeaderFields ?? [:]).keys.contains { $0.lowercased().hasPrefix("x-iroha-") })
        }
        let refillBody = try Self.requestBody(requests[0])
        XCTAssertEqual(try Self.string(refillBody, "account_id"), accountId)
        XCTAssertEqual(try Self.string(refillBody, "operation_id"), "operation-refill-1")
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
    /// `OFFLINE_V2_SIGNATURE_INVALID` (403). Base64 fields routinely contain `/`,
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
        let deviceBinding = try OfflineNoteV2IssuerDeviceBinding(
            deviceId: "device-1",
            offlinePublicKey: offlinePublicKey,
            deviceBinding: [
                "device_id": "device-1",
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
        let client = ToriiOfflineNoteV2IssuerClient(
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
    }

    func testOfflineNoteV2WalletLifecycleBuildsAuditAcceptAndRedeemTransactions() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let senderStore = InMemoryOfflineNoteV2Store()
        try senderStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let senderWallet = OfflineNoteV2Wallet(
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
        let recipientWallet = OfflineNoteV2Wallet(
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
        XCTAssertEqual(recipientSubmitter.redemptions.count, 1)
        XCTAssertEqual(
            try recipientSubmitter.redemptions[0].publicInputsHash().hexLowercased(),
            fixture.chainVectors.redeem.publicInputsHash
        )
    }

    func testOfflineNoteV2WalletSyncReconcilesPendingSpendChangeAndRedeemStates() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let senderStore = InMemoryOfflineNoteV2Store()
        try senderStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let syncResolver = RecordingSyncResolver(resolutions: [
            derivation.sourceNoteCommitment: .spent,
            derivation.changeOutputCommitment: .spendable
        ])
        let senderWallet = OfflineNoteV2Wallet(
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
        let recipientWallet = OfflineNoteV2Wallet(
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

    func testOfflineNoteV2WalletRejectsDuplicateTokenAndAlreadyPendingInputs() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let senderStore = InMemoryOfflineNoteV2Store()
        try senderStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let senderWallet = OfflineNoteV2Wallet(
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
        let recipientWallet = OfflineNoteV2Wallet(
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

    func testOfflineNoteV2WalletRejectsAdversarialCertificateBindings() throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let senderAccountId = Self.accountId(fromAssetId: fixture.chainVectors.issue.assetId)
        let assetDefinitionId = Self.assetDefinition(fromAssetId: fixture.chainVectors.issue.assetId)

        let defaultRejectingWallet = OfflineNoteV2Wallet(
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
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .certificateVerificationFailed)
        }
        let wrongAccountReceiveWallet = OfflineNoteV2Wallet(
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
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .certificateVerificationFailed)
        }

        let senderStore = InMemoryOfflineNoteV2Store()
        try senderStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let senderWallet = OfflineNoteV2Wallet(
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
        let recipientWallet = OfflineNoteV2Wallet(
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
        let accountSubstitution = try OfflineNoteV2ReceiveRequest(
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
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .certificateVerificationFailed)
        }
        let chainSubstitution = try OfflineNoteV2ReceiveRequest(
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
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .chainMismatch)
        }
        let assetOwnerSubstitution = try OfflineNoteV2ReceiveRequest(
            chainId: receiveRequest.chainId,
            paymentRequestId: receiveRequest.paymentRequestId,
            accountId: receiveRequest.accountId,
            assetDefinitionId: receiveRequest.assetDefinitionId,
            assetId: "\(receiveRequest.assetDefinitionId)#\(senderAccountId)",
            amount: receiveRequest.amount,
            keyCertificate: receiveRequest.keyCertificate,
            outputCommitment: receiveRequest.outputCommitment
        )
        let assetOwnerSubstitutionStore = InMemoryOfflineNoteV2Store()
        try assetOwnerSubstitutionStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let assetOwnerSubstitutionSender = OfflineNoteV2Wallet(
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
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .certificateVerificationFailed)
        }

        let forgedInputStore = InMemoryOfflineNoteV2Store()
        try forgedInputStore.upsert(try Self.sourceWalletNote(
            fixture,
            certificate: Self.tamperedSignatureCertificate(senderCertificate)
        ))
        let forgedInputWallet = OfflineNoteV2Wallet(
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
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .certificateVerificationFailed)
        }
        let wrongAccountInputStore = InMemoryOfflineNoteV2Store()
        try wrongAccountInputStore.upsert(try Self.sourceWalletNote(fixture, certificate: recipientCertificate))
        let wrongAccountInputWallet = OfflineNoteV2Wallet(
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
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .certificateVerificationFailed)
        }
        let commitmentSubstitutionStore = InMemoryOfflineNoteV2Store()
        try commitmentSubstitutionStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let commitmentSubstitutionSender = OfflineNoteV2Wallet(
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
        let commitmentSubstitution = try OfflineNoteV2ReceiveRequest(
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
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .noPendingOutput)
        }
        let forgedOutputAmount = receiveRequest.amount == "1" ? "2" : "1"
        let amountSubstitutionStore = InMemoryOfflineNoteV2Store()
        try amountSubstitutionStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let amountSubstitutionSender = OfflineNoteV2Wallet(
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
        let amountSubstitution = try OfflineNoteV2ReceiveRequest(
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
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .outputMismatch)
        }

        let token = try senderWallet.pay(receiveRequest)
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingChainId(token, chainId: "\(token.chainId)-evil")
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .chainMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingPaymentRequestId(token, paymentRequestId: "\(token.paymentRequestId)-evil")
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .outputMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingTopLevelTokenId(token)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2PaymentTokenCodecError, .tokenIdMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingAuditTokenId(token)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2PaymentTokenCodecError, .tokenIdMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingFirstOutputAmountWithoutProofRebind(token, amount: forgedOutputAmount)
        )) { error in
            guard case .proofPublicInputsHashMismatch = error as? OfflineNoteV2Error else {
                XCTFail("expected proof public input mismatch, got \(error)")
                return
            }
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingFirstOutputAmount(token, amount: forgedOutputAmount)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .outputMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingFirstOutputAsset(
                token,
                assetId: "\(receiveRequest.assetId)#dataspace:1"
            )
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .outputMismatch)
        }
        XCTAssertGreaterThanOrEqual(token.audit.outputClaims.count, 2)
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReversingOutputs(token)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .outputMismatch)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenDroppingFirstOutput(token)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .noPendingOutput)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingFirstOutputCertificate(token, certificate: senderCertificate)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .certificateVerificationFailed)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingLastOutputCertificate(token, certificate: recipientCertificate)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .certificateVerificationFailed)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingFirstInputClaimHash(
                token,
                keyCertificatePayloadHash: try recipientCertificate.payloadHash()
            )
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .certificateVerificationFailed)
        }
        XCTAssertThrowsError(try recipientWallet.accept(
            Self.paymentTokenReplacingSenderCertificate(token, certificate: recipientCertificate)
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2WalletError, .certificateVerificationFailed)
        }

        let accepted = try recipientWallet.accept(token)
        XCTAssertEqual(accepted.state, .spendable)
    }

    func testOfflineNoteV2WalletSyncReconcilesFailedAuditAndRedeemOutcomes() async throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let senderCertificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let senderStore = InMemoryOfflineNoteV2Store()
        try senderStore.upsert(try Self.sourceWalletNote(fixture, certificate: senderCertificate))
        let senderWallet = OfflineNoteV2Wallet(
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
        let recipientStore = InMemoryOfflineNoteV2Store()
        let recipientWallet = OfflineNoteV2Wallet(
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

        let redeemStore = InMemoryOfflineNoteV2Store()
        let redeemNote = try Self.sourceWalletNote(fixture, certificate: senderCertificate)
        try redeemStore.upsert(redeemNote)
        let redeemWallet = OfflineNoteV2Wallet(
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
        XCTAssertEqual(try redeemStore.findNote(noteCommitment: try Self.hex(derivation.sourceNoteCommitment))?.state, .redeemPending)

        _ = try await redeemWallet.sync()

        XCTAssertEqual(try redeemStore.findNote(noteCommitment: try Self.hex(derivation.sourceNoteCommitment))?.state, .spendable)
    }

    func testOfflineNoteV2OutcomeIndexResolvesCommittedAndRejectedExplorerInstructions() throws {
        let fixture = try Self.loadFixture()
        let derivation = fixture.chainVectors.derivation
        let recipientCertificate = try Self.certificate(fixture.paymentToken.recipientKeyCertificate)
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)
        let redeemPending = try OfflineNoteV2WalletNote(
            chainId: derivation.chainId,
            accountId: fixture.paymentToken.recipientAccountId,
            assetId: fixture.chainVectors.redeem.assetId,
            amount: fixture.chainVectors.redeem.amount,
            keyCertificate: recipientCertificate,
            noteCommitment: redeem.sourceNoteCommitment,
            noteSecret: Self.hex(derivation.recipientNoteSecretHex),
            origin: .p2pOutput(OfflineNoteP2pOutputOriginV2(
                paymentRequestId: derivation.paymentRequestId,
                outputIndex: 0
            )),
            state: .redeemPending,
            createdAtMs: 1_700_000_002_000,
            updatedAtMs: 1_700_000_003_000
        )

        let committed = try OfflineNoteV2OutcomeIndex.fromExplorerOutcomes([
            OfflineNoteV2ExplorerInstructionOutcome(
                kind: OfflineNoteV2OutcomeIndex.kindAudit,
                transactionStatus: "Committed",
                transactionHashHex: "audit-tx",
                encodedInstruction: try Self.auditInstructionEnvelope(audit)
            ),
            OfflineNoteV2ExplorerInstructionOutcome(
                kind: OfflineNoteV2OutcomeIndex.kindRedeem,
                transactionStatus: "Committed",
                transactionHashHex: "redeem-tx",
                encodedInstruction: try Self.redeemInstructionEnvelope(redeem)
            ),
        ])

        XCTAssertEqual(try committed.resolve(redeemPending), OfflineNoteV2SyncResolution(
            state: .redeemed,
            transactionHashHex: "redeem-tx"
        ))

        let rejected = OfflineNoteV2OutcomeIndex()
            .recordRejectedAudit(audit, transactionHashHex: "audit-rejected")
            .recordRejectedRedeem(redeem, transactionHashHex: "redeem-rejected")

        XCTAssertEqual(try rejected.resolve(redeemPending), OfflineNoteV2SyncResolution(
            state: .spendable,
            transactionHashHex: "redeem-rejected"
        ))
    }

    func testOfflineNoteV2TransactionBuildersProduceSignedEnvelopes() throws {
        let fixture = try Self.loadFixture()
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let chainId = "00000000-0000-0000-0000-000000000000"
        let creationTimeMs: UInt64 = 1_706_000_000_000

        let issue = try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                issue: Self.issue(fixture),
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )
        let audit = try SwiftTransactionEncoder.encodeAuditOfflineNoteV2(
            request: AuditOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                audit: Self.audit(fixture),
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )
        let redeem = try SwiftTransactionEncoder.encodeRedeemOfflineNoteV2(
            request: RedeemOfflineNoteV2Request(
                chainId: chainId,
                authority: authority,
                redemption: Self.redeem(fixture),
                ttlMs: 60_000
            ),
            keypair: keypair,
            creationTimeMs: creationTimeMs
        )

        for envelope in [issue, audit, redeem] {
            XCTAssertEqual(envelope.norito.first, 1)
            XCTAssertEqual(Data(envelope.norito.dropFirst()), envelope.signedTransaction)
            XCTAssertEqual(envelope.transactionHash.count, 32)
            XCTAssertNil(envelope.payload)
        }
        XCTAssertNotEqual(issue.transactionHash, audit.transactionHash)
        XCTAssertNotEqual(audit.transactionHash, redeem.transactionHash)
    }

    func testRedeemBuilderRejectsMismatchedProofBinding() throws {
        let fixture = try Self.loadFixture()
        let redeem = try Self.redeem(fixture)
        let badProof = try OfflineNoteRecursiveProofV2(
            publicInputsHash: IrohaHash.hash(Data("wrong-public-inputs".utf8)),
            proofBytes: Data("offline-v2-vector-redeem-proof".utf8)
        )
        let forged = try OfflineNoteRedeemV2(
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
            try SwiftTransactionEncoder.encodeRedeemOfflineNoteV2(
                request: RedeemOfflineNoteV2Request(
                    chainId: "00000000-0000-0000-0000-000000000000",
                    authority: authority,
                    redemption: forged
                ),
                keypair: keypair,
                creationTimeMs: 1
            )
        ) { error in
            guard case OfflineNoteV2Error.proofPublicInputsHashMismatch = error else {
                return XCTFail("expected proofPublicInputsHashMismatch, got \(error)")
            }
        }
    }

    func testOfflineNoteV2ProofAndHashValidationRejectsMalformedValues() throws {
        let fixture = try Self.loadFixture()
        let publicInputsHash = try Self.hex(fixture.chainVectors.audit.publicInputsHash)

        let trimmedProof = try OfflineNoteProofBox(
            backend: "  \(OfflineNoteV2Constants.recursiveBackend)  ",
            bytes: Data([0x01])
        )
        XCTAssertEqual(trimmedProof.backend, OfflineNoteV2Constants.recursiveBackend)

        XCTAssertThrowsError(try OfflineNoteProofBox(backend: " \n ", bytes: Data([0x01]))) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyProofBackend)
        }
        XCTAssertThrowsError(try OfflineNoteProofBox(backend: "halo2/ipa", bytes: Data())) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyProofBytes)
        }
        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            publicInputsHash: Data(repeating: 0x01, count: 31),
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "public_inputs_hash", expected: 32, actual: 31)
            )
        }

        var nonCanonicalHash = publicInputsHash
        nonCanonicalHash[31] &= 0xfe
        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            publicInputsHash: nonCanonicalHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .invalidHash(field: "public_inputs_hash"))
        }
    }

    func testOfflineNoteV2CertificateValidationRejectsMalformedValues() throws {
        let fixture = try Self.loadFixture()
        let cert = fixture.paymentToken.senderKeyCertificate
        let publicKey = try Self.base64(cert.publicKey)
        let assertionPublicKey = try Self.base64(cert.assertionPublicKey)
        let issuerSignature = try Self.base64(cert.issuerSignatureBase64)

        XCTAssertThrowsError(try OfflineNoteKeyCertificateV2(
            version: 1,
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
            XCTAssertEqual(error as? OfflineNoteV2Error, .invalidCertificateVersion(1))
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificateV2(
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
            XCTAssertEqual(error as? OfflineNoteV2Error, .certificateMustBeOneUse)
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificateV2(
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
                error as? OfflineNoteV2Error,
                .invalidNotePublicKeyLength(expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteKeyCertificateV2(
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
                error as? OfflineNoteV2Error,
                .invalidIssuerSignatureLength(expected: 64, actual: 63)
            )
        }
    }

    func testOfflineNoteV2AuditBundleRejectsInvalidShapes() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)

        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: [],
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyInputNullifiers)
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: [],
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyInputClaims)
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers + [audit.inputNullifiers[0]],
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .auditInputCountMismatch(nullifiers: audit.inputNullifiers.count + 1, claims: audit.inputClaims.count)
            )
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: [],
            outputClaims: audit.outputClaims,
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyOutputCommitments)
        }
        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: [],
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyOutputClaims)
        }

        let uncommittedClaim = try OfflineNoteAuditOutputClaimV2(
            noteCommitment: Data(repeating: 0x03, count: 32),
            keyCertificate: audit.outputClaims[0].keyCertificate,
            assetId: audit.outputClaims[0].assetId,
            amount: audit.outputClaims[0].amount
        )
        XCTAssertThrowsError(try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: [uncommittedClaim],
            recursiveProof: audit.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .auditOutputClaimNotCommitted(uncommittedClaim.noteCommitment.hexLowercased())
            )
        }
    }

    func testOfflineNoteV2IssueAndClaimValidationCoversDerivedClaimAndFailures() throws {
        let fixture = try Self.loadFixture()
        let certificate = try Self.certificate(fixture.paymentToken.senderKeyCertificate)
        let noteCommitment = try Self.hex(fixture.chainVectors.issue.noteCommitment)
        let issue = try OfflineNoteIssueV2(
            noteCommitment: noteCommitment,
            keyCertificate: certificate,
            assetId: fixture.chainVectors.issue.assetId,
            amount: "5.5000"
        )

        XCTAssertEqual(issue.amount, "5.5000")
        let claim = try issue.issuedClaim()
        XCTAssertEqual(claim.domain, OfflineNoteV2Constants.issuedClaimDomain)
        XCTAssertEqual(claim.noteCommitment, issue.noteCommitment)
        XCTAssertEqual(claim.keyCertificatePayloadHash, try certificate.payloadHash())
        XCTAssertEqual(claim.assetId, issue.assetId)
        XCTAssertEqual(claim.amount, "5.5000")
        XCTAssertEqual(try claim.claimHash().count, 32)

        XCTAssertThrowsError(try OfflineNoteIssueV2(
            noteCommitment: Data(repeating: 0x01, count: 31),
            keyCertificate: certificate,
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "note_commitment", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteIssueV2(
            noteCommitment: noteCommitment,
            keyCertificate: certificate,
            assetId: "cash#branch.sbp",
            amount: fixture.chainVectors.issue.amount
        )) { error in
            guard case OfflineNoritoError.invalidAssetId("cash#branch.sbp") = error else {
                return XCTFail("expected invalidAssetId, got \(error)")
            }
        }
        XCTAssertThrowsError(try OfflineNoteIssueV2(
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

    func testOfflineNoteV2RedeemValidationRejectsBadInputsAndDerivesIssuedClaim() throws {
        let fixture = try Self.loadFixture()
        let redeem = try Self.redeem(fixture)
        let issuedClaim = try redeem.issuedClaim()

        XCTAssertEqual(issuedClaim.noteCommitment, redeem.sourceNoteCommitment)
        XCTAssertEqual(issuedClaim.keyCertificatePayloadHash, try redeem.senderKeyCertificate.payloadHash())
        XCTAssertEqual(issuedClaim.assetId, redeem.assetId)
        XCTAssertEqual(issuedClaim.amount, redeem.amount)

        XCTAssertThrowsError(try OfflineNoteRedeemV2(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: [],
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyInputNullifiers)
        }
        XCTAssertThrowsError(try OfflineNoteRedeemV2(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: [Data(repeating: 0x01, count: 31)],
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "input_nullifiers[0]", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteRedeemV2(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            senderKeyCertificate: redeem.senderKeyCertificate,
            recipient: "\(redeem.recipient)@bad",
            assetId: redeem.assetId,
            amount: redeem.amount,
            recursiveProof: redeem.recursiveProof
        ))
    }

    func testOfflineNoteV2AuditValidateProofBindingReportsExpectedAndActualHashes() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        var wrongPublicInputsHash = try audit.publicInputsHash()
        wrongPublicInputsHash[0] ^= 0x01
        let forgedProof = try OfflineNoteRecursiveProofV2(
            publicInputsHash: wrongPublicInputsHash,
            proofBytes: audit.recursiveProof.proof.bytes
        )
        let forgedAudit = try OfflineNoteAuditBundleV2(
            tokenId: audit.tokenId,
            senderKeyCertificate: audit.senderKeyCertificate,
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: audit.outputClaims,
            recursiveProof: forgedProof
        )

        XCTAssertThrowsError(try forgedAudit.validateProofBinding()) { error in
            guard case let OfflineNoteV2Error.proofPublicInputsHashMismatch(expected, actual) = error else {
                return XCTFail("expected proofPublicInputsHashMismatch, got \(error)")
            }
            XCTAssertEqual(expected, try? audit.publicInputsHash().hexLowercased())
            XCTAssertEqual(actual, forgedProof.publicInputsHash.hexLowercased())
        }
    }

    func testOfflineNoteV2TransactionBuilderCoversOptionalNonceAndInputValidation() throws {
        let fixture = try Self.loadFixture()
        let keypair = try Keypair(privateKeyBytes: Data(0..<32))
        let authority = AccountId.make(publicKey: keypair.publicKey)
        let chainId = "00000000-0000-0000-0000-000000000000"
        let issue = try Self.issue(fixture)

        let defaultEnvelope = try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(chainId: chainId, authority: authority, issue: issue),
            keypair: keypair,
            creationTimeMs: 1_706_000_000_000
        )
        let nonceEnvelope = try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(
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

        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(chainId: " \n ", authority: authority, issue: issue),
            keypair: keypair,
            creationTimeMs: 1
        )) { error in
            XCTAssertEqual(error as? TransactionInputError, .emptyChainId)
        }
        XCTAssertThrowsError(try SwiftTransactionEncoder.encodeIssueOfflineNoteV2(
            request: IssueOfflineNoteV2Request(chainId: chainId, authority: "\(authority)@bad", issue: issue),
            keypair: keypair,
            creationTimeMs: 1
        )) { error in
            XCTAssertEqual(
                error as? TransactionInputError,
                .malformedAccountId(field: "authority", value: "\(authority)@bad")
            )
        }
    }

    func testOfflineNoteV2RecursiveProofCoversCustomVerifierAndVerifierValidation() throws {
        let publicInputsHash = try Self.audit(Self.loadFixture()).publicInputsHash()
        let proof = try OfflineNoteRecursiveProofV2(
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

        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            verifierBackend: "",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .emptyBackend)
        }
        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            verifierBackend: "custom_backend",
            verifierName: "",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .emptyName)
        }
        XCTAssertThrowsError(try OfflineNoteRecursiveProofV2(
            verifierBackend: "halo2:ipa",
            verifierName: "custom_vk",
            publicInputsHash: publicInputsHash,
            proofBytes: Data([0x01])
        )) { error in
            XCTAssertEqual(error as? VerifyingKeyIdError, .invalidSeparator)
        }
    }

    func testOfflineNoteV2CertificatePayloadValidationAndEncodingBranches() throws {
        let certificate = try Self.certificate(Self.loadFixture().paymentToken.senderKeyCertificate)
        let payload = try certificate.signingPayload()

        XCTAssertEqual(payload.domain, OfflineNoteV2Constants.keyCertificatePayloadDomain)
        XCTAssertEqual(payload.version, certificate.version)
        XCTAssertEqual(payload.publicKey, certificate.publicKey)
        XCTAssertEqual(payload.oneUse, true)
        XCTAssertNotEqual(try payload.noritoEncoded(), try certificate.noritoEncoded())

        let noLimitPayload = try OfflineNoteKeyCertificatePayloadV2(
            version: 2,
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
        let limitedPayload = try OfflineNoteKeyCertificatePayloadV2(
            version: 2,
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

        XCTAssertThrowsError(try OfflineNoteKeyCertificatePayloadV2(
            version: 2,
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
        XCTAssertThrowsError(try OfflineNoteKeyCertificatePayloadV2(
            version: 2,
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
                error as? OfflineNoteV2Error,
                .invalidNotePublicKeyLength(expected: 32, actual: 31)
            )
        }
    }

    func testOfflineNoteV2PublicInputConstructorsRejectMalformedInputs() throws {
        let fixture = try Self.loadFixture()
        let audit = try Self.audit(fixture)
        let redeem = try Self.redeem(fixture)
        let auditOutputClaims = try audit.outputClaims.map(OfflineNoteIssuedClaimV2.fromAuditOutput)

        XCTAssertThrowsError(try OfflineNoteRedeemPublicInputsV2(
            sourceNoteCommitment: Data(repeating: 0x01, count: 31),
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: try redeem.senderKeyCertificate.payloadHash(),
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "source_note_commitment", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteRedeemPublicInputsV2(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: Data(repeating: 0x01, count: 31),
            recipient: redeem.recipient,
            assetId: redeem.assetId,
            amount: redeem.amount
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "key_certificate_payload_hash", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteRedeemPublicInputsV2(
            sourceNoteCommitment: redeem.sourceNoteCommitment,
            inputNullifiers: redeem.inputNullifiers,
            keyCertificatePayloadHash: try redeem.senderKeyCertificate.payloadHash(),
            recipient: "\(redeem.recipient)@bad",
            assetId: redeem.assetId,
            amount: redeem.amount
        ))

        XCTAssertThrowsError(try OfflineNoteAuditPublicInputsV2(
            tokenId: Data(repeating: 0x01, count: 31),
            keyCertificatePayloadHash: try audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: auditOutputClaims
        )) { error in
            XCTAssertEqual(
                error as? OfflineNoteV2Error,
                .invalidHashLength(field: "token_id", expected: 32, actual: 31)
            )
        }
        XCTAssertThrowsError(try OfflineNoteAuditPublicInputsV2(
            tokenId: audit.tokenId,
            keyCertificatePayloadHash: try audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: audit.inputNullifiers,
            inputClaims: audit.inputClaims,
            outputCommitments: audit.outputCommitments,
            outputClaims: []
        )) { error in
            XCTAssertEqual(error as? OfflineNoteV2Error, .emptyOutputClaims)
        }
    }

    private static func issue(_ fixture: OfflineInteropFixture) throws -> OfflineNoteIssueV2 {
        try OfflineNoteIssueV2(
            noteCommitment: hex(fixture.chainVectors.issue.noteCommitment),
            keyCertificate: certificate(fixture.paymentToken.senderKeyCertificate),
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount
        )
    }

    private static func redeem(_ fixture: OfflineInteropFixture) throws -> OfflineNoteRedeemV2 {
        let vector = fixture.chainVectors.redeem
        return try OfflineNoteRedeemV2(
            sourceNoteCommitment: hex(vector.sourceNoteCommitment),
            inputNullifiers: try vector.inputNullifiers.map(hex),
            senderKeyCertificate: certificate(fixture.paymentToken.recipientKeyCertificate),
            recipient: fixture.paymentToken.recipientAccountId,
            assetId: vector.assetId,
            amount: vector.amount,
            recursiveProof: OfflineNoteRecursiveProofV2(
                publicInputsHash: hex(vector.publicInputsHash),
                proofBytes: Data("offline-v2-vector-redeem-proof".utf8)
            )
        )
    }

    private static func audit(_ fixture: OfflineInteropFixture) throws -> OfflineNoteAuditBundleV2 {
        let vector = fixture.chainVectors.audit
        return try OfflineNoteAuditBundleV2(
            tokenId: hex(vector.tokenId),
            senderKeyCertificate: certificate(fixture.paymentToken.senderKeyCertificate),
            inputNullifiers: try vector.inputNullifiers.map(hex),
            inputClaims: try fixture.paymentToken.inputClaims.map(issuedClaim),
            outputCommitments: try vector.outputCommitments.map(hex),
            outputClaims: try fixture.paymentToken.outputClaims.map(auditOutputClaim),
            recursiveProof: OfflineNoteRecursiveProofV2(
                publicInputsHash: hex(vector.publicInputsHash),
                proofBytes: Data("offline-v2-vector-audit-proof".utf8)
            )
        )
    }

    private static func certificate(_ json: OfflineCertificateJSON) throws -> OfflineNoteKeyCertificateV2 {
        try OfflineNoteKeyCertificateV2(
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

    private static func certificateVerifier(_ fixture: OfflineInteropFixture) throws -> Ed25519OfflineNoteV2CertificateVerifier {
        Ed25519OfflineNoteV2CertificateVerifier(
            trustedIssuerPublicKeys: [try base64(fixture.offlineFiPublicKeyBase64)]
        )
    }

    private static func tamperedSignatureCertificate(
        _ certificate: OfflineNoteKeyCertificateV2
    ) throws -> OfflineNoteKeyCertificateV2 {
        var signature = certificate.issuerSignature
        signature[signature.startIndex] ^= 0x01
        return try OfflineNoteKeyCertificateV2(
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
        _ token: OfflineNoteV2PaymentToken,
        certificate: OfflineNoteKeyCertificateV2
    ) throws -> OfflineNoteV2PaymentToken {
        var outputClaims = token.audit.outputClaims
        let output = try XCTUnwrap(outputClaims.first)
        outputClaims[0] = try OfflineNoteAuditOutputClaimV2(
            noteCommitment: output.noteCommitment,
            keyCertificate: certificate,
            assetId: output.assetId,
            amount: output.amount
        )
        return try paymentTokenReplacingAuditClaims(token, inputClaims: token.audit.inputClaims, outputClaims: outputClaims)
    }

    private static func paymentTokenReplacingFirstOutputAmount(
        _ token: OfflineNoteV2PaymentToken,
        amount: String
    ) throws -> OfflineNoteV2PaymentToken {
        var outputClaims = token.audit.outputClaims
        let output = try XCTUnwrap(outputClaims.first)
        outputClaims[0] = try OfflineNoteAuditOutputClaimV2(
            noteCommitment: output.noteCommitment,
            keyCertificate: output.keyCertificate,
            assetId: output.assetId,
            amount: amount
        )
        return try paymentTokenReplacingAuditClaims(token, inputClaims: token.audit.inputClaims, outputClaims: outputClaims)
    }

    private static func paymentTokenReplacingFirstOutputAmountWithoutProofRebind(
        _ token: OfflineNoteV2PaymentToken,
        amount: String
    ) throws -> OfflineNoteV2PaymentToken {
        var outputClaims = token.audit.outputClaims
        let output = try XCTUnwrap(outputClaims.first)
        outputClaims[0] = try OfflineNoteAuditOutputClaimV2(
            noteCommitment: output.noteCommitment,
            keyCertificate: output.keyCertificate,
            assetId: output.assetId,
            amount: amount
        )
        return OfflineNoteV2PaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: token.tokenId,
            audit: try OfflineNoteAuditBundleV2(
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
        _ token: OfflineNoteV2PaymentToken,
        assetId: String
    ) throws -> OfflineNoteV2PaymentToken {
        var outputClaims = token.audit.outputClaims
        let output = try XCTUnwrap(outputClaims.first)
        outputClaims[0] = try OfflineNoteAuditOutputClaimV2(
            noteCommitment: output.noteCommitment,
            keyCertificate: output.keyCertificate,
            assetId: assetId,
            amount: output.amount
        )
        return try paymentTokenReplacingAuditClaims(token, inputClaims: token.audit.inputClaims, outputClaims: outputClaims)
    }

    private static func paymentTokenReversingOutputs(
        _ token: OfflineNoteV2PaymentToken
    ) throws -> OfflineNoteV2PaymentToken {
        try paymentTokenReplacingOutputs(
            token,
            outputClaims: Array(token.audit.outputClaims.reversed()),
            outputCommitments: Array(token.audit.outputCommitments.reversed())
        )
    }

    private static func paymentTokenDroppingFirstOutput(
        _ token: OfflineNoteV2PaymentToken
    ) throws -> OfflineNoteV2PaymentToken {
        try paymentTokenReplacingOutputs(
            token,
            outputClaims: Array(token.audit.outputClaims.dropFirst()),
            outputCommitments: Array(token.audit.outputCommitments.dropFirst())
        )
    }

    private static func paymentTokenReplacingChainId(
        _ token: OfflineNoteV2PaymentToken,
        chainId: String
    ) -> OfflineNoteV2PaymentToken {
        OfflineNoteV2PaymentToken(
            chainId: chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: token.tokenId,
            audit: token.audit,
            createdAtMs: token.createdAtMs
        )
    }

    private static func paymentTokenReplacingLastOutputCertificate(
        _ token: OfflineNoteV2PaymentToken,
        certificate: OfflineNoteKeyCertificateV2
    ) throws -> OfflineNoteV2PaymentToken {
        var outputClaims = token.audit.outputClaims
        let output = try XCTUnwrap(outputClaims.last)
        outputClaims[outputClaims.count - 1] = try OfflineNoteAuditOutputClaimV2(
            noteCommitment: output.noteCommitment,
            keyCertificate: certificate,
            assetId: output.assetId,
            amount: output.amount
        )
        return try paymentTokenReplacingAuditClaims(token, inputClaims: token.audit.inputClaims, outputClaims: outputClaims)
    }

    private static func paymentTokenReplacingFirstInputClaimHash(
        _ token: OfflineNoteV2PaymentToken,
        keyCertificatePayloadHash: Data
    ) throws -> OfflineNoteV2PaymentToken {
        var inputClaims = token.audit.inputClaims
        let input = try XCTUnwrap(inputClaims.first)
        inputClaims[0] = try OfflineNoteIssuedClaimV2(
            domain: input.domain,
            noteCommitment: input.noteCommitment,
            keyCertificatePayloadHash: keyCertificatePayloadHash,
            assetId: input.assetId,
            amount: input.amount
        )
        return try paymentTokenReplacingAuditClaims(token, inputClaims: inputClaims, outputClaims: token.audit.outputClaims)
    }

    private static func paymentTokenReplacingSenderCertificate(
        _ token: OfflineNoteV2PaymentToken,
        certificate: OfflineNoteKeyCertificateV2
    ) throws -> OfflineNoteV2PaymentToken {
        let certificateHash = try certificate.payloadHash()
        let inputClaims = try token.audit.inputClaims.map { input in
            try OfflineNoteIssuedClaimV2(
                domain: input.domain,
                noteCommitment: input.noteCommitment,
                keyCertificatePayloadHash: certificateHash,
                assetId: input.assetId,
                amount: input.amount
            )
        }
        let tokenId = try OfflineNotePaymentTokenIdPreimageV2(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            createdAtMs: token.createdAtMs,
            tokenNonce: token.tokenNonce,
            senderKeyCertificatePayloadHash: certificateHash,
            inputNullifiers: token.audit.inputNullifiers,
            outputCommitments: token.audit.outputCommitments
        ).derivePaymentTokenId()
        let draft = try OfflineNoteAuditBundleV2(
            tokenId: tokenId,
            senderKeyCertificate: certificate,
            inputNullifiers: token.audit.inputNullifiers,
            inputClaims: inputClaims,
            outputCommitments: token.audit.outputCommitments,
            outputClaims: token.audit.outputClaims,
            recursiveProof: token.audit.recursiveProof
        )
        let proof = try OfflineNoteRecursiveProofV2(
            verifierKeyId: token.audit.recursiveProof.verifierKeyId,
            publicInputsHash: draft.publicInputsHash(),
            proof: token.audit.recursiveProof.proof
        )
        return OfflineNoteV2PaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: tokenId,
            audit: try draft.replacingRecursiveProof(proof),
            createdAtMs: token.createdAtMs
        )
    }

    private static func paymentTokenReplacingPaymentRequestId(
        _ token: OfflineNoteV2PaymentToken,
        paymentRequestId: String
    ) throws -> OfflineNoteV2PaymentToken {
        let tokenId = try OfflineNotePaymentTokenIdPreimageV2(
            chainId: token.chainId,
            paymentRequestId: paymentRequestId,
            createdAtMs: token.createdAtMs,
            tokenNonce: token.tokenNonce,
            senderKeyCertificatePayloadHash: token.audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: token.audit.inputNullifiers,
            outputCommitments: token.audit.outputCommitments
        ).derivePaymentTokenId()
        let draft = try OfflineNoteAuditBundleV2(
            tokenId: tokenId,
            senderKeyCertificate: token.audit.senderKeyCertificate,
            inputNullifiers: token.audit.inputNullifiers,
            inputClaims: token.audit.inputClaims,
            outputCommitments: token.audit.outputCommitments,
            outputClaims: token.audit.outputClaims,
            recursiveProof: token.audit.recursiveProof
        )
        let proof = try OfflineNoteRecursiveProofV2(
            verifierKeyId: token.audit.recursiveProof.verifierKeyId,
            publicInputsHash: draft.publicInputsHash(),
            proof: token.audit.recursiveProof.proof
        )
        return OfflineNoteV2PaymentToken(
            chainId: token.chainId,
            paymentRequestId: paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: tokenId,
            audit: try draft.replacingRecursiveProof(proof),
            createdAtMs: token.createdAtMs
        )
    }

    private static func paymentTokenReplacingTopLevelTokenId(
        _ token: OfflineNoteV2PaymentToken
    ) -> OfflineNoteV2PaymentToken {
        OfflineNoteV2PaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: flippedHash(token.tokenId),
            audit: token.audit,
            createdAtMs: token.createdAtMs
        )
    }

    private static func paymentTokenReplacingAuditTokenId(
        _ token: OfflineNoteV2PaymentToken
    ) throws -> OfflineNoteV2PaymentToken {
        let auditTokenId = flippedHash(token.audit.tokenId)
        let draft = try OfflineNoteAuditBundleV2(
            tokenId: auditTokenId,
            senderKeyCertificate: token.audit.senderKeyCertificate,
            inputNullifiers: token.audit.inputNullifiers,
            inputClaims: token.audit.inputClaims,
            outputCommitments: token.audit.outputCommitments,
            outputClaims: token.audit.outputClaims,
            recursiveProof: token.audit.recursiveProof
        )
        let proof = try OfflineNoteRecursiveProofV2(
            verifierKeyId: token.audit.recursiveProof.verifierKeyId,
            publicInputsHash: draft.publicInputsHash(),
            proof: token.audit.recursiveProof.proof
        )
        return OfflineNoteV2PaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: token.tokenId,
            audit: try draft.replacingRecursiveProof(proof),
            createdAtMs: token.createdAtMs
        )
    }

    private static func paymentTokenReplacingOutputs(
        _ token: OfflineNoteV2PaymentToken,
        outputClaims: [OfflineNoteAuditOutputClaimV2],
        outputCommitments: [Data]
    ) throws -> OfflineNoteV2PaymentToken {
        let tokenId = try OfflineNotePaymentTokenIdPreimageV2(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            createdAtMs: token.createdAtMs,
            tokenNonce: token.tokenNonce,
            senderKeyCertificatePayloadHash: token.audit.senderKeyCertificate.payloadHash(),
            inputNullifiers: token.audit.inputNullifiers,
            outputCommitments: outputCommitments
        ).derivePaymentTokenId()
        let draft = try OfflineNoteAuditBundleV2(
            tokenId: tokenId,
            senderKeyCertificate: token.audit.senderKeyCertificate,
            inputNullifiers: token.audit.inputNullifiers,
            inputClaims: token.audit.inputClaims,
            outputCommitments: outputCommitments,
            outputClaims: outputClaims,
            recursiveProof: token.audit.recursiveProof
        )
        let proof = try OfflineNoteRecursiveProofV2(
            verifierKeyId: token.audit.recursiveProof.verifierKeyId,
            publicInputsHash: draft.publicInputsHash(),
            proof: token.audit.recursiveProof.proof
        )
        return OfflineNoteV2PaymentToken(
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
        _ token: OfflineNoteV2PaymentToken,
        inputClaims: [OfflineNoteIssuedClaimV2],
        outputClaims: [OfflineNoteAuditOutputClaimV2]
    ) throws -> OfflineNoteV2PaymentToken {
        let draft = try OfflineNoteAuditBundleV2(
            tokenId: token.audit.tokenId,
            senderKeyCertificate: token.audit.senderKeyCertificate,
            inputNullifiers: token.audit.inputNullifiers,
            inputClaims: inputClaims,
            outputCommitments: token.audit.outputCommitments,
            outputClaims: outputClaims,
            recursiveProof: token.audit.recursiveProof
        )
        let proof = try OfflineNoteRecursiveProofV2(
            verifierKeyId: token.audit.recursiveProof.verifierKeyId,
            publicInputsHash: draft.publicInputsHash(),
            proof: token.audit.recursiveProof.proof
        )
        return OfflineNoteV2PaymentToken(
            chainId: token.chainId,
            paymentRequestId: token.paymentRequestId,
            tokenNonce: token.tokenNonce,
            tokenId: token.tokenId,
            audit: try draft.replacingRecursiveProof(proof),
            createdAtMs: token.createdAtMs
        )
    }

    private static func issuedClaim(_ json: OfflineInputClaimJSON) throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteIssuedClaimV2(
            domain: json.domain,
            noteCommitment: hex(json.noteCommitment),
            keyCertificatePayloadHash: hex(json.keyCertificatePayloadHash),
            assetId: json.assetId,
            amount: json.amount
        )
    }

    private static func auditOutputClaim(_ json: OfflineOutputClaimJSON) throws -> OfflineNoteAuditOutputClaimV2 {
        try OfflineNoteAuditOutputClaimV2(
            noteCommitment: hex(json.noteCommitment),
            keyCertificate: certificate(json.keyCertificate),
            assetId: "\(json.assetDefinitionId)#\(json.accountId)",
            amount: json.amount
        )
    }

    private static func sourceWalletNote(
        _ fixture: OfflineInteropFixture,
        certificate: OfflineNoteKeyCertificateV2
    ) throws -> OfflineNoteV2WalletNote {
        let derivation = fixture.chainVectors.derivation
        return try OfflineNoteV2WalletNote(
            chainId: derivation.chainId,
            accountId: accountId(fromAssetId: fixture.chainVectors.issue.assetId),
            assetId: fixture.chainVectors.issue.assetId,
            amount: fixture.chainVectors.issue.amount,
            keyCertificate: certificate,
            noteCommitment: hex(derivation.sourceNoteCommitment),
            noteSecret: hex(derivation.sourceNoteSecretHex),
            origin: .issuerLoad(OfflineNoteIssuerLoadOriginV2(
                operationId: derivation.issuerLoadOperationId,
                lineageId: derivation.issuerLoadLineageId,
                localRevision: derivation.issuerLoadLocalRevision
            )),
            state: .spendable,
            createdAtMs: 1_700_000_000_000,
            updatedAtMs: 1_700_000_000_000
        )
    }

    private static func storedCollectionData(notes: [OfflineNoteV2WalletNote]) throws -> Data {
        let stored = try notes.map { note in
            [
                "commitmentHex": note.noteCommitmentHex,
                "payloadBase64": try OfflineNoteV2WalletNoteJsonCodec.encode(note).base64EncodedString()
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

    private final class RecordingOfflineNoteV2KeychainBacking: OfflineNoteV2KeychainBackingStore {
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
                throw OfflineNoteV2KeychainStoreError.keychainFailure(-1)
            }
            values[label] = data
        }

        func delete(label: String) throws {
            operations.append("delete:\(label)")
            if deleteFailures.contains(label) {
                throw OfflineNoteV2KeychainStoreError.keychainFailure(-1)
            }
            values.removeValue(forKey: label)
        }
    }

    private struct StaticAttestationProvider: OfflineNoteV2AttestationProvider {
        let certificate: OfflineNoteKeyCertificateV2

        func currentKeyCertificate() throws -> OfflineNoteKeyCertificateV2 {
            certificate
        }
    }

    private final class QueueRandomSource: OfflineNoteV2RandomSource {
        private let values: [Data]
        private var index = 0

        init(values: [Data]) {
            self.values = values
        }

        func nextBytes(count: Int) throws -> Data {
            guard index < values.count else {
                throw OfflineNoteV2FixtureError.randomSourceExhausted
            }
            let value = values[index]
            index += 1
            guard value.count == count else {
                throw OfflineNoteV2WalletError.randomLength(expected: count, actual: value.count)
            }
            return value
        }
    }

    private struct FixedIdGenerator: OfflineNoteV2IdGenerator {
        let id: String

        func nextId(prefix: String) -> String {
            id
        }
    }

    private final class SequenceIdGenerator: OfflineNoteV2IdGenerator {
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

    private struct StaticIssuerDeviceBindingProvider: OfflineNoteV2IssuerDeviceBindingProvider {
        let binding: OfflineNoteV2IssuerDeviceBinding

        func currentDeviceBinding(chainId: String,
                                  accountId: String,
                                  assetDefinitionId: String) throws -> OfflineNoteV2IssuerDeviceBinding {
            binding
        }
    }

    private struct BindingProofProvider: OfflineNoteV2ProofProvider {
        func proveAudit(_ audit: OfflineNoteAuditBundleV2) throws -> OfflineNoteRecursiveProofV2 {
            try OfflineNoteRecursiveProofV2(
                publicInputsHash: audit.publicInputsHash(),
                proofBytes: Data("wallet-audit-proof".utf8)
            )
        }

        func proveRedeem(_ redemption: OfflineNoteRedeemV2) throws -> OfflineNoteRecursiveProofV2 {
            try OfflineNoteRecursiveProofV2(
                publicInputsHash: redemption.publicInputsHash(),
                proofBytes: Data("wallet-redeem-proof".utf8)
            )
        }
    }

    private struct BindingProofVerifier: OfflineNoteV2ProofVerifier {
        func verifyAudit(_ audit: OfflineNoteAuditBundleV2) throws -> Bool {
            try audit.recursiveProof.publicInputsHash == audit.publicInputsHash()
        }

        func verifyRedeem(_ redemption: OfflineNoteRedeemV2) throws -> Bool {
            try redemption.recursiveProof.publicInputsHash == redemption.publicInputsHash()
        }
    }

    private final class RecordingIssuerClient: OfflineNoteV2IssuerClient {
        let loadContext: OfflineNoteV2LoadContext
        var lastIssueRequest: OfflineNoteV2IssueRequest?
        var lastPrepareLoadAssetDefinitionId: String?

        init(loadContext: OfflineNoteV2LoadContext) {
            self.loadContext = loadContext
        }

        func prepareLoad(chainId: String,
                         accountId: String,
                         assetDefinitionId: String,
                         amount: String) async throws -> OfflineNoteV2LoadContext {
            lastPrepareLoadAssetDefinitionId = assetDefinitionId
            return loadContext
        }

        func issueNote(_ request: OfflineNoteV2IssueRequest) async throws -> OfflineNoteV2IssueResponse {
            lastIssueRequest = request
            return OfflineNoteV2IssueResponse(
                noteCommitment: request.noteCommitment,
                operationId: request.loadContext.operationId,
                lineageId: request.loadContext.lineageId,
                localRevision: request.loadContext.localRevision,
                keyCertificate: request.loadContext.keyCertificate,
                settlementEntryHashHex: "settlement-entry-hash"
            )
        }
    }

    private final class RecordingTransactionSubmitter: OfflineNoteV2TransactionSubmitter {
        private(set) var audits: [OfflineNoteAuditBundleV2] = []
        private(set) var redemptions: [OfflineNoteRedeemV2] = []

        func submitAudit(_ audit: OfflineNoteAuditBundleV2) async throws {
            audits.append(audit)
        }

        func submitRedeem(_ redemption: OfflineNoteRedeemV2) async throws {
            redemptions.append(redemption)
        }
    }

    private struct FailingTransactionSubmitter: OfflineNoteV2TransactionSubmitter {
        func submitAudit(_ audit: OfflineNoteAuditBundleV2) async throws {
            throw OfflineNoteV2WalletError.invalidState
        }

        func submitRedeem(_ redemption: OfflineNoteRedeemV2) async throws {
            throw OfflineNoteV2WalletError.invalidState
        }
    }

    private final class RecordingSyncResolver: OfflineNoteV2SyncResolver {
        var resolutions: [String: OfflineNoteV2WalletNoteState]
        private(set) var resolvedCommitments: [String] = []

        init(resolutions: [String: OfflineNoteV2WalletNoteState]) {
            self.resolutions = resolutions
        }

        func resolvePendingNote(_ note: OfflineNoteV2WalletNote) async throws -> OfflineNoteV2SyncResolution? {
            let commitment = note.noteCommitmentHex
            resolvedCommitments.append(commitment)
            guard let state = resolutions[commitment] else {
                return nil
            }
            return OfflineNoteV2SyncResolution(state: state, transactionHashHex: "tx-\(commitment)")
        }
    }

    private static func issueInstructionEnvelope(_ issue: OfflineNoteIssueV2) throws -> Data {
        rawInstructionPair(
            wireName: OfflineNoteV2TypeNames.issueInstruction,
            wirePayload: try instructionWirePayload(
                typeName: OfflineNoteV2TypeNames.issueInstruction,
                modelPayload: OfflineNoteV2Encoding.encodeIssue(issue)
            )
        )
    }

    private static func auditInstructionEnvelope(_ audit: OfflineNoteAuditBundleV2) throws -> Data {
        rawInstructionPair(
            wireName: OfflineNoteV2TypeNames.auditInstruction,
            wirePayload: try instructionWirePayload(
                typeName: OfflineNoteV2TypeNames.auditInstruction,
                modelPayload: OfflineNoteV2Encoding.encodeAudit(audit)
            )
        )
    }

    private static func redeemInstructionEnvelope(_ redemption: OfflineNoteRedeemV2) throws -> Data {
        rawInstructionPair(
            wireName: OfflineNoteV2TypeNames.redeemInstruction,
            wirePayload: try instructionWirePayload(
                typeName: OfflineNoteV2TypeNames.redeemInstruction,
                modelPayload: OfflineNoteV2Encoding.encodeRedeem(redemption)
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
            .appendingPathComponent("../../../fixtures/offline/interop_contract_v2.json")
            .standardizedFileURL
        let data = try Data(contentsOf: fixtureURL)
        return try JSONDecoder().decode(OfflineInteropFixture.self, from: data)
    }

    private static func hex(_ value: String) throws -> Data {
        guard let data = Data(hexString: value) else {
            throw OfflineNoteV2FixtureError.invalidHex(value)
        }
        return data
    }

    private static func base64(_ value: String) throws -> Data {
        guard let data = Data(base64Encoded: value) else {
            throw OfflineNoteV2FixtureError.invalidBase64
        }
        return data
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
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("request_body")
        }
        let parsed = try JSONSerialization.jsonObject(with: body)
        guard let object = parsed as? [String: Any] else {
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("request_body")
        }
        return object
    }

    private static func object(_ object: [String: Any], _ key: String) throws -> [String: Any] {
        guard let value = object[key] as? [String: Any] else {
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON(key)
        }
        return value
    }

    private static func string(_ object: [String: Any], _ key: String) throws -> String {
        guard let value = object[key] as? String else {
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON(key)
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
        throw ToriiOfflineNoteV2IssuerClientError.invalidJSON(key)
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
                throw ToriiOfflineNoteV2IssuerClientError.invalidURL(request.url?.absoluteString ?? "")
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

private enum OfflineNoteV2FixtureError: Error {
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
    let paymentTokenQrV1: OfflineQrFixture

    private enum CodingKeys: String, CodingKey {
        case paymentTokenNoritoBase64 = "payment_token_norito_base64"
        case paymentTokenText = "payment_token_text"
        case paymentTokenQrV1 = "payment_token_qr_v1"
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
