import Foundation

/// Petal payload streams (port of `crates/iroha_petal/src/stream.rs`).
///
/// Every frame carries a handful of fountain atoms, one lane at a time: lane
/// `P` one atom, lane `K` five, and lane `D` one atom — except on every fourth
/// frame (`frame % 4 == 0`), when lane `D` carries the stream *beacon*
/// instead, so a receiver can join at any frame within a fraction of a
/// second. Atom ids run contiguously over the atoms actually sent (see
/// ``firstAtomID(frame:)``). Any single readable lane is useful on its own.
///
/// Lane data layouts (all big-endian):
///
/// * every lane starts with `tag:u8, frame:u16`; `tag` is the low byte of the
///   payload CRC-32C.
/// * lanes `P`, `K` and non-beacon `D`: atoms (16 bytes each).
/// * beacon `D`: `version:u8, kind:u8, len:u24, crc:u32`, zero padded.
public enum PetalStream {
    /// Version/profile byte of the beacon: format version 1, layout profile 0.
    public static let formatVersion: UInt8 = 0x10
    /// Largest payload a beacon can describe (`u24`).
    public static let maximumPayloadLength = (1 << 24) - 1
    /// Default receiver payload limit; override with ``PetalAssemblerLimits``.
    public static let defaultMaximumPayloadLength = 65_536
    /// A beacon replaces the lane-`D` atom on frames divisible by this interval.
    public static let beaconInterval: UInt16 = 4

    /// Whether `frame` carries the beacon in lane `D`.
    public static func isBeaconFrame(_ frame: UInt16) -> Bool {
        frame % beaconInterval == 0
    }

    /// Fountain atoms carried by `frame`.
    public static func atomsInFrame(_ frame: UInt16) -> Int {
        isBeaconFrame(frame)
            ? PetalLane.p.atomCount + PetalLane.k.atomCount
            : PetalLane.atomsPerFrame
    }

    /// Fountain id of the first atom of `frame`.
    ///
    /// Frame `f` follows `f` earlier frames, `ceil(f / 4)` of which were
    /// beacon frames with one atom fewer.
    public static func firstAtomID(frame: UInt16) -> UInt32 {
        let f = UInt32(frame)
        let interval = UInt32(beaconInterval)
        return f * UInt32(PetalLane.atomsPerFrame) - (f + interval - 1) / interval
    }

    static func laneFirstID(_ lane: PetalLane, frame: UInt16) -> UInt32 {
        let base = firstAtomID(frame: frame)
        switch lane {
        case .p:
            return base
        case .d:
            return base + UInt32(PetalLane.p.atomCount)
        case .k:
            let dAtoms = isBeaconFrame(frame) ? 0 : PetalLane.d.atomCount
            return base + UInt32(PetalLane.p.atomCount + dAtoms)
        }
    }

    static func parseHeader(_ data: [UInt8]) -> PetalLaneHeader {
        PetalLaneHeader(tag: data[0], frame: UInt16(data[1]) << 8 | UInt16(data[2]))
    }

    static func parseAtoms(
        _ lane: PetalLane,
        _ data: [UInt8],
        header: PetalLaneHeader,
        count: Int
    ) -> PetalAtomPacket {
        let atoms = (0..<count).map { index -> [UInt8] in
            let start = PetalLane.headerLength + index * PetalLane.atomLength
            return Array(data[start..<(start + PetalLane.atomLength)])
        }
        return PetalAtomPacket(
            header: header,
            firstID: laneFirstID(lane, frame: header.frame),
            atoms: atoms
        )
    }

    /// Parses the data bytes of lane `P` or lane `K`; `nil` for lane `D` or a
    /// length mismatch.
    public static func parseAtomLane(_ lane: PetalLane, data: [UInt8]) -> PetalAtomPacket? {
        guard lane != .d, data.count == lane.dataLength else { return nil }
        return parseAtoms(lane, data, header: parseHeader(data), count: lane.atomCount)
    }

