import Foundation

/// State-independent SCCP governance admission, shared by stored reads and draft inputs.
struct SccpGovernanceProposalValidator {
    let codingPath: [CodingKey]

    private static let safeMaximum: UInt64 = 9_007_199_254_740_991
    // Rust enum declaration order defines the canonical base-revision ordering.
    private static let subjects = [
        "route", "route_control", "light_client", "parameters", "forgery_hold",
        "fast_hold", "fast_pause_control",
    ]
    private static let networks = ["ethereum_mainnet", "bsc_mainnet", "ton_mainnet"]
    private static let actions: [String: (subject: String, fields: Set<String>)] = [
        "register_route": ("route", ["revision", "deployment", "initial_committee", "max_wrapped_supply"]),
        "activate_revision": ("route", ["revision"]),
        "switch_revision": ("route", ["from", "to"]),
        "deactivate_outbound": ("route", ["revision"]),
        "retire_revision": ("route", ["revision"]),
        "remove_staged": ("route", ["revision"]),
        "release_stranded": ("route", ["amount", "recipient", "memo"]),
        "set_taira_paused": ("route", ["paused"]),
        "set_destination_paused": ("route_control", ["revision", "paused"]),
        "initialize_light_client": ("light_client", ["expected", "params", "bootstrap"]),
        "install_trusted_checkpoint": ("light_client", ["checkpoint"]),
        "freeze_light_client": ("light_client", []),
        "set_parameters": ("parameters", ["next"]),
        "activate_light_client_profile": ("light_client", ["version", "profile_hash"]),
        "attest_deployment_progress": ("forgery_hold", ["revision", "at"]),
        "quarantine_revision": ("forgery_hold", ["revision"]),
        "open_reconciliation": ("forgery_hold", ["revision", "window_ms"]),
        "set_fast_pause_suspended": ("fast_pause_control", ["duration_ms"]),
    ]
    private static let parameterBounds: [String: ClosedRange<UInt64>] = [
        "committee_validity_ms": 0...2_592_000_000,
        "committee_heartbeat_ms": 3_600_000...safeMaximum,
        "forgery_evidence_grace_ms": 86_400_000...2_592_000_000,
        "destination_progress_cap_ms": 0...2_592_000_000,
        "outbound_ttl_ms": 86_400_000...7_776_000_000,
        "max_exempt_transactions_per_block": 8...1024,
        "fast_pause_panel_seats": 5...9,
        "fast_pause_panel_term_ms": 3_600_000...safeMaximum,
        "fast_pause_seat_accept_window_ms": 600_000...safeMaximum,
        "fast_pause_endorse_window_ms": 300_000...21_600_000,
        "fast_pause_renew_lead_ms": 0...safeMaximum,
        "fast_pause_hold_ms": 0...2_505_600_000,
    ]

    func validate(_ value: ToriiJSONValue) throws {
        let body = try object(value, ["network_id", "base_revisions", "actions"], "proposal")
        guard case .string(let literal)? = body["network_id"],
            (try? NetworkId(literal: literal)) != nil
        else {
            throw invalid("network_id must be a canonical NetworkId literal")
        }
        let bases = try entries(body["base_revisions"], "base_revisions")
        let baseSubjects = try bases.enumerated().map { index, item in
            let label = "base_revisions[\(index)]"
            let base = try object(item, ["subject", "revision"], label)
            try uint(base["revision"], "\(label).revision")
            return try subject(base["subject"], "\(label).subject")
        }
        var destinations = Set<Data>()
        let actions = try entries(body["actions"], "actions")
        let touched = try actions.enumerated().map { index, item in
            try action(item, "actions[\(index)]", destinations: &destinations)
        }
        guard baseSubjects == Set(touched).sorted() else {
            throw invalid("base_revisions must list exactly the sorted, unique action subjects")
        }
    }

    private func subject(_ value: ToriiJSONValue?, _ label: String) throws -> Int {
        let item = try object(value, ["subject", "key"], label)
        guard case .string(let tag)? = item["subject"],
            let index = Self.subjects.firstIndex(of: tag)
        else {
            throw invalid("\(label).subject is unknown")
        }
        if tag == "parameters" {
            guard item["key"] == .null else { throw invalid("\(label).key must be null") }
            return index * 3
        }
        return index * 3 + (try network(item["key"], "\(label).key"))
    }

