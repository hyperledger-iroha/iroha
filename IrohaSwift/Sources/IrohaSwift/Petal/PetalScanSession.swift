import Foundation

/// Limits of a Petal scan session.
public struct PetalScanLimits: Equatable, Sendable {
    /// Forget a half-received stream after this long without progress.
    public var idleTimeoutMilliseconds: UInt64
    /// Forget a stream that has not finished this long after it started.
    public var absoluteTimeoutMilliseconds: UInt64
    /// Assembler memory and size limits.
    public var assembler: PetalAssemblerLimits
    /// Image decoder options.
    public var decode: PetalDecodeOptions

    public init(
        idleTimeoutMilliseconds: UInt64 = 30_000,
        absoluteTimeoutMilliseconds: UInt64 = 180_000,
        assembler: PetalAssemblerLimits = PetalAssemblerLimits(),
        decode: PetalDecodeOptions = PetalDecodeOptions()
    ) {
        self.idleTimeoutMilliseconds = idleTimeoutMilliseconds
        self.absoluteTimeoutMilliseconds = absoluteTimeoutMilliseconds
        self.assembler = assembler
        self.decode = decode
    }
}

/// Counters for diagnostics and UI hints.
public struct PetalScanStats: Equatable, Sendable {
    /// Camera frames offered.
    public internal(set) var frames: UInt32 = 0
    /// Frames in which a code was located, whether or not a lane could be read.
    public internal(set) var located: UInt32 = 0
    /// Frames in which at least one lane decoded.
    public internal(set) var readable: UInt32 = 0
    /// Lane `D` successes.
    public internal(set) var laneD: UInt32 = 0
    /// Field lane successes (up to four per frame).
    public internal(set) var fieldLanes: UInt32 = 0
    /// Frames read by tracking the previous pose instead of a full search.
    public internal(set) var tracked: UInt32 = 0
    /// Frames read with one corner finder hidden and inferred.
    public internal(set) var inferred: UInt32 = 0
    /// Merges of two or more captures of one shown frame.
    public internal(set) var merges: UInt32 = 0
    /// Field lanes recovered by merging captures.
    public internal(set) var mergedLanes: UInt32 = 0
    /// Turn lanes tried (each at most once per shown frame).
    public internal(set) var turnTries: UInt32 = 0
    /// Turn lanes read.
    public internal(set) var turnLanes: UInt32 = 0
    /// Bands tried (each at most once per shown frame).
    public internal(set) var kanaTries: UInt32 = 0
    /// Kana lanes read (up to four per shown frame).
    public internal(set) var kanaLanes: UInt32 = 0
    /// Captures whose bands read a lane that also tried the turn lanes, having
    /// time to spare.
    public internal(set) var spareTurns: UInt32 = 0

    public init() {}
}

/// A part of a capture's work, as a ``PetalScanSession`` times it to tell
/// whether a capture has time to spare for the turn lanes (see the session's
/// documentation).
public enum PetalWork: Equatable, Sendable {
    /// Finding the code by a full search and taking in the lanes read.
    case search
    /// Following the code from the previous capture's pose and taking in the
    /// lanes read.
    case track
    /// Merging the held captures of a shown frame.
    case merge
    /// Reading the kana lanes of this many bands.
    case bands(Int)
    /// Reading the turn lanes.
    case turns
}

/// The result of offering one camera frame to a ``PetalScanSession``.
public struct PetalScanOutcome: Equatable, Sendable {
    /// Why the frame produced nothing, when it did not.
    /// ``PetalDecodeError/noOrientation`` means a code was located but no lane
    /// could be read (too far, too blurry).
    public let error: PetalDecodeError?
    /// Lanes that decoded in this capture alone, as letters: `D` and the
    /// sector digits `0`–`3`.
    public let lanes: String
    /// Field lanes recovered by merging this capture with earlier captures of
    /// its shown frame, as sector digits.
    public let merged: String
    /// Turn lanes read from this capture, as the sector digits of the field
    /// lanes they ride on.
    public let turned: String
    /// Kana lanes read from this capture, as their names (`P0`, `K0`, `P1`,
    /// `K1`).
    public let kana: String
    /// Receive progress after this frame.
    public let progress: PetalProgress
    /// The finished payload, delivered exactly once.
    public let completed: PetalCompletedPayload?
}

