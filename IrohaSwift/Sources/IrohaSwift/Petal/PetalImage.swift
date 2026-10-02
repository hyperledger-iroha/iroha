import Foundation

/// Failures when wrapping or producing Petal pixel buffers.
public enum PetalImageError: Error, Equatable, LocalizedError, Sendable {
    /// The dimensions are negative, overflow, or do not match the buffer.
    case invalidDimensions
    /// Render options are outside their supported range.
    case invalidRenderOptions

    public var errorDescription: String? {
        switch self {
        case .invalidDimensions: return "Petal image dimensions do not match the pixel buffer."
        case .invalidRenderOptions: return "Petal render size must be positive and supersampling 1-4."
        }
    }
}

/// A single-channel 8-bit image, for example a camera luma (Y) plane.
///
/// Pixel `(i, j)` covers `[i, i+1) × [j, j+1)` and its centre is at
/// `(i + 0.5, j + 0.5)`. Every homography of the decoder maps into these
/// pixel-edge coordinates.
public struct PetalLuma: Equatable, Sendable {
    /// Width in pixels.
    public let width: Int
    /// Height in pixels.
    public let height: Int
    /// Row-major samples, `width * height` bytes.
    public let pixels: [UInt8]

    /// Creates a black image.
    public init(width: Int, height: Int) throws {
        guard width >= 0, height >= 0 else { throw PetalImageError.invalidDimensions }
        let (count, overflow) = width.multipliedReportingOverflow(by: height)
        guard !overflow else { throw PetalImageError.invalidDimensions }
        self.width = width
        self.height = height
        pixels = [UInt8](repeating: 0, count: count)
    }

    /// Wraps an existing row-major buffer of exactly `width * height` bytes.
    public init(width: Int, height: Int, pixels: [UInt8]) throws {
        guard width >= 0, height >= 0 else { throw PetalImageError.invalidDimensions }
        let (count, overflow) = width.multipliedReportingOverflow(by: height)
        guard !overflow, count == pixels.count else { throw PetalImageError.invalidDimensions }
        self.width = width
        self.height = height
        self.pixels = pixels
    }

    /// Copies a strided plane (for example the Y plane of a bi-planar camera
    /// frame), dropping the row padding.
    ///
    /// - Throws: ``PetalImageError/invalidDimensions`` when `bytesPerRow` is
    ///   smaller than `width` or the plane is too short.
    public init(width: Int, height: Int, bytesPerRow: Int, plane: UnsafeRawBufferPointer) throws {
        guard width > 0, height > 0, bytesPerRow >= width else {
            throw PetalImageError.invalidDimensions
        }
        let (rows, rowOverflow) = bytesPerRow.multipliedReportingOverflow(by: height - 1)
        let (required, overflow) = rows.addingReportingOverflow(width)
        guard !rowOverflow, !overflow, plane.count >= required,
              let base = plane.baseAddress else {
            throw PetalImageError.invalidDimensions
        }
        var pixels = [UInt8](repeating: 0, count: width * height)
        pixels.withUnsafeMutableBytes { destination in
            guard let target = destination.baseAddress else { return }
            for row in 0..<height {
                (target + row * width).copyMemory(from: base + row * bytesPerRow, byteCount: width)
            }
        }
        self.width = width
        self.height = height
        self.pixels = pixels
    }

    /// Copies a strided plane held in an array.
    public init(width: Int, height: Int, bytesPerRow: Int, plane: [UInt8]) throws {
        self = try plane.withUnsafeBytes {
            try PetalLuma(width: width, height: height, bytesPerRow: bytesPerRow, plane: $0)
        }
    }

    init(validatedWidth width: Int, height: Int, pixels: [UInt8]) {
        self.width = width
        self.height = height
        self.pixels = pixels
    }

    /// Reads pixel `(x, y)`; `nil` outside the image.
    public func pixel(x: Int, y: Int) -> UInt8? {
        guard x >= 0, y >= 0, x < width, y < height else { return nil }
        return pixels[y * width + x]
    }

    /// Bilinear sample at continuous pixel-edge coordinates, clamped to the
    /// image border. An empty image samples as zero.
    public func sample(x: Double, y: Double) -> Double {
        withView { $0.sample(x, y) }
    }

    /// Runs `body` with an unsafe view for the decoder's hot loops.
    func withView<Result>(_ body: (PetalLumaView) throws -> Result) rethrows -> Result {
        try pixels.withUnsafeBufferPointer { buffer in
            try body(PetalLumaView(buffer: buffer, width: width, height: height))
        }
    }
}

/// Borrowed, bounds-trusted view of a ``PetalLuma`` used inside hot loops.
struct PetalLumaView {
    let buffer: UnsafeBufferPointer<UInt8>
    let width: Int
    let height: Int
    private let maxX: Double
    private let maxY: Double

    init(buffer: UnsafeBufferPointer<UInt8>, width: Int, height: Int) {
        self.buffer = buffer
        self.width = width
        self.height = height
        maxX = Double(width - 1)
        maxY = Double(height - 1)
    }