    private func network(_ value: ToriiJSONValue?, _ label: String) throws -> Int {
        let item = try object(value, ["network", "profile"], label)
        guard case .string(let tag)? = item["network"],
            let index = Self.networks.firstIndex(of: tag), item["profile"] == .null
        else {
            throw invalid("\(label) must name an external SCCP network with null profile")
        }
        return index
    }

    private func action(
        _ value: ToriiJSONValue, _ label: String, destinations: inout Set<Data>
    ) throws -> Int {
        let entry = try object(value, ["action", "payload"], label)
        guard case .string(let tag)? = entry["action"], let shape = Self.actions[tag] else {
            throw invalid("\(label).action is unknown")
        }
        let label = "\(label).payload"
        let fields = shape.subject == "parameters" ? shape.fields : shape.fields.union(["network"])
        let body = try object(entry["payload"], fields, label)
        let net = shape.subject == "parameters" ? 0 : try network(body["network"], "\(label).network")
        if shape.fields.contains("revision") {
            try uint(body["revision"], "\(label).revision", 1...UInt64(UInt32.max))
        }
        switch tag {
        case "register_route":
            let word = try deployment(body["deployment"], net, "\(label).deployment")
            try committeePin(body["initial_committee"], "\(label).initial_committee")
            try amount(
                body["max_wrapped_supply"], "\(label).max_wrapped_supply",
                maximum: net == 2 ? "79228162514264337593543950335" : SccpUInt128.maximumDecimal)
            guard destinations.insert(word).inserted else {
                throw invalid("\(label).deployment repeats a registered destination word")
            }
        case "switch_revision":
            let from = try uint(body["from"], "\(label).from", 1...UInt64(UInt32.max))
            let to = try uint(body["to"], "\(label).to", 1...UInt64(UInt32.max))
            guard from != to else { throw invalid("\(label) must name distinct revisions") }
        case "release_stranded":
            try amount(body["amount"], "\(label).amount")
            guard case .string(let memo)? = body["memo"], memo.utf8.count <= 256 else {
                throw invalid("\(label).memo must be a string of at most 256 UTF-8 bytes")
            }
            guard case .string(let recipient)? = body["recipient"] else {
                throw invalid("\(label).recipient must be a canonical account address")
            }
            _ = try governanceCanonicalAccount(recipient, codingPath: codingPath, field: "\(label).recipient")
        case "set_taira_paused", "set_destination_paused":
            try boolean(body["paused"], "\(label).paused")
        case "initialize_light_client":
            let expected = try object(body["expected"], ["expected", "detail"], "\(label).expected")
            guard expected["expected"] == .string("absent") || expected["expected"] == .string("unusable"),
                expected["detail"] == .null
            else {
                throw invalid("\(label).expected must name absent or unusable with null detail")
            }
            try lightClient(body["params"], net, "\(label).params")
            try bootstrap(body["bootstrap"], net, "\(label).bootstrap")
        case "install_trusted_checkpoint":
            guard net != 2 else { throw invalid("\(label).network cannot be TON for a trusted checkpoint") }
            try checkpoint(body["checkpoint"], "\(label).checkpoint")
        case "set_parameters":
            try parameters(body["next"], "\(label).next")
        case "activate_light_client_profile":
            try uint(body["version"], "\(label).version", 2...UInt64(UInt32.max))
            try bytes(body["profile_hash"], "\(label).profile_hash", nonzero: true)
        case "attest_deployment_progress":
            try progress(body["at"], "\(label).at")
        case "open_reconciliation":
            try uint(body["window_ms"], "\(label).window_ms", 1...Self.safeMaximum)
        case "set_fast_pause_suspended":
            try uint(body["duration_ms"], "\(label).duration_ms", 0...7_776_000_000)
        default:
            break  // Revision-only actions and freeze_light_client are fully checked above.
        }
        return Self.subjects.firstIndex(of: shape.subject)! * 3 + net
    }

