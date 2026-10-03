import Foundation

/// Decoder tuning.
public struct PetalDecodeOptions: Equatable, Sendable {
    /// Also try horizontally mirrored images (front-camera previews).
    public var tryMirrored: Bool
    /// Blur widths (in template cells) tried for glyph matching.
    public var templateSigmas: [Double]
    /// Largest image (in pixels) the decoder accepts; larger frames should be
    /// downscaled by the caller. Bounds memory and work on hostile input.
    public var maximumPixels: Int

    public init(
        tryMirrored: Bool = true,
        templateSigmas: [Double] = [0.0, 0.5, 0.8, 1.1, 1.5],
        maximumPixels: Int = 12_000_000
    ) {
        self.tryMirrored = tryMirrored
        self.templateSigmas = templateSigmas
        self.maximumPixels = maximumPixels
    }
}

/// A lane that passed its Reed–Solomon check.
public struct PetalLaneResult: Equatable, Sendable {
    /// Lane data bytes (header and atoms, or the beacon).
    public let data: [UInt8]
    /// Byte positions the Reed–Solomon decoder rewrote (erased bytes plus
    /// unflagged errors).
    public let corrected: Int
    /// Bytes that were passed to the Reed–Solomon decoder as erasures.
    public let erasures: Int
}

/// Everything read from one camera frame.
public struct PetalDecodedFrame: Equatable, Sendable {
    /// Canvas-to-pixel homography that was used.
    public let homography: PetalHomography
    /// Orientation: how many quarter turns the code is rotated, i.e. which
    /// image corner (clockwise from the top-left) holds the code's top-left
    /// finder.
    public let rotation: Int
    /// Whether the image was mirrored.
    public let mirrored: Bool
    /// Lane `P` result.
    public let p: PetalLaneResult?
    /// Lane `K` result.
    public let k: PetalLaneResult?
    /// Lane `D` result.
    public let d: PetalLaneResult?

    /// Number of lanes that decoded.
    public var lanesOK: Int {
        (p == nil ? 0 : 1) + (k == nil ? 0 : 1) + (d == nil ? 0 : 1)
    }

    /// Letters (`"PKD"` order) of the lanes that decoded.
    public var laneLetters: String {
        var letters = ""
        if p != nil { letters.append(PetalLane.p.letter) }
        if k != nil { letters.append(PetalLane.k.letter) }
        if d != nil { letters.append(PetalLane.d.letter) }
        return letters
    }

    /// What lane `D` carried, when it decoded.
    public var dLane: PetalDLane? {
        d.flatMap { PetalStream.parseDLane($0.data) }
    }

    /// The beacon, when lane `D` decoded on a beacon frame.
    public var beacon: PetalBeacon? {
        if case .beacon(let beacon) = dLane { return beacon }
        return nil
    }

    /// Atom packets from every lane that decoded.
    public var atomPackets: [PetalAtomPacket] {
        var packets: [PetalAtomPacket] = []
        if let p, let packet = PetalStream.parseAtomLane(.p, data: p.data) { packets.append(packet) }
        if let k, let packet = PetalStream.parseAtomLane(.k, data: k.data) { packets.append(packet) }
        if case .atoms(let packet) = dLane { packets.append(packet) }
        return packets
    }

    /// Offers everything this frame carries to `assembler`.
    public func feed(_ assembler: inout PetalStreamAssembler) {
        if let lane = dLane { assembler.push(dLane: lane) }
        if let p, let packet = PetalStream.parseAtomLane(.p, data: p.data) {
            assembler.push(atoms: packet)
        }
        if let k, let packet = PetalStream.parseAtomLane(.k, data: k.data) {
            assembler.push(atoms: packet)
        }
    }
}

/// Why a frame could not be decoded at all.
public enum PetalDecodeError: Error, Equatable, LocalizedError, Sendable {
    /// The image is smaller than 48 pixels on a side or larger than
    /// ``PetalDecodeOptions/maximumPixels``.
    case unsupportedImage
    /// The four corner finders were not found.
    case noFinders
    /// Finders were found but no orientation produced a readable lane.
    case noOrientation