    @inline(__always)
    func at(_ x: Int, _ y: Int) -> UInt8 {
        buffer[y * width + x]
    }

    /// Port of `Luma::sample`: bilinear interpolation between pixel centres.
    @inline(__always)
    func sample(_ x: Double, _ y: Double) -> Double {
        guard width > 0, height > 0 else { return 0 }
        let fx = PetalNumeric.clamp(x - 0.5, 0.0, maxX)
        let fy = PetalNumeric.clamp(y - 0.5, 0.0, maxY)
        let x0 = fx.isNaN ? 0 : Int(fx.rounded(.down))
        let y0 = fy.isNaN ? 0 : Int(fy.rounded(.down))
        let x1 = Swift.min(x0 + 1, width - 1)
        let y1 = Swift.min(y0 + 1, height - 1)
        let tx = fx - Double(x0)
        let ty = fy - Double(y0)
        let row0 = y0 * width
        let row1 = y1 * width
        let top = Double(buffer[row0 + x0]) * (1.0 - tx) + Double(buffer[row0 + x1]) * tx
        let bottom = Double(buffer[row1 + x0]) * (1.0 - tx) + Double(buffer[row1 + x1]) * tx
        return top * (1.0 - ty) + bottom * ty
    }
}

/// An interleaved 8-bit RGB image produced by ``PetalRenderer``.
public struct PetalRGBImage: Equatable, Sendable {
    /// Width in pixels.
    public let width: Int
    /// Height in pixels.
    public let height: Int
    /// Row-major `r, g, b` triples.
    public let pixels: [UInt8]

    /// Rec. 601 luma of the image (`(299 r + 587 g + 114 b + 500) / 1000`).
    public func luma() -> PetalLuma {
        var data = [UInt8](repeating: 0, count: width * height)
        pixels.withUnsafeBufferPointer { rgb in
            data.withUnsafeMutableBufferPointer { out in
                for index in 0..<out.count {
                    let r = UInt32(rgb[index * 3])
                    let g = UInt32(rgb[index * 3 + 1])
                    let b = UInt32(rgb[index * 3 + 2])
                    out[index] = UInt8(truncatingIfNeeded: (299 * r + 587 * g + 114 * b + 500) / 1000)
                }
            }
        }
        return PetalLuma(validatedWidth: width, height: height, pixels: data)
    }
}

/// A 3×3 plane projective transform, stored row-major.
public struct PetalHomography: Equatable, Sendable {
    let h0, h1, h2, h3, h4, h5, h6, h7, h8: Double

    /// The identity transform.
    public static let identity = PetalHomography(1, 0, 0, 0, 1, 0, 0, 0, 1)

    init(
        _ h0: Double, _ h1: Double, _ h2: Double,
        _ h3: Double, _ h4: Double, _ h5: Double,
        _ h6: Double, _ h7: Double, _ h8: Double
    ) {
        self.h0 = h0; self.h1 = h1; self.h2 = h2
        self.h3 = h3; self.h4 = h4; self.h5 = h5
        self.h6 = h6; self.h7 = h7; self.h8 = h8
    }

    /// Creates a homography from nine row-major coefficients.
    ///
    /// - Throws: ``PetalImageError/invalidDimensions`` unless exactly nine
    ///   coefficients are given.
    public init(elements: [Double]) throws {
        guard elements.count == 9 else { throw PetalImageError.invalidDimensions }
        self.init(
            elements[0], elements[1], elements[2],
            elements[3], elements[4], elements[5],
            elements[6], elements[7], elements[8]
        )
    }

    /// The nine row-major coefficients.
    public var elements: [Double] { [h0, h1, h2, h3, h4, h5, h6, h7, h8] }

    /// Maps a point.
    @inline(__always)
    public func apply(_ point: PetalPoint) -> PetalPoint {
        let (x, y) = apply(point.x, point.y)
        return PetalPoint(x: x, y: y)
    }

    @inline(__always)
    func apply(_ x: Double, _ y: Double) -> (Double, Double) {
        let w = h6 * x + h7 * y + h8
        return ((h0 * x + h1 * y + h2) / w, (h3 * x + h4 * y + h5) / w)
    }

    /// The inverse transform, or `nil` when singular.
    public func inverse() -> PetalHomography? {
        let c00 = h4 * h8 - h5 * h7
        let c01 = h5 * h6 - h3 * h8
        let c02 = h3 * h7 - h4 * h6
        let det = h0 * c00 + h1 * c01 + h2 * c02
        guard !(abs(det) < 1e-18) else { return nil }
        let inv = 1.0 / det
        return PetalHomography(
            c00 * inv,
            (h2 * h7 - h1 * h8) * inv,
            (h1 * h5 - h2 * h4) * inv,
            c01 * inv,
            (h0 * h8 - h2 * h6) * inv,
            (h2 * h3 - h0 * h5) * inv,
            c02 * inv,
            (h1 * h6 - h0 * h7) * inv,
            (h0 * h4 - h1 * h3) * inv
        )
    }