/// The receive-side object an app holds while its camera is open: decodes
/// camera frames and reassembles the stream they carry (port of
/// `crates/iroha_petal/src/session.rs`).
///
/// After a frame decodes, the next frames are first read by tracking the code
/// from its last pose (``PetalDecoder/track(_:previous:options:)``), which
/// skips the finder search; a full ``PetalDecoder/decode(_:options:)`` runs
/// when tracking fails or the last pose is older than
/// ``trackWindowMilliseconds``. A half-received stream is forgotten after
/// ``PetalScanLimits/idleTimeoutMilliseconds`` without progress or
/// ``PetalScanLimits/absoluteTimeoutMilliseconds`` after it started.
///
/// A camera sees each shown frame two to four times, and noise, display
/// banding or hand shake can cost every one of those captures a field lane.
/// When a capture reads some lane but not every field lane, the session keeps
/// its ``PetalFieldEvidence`` under the shown frame its decoded lane headers
/// name, up to ``mergeCaptures`` captures of ``mergeWindow`` shown frames.
/// From the second such capture on, it merges them
/// (``PetalDecoder/mergeFields(_:skip:)``) and offers a recovered lane only
/// when that lane's own header names the same frame. Unkeyed and torn
/// captures are never combined.
///
/// Every field lane a capture decodes also lets the session read the turn
/// lane on top of it (``PetalTurns/readTurns(_:frame:skip:)``). A turn lane is
/// tried once per shown frame: the session remembers the last ``turnMemory``
/// it tried and does not look at those petals again. Reading turns costs a
/// slow phone core several times a frame's decode, so after a try that reads
/// no turn lane (a camera too soft or too shaky for the turns) the session
/// passes over the next captures that would try one, one more after every
/// such try, at most ``turnBackoff``, until a try reads a lane again.
///
/// The katakana bands above and below the square are read the same way
/// (``PetalKana/readKana(_:frame:header:skip:)``): each band once per shown
/// frame (the last ``kanaMemory`` tried are remembered), with its own back-off
/// of at most ``kanaBackoff`` captures after tries that read no kana lane (a
/// camera that does not see the bands, or too blurred for their glyphs and
/// polarity).
///
/// The bands go first. A capture whose bands read a kana lane tries the turn
/// lanes too only when it has time to spare: when the work it has done
/// (finding or following the code, a merge, the bands) and what the session's
/// last turn try took fit together in ``captureBudgetMilliseconds``, a 30 fps
/// camera's frame time (before the first turn try, twice the work done stands
/// in for it). Otherwise the turn lanes of that shown frame wait for its next
/// capture. On a slow phone core a turn try costs five tracked decodes and a
/// band try one, so reading both on one capture overran the time a frame is
/// shown at 12 fps and the session missed the next shown frames; a fast
/// decoder reads both at once, which matters when its camera gives it few
/// frames a second. A band try that reads nothing (bands out of view, too
/// blurred, or a torn capture) leaves the turn lanes to the same capture.
///
/// The session times its work on the monotonic clock; the `workCost` given
/// to ``init(limits:workCost:)`` puts a model's costs in place of the measured
/// times, as tests do.
public struct PetalScanSession: Sendable {
    /// How long a decoded pose stays usable for tracking the next frames.
    public static let trackWindowMilliseconds: UInt64 = 500
    /// Shown frames whose captures are held for merging.
    public static let mergeWindow = 6
    /// Captures held per shown frame; a further capture replaces the oldest.
    public static let mergeCaptures = 4
    /// Turn lanes remembered as tried (the four of each of the last
    /// ``mergeWindow`` shown frames).
    public static let turnMemory = mergeWindow * PetalLayout.sectors
    /// Most captures passed over after tries that read no turn lane.
    public static let turnBackoff = 8
    /// Bands remembered as tried (the two of each of the last ``mergeWindow``
    /// shown frames).
    public static let kanaMemory = mergeWindow * PetalLayout.kanaBands
    /// Most captures passed over after tries that read no kana lane.
    public static let kanaBackoff = 8
    /// The time a capture's work may take, turn lanes included, for a capture
    /// whose bands read a lane to try the turn lanes too: a 30 fps camera's
    /// frame time, so a decoder that keeps to it misses no camera frame.
    public static let captureBudgetMilliseconds = 1000.0 / 30.0