    public var errorDescription: String? {
        switch self {
        case .unsupportedImage: return "Petal image has an unsupported size."
        case .noFinders: return "Petal finders not found."
        case .noOrientation: return "No Petal orientation produced a readable lane."
        }
    }
}

/// From a camera luma plane to lane data (port of
/// `crates/iroha_petal/src/decode.rs`).
///
/// The decoder locates the four finders, derives a homography for each
/// orientation hypothesis (four rotations, optionally mirrored), picks the
/// orientation whose ring gates line up (and whose lane `D` codeword checks
/// out), then reads the tiles and dots. Every tile is classified jointly: the
/// 8×8 sample patch is compared against the 32 hypotheses (polarity × glyph)
/// and the best match wins. Cells the decoder is unsure about become
/// Reed–Solomon erasures. A lane is only reported when its codeword checks
/// out; the payload CRC-32C of the stream catches the rare miscorrection.
///
/// The tile *level read* judges every patch against the light and dark levels
/// measured at the finders. When it leaves lane `P` or `K` unreadable, the
/// *normalised read* is tried: it rescales each patch (and each template) by
/// its own contrast, so over-exposure, veiling light, glare, shadows and
/// gradients cancel out.
public enum PetalDecoder {
    /// Relative level of the glyph ink on a light tile (ink / light fill).
    static let inkOnLight = 0.04
    /// Relative level of a pink glyph on a dark tile (pink / light fill).
    static let pinkOnDark = 0.83
    static let patch = PetalGlyphs.templateSize
    static let cells = patch * patch
    static let hypothesisCount = 2 * PetalGlyphs.count
    /// Cells cut from each end of a sorted patch to find its robust darkest
    /// and brightest level.
    static let patchCut = cells / 10
    /// Tiles whose contrast is below this fraction of the median tile
    /// contrast become erasures in the normalised read.
    static let weakTile = 0.25

    /// Decodes one camera frame.
    ///
    /// - Throws: ``PetalDecodeError/unsupportedImage`` for unusable sizes,
    ///   ``PetalDecodeError/noFinders`` when no code is visible and
    ///   ``PetalDecodeError/noOrientation`` when no orientation yields a
    ///   readable lane.
    public static func decode(
        _ image: PetalLuma,
        options: PetalDecodeOptions = PetalDecodeOptions()
    ) throws -> PetalDecodedFrame {
        try decodeResult(image, options: options).get()
    }

    /// ``decode(_:options:)`` with a typed failure.
    static func decodeResult(
        _ image: PetalLuma,
        options: PetalDecodeOptions
    ) -> Result<PetalDecodedFrame, PetalDecodeError> {
        guard isSupported(image, options: options) else { return .failure(.unsupportedImage) }
        return image.withView { view in
            guard let finders = PetalLocator.locate(view) else { return .failure(.noFinders) }
            var scored: [Candidate] = []
            for hypothesis in hypotheses(finders, tryMirrored: options.tryMirrored) {
                guard let reference = referenceLevels(view, hypothesis.homography) else { continue }
                let score = gateScore(view, hypothesis.homography, reference)
                scored.append(Candidate(
                    score: score,
                    rotation: hypothesis.rotation,
                    mirrored: hypothesis.mirrored,
                    homography: hypothesis.homography,
                    reference: reference
                ))
            }
            // Stable descending sort under total order (Rust `sort_by`).
            let order = scored.indices.sorted { left, right in
                let a = PetalNumeric.totalOrderKey(scored[left].score)
                let b = PetalNumeric.totalOrderKey(scored[right].score)
                return a != b ? a > b : left < right
            }
            let ranked = order.map { scored[$0] }
            // 1. the ring beacon is the cheapest and strongest orientation check
            for candidate in ranked.prefix(3) {
                if candidate.score < 0.2 { break }
                if let d = readLaneD(view, candidate.homography, candidate.reference) {
                    return .success(finish(view, options, candidate, d: d))
                }
            }
            // 2. fall back to the tile lanes under the most promising orientations
            for candidate in ranked.prefix(4) {
                let patches = samplePatches(view, candidate.homography)
                let lanes = readTileLanes(patches, candidate.reference, options.templateSigmas)
                if lanes.p != nil || lanes.k != nil {
                    return .success(PetalDecodedFrame(
                        homography: candidate.homography,
                        rotation: candidate.rotation,
                        mirrored: candidate.mirrored,
                        p: lanes.p,
                        k: lanes.k,
                        d: readLaneD(view, candidate.homography, candidate.reference)
                    ))
                }
            }
            return .failure(.noOrientation)
        }
    }