    /// `self * other` (apply `other` first).
    public func compose(_ other: PetalHomography) -> PetalHomography {
        let a = elements
        let b = other.elements
        var out = [Double](repeating: 0, count: 9)
        for r in 0..<3 {
            for c in 0..<3 {
                // Rust's float `Sum` starts from -0.0.
                var sum = -0.0
                for k in 0..<3 { sum += a[r * 3 + k] * b[k * 3 + c] }
                out[r * 3 + c] = sum
            }
        }
        return PetalHomography(out[0], out[1], out[2], out[3], out[4], out[5], out[6], out[7], out[8])
    }

    /// Fits the homography taking `source[i]` to `destination[i]` (least
    /// squares for more than four pairs, exact for four), using
    /// Hartley-normalised DLT and an 8×8 Gaussian elimination.
    ///
    /// Returns `nil` for fewer than four pairs, mismatched counts or a
    /// degenerate configuration.
    public static func fit(from source: [PetalPoint], to destination: [PetalPoint]) -> PetalHomography? {
        guard source.count == destination.count, source.count >= 4 else { return nil }
        let (ts, scaleS) = normalisation(source)
        let (td, scaleD) = normalisation(destination)
        var ata = [Double](repeating: 0, count: 64)
        var atb = [Double](repeating: 0, count: 8)
        for (s, d) in zip(source, destination) {
            let x = ts.h0 * s.x + ts.h2
            let y = ts.h4 * s.y + ts.h5
            let u = td.h0 * d.x + td.h2
            let v = td.h4 * d.y + td.h5
            let rows: [([Double], Double)] = [
                ([x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y], u),
                ([0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y], v),
            ]
            for (row, rhs) in rows {
                for i in 0..<8 {
                    for j in 0..<8 {
                        ata[i * 8 + j] += row[i] * row[j]
                    }
                    atb[i] += row[i] * rhs
                }
            }
        }
        guard let h = solve8(ata, atb) else { return nil }
        let normalised = PetalHomography(h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0)
        // H = Td^-1 * Hn * Ts
        let tdInverse = PetalHomography(
            1.0 / scaleD, 0.0, -td.h2 / scaleD,
            0.0, 1.0 / scaleD, -td.h5 / scaleD,
            0.0, 0.0, 1.0
        )
        let tsMatrix = PetalHomography(scaleS, 0.0, ts.h2, 0.0, scaleS, ts.h5, 0.0, 0.0, 1.0)
        let composed = tdInverse.compose(normalised).compose(tsMatrix)
        let norm = composed.h8
        guard !(abs(norm) < 1e-15) else { return nil }
        let e = composed.elements.map { $0 / norm }
        return PetalHomography(e[0], e[1], e[2], e[3], e[4], e[5], e[6], e[7], e[8])
    }

    /// Translation to the centroid and isotropic scale to mean distance √2.
    private static func normalisation(_ points: [PetalPoint]) -> (PetalHomography, Double) {
        let n = Double(points.count)
        var cx = 0.0
        var cy = 0.0
        for point in points {
            cx = cx + point.x / n
            cy = cy + point.y / n
        }
        var total = -0.0
        for point in points {
            let dx = point.x - cx
            let dy = point.y - cy
            total += (dx * dx + dy * dy).squareRoot()
        }
        let mean = total / n
        let scale = mean > 1e-12 ? Double(2).squareRoot() / mean : 1.0
        return (
            PetalHomography(scale, 0.0, -scale * cx, 0.0, scale, -scale * cy, 0.0, 0.0, 1.0),
            scale
        )
    }

    /// Gaussian elimination with partial pivoting for an 8×8 system
    /// (row-major `a`).
    private static func solve8(_ matrix: [Double], _ rhs: [Double]) -> [Double]? {
        var a = matrix
        var b = rhs
        for col in 0..<8 {
            // Last maximum under total order, like Rust's `Iterator::max_by`.
            var pivot = col
            var pivotKey = PetalNumeric.totalOrderKey(abs(a[col * 8 + col]))
            for row in (col + 1)..<8 {
                let key = PetalNumeric.totalOrderKey(abs(a[row * 8 + col]))
                if key >= pivotKey {
                    pivot = row
                    pivotKey = key
                }
            }
            if abs(a[pivot * 8 + col]) < 1e-14 { return nil }
            if pivot != col {
                for j in 0..<8 { a.swapAt(col * 8 + j, pivot * 8 + j) }
                b.swapAt(col, pivot)
            }
            for row in (col + 1)..<8 {
                let factor = a[row * 8 + col] / a[col * 8 + col]
                for j in col..<8 {
                    a[row * 8 + j] -= factor * a[col * 8 + j]
                }
                b[row] -= factor * b[col]
            }
        }
        var x = [Double](repeating: 0, count: 8)
        for row in stride(from: 7, through: 0, by: -1) {
            var tail = -0.0
            for k in (row + 1)..<8 { tail += a[row * 8 + k] * x[k] }
            x[row] = (b[row] - tail) / a[row * 8 + row]
        }
        return x
    }
}
