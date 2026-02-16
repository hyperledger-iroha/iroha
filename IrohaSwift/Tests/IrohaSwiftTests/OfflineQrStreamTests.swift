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
            payloadKind: .offlineSpendReceipt,
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
            payloadKind: .offlineSpendReceipt,
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

    /// Full chain: encode → TextCodec → CIFilter QR → Vision detect → TextCodec decode → ingest
    func testQrStreamFullChainViaQRImage() throws {
        #if canImport(CoreImage) && canImport(Vision)
        let payload = makePayload(length: 256)
        let options = OfflineQrStreamOptions(chunkSize: 100, parityGroup: 0)
        let frameBytesList = try OfflineQrStreamEncoder.encodeFrameBytes(
            payload: payload,
            payloadKind: .offlineSpendReceipt,
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

            // Scale QR to reasonable pixel size for Vision
            let scale = CGAffineTransform(scaleX: 10, y: 10)
            let scaledImage = ciImage.transformed(by: scale)
            let context = CIContext()
            guard let cgImage = context.createCGImage(scaledImage, from: scaledImage.extent) else {
                XCTFail("Frame \(index): cannot create CGImage")
                continue
            }

            // Step 2: Detect with Vision
            let request = VNDetectBarcodesRequest()
            request.symbologies = [.qr]
            let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
            try handler.perform([request])
            guard let results = request.results, !results.isEmpty else {
                XCTFail("Frame \(index): Vision detected 0 barcodes")
                continue
            }

            // Step 3: Read payloadStringValue
            guard let detected = results.first?.payloadStringValue else {
                XCTFail("Frame \(index): no payloadStringValue")
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

    private func makePayload(length: Int) -> Data {
        var bytes = [UInt8](repeating: 0, count: length)
        for index in bytes.indices {
            bytes[index] = UInt8((index * 31 + 7) % 256)
        }
        return Data(bytes)
    }
}