    /// The session limits.
    public let limits: PetalScanLimits
    private var assembler: PetalStreamAssembler
    /// Diagnostic counters.
    public private(set) var stats = PetalScanStats()
    private var startedMilliseconds: UInt64?
    private var progressMilliseconds: UInt64 = 0
    private var lastRank = 0
    /// The last frame that decoded and when it was captured.
    private var lastPose: (frame: PetalDecodedFrame, at: UInt64)?
    /// The captures of one shown frame held for merging.
    struct Held: Sendable {
        let shown: PetalLaneHeader
        var evidence: [PetalFieldEvidence] = []
        var done = [Bool](repeating: false, count: PetalLayout.sectors)
    }
    private(set) var held: [Held] = []
    /// A turn lane tried: the header of the field lane it rides on, and its sector.
    struct TurnTry: Equatable, Sendable {
        let header: PetalLaneHeader
        let sector: Int
    }
    /// The turn lanes tried most recently, oldest first.
    private(set) var tried: [TurnTry] = []
    /// Tries in a row that read no turn lane (at most ``turnBackoff``).
    private(set) var turnMisses = 0
    /// Captures still to pass over before the next try.
    var turnWait = 0
    /// A band tried: the shown frame and the band.
    struct KanaTry: Equatable, Sendable {
        let header: PetalLaneHeader
        let band: Int
    }
    /// The bands tried most recently, oldest first.
    private(set) var kanaTried: [KanaTry] = []
    /// Band tries in a row that read no kana lane (at most ``kanaBackoff``).
    private(set) var kanaMisses = 0
    /// Captures still to pass over before the next band try.
    var kanaWait = 0
    /// What a part of a capture costs, given the milliseconds it took.
    private let workCost: @Sendable (PetalWork, Double) -> Double
    /// What the last turn try cost, in milliseconds.
    private var turnCostMilliseconds: Double?

    /// Creates a session whose work costs what `workCost` says, given the part
    /// of a capture and the milliseconds it took: by default the measured time.
    public init(
        limits: PetalScanLimits = PetalScanLimits(),
        workCost: @escaping @Sendable (PetalWork, Double) -> Double = { _, measured in measured }
    ) {
        self.limits = limits
        self.workCost = workCost
        assembler = PetalStreamAssembler(limits: limits.assembler)
    }

    /// What `work` cost, given that it started at `started` (uptime nanoseconds).
    private func cost(_ work: PetalWork, since started: UInt64) -> Double {
        workCost(work, Double(DispatchTime.now().uptimeNanoseconds &- started) / 1e6)
    }

    /// Current progress.
    public var progress: PetalProgress { assembler.progress }

    /// Drops all partial state.
    public mutating func reset() {
        assembler.reset()
        startedMilliseconds = nil
        lastRank = 0
        lastPose = nil
        held.removeAll()
        forgetTurns()
    }

    /// Forgets the turn lanes and bands tried and any back-off.
    private mutating func forgetTurns() {
        tried.removeAll()
        turnMisses = 0
        turnWait = 0
        kanaTried.removeAll()
        kanaMisses = 0
        kanaWait = 0
    }

