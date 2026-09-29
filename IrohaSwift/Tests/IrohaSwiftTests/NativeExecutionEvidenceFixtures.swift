import Foundation
@testable import IrohaSwift

/// Parity checks for a completed native Kagami export. Signature verification,
/// context membership and actual input/output proofs belong to independent Rust
/// replay against the retained launch plan; this decoder does not grant finality.
enum NativeExecutionEvidenceFixtures {
    static let schema = "iroha_kagami::scaling_evidence::ExportEnvelopeV1"
    static let maximum = 32 * 1024 * 1024
    static let maxItems = 4096
    struct Failure: Error {}
    struct Key: CodingKey {
        let stringValue: String
        var intValue: Int? { nil }
        init(stringValue: String) { self.stringValue = stringValue }
        init?(intValue: Int) { return nil }
    }
    struct Row: Decodable {
        let logicalID: String, phase: String, authority: String
        let sequence: UInt64, carrierHeight: UInt64, dataspaceID: UInt64
        let leafIndex: UInt32, laneID: UInt32
        let entrypointHash: Data, carrierHash: Data
        let laneSource: Source?
        init(from decoder: Decoder) throws {
            let c = try exact(decoder, ["logical_id", "phase", "sequence", "authority", "entrypoint_hash", "carrier_height", "carrier_hash", "lane_source", "leaf_index", "lane_id", "dataspace_id"])
            func text(_ key: String) throws -> String { try c.decode(String.self, forKey: Key(stringValue: key)) }
            func number(_ key: String) throws -> UInt64 { try c.decode(UInt64.self, forKey: Key(stringValue: key)) }
            logicalID = try text("logical_id"); phase = try text("phase"); authority = try text("authority")
            sequence = try number("sequence"); carrierHeight = try number("carrier_height"); dataspaceID = try number("dataspace_id")
            leafIndex = try c.decode(UInt32.self, forKey: Key(stringValue: "leaf_index"))
            laneID = try c.decode(UInt32.self, forKey: Key(stringValue: "lane_id"))
            entrypointHash = try hash(text("entrypoint_hash")); carrierHash = try hash(text("carrier_hash"))
            laneSource = try c.decodeIfPresent(Source.self, forKey: Key(stringValue: "lane_source"))
            guard (laneID == 0) == (laneSource == nil) else { throw Failure() }
            if let source = laneSource { guard source.anchorHeight < carrierHeight else { throw Failure() } }
            guard logicalID.count == 64, logicalID.utf8.allSatisfy(isHex),
                  phase == "warmup" || phase == "measurement", sequence > 0, carrierHeight > 0 else { throw Failure() }
            _ = try CanonicalNorito.encodeCompactAccountId(authority)
        }
        func encoded() throws -> Data {
            let id = Data(logicalID.utf8)
            let request = record([length(id.count) + id, integer(phase == "warmup" ? 0 : 1, 4),
                try CanonicalNorito.encodeCompactAccountId(authority), entrypointHash, integer(carrierHeight, 8),
                carrierHash, laneSource.map { Data([1]) + record([$0.encoded()]) } ?? Data([0]),
                integer(UInt64(leafIndex), 4), record([integer(UInt64(laneID), 4)]), record([integer(dataspaceID, 8)])])
            return record([integer(sequence, 8), request])
        }
    }
    struct Source: Decodable {
        let incarnation: Data, instance: Data, blockHash: Data, result: Data, anchorHash: Data
        let height: UInt64, anchorHeight: UInt64
        let batchIndex: UInt32
        init(from decoder: Decoder) throws {
            let c = try exact(decoder, ["incarnation", "instance", "height", "block_hash", "result", "batch_index", "anchor_height", "anchor_hash"])
            func raw(_ name: String) throws -> Data {
                let bytes = try hex(c.decode(String.self, forKey: Key(stringValue: name)), limit: 32)
                guard bytes.count == 32 else { throw Failure() }; return bytes
            }
            incarnation = try raw("incarnation"); instance = try raw("instance")
            blockHash = try raw("block_hash"); result = try raw("result")
            anchorHash = try hash(c.decode(String.self, forKey: Key(stringValue: "anchor_hash")))
            height = try c.decode(UInt64.self, forKey: Key(stringValue: "height"))
            anchorHeight = try c.decode(UInt64.self, forKey: Key(stringValue: "anchor_height"))
            batchIndex = try c.decode(UInt32.self, forKey: Key(stringValue: "batch_index"))
            guard height > 0, anchorHeight > 0 else { throw Failure() }
        }
        func encoded() -> Data {
            record([incarnation, instance, integer(height, 8), blockHash, result,
                integer(UInt64(batchIndex), 4), integer(anchorHeight, 8), anchorHash])
        }
    }
    private struct Document: Decodable {
        let artifact: Data, artifactHash: Data, rows: [Row]
        init(from decoder: Decoder) throws {
            let c = try exact(decoder, ["version", "artifact_schema", "artifact_hash", "canonical_artifact_hex", "requests"])
            guard try c.decode(UInt16.self, forKey: Key(stringValue: "version")) == 1,
                  try c.decode(String.self, forKey: Key(stringValue: "artifact_schema")) == schema else { throw Failure() }
            artifact = try hex(c.decode(String.self, forKey: Key(stringValue: "canonical_artifact_hex")), limit: maximum)
            let literal = try c.decode(String.self, forKey: Key(stringValue: "artifact_hash"))
            guard let normalized = ToriiCanonicalHashLiteral.normalizedHex(from: literal),
                  let canonical = ToriiCanonicalHashLiteral.literal(fromNormalizedHex: normalized), canonical == literal else { throw Failure() }
            artifactHash = try hash(normalized.lowercased())
            rows = try c.decode([Row].self, forKey: Key(stringValue: "requests"))
            guard !rows.isEmpty, rows.count <= maxItems else { throw Failure() }
        }
    }
    static func load(_ lanes: Int) throws -> Data {
        guard lanes == 1 || lanes == 4 else { throw Failure() }
        var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        while directory.path != "/" {
            let path = directory.appendingPathComponent("fixtures/sumeragi/native_execution_evidence_\(lanes)_lanes_v1.json")
            if FileManager.default.fileExists(atPath: path.path) {
                let size = try FileManager.default.attributesOfItem(atPath: path.path)[.size] as? NSNumber
                guard let size, size.uint64Value > 0, size.uint64Value <= UInt64(maximum) else { throw Failure() }
                return try Data(contentsOf: path)
            }
            directory.deleteLastPathComponent()
        }
        // No skip or handwritten evidence fallback when Rust capture is absent.
        throw CocoaError(.fileNoSuchFile)
    }
    static func inspect(_ bytes: Data) throws -> [Row] {
        guard !bytes.isEmpty, bytes.count <= maximum else { throw Failure() }
        try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(in: bytes, requireAllNumbersInteger: true)
        var quoted = false, escaped = false
        for byte in bytes {
            if quoted {
                if escaped { escaped = false } else if byte == 92 { escaped = true } else if byte == 34 { quoted = false }
            } else if byte == 34 { quoted = true } else if byte == 45 { throw Failure() }
        }
        let document = try JSONDecoder().decode(Document.self, from: bytes)
        guard IrohaHash.hash(document.artifact) == document.artifactHash else { throw Failure() }
        var r = try Reader(frame(document.artifact, schema: schema))
        guard try r.number(2) == 1 else { throw Failure() }
        var heightSequence = try Reader(r.field())
        let heights: [(UInt64, Data, Int)] = try heightSequence.sequence { bytes in
            var h = Reader(bytes)
            let height = try h.number(8)
            var carrierVector = try Reader(h.field())
            let carrier = try carrierVector.byteVector()
            guard height > 0, carrier.first == 1 else { throw Failure() }
            _ = try frame(Data(carrier.dropFirst()), schema: "iroha_data_model::block::model::SignedBlock")
            var evidenceVector = try Reader(h.field())
            var evidence = try Reader(frame(evidenceVector.byteVector(), schema: "iroha_kagami::scaling_evidence::LaneMergeEvidenceV1"))
            var state = try Reader(evidence.field())
            guard try state.number(8) == height else { throw Failure() }
            let carrierHash = try state.fixed(32)
            guard carrierHash[31] & 1 == 1 else { throw Failure() }
            // Rust replay authenticates complete lanes and ordered ordinary writes
            // against their native result roots, then verifies original lane frames.
            _ = try state.field()
            var writes = try Reader(state.field())
            _ = try writes.sequence { bytes in
                var write = Reader(bytes)
                var key = try Reader(write.field())
                var value = try Reader(write.field())
                _ = try key.byteVector(); _ = try value.byteVector()
                try write.finish()
            }
            var casting = try Reader(state.field())
            // These workload captures contain no Parliament casting bindings.
            guard try casting.sequence({ $0 }).isEmpty else { throw Failure() }
            try state.finish()
            var originals = try Reader(evidence.field())
            _ = try originals.sequence { bytes in
                var original = Reader(bytes)
                var lane = try Reader(original.field())
                guard try lane.number(4) > 0 else { throw Failure() }
                try lane.finish()
                var vector = try Reader(original.field())
                var frame = try Reader(vector.byteVector())
                _ = try frame.field(); _ = try frame.field(); try frame.finish(); try original.finish()
            }
            try evidence.finish()
            var queries = try Reader(h.field())
            let queryCount = try queries.sequence { bytes in
                var query = Reader(bytes)
                return try frame(query.byteVector(), schema: "iroha_data_model::query::model::CommittedTransaction")
            }.count
            try h.finish()
            return (height, carrierHash, queryCount)
        }
        guard !heights.isEmpty else { throw Failure() }
        for (a, b) in zip(heights, heights.dropFirst()) {
            guard a.0 != UInt64.max, b.0 == a.0 + 1 else { throw Failure() }
        }
        var rowSequence = try Reader(r.field())
        let encodedRows = try rowSequence.sequence { $0 }
        try r.finish()
        guard encodedRows.count == document.rows.count else { throw Failure() }
        var identities = Set<String>(), slots = Set<String>()
        var sequences: [String: UInt64] = ["warmup": 0, "measurement": 0]
        var measurement = false
        for (row, encoded) in zip(document.rows, encodedRows) {
            if row.phase == "measurement" { measurement = true } else if measurement { throw Failure() }
            let prior = sequences[row.phase]!
            guard prior != UInt64.max, row.sequence == prior + 1,
                  identities.insert(row.logicalID).inserted,
                  slots.insert("\(row.carrierHeight):\(row.leafIndex)").inserted,
                  let carrier = heights.first(where: { $0.0 == row.carrierHeight }),
                  UInt64(row.leafIndex) < UInt64(carrier.2), row.carrierHash == carrier.1,
                  try row.encoded() == encoded else { throw Failure() }
            sequences[row.phase] = row.sequence
        }
        guard sequences.values.allSatisfy({ $0 > 0 }) else { throw Failure() }
        return document.rows
    }
    static func frame(_ bytes: Data, schema: String) throws -> Data {
        guard bytes.count >= NoritoHeader.encodedLength, bytes.count <= maximum,
              bytes[22] == 0, bytes[39] == NoritoHeader.compactLen,
              let frame = noritoDecodeFrame(bytes), frame.paddingLength == 0,
              frame.header.schema == noritoSchemaHash(forTypeName: schema),
              frame.header.flags == NoritoHeader.compactLen, frame.header.compression == .none,
              noritoEncode(typeName: schema, payload: frame.payload, flags: NoritoHeader.compactLen, payloadAlignment: 8) == bytes else { throw Failure() }
        return frame.payload
    }
    private static func exact(_ decoder: Decoder, _ fields: Set<String>) throws -> KeyedDecodingContainer<Key> {
        let c = try decoder.container(keyedBy: Key.self)
        guard Set(c.allKeys.map(\.stringValue)) == fields else { throw Failure() }
        return c
    }
    private static func isHex(_ byte: UInt8) -> Bool { (48...57).contains(byte) || (97...102).contains(byte) }
    private static func hex(_ value: String, limit: Int) throws -> Data {
        guard !value.isEmpty, value.utf8.count <= limit * 2, value.utf8.count % 2 == 0,
              value.utf8.allSatisfy(isHex), let decoded = Data(hexString: value) else { throw Failure() }
        return decoded
    }
    private static func hash(_ value: String) throws -> Data {
        let bytes = try hex(value, limit: 32)
        guard bytes.count == 32, bytes[31] & 1 == 1 else { throw Failure() }
        return bytes
    }
    static func integer(_ value: UInt64, _ size: Int) -> Data {
        Data((0..<size).map { UInt8(truncatingIfNeeded: value >> ($0 * 8)) })
    }
    static func length(_ value: Int) -> Data {
        var n = value, out = Data()
        repeat { let byte = UInt8(n & 127); n >>= 7; out.append(byte | (n == 0 ? 0 : 128)) } while n != 0
        return out
    }
    static func record(_ fields: [Data]) -> Data {
        var out = Data()
        for field in fields { out.append(length(field.count)); out.append(field) }
        return out
    }
    struct Reader {
        private let bytes: Data
        private var cursor = 0
        init(_ bytes: Data) { self.bytes = Data(bytes) }
        private var remaining: Int { bytes.count - cursor }
        mutating func raw(_ size: Int) throws -> Data {
            guard size >= 0, size <= remaining else { throw Failure() }
            defer { cursor += size }; return Data(bytes[cursor..<(cursor + size)])
        }
        func finish() throws { guard remaining == 0 else { throw Failure() } }
        mutating func field() throws -> Data {
            var value = 0, shift = 0, count = 0
            while true {
                guard shift <= 28 else { throw Failure() }
                let byte = try raw(1)[0]
                guard shift < 28 || byte <= 7 else { throw Failure() }
                value |= Int(byte & 127) << shift; count += 1
                if byte & 128 == 0 { break }; shift += 7
            }
            guard value <= maximum, length(value).count == count else { throw Failure() }
            return try raw(value)
        }
        mutating func fixed(_ size: Int) throws -> Data {
            let bytes = try field(); guard bytes.count == size else { throw Failure() }; return bytes
        }
        mutating func number(_ size: Int) throws -> UInt64 { try Self.scalar(fixed(size)) }
        private static func scalar(_ bytes: Data) throws -> UInt64 {
            guard bytes.count <= 8 else { throw Failure() }
            return bytes.enumerated().reduce(UInt64(0)) { $0 | UInt64($1.element) << ($1.offset * 8) }
        }
        private mutating func count(_ maximum: Int) throws -> Int {
            let value = try Self.scalar(raw(8))
            guard value <= UInt64(maximum), value <= UInt64(remaining) else { throw Failure() }
            return Int(value)
        }
        mutating func byteVector() throws -> Data {
            let size = try count(maximum)
            let bytes = try raw(size); try finish(); return bytes
        }
        mutating func sequence<T>(_ decode: (Data) throws -> T) throws -> [T] {
            let n = try count(maxItems)
            var values = [T](); values.reserveCapacity(n)
            for _ in 0..<n { values.append(try decode(field())) }
            try finish(); return values
        }
    }
}