    /// Reads all lanes with a known canvas-to-pixel homography (no finder
    /// search).
    ///
    /// Returns `nil` when the image is unusable or the finder reference
    /// levels are too weak. Used by trackers that already know the pose and
    /// by qualification tooling with a ground-truth pose.
    public static func decode(
        _ image: PetalLuma,
        homography: PetalHomography,
        options: PetalDecodeOptions = PetalDecodeOptions()
    ) -> PetalDecodedFrame? {
        guard isSupported(image, options: options) else { return nil }
        return image.withView { view in
            guard let reference = referenceLevels(view, homography) else { return nil }
            let candidate = Candidate(
                score: 0,
                rotation: 0,
                mirrored: false,
                homography: homography,
                reference: reference
            )
            return finish(view, options, candidate, d: nil)
        }
    }

    /// Builds the cells a decoder believes it saw, for diagnostics.
    ///
    /// Tiles are taken from the level read (the one that judges against the
    /// finder levels), even for a frame whose lanes were rescued by the
    /// normalised read. Returns `nil` for an image ``decode(_:options:)``
    /// refuses.
    public static func observedCells(
        _ image: PetalLuma,
        frame: PetalDecodedFrame,
        options: PetalDecodeOptions = PetalDecodeOptions()
    ) -> PetalFrameCells? {
        guard isSupported(image, options: options) else { return nil }
        return image.withView { view in
            guard let reference = referenceLevels(view, frame.homography) else { return nil }
            let patches = samplePatches(view, frame.homography)
            let reads = readTiles(patches, reference, options.templateSigmas)
            let words = tileWords(reads)
            let (d, _) = readDots(view, frame.homography, reference)
            return PetalFrameCells(validatedP: words.p, k: words.k, d: d)
        }
    }

    /// Mean squared tile-match error of the level read, a quick image-quality
    /// indicator.
    ///
    /// It can be large for a frame whose lanes were rescued by the normalised
    /// read, which is the point: the finder levels did not describe that
    /// picture. Returns `nil` for an image ``decode(_:options:)`` refuses.
    public static func tileMatchError(
        _ image: PetalLuma,
        frame: PetalDecodedFrame,
        options: PetalDecodeOptions = PetalDecodeOptions()
    ) -> Double? {
        guard isSupported(image, options: options) else { return nil }
        return image.withView { view in
            guard let reference = referenceLevels(view, frame.homography) else { return nil }
            let patches = samplePatches(view, frame.homography)
            let reads = readTiles(patches, reference, options.templateSigmas)
            var total = -0.0
            for read in reads { total += read.error }
            return total / Double(reads.count)
        }
    }

    private static func isSupported(_ image: PetalLuma, options: PetalDecodeOptions) -> Bool {
        let (pixels, overflow) = image.width.multipliedReportingOverflow(by: image.height)
        return image.width >= 48 && image.height >= 48
            && !overflow && pixels <= options.maximumPixels
            && image.pixels.count == pixels
    }

    // MARK: - Orientation

    struct Candidate {
        let score: Double
        let rotation: Int
        let mirrored: Bool
        let homography: PetalHomography
        let reference: Reference
    }

    struct Hypothesis {
        let rotation: Int
        let mirrored: Bool
        let homography: PetalHomography
    }

    static func hypotheses(_ finders: [PetalFinder], tryMirrored: Bool) -> [Hypothesis] {
        let canonical = PetalLayout.finderCenters
        var out: [Hypothesis] = []
        for mirrored in [false, true] {
            if mirrored && !tryMirrored { continue }
            for rotation in 0..<4 {
                let destination = (0..<4).map { i -> PetalPoint in
                    let q = mirrored ? finders[(rotation + 4 - i) % 4] : finders[(i + rotation) % 4]
                    return PetalPoint(x: q.x, y: q.y)
                }
                if let homography = PetalHomography.fit(from: canonical, to: destination) {
                    out.append(Hypothesis(rotation: rotation, mirrored: mirrored, homography: homography))
                }
            }
        }
        return out
    }

