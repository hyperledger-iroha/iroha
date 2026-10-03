import Foundation

/// A detected corner finder.
public struct PetalFinder: Equatable, Sendable {
    /// Centre `x` in pixel-edge coordinates.
    public var x: Double
    /// Centre `y` in pixel-edge coordinates.
    public var y: Double
    /// Apparent outer diameter in pixels.
    public var size: Double

    public init(x: Double, y: Double, size: Double) {
        self.x = x
        self.y = y
        self.size = size
    }
}

/// A 4-connected component of a binarised image.
public struct PetalComponent: Equatable, Sendable {
    /// Pixel count.
    public internal(set) var area: UInt32 = 0
    /// Left-most pixel column.
    public internal(set) var minX: UInt32 = 0
    /// Right-most pixel column.
    public internal(set) var maxX: UInt32 = 0
    /// Top-most pixel row.
    public internal(set) var minY: UInt32 = 0
    /// Bottom-most pixel row.
    public internal(set) var maxY: UInt32 = 0
    var sumX = 0.0
    var sumY = 0.0
    var sumXX = 0.0
    var sumYY = 0.0
    var sumXY = 0.0

    var width: Double { Double(maxX - minX + 1) }
    var height: Double { Double(maxY - minY + 1) }

    /// Centroid in pixel-edge coordinates.
    public var centroid: PetalPoint {
        PetalPoint(x: sumX / Double(area), y: sumY / Double(area))
    }

    /// Ratio of the smaller to the larger principal axis of the blob.
    var axisRatio: Double {
        let n = Double(area)
        let c = centroid
        let vxx = sumXX / n - c.x * c.x
        let vyy = sumYY / n - c.y * c.y
        let vxy = sumXY / n - c.x * c.y
        let mean = 0.5 * (vxx + vyy)
        let difference = vxx - vyy
        let spread = (0.25 * (difference * difference) + vxy * vxy).squareRoot()
        let major = mean + spread
        let minor = Double.maximum(mean - spread, 0.0)
        if major <= 0.0 { return 0.0 }
        return (minor / major).squareRoot()
    }
}

/// Finding the four corner finders in a camera luma plane (port of
/// `crates/iroha_petal/src/locate.rs`).
///
/// Pipeline: adaptive threshold (local mean via an integral image) →
/// 4-connected component labelling → blossom detection (a large, round,
/// isolated blob) → selection of the four finders that form a plausible,
/// similarly sized quadrilateral. Solid blossoms survive defocus that would
/// fill in the gaps of a bullseye.
public enum PetalLocator {
    /// Sensitivities tried in order by ``locate(_:)``.
    static let sensitivities: [Double] = [0.12, 0.22, 0.34]

    /// Marks pixels that are clearly brighter than their neighbourhood.
    ///
    /// `sensitivity` scales the margin above the local mean in units of the
    /// image's dynamic range (≈ 0.12 for faint codes, larger values separate
    /// blurred finder rings from their cores).
    public static func adaptiveBinarize(_ image: PetalLuma, sensitivity: Double) -> [Bool] {
        image.withView { view in
            PetalBinarizer(view).mask(view, sensitivity: sensitivity)
        }
    }

    /// Labels 4-connected components of `mask` (`width * height` entries, row
    /// major); returns the components in order of their first pixel. A mask of
    /// the wrong size has no components.
    public static func labelComponents(_ mask: [Bool], width: Int, height: Int) -> [PetalComponent] {
        let (count, overflow) = width.multipliedReportingOverflow(by: height)
        guard width >= 0, height >= 0, !overflow, count == mask.count else { return [] }
        var labels = [UInt32](repeating: 0, count: mask.count)
        return labelComponents(mask, width: width, height: height, labels: &labels)
    }