    private func deployment(_ value: ToriiJSONValue?, _ net: Int, _ label: String) throws -> Data {
        let item = try object(value, ["family", "deployment"], label)
        if net != 2, item["family"] == .string("evm") {
            let body = try object(item["deployment"], ["address", "runtime_code_hash"], label)
            let address = try bytes(body["address"], "\(label).address", count: 20, nonzero: true)
            try bytes(body["runtime_code_hash"], "\(label).runtime_code_hash")
            return Data(repeating: 0, count: 12) + address
        }
        guard net == 2, item["family"] == .string("ton") else {
            throw invalid("\(label).family must match the external network")
        }
        let body = try object(
            item["deployment"], ["master_account", "minter_code", "wallet_code", "bucket_code"], label)
        let word = try bytes(body["master_account"], "\(label).master_account", nonzero: true)
        for field in ["minter_code", "wallet_code", "bucket_code"] {
            let code = try object(body[field], ["hash", "depth"], "\(label).\(field)")
            try bytes(code["hash"], "\(label).\(field).hash")
            try uint(code["depth"], "\(label).\(field).depth", 0...UInt64(UInt16.max))
        }
        return word
    }

    private func committeePin(_ value: ToriiJSONValue?, _ label: String) throws {
        let pin = try object(value, ["generation", "committee_root", "start_height", "until_ms"], label)
        for field in ["generation", "start_height", "until_ms"] {
            try uint(pin[field], "\(label).\(field)", 1...Self.safeMaximum)
        }
        try bytes(pin["committee_root"], "\(label).committee_root", nonzero: true)
    }

    private func lightClient(_ value: ToriiJSONValue?, _ net: Int, _ label: String) throws {
        let times = ["ws_bound_ms", "set_retention_ms", "checkpoint_stride", "checkpoint_prune_after_ms"]
        let counts = [
            "max_updates_per_advance", "max_segment_headers", "max_ancestry_headers",
            "max_backfill_headers", "max_advance_bytes", "max_proof_bytes",
        ]
        let params = try object(value, Set(times + counts + ["network"]), label)
        guard try network(params["network"], "\(label).network") == net else {
            throw invalid("\(label).network must match the action network")
        }
        for field in times {
            let derived = net == 2 && ["ws_bound_ms", "checkpoint_stride"].contains(field)
            try uint(params[field], "\(label).\(field)", derived ? 0...0 : 1...Self.safeMaximum)
        }
        for field in counts {
            try uint(params[field], "\(label).\(field)", 1...UInt64(UInt32.max))
        }
    }

    private func bootstrap(_ value: ToriiJSONValue?, _ net: Int, _ label: String) throws {
        let body = try object(value, ["network", "bytes"], label)
        guard try network(body["network"], "\(label).network") == net else {
            throw invalid("\(label).network must match the action network")
        }
        let maximum = 1_048_576
        guard case .string(let encoded)? = body["bytes"],
            (1...(4 * ((maximum + 2) / 3))).contains(encoded.utf8.count),
            let decoded = Data(base64Encoded: encoded), (1...maximum).contains(decoded.count),
            decoded.base64EncodedString() == encoded
        else {
            throw invalid("\(label).bytes must be canonical base64 encoding 1...\(maximum) bytes")
        }
    }

    private func checkpoint(_ value: ToriiJSONValue?, _ label: String) throws {
        let item = try object(
            value, ["source_height", "block_hash", "state_root", "receipts_or_tx_root", "source_time_ms"], label)
        for field in ["source_height", "source_time_ms"] {
            try uint(item[field], "\(label).\(field)")
        }
        for field in ["block_hash", "receipts_or_tx_root"] {
            try bytes(item[field], "\(label).\(field)")
        }
        if item["state_root"] != .null { try bytes(item["state_root"], "\(label).state_root") }
    }

    private func progress(_ value: ToriiJSONValue?, _ label: String) throws {
        let item = try object(value, ["at", "detail"], label)
        switch item["at"] {
        case .string("frozen")?:
            guard item["detail"] == .null else { throw invalid("\(label).detail must be null") }
        case .string("generation")?:
            let detail = try object(item["detail"], ["generation", "committee_root"], "\(label).detail")
            try uint(detail["generation"], "\(label).detail.generation", 1...Self.safeMaximum)
            try bytes(detail["committee_root"], "\(label).detail.committee_root", nonzero: true)
        default:
            throw invalid("\(label).at must be frozen or generation")
        }
    }