    // MARK: - Reference levels and dots

    /// Lit and dark levels at the four finders, interpolated over the canvas.
    struct Reference {
        let lit: [Double]
        let dark: [Double]

        /// Bilinear interpolation over the canvas of the four corner estimates.
        @inline(__always)
        func at(_ x: Double, _ y: Double) -> (lit: Double, dark: Double) {
            let u = PetalNumeric.clamp(x / 1024.0, 0.0, 1.0)
            let v = PetalNumeric.clamp(y / 1024.0, 0.0, 1.0)
            func mix(_ c: [Double]) -> Double {
                let top = c[0] * (1.0 - u) + c[1] * u
                let bottom = c[3] * (1.0 - u) + c[2] * u
                return top * (1.0 - v) + bottom * v
            }
            return (mix(lit), mix(dark))
        }
    }

    @inline(__always)
    static func dotSamples(
        _ image: PetalLumaView,
        _ h: PetalHomography,
        _ x: Double,
        _ y: Double,
        _ spread: Double
    ) -> Double {
        var sum = 0.0
        var (px, py) = h.apply(x + 0.0, y + 0.0)
        sum += image.sample(px, py)
        (px, py) = h.apply(x + spread, y + 0.0)
        sum += image.sample(px, py)
        (px, py) = h.apply(x + -spread, y + 0.0)
        sum += image.sample(px, py)
        (px, py) = h.apply(x + 0.0, y + spread)
        sum += image.sample(px, py)
        (px, py) = h.apply(x + 0.0, y + -spread)
        sum += image.sample(px, py)
        return sum / 5.0
    }

    static func referenceLevels(_ image: PetalLumaView, _ h: PetalHomography) -> Reference? {
        var lit = [Double](repeating: 0, count: 4)
        var dark = [Double](repeating: 0, count: 4)
        let tau = 2 * Double.pi
        for (i, center) in PetalLayout.finderCenters.enumerated() {
            let cx = center.x
            let cy = center.y
            // the blossom is solid out to radius 24 around its centre
            var (px, py) = h.apply(cx, cy)
            var sum = image.sample(px, py)
            for k in 0..<8 {
                let angle = tau * Double(k) / 8.0
                (px, py) = h.apply(cx + 20.0 * cos(angle), cy + 20.0 * sin(angle))
                sum += image.sample(px, py)
            }
            lit[i] = sum / 9.0
            let sx = cx < 512.0 ? 1.0 : -1.0
            let sy = cy < 512.0 ? 1.0 : -1.0
            let a = dotSamples(image, h, cx + sx * 100.0, cy, 5.0)
            let b = dotSamples(image, h, cx, cy + sy * 100.0, 5.0)
            dark[i] = 0.5 * (a + b)
            if lit[i] - dark[i] < 12.0 { return nil }
        }
        return Reference(lit: lit, dark: dark)
    }

    @inline(__always)
    static func normalisedDot(
        _ image: PetalLumaView,
        _ h: PetalHomography,
        _ reference: Reference,
        _ flat: Int
    ) -> Double {
        let center = PetalLayout.flatSlotCenters[flat]
        let (lit, dark) = reference.at(center.x, center.y)
        return (dotSamples(image, h, center.x, center.y, 3.5) - dark) / (lit - dark)
    }

    static func gateScore(_ image: PetalLumaView, _ h: PetalHomography, _ reference: Reference) -> Double {
        func mean(_ slots: [Int]) -> Double {
            var sum = -0.0
            for slot in slots { sum += normalisedDot(image, h, reference, slot) }
            return sum / Double(slots.count)
        }
        return mean(PetalLayout.gateSlots) - mean(PetalLayout.guardSlots)
    }

