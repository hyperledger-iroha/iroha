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
    /// Frames in which the four finders were located, readable or not.
    public internal(set) var located: UInt32 = 0
    /// Frames in which at least one lane decoded.
    public internal(set) var readable: UInt32 = 0
    /// Lane `P` successes.
    public internal(set) var laneP: UInt32 = 0
    /// Lane `K` successes.
    public internal(set) var laneK: UInt32 = 0
    /// Lane `D` successes.
    public internal(set) var laneD: UInt32 = 0

    public init() {}
}

/// The result of offering one camera frame to a ``PetalScanSession``.
public struct PetalScanOutcome: Equatable, Sendable {
    /// Why the frame produced nothing, when it did not.
    public let error: PetalDecodeError?
    /// Lanes that decoded, as letters from `"PKD"`.
    public let lanes: String
    /// Receive progress after this frame.
    public let progress: PetalProgress
    /// The finished payload, delivered exactly once.
    public let completed: PetalCompletedPayload?
}

/// The receive-side object an app holds while its camera is open: decodes
/// camera frames and reassembles the stream they carry (port of
/// `crates/iroha_petal/src/session.rs`).
///
/// A half-received stream is forgotten after
/// ``PetalScanLimits/idleTimeoutMilliseconds`` without progress or
/// ``PetalScanLimits/absoluteTimeoutMilliseconds`` after it started.
public struct PetalScanSession: Sendable {
    /// The session limits.
    public let limits: PetalScanLimits
    private var assembler: PetalStreamAssembler
    /// Diagnostic counters.
    public private(set) var stats = PetalScanStats()
    private var startedMilliseconds: UInt64?
    private var progressMilliseconds: UInt64 = 0
    private var lastRank = 0

    /// Creates a session.
    public init(limits: PetalScanLimits = PetalScanLimits()) {
        self.limits = limits
        assembler = PetalStreamAssembler(limits: limits.assembler)
    }

    /// Current progress.
    public var progress: PetalProgress { assembler.progress }

    /// Drops all partial state.
    public mutating func reset() {
        assembler.reset()
        startedMilliseconds = nil
        lastRank = 0
    }

    /// Offers one camera luma plane captured at monotonic time `nowMilliseconds`.
    public mutating func push(_ image: PetalLuma, nowMilliseconds now: UInt64) -> PetalScanOutcome {
        if let start = startedMilliseconds,
           Self.elapsed(now, since: progressMilliseconds) > limits.idleTimeoutMilliseconds
            || Self.elapsed(now, since: start) > limits.absoluteTimeoutMilliseconds {
            reset()
        }
        stats.frames &+= 1
        let error: PetalDecodeError?
        let lanes: String
        switch PetalDecoder.decodeResult(image, options: limits.decode) {
        case .success(let frame):
            error = nil
            lanes = absorb(frame)
        case .failure(let failure):
            error = failure
            lanes = ""
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
            progress: progress,
            completed: assembler.takeCompleted()
        )
    }

    private static func elapsed(_ now: UInt64, since start: UInt64) -> UInt64 {
        now >= start ? now - start : 0
    }

    private mutating func absorb(_ frame: PetalDecodedFrame) -> String {
        if frame.p != nil { stats.laneP &+= 1 }
        if frame.k != nil { stats.laneK &+= 1 }
        if frame.d != nil { stats.laneD &+= 1 }
        let lanes = frame.laneLetters
        if !lanes.isEmpty { stats.readable &+= 1 }
        frame.feed(&assembler)
        return lanes
    }
}