    private func parameters(_ value: ToriiJSONValue?, _ label: String) throws {
        let fields = Set(Self.parameterBounds.keys).union(["enabled", "min_outbound_amount", "inbound_self_claim_fee"])
        let item = try object(value, fields, label)
        try boolean(item["enabled"], "\(label).enabled")
        var values: [String: UInt64] = [:]
        for (field, bounds) in Self.parameterBounds {
            values[field] = try uint(item[field], "\(label).\(field)", bounds)
        }
        try amount(item["min_outbound_amount"], "\(label).min_outbound_amount", maximum: "1000000000000000")
        try amount(
            item["inbound_self_claim_fee"], "\(label).inbound_self_claim_fee", maximum: "10000000000", allowZero: true)
        guard [5, 7, 9].contains(values["fast_pause_panel_seats"]!) else {
            throw invalid("\(label).fast_pause_panel_seats must be 5, 7 or 9")
        }
        // Each operand is at most 2^53 - 1. UInt64 keeps the joint bounds exact
        // (including products above the safe Double range) without overflowing.
        let floor = values["fast_pause_hold_ms"]! + 2 * values["committee_heartbeat_ms"]! + 86_400_000
        guard values["committee_validity_ms"]! >= floor,
            2 * values["fast_pause_seat_accept_window_ms"]! <= values["fast_pause_panel_term_ms"]!
        else {
            throw invalid("\(label) violates the committee validity or panel acceptance window bound")
        }
        let lead = values["fast_pause_renew_lead_ms"]!
        guard lead >= 2 * values["fast_pause_endorse_window_ms"]!, 2 * lead <= values["fast_pause_hold_ms"]! else {
            throw invalid("\(label).fast_pause_renew_lead_ms must span two endorsements and fit half the hold")
        }
    }

    private func object(_ value: ToriiJSONValue?, _ fields: Set<String>, _ label: String) throws -> [String:
        ToriiJSONValue]
    {
        guard case .object(let item)? = value, Set(item.keys) == fields else {
            throw invalid("\(label) must have exactly the fields \(fields.sorted().joined(separator: ", "))")
        }
        return item
    }

    private func entries(_ value: ToriiJSONValue?, _ label: String) throws -> [ToriiJSONValue] {
        guard case .array(let items)? = value, (1...16).contains(items.count) else {
            throw invalid("\(label) must hold 1...16 entries")
        }
        return items
    }

    @discardableResult
    private func uint(
        _ value: ToriiJSONValue?, _ label: String,
        _ bounds: ClosedRange<UInt64> = 0...SccpGovernanceProposalValidator.safeMaximum
    ) throws
        -> UInt64
    {
        guard case .number(let number)? = value, number.isFinite, number >= 0,
            number <= Double(Self.safeMaximum), number.rounded(.towardZero) == number,
            bounds.contains(UInt64(number))
        else {
            throw invalid("\(label) must be an exact unsigned integer in \(bounds)")
        }
        return UInt64(number)
    }

    private func boolean(_ value: ToriiJSONValue?, _ label: String) throws {
        guard case .bool? = value else { throw invalid("\(label) must be a boolean") }
    }

    @discardableResult
    private func bytes(_ value: ToriiJSONValue?, _ label: String, count: Int = 32, nonzero: Bool = false) throws -> Data
    {
        guard case .array(let items)? = value, items.count == count else {
            throw invalid("\(label) must be an exact \(count)-byte array")
        }
        let data = try Data(items.map { UInt8(try uint($0, label, 0...255)) })
        guard !nonzero || data.contains(where: { $0 != 0 }) else { throw invalid("\(label) must be nonzero") }
        return data
    }

    private func amount(
        _ value: ToriiJSONValue?, _ label: String, maximum: String = SccpUInt128.maximumDecimal, allowZero: Bool = false
    ) throws {
        guard case .string(let raw)? = value, raw.utf8.count <= maximum.utf8.count,
            let parsed = SccpUInt128.parse(raw), allowZero || parsed.decimalString != "0",
            raw.utf8.count < maximum.utf8.count || !maximum.utf8.lexicographicallyPrecedes(raw.utf8)
        else {
            throw invalid("\(label) must be a canonical decimal in \(allowZero ? "0" : "1")...\(maximum)")
        }
    }

    private func invalid(_ message: String) -> DecodingError {
        .dataCorrupted(.init(codingPath: codingPath, debugDescription: "SCCP route-governance proposal \(message)"))
    }
}