    /// Reads lane `D`: the transmitted bytes and per-byte confidence.
    static func readDots(
        _ image: PetalLumaView,
        _ h: PetalHomography,
        _ reference: Reference
    ) -> ([UInt8], [Double]) {
        // per-ring thresholds from the gates (lit) and guards (dark)
        var thresholds = [Double](repeating: 0.5, count: PetalLayout.ringCount)
        for ring in 0..<PetalLayout.ringCount {
            var litSum = -0.0
            var litCount = 0
            for slot in PetalLayout.gateSlots where PetalLayout.splitSlot(slot).ring == ring {
                litSum += normalisedDot(image, h, reference, slot)
                litCount += 1
            }
            var darkSum = -0.0
            var darkCount = 0
            for slot in PetalLayout.guardSlots where PetalLayout.splitSlot(slot).ring == ring {
                darkSum += normalisedDot(image, h, reference, slot)
                darkCount += 1
            }
            if litCount > 0 && darkCount > 0 {
                let l = litSum / Double(litCount)
                let d = darkSum / Double(darkCount)
                if l - d > 0.2 { thresholds[ring] = 0.5 * (l + d) }
            }
        }
        var bytes = [UInt8](repeating: 0, count: PetalLane.d.wordLength)
        var confidence = [Double](repeating: .greatestFiniteMagnitude, count: PetalLane.d.wordLength)
        for (bit, slot) in PetalLayout.dataSlots.enumerated() {
            let value = normalisedDot(image, h, reference, slot)
            let threshold = thresholds[PetalLayout.splitSlot(slot).ring]
            if value > threshold { bytes[bit / 8] |= 1 << (7 - bit % 8) }
            confidence[bit / 8] = Double.minimum(confidence[bit / 8], abs(value - threshold))
        }
        return (bytes, confidence)
    }

    /// Lane `D` under one pose.
    static func readLaneD(
        _ image: PetalLumaView,
        _ h: PetalHomography,
        _ reference: Reference
    ) -> PetalLaneResult? {
        let (word, confidence) = readDots(image, h, reference)
        return decodeWithErasures(.d, word, confidence)
    }

    /// Tries Reed–Solomon with growing numbers of erasures, least confident
    /// first.
    ///
    /// The schedule erases 0, ⅛, ¼, ⅓ and ½ of the parity bytes, and for lane
    /// `K` also ⅔. Lanes `D` and `P` stop at ½: their words have only 11 and
    /// 13 parity bytes, and a further erasure step leaves so few spare ones
    /// that it accepts wrong codewords (lane `D` at 7 erasures: about 0.4 % of
    /// random words, and 5 wrong lanes in 2 900 simulated harsh frames; lane
    /// `P` at 8: 2 wrong lanes in 600 banded 480p frames). Capping them costs
    /// 0.65 % of the lane `D` reads and 0.15 % of the lane `P` reads in those
    /// frames.
    static func decodeWithErasures(_ lane: PetalLane, _ word: [UInt8], _ confidence: [Double]) -> PetalLaneResult? {
        let nsym = lane.parityLength
        let keys = confidence.map(PetalNumeric.totalOrderKey)
        // Stable ascending sort under total order (Rust `sort_by`).
        let order = word.indices.sorted { a, b in
            keys[a] != keys[b] ? keys[a] < keys[b] : a < b
        }
        var steps = [0, nsym / 8, nsym / 4, nsym / 3, nsym / 2]
        if lane == .k { steps.append(nsym * 2 / 3) }
        // Rust `Vec::dedup`: drop consecutive repeats.
        var schedule: [Int] = []
        for erasures in steps where schedule.last != erasures {
            schedule.append(erasures)
        }
        for erasures in schedule {
            let positions = Array(order.prefix(erasures))
            var trial = word
            // zero the erased bytes so stale values cannot leak through
            for position in positions { trial[position] = 0 }
            if let result = try? lane.decodeCounted(trial, erasures: positions) {
                return PetalLaneResult(data: result.data, corrected: result.corrected, erasures: positions.count)
            }
        }
        return nil
    }

    // MARK: - Tiles