    /// Parses the data bytes of lane `D`.
    ///
    /// Returns `nil` on a length mismatch, an unknown beacon version or a
    /// zero-length beacon.
    public static func parseDLane(_ data: [UInt8]) -> PetalDLane? {
        guard data.count == PetalLane.d.dataLength else { return nil }
        let header = parseHeader(data)
        guard isBeaconFrame(header.frame) else {
            return .atoms(parseAtoms(.d, data, header: header, count: PetalLane.d.atomCount))
        }
        let body = Array(data[PetalLane.headerLength...])
        guard body[0] == formatVersion else { return nil }
        let length = UInt32(body[2]) << 16 | UInt32(body[3]) << 8 | UInt32(body[4])
        guard length != 0 else { return nil }
        let crc = UInt32(body[5]) << 24 | UInt32(body[6]) << 16 | UInt32(body[7]) << 8 | UInt32(body[8])
        return .beacon(PetalBeacon(
            header: header,
            meta: PetalStreamMeta(kind: body[1], length: length, crc: crc)
        ))
    }
}

/// Errors raised by the Petal sender.
public enum PetalStreamError: Error, Equatable, LocalizedError, Sendable {
    /// The payload is empty.
    case emptyPayload
    /// The payload exceeds ``PetalStream/maximumPayloadLength``.
    case payloadTooLarge

    public var errorDescription: String? {
        switch self {
        case .emptyPayload: return "Petal stream payload is empty."
        case .payloadTooLarge: return "Petal stream payload exceeds the 24-bit length field."
        }
    }
}

/// Identity of a stream, as carried by every beacon.
public struct PetalStreamMeta: Equatable, Hashable, Sendable {
    /// Application payload kind.
    public let kind: UInt8
    /// Payload length in bytes.
    public let length: UInt32
    /// CRC-32C of the payload.
    public let crc: UInt32

    public init(kind: UInt8, length: UInt32, crc: UInt32) {
        self.kind = kind
        self.length = length
        self.crc = crc
    }

    /// The one-byte stream tag repeated in every lane header.
    public var tag: UInt8 { UInt8(truncatingIfNeeded: crc) }

    /// Number of fountain source atoms.
    public var sourceAtoms: Int {
        (Int(length) + PetalLane.atomLength - 1) / PetalLane.atomLength
    }
}

/// The common three-byte header of every lane.
public struct PetalLaneHeader: Equatable, Hashable, Sendable {
    /// Stream tag.
    public let tag: UInt8
    /// Frame counter (wraps at 65536).
    public let frame: UInt16

    public init(tag: UInt8, frame: UInt16) {
        self.tag = tag
        self.frame = frame
    }
}

/// A decoded beacon.
public struct PetalBeacon: Equatable, Hashable, Sendable {
    /// Lane header.
    public let header: PetalLaneHeader
    /// Stream identity.
    public let meta: PetalStreamMeta

    public init(header: PetalLaneHeader, meta: PetalStreamMeta) {
        self.header = header
        self.meta = meta
    }
}

/// Atoms read from one lane.
public struct PetalAtomPacket: Equatable, Sendable {
    /// Lane header.
    public let header: PetalLaneHeader
    /// Fountain id of the first atom; the rest follow consecutively.
    public let firstID: UInt32
    /// The 16-byte atoms.
    public let atoms: [[UInt8]]

    public init(header: PetalLaneHeader, firstID: UInt32, atoms: [[UInt8]]) {
        self.header = header
        self.firstID = firstID
        self.atoms = atoms
    }
}

/// What lane `D` carried.
public enum PetalDLane: Equatable, Sendable {
    /// The stream beacon.
    case beacon(PetalBeacon)
    /// A payload atom.
    case atoms(PetalAtomPacket)
}

/// Sender side: turns one payload into an endless sequence of frames.
public struct PetalStreamEncoder: Sendable {
    /// Stream identity.
    public let meta: PetalStreamMeta
    /// Zero-padded source atoms, concatenated.
    private let source: [UInt8]

