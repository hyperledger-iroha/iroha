import CoreImage
import Foundation
import Vision
import XCTest
@testable import IrohaSwift

final class OfflineQrStreamTests: XCTestCase {
    func testQrStreamRoundTrip() throws {
        let payload = makePayload(length: 1024)
        let frames = try OfflineQrStreamEncoder.encodeFrameBytes(
            payload: payload,
            options: OfflineQrStreamOptions(chunkSize: 200, parityGroup: 0)
        )
        let decoder = OfflineQrStreamDecoder()
        var result: OfflineQrStreamDecodeResult?
        for frame in frames {
            result = try decoder.ingest(frameBytes: frame)
        }
        guard let final = result else {
            return XCTFail("Missing decode result")
        }
        XCTAssertTrue(final.isComplete)
        XCTAssertEqual(final.payload, payload)
    }

    func testQrStreamParityRecoversMissingChunk() throws {
        let payload = makePayload(length: 900)
        let frames = try OfflineQrStreamEncoder.encodeFrames(
            payload: payload,
            payloadKind: .offlinePaymentToken,
            options: OfflineQrStreamOptions(chunkSize: 180, parityGroup: 3)
        )
        let header = frames.first(where: { $0.kind == .header })
        let dataFrames = frames.filter { $0.kind == .data }
        let parityFrames = frames.filter { $0.kind == .parity }
        XCTAssertNotNil(header)
        XCTAssertFalse(dataFrames.isEmpty)
        XCTAssertFalse(parityFrames.isEmpty)
        let dropped = dataFrames[1]
        let decoder = OfflineQrStreamDecoder()
        _ = try decoder.ingest(frameBytes: header!.encode())
        var result: OfflineQrStreamDecodeResult?
        for frame in dataFrames where frame.index != dropped.index {
            result = try decoder.ingest(frameBytes: frame.encode())
        }
        for frame in parityFrames {
            result = try decoder.ingest(frameBytes: frame.encode())
        }
        guard let final = result else {
            return XCTFail("Missing decode result")
        }
        XCTAssertTrue(final.isComplete)
        XCTAssertEqual(final.payload, payload)
        XCTAssertEqual(final.recoveredChunks, 1)
    }

    func testQrStreamRejectsBadChecksum() throws {
        let payload = makePayload(length: 300)
        let frames = try OfflineQrStreamEncoder.encodeFrameBytes(payload: payload)
        var corrupted = frames[0]
        corrupted[corrupted.count - 1] ^= 0x11
        let decoder = OfflineQrStreamDecoder()
        XCTAssertThrowsError(try decoder.ingest(frameBytes: corrupted)) { error in
            guard case OfflineQrStreamError.checksumMismatch = error else {
                return XCTFail("Expected checksumMismatch, got \(error)")
            }
        }
    }

    func testQrStreamTextCodecRejectsLegacyVersionedPrefix() throws {
        let payload = makePayload(length: 64)
        let legacy = "iroha:qr-old:" + payload.base64EncodedString()

        XCTAssertThrowsError(
            try OfflineQrStreamTextCodec.decode(legacy, encoding: .base64)
        )
    }