    /// `patterns[(polarity * 16 + glyph) * 64 + cell]`: the expected
    /// normalised patch for each of the 32 hypotheses.
    static func buildPatterns(_ sigma: Double) -> [Double] {
        var raw = [Double](repeating: 0, count: 5)
        for i in -2...2 {
            if sigma < 0.05 {
                raw[i + 2] = i == 0 ? 1.0 : 0.0
            } else {
                let di = Double(i)
                raw[i + 2] = exp(-(di * di) / (2.0 * sigma * sigma))
            }
        }
        var sum = -0.0
        for value in raw { sum += value }
        let kernel = raw.map { $0 / sum }
        var patterns = [Double](repeating: 0, count: hypothesisCount * cells)
        var hypothesis = 0
        for polarity in 0..<2 {
            for template in PetalGlyphs.templates {
                let coverage = template.map { Double($0) / 255.0 }
                for v in 0..<patch {
                    for u in 0..<patch {
                        var acc = 0.0
                        for (ky, wy) in kernel.enumerated() {
                            for (kx, wx) in kernel.enumerated() {
                                let sx = u + kx - 2
                                let sy = v + ky - 2
                                if sx >= 0 && sx < patch && sy >= 0 && sy < patch {
                                    acc += wx * wy * coverage[sy * patch + sx]
                                }
                            }
                        }
                        let index = hypothesis * cells + v * patch + u
                        patterns[index] = polarity == 1
                            ? 1.0 - (1.0 - inkOnLight) * acc
                            : pinkOnDark * acc
                    }
                }
                hypothesis += 1
            }
        }
        return patterns
    }

    struct TileRead {
        let light: Bool
        let glyph: UInt8
        let polarityMargin: Double
        let glyphMargin: Double
        let error: Double
    }

    /// The raw 8×8 luma patch of every tile, as captured: row-major, 64 cells
    /// per tile, each the mean of four bilinear samples at ±¼ cell, in luma
    /// levels (no reference levels applied).
    static func samplePatches(_ image: PetalLumaView, _ h: PetalHomography) -> [Double] {
        var patches = [Double](repeating: 0, count: PetalLayout.tileCount * cells)
        let half = PetalLayout.glyphBox / 2.0
        let cell = PetalLayout.glyphBox / Double(patch)
        let offsets: [(Double, Double)] = [(-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)]
        patches.withUnsafeMutableBufferPointer { patches in
            for tile in 0..<PetalLayout.tileCount {
                let center = PetalLayout.tileCenter(tile)
                let cx = center.x
                let cy = center.y
                let base = tile * cells
                for v in 0..<patch {
                    for u in 0..<patch {
                        let gx = cx - half + (Double(u) + 0.5) * cell
                        let gy = cy - half + (Double(v) + 0.5) * cell
                        var sum = 0.0
                        for (ox, oy) in offsets {
                            let (px, py) = h.apply(gx + ox * cell, gy + oy * cell)
                            sum += image.sample(px, py)
                        }
                        patches[base + v * patch + u] = sum / 4.0
                    }
                }
            }
        }
        return patches
    }

    /// The robust darkest and brightest level of a patch: the values
    /// `patchCut` cells in from either end of the sorted cells.
    ///
    /// `values` holds one patch (64 cells). The sort follows the IEEE total
    /// order (Rust `sort_by(f64::total_cmp)`), so a damaged cell cannot make
    /// the result depend on the sorting algorithm.
    static func patchLevels(_ values: UnsafeBufferPointer<Double>) -> (low: Double, high: Double) {
        withUnsafeTemporaryAllocation(of: Double.self, capacity: cells) { sorted in
            // insertion sort: 64 cells, no allocation
            for index in 0..<cells {
                let value = values[index]
                let key = PetalNumeric.totalOrderKey(value)
                var slot = index
                while slot > 0 && PetalNumeric.totalOrderKey(sorted[slot - 1]) > key {
                    sorted[slot] = sorted[slot - 1]
                    slot -= 1
                }
                sorted[slot] = value
            }
            return (sorted[patchCut], sorted[cells - 1 - patchCut])
        }
    }