    /// Prepares `payload` of application kind `kind` for streaming.
    ///
    /// - Throws: ``PetalStreamError`` for empty or oversized payloads.
    public init(payload: Data, kind: UInt8) throws {
        try self.init(payload: [UInt8](payload), kind: kind)
    }

    /// Prepares `payload` of application kind `kind` for streaming.
    ///
    /// - Throws: ``PetalStreamError`` for empty or oversized payloads.
    public init(payload: [UInt8], kind: UInt8) throws {
        guard !payload.isEmpty else { throw PetalStreamError.emptyPayload }
        guard payload.count <= PetalStream.maximumPayloadLength else {
            throw PetalStreamError.payloadTooLarge
        }
        meta = PetalStreamMeta(
            kind: kind,
            length: UInt32(payload.count),
            crc: IrohaPeerCRC32CV1.checksum(payload)
        )
        var padded = payload
        let atoms = meta.sourceAtoms
        padded.append(contentsOf: repeatElement(0, count: atoms * PetalLane.atomLength - payload.count))
        source = padded
    }

    /// Frames needed to send every source atom once (no losses, no repair).
    public var systematicFrames: Int {
        var frames = 0
        var atoms = 0
        while atoms < meta.sourceAtoms {
            atoms += PetalStream.atomsInFrame(UInt16(truncatingIfNeeded: frames))
            frames += 1
        }
        return frames
    }

    private func appendAtoms(firstID: UInt32, count: Int, to data: inout [UInt8]) {
        var mask = [UInt32](repeating: 0, count: PetalFountain.maskLength(sourceAtoms: meta.sourceAtoms))
        for index in 0..<count {
            let offset = data.count
            data.append(contentsOf: repeatElement(0, count: PetalLane.atomLength))
            PetalFountain.encodeAtom(
                flatSource: source,
                sourceAtoms: meta.sourceAtoms,
                crc: meta.crc,
                id: firstID + UInt32(index),
                mask: &mask,
                into: &data,
                at: offset
            )
        }
    }

    /// The data bytes of every lane of `frame`.
    public func laneData(frame: UInt16) -> (p: [UInt8], k: [UInt8], d: [UInt8]) {
        let header: [UInt8] = [
            meta.tag,
            UInt8(truncatingIfNeeded: frame >> 8),
            UInt8(truncatingIfNeeded: frame),
        ]
        var p = header
        appendAtoms(firstID: PetalStream.laneFirstID(.p, frame: frame), count: PetalLane.p.atomCount, to: &p)
        var k = header
        appendAtoms(firstID: PetalStream.laneFirstID(.k, frame: frame), count: PetalLane.k.atomCount, to: &k)
        var d = header
        if PetalStream.isBeaconFrame(frame) {
            d.append(PetalStream.formatVersion)
            d.append(meta.kind)
            d.append(UInt8(truncatingIfNeeded: meta.length >> 16))
            d.append(UInt8(truncatingIfNeeded: meta.length >> 8))
            d.append(UInt8(truncatingIfNeeded: meta.length))
            d.append(UInt8(truncatingIfNeeded: meta.crc >> 24))
            d.append(UInt8(truncatingIfNeeded: meta.crc >> 16))
            d.append(UInt8(truncatingIfNeeded: meta.crc >> 8))
            d.append(UInt8(truncatingIfNeeded: meta.crc))
            d.append(contentsOf: repeatElement(0, count: PetalLane.d.dataLength - d.count))
        } else {
            appendAtoms(firstID: PetalStream.laneFirstID(.d, frame: frame), count: PetalLane.d.atomCount, to: &d)
        }
        return (p, k, d)
    }