    func testTransferTextPayloadCodecRoundTripsKindsAndMapsQrPayloadKind() throws {
        let payload = Data(#"{"version":2}"#.utf8)
        let challenge = try OfflineNoteTransferTextPayloadCodec.encode(payload, kind: .receiveRequest)
        let payment = try OfflineNoteTransferTextPayloadCodec.encode(payload, kind: .paymentToken)
        let ack = try OfflineNoteTransferTextPayloadCodec.encode(payload, kind: .receiptAck)

        XCTAssertTrue(challenge.hasPrefix(OfflineNoteTransferTextPayloadCodec.receiveRequestPrefix))
        XCTAssertTrue(payment.hasPrefix(OfflineNoteTransferTextPayloadCodec.paymentTokenPrefix))
        XCTAssertTrue(ack.hasPrefix(OfflineNoteTransferTextPayloadCodec.receiptAckPrefix))
        XCTAssertEqual(
            OfflineNoteTransferTextPayloadCodec.payloadKind(for: challenge),
            .offlineReceiveRequest
        )
        XCTAssertEqual(
            OfflineNoteTransferTextPayloadCodec.payloadKind(for: payment),
            .offlinePaymentToken
        )
        XCTAssertEqual(
            OfflineNoteTransferTextPayloadCodec.payloadKind(for: ack),
            .offlineReceiptAck
        )
        XCTAssertEqual(
            try OfflineNoteTransferTextPayloadCodec.decode(challenge, expectedKind: .receiveRequest).payload,
            payload
        )
        XCTAssertThrowsError(try OfflineNoteTransferTextPayloadCodec.decode(payment, expectedKind: .receiveRequest))
    }

    func testCompatibilityReceiptAckTextCodecValidatesRequiredFields() throws {
        let ack = OfflineReceiptAck(
            tokenId: "token-1",
            recipientAccountId: "recipient@paynet",
            acceptedAtMs: 1_706_000_000_000
        )
        let encoded = try OfflineNoteTransferTextPayloadCodec.encodeReceiptAck(ack)

        XCTAssertEqual(try OfflineNoteTransferTextPayloadCodec.decodeReceiptAck(encoded), ack)
        XCTAssertThrowsError(
            try OfflineNoteTransferTextPayloadCodec.decodeReceiptAck(
                try OfflineNoteTransferTextPayloadCodec.encodeReceiptAck(
                    OfflineReceiptAck(tokenId: "", recipientAccountId: ack.recipientAccountId, acceptedAtMs: ack.acceptedAtMs)
                )
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteTransferTextPayloadCodec.decodeReceiptAck(
                try OfflineNoteTransferTextPayloadCodec.encodeReceiptAck(
                    OfflineReceiptAck(tokenId: ack.tokenId, recipientAccountId: " ", acceptedAtMs: ack.acceptedAtMs)
                )
            )
        )
        XCTAssertThrowsError(
            try OfflineNoteTransferTextPayloadCodec.decodeReceiptAck(
                try OfflineNoteTransferTextPayloadCodec.encodeReceiptAck(
                    OfflineReceiptAck(tokenId: ack.tokenId, recipientAccountId: ack.recipientAccountId, acceptedAtMs: 0)
                )
            )
        )
    }

    func testTransferTextNearbyEnvelopeRoundTripsKinds() throws {
        let payloads = try OfflineBearerWalletTests.bearerTextPayloadFixture()
        let challenge = payloads.receiveRequest
        let payment = payloads.payment
        let ack = payloads.ack
        let pairing = try OfflineNoteNearbyPairingChallenge(assetName: "nearby_pairing_stars")

        let challengeBytes = try OfflineNoteTransferHandoff.nearbyTextEnvelopeBytes(
            payload: challenge,
            kind: .receiveRequest,
            pairingChallenge: pairing
        )
        let paymentBytes = try OfflineNoteTransferHandoff.nearbyTextEnvelopeBytes(
            payload: payment,
            kind: .paymentToken
        )
        let ackBytes = try OfflineNoteTransferHandoff.nearbyTextEnvelopeBytes(
            payload: ack,
            kind: .receiptAck
        )

        let decodedChallenge = try OfflineNoteTransferHandoff.decodeNearbyTextPayload(
            from: challengeBytes,
            expectedKind: .receiveRequest
        )
        let decodedPayment = try OfflineNoteTransferHandoff.decodeNearbyTextPayload(
            from: paymentBytes,
            expectedKind: .paymentToken
        )
        let decodedAck = try OfflineNoteTransferHandoff.decodeNearbyTextPayload(
            from: ackBytes,
            expectedKind: .receiptAck
        )

        XCTAssertEqual(decodedChallenge.kind, .receiveRequest)
        XCTAssertEqual(decodedChallenge.payload, challenge)
        XCTAssertEqual(decodedChallenge.pairingChallenge, pairing)
        XCTAssertEqual(decodedPayment.kind, .paymentToken)
        XCTAssertEqual(decodedPayment.payload, payment)
        XCTAssertEqual(decodedAck.kind, .receiptAck)
        XCTAssertEqual(decodedAck.payload, ack)
        XCTAssertThrowsError(try OfflineNoteTransferHandoff.decodeNearbyTextPayload(from: paymentBytes, expectedKind: .receiptAck))
    }

    func testQrStreamRejectsAdversarialEnvelopeAndChunkShapes() throws {
        let payload = makePayload(length: 300)
        let frames = try OfflineQrStreamEncoder.encodeFrames(
            payload: payload,
            payloadKind: .offlinePaymentToken,
            options: OfflineQrStreamOptions(chunkSize: 100, parityGroup: 2)
        )
        let header = try XCTUnwrap(frames.first(where: { $0.kind == .header }))
        let dataFrames = frames.filter { $0.kind == .data }
        let parityFrames = frames.filter { $0.kind == .parity }
        let firstData = try XCTUnwrap(dataFrames.first)
        let firstParity = try XCTUnwrap(parityFrames.first)

        XCTAssertThrowsError(
            try OfflineQrStreamFrame(
                kind: .data,
                streamId: header.streamId,
                index: 0,
                total: 1,
                payload: Data(repeating: 0, count: Int(UInt16.max) + 1)
            )
        )

        var trailingFrame = header.encode()
        trailingFrame.append(0x00)
        XCTAssertThrowsError(try OfflineQrStreamFrame.decode(trailingFrame))

        var unknownFrameKind = header.encode()
        unknownFrameKind[unknownFrameKind.startIndex + 2] = 0x7f
        XCTAssertThrowsError(try OfflineQrStreamFrame.decode(unknownFrameKind))

        XCTAssertThrowsError(
            try OfflineQrStreamDecoder().ingest(
                frameBytes: OfflineQrStreamFrame(
                    kind: .header,
                    streamId: Data(repeating: 0xa5, count: 16),
                    index: 0,
                    total: 1,
                    payload: header.payload
                ).encode()
            )
        )

        XCTAssertThrowsError(
            try OfflineQrStreamDecoder().ingest(
                frameBytes: OfflineQrStreamFrame(
                    kind: .header,
                    streamId: header.streamId,
                    index: 1,
                    total: 1,
                    payload: header.payload
                ).encode()
            )
        )

        XCTAssertThrowsError(
            try OfflineQrStreamDecoder().ingest(frameBytes: mutatedHeaderFrame(header) { envelope in
                envelope.append(0x00)
            })
        )
        XCTAssertThrowsError(
            try OfflineQrStreamDecoder().ingest(frameBytes: mutatedHeaderFrame(header) { envelope in
                envelope[envelope.startIndex + 1] = 0x7f
            })
        )
        XCTAssertThrowsError(
            try OfflineQrStreamDecoder().ingest(frameBytes: mutatedHeaderFrame(header) { envelope in
                setUInt16LE(&envelope, offset: 3, value: 0)
            })
        )
        XCTAssertThrowsError(
            try OfflineQrStreamDecoder().ingest(frameBytes: mutatedHeaderFrame(header) { envelope in
                setUInt16LE(&envelope, offset: 5, value: 1)
            })
        )
        XCTAssertThrowsError(
            try OfflineQrStreamDecoder().ingest(frameBytes: mutatedHeaderFrame(header) { envelope in
                setUInt16LE(&envelope, offset: 7, value: 0)
            })
        )
        XCTAssertThrowsError(
            try OfflineQrStreamDecoder().ingest(frameBytes: mutatedHeaderFrame(header) { envelope in
                envelope[envelope.startIndex] = 0x01
            })
        )

        let repeatedHeaderDecoder = OfflineQrStreamDecoder()
        _ = try repeatedHeaderDecoder.ingest(frameBytes: header.encode())
        XCTAssertNoThrow(try repeatedHeaderDecoder.ingest(frameBytes: header.encode()))
        XCTAssertThrowsError(
            try repeatedHeaderDecoder.ingest(frameBytes: mutatedHeaderFrame(header) { envelope in
                setUInt16LE(&envelope, offset: 9, value: OfflineQrPayloadKind.offlineReceiveRequest.rawValue)
            })
        )

        let decoder = OfflineQrStreamDecoder()
        _ = try decoder.ingest(frameBytes: header.encode())
        XCTAssertThrowsError(
            try decoder.ingest(
                frameBytes: OfflineQrStreamFrame(
                    kind: .data,
                    streamId: firstData.streamId,
                    index: firstData.index,
                    total: firstData.total,
                    payload: Data(firstData.payload.dropLast())
                ).encode()
            )
        )

        let longDataDecoder = OfflineQrStreamDecoder()
        _ = try longDataDecoder.ingest(frameBytes: header.encode())
        XCTAssertThrowsError(
            try longDataDecoder.ingest(
                frameBytes: OfflineQrStreamFrame(
                    kind: .data,
                    streamId: firstData.streamId,
                    index: firstData.index,
                    total: firstData.total,
                    payload: firstData.payload + Data([0x00])
                ).encode()
            )
        )

        let wrongTotalDecoder = OfflineQrStreamDecoder()
        _ = try wrongTotalDecoder.ingest(frameBytes: header.encode())
        XCTAssertThrowsError(
            try wrongTotalDecoder.ingest(
                frameBytes: OfflineQrStreamFrame(
                    kind: .data,
                    streamId: firstData.streamId,
                    index: firstData.index,
                    total: firstData.total + 1,
                    payload: firstData.payload
                ).encode()
            )
        )

        let pendingBadDataDecoder = OfflineQrStreamDecoder()
        _ = try pendingBadDataDecoder.ingest(
            frameBytes: OfflineQrStreamFrame(
                kind: .data,
                streamId: firstData.streamId,
                index: firstData.index,
                total: firstData.total + 1,
                payload: firstData.payload
            ).encode()
        )
        XCTAssertThrowsError(try pendingBadDataDecoder.ingest(frameBytes: header.encode()))

        let conflictingDataDecoder = OfflineQrStreamDecoder()
        _ = try conflictingDataDecoder.ingest(frameBytes: header.encode())
        _ = try conflictingDataDecoder.ingest(frameBytes: firstData.encode())
        var conflictingDataPayload = firstData.payload
        conflictingDataPayload[conflictingDataPayload.startIndex] ^= 0xff
        XCTAssertThrowsError(
            try conflictingDataDecoder.ingest(
                frameBytes: OfflineQrStreamFrame(
                    kind: .data,
                    streamId: firstData.streamId,
                    index: firstData.index,
                    total: firstData.total,
                    payload: conflictingDataPayload
                ).encode()
            )
        )

        let poisonedParityDecoder = OfflineQrStreamDecoder()
        _ = try poisonedParityDecoder.ingest(frameBytes: header.encode())
        _ = try poisonedParityDecoder.ingest(frameBytes: firstData.encode())
        var poisonedParityPayload = firstParity.payload
        poisonedParityPayload[poisonedParityPayload.startIndex] ^= 0xff
        _ = try poisonedParityDecoder.ingest(
            frameBytes: OfflineQrStreamFrame(
                kind: .parity,
                streamId: firstParity.streamId,
                index: firstParity.index,
                total: firstParity.total,
                payload: poisonedParityPayload
            ).encode()
        )
        XCTAssertThrowsError(
            try poisonedParityDecoder.ingest(frameBytes: dataFrames[1].encode())
        )

        let hashMismatchDecoder = OfflineQrStreamDecoder()
        _ = try hashMismatchDecoder.ingest(frameBytes: header.encode())
        var mutatedFirstDataPayload = firstData.payload
        mutatedFirstDataPayload[mutatedFirstDataPayload.startIndex] ^= 0xff
        _ = try hashMismatchDecoder.ingest(
            frameBytes: OfflineQrStreamFrame(
                kind: .data,
                streamId: firstData.streamId,
                index: firstData.index,
                total: firstData.total,
                payload: mutatedFirstDataPayload
            ).encode()
        )
        _ = try hashMismatchDecoder.ingest(frameBytes: dataFrames[1].encode())
        XCTAssertThrowsError(try hashMismatchDecoder.ingest(frameBytes: dataFrames[2].encode()))

        let shortParityDecoder = OfflineQrStreamDecoder()
        _ = try shortParityDecoder.ingest(frameBytes: header.encode())
        XCTAssertThrowsError(
            try shortParityDecoder.ingest(
                frameBytes: OfflineQrStreamFrame(
                    kind: .parity,
                    streamId: firstParity.streamId,
                    index: firstParity.index,
                    total: firstParity.total,
                    payload: Data(firstParity.payload.dropLast())
                ).encode()
            )
        )

        let conflictingParityDecoder = OfflineQrStreamDecoder()
        _ = try conflictingParityDecoder.ingest(frameBytes: header.encode())
        _ = try conflictingParityDecoder.ingest(frameBytes: firstParity.encode())
        var conflictingParityPayload = firstParity.payload
        conflictingParityPayload[conflictingParityPayload.startIndex] ^= 0xff
        XCTAssertThrowsError(
            try conflictingParityDecoder.ingest(
                frameBytes: OfflineQrStreamFrame(
                    kind: .parity,
                    streamId: firstParity.streamId,
                    index: firstParity.index,
                    total: firstParity.total,
                    payload: conflictingParityPayload
                ).encode()
            )
        )
    }

    func testQrStreamTextCodecRoundTrip() throws {
        let payload = makePayload(length: 128)
        let encoded = OfflineQrStreamTextCodec.encode(payload, encoding: .base64)
        let decoded = try OfflineQrStreamTextCodec.decode(encoded, encoding: .base64)
        XCTAssertEqual(decoded, payload)
    }

    func testSakuraStormPlaybackSkinMatchesPreset() {
        XCTAssertEqual(OfflineQrStreamPlaybackSkin.sakuraStorm.name, "sakura-storm")
        XCTAssertEqual(OfflineQrStreamPlaybackSkin.sakuraStorm.frameRate, 12)
        XCTAssertEqual(OfflineQrStreamPlaybackSkin.sakuraStorm.petalDriftSpeed, 0.6)
        XCTAssertEqual(OfflineQrStreamPlaybackSkin.sakuraStorm.progressOverlayAlpha, 0.34)
        XCTAssertEqual(OfflineQrStreamPlaybackSkin.sakuraStorm.theme.backgroundStart.red, 0.05)
        XCTAssertEqual(OfflineQrStreamPlaybackSkin.sakuraStorm.theme.backgroundEnd.blue, 0.04)
    }

    func testSakuraStormScanSessionPresetRecoversDroppedFrame() throws {
        let payload = makePayload(length: 6 * 1024)
        let frames = try OfflineQrStreamEncoder.encodeFrames(
            payload: payload,
            payloadKind: .offlinePaymentToken,
            options: OfflineQrStreamOptions(chunkSize: 336, parityGroup: 4)
        )
        let header = try XCTUnwrap(frames.first(where: { $0.kind == .header }))
        let dataFrames = frames.filter { $0.kind == .data }
        let parityFrames = frames.filter { $0.kind == .parity }
        XCTAssertFalse(dataFrames.isEmpty)
        XCTAssertFalse(parityFrames.isEmpty)

        let dropped = try XCTUnwrap(dataFrames.first(where: { $0.index == 1 }))
        let session = OfflineQrStreamScanSession()
        _ = try session.ingest(
            frameString: OfflineQrStreamTextCodec.encode(header.encode(), encoding: .base64),
            encoding: .base64
        )

        var result: OfflineQrStreamDecodeResult?
        for frame in dataFrames where frame.index != dropped.index {
            result = try session.ingest(
                frameString: OfflineQrStreamTextCodec.encode(frame.encode(), encoding: .base64),
                encoding: .base64
            )
        }
        for frame in parityFrames {
            result = try session.ingest(
                frameString: OfflineQrStreamTextCodec.encode(frame.encode(), encoding: .base64),
                encoding: .base64
            )
        }
        let final = try XCTUnwrap(result)
        XCTAssertTrue(final.isComplete)
        XCTAssertEqual(final.payload, payload)
        XCTAssertEqual(final.recoveredChunks, 1)
    }
    /// Full chain: encode → TextCodec → CIFilter QR → platform QR detect → TextCodec decode → ingest
    func testQrStreamFullChainViaQRImage() throws {
        #if canImport(CoreImage) && canImport(Vision)
        let payload = makePayload(length: 256)
        let options = OfflineQrStreamOptions(chunkSize: 100, parityGroup: 0)
        let frameBytesList = try OfflineQrStreamEncoder.encodeFrameBytes(
            payload: payload,
            payloadKind: .offlinePaymentToken,
            options: options
        )
        XCTAssertFalse(frameBytesList.isEmpty, "Should produce at least 1 frame")

        // Convert each frame to text QR string (matching sender)
        let textFrames = frameBytesList.map {
            OfflineQrStreamTextCodec.encode($0, encoding: .base64)
        }
        print("[Test] Produced \(textFrames.count) text frames")
        print("[Test] Frame[0] prefix: \(String(textFrames[0].prefix(50)))")

        let decoder = OfflineQrStreamDecoder()
        var lastResult: OfflineQrStreamDecodeResult?
        let context = CIContext()
        try requireQrDetectorAvailable(context: context)

        for (index, textFrame) in textFrames.enumerated() {
            // Step 1: Generate QR image with CIFilter (same as sender)
            guard let textData = textFrame.data(using: .utf8) else {
                XCTFail("Frame \(index): cannot encode to UTF-8")
                continue
            }
            guard let filter = CIFilter(name: "CIQRCodeGenerator") else {
                XCTFail("CIQRCodeGenerator unavailable")
                return
            }
            filter.setValue(textData, forKey: "inputMessage")
            filter.setValue("M", forKey: "inputCorrectionLevel")
            guard let ciImage = filter.outputImage else {
                XCTFail("Frame \(index): CIFilter produced no output")
                continue
            }

            guard let cgImage = renderedQrImage(ciImage, context: context) else {
                XCTFail("Frame \(index): cannot create CGImage")
                continue
            }
            let renderedImage = CIImage(cgImage: cgImage)

            // Step 2: Detect with Vision, falling back to CoreImage on simulator runtimes
            // where Vision barcode inference resources are not installed.
            guard let detected = try detectQrString(cgImage: cgImage, ciImage: renderedImage, context: context) else {
                XCTFail("Frame \(index): QR detector found no payload")
                continue
            }
            print("[Test] Frame[\(index)] detected string prefix: \(String(detected.prefix(50)))")

            // Verify roundtrip fidelity
            XCTAssertEqual(detected, textFrame, "Frame \(index): Vision string != original text")

            // Step 4: Decode through TextCodec
            let frameBytes = try OfflineQrStreamTextCodec.decode(detected, encoding: .base64)

            // Verify binary matches encoder output
            XCTAssertEqual(frameBytes, frameBytesList[index], "Frame \(index): decoded bytes != original frame bytes")

            // Step 5: Ingest into decoder
            lastResult = try decoder.ingest(frameBytes: frameBytes)
            print("[Test] Frame[\(index)] progress: \(lastResult!.receivedChunks)/\(lastResult!.totalChunks)")
        }

        guard let finalResult = lastResult else {
            return XCTFail("No frames ingested")
        }
        XCTAssertTrue(finalResult.isComplete, "Stream should be complete")
        XCTAssertEqual(finalResult.payload, payload, "Decoded payload should match original")
        print("[Test] SUCCESS: decoded \(finalResult.payload?.count ?? 0) bytes")
        #else
        throw XCTSkip("Requires CoreImage and Vision frameworks")
        #endif
    }

    private func detectQrString(cgImage: CGImage, ciImage: CIImage, context: CIContext) throws -> String? {
        #if canImport(Vision)
        let request = VNDetectBarcodesRequest()
        request.symbologies = [.qr]
        let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
        do {
            try handler.perform([request])
            if let detected = request.results?.compactMap(\.payloadStringValue).first {
                return detected
            }
        } catch {
            print("[Test] Vision QR detection unavailable, falling back to CoreImage: \(error)")
        }
        #endif
        let detector = CIDetector(
            ofType: CIDetectorTypeQRCode,
            context: context,
            options: [CIDetectorAccuracy: CIDetectorAccuracyHigh]
        )
        return detector?
            .features(in: ciImage)
            .compactMap { ($0 as? CIQRCodeFeature)?.messageString }
            .first
    }

    private func requireQrDetectorAvailable(context: CIContext) throws {
        let sentinel = "qr-detector-health-check"
        guard let textData = sentinel.data(using: .utf8),
              let filter = CIFilter(name: "CIQRCodeGenerator")
        else {
            throw XCTSkip("QR detector health check could not be built")
        }
        filter.setValue(textData, forKey: "inputMessage")
        filter.setValue("M", forKey: "inputCorrectionLevel")
        guard let ciImage = filter.outputImage,
              let cgImage = renderedQrImage(ciImage, context: context)
        else {
            throw XCTSkip("QR detector health check image could not be rendered")
        }
        let detected = try detectQrString(cgImage: cgImage, ciImage: CIImage(cgImage: cgImage), context: context)
        guard detected == sentinel else {
            throw XCTSkip("Platform QR detector unavailable in this runtime")
        }
    }

    private func renderedQrImage(
        _ image: CIImage,
        context: CIContext,
        quietZoneModules: Int = 4,
        scale: Int = 10
    ) -> CGImage? {
        guard let rawImage = context.createCGImage(image, from: image.extent) else {
            return nil
        }
        let width = (rawImage.width + quietZoneModules * 2) * scale
        let height = (rawImage.height + quietZoneModules * 2) * scale
        guard let bitmap = CGContext(
            data: nil,
            width: width,
            height: height,
            bitsPerComponent: 8,
            bytesPerRow: width * 4,
            space: CGColorSpaceCreateDeviceRGB(),
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ) else {
            return nil
        }
        bitmap.setFillColor(CGColor(red: 1, green: 1, blue: 1, alpha: 1))
        bitmap.fill(CGRect(x: 0, y: 0, width: width, height: height))
        bitmap.interpolationQuality = .none
        bitmap.draw(
            rawImage,
            in: CGRect(
                x: quietZoneModules * scale,
                y: quietZoneModules * scale,
                width: rawImage.width * scale,
                height: rawImage.height * scale
            )
        )
        return bitmap.makeImage()
    }

    private func makePayload(length: Int) -> Data {
        var bytes = [UInt8](repeating: 0, count: length)
        for index in bytes.indices {
            bytes[index] = UInt8((index * 31 + 7) % 256)
        }
        return Data(bytes)
    }

    private static func fixturePaymentTokenText() throws -> String {
        let testFile = URL(fileURLWithPath: #filePath)
        let fixtureURL = testFile
            .deletingLastPathComponent()
            .appendingPathComponent("../../../fixtures/offline/interop_contract.json")
            .standardizedFileURL
        let data = try Data(contentsOf: fixtureURL)
        return try JSONDecoder().decode(TextNearbyInteropFixture.self, from: data).sdkInterop.paymentTokenText
    }

    private func mutatedHeaderFrame(
        _ header: OfflineQrStreamFrame,
        mutate: (inout Data) -> Void
    ) throws -> Data {
        var envelope = header.payload
        mutate(&envelope)
        return try OfflineQrStreamFrame(
            kind: .header,
            streamId: header.streamId,
            index: header.index,
            total: header.total,
            payload: envelope
        ).encode()
    }

    private func setUInt16LE(_ data: inout Data, offset: Int, value: UInt16) {
        data[data.startIndex + offset] = UInt8(value & 0xff)
        data[data.startIndex + offset + 1] = UInt8((value >> 8) & 0xff)
    }
}

private struct TextNearbyInteropFixture: Decodable {
    let sdkInterop: TextNearbySdkInterop

    private enum CodingKeys: String, CodingKey {
        case sdkInterop = "sdk_interop"
    }
}

private struct TextNearbySdkInterop: Decodable {
    let paymentTokenText: String

    private enum CodingKeys: String, CodingKey {
        case paymentTokenText = "payment_token_text"
    }
}
