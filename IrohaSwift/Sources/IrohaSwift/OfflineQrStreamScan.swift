#if canImport(AVFoundation) && canImport(Vision)
import AVFoundation
import Foundation
import Vision

public final class OfflineQrStreamVisionScanner {
    private let request: VNDetectBarcodesRequest

    public init() {
        request = VNDetectBarcodesRequest()
        request.symbologies = [.qr]
    }

    public func decode(sampleBuffer: CMSampleBuffer) throws -> [Data] {
        guard let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else {
            return []
        }
        return try decode(pixelBuffer: imageBuffer)
    }

    public func decode(pixelBuffer: CVPixelBuffer) throws -> [Data] {
        let handler = VNImageRequestHandler(cvPixelBuffer: pixelBuffer, options: [:])
        try handler.perform([request])
        guard let results = request.results else {
            return []
        }
        return results.compactMap { observation -> Data? in
            guard let string = observation.payloadStringValue else {
                return nil
            }
            do {
                return try OfflineQrStreamTextCodec.decode(string, encoding: .base64)
            } catch {
                return nil
            }
        }
    }
}

public final class OfflineQrStreamCameraSession: NSObject {
    public let captureSession: AVCaptureSession
    public let scanSession: OfflineQrStreamScanSession
    public var onProgress: ((OfflineQrStreamDecodeResult) -> Void)?
    public var onPayload: ((Data) -> Void)?

    private let scanner: OfflineQrStreamVisionScanner
    private let output: AVCaptureVideoDataOutput

    public init(scanSession: OfflineQrStreamScanSession = OfflineQrStreamScanSession()) {
        self.captureSession = AVCaptureSession()
        self.scanSession = scanSession
        self.scanner = OfflineQrStreamVisionScanner()
        self.output = AVCaptureVideoDataOutput()
        super.init()
        configureSession()
    }

    private func configureSession() {
        captureSession.beginConfiguration()
        captureSession.sessionPreset = .high
        guard let device = AVCaptureDevice.default(for: .video) else {
            captureSession.commitConfiguration()
            return
        }
        guard let input = try? AVCaptureDeviceInput(device: device) else {
            captureSession.commitConfiguration()
            return
        }
        if captureSession.canAddInput(input) {
            captureSession.addInput(input)
        }
        let queue = DispatchQueue(label: "iroha.qrstream.capture")
        output.setSampleBufferDelegate(self, queue: queue)
        output.alwaysDiscardsLateVideoFrames = true
        if captureSession.canAddOutput(output) {
            captureSession.addOutput(output)
        }
        captureSession.commitConfiguration()
    }

    public func start() {
        if !captureSession.isRunning {
            captureSession.startRunning()
        }
    }

    public func stop() {
        if captureSession.isRunning {
            captureSession.stopRunning()
        }
    }
}

extension OfflineQrStreamCameraSession: AVCaptureVideoDataOutputSampleBufferDelegate {
    public func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        let frames: [Data]
        do {
            frames = try scanner.decode(sampleBuffer: sampleBuffer)
        } catch {
            return
        }
        for frame in frames {
            do {
                let result = try scanSession.ingest(frameBytes: frame)
                onProgress?(result)
                if let payload = result.payload {
                    onPayload?(payload)
                }
            } catch {
            }
        }
    }
}
#endif
