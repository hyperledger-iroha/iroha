import Foundation

/// Decoder tuning.
public struct PetalDecodeOptions: Equatable, Sendable {
    /// Also try horizontally mirrored images (front-camera previews).
    public var tryMirrored: Bool
    /// Largest image (in pixels) the decoder accepts; larger frames should be
    /// downscaled by the caller. Bounds memory and work on hostile input.
    public var maximumPixels: Int

    public init(tryMirrored: Bool = true, maximumPixels: Int = 12_000_000) {
        self.tryMirrored = tryMirrored
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

/// What one capture shows of a frame's petal field: the level of every field
/// cell sampled under the capture's decoded pose and normalised by that
/// capture's finder levels. Use ``PetalDecoder/fieldEvidence(_:frame:options:)``
/// to sample it and ``PetalDecoder/mergeFields(_:skip:)`` to average captures of
/// one shown frame. One evidence holds about 27 KB.
public struct PetalFieldEvidence: Equatable, Sendable {
    /// The normalised level of every field cell, in cell order.
    let levels: [Double]
}

/// Everything read from one camera frame.
public struct PetalDecodedFrame: Equatable, Sendable {
    /// Canvas-to-pixel pose that was used.
    public let pose: PetalPose
    /// Orientation: how many quarter turns the code is rotated, i.e. which
    /// image corner (clockwise from the top-left) holds the code's top-left
    /// finder.
    public let rotation: Int
    /// Whether the image was mirrored.
    public let mirrored: Bool
    /// Lane `D` result.
    public let d: PetalLaneResult?
    /// Field lane results, sector 0 first (always four entries).
    public let fields: [PetalLaneResult?]
    /// The corner finder that was hidden (by a finger, a glare or the edge of
    /// the frame) and inferred from the other three, as its canonical index:
    /// 0 top-left, 1 top-right, 2 bottom-right, 3 bottom-left of the upright
    /// code.
    public let inferredCorner: Int?

    /// Number of lanes that decoded.
    public var lanesOK: Int {
        (d == nil ? 0 : 1) + fields.reduce(0) { $0 + ($1 == nil ? 0 : 1) }
    }

    /// The result of `lane`, when it decoded.
    public func lane(_ lane: PetalLane) -> PetalLaneResult? {
        switch lane {
        case .d: return d
        case .field(let sector): return fields.indices.contains(sector) ? fields[sector] : nil
        }
    }

    /// The lanes that decoded, as letters: `D` and the sector digits `0`–`3`.
    public var laneLetters: String {
        var letters = ""
        for lane in PetalLane.decodeOrder where self.lane(lane) != nil {
            letters.append(lane.letter)
        }
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

    /// Atom packets from every lane that decoded: the field lanes in sector
    /// order, then lane `D`.
    public var atomPackets: [PetalAtomPacket] {
        var packets: [PetalAtomPacket] = []
        for lane in PetalLane.fields {
            if let result = self.lane(lane), let packet = PetalStream.parseAtomLane(lane, data: result.data) {
                packets.append(packet)
            }
        }
        if case .atoms(let packet) = dLane { packets.append(packet) }
        return packets
    }

    /// Offers everything this frame carries to `assembler`.
    public func feed(_ assembler: inout PetalStreamAssembler) {
        if let lane = dLane { assembler.push(dLane: lane) }
        for lane in PetalLane.fields {
            if let result = self.lane(lane), let packet = PetalStream.parseAtomLane(lane, data: result.data) {
                assembler.push(atoms: packet)
            }
        }
    }
}

/// Why a frame could not be decoded at all.
public enum PetalDecodeError: Error, Equatable, LocalizedError, Sendable {
    /// The image is smaller than 48 pixels on a side or larger than
    /// ``PetalDecodeOptions/maximumPixels``.
    case unsupportedImage
    /// No set of corner finders (four, or three forming a corner) was found.
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
/// The decoder locates the corner finders (four, or three with the fourth
/// inferred), derives a pose for each orientation hypothesis (four rotations,
/// optionally mirrored), ranks the orientations by how well the ring gates
/// line up (lane `D` must check out), then reads the dots and the petal field.
/// Every petal is one bit: its level, normalised by the finder levels, is
/// compared with a threshold computed for its codeword and colour class (the
/// petals inside the `天` are lighter than the pink ones). Bytes the decoder is
/// unsure about become Reed–Solomon erasures. A lane is only reported when its
/// codeword checks out; the payload CRC-32C of the stream catches the rare
/// miscorrection.
///
/// A field lane the levels leave unread is read again from *local levels*:
/// every cell rescaled by the darkest and brightest cells of its
/// neighbourhood, so shadows, glare, banding and veiling light cancel. When
/// some field lanes do not decode, the pose is searched for radial lens
/// distortion: a cheap camera's barrel distortion moves the petals by a third
/// of their pitch at the edge of a 480p frame, while the finders that fix the
/// homography sit far outside the field.
///
/// A camera sees each shown frame two to four times.
/// ``fieldEvidence(_:frame:options:)`` keeps what one capture shows of the
/// petal field, and ``mergeFields(_:skip:)`` reads the field lanes from the
/// average of several captures of one shown frame, which recovers lanes that
/// noise, display banding or hand shake cost each capture alone.
public enum PetalDecoder {
    /// Distortion steps tried, relative to the starting pose, when a plain
    /// pose leaves field lanes unread; the best is then refined by
    /// ±``distortionRefine``.
    static let distortionSteps: [Double] = [-0.03, -0.06, -0.09, -0.12, -0.16, 0.03]
    /// Final refinement step of the distortion search.
    static let distortionRefine = 0.015
    /// A colour class with fewer cells in a codeword than this takes the
    /// codeword's threshold.
    static let classMinimumCells = 16
    /// Field cells within this distance of a cell are its neighbourhood for
    /// the local read.
    static let neighbourhood = 32.0
    /// Cells averaged at each end of a sorted neighbourhood for its local dark
    /// and lit levels.
    static let neighbourhoodEnds = 5
    /// Sample spread of a ring dot, in canvas units.
    static let dotSpread = 0.32 * PetalLayout.dotRadius
    /// Sample spread of a petal, in canvas units.
    static let petalSpread = 0.4 * PetalLayout.petalRadius
    /// Cells of a sector that carry its codeword.
    static let fieldBits = PetalLane.field(0).wordLength * 8
    /// No sector skipped.
    static let readAll = [Bool](repeating: false, count: PetalLayout.sectors)
    /// No field lane read.
    static let noFields = [PetalLaneResult?](repeating: nil, count: PetalLayout.sectors)

    /// Decodes one camera frame.
    ///
    /// Tries the finder candidates of ``PetalLocator/candidates(_:)`` in order
    /// and returns the first that reads. For each, the orientation hypotheses
    /// (four quarter turns, optionally mirrored) are ranked by the ring gates;
    /// lane `D` is tried under the best three whose gate score is at least
    /// 0.2, then the field lanes under the best four.
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
            var candidates = PetalCandidateSearch()
            var located = false
            while let set = candidates.next(view) {
                located = true
                if let frame = decodeCandidate(view, options, set) { return .success(frame) }
            }
            return .failure(located ? .noOrientation : .noFinders)
        }
    }

    /// Follows a code from the previous frame that decoded, without searching
    /// the whole image for finders (the most expensive part of
    /// ``decode(_:options:)``).
    ///
    /// Each corner finder seen in the previous frame is re-found near where the
    /// previous pose puts it (see ``PetalLocator/follow(_:expected:)``); the
    /// mean movement of those predicts the rest. A corner inferred in the
    /// previous frame counts as seen again only when its blossom is re-found
    /// within a quarter diameter of that prediction (so a thumb beside it does
    /// not count). When exactly one corner is missing it is placed at the
    /// prediction and refined against the rings like an inferred corner. The
    /// orientation and the lens distortion are kept from the previous frame
    /// (the distortion is searched again when a field lane does not decode).
    /// Returns `nil` for a broken previous pose (non-finite, or finders larger
    /// than the image), when two corners are lost or when no lane decodes; the
    /// caller then runs ``decode(_:options:)``.
    public static func track(
        _ image: PetalLuma,
        previous: PetalDecodedFrame,
        options: PetalDecodeOptions = PetalDecodeOptions()
    ) -> PetalDecodedFrame? {
        // a frame built by the caller may name a corner that does not exist
        guard isSupported(image, options: options),
              previous.inferredCorner.map({ (0...3).contains($0) }) ?? true else { return nil }
        return image.withView { view in track(view, previous: previous) }
    }

    static func track(_ image: PetalLumaView, previous: PetalDecodedFrame) -> PetalDecodedFrame? {
        let p0 = previous.pose
        func span(_ a: (Double, Double), _ b: (Double, Double)) -> Double {
            let dx = a.0 - b.0
            let dy = a.1 - b.1
            return (dx * dx + dy * dy).squareRoot()
        }
        let expected = PetalLayout.finderCenters.map { center -> PetalFinder in
            let cx = center.x
            let cy = center.y
            let (x, y) = p0.apply(cx, cy)
            let size = Double.maximum(
                span(p0.apply(cx - 60.0, cy), p0.apply(cx + 60.0, cy)),
                span(p0.apply(cx, cy - 60.0), p0.apply(cx, cy + 60.0))
            )
            return PetalFinder(x: x, y: y, size: size)
        }
        let short = Double(min(image.width, image.height))
        for finder in expected
        where !(finder.x.isFinite && finder.y.isFinite && finder.size.isFinite) || finder.size > short {
            return nil
        }
        let previouslyInferred = previous.inferredCorner
        var found: [PetalFinder?] = (0..<4).map { index in
            previouslyInferred == index ? nil : PetalLocator.follow(image, expected: expected[index])
        }
        // the mean movement of the corners that were followed predicts the others
        var moved: [(x: Double, y: Double)] = []
        for index in 0..<4 {
            if let f = found[index] { moved.append((f.x - expected[index].x, f.y - expected[index].y)) }
        }
        if moved.count < 3 { return nil }
        var sumX = -0.0
        for m in moved { sumX += m.x }
        var sumY = -0.0
        for m in moved { sumY += m.y }
        let shift = (x: sumX / Double(moved.count), y: sumY / Double(moved.count))
        func predicted(_ index: Int) -> PetalFinder {
            PetalFinder(x: expected[index].x + shift.x, y: expected[index].y + shift.y, size: expected[index].size)
        }
        // a corner that was hidden is seen again only when its blossom is found right where
        // the others say it is (a bright thumb beside it must not count)
        if let m = previouslyInferred, (0..<4).contains(m) {
            let at = predicted(m)
            found[m] = PetalLocator.follow(image, expected: at).flatMap { f in
                let dx = f.x - at.x
                let dy = f.y - at.y
                return (dx * dx + dy * dy).squareRoot() <= 0.25 * at.size ? f : nil
            }
        }
        let lost = (0..<4).filter { found[$0] == nil }
        let inferred: Int?
        switch lost.count {
        case 0:
            inferred = nil
        case 1:
            found[lost[0]] = predicted(lost[0])
            inferred = lost[0]
        default:
            return nil
        }
        var corners = found.map { $0 ?? expected[0] }
        if let m = inferred { corners = refineInferredCorner(image, corners, inferred: m) }
        let points = corners.map { PetalPoint(x: $0.x, y: $0.y) }
        guard let read = readFieldSearching(image, points, start: previous.pose.distortion, inferred: inferred) else {
            return nil
        }
        let d = readLaneD(image, read.pose, read.reference)
        if d == nil && read.decoded == 0 { return nil }
        return PetalDecodedFrame(
            pose: read.pose,
            rotation: previous.rotation,
            mirrored: previous.mirrored,
            d: d,
            fields: read.fields,
            inferredCorner: inferred
        )
    }

    /// Reads all lanes with a known canvas-to-pixel pose (no finder search, no
    /// distortion search).
    ///
    /// Returns `nil` when the image is unusable or the finder reference levels
    /// are too weak. Used by refinement passes and by qualification tooling with
    /// a ground-truth pose. The reference levels come from all four corner
    /// finders, so a frame whose corner blossom is hidden is refused here (the
    /// hidden corner has no contrast); ``decode(_:options:)`` and
    /// ``track(_:previous:options:)`` read such frames by inferring that corner.
    public static func decode(
        _ image: PetalLuma,
        pose: PetalPose,
        options: PetalDecodeOptions = PetalDecodeOptions()
    ) -> PetalDecodedFrame? {
        guard isSupported(image, options: options) else { return nil }
        return image.withView { view in
            guard let reference = referenceLevels(view, pose, inferred: nil) else { return nil }
            let fields = readFields(fieldLevels(view, pose, reference), skip: readAll).results
            return PetalDecodedFrame(
                pose: pose,
                rotation: 0,
                mirrored: false,
                d: readLaneD(view, pose, reference),
                fields: fields,
                inferredCorner: nil
            )
        }
    }

    /// Builds the cells a decoder believes it saw under `frame`'s pose, for
    /// diagnostics: the dots as lane `D` reads them, and every codeword
    /// thresholded as the first read of ``decode(_:options:)`` thresholds it
    /// (no local read). The petals are left unturned (the turns a reader sees
    /// are ``PetalTurns/observedTurns(_:frame:)``). Returns `nil` for an image
    /// or frame that ``fieldEvidence(_:frame:options:)`` refuses.
    public static func observedCells(
        _ image: PetalLuma,
        frame: PetalDecodedFrame,
        options: PetalDecodeOptions = PetalDecodeOptions()
    ) -> PetalFrameCells? {
        guard let evidence = fieldEvidence(image, frame: frame, options: options) else { return nil }
        return image.withView { view in
            guard let reference = referenceLevels(view, frame.pose, inferred: frame.inferredCorner) else {
                return nil
            }
            let (d, _) = readDots(view, frame.pose, reference)
            var margins = 0.0
            var scratch = WordScratch()
            let words = (0..<PetalLayout.sectors).map { sector in
                readWord(evidence.levels, sector: sector, margins: &margins, scratch: &scratch).bytes
            }
            return PetalFrameCells(validatedD: d, fields: words)
        }
    }

    /// The field evidence of `image` under the pose `frame` was decoded with.
    ///
    /// Returns `nil` for an unusable image, a frame that names a corner that
    /// does not exist, or finder levels too weak to read. Evidence is in
    /// canonical code coordinates, so captures may have different positions,
    /// rotations and reflections.
    public static func fieldEvidence(
        _ image: PetalLuma,
        frame: PetalDecodedFrame,
        options: PetalDecodeOptions = PetalDecodeOptions()
    ) -> PetalFieldEvidence? {
        // a frame built by the caller may name a corner that does not exist
        guard isSupported(image, options: options),
              frame.inferredCorner.map({ (0...3).contains($0) }) ?? true else { return nil }
        return image.withView { view in
            guard let reference = referenceLevels(view, frame.pose, inferred: frame.inferredCorner) else {
                return nil
            }
            return PetalFieldEvidence(levels: fieldLevels(view, frame.pose, reference))
        }
    }

    /// The field lanes from several captures of one shown frame: the
    /// cell-by-cell mean of their levels (summed in capture order, then divided
    /// by the number of captures), read as one capture's levels are, local read
    /// included. Averaging cancels the sensor noise, banding and shake that
    /// differ between captures; one evidence reads exactly the field lanes
    /// ``decode(_:options:)`` read from that capture's pose. Sectors flagged in
    /// `skip` are not read.
    ///
    /// The caller must group evidence by independently decoded lane headers and
    /// check each recovered lane's header before offering its atoms: a lane
    /// read here names its frame in its own header.
    public static func mergeFields(
        _ evidence: [PetalFieldEvidence],
        skip: [Bool] = [false, false, false, false]
    ) -> [PetalLaneResult?] {
        guard !evidence.isEmpty else { return noFields }
        var mean = [Double](repeating: 0, count: PetalLayout.fieldCells)
        for capture in evidence {
            for index in mean.indices { mean[index] += capture.levels[index] }
        }
        let count = Double(evidence.count)
        for index in mean.indices { mean[index] /= count }
        let skip = (0..<PetalLayout.sectors).map { $0 < skip.count && skip[$0] }
        return readFields(mean, skip: skip).results
    }

    private static func isSupported(_ image: PetalLuma, options: PetalDecodeOptions) -> Bool {
        let (pixels, overflow) = image.width.multipliedReportingOverflow(by: image.height)
        return image.width >= 48 && image.height >= 48
            && !overflow && pixels <= options.maximumPixels
            && image.pixels.count == pixels
    }

    // MARK: - Orientation

    /// One hypothesis: gate score, orientation, finder centres, plain pose and
    /// levels.
    struct Scored {
        let gate: Double
        let rotation: Int
        let mirrored: Bool
        let destination: [PetalPoint]
        let pose: PetalPose
        let reference: Reference
    }

    /// Reads one finder candidate: ranks its orientation hypotheses by gate
    /// score, tries lane `D` under the best three whose gate score is at least
    /// 0.2, then the field lanes under the best four.
    static func decodeCandidate(
        _ image: PetalLumaView,
        _ options: PetalDecodeOptions,
        _ set: PetalFinderSet
    ) -> PetalDecodedFrame? {
        let corners = set.inferred.map { refineInferredCorner(image, set.corners, inferred: $0) } ?? set.corners
        func inferredCorner(_ rotation: Int, _ mirrored: Bool) -> Int? {
            set.inferred.map { canonicalCorner($0, rotation: rotation, mirrored: mirrored) }
        }
        var scored: [Scored] = []
        for hypothesis in hypotheses(corners, tryMirrored: options.tryMirrored) {
            let inferred = inferredCorner(hypothesis.rotation, hypothesis.mirrored)
            guard let pose = fitPose(image, hypothesis.destination, distortion: 0.0),
                  let reference = referenceLevels(image, pose, inferred: inferred) else { continue }
            scored.append(Scored(
                gate: gateScore(image, pose, reference),
                rotation: hypothesis.rotation,
                mirrored: hypothesis.mirrored,
                destination: hypothesis.destination,
                pose: pose,
                reference: reference
            ))
        }
        // Stable descending sort under total order (Rust `sort_by`).
        let keys = scored.map { PetalNumeric.totalOrderKey($0.gate) }
        let ranked = scored.indices.sorted { left, right in
            keys[left] != keys[right] ? keys[left] > keys[right] : left < right
        }.map { scored[$0] }
        // 1. the ring beacon is the cheapest and strongest orientation check
        for candidate in ranked.prefix(3) {
            if candidate.gate < 0.2 { continue }
            if let d = readLaneD(image, candidate.pose, candidate.reference) {
                let inferred = inferredCorner(candidate.rotation, candidate.mirrored)
                let read = readFieldSearching(image, candidate.destination, start: 0.0, inferred: inferred)
                return PetalDecodedFrame(
                    pose: read?.pose ?? candidate.pose,
                    rotation: candidate.rotation,
                    mirrored: candidate.mirrored,
                    d: d,
                    fields: read?.fields ?? noFields,
                    inferredCorner: inferred
                )
            }
        }
        // 2. fall back to the field lanes under the most promising orientations: a wrong
        //    orientation reads permuted petals, which no field codeword accepts. Strong lens
        //    distortion can hide both lane D and the field from a plain pose, so the distortion
        //    is searched under the best orientation whose gates line up even when its plain
        //    pose reads nothing.
        for (index, candidate) in ranked.prefix(4).enumerated() {
            let inferred = inferredCorner(candidate.rotation, candidate.mirrored)
            guard let plain = readFieldAt(image, candidate.destination, distortion: 0.0, inferred: inferred) else {
                continue
            }
            let confident = index == 0 && candidate.gate >= 0.2
            // the local read costs a sort per cell: only the best orientation gets it for free
            let read: FieldRead
            if plain.decoded == PetalLayout.sectors {
                read = plain
            } else if plain.decoded == 0 && !confident {
                read = index == 0 ? plain.finish() : plain
            } else {
                read = readFieldSearching(image, candidate.destination, start: 0.0, inferred: inferred)
                    ?? plain.finish()
            }
            if read.decoded == 0 { continue }
            return PetalDecodedFrame(
                pose: read.pose,
                rotation: candidate.rotation,
                mirrored: candidate.mirrored,
                d: readLaneD(image, read.pose, read.reference),
                fields: read.fields,
                inferredCorner: inferred
            )
        }
        return nil
    }

    /// Canonical index of the corner at index `index` of a finder quad under
    /// one orientation hypothesis.
    static func canonicalCorner(_ index: Int, rotation: Int, mirrored: Bool) -> Int {
        mirrored ? (rotation + 4 - index) % 4 : (index + 4 - rotation) % 4
    }

    /// One orientation hypothesis: quarter turns, mirrored, and the finder
    /// centres in canonical corner order.
    struct Hypothesis {
        let rotation: Int
        let mirrored: Bool
        let destination: [PetalPoint]
    }

    /// The four orientation hypotheses (and four mirrored ones).
    static func hypotheses(_ finders: [PetalFinder], tryMirrored: Bool) -> [Hypothesis] {
        var out: [Hypothesis] = []
        for mirrored in [false, true] {
            if mirrored && !tryMirrored { continue }
            for rotation in 0..<4 {
                let destination = (0..<4).map { i -> PetalPoint in
                    let q = mirrored ? finders[(rotation + 4 - i) % 4] : finders[(i + rotation) % 4]
                    return PetalPoint(x: q.x, y: q.y)
                }
                out.append(Hypothesis(rotation: rotation, mirrored: mirrored, destination: destination))
            }
        }
        return out
    }

    // MARK: - Pose, reference levels and dots

    /// The pose with lens distortion `distortion` through the four finder
    /// centres `destination` (image pixels, in canonical corner order).
    @inline(__always)
    static func fitPose(_ image: PetalLumaView, _ destination: [PetalPoint], distortion: Double) -> PetalPose? {
        PetalPose.fit(
            from: PetalLayout.finderCenters,
            to: destination,
            distortion: distortion,
            width: image.width,
            height: image.height
        )
    }

    /// Lit and dark levels at the four finders, interpolated over the canvas.
    struct Reference: Equatable, Sendable {
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

    /// The mean of five samples: the centre and four at `±spread` canvas units
    /// on `x`, then `y`.
    @inline(__always)
    static func dotSamples(
        _ image: PetalLumaView,
        _ pose: PetalPose,
        _ x: Double,
        _ y: Double,
        _ spread: Double
    ) -> Double {
        var sum = 0.0
        var (px, py) = pose.apply(x + 0.0, y + 0.0)
        sum += image.sample(px, py)
        (px, py) = pose.apply(x + spread, y + 0.0)
        sum += image.sample(px, py)
        (px, py) = pose.apply(x + -spread, y + 0.0)
        sum += image.sample(px, py)
        (px, py) = pose.apply(x + 0.0, y + spread)
        sum += image.sample(px, py)
        (px, py) = pose.apply(x + 0.0, y + -spread)
        sum += image.sample(px, py)
        return sum / 5.0
    }

    /// Light and dark levels at the four corners: the solid blossom core, and
    /// the black canvas 100 units inward of it. An `inferred` corner
    /// (canonical index) was not seen, so its levels are extrapolated from the
    /// other three by the parallelogram rule and kept within their range.
    static func referenceLevels(_ image: PetalLumaView, _ pose: PetalPose, inferred: Int?) -> Reference? {
        var lit = [Double](repeating: 0, count: 4)
        var dark = [Double](repeating: 0, count: 4)
        let tau = 2 * Double.pi
        for (i, center) in PetalLayout.finderCenters.enumerated() where i != inferred {
            let cx = center.x
            let cy = center.y
            // the blossom is solid out to radius 24 around its centre
            var (px, py) = pose.apply(cx, cy)
            var sum = image.sample(px, py)
            for k in 0..<8 {
                let (s, c) = PetalNumeric.sinCos(tau * Double(k) / 8.0)
                (px, py) = pose.apply(cx + 20.0 * c, cy + 20.0 * s)
                sum += image.sample(px, py)
            }
            lit[i] = sum / 9.0
            let sx = cx < 512.0 ? 1.0 : -1.0
            let sy = cy < 512.0 ? 1.0 : -1.0
            let a = dotSamples(image, pose, cx + sx * 100.0, cy, 5.0)
            let b = dotSamples(image, pose, cx, cy + sy * 100.0, 5.0)
            dark[i] = 0.5 * (a + b)
            // a NaN contrast (from a broken pose) refuses as well
            if !(lit[i] - dark[i]).isFinite || lit[i] - dark[i] < 12.0 { return nil }
        }
        if let m = inferred {
            let n1 = (m + 1) % 4
            let opposite = (m + 2) % 4
            let n2 = (m + 3) % 4
            func extrapolate(_ v: [Double]) -> Double {
                let low = Double.minimum(Double.minimum(v[n1], v[opposite]), v[n2])
                let high = Double.maximum(Double.maximum(v[n1], v[opposite]), v[n2])
                return PetalNumeric.clamp(v[n1] + v[n2] - v[opposite], low, high)
            }
            lit[m] = extrapolate(lit)
            dark[m] = extrapolate(dark)
            // uneven light can push the estimates past each other; an inferred
            // corner needs the same contrast as a seen one
            if lit[m] - dark[m] < 12.0 { return nil }
        }
        return Reference(lit: lit, dark: dark)
    }

    /// The brightness summed over all ring slots under the plain pose that maps
    /// the canonical corners onto `corners` (in quad order). The slots form the
    /// same set of points under every quarter turn and mirror of the canvas
    /// (88, 100 and 108 are multiples of four), so the value does not depend on
    /// the orientation.
    static func ringBrightness(_ image: PetalLumaView, _ corners: [PetalPoint]) -> Double? {
        guard let pose = fitPose(image, corners, distortion: 0.0) else { return nil }
        var sum = -0.0
        for center in PetalLayout.flatSlotCenters {
            sum += dotSamples(image, pose, center.x, center.y, dotSpread)
        }
        return sum
    }

    /// Moves an inferred corner to where the three dotted rings line up best:
    /// a 13 × 13 search in steps of 2 % of the mean leg around the
    /// parallelogram estimate, then a 9 × 9 search in steps of 0.5 % around the
    /// best point. The rings fix the geometry only; the orientation is decided
    /// afterwards by the gates and the lanes.
    static func refineInferredCorner(
        _ image: PetalLumaView,
        _ corners: [PetalFinder],
        inferred: Int
    ) -> [PetalFinder] {
        var points = corners.map { PetalPoint(x: $0.x, y: $0.y) }
        let start = points[inferred]
        func distance(_ other: Int) -> Double {
            let dx = points[other].x - start.x
            let dy = points[other].y - start.y
            return (dx * dx + dy * dy).squareRoot()
        }
        let leg = 0.5 * (distance((inferred + 1) % 4) + distance((inferred + 3) % 4))
        var best = (brightness: -Double.greatestFiniteMagnitude, point: start)
        func search(_ centre: PetalPoint, step: Double, reach: Int) {
            for dy in -reach...reach {
                for dx in -reach...reach {
                    let candidate = PetalPoint(x: centre.x + Double(dx) * step, y: centre.y + Double(dy) * step)
                    points[inferred] = candidate
                    if let brightness = ringBrightness(image, points), brightness > best.brightness {
                        best = (brightness, candidate)
                    }
                }
            }
        }
        search(start, step: 0.02 * leg, reach: 6)
        search(best.point, step: 0.005 * leg, reach: 4)
        var refined = corners
        refined[inferred] = PetalFinder(x: best.point.x, y: best.point.y, size: corners[inferred].size)
        return refined
    }

    @inline(__always)
    static func normalisedDot(
        _ image: PetalLumaView,
        _ pose: PetalPose,
        _ reference: Reference,
        _ flat: Int
    ) -> Double {
        let center = PetalLayout.flatSlotCenters[flat]
        let (lit, dark) = reference.at(center.x, center.y)
        return (dotSamples(image, pose, center.x, center.y, dotSpread) - dark) / (lit - dark)
    }

    /// The mean normalised sample of the gate dots minus that of the guards.
    static func gateScore(_ image: PetalLumaView, _ pose: PetalPose, _ reference: Reference) -> Double {
        func mean(_ slots: [Int]) -> Double {
            var sum = -0.0
            for slot in slots { sum += normalisedDot(image, pose, reference, slot) }
            return sum / Double(slots.count)
        }
        return mean(PetalLayout.gateSlots) - mean(PetalLayout.guardSlots)
    }

    /// Reads lane `D`: the transmitted bytes and per-byte confidence.
    static func readDots(
        _ image: PetalLumaView,
        _ pose: PetalPose,
        _ reference: Reference
    ) -> ([UInt8], [Double]) {
        // per-ring thresholds from the gates (lit) and guards (dark)
        var thresholds = [Double](repeating: 0.5, count: PetalLayout.ringCount)
        for ring in 0..<PetalLayout.ringCount {
            var litSum = -0.0
            var litCount = 0
            for slot in PetalLayout.gateSlots where PetalLayout.splitSlot(slot).ring == ring {
                litSum += normalisedDot(image, pose, reference, slot)
                litCount += 1
            }
            var darkSum = -0.0
            var darkCount = 0
            for slot in PetalLayout.guardSlots where PetalLayout.splitSlot(slot).ring == ring {
                darkSum += normalisedDot(image, pose, reference, slot)
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
            let value = normalisedDot(image, pose, reference, slot)
            let threshold = thresholds[PetalLayout.splitSlot(slot).ring]
            if value > threshold { bytes[bit / 8] |= 1 << (7 - bit % 8) }
            confidence[bit / 8] = Double.minimum(confidence[bit / 8], abs(value - threshold))
        }
        return (bytes, confidence)
    }

    /// Lane `D` under one pose.
    static func readLaneD(
        _ image: PetalLumaView,
        _ pose: PetalPose,
        _ reference: Reference
    ) -> PetalLaneResult? {
        let (word, confidence) = readDots(image, pose, reference)
        return decodeWithErasures(.d, word, confidence)
    }

    /// Tries Reed–Solomon with growing numbers of erasures, least confident
    /// first.
    ///
    /// The schedule erases 0, ⅛, ¼, ⅓ and ½ of the parity bytes (integer
    /// division: lane `D` 0, 1, 2, 3, 5; a field lane 0, 2, 5, 7, 11). Lane `D`
    /// has only 11 parity bytes, and a further step leaves so few spare ones
    /// that it accepts wrong codewords (at 7 erasures about 0.4 % of random
    /// words). Bytes of equal confidence are erased in position order.
    static func decodeWithErasures(_ lane: PetalLane, _ word: [UInt8], _ confidence: [Double]) -> PetalLaneResult? {
        let nsym = lane.parityLength
        let keys = confidence.map(PetalNumeric.totalOrderKey)
        // Stable ascending sort under total order (Rust `sort_by`).
        let order = word.indices.sorted { a, b in
            keys[a] != keys[b] ? keys[a] < keys[b] : a < b
        }
        // Rust `Vec::dedup`: drop consecutive repeats.
        var schedule: [Int] = []
        for erasures in [0, nsym / 8, nsym / 4, nsym / 3, nsym / 2] where schedule.last != erasures {
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

    // MARK: - The petal field

    /// The two-means threshold of a set of values: start at their mean, then
    /// move to the midpoint of the means of the values above and at or below
    /// the threshold, at most eight times, stopping when it moves less than
    /// 10⁻⁴ or one side is empty. Sums run in the order given.
    static func twoMeans(_ values: [Double]) -> Double {
        values.withUnsafeBufferPointer { values in
            var sum = -0.0
            for value in values { sum += value }
            var threshold = sum / Double(values.count)
            for _ in 0..<8 {
                var low = 0.0
                var lows = 0
                var high = 0.0
                var highs = 0
                for value in values {
                    if value > threshold {
                        high += value
                        highs += 1
                    } else {
                        low += value
                        lows += 1
                    }
                }
                if lows == 0 || highs == 0 { break }
                let next = 0.5 * (low / Double(lows) + high / Double(highs))
                let moved = abs(next - threshold)
                threshold = next
                if moved < 1e-4 { break }
            }
            return threshold
        }
    }

    /// The 840 codeword cells of every sector, in bit order.
    static let sectorBits: [[Int]] = (0..<PetalLayout.sectors).map {
        Array(PetalLayout.sectorTable[$0].prefix(fieldBits))
    }
    /// Whether each codeword cell of every sector is light.
    static let sectorBitLight: [[Bool]] = sectorBits.map { cells in cells.map { PetalLayout.lightCells[$0] } }

    /// The level of every field cell under one pose, normalised by the finder
    /// levels.
    static func fieldLevels(_ image: PetalLumaView, _ pose: PetalPose, _ reference: Reference) -> [Double] {
        let xs = PetalLayout.cellCenterX
        let ys = PetalLayout.cellCenterY
        let spread = petalSpread
        return [Double](unsafeUninitializedCapacity: PetalLayout.fieldCells) { levels, count in
            for cell in 0..<PetalLayout.fieldCells {
                let x = xs[cell]
                let y = ys[cell]
                let (lit, dark) = reference.at(x, y)
                levels[cell] = (dotSamples(image, pose, x, y, spread) - dark) / (lit - dark)
            }
            count = PetalLayout.fieldCells
        }
    }

    /// The neighbourhood of every field cell, flattened: the cells (itself
    /// included) whose centres lie within ``neighbourhood`` units, in index
    /// order, are `cells[starts[c]..<starts[c + 1]]`. Bands are 11 units apart,
    /// so only the three bands on either side can hold neighbours.
    static let neighbourhoods: (starts: [Int], cells: [UInt16]) = {
        let xs = PetalLayout.cellCenterX
        let ys = PetalLayout.cellCenterY
        let limit = neighbourhood * neighbourhood
        var starts = [0]
        starts.reserveCapacity(PetalLayout.fieldCells + 1)
        var cells: [UInt16] = []
        for cell in 0..<PetalLayout.fieldCells {
            let band = Int(PetalLayout.cellBands[cell])
            let first = PetalLayout.bandOffsets[max(band - 3, 0)]
            let last = PetalLayout.bandOffsets[min(band + 4, PetalLayout.bandCount)]
            for other in first..<last {
                let dx = xs[other] - xs[cell]
                let dy = ys[other] - ys[cell]
                if dx * dx + dy * dy <= limit { cells.append(UInt16(other)) }
            }
            starts.append(cells.count)
        }
        return (starts, cells)
    }()

    /// Cell levels rescaled by their neighbourhood: the mean of the lowest and
    /// of the highest ``neighbourhoodEnds`` levels around a cell (IEEE total
    /// order, summed ascending) become 0 and 1 (a span below 0.05 counts as
    /// 0.05). About half the petals around any cell are lit, so shadows, glare,
    /// banding and veiling light, which the finder levels cannot describe,
    /// cancel.
    static func localLevels(_ levels: [Double]) -> [Double] {
        let table = neighbourhoods
        let ends = neighbourhoodEnds
        return [Double](unsafeUninitializedCapacity: PetalLayout.fieldCells) { out, count in
            levels.withUnsafeBufferPointer { levels in
                table.starts.withUnsafeBufferPointer { starts in
                    table.cells.withUnsafeBufferPointer { cells in
                        withUnsafeTemporaryAllocation(of: Double.self, capacity: 2 * ends) { scratch in
                            let low = UnsafeMutableBufferPointer(rebasing: scratch[0..<ends])
                            let high = UnsafeMutableBufferPointer(rebasing: scratch[ends..<(2 * ends)])
                            for cell in 0..<PetalLayout.fieldCells {
                                let start = starts[cell]
                                let end = starts[cell + 1]
                                let take = max(min(ends, (end - start) / 2), 1)
                                // `low[..<take]`: the smallest levels, `high[..<take]`: the
                                // largest, both ascending, as the first and last `take`
                                // entries of the sorted neighbourhood
                                var lows = 0
                                var highs = 0
                                for index in start..<end {
                                    let value = levels[Int(cells[index])]
                                    let key = PetalNumeric.totalOrderKey(value)
                                    if lows < take || key < PetalNumeric.totalOrderKey(low[take - 1]) {
                                        var slot = lows < take ? lows : take - 1
                                        while slot > 0 && PetalNumeric.totalOrderKey(low[slot - 1]) > key {
                                            low[slot] = low[slot - 1]
                                            slot -= 1
                                        }
                                        low[slot] = value
                                        if lows < take { lows += 1 }
                                    }
                                    if highs < take {
                                        var slot = highs
                                        while slot > 0 && PetalNumeric.totalOrderKey(high[slot - 1]) > key {
                                            high[slot] = high[slot - 1]
                                            slot -= 1
                                        }
                                        high[slot] = value
                                        highs += 1
                                    } else if key > PetalNumeric.totalOrderKey(high[0]) {
                                        var slot = 0
                                        while slot + 1 < take && PetalNumeric.totalOrderKey(high[slot + 1]) < key {
                                            high[slot] = high[slot + 1]
                                            slot += 1
                                        }
                                        high[slot] = value
                                    }
                                }
                                var lowSum = -0.0
                                var highSum = -0.0
                                for index in 0..<take {
                                    lowSum += low[index]
                                    highSum += high[index]
                                }
                                let dark = lowSum / Double(take)
                                let lit = highSum / Double(take)
                                out[cell] = (levels[cell] - dark) / Double.maximum(lit - dark, 0.05)
                            }
                        }
                    }
                }
            }
            count = PetalLayout.fieldCells
        }
    }

    /// The field lanes read from normalised cell levels, skipping the sectors
    /// in `skip`, and how far the cells sit from their thresholds on average.
    /// Sectors the levels leave unread are read again from their
    /// ``localLevels(_:)``.
    static func readFields(_ levels: [Double], skip: [Bool]) -> (results: [PetalLaneResult?], separation: Double) {
        var (results, separation) = readSectors(levels, skip: skip)
        let unread = (0..<PetalLayout.sectors).map { !skip[$0] && results[$0] == nil }
        if unread.contains(true) {
            let retry = readSectors(localLevels(levels), skip: unread.map { !$0 }).results
            for sector in 0..<PetalLayout.sectors where results[sector] == nil {
                results[sector] = retry[sector]
            }
        }
        return (results, separation)
    }

    /// One read of the field lanes from cell levels with per-codeword,
    /// per-colour thresholds, and the mean distance of the cells read from
    /// their thresholds (a pose that lands on the petals separates lit from
    /// dark cells best).
    static func readSectors(_ levels: [Double], skip: [Bool]) -> (results: [PetalLaneResult?], separation: Double) {
        var separation = 0.0
        var counted = 0
        var results = noFields
        var scratch = WordScratch()
        for sector in 0..<PetalLayout.sectors where !skip[sector] {
            let (bytes, confidence) = readWord(levels, sector: sector, margins: &separation, scratch: &scratch)
            counted += fieldBits
            results[sector] = decodeWithErasures(.field(sector), bytes, confidence)
        }
        return (results, separation / Double(max(counted, 1)))
    }

    /// Reusable buffers of ``readWord(_:sector:margins:scratch:)``.
    struct WordScratch {
        var values = [Double](repeating: 0, count: fieldBits)
        var light: [Double] = []
        var pink: [Double] = []

        init() {
            light.reserveCapacity(fieldBits)
            pink.reserveCapacity(fieldBits)
        }
    }

    /// The transmitted word of one sector read from cell levels, and each
    /// byte's confidence (the smallest margin of its bits). Every bit's margin
    /// from its threshold is added to `margins`.
    static func readWord(
        _ levels: [Double],
        sector: Int,
        margins: inout Double,
        scratch: inout WordScratch
    ) -> (bytes: [UInt8], confidence: [Double]) {
        let cells = sectorBits[sector]
        let lightCell = sectorBitLight[sector]
        scratch.light.removeAll(keepingCapacity: true)
        scratch.pink.removeAll(keepingCapacity: true)
        for bit in 0..<fieldBits {
            let value = levels[cells[bit]]
            scratch.values[bit] = value
            if lightCell[bit] { scratch.light.append(value) } else { scratch.pink.append(value) }
        }
        let all = twoMeans(scratch.values)
        // the light (`天`) and pink petals are thresholded apart: their lit levels differ
        let lightThreshold = scratch.light.count < classMinimumCells ? all : twoMeans(scratch.light)
        let pinkThreshold = scratch.pink.count < classMinimumCells ? all : twoMeans(scratch.pink)
        var bytes = [UInt8](repeating: 0, count: PetalLane.field(sector).wordLength)
        var confidence = [Double](repeating: .greatestFiniteMagnitude, count: bytes.count)
        for bit in 0..<fieldBits {
            let value = scratch.values[bit]
            let threshold = lightCell[bit] ? lightThreshold : pinkThreshold
            if value > threshold { bytes[bit / 8] |= 1 << (7 - bit % 8) }
            let margin = abs(value - threshold)
            margins += margin
            confidence[bit / 8] = Double.minimum(confidence[bit / 8], margin)
        }
        return (bytes, confidence)
    }

    /// A read of the field under one pose.
    struct FieldRead {
        let pose: PetalPose
        let reference: Reference
        let levels: [Double]
        let fields: [PetalLaneResult?]
        let decoded: Int
        let separation: Double

        /// More lanes, then a clearer separation.
        func betterThan(_ other: FieldRead) -> Bool {
            decoded != other.decoded ? decoded > other.decoded : separation > other.separation
        }

        /// Reads the sectors this pose left unread again from their
        /// ``PetalDecoder/localLevels(_:)``.
        func finish() -> FieldRead {
            guard decoded < PetalLayout.sectors else { return self }
            let retry = readSectors(localLevels(levels), skip: fields.map { $0 != nil }).results
            var fields = self.fields
            for sector in 0..<PetalLayout.sectors where fields[sector] == nil {
                fields[sector] = retry[sector]
            }
            return FieldRead(
                pose: pose,
                reference: reference,
                levels: levels,
                fields: fields,
                decoded: fields.reduce(0) { $0 + ($1 == nil ? 0 : 1) },
                separation: separation
            )
        }
    }

    /// The field read under the pose with distortion `distortion` through the
    /// finder centres `destination` (no local read).
    static func readFieldAt(
        _ image: PetalLumaView,
        _ destination: [PetalPoint],
        distortion: Double,
        inferred: Int?
    ) -> FieldRead? {
        guard let pose = fitPose(image, destination, distortion: distortion),
              let reference = referenceLevels(image, pose, inferred: inferred) else { return nil }
        let levels = fieldLevels(image, pose, reference)
        let (fields, separation) = readSectors(levels, skip: readAll)
        return FieldRead(
            pose: pose,
            reference: reference,
            levels: levels,
            fields: fields,
            decoded: fields.reduce(0) { $0 + ($1 == nil ? 0 : 1) },
            separation: separation
        )
    }

    /// Reads the field through the finder centres `destination`, starting from
    /// lens distortion `start`, and searches the distortion while some field
    /// lane stays unread: `start` plus each of ``distortionSteps`` in turn, then
    /// the best ±``distortionRefine``. The pose that decodes the most lanes wins
    /// (ties: the clearer separation); the search stops once every lane
    /// decodes. Lanes the winning pose still leaves unread are read again from
    /// its ``localLevels(_:)``.
    static func readFieldSearching(
        _ image: PetalLumaView,
        _ destination: [PetalPoint],
        start: Double,
        inferred: Int?
    ) -> FieldRead? {
        guard var best = readFieldAt(image, destination, distortion: start, inferred: inferred) else { return nil }
        if best.decoded == PetalLayout.sectors { return best }
        for step in distortionSteps {
            if let read = readFieldAt(image, destination, distortion: start + step, inferred: inferred),
               read.betterThan(best) {
                best = read
                if best.decoded == PetalLayout.sectors { return best }
            }
        }
        let centre = best.pose.distortion
        for k in [centre - distortionRefine, centre + distortionRefine] {
            if let read = readFieldAt(image, destination, distortion: k, inferred: inferred), read.betterThan(best) {
                best = read
            }
        }
        return best.finish()
    }
}