    /// Maps a patch's own darkest level to 0 and brightest to 1, in place,
    /// with `floor` as the smallest span that counts as contrast. Results are
    /// clamped to [-0.25, 1.25].
    static func rescale(_ values: UnsafeMutableBufferPointer<Double>, floor: Double) {
        let (low, high) = patchLevels(UnsafeBufferPointer(values))
        let range = Double.maximum(high - low, floor)
        for index in 0..<cells {
            values[index] = PetalNumeric.clamp((values[index] - low) / range, -0.25, 1.25)
        }
    }

    /// Picks for every patch the polarity and glyph whose template matches
    /// best.
    ///
    /// `patches` holds 64 cells per tile. The template blur is chosen per
    /// frame, by the lowest total error. With `rescaleTemplates` the templates
    /// are rescaled like the patches; tiles flagged in `erased` get zero
    /// margins, so they are the first the Reed–Solomon decoder treats as
    /// erasures.
    static func classify(
        _ patches: [Double],
        _ sigmas: [Double],
        rescaleTemplates: Bool,
        erased: [Bool]
    ) -> [TileRead] {
        var bestTotal = Double.greatestFiniteMagnitude
        var bestReads: [TileRead] = []
        var errors = [Double](repeating: 0, count: hypothesisCount)
        for sigma in sigmas {
            var patterns = buildPatterns(sigma)
            if rescaleTemplates {
                patterns.withUnsafeMutableBufferPointer { patterns in
                    for hypothesis in 0..<hypothesisCount {
                        let base = hypothesis * cells
                        rescale(UnsafeMutableBufferPointer(rebasing: patterns[base..<base + cells]), floor: 0.001)
                    }
                }
            }
            var total = 0.0
            var reads: [TileRead] = []
            reads.reserveCapacity(PetalLayout.tileCount)
            patches.withUnsafeBufferPointer { patches in
                patterns.withUnsafeBufferPointer { patterns in
                    errors.withUnsafeMutableBufferPointer { errors in
                        for tile in 0..<PetalLayout.tileCount {
                            let patchBase = tile * cells
                            for hypothesis in 0..<hypothesisCount {
                                let patternBase = hypothesis * cells
                                var error = -0.0
                                for index in 0..<cells {
                                    let difference = patches[patchBase + index] - patterns[patternBase + index]
                                    error += difference * difference
                                }
                                errors[hypothesis] = error
                            }
                            let best = PetalNumeric.firstMinimumIndex(UnsafeBufferPointer(errors))
                            let polarity = best / PetalGlyphs.count
                            let glyph = best % PetalGlyphs.count
                            var otherPolarity = Double.greatestFiniteMagnitude
                            var otherGlyph = Double.greatestFiniteMagnitude
                            for g in 0..<PetalGlyphs.count {
                                otherPolarity = Double.minimum(
                                    otherPolarity,
                                    errors[(1 - polarity) * PetalGlyphs.count + g]
                                )
                                if g != glyph {
                                    otherGlyph = Double.minimum(otherGlyph, errors[polarity * PetalGlyphs.count + g])
                                }
                            }
                            total += errors[best]
                            let flagged = erased[tile]
                            reads.append(TileRead(
                                light: polarity == 1,
                                glyph: UInt8(glyph),
                                polarityMargin: flagged ? 0.0 : otherPolarity - errors[best],
                                glyphMargin: flagged ? 0.0 : otherGlyph - errors[best],
                                error: errors[best]
                            ))
                        }
                    }
                }
            }
            if total < bestTotal {
                bestTotal = total
                bestReads = reads
            }
        }
        return bestReads
    }

    /// The level read: every patch is judged against the light and dark
    /// levels measured at the finders, interpolated to the tile.
    static func readTiles(_ patches: [Double], _ reference: Reference, _ sigmas: [Double]) -> [TileRead] {
        var levelled = patches
        levelled.withUnsafeMutableBufferPointer { levelled in
            for tile in 0..<PetalLayout.tileCount {
                let center = PetalLayout.tileCenter(tile)
                let (lit, dark) = reference.at(center.x, center.y)
                let base = tile * cells
                for index in base..<base + cells {
                    levelled[index] = (levelled[index] - dark) / (lit - dark)
                }
            }
        }
        return classify(
            levelled,
            sigmas,
            rescaleTemplates: false,
            erased: [Bool](repeating: false, count: PetalLayout.tileCount)
        )
    }

