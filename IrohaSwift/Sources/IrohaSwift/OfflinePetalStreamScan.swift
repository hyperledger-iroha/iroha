#if canImport(AVFoundation)
import AVFoundation
import Foundation

public final class OfflinePetalStreamCameraSession: NSObject {
    public let captureSession: AVCaptureSession
    public let scanSession: OfflinePetalStreamScanSession
    public var onProgress: ((OfflineQrStreamDecodeResult) -> Void)?
    public var onPayload: ((Data) -> Void)?
    public var onError: ((Error) -> Void)?

    private let output: AVCaptureVideoDataOutput
    private let options: OfflinePetalStreamOptions
    private var resolvedGridSize: UInt16?

    public init(
        scanSession: OfflinePetalStreamScanSession = OfflinePetalStreamScanSession(),
        options: OfflinePetalStreamOptions = OfflinePetalStreamOptions()
    ) {
        self.captureSession = AVCaptureSession()
        self.scanSession = scanSession
        self.output = AVCaptureVideoDataOutput()
        self.options = options
        self.resolvedGridSize = options.gridSize == 0 ? nil : options.gridSize
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
        output.alwaysDiscardsLateVideoFrames = true
        output.videoSettings = [
            kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
        ]
        let queue = DispatchQueue(label: "iroha.petalstream.capture")
        output.setSampleBufferDelegate(self, queue: queue)
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

extension OfflinePetalStreamCameraSession: AVCaptureVideoDataOutputSampleBufferDelegate {
    public func captureOutput(
        _ output: AVCaptureOutput,
        didOutput sampleBuffer: CMSampleBuffer,
        from connection: AVCaptureConnection
    ) {
        guard let imageBuffer = CMSampleBufferGetImageBuffer(sampleBuffer) else {
            return
        }
        do {
            let frameBytes = try decodeFrame(from: imageBuffer)
            guard let result = try? scanSession.qrSession.ingest(frameBytes: frameBytes) else {
                return
            }
            onProgress?(result)
            if let payload = result.payload {
                onPayload?(payload)
            }
        } catch {
            onError?(error)
        }
    }
}

private extension OfflinePetalStreamCameraSession {
    func decodeFrame(from pixelBuffer: CVPixelBuffer) throws -> Data {
        if let gridSize = resolvedGridSize {
            let samples = try sampleGrid(from: pixelBuffer, gridSize: gridSize)
            return try OfflinePetalStreamDecoder.decodeSamples(samples, options: options)
        }
        let autoOptions = OfflinePetalStreamOptions(
            gridSize: 0,
            border: options.border,
            anchorSize: options.anchorSize
        )
        for candidate in OfflinePetalStreamEncoder.gridSizes {
            if let samples = try? sampleGrid(from: pixelBuffer, gridSize: candidate),
               let payload = try? OfflinePetalStreamDecoder.decodeSamples(samples, options: autoOptions) {
                resolvedGridSize = candidate
                return payload
            }
        }
        throw OfflinePetalStreamError.invalidOptions("auto-detect failed")
    }

    func sampleGrid(from pixelBuffer: CVPixelBuffer, gridSize: UInt16) throws -> OfflinePetalStreamSampleGrid {
        CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }
        let width = CVPixelBufferGetWidth(pixelBuffer)
        let height = CVPixelBufferGetHeight(pixelBuffer)
        if CVPixelBufferGetPlaneCount(pixelBuffer) > 0,
           let base = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 0) {
            let rowStride = CVPixelBufferGetBytesPerRowOfPlane(pixelBuffer, 0)
            let luma = base.assumingMemoryBound(to: UInt8.self)
            return try sampleGridFromLuma(
                luma: luma,
                width: width,
                height: height,
                rowStride: rowStride,
                gridSize: gridSize
            )
        }
        guard let base = CVPixelBufferGetBaseAddress(pixelBuffer) else {
            throw OfflinePetalStreamError.invalidOptions("pixel buffer missing base address")
        }
        let rowStride = CVPixelBufferGetBytesPerRow(pixelBuffer)
        let pixels = base.assumingMemoryBound(to: UInt8.self)
        return try sampleGridFromBGRA(
            pixels: pixels,
            width: width,
            height: height,
            rowStride: rowStride,
            gridSize: gridSize
        )
    }

    func sampleGridFromLuma(
        luma: UnsafePointer<UInt8>,
        width: Int,
        height: Int,
        rowStride: Int,
        gridSize: UInt16
    ) throws -> OfflinePetalStreamSampleGrid {
        guard width > 0, height > 0 else {
            throw OfflinePetalStreamError.invalidOptions("image dimensions must be positive")
        }
        let size = min(width, height)
        let offsetX = (width - size) / 2
        let offsetY = (height - size) / 2
        let cellSize = max(1, size / Int(gridSize))
        var samples: [UInt8] = []
        samples.reserveCapacity(Int(gridSize) * Int(gridSize))
        for y in 0..<gridSize {
            for x in 0..<gridSize {
                var sum = 0
                var count = 0
                for oy in [0.25, 0.5, 0.75] {
                    for ox in [0.25, 0.5, 0.75] {
                        let px = min(
                            width - 1,
                            offsetX + Int((Double(x) + ox) * Double(cellSize))
                        )
                        let py = min(
                            height - 1,
                            offsetY + Int((Double(y) + oy) * Double(cellSize))
                        )
                        let value = Int(luma[py * rowStride + px])
                        sum += value
                        count += 1
                    }
                }
                let avg = count == 0 ? 0 : sum / count
                samples.append(UInt8(avg))
            }
        }
        return try OfflinePetalStreamSampleGrid(gridSize: gridSize, samples: samples)
    }

    func sampleGridFromBGRA(
        pixels: UnsafePointer<UInt8>,
        width: Int,
        height: Int,
        rowStride: Int,
        gridSize: UInt16
    ) throws -> OfflinePetalStreamSampleGrid {
        guard width > 0, height > 0 else {
            throw OfflinePetalStreamError.invalidOptions("image dimensions must be positive")
        }
        let size = min(width, height)
        let offsetX = (width - size) / 2
        let offsetY = (height - size) / 2
        let cellSize = max(1, size / Int(gridSize))
        var samples: [UInt8] = []
        samples.reserveCapacity(Int(gridSize) * Int(gridSize))
        for y in 0..<gridSize {
            for x in 0..<gridSize {
                var sum = 0
                var count = 0
                for oy in [0.25, 0.5, 0.75] {
                    for ox in [0.25, 0.5, 0.75] {
                        let px = min(
                            width - 1,
                            offsetX + Int((Double(x) + ox) * Double(cellSize))
                        )
                        let py = min(
                            height - 1,
                            offsetY + Int((Double(y) + oy) * Double(cellSize))
                        )
                        let offset = py * rowStride + px * 4
                        let b = Int(pixels[offset])
                        let g = Int(pixels[offset + 1])
                        let r = Int(pixels[offset + 2])
                        let luma = (77 * r + 150 * g + 29 * b) >> 8
                        sum += luma
                        count += 1
                    }
                }
                let avg = count == 0 ? 0 : sum / count
                samples.append(UInt8(avg))
            }
        }
        return try OfflinePetalStreamSampleGrid(gridSize: gridSize, samples: samples)
    }
}
#endif
