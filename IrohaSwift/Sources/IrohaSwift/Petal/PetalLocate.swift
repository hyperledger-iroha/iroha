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

/// One plausible set of corner finders for a frame.
public struct PetalFinderSet: Equatable, Sendable {
    /// The four corners, clockwise from the one nearest the top-left of the
    /// image.
    public let corners: [PetalFinder]
    /// Index into ``corners`` of a corner that was not seen but inferred from
    /// the other three, if any.
    public let inferred: Int?

    public init(corners: [PetalFinder], inferred: Int?) {
        self.corners = corners
        self.inferred = inferred
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
///
/// When a finger, a glare or the edge of the frame hides one blossom, three
/// large blossoms that form a corner still identify the code: the fourth
/// corner is inferred (and later refined by the decoder).
public enum PetalLocator {
    /// Binarisation thresholds, from the most to the least permissive: a
    /// higher sensitivity separates blurred blossoms from their surroundings.
    public static let sensitivities: [Double] = [0.12, 0.22, 0.34]

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

    /// The finders of the largest size class: lit tiles and merged dots form
    /// blob candidates too, but the corner finders are the biggest isolated
    /// round blobs in view.
    static func strongFinders(_ finders: [PetalFinder]) -> [PetalFinder] {
        var largest = 0.0
        for finder in finders { largest = Double.maximum(largest, finder.size) }
        return finders.filter { $0.size >= 0.55 * largest }
    }

    /// The ten largest candidates, largest first (ties keep discovery order),
    /// so that clutter in a busy scene cannot push the real finders out of the
    /// set that is combined. The tie-break is explicit: the result never
    /// depends on sort stability.
    static func ranked(_ finders: [PetalFinder]) -> [PetalFinder] {
        let keys = finders.map { PetalNumeric.totalOrderKey($0.size) }
        let order = finders.indices.sorted { a, b in
            keys[a] != keys[b] ? keys[a] > keys[b] : a < b
        }
        return order.prefix(10).map { finders[$0] }
    }

    /// Chooses four finders that look like the corners of one code.
    ///
    /// Tile glyphs such as `ロ` form tile-sized blobs too, so the largest size
    /// class is tried first: the corner finders are always the biggest
    /// isolated blossoms in view. Within a class the ten largest candidates
    /// are combined. Returns the four corners clockwise from the top-left.
    public static func selectQuad(_ finders: [PetalFinder]) -> [PetalFinder]? {
        selectQuad(from: strongFinders(finders)) ?? selectQuad(from: finders)
    }

    /// Chooses three finders that look like three corners of one code (an
    /// `L`: similar sizes, two similar legs at a roughly right angle) and
    /// completes the fourth corner as a parallelogram.
    ///
    /// Returns the clockwise quad and the index of the inferred corner in it.
    public static func selectTriple(_ finders: [PetalFinder]) -> (quad: [PetalFinder], inferred: Int)? {
        let ranked = ranked(finders)
        let n = ranked.count
        var best: (score: Double, quad: [PetalFinder], inferred: Int)?
        for a in 0..<n {
            for b in (a + 1)..<max(a + 1, n) {
                for c in (b + 1)..<max(b + 1, n) {
                    let set = [ranked[a], ranked[b], ranked[c]]
                    var smin = Double.greatestFiniteMagnitude
                    var smax = 0.0
                    for finder in set {
                        smin = Double.minimum(smin, finder.size)
                        smax = Double.maximum(smax, finder.size)
                    }
                    if smax / smin > 1.9 { continue }
                    var sizeSum = -0.0
                    for finder in set { sizeSum += finder.size }
                    let meanSize = sizeSum / 3.0
                    for corner in 0..<3 {
                        let k = set[corner]
                        let p = set[(corner + 1) % 3]
                        let q = set[(corner + 2) % 3]
                        let ux = p.x - k.x
                        let uy = p.y - k.y
                        let vx = q.x - k.x
                        let vy = q.y - k.y
                        let lu = (ux * ux + uy * uy).squareRoot()
                        let lv = (vx * vx + vy * vy).squareRoot()
                        if lu <= 0.0 || lv <= 0.0 { continue }
                        let legs = Double.maximum(lu, lv) / Double.minimum(lu, lv)
                        let cosine = (ux * vx + uy * vy) / (lu * lv)
                        // canvas geometry: side / finder diameter = 880 / 120
                        let ratio = 0.5 * (lu + lv) / meanSize
                        if legs > 2.0 || abs(cosine) > 0.5 || !(4.8...10.5).contains(ratio) { continue }
                        let fourth = PetalFinder(x: p.x + q.x - k.x, y: p.y + q.y - k.y, size: meanSize)
                        guard let quad = orderClockwise([k, p, q, fourth]) else { continue }
                        guard let inferred = quad.firstIndex(where: {
                            $0.x.bitPattern == fourth.x.bitPattern && $0.y.bitPattern == fourth.y.bitPattern
                        }) else { continue }
                        let score = (smax / smin - 1.0) + (legs - 1.0) + abs(cosine) + abs((ratio - 7.33) / 7.33)
                        if best == nil || score < (best?.score ?? 0) {
                            best = (score, quad, inferred)
                        }
                    }
                }
            }
        }
        return best.map { ($0.quad, $0.inferred) }
    }

    /// A blob of at least 0.3 × the finder size within 0.3 legs of the
    /// inferred corner of a triple completes it into a seen quad.
    static func completeTriple(_ finders: [PetalFinder], quad: [PetalFinder], missing: Int) -> [PetalFinder]? {
        let d = quad[missing]
        func distance(_ f: PetalFinder) -> Double {
            ((f.x - d.x) * (f.x - d.x) + (f.y - d.y) * (f.y - d.y)).squareRoot()
        }
        let leg = 0.5 * (distance(quad[(missing + 1) % 4]) + distance(quad[(missing + 3) % 4]))
        // the first of the nearest under the total order (Rust `Iterator::min_by`)
        var fourth: PetalFinder?
        var fourthKey = Int64.max
        for finder in finders where finder.size >= 0.3 * d.size && distance(finder) <= 0.3 * leg {
            let key = PetalNumeric.totalOrderKey(distance(finder))
            if fourth == nil || key < fourthKey {
                fourth = finder
                fourthKey = key
            }
        }
        guard let fourth else { return nil }
        var full = quad
        full[missing] = fourth
        return orderClockwise(full)
    }

    static func selectQuad(from finders: [PetalFinder]) -> [PetalFinder]? {
        guard finders.count >= 4 else { return nil }
        let ranked = ranked(finders)
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
        centroid(image, finder: finder) ?? finder
    }

    /// The intensity-weighted centroid of the bright part of the disc of
    /// diameter `finder.size` around the finder, or `nil` when that disc has
    /// less than 20 levels of contrast (nothing bright is there).
    static func centroid(_ image: PetalLumaView, finder: PetalFinder) -> PetalFinder? {
        guard finder.x.isFinite, finder.y.isFinite, finder.size.isFinite,
              image.width > 0, image.height > 0 else {
            return nil
        }
        let radius = PetalNumeric.saturatingInt((finder.size * 0.5).rounded(.up))
        let cx = PetalNumeric.saturatingInt(finder.x.rounded(.down))
        let cy = PetalNumeric.saturatingInt(finder.y.rounded(.down))
        guard radius >= 0 else { return nil }
        // Only in-image offsets contribute, so the scan is clipped to the image
        // (same samples, same order as the reference loop). Far-away finders
        // clip to nothing instead of overflowing.
        let (top, topOverflow) = cy.subtractingReportingOverflow(radius)
        let (bottom, bottomOverflow) = cy.addingReportingOverflow(radius)
        let (left, leftOverflow) = cx.subtractingReportingOverflow(radius)
        let (right, rightOverflow) = cx.addingReportingOverflow(radius)
        let yStart = topOverflow ? 0 : max(top, 0)
        let yEnd = bottomOverflow ? image.height - 1 : min(bottom, image.height - 1)
        let xStart = leftOverflow ? 0 : max(left, 0)
        let xEnd = rightOverflow ? image.width - 1 : min(right, image.width - 1)
        guard yStart <= yEnd, xStart <= xEnd else { return nil }
        let limit = finder.size * 0.5
        // Two passes over the same samples in the same order (the reference
        // collects them first): levels, then the weighted sums. No allocation.
        var floor = Double.greatestFiniteMagnitude
        var peak = 0.0
        for y in yStart...yEnd {
            let py = Double(y) + 0.5
            let dy = py - finder.y
            for x in xStart...xEnd {
                let dx = Double(x) + 0.5 - finder.x
                if (dx * dx + dy * dy).squareRoot() <= limit {
                    let value = Double(image.at(x, y))
                    floor = Double.minimum(floor, value)
                    peak = Double.maximum(peak, value)
                }
            }
        }
        if peak - floor < 20.0 { return nil }
        let threshold = floor + 0.5 * (peak - floor)
        var sw = 0.0
        var sx = 0.0
        var sy = 0.0
        for y in yStart...yEnd {
            let py = Double(y) + 0.5
            let dy = py - finder.y
            for x in xStart...xEnd {
                let px = Double(x) + 0.5
                let dx = px - finder.x
                if (dx * dx + dy * dy).squareRoot() <= limit {
                    let weight = Double.maximum(Double(image.at(x, y)) - threshold, 0.0)
                    sw += weight
                    sx += weight * px
                    sy += weight * py
                }
            }
        }
        if sw <= 0.0 { return nil }
        return PetalFinder(x: sx / sw, y: sy / sw, size: finder.size)
    }

    /// Re-finds a finder near where it is expected (from the previous frame's
    /// pose).
    ///
    /// A first centroid over a disc twice the finder's diameter catches a
    /// blossom that moved up to about one diameter (nothing else bright is
    /// that close to a corner finder); centroids over the finder's own disc
    /// then repeat, at most five times, until the centre moves less than a
    /// quarter pixel. `nil` when nothing bright is there or the result is
    /// more than 0.75 diameters from the expected centre, which means the code
    /// moved too far for tracking.
    public static func follow(_ image: PetalLuma, expected: PetalFinder) -> PetalFinder? {
        image.withView { follow($0, expected: expected) }
    }

    static func follow(_ image: PetalLumaView, expected: PetalFinder) -> PetalFinder? {
        let wideDisc = PetalFinder(x: expected.x, y: expected.y, size: 2.0 * expected.size)
        guard let wide = centroid(image, finder: wideDisc) else { return nil }
        var current = PetalFinder(x: wide.x, y: wide.y, size: expected.size)
        for _ in 0..<5 {
            guard let next = centroid(image, finder: current) else { return nil }
            let dx = next.x - current.x
            let dy = next.y - current.y
            let step = (dx * dx + dy * dy).squareRoot()
            current = next
            if step < 0.25 { break }
        }
        let dx = current.x - expected.x
        let dy = current.y - expected.y
        let moved = (dx * dx + dy * dy).squareRoot()
        return moved <= 0.75 * expected.size ? current : nil
    }

    /// Locates four seen finders of a code: the first candidate of
    /// ``candidates(_:)`` without an inferred corner. Returns the refined
    /// corners clockwise from the top-left of the image.
    public static func locate(_ image: PetalLuma) -> [PetalFinder]? {
        image.withView { locate($0) }
    }

    static func locate(_ image: PetalLumaView) -> [PetalFinder]? {
        var candidates = PetalCandidateSearch()
        while let set = candidates.next(image) {
            if set.inferred == nil { return set.corners }
        }
        return nil
    }

    /// All candidate finder sets for one frame, in the order of
    /// ``candidates(_:)``.
    public static func locateCandidates(_ image: PetalLuma) -> [PetalFinderSet] {
        Array(candidates(image))
    }

    /// Candidate finder sets for one frame, produced lazily in the order a
    /// decoder should try them, so that a clean frame costs one binarisation.
    ///
    /// For each threshold of ``sensitivities`` in turn: four finders of the
    /// largest size class that form a quad. Then, from the first threshold
    /// that had them, three large finders forming a corner — completed by the
    /// nearest smaller blob within 0.3 legs of where the fourth corner belongs
    /// (steep tilt makes the far finder small) — then the first quad that
    /// smaller blobs form, and last the same three finders with the fourth
    /// corner inferred (the nearby blob may have been merged ring dots, a quad
    /// may have been clutter).
    public static func candidates(_ image: PetalLuma) -> PetalFinderCandidates {
        PetalFinderCandidates(image: image)
    }
}

/// The lazy sequence of ``PetalLocator/candidates(_:)``.
public struct PetalFinderCandidates: Sequence, IteratorProtocol {
    private let image: PetalLuma
    private var search = PetalCandidateSearch()

    init(image: PetalLuma) {
        self.image = image
    }

    public mutating func next() -> PetalFinderSet? {
        let image = image
        return image.withView { search.next($0) }
    }
}

/// The state of the candidate search over one image (Rust `Candidates`).
///
/// The caller passes the same image to every ``next(_:)``. The integral image
/// and the label buffer are built on the first call and shared by the three
/// thresholds.
struct PetalCandidateSearch {
    private var stage = 0
    private var binarizer: PetalBinarizer?
    private var labels: [UInt32] = []
    private var completed: [PetalFinder]?
    private var smaller: [PetalFinder]?
    private var inferred: (corners: [PetalFinder], missing: Int)?
    /// The sets left after the last threshold, in reverse order of delivery.
    private var tail: [PetalFinderSet] = []

    mutating func next(_ image: PetalLumaView) -> PetalFinderSet? {
        let sensitivities = PetalLocator.sensitivities
        while stage < sensitivities.count {
            let sensitivity = sensitivities[stage]
            stage += 1
            if binarizer == nil {
                binarizer = PetalBinarizer(image)
                labels = [UInt32](repeating: 0, count: image.width * image.height)
            }
            guard let binarizer else { return nil }
            let mask = binarizer.mask(image, sensitivity: sensitivity)
            let components = PetalLocator.labelComponents(
                mask,
                width: image.width,
                height: image.height,
                labels: &labels
            )
            let finders = PetalLocator.blossoms(components)
            let strong = PetalLocator.strongFinders(finders)
            if inferred == nil, let triple = PetalLocator.selectTriple(strong) {
                completed = PetalLocator.completeTriple(finders, quad: triple.quad, missing: triple.inferred)
                    .map { full in full.map { PetalLocator.refineCenter(image, finder: $0) } }
                var corners = triple.quad
                for index in 0..<4 where index != triple.inferred {
                    corners[index] = PetalLocator.refineCenter(image, finder: corners[index])
                }
                inferred = (corners, triple.inferred)
            }
            if smaller == nil {
                smaller = PetalLocator.selectQuad(from: finders).map { quad in
                    quad.map { PetalLocator.refineCenter(image, finder: $0) }
                }
            }
            if let quad = PetalLocator.selectQuad(from: strong) {
                return PetalFinderSet(
                    corners: quad.map { PetalLocator.refineCenter(image, finder: $0) },
                    inferred: nil
                )
            }
        }
        if stage == sensitivities.count {
            stage += 1
            var sets: [PetalFinderSet] = []
            if let completed { sets.append(PetalFinderSet(corners: completed, inferred: nil)) }
            if let smaller { sets.append(PetalFinderSet(corners: smaller, inferred: nil)) }
            if let inferred { sets.append(PetalFinderSet(corners: inferred.corners, inferred: inferred.missing)) }
            completed = nil
            smaller = nil
            inferred = nil
            tail = sets.reversed()
        }
        return tail.popLast()
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
