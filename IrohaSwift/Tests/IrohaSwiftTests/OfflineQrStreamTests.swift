import Foundation
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

    private func makePayload(length: Int) -> Data {
        var bytes = [UInt8](repeating: 0, count: length)
        for index in bytes.indices {
            bytes[index] = UInt8((index * 31 + 7) % 256)
        }
        return Data(bytes)
    }
}