    static func labelComponents(
        _ mask: [Bool],
        width w: Int,
        height h: Int,
        labels: inout [UInt32]
    ) -> [PetalComponent] {
        var parent: [UInt32] = [0]
        mask.withUnsafeBufferPointer { mask in
            labels.withUnsafeMutableBufferPointer { labels in
                for y in 0..<h {
                    let row = y * w
                    for x in 0..<w {
                        let i = row + x
                        if !mask[i] {
                            labels[i] = 0
                            continue
                        }
                        let left = x > 0 ? labels[i - 1] : 0
                        let up = y > 0 ? labels[i - w] : 0
                        if left == 0 && up == 0 {
                            let label = UInt32(parent.count)
                            parent.append(label)
                            labels[i] = label
                        } else if up == 0 {
                            labels[i] = left
                        } else if left == 0 {
                            labels[i] = up
                        } else {
                            let a = findRoot(&parent, left)
                            let b = findRoot(&parent, up)
                            let keep = a < b ? a : b
                            let drop = a < b ? b : a
                            parent[Int(drop)] = keep
                            labels[i] = keep
                        }
                    }
                }
            }
        }
        var components = [PetalComponent](repeating: PetalComponent(), count: parent.count)
        labels.withUnsafeBufferPointer { labels in
            components.withUnsafeMutableBufferPointer { components in
                for y in 0..<h {
                    let row = y * w
                    let py = Double(y) + 0.5
                    let cy = UInt32(y)
                    for x in 0..<w {
                        let label = labels[row + x]
                        if label == 0 { continue }
                        let root = Int(findRoot(&parent, label))
                        let cx = UInt32(x)
                        if components[root].area == 0 {
                            components[root].minX = cx
                            components[root].maxX = cx
                            components[root].minY = cy
                            components[root].maxY = cy
                        }
                        components[root].area += 1
                        if cx < components[root].minX { components[root].minX = cx }
                        if cx > components[root].maxX { components[root].maxX = cx }
                        if cy < components[root].minY { components[root].minY = cy }
                        if cy > components[root].maxY { components[root].maxY = cy }
                        let px = Double(x) + 0.5
                        components[root].sumX += px
                        components[root].sumY += py
                        components[root].sumXX += px * px
                        components[root].sumYY += py * py
                        components[root].sumXY += px * py
                    }
                }
            }
        }
        return components.filter { $0.area > 0 }
    }

    @inline(__always)
    private static func findRoot(_ parent: inout [UInt32], _ label: UInt32) -> UInt32 {
        var label = label
        while parent[Int(label)] != label {
            parent[Int(label)] = parent[Int(parent[Int(label)])]
            label = parent[Int(label)]
        }
        return label
    }

    /// Detects blossom finders: large, round, isolated blobs.
    public static func blossoms(_ components: [PetalComponent]) -> [PetalFinder] {
        var found: [PetalFinder] = []
        for (index, blob) in components.enumerated() {
            let size = Double.maximum(blob.width, blob.height)
            let fill = Double(blob.area) / (blob.width * blob.height)
            if size < 14.0 || blob.area < 100 || !(0.45...0.9).contains(fill) || blob.axisRatio < 0.5 {
                continue
            }
            let center = blob.centroid
            // isolation: nothing else of substance close by
            var crowded = false
            for (other, component) in components.enumerated() {
                if other == index || component.area < 8
                    || Double(component.area) < 0.015 * Double(blob.area) {
                    continue
                }
                let c = component.centroid
                let dx = c.x - center.x
                let dy = c.y - center.y
                if (dx * dx + dy * dy).squareRoot() < 0.8 * size {
                    crowded = true
                    break
                }
            }
            if !crowded {
                found.append(PetalFinder(x: center.x, y: center.y, size: size))
            }
        }
        return found
    }