    /// Offers one camera luma plane captured at monotonic time `nowMilliseconds`.
    public mutating func push(_ image: PetalLuma, nowMilliseconds now: UInt64) -> PetalScanOutcome {
        if let start = startedMilliseconds,
           Self.elapsed(now, since: progressMilliseconds) > limits.idleTimeoutMilliseconds
            || Self.elapsed(now, since: start) > limits.absoluteTimeoutMilliseconds {
            reset()
        }
        stats.frames &+= 1
        let pushStarted = DispatchTime.now().uptimeNanoseconds
        var tracked: PetalDecodedFrame?
        if let pose = lastPose, Self.elapsed(now, since: pose.at) <= Self.trackWindowMilliseconds {
            tracked = PetalDecoder.track(image, previous: pose.frame, options: limits.decode)
        }
        if tracked != nil { stats.tracked &+= 1 }
        let result = tracked.map { Result.success($0) } ?? PetalDecoder.decodeResult(image, options: limits.decode)
        let error: PetalDecodeError?
        let lanes: String
        let merged: String
        let turned: String
        let kana: String
        switch result {
        case .success(let frame):
            error = nil
            if frame.inferredCorner != nil { stats.inferred &+= 1 }
            lastPose = (frame, now)
            lanes = absorb(frame)
            var spent = cost(tracked != nil ? .track : .search, since: pushStarted)
            let mergesBefore = stats.merges
            var partStarted = DispatchTime.now().uptimeNanoseconds
            merged = merge(image, frame: frame)
            if stats.merges != mergesBefore { spent += cost(.merge, since: partStarted) }
            let bandsBefore = stats.kanaTries
            partStarted = DispatchTime.now().uptimeNanoseconds
            kana = self.kana(image, frame: frame)
            let bands = Int(stats.kanaTries &- bandsBefore)
            if bands > 0 { spent += cost(.bands(bands), since: partStarted) }
            // a capture whose bands read a lane tries the turn lanes only with
            // time to spare (see the type's documentation)
            if kana.isEmpty {
                turned = turn(image, frame: frame)
            } else if spent + (turnCostMilliseconds ?? 2 * spent) <= Self.captureBudgetMilliseconds {
                let triesBefore = stats.turnTries
                turned = turn(image, frame: frame)
                if stats.turnTries != triesBefore { stats.spareTurns &+= 1 }
            } else {
                turned = ""
            }
        case .failure(let failure):
            error = failure
            lanes = ""
            merged = ""
            turned = ""
            kana = ""
        }
        // A code was seen unless no finders were found or the image was unusable.
        if error != .noFinders && error != .unsupportedImage { stats.located &+= 1 }
        let progress = assembler.progress
        if progress.rank > lastRank || (progress.meta != nil && startedMilliseconds == nil) {
            progressMilliseconds = now
            if startedMilliseconds == nil { startedMilliseconds = now }
        }
        lastRank = progress.rank
        return PetalScanOutcome(
            error: error,
            lanes: lanes,
            merged: merged,
            turned: turned,
            kana: kana,
            progress: progress,
            completed: assembler.takeCompleted()
        )
    }

    private static func elapsed(_ now: UInt64, since start: UInt64) -> UInt64 {
        now >= start ? now - start : 0
    }

    mutating func absorb(_ frame: PetalDecodedFrame) -> String {
        let lanes = frame.laneLetters
        if frame.d != nil { stats.laneD &+= 1 }
        stats.fieldLanes &+= UInt32(frame.fields.reduce(0) { $0 + ($1 == nil ? 0 : 1) })
        if !lanes.isEmpty { stats.readable &+= 1 }
        let previousMeta = assembler.progress.meta
        frame.feed(&assembler)
        // The tag is only eight bits: two streams can share a tag and frame
        // counter. Once the assembler accepts a different full stream identity,
        // none of the previous stream's evidence, completed-lane flags or turn lanes apply.
        if let previousMeta, assembler.progress.meta != previousMeta {
            held.removeAll()
            forgetTurns()
        }
        return lanes
    }

    /// The header of lane data; `nil` when it is too short to hold one.
    static func laneHeader(_ data: [UInt8]) -> PetalLaneHeader? {
        guard data.count >= PetalLane.headerLength else { return nil }
        return PetalLaneHeader(tag: data[0], frame: UInt16(data[1]) << 8 | UInt16(data[2]))
    }

    /// Tries the turn lanes on top of the field lanes this capture decoded,
    /// except those already tried for their shown frame, and offers the atoms
    /// of those it reads; after tries that read nothing it passes over the next
    /// captures that would try one (see the type's documentation).
    mutating func turn(_ image: PetalLuma, frame: PetalDecodedFrame) -> String {
        var skip = [Bool](repeating: true, count: PetalLayout.sectors)
        var headers = [PetalLaneHeader?](repeating: nil, count: PetalLayout.sectors)
        for sector in 0..<PetalLayout.sectors {
            guard let field = frame.lane(.field(sector)), let header = Self.laneHeader(field.data) else { continue }
            headers[sector] = header
            skip[sector] = tried.contains(TurnTry(header: header, sector: sector))
        }
        if !skip.contains(false) { return "" }
        if turnWait > 0 {
            turnWait -= 1
            return ""
        }
        for sector in 0..<PetalLayout.sectors where !skip[sector] {
            guard let header = headers[sector] else { continue }
            if tried.count == Self.turnMemory { tried.removeFirst() }
            tried.append(TurnTry(header: header, sector: sector))
            stats.turnTries &+= 1
        }
        let readStarted = DispatchTime.now().uptimeNanoseconds
        let results = PetalTurns.readTurns(image, frame: frame, skip: skip)
        turnCostMilliseconds = cost(.turns, since: readStarted)
        var letters = ""
        for (sector, result) in results.enumerated() {
            guard let result, let header = headers[sector],
                  let packet = PetalTurns.turnPacket(header, sector: sector, data: result.data) else { continue }
            assembler.push(atoms: packet)
            stats.turnLanes &+= 1
            letters.append(PetalLane.field(sector).letter)
        }
        if letters.isEmpty {
            turnMisses = min(turnMisses + 1, Self.turnBackoff)
            turnWait = turnMisses
        } else {
            turnMisses = 0
        }
        return letters
    }