    /// The transmitted (Reed–Solomon encoded, whitened) codewords of `frame`.
    public func words(frame: UInt16) -> (p: [UInt8], k: [UInt8], d: [UInt8]) {
        let data = laneData(frame: frame)
        return (
            PetalLane.p.encodeValidated(data.p),
            PetalLane.k.encodeValidated(data.k),
            PetalLane.d.encodeValidated(data.d)
        )
    }

    /// Every cell of `frame`, ready to render.
    public func cells(frame: UInt16) -> PetalFrameCells {
        let words = words(frame: frame)
        return PetalFrameCells(validatedP: words.p, k: words.k, d: words.d)
    }
}

/// Receiver limits that bound memory and work.
public struct PetalAssemblerLimits: Equatable, Sendable {
    /// Largest payload the receiver accepts.
    public var maximumPayloadLength: Int
    /// Atoms buffered while waiting for the first beacon; zero disables the
    /// buffer.
    public var maximumPendingAtoms: Int

    public init(
        maximumPayloadLength: Int = PetalStream.defaultMaximumPayloadLength,
        maximumPendingAtoms: Int = 128
    ) {
        self.maximumPayloadLength = maximumPayloadLength
        self.maximumPendingAtoms = maximumPendingAtoms
    }
}

/// A reassembled, CRC-verified payload.
public struct PetalCompletedPayload: Equatable, Sendable {
    /// Stream identity.
    public let meta: PetalStreamMeta
    /// The payload bytes.
    public let payload: Data
}

/// Snapshot of receive progress for a UI.
public struct PetalProgress: Equatable, Sendable {
    /// Stream identity once a beacon was accepted.
    public var meta: PetalStreamMeta?
    /// Source atoms of the active stream.
    public var sourceAtoms: Int
    /// Independent atoms collected so far.
    public var rank: Int
    /// Atoms offered to the decoder (including duplicates).
    public var atomsReceived: UInt32
    /// Reassembled payloads that failed the CRC check and were discarded
    /// (cumulative over the assembler's lifetime, not cleared by `reset()`).
    public var integrityFailures: UInt32
    /// Whether the payload is complete and verified.
    public var complete: Bool

    public init(
        meta: PetalStreamMeta? = nil,
        sourceAtoms: Int = 0,
        rank: Int = 0,
        atomsReceived: UInt32 = 0,
        integrityFailures: UInt32 = 0,
        complete: Bool = false
    ) {
        self.meta = meta
        self.sourceAtoms = sourceAtoms
        self.rank = rank
        self.atomsReceived = atomsReceived
        self.integrityFailures = integrityFailures
        self.complete = complete
    }

    /// Fraction of the source atoms collected, in `0...1`.
    public var fractionComplete: Double {
        guard sourceAtoms > 0 else { return 0 }
        return min(1, Double(rank) / Double(sourceAtoms))
    }
}

/// Receiver side: collects atoms from any lane of any frame.
///
/// A beacon starts the active stream; atoms seen before the first beacon are
/// buffered (bounded by ``PetalAssemblerLimits/maximumPendingAtoms``). A
/// different stream replaces the active one only after two consecutive
/// sightings of its beacon. A reassembled payload is released only when its
/// CRC-32C matches the beacon; otherwise the elimination starts over and the
/// failure is counted. A completed stream is delivered exactly once; further
/// atoms of it are ignored.
public struct PetalStreamAssembler: Sendable {
    private struct Active: Sendable {
        var meta: PetalStreamMeta
        var decoder: PetalFountainDecoder
        var done: Bool
    }

    private struct PendingAtom: Sendable {
        let tag: UInt8
        let id: UInt32
        let atom: [UInt8]
    }

    private let limits: PetalAssemblerLimits
    private var active: Active?
    private var pending: [PendingAtom] = []
    private var conflicting: (meta: PetalStreamMeta, sightings: UInt8)?
    private var completed: PetalCompletedPayload?
    private var atomsReceived: UInt32 = 0
    private var integrityFailures: UInt32 = 0

    /// Creates an assembler.
    public init(limits: PetalAssemblerLimits = PetalAssemblerLimits()) {
        self.limits = limits
    }