    private static func cross(_ o: PetalFinder, _ a: PetalFinder, _ b: PetalFinder) -> Double {
        (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
    }

    /// Orders four finders clockwise (as displayed, `y` down) starting from
    /// the one nearest the top-left of the quadrilateral's bounding box.
    static func orderClockwise(_ input: [PetalFinder]) -> [PetalFinder]? {
        guard input.count == 4 else { return nil }
        var cx = -0.0
        var cy = -0.0
        for finder in input { cx += finder.x }
        for finder in input { cy += finder.y }
        cx /= 4.0
        cy /= 4.0
        // Stable insertion sort by angle under total order (Rust `sort_by`).
        var quad = input
        let keys = quad.map { PetalNumeric.totalOrderKey(atan2($0.y - cy, $0.x - cx)) }
        var order = [0, 1, 2, 3]
        for i in 1..<4 {
            var j = i
            while j > 0 && keys[order[j]] < keys[order[j - 1]] {
                order.swapAt(j, j - 1)
                j -= 1
            }
        }
        quad = order.map { input[$0] }
        // atan2 grows clockwise on screen because y points down; verify convexity
        for i in 0..<4 where cross(quad[i], quad[(i + 1) % 4], quad[(i + 2) % 4]) <= 0.0 {
            return nil
        }
        var start = 0
        var startKey = PetalNumeric.totalOrderKey(quad[0].x + quad[0].y)
        for i in 1..<4 {
            let key = PetalNumeric.totalOrderKey(quad[i].x + quad[i].y)
            if key < startKey {
                start = i
                startKey = key
            }
        }
        return (0..<4).map { quad[(start + $0) % 4] }
    }

    /// Chooses four finders that look like the corners of one code.
    ///
    /// Tile glyphs such as `ロ` form tile-sized blobs too, so the largest size
    /// class is tried first: the corner finders are always the biggest
    /// isolated blossoms in view. Within a class the ten largest candidates
    /// are combined. Returns the four corners clockwise from the top-left.
    public static func selectQuad(_ finders: [PetalFinder]) -> [PetalFinder]? {
        var largest = 0.0
        for finder in finders { largest = Double.maximum(largest, finder.size) }
        let strong = finders.filter { $0.size >= 0.55 * largest }
        return selectQuad(from: strong) ?? selectQuad(from: finders)
    }

    private static func selectQuad(from finders: [PetalFinder]) -> [PetalFinder]? {
        guard finders.count >= 4 else { return nil }
        // Largest first (ties keep discovery order) so that clutter in a busy
        // scene cannot push the real finders out of the ten candidates that are
        // combined. The tie-break is explicit: the result never depends on sort
        // stability.
        let keys = finders.map { PetalNumeric.totalOrderKey($0.size) }
        let order = finders.indices.sorted { a, b in
            keys[a] != keys[b] ? keys[a] > keys[b] : a < b
        }
        let ranked = order.prefix(10).map { finders[$0] }
        var best: (score: Double, quad: [PetalFinder])?
        let n = ranked.count
        for a in 0..<n {
            for b in (a + 1)..<max(a + 1, n) {
                for c in (b + 1)..<max(b + 1, n) {
                    for d in (c + 1)..<max(c + 1, n) {
                        let set = [ranked[a], ranked[b], ranked[c], ranked[d]]
                        var smin = Double.greatestFiniteMagnitude
                        var smax = 0.0
                        for finder in set {
                            smin = Double.minimum(smin, finder.size)
                            smax = Double.maximum(smax, finder.size)
                        }
                        if smax / smin > 1.9 { continue }
                        guard let quad = orderClockwise(set) else { continue }
                        var sides = [Double](repeating: 0, count: 4)
                        for i in 0..<4 {
                            let p = quad[i]
                            let q = quad[(i + 1) % 4]
                            sides[i] = ((p.x - q.x) * (p.x - q.x) + (p.y - q.y) * (p.y - q.y)).squareRoot()
                        }
                        var lmin = Double.greatestFiniteMagnitude
                        var lmax = 0.0
                        for side in sides {
                            lmin = Double.minimum(lmin, side)
                            lmax = Double.maximum(lmax, side)
                        }
                        var sizeSum = -0.0
                        for finder in set { sizeSum += finder.size }
                        let meanSize = sizeSum / 4.0
                        var sideSum = -0.0
                        for side in sides { sideSum += side }
                        // canvas geometry: side / finder diameter = 880 / 120
                        let ratio = (sideSum / 4.0) / meanSize
                        if lmax / lmin > 2.6 || !(4.8...10.5).contains(ratio) { continue }
                        let score = (smax / smin - 1.0) + (lmax / lmin - 1.0) + abs((ratio - 7.33) / 7.33)
                        if best == nil || score < (best?.score ?? 0) {
                            best = (score, quad)
                        }
                    }
                }
            }
        }
        return best?.quad
    }

    /// Sharpens a finder centre with an intensity-weighted centroid.
    public static func refineCenter(_ image: PetalLuma, finder: PetalFinder) -> PetalFinder {
        image.withView { refineCenter($0, finder: finder) }
    }

    static func refineCenter(_ image: PetalLumaView, finder: PetalFinder) -> PetalFinder {
        guard finder.x.isFinite, finder.y.isFinite, finder.size.isFinite,
              image.width > 0, image.height > 0 else {
            return finder
        }
        let radius = PetalNumeric.saturatingInt((finder.size * 0.5).rounded(.up))
        let cx = PetalNumeric.saturatingInt(finder.x.rounded(.down))
        let cy = PetalNumeric.saturatingInt(finder.y.rounded(.down))
        guard radius >= 0 else { return finder }
        // Only in-image offsets contribute, so the scan is clipped to the image
        // (same samples, same order as the reference loop).
        let yStart = max(cy - radius, 0)
        let yEnd = min(cy + radius, image.height - 1)
        let xStart = max(cx - radius, 0)
        let xEnd = min(cx + radius, image.width - 1)
        guard yStart <= yEnd, xStart <= xEnd else { return finder }
        var samples: [(Double, Double, Double)] = []
        let limit = finder.size * 0.5
        for y in yStart...yEnd {
            for x in xStart...xEnd {
                let px = Double(x) + 0.5
                let py = Double(y) + 0.5
                let dx = px - finder.x
                let dy = py - finder.y
                if (dx * dx + dy * dy).squareRoot() <= limit {
                    samples.append((px, py, Double(image.at(x, y))))
                }
            }
        }
        var floor = Double.greatestFiniteMagnitude
        var peak = 0.0
        for sample in samples {
            floor = Double.minimum(floor, sample.2)
            peak = Double.maximum(peak, sample.2)
        }
        if peak - floor < 20.0 { return finder }
        let threshold = floor + 0.5 * (peak - floor)
        var sw = 0.0
        var sx = 0.0
        var sy = 0.0
        for (px, py, value) in samples {
            let weight = Double.maximum(value - threshold, 0.0)
            sw += weight
            sx += weight * px
            sy += weight * py
        }
        if sw <= 0.0 { return finder }
        return PetalFinder(x: sx / sw, y: sy / sw, size: finder.size)
    }

    /// Locates the four finders of a code, trying progressively stricter
    /// thresholds so blurred rings still separate from their cores. Returns
    /// the refined corners clockwise from the top-left of the image.
    public static func locate(_ image: PetalLuma) -> [PetalFinder]? {
        image.withView { locate($0) }
    }

    static func locate(_ image: PetalLumaView) -> [PetalFinder]? {
        let binarizer = PetalBinarizer(image)
        var labels = [UInt32](repeating: 0, count: image.width * image.height)
        for sensitivity in sensitivities {
            let mask = binarizer.mask(image, sensitivity: sensitivity)
            let components = labelComponents(mask, width: image.width, height: image.height, labels: &labels)
            let finders = blossoms(components)
            if let quad = selectQuad(finders) {
                return quad.map { refineCenter(image, finder: $0) }
            }
        }
        return nil
    }
}

/// The sensitivity-independent part of `adaptive_binarize`: integral image,
/// dynamic range and window radius, computed once per image.
struct PetalBinarizer {
    private let integral: [Int]
    private let low: Double
    private let range: Double
    private let radius: Int

