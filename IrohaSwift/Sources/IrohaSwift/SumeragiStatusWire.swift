import Foundation

/// Canonical native status Norito frame. There is no bare-payload or retired-status decoder.
public enum SumeragiStatusWire {
    private static let schema = "iroha_data_model::sumeragi::SumeragiStatus"
    private static let limit = 1_048_576
    public static func encode(_ value: ToriiSumeragiStatusSnapshot) throws -> Data {
        let payload = try record([
            integer(UInt64(value.protocolVersion), 2),
            hashBytes(value.configFingerprint),
            option(try value.beaconHorizon.map(encodeHorizon)),
            Data(hexString: value.instance)!,
            integer(value.height, 8),
            integer(value.view, 8),
            integer(UInt64(value.stage), 1),
            option(try value.leader.map(encodeKey)),
            option(try value.proxyTail.map(encodeKey)),
            option(value.highQcView.map { integer($0, 8) }),
            integer(UInt64(value.level), 4),
            integer(UInt64(value.startLevel), 4),
            integer(value.tRetxMs, 8),
            integer(value.committedHeight, 8),
            integer(value.appliedHeight, 8),
            boolean(value.awaiting),
            option(try value.signer.map(encodeKey)),
            boolean(value.unanchored),
            boolean(value.abstaining),
            option(try value.halted.map(encodeHalt)),
            encodeFootprint(value.footprint)
        ])
        guard payload.count <= limit - NoritoHeader.encodedLength else { throw failure() }
        return noritoEncode(typeName: schema, payload: payload, flags: NoritoHeader.compactLen, payloadAlignment: 8)
    }
    public static func decodeCanonical(_ bytes: Data) throws -> ToriiSumeragiStatusSnapshot {
        guard bytes.count <= limit, let frame = noritoDecodeFrame(bytes),
              frame.header.schema == noritoSchemaHash(forTypeName: schema),
              frame.header.flags == NoritoHeader.compactLen, frame.header.compression == .none, frame.paddingLength == 0 else { throw failure() }
        var r = Reader(frame.payload)
        let object: [String: Any] = [
            "protocol_version": try r.number(2),
            "config_fingerprint": try hashLiteral(r.bytes(32)),
            "beacon_horizon": try r.optional(decodeHorizon),
            "instance": try r.bytes(32).hexLowercased(),
            "height": try r.number(8),
            "view": try r.number(8),
            "stage": try r.number(1),
            "leader": try r.optional(decodeKey),
            "proxy_tail": try r.optional(decodeKey),
            "high_qc_view": try r.optional { try wholeNumber($0, 8) },
            "level": try r.number(4),
            "start_level": try r.number(4),
            "t_retx_ms": try r.number(8),
            "committed_height": try r.number(8),
            "applied_height": try r.number(8),
            "awaiting": try r.boolean(),
            "signer": try r.optional(decodeKey),
            "unanchored": try r.boolean(),
            "abstaining": try r.boolean(),
            "halted": try r.optional(decodeHalt),
            "footprint": try decodeFootprint(r.field())
        ]
        try r.finish()
        let value = try ToriiSumeragiStatusSnapshot.parseJSON(JSONSerialization.data(withJSONObject: object))
        guard try encode(value) == bytes else { throw failure() }
        return value
    }
    private static func encodeFootprint(_ v: ToriiSumeragiFootprint) -> Data {
        record([integer(v.votes, 8), integer(v.timeouts, 8), integer(v.blocks, 8), integer(v.execEntries, 8), integer(v.wants, 8), integer(v.pendingApply, 8), integer(v.syncEntries, 8), integer(v.syncBytes, 8), integer(v.peers, 8), integer(v.recentHeaders, 8), integer(v.configs, 8), integer(v.certCache, 8), integer(v.evidenceKeys, 8), integer(v.probe, 8)])
    }
    private static func decodeFootprint(_ bytes: Data) throws -> [String: Any] {
        var r = Reader(bytes)
        let object: [String: Any] = ["votes": try r.number(8), "timeouts": try r.number(8), "blocks": try r.number(8), "exec_entries": try r.number(8), "wants": try r.number(8), "pending_apply": try r.number(8), "sync_entries": try r.number(8), "sync_bytes": try r.number(8), "peers": try r.number(8), "recent_headers": try r.number(8), "configs": try r.number(8), "cert_cache": try r.number(8), "evidence_keys": try r.number(8), "probe": try r.number(8)]
        try r.finish(); return object
    }
    private static func encodeHorizon(_ v: ToriiSumeragiBeaconHorizon) throws -> Data {
        record([integer(v.epochLengthBlocks, 8), option(v.nextRequiredPulseHeight.map { integer($0, 8) }),
                option(v.activeSessionId.map { Data(hexString: $0)! }), boolean(v.sessionCoversNextPulse), boolean(v.localProviderReady)])
    }
    private static func decodeHorizon(_ bytes: Data) throws -> [String: Any] {
        var r = Reader(bytes)
        let object: [String: Any] = ["epoch_length_blocks": try r.number(8),
            "next_required_pulse_height": try r.optional { try wholeNumber($0, 8) },
            "active_session_id": try r.optional { guard $0.count == 32 else { throw failure() }; return $0.hexUppercased() },
            "session_covers_next_pulse": try r.boolean(), "local_provider_ready": try r.boolean()]
        try r.finish(); return object
    }
    private static func encodeHalt(_ v: ToriiSumeragiHaltReason) throws -> Data {
        switch v {
        case .safetyRecordCorrupt: return integer(0, 4)
        case .safetyRecordInconsistent: return integer(1, 4)
        case .safetyViolation(let h): return integer(2, 4) + record([integer(h, 8)])
        case .applyDiverged(let h): return integer(3, 4) + record([integer(h, 8)])
        case .publicationRecoveryRequired(let h): return integer(4, 4) + record([integer(h, 8)])
        case .driverAnomaly: return integer(5, 4)
        }
    }
    private static func decodeHalt(_ bytes: Data) throws -> [String: Any] {
        var r = Reader(bytes)
        let tag = try wholeNumber(r.raw(4), 4)
        let names = ["safety_record_corrupt", "safety_record_inconsistent", "safety_violation", "apply_diverged", "publication_recovery_required", "driver_anomaly"]
        guard tag < UInt64(names.count) else { throw failure() }
        let details: Any = (2...4).contains(tag) ? try r.number(8) : NSNull()
        try r.finish(); return ["reason": names[Int(tag)], "details": details]
    }
    private static func encodeKey(_ literal: String) throws -> Data {
        let key = try governanceKagemushaPublicKeyOrderV1(literal, codingPath: [])
        let bytes = [key.algorithm] + key.payload
        return integer(UInt64(bytes.count), 8) + record(bytes.map { Data([$0]) })
    }
    private static func decodeKey(_ bytes: Data) throws -> String {
        var r = Reader(bytes)
        let count = try wholeNumber(r.raw(8), 8)
        guard count >= 2, count <= 65_536, count <= UInt64(r.remaining / 2) else { throw failure() }
        var payload = Data()
        for _ in 0..<count { payload.append(try r.bytes(1)) }
        try r.finish()
        guard let algorithm = SigningAlgorithm(noritoDiscriminant: payload[0]) else { throw failure() }
        return CanonicalNorito.publicKeyMultihash(algorithm: algorithm, payload: Data(payload.dropFirst()))
    }
    private static func hashBytes(_ literal: String) throws -> Data {
        guard let hex = ToriiCanonicalHashLiteral.normalizedHex(from: literal), let data = Data(hexString: hex),
              data.count == 32, data[31] & 1 == 1 else { throw failure() }
        return data
    }
    private static func hashLiteral(_ bytes: Data) throws -> String {
        guard bytes.count == 32, bytes[31] & 1 == 1,
              let value = ToriiCanonicalHashLiteral.literal(fromNormalizedHex: bytes.hexLowercased()) else { throw failure() }
        return value
    }
    private static func failure() -> DecodingError {
        .dataCorrupted(.init(codingPath: [], debugDescription: "non-canonical native status frame"))
    }
    private static func integer(_ v: UInt64, _ size: Int) -> Data {
        Data((0..<size).map { UInt8(truncatingIfNeeded: v >> ($0 * 8)) })
    }
    private static func wholeNumber(_ bytes: Data, _ size: Int) throws -> UInt64 {
        guard bytes.count == size, size <= 8 else { throw failure() }
        return bytes.enumerated().reduce(UInt64(0)) { $0 | UInt64($1.element) << ($1.offset * 8) }
    }
    private static func boolean(_ v: Bool) -> Data { Data([v ? 1 : 0]) }
    private static func length(_ size: Int) -> Data {
        var n = size; var bytes = Data()
        repeat { let b = UInt8(n & 127); n >>= 7; bytes.append(b | (n == 0 ? 0 : 128)) } while n != 0
        return bytes
    }
    private static func record(_ fields: [Data]) -> Data {
        var out = Data()
        for field in fields { out.append(length(field.count)); out.append(field) }
        return out
    }
    private static func option(_ bytes: Data?) -> Data {
        bytes.map { Data([1]) + record([$0]) } ?? Data([0])
    }
    private struct Reader {
        private let data: Data
        private var offset = 0
        init(_ data: Data) { self.data = Data(data) }
        var remaining: Int { data.count - offset }
        mutating func raw(_ count: Int) throws -> Data {
            guard count >= 0, count <= remaining else { throw failure() }
            defer { offset += count }; return Data(data[offset..<(offset + count)])
        }
        func finish() throws { guard remaining == 0 else { throw failure() } }
        mutating func field() throws -> Data {
            var size = 0; var shift = 0; var count = 0
            while true {
                let byte = try raw(1)[0]
                guard shift < 21 else { throw failure() }
                size |= Int(byte & 127) << shift; count += 1
                if byte & 128 == 0 { break }; shift += 7
            }
            guard length(size).count == count, size <= limit else { throw failure() }
            return try raw(size)
        }
        mutating func bytes(_ count: Int) throws -> Data {
            let bytes = try field(); guard bytes.count == count else { throw failure() }; return bytes
        }
        mutating func number(_ count: Int) throws -> UInt64 { try wholeNumber(bytes(count), count) }
        mutating func boolean() throws -> Bool {
            let byte = try bytes(1)[0]; guard byte <= 1 else { throw failure() }; return byte == 1
        }
        mutating func optional<T>(_ decode: (Data) throws -> T) throws -> Any {
            var r = try Reader(field()); let tag = try r.raw(1)[0]
            let value: Any
            switch tag { case 0: value = NSNull(); case 1: value = try decode(r.field()); default: throw failure() }
            try r.finish(); return value
        }
    }
}