    /// Forgets the active stream and any completed payload. The cumulative
    /// ``PetalProgress/integrityFailures`` count is kept.
    public mutating func reset() {
        active = nil
        pending.removeAll()
        conflicting = nil
        completed = nil
        atomsReceived = 0
    }

    /// Current progress.
    public var progress: PetalProgress {
        PetalProgress(
            meta: active?.meta,
            sourceAtoms: active?.decoder.sourceAtoms ?? 0,
            rank: active?.decoder.rank ?? 0,
            atomsReceived: atomsReceived,
            integrityFailures: integrityFailures,
            complete: active?.done ?? false
        )
    }

    /// Takes the completed payload, if any; it is delivered exactly once.
    public mutating func takeCompleted() -> PetalCompletedPayload? {
        defer { completed = nil }
        return completed
    }

    private mutating func start(_ meta: PetalStreamMeta) {
        active = Active(
            meta: meta,
            decoder: PetalFountainDecoder(validatedSourceAtoms: meta.sourceAtoms),
            done: false
        )
        conflicting = nil
        completed = nil
        atomsReceived = 0
        let buffered = pending
        pending.removeAll()
        for atom in buffered where atom.tag == meta.tag {
            add(id: atom.id, atom: atom.atom)
        }
    }

    /// Offers a beacon read from lane `D`.
    public mutating func push(beacon: PetalBeacon) {
        let meta = beacon.meta
        guard meta.length != 0, Int(meta.length) <= limits.maximumPayloadLength else { return }
        guard let current = active else {
            start(meta)
            return
        }
        if current.meta == meta {
            conflicting = nil
            return
        }
        // A different stream: switch only after two consecutive sightings.
        let seen: UInt8
        if let candidate = conflicting, candidate.meta == meta {
            seen = candidate.sightings &+ 1
        } else {
            seen = 1
        }
        if seen >= 2 {
            start(meta)
        } else {
            conflicting = (meta, seen)
        }
    }

    /// Offers atoms read from a lane.
    public mutating func push(atoms packet: PetalAtomPacket) {
        for (index, atom) in packet.atoms.enumerated() {
            let id = packet.firstID &+ UInt32(truncatingIfNeeded: index)
            if let current = active {
                if current.meta.tag == packet.header.tag { add(id: id, atom: atom) }
            } else {
                // A zero limit disables buffering before the first beacon.
                guard limits.maximumPendingAtoms > 0 else { continue }
                if pending.count >= limits.maximumPendingAtoms {
                    pending.removeFirst()
                }
                pending.append(PendingAtom(tag: packet.header.tag, id: id, atom: atom))
            }
        }
    }

    /// Offers whatever lane `D` carried.
    public mutating func push(dLane: PetalDLane) {
        switch dLane {
        case .beacon(let beacon): push(beacon: beacon)
        case .atoms(let packet): push(atoms: packet)
        }
    }

    private mutating func add(id: UInt32, atom: [UInt8]) {
        guard var current = active, !current.done else { return }
        // Release the stored copy so the decoder mutates without copying.
        active = nil
        defer { active = current }
        atomsReceived = atomsReceived == UInt32.max ? atomsReceived : atomsReceived + 1
        current.decoder.addEncoded(crc: current.meta.crc, id: id, atom: atom)
        guard current.decoder.isComplete,
              let payload = current.decoder.solvedPayload(length: Int(current.meta.length)) else {
            return
        }
        if IrohaPeerCRC32CV1.checksum(payload) == current.meta.crc {
            current.done = true
            completed = PetalCompletedPayload(meta: current.meta, payload: Data(payload))
        } else {
            // Corrupt atoms slipped through: start the elimination over.
            integrityFailures = integrityFailures == UInt32.max ? integrityFailures : integrityFailures + 1
            current.decoder = PetalFountainDecoder(validatedSourceAtoms: current.meta.sourceAtoms)
        }
    }
}