    /// Tries the bands of this capture's shown frame not tried yet and offers
    /// the atoms of the kana lanes it reads; after tries that read nothing it
    /// passes over the next captures that would try a band (see the type's
    /// documentation).
    mutating func kana(_ image: PetalLuma, frame: PetalDecodedFrame) -> String {
        guard let header = Self.shownFrame(frame) else { return "" }
        let skip = (0..<PetalLayout.kanaBands).map { kanaTried.contains(KanaTry(header: header, band: $0)) }
        if !skip.contains(false) { return "" }
        if kanaWait > 0 {
            kanaWait -= 1
            return ""
        }
        for band in 0..<PetalLayout.kanaBands where !skip[band] {
            if kanaTried.count == Self.kanaMemory { kanaTried.removeFirst() }
            kanaTried.append(KanaTry(header: header, band: band))
            stats.kanaTries &+= 1
        }
        var read = ""
        for (index, result) in PetalKana.readKana(image, frame: frame, header: header, skip: skip).enumerated() {
            guard let result, let lane = PetalKanaLane(rawValue: index),
                  let packet = PetalKana.kanaPacket(result.header, lane: lane, data: result.result.data) else { continue }
            assembler.push(atoms: packet)
            stats.kanaLanes &+= 1
            read += lane.name
        }
        if read.isEmpty {
            kanaMisses = min(kanaMisses + 1, Self.kanaBackoff)
            kanaWait = kanaMisses
        } else {
            kanaMisses = 0
        }
        return read
    }

    /// The shown frame every decoded lane header of `frame` names; `nil` when
    /// no lane decoded or the lanes of a torn capture name different frames.
    static func shownFrame(_ frame: PetalDecodedFrame) -> PetalLaneHeader? {
        var shown: PetalLaneHeader?
        for lane in PetalLane.decodeOrder {
            guard let result = frame.lane(lane) else { continue }
            guard let header = laneHeader(result.data) else { return nil }
            if let shown, shown != header { return nil }
            shown = header
        }
        return shown
    }

    /// Holds this capture's field evidence under its shown frame and, from the
    /// second capture on, merges the held captures of that frame. Recovered
    /// atoms must name exactly that frame.
    mutating func merge(_ image: PetalLuma, frame: PetalDecodedFrame) -> String {
        guard let shown = Self.shownFrame(frame) else { return "" }
        let index: Int
        if let existing = held.firstIndex(where: { $0.shown == shown }) {
            index = existing
        } else {
            if held.count == Self.mergeWindow { held.removeFirst() }
            held.append(Held(shown: shown))
            index = held.count - 1
        }
        for sector in 0..<PetalLayout.sectors where frame.lane(.field(sector)) != nil {
            held[index].done[sector] = true
        }
        if !held[index].done.contains(false) {
            held[index].evidence.removeAll()
            return ""
        }
        guard let evidence = PetalDecoder.fieldEvidence(image, frame: frame, options: limits.decode) else {
            return ""
        }
        if held[index].evidence.count == Self.mergeCaptures { held[index].evidence.removeFirst() }
        held[index].evidence.append(evidence)
        if held[index].evidence.count < 2 { return "" }
        stats.merges &+= 1
        let results = PetalDecoder.mergeFields(held[index].evidence, skip: held[index].done)
        var merged = ""
        for (sector, result) in results.enumerated() {
            let lane = PetalLane.field(sector)
            guard let result, let packet = PetalStream.parseAtomLane(lane, data: result.data) else { continue }
            // a merge offers only atoms of the shown frame it was made for
            if packet.header != shown { continue }
            held[index].done[sector] = true
            assembler.push(atoms: packet)
            stats.mergedLanes &+= 1
            merged.append(lane.letter)
        }
        if !held[index].done.contains(false) { held[index].evidence.removeAll() }
        return merged
    }
}