    init(_ image: PetalLumaView) {
        let w = image.width
        let h = image.height
        var integral = [Int](repeating: 0, count: (w + 1) * (h + 1))
        var histogram = [Int](repeating: 0, count: 256)
        integral.withUnsafeMutableBufferPointer { integral in
            for y in 0..<h {
                var row = 0
                let source = y * w
                let above = y * (w + 1)
                let target = (y + 1) * (w + 1)
                for x in 0..<w {
                    let value = Int(image.buffer[source + x])
                    histogram[value] += 1
                    row += value
                    integral[target + x + 1] = integral[above + x + 1] + row
                }
            }
        }
        let total = Double(w * h)
        func percentile(_ p: Double) -> Double {
            let target = total * p
            var seen = 0.0
            for level in 0..<256 {
                seen += Double(histogram[level])
                if seen >= target { return Double(level) }
            }
            return 255.0
        }
        let low = percentile(0.02)
        let high = percentile(0.995)
        self.integral = integral
        self.low = low
        range = Double.maximum(high - low, 8.0)
        radius = min(max(min(w, h) / 8, 12), 64)
    }

    /// The binary mask for one sensitivity.
    func mask(_ image: PetalLumaView, sensitivity: Double) -> [Bool] {
        let w = image.width
        let h = image.height
        let margin = Double.maximum(sensitivity * range, 5.0)
        let floor = low + 0.2 * range
        var mask = [Bool](repeating: false, count: w * h)
        integral.withUnsafeBufferPointer { integral in
            mask.withUnsafeMutableBufferPointer { mask in
                let stride = w + 1
                for y in 0..<h {
                    let y0 = max(y - radius, 0)
                    let y1 = min(y + radius + 1, h)
                    let height = y1 - y0
                    let rowTop = y0 * stride
                    let rowBottom = y1 * stride
                    let source = y * w
                    for x in 0..<w {
                        let x0 = max(x - radius, 0)
                        let x1 = min(x + radius + 1, w)
                        let sum = integral[rowBottom + x1] + integral[rowTop + x0]
                            - integral[rowTop + x1] - integral[rowBottom + x0]
                        let mean = Double(sum) / Double((x1 - x0) * height)
                        let value = Double(image.buffer[source + x])
                        mask[source + x] = value > mean + margin && value > floor
                    }
                }
            }
        }
        return mask
    }
}