    /// The normalised read: every patch and every template is rescaled by its
    /// own contrast before they are compared, so the judgement does not
    /// depend on absolute levels. Tiles whose contrast is below `weakTile`
    /// times the median tile contrast are flagged as erasures.
    static func readTilesNormalised(_ patches: [Double], _ sigmas: [Double]) -> [TileRead] {
        var spans = [Double](repeating: 0, count: PetalLayout.tileCount)
        var scaled = patches
        patches.withUnsafeBufferPointer { patches in
            scaled.withUnsafeMutableBufferPointer { scaled in
                for tile in 0..<PetalLayout.tileCount {
                    let base = tile * cells
                    let (low, high) = patchLevels(UnsafeBufferPointer(rebasing: patches[base..<base + cells]))
                    spans[tile] = high - low
                    rescale(UnsafeMutableBufferPointer(rebasing: scaled[base..<base + cells]), floor: 1.0)
                }
            }
        }
        // element `tileCount / 2` of the ascending spans, under the IEEE total order
        let median = spans.sorted {
            PetalNumeric.totalOrderKey($0) < PetalNumeric.totalOrderKey($1)
        }[PetalLayout.tileCount / 2]
        var erased = [Bool](repeating: false, count: PetalLayout.tileCount)
        for (tile, span) in spans.enumerated() { erased[tile] = span < weakTile * median }
        return classify(scaled, sigmas, rescaleTemplates: true, erased: erased)
    }

    /// Lanes `P` and `K` from one set of patches: the level read first, then,
    /// for any lane still unreadable, the normalised read.
    static func readTileLanes(
        _ patches: [Double],
        _ reference: Reference,
        _ sigmas: [Double]
    ) -> (p: PetalLaneResult?, k: PetalLaneResult?) {
        let words = tileWords(readTiles(patches, reference, sigmas))
        var p = decodeWithErasures(.p, words.p, words.pConfidence)
        var k = decodeWithErasures(.k, words.k, words.kConfidence)
        if p == nil || k == nil {
            let words = tileWords(readTilesNormalised(patches, sigmas))
            if p == nil { p = decodeWithErasures(.p, words.p, words.pConfidence) }
            if k == nil { k = decodeWithErasures(.k, words.k, words.kConfidence) }
        }
        return (p, k)
    }

    struct TileWords {
        var p: [UInt8]
        var pConfidence: [Double]
        var k: [UInt8]
        var kConfidence: [Double]
    }

    static func tileWords(_ reads: [TileRead]) -> TileWords {
        var words = TileWords(
            p: [UInt8](repeating: 0, count: PetalLane.p.wordLength),
            pConfidence: [Double](repeating: .greatestFiniteMagnitude, count: PetalLane.p.wordLength),
            k: [UInt8](repeating: 0, count: PetalLane.k.wordLength),
            kConfidence: [Double](repeating: .greatestFiniteMagnitude, count: PetalLane.k.wordLength)
        )
        for (tile, read) in reads.enumerated() {
            if read.light { words.p[tile / 8] |= 1 << (7 - tile % 8) }
            words.pConfidence[tile / 8] = Double.minimum(words.pConfidence[tile / 8], read.polarityMargin)
            words.k[tile / 2] |= tile % 2 == 0 ? read.glyph << 4 : read.glyph
            words.kConfidence[tile / 2] = Double.minimum(
                words.kConfidence[tile / 2],
                Double.minimum(read.glyphMargin, read.polarityMargin)
            )
        }
        return words
    }

    static func finish(
        _ image: PetalLumaView,
        _ options: PetalDecodeOptions,
        _ candidate: Candidate,
        d: PetalLaneResult?
    ) -> PetalDecodedFrame {
        let h = candidate.homography
        let d = d ?? readLaneD(image, h, candidate.reference)
        let patches = samplePatches(image, h)
        let lanes = readTileLanes(patches, candidate.reference, options.templateSigmas)
        return PetalDecodedFrame(
            homography: h,
            rotation: candidate.rotation,
            mirrored: candidate.mirrored,
            p: lanes.p,
            k: lanes.k,
            d: d
        )
    }
}
