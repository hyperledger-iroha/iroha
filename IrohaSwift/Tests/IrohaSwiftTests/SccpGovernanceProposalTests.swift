import Foundation
import XCTest

@testable import IrohaSwift

/// SCCP static admission at the public stored-reader and Parliament draft boundaries.
final class SccpGovernanceProposalTests: XCTestCase {
    private let safeMaximum: UInt64 = 9_007_199_254_740_991
    private let networks = ["ethereum_mainnet", "bsc_mainnet", "ton_mainnet"]
    // Independent inventory from SccpGovernanceActionV1::subject in the Rust model.
    private let subjects = [
        "register_route": "route", "activate_revision": "route", "switch_revision": "route",
        "deactivate_outbound": "route", "retire_revision": "route", "remove_staged": "route",
        "release_stranded": "route", "set_taira_paused": "route",
        "set_destination_paused": "route_control", "initialize_light_client": "light_client",
        "install_trusted_checkpoint": "light_client", "freeze_light_client": "light_client",
        "set_parameters": "parameters", "activate_light_client_profile": "light_client",
        "attest_deployment_progress": "forgery_hold", "quarantine_revision": "forgery_hold",
        "open_reconciliation": "forgery_hold", "set_fast_pause_suspended": "fast_pause_control",
    ]
    private let owner = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"

    private func network(_ name: String) -> [String: Any] {
        ["network": name, "profile": NSNull()]
    }

    private func base(_ tag: String, _ name: String, revision: UInt64 = 0) -> [String: Any] {
        var subject: [String: Any] = ["subject": tag, "key": NSNull()]
        if tag != "parameters" { subject["key"] = network(name) }
        return ["subject": subject, "revision": revision]
    }

    private func parameters() -> [String: Any] {
        let day = 86_400_000
        return [
            "enabled": true, "committee_validity_ms": 14 * day, "committee_heartbeat_ms": day,
            "forgery_evidence_grace_ms": 7 * day, "destination_progress_cap_ms": 30 * day,
            "outbound_ttl_ms": 7 * day, "min_outbound_amount": "1000000000",
            "inbound_self_claim_fee": "10000000", "max_exempt_transactions_per_block": 32,
            "fast_pause_panel_seats": 9, "fast_pause_panel_term_ms": 7 * day,
            "fast_pause_seat_accept_window_ms": 43_200_000, "fast_pause_endorse_window_ms": 1_800_000,
            "fast_pause_renew_lead_ms": day, "fast_pause_hold_ms": 10 * day,
        ]
    }

    private func payload(_ action: String, _ name: String = "bsc_mainnet") -> [String: Any] {
        var body: [String: Any] = ["network": network(name)]
        switch action {
        case "register_route":
            let code: [String: Any] = ["hash": Array(repeating: 0, count: 32), "depth": 0]
            body["revision"] = 1
            body["deployment"] =
                name == "ton_mainnet"
                ? [
                    "family": "ton",
                    "deployment": [
                        "master_account": Array(repeating: 1, count: 32),
                        "minter_code": code, "wallet_code": code, "bucket_code": code,
                    ],
                ]
                : [
                    "family": "evm",
                    "deployment": [
                        "address": Array(repeating: 1, count: 20),
                        "runtime_code_hash": Array(repeating: 0, count: 32),
                    ],
                ]
            body["initial_committee"] = [
                "generation": 1, "committee_root": Array(repeating: 1, count: 32),
                "start_height": 1, "until_ms": 1,
            ]
            body["max_wrapped_supply"] = "1"
        case "switch_revision":
            body["from"] = 1
            body["to"] = 2
        case "release_stranded":
            body["amount"] = "1"
            body["recipient"] = owner
            body["memo"] = ""
        case "set_taira_paused": body["paused"] = true
        case "set_destination_paused":
            body["revision"] = 1
            body["paused"] = false
        case "initialize_light_client":
            body["expected"] = ["expected": "absent", "detail": NSNull()]
            var params: [String: Any] = [
                "network": network(name), "set_retention_ms": 1,
                "checkpoint_prune_after_ms": 1, "ws_bound_ms": name == "ton_mainnet" ? 0 : 1,
                "checkpoint_stride": name == "ton_mainnet" ? 0 : 1,
            ]
            for field in [
                "max_updates_per_advance", "max_segment_headers", "max_ancestry_headers",
                "max_backfill_headers", "max_advance_bytes", "max_proof_bytes",
            ] { params[field] = 1 }
            body["params"] = params
            body["bootstrap"] = ["network": network(name), "bytes": "AA=="]
        case "install_trusted_checkpoint":
            body["checkpoint"] = [
                "source_height": 0, "source_time_ms": 0, "state_root": NSNull(),
                "block_hash": Array(repeating: 0, count: 32), "receipts_or_tx_root": Array(repeating: 0, count: 32),
            ]
        case "freeze_light_client": break
        case "set_parameters": return ["next": parameters()]
        case "activate_light_client_profile":
            body["version"] = 2
            body["profile_hash"] = Array(repeating: 1, count: 32)
        case "attest_deployment_progress":
            body["revision"] = 1
            body["at"] = ["at": "frozen", "detail": NSNull()]
        case "open_reconciliation":
            body["revision"] = 1
            body["window_ms"] = 1
        case "set_fast_pause_suspended": body["duration_ms"] = 0
        default: body["revision"] = 1
        }
        return body
    }

    private func proposal(_ action: String, _ body: [String: Any]? = nil, _ name: String = "bsc_mainnet") -> [String:
        Any]
    {
        [
            "network_id": TestNetworkIds.canonical.literal,
            "base_revisions": [base(subjects[action]!, name)],
            "actions": [["action": action, "payload": body ?? payload(action, name)]],
        ]
    }

    private func wire(_ proposal: [String: Any]) throws -> Data {
        try JSONSerialization.data(
            withJSONObject: ["kind": "SccpRouteGovernance", "payload": ["proposal": proposal]], options: [.sortedKeys])
    }

    private func accepted(
        _ proposal: [String: Any], _ label: String = "", file: StaticString = #filePath, line: UInt = #line
    ) throws {
        let data = try wire(proposal)
        XCTAssertNoThrow(
            try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: data), label, file: file, line: line)
        XCTAssertNoThrow(try ToriiParliamentProposalV1(validating: data), label, file: file, line: line)
    }

    private func rejected(
        _ proposal: [String: Any], _ label: String = "", file: StaticString = #filePath, line: UInt = #line
    ) throws {
        let data = try wire(proposal)
        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: data), label, file: file, line: line)
        XCTAssertThrowsError(try ToriiParliamentProposalV1(validating: data), label, file: file, line: line)
    }

    private func replacing(_ body: [String: Any], _ path: [String], _ value: Any?) -> [String: Any] {
        var result = body
        if path.count == 1 {
            result[path[0]] = value
        } else {
            result[path[0]] = replacing(body[path[0]] as! [String: Any], Array(path.dropFirst()), value)
        }
        return result
    }

    func testAllActionBodiesAndExactNestedFields() throws {
        for action in subjects.keys.sorted() {
            let body = payload(action)
            try accepted(proposal(action), action)
            for field in body.keys {
                try rejected(proposal(action, replacing(body, [field], nil)), "\(action) missing \(field)")
            }
            try rejected(proposal(action, replacing(body, ["extra"], 0)), action)
        }
        let nested: [(String, String, [String])] = [
            ("register_route", "bsc_mainnet", ["deployment"]),
            ("register_route", "bsc_mainnet", ["deployment", "deployment"]),
            ("register_route", "bsc_mainnet", ["initial_committee"]),
            ("register_route", "ton_mainnet", ["deployment", "deployment"]),
            ("register_route", "ton_mainnet", ["deployment", "deployment", "minter_code"]),
            ("register_route", "ton_mainnet", ["deployment", "deployment", "wallet_code"]),
            ("register_route", "ton_mainnet", ["deployment", "deployment", "bucket_code"]),
            ("initialize_light_client", "bsc_mainnet", ["expected"]),
            ("initialize_light_client", "bsc_mainnet", ["params"]),
            ("initialize_light_client", "bsc_mainnet", ["bootstrap"]),
            ("install_trusted_checkpoint", "bsc_mainnet", ["checkpoint"]),
            ("attest_deployment_progress", "bsc_mainnet", ["at"]),
            ("set_parameters", "bsc_mainnet", ["next"]),
        ]
        for (action, name, path) in nested {
            let body = payload(action, name)
            var item = body
            for key in path { item = item[key] as! [String: Any] }
            for field in item.keys {
                try rejected(proposal(action, replacing(body, path + [field], nil), name), "\(action).\(path).\(field)")
            }
            try rejected(proposal(action, replacing(body, path + ["extra"], 0), name), "\(action).\(path)")
        }
    }

    func testRevisionBooleanAndByteBounds() throws {
        for action in [
            "activate_revision", "deactivate_outbound", "retire_revision", "remove_staged",
            "quarantine_revision", "register_route", "set_destination_paused",
            "attest_deployment_progress", "open_reconciliation",
        ] {
            for invalid: Any in [0, -1, true, "1", UInt64(UInt32.max) + 1] {
                try rejected(proposal(action, replacing(payload(action), ["revision"], invalid)), action)
            }
            try accepted(proposal(action, replacing(payload(action), ["revision"], UInt32.max)), action)
        }
        for action in ["set_taira_paused", "set_destination_paused"] {
            for invalid: Any in [0, 1, "true", NSNull()] {
                try rejected(proposal(action, replacing(payload(action), ["paused"], invalid)), action)
            }
        }
        let cases: [(String, [String], Any)] = [
            ("switch_revision", ["to"], 1), ("switch_revision", ["from"], 0),
            ("switch_revision", ["to"], UInt64(UInt32.max) + 1),
            ("activate_light_client_profile", ["version"], 1),
            ("activate_light_client_profile", ["version"], UInt64(UInt32.max) + 1),
            ("activate_light_client_profile", ["profile_hash"], Array(repeating: 0, count: 32)),
            ("activate_light_client_profile", ["profile_hash"], Array(repeating: true, count: 32)),
            ("activate_light_client_profile", ["profile_hash"], Array(repeating: 256, count: 32)),
            ("activate_light_client_profile", ["profile_hash"], Array(repeating: 1, count: 31)),
            ("open_reconciliation", ["window_ms"], 0),
            ("open_reconciliation", ["window_ms"], safeMaximum + 1),
            ("set_fast_pause_suspended", ["duration_ms"], 7_776_000_001 as UInt64),
            ("attest_deployment_progress", ["at", "detail"], [:] as [String: Any]),
            ("attest_deployment_progress", ["at", "at"], "unknown"),
            ("register_route", ["deployment", "deployment", "address"], Array(repeating: 0, count: 20)),
            ("register_route", ["deployment", "family"], "ton"),
            ("register_route", ["initial_committee", "committee_root"], Array(repeating: 0, count: 32)),
        ]
        for (action, path, invalid) in cases {
            try rejected(proposal(action, replacing(payload(action), path, invalid)), "\(action).\(path)")
        }
        for field in ["generation", "start_height", "until_ms"] {
            for invalid in [UInt64(0), safeMaximum + 1] {
                try rejected(
                    proposal(
                        "register_route", replacing(payload("register_route"), ["initial_committee", field], invalid)),
                    field)
            }
        }
        try accepted(
            proposal(
                "set_fast_pause_suspended",
                replacing(payload("set_fast_pause_suspended"), ["duration_ms"], 7_776_000_000 as UInt64)))
        try accepted(
            proposal("open_reconciliation", replacing(payload("open_reconciliation"), ["window_ms"], safeMaximum)))
    }

    func testDeploymentAmountAndTonBoundaries() throws {
        for name in networks {
            var body = payload("register_route", name)
            let maximum =
                name == "ton_mainnet" ? "79228162514264337593543950335" : "340282366920938463463374607431768211455"
            body["max_wrapped_supply"] = maximum
            for field in ["generation", "start_height", "until_ms"] {
                body = replacing(body, ["initial_committee", field], safeMaximum)
            }
            try accepted(proposal("register_route", body, name))
            let overflow =
                name == "ton_mainnet" ? "79228162514264337593543950336" : "340282366920938463463374607431768211456"
            for invalid: Any in ["0", "01", "-1", "1.0", 1, overflow, String(repeating: "9", count: 10000)] {
                body["max_wrapped_supply"] = invalid
                try rejected(proposal("register_route", body, name), "\(name) cap")
            }
        }
        let ton = payload("register_route", "ton_mainnet")
        for field in ["minter_code", "wallet_code", "bucket_code"] {
            let path = ["deployment", "deployment", field, "depth"]
            try accepted(proposal("register_route", replacing(ton, path, 65535), "ton_mainnet"))
            try rejected(proposal("register_route", replacing(ton, path, 65536), "ton_mainnet"))
        }
    }

    func testSubjectOrderingAndDuplicateDestinations() throws {
        var body = proposal("remove_staged")
        let action = (body["actions"] as! [[String: Any]])[0]
        body["actions"] = Array(repeating: action, count: 16)
        try accepted(body)
        body["actions"] = Array(repeating: action, count: 17)
        try rejected(body)
        body["actions"] = [action]
        body["base_revisions"] = [base("route", "bsc_mainnet"), base("route", "bsc_mainnet")]
        try rejected(body)

        var actions: [[String: Any]] = []
        var bases: [[String: Any]] = []
        for tag in [
            "set_taira_paused", "set_destination_paused", "freeze_light_client", "set_parameters",
            "quarantine_revision", "set_fast_pause_suspended",
        ] {
            for name in tag == "set_parameters" ? ["bsc_mainnet"] : networks {
                actions.append(["action": tag, "payload": payload(tag, name)])
                bases.append(base(subjects[tag]!, name, revision: safeMaximum))
            }
        }
        XCTAssertEqual(actions.count, 16)
        body["actions"] = Array(actions.reversed())
        body["base_revisions"] = bases
        try accepted(body)
        body["base_revisions"] = Array(bases.reversed())
        try rejected(body)
        body["base_revisions"] = Array(bases.dropLast())
        try rejected(body)

        for name in ["ethereum_mainnet", "ton_mainnet"] {
            var other = payload("register_route", name)
            let field = name == "ton_mainnet" ? "master_account" : "address"
            let address =
                name == "ton_mainnet"
                ? Array(repeating: 0, count: 12) + Array(repeating: 1, count: 20) : Array(repeating: 1, count: 20)
            other = replacing(other, ["deployment", "deployment", field], address)
            var pair = proposal("register_route")
            let first = (pair["actions"] as! [[String: Any]])[0]
            pair["actions"] = [first, ["action": "register_route", "payload": other]]
            pair["base_revisions"] = (name == "ethereum_mainnet" ? [name, "bsc_mainnet"] : ["bsc_mainnet", name]).map {
                base("route", $0)
            }
            try rejected(pair, "duplicate destination across \(name)")
            var distinct = address
            distinct[distinct.count - 1] = 2
            other = replacing(other, ["deployment", "deployment", field], distinct)
            pair["actions"] = [first, ["action": "register_route", "payload": other]]
            try accepted(pair)
        }
    }

    func testLightClientBoundsAndCanonicalBootstrap() throws {
        let action = "initialize_light_client"
        for name in networks {
            let body = payload(action, name)
            try accepted(proposal(action, body, name))
            for field in ["params", "bootstrap"] {
                let other = name == "ton_mainnet" ? "bsc_mainnet" : "ton_mainnet"
                try rejected(proposal(action, replacing(body, [field, "network"], network(other)), name))
            }
            let params = body["params"] as! [String: Any]
            for field in params.keys where field != "network" {
                let derived = name == "ton_mainnet" && ["ws_bound_ms", "checkpoint_stride"].contains(field)
                try rejected(proposal(action, replacing(body, ["params", field], derived ? 1 : 0), name), field)
            }
            try accepted(
                proposal(
                    action,
                    replacing(
                        body, ["bootstrap", "bytes"], Data(repeating: 0, count: 1_048_576).base64EncodedString()), name)
            )
            try rejected(
                proposal(
                    action,
                    replacing(
                        body, ["bootstrap", "bytes"], Data(repeating: 0, count: 1_048_577).base64EncodedString()), name)
            )
        }
        let body = payload(action)
        for invalid: Any in ["", "AA", "AB==", " AA==", "AA==\n", "AA-_", 1] {
            try rejected(proposal(action, replacing(body, ["bootstrap", "bytes"], invalid)))
        }
        try rejected(proposal(action, replacing(body, ["expected", "expected"], "present")))
        try rejected(proposal(action, replacing(body, ["expected", "detail"], [:] as [String: Any])))
        try accepted(proposal(action, replacing(body, ["expected", "expected"], "unusable")))
        try rejected(proposal(action, replacing(body, ["params", "max_proof_bytes"], UInt64(UInt32.max) + 1)))
        try rejected(proposal(action, replacing(body, ["params", "set_retention_ms"], safeMaximum + 1)))
    }

    func testCheckpointAndDeploymentProgress() throws {
        let checkpoint = "install_trusted_checkpoint"
        var body = payload(checkpoint)
        for field in ["source_height", "source_time_ms"] {
            body = replacing(body, ["checkpoint", field], safeMaximum)
            try rejected(proposal(checkpoint, replacing(body, ["checkpoint", field], safeMaximum + 1)))
        }
        body = replacing(body, ["checkpoint", "state_root"], Array(repeating: 0, count: 32))
        try accepted(proposal(checkpoint, body))
        try rejected(proposal(checkpoint, payload(checkpoint, "ton_mainnet"), "ton_mainnet"))
        for field in ["block_hash", "state_root", "receipts_or_tx_root"] {
            try rejected(proposal(checkpoint, replacing(body, ["checkpoint", field], Array(repeating: 0, count: 31))))
        }
        let action = "attest_deployment_progress"
        let progress = replacing(
            payload(action), ["at"],
            [
                "at": "generation",
                "detail": ["generation": safeMaximum, "committee_root": Array(repeating: 1, count: 32)],
            ])
        try accepted(proposal(action, progress))
        for value in [UInt64(0), safeMaximum + 1] {
            try rejected(proposal(action, replacing(progress, ["at", "detail", "generation"], value)))
        }
        try rejected(
            proposal(action, replacing(progress, ["at", "detail", "committee_root"], Array(repeating: 0, count: 32))))
    }

    func testParameterOwnAndJointBounds() throws {
        let invalid: [(String, Any)] = [
            ("enabled", 1), ("committee_validity_ms", 2_592_000_001), ("committee_validity_ms", 1),
            ("committee_heartbeat_ms", 3_599_999), ("committee_heartbeat_ms", safeMaximum),
            ("forgery_evidence_grace_ms", 86_399_999), ("forgery_evidence_grace_ms", 2_592_000_001),
            ("destination_progress_cap_ms", -1), ("destination_progress_cap_ms", 2_592_000_001),
            ("outbound_ttl_ms", 86_399_999), ("outbound_ttl_ms", 7_776_000_001 as UInt64),
            ("min_outbound_amount", "0"), ("min_outbound_amount", "1000000000000001"),
            ("inbound_self_claim_fee", "10000000001"), ("max_exempt_transactions_per_block", 7),
            ("max_exempt_transactions_per_block", 1025), ("fast_pause_panel_seats", 6),
            ("fast_pause_panel_term_ms", 3_599_999), ("fast_pause_seat_accept_window_ms", 599_999),
            ("fast_pause_seat_accept_window_ms", 302_400_001), ("fast_pause_endorse_window_ms", 299_999),
            ("fast_pause_endorse_window_ms", 21_600_001), ("fast_pause_renew_lead_ms", 3_599_999),
            ("fast_pause_renew_lead_ms", 432_000_001), ("fast_pause_hold_ms", 2_505_600_001),
            ("fast_pause_panel_term_ms", safeMaximum + 1),
        ]
        for (field, value) in invalid {
            try rejected(
                proposal("set_parameters", replacing(payload("set_parameters"), ["next", field], value)), field)
        }
        var boundary = parameters()
        boundary["committee_validity_ms"] = 1_123_200_000
        boundary["min_outbound_amount"] = "1000000000000000"
        boundary["inbound_self_claim_fee"] = "0"
        boundary["fast_pause_panel_term_ms"] = safeMaximum
        boundary["fast_pause_seat_accept_window_ms"] = safeMaximum / 2
        try accepted(proposal("set_parameters", ["next": boundary]))
        boundary["fast_pause_seat_accept_window_ms"] = safeMaximum / 2 + 1
        try rejected(proposal("set_parameters", ["next": boundary]))

        // Static admission cannot apply the live staking delay checked at enactment.
        var staticPolicy = parameters()
        staticPolicy["committee_validity_ms"] = 2_592_000_000
        staticPolicy["forgery_evidence_grace_ms"] = 2_592_000_000
        try accepted(proposal("set_parameters", ["next": staticPolicy]))
    }

    func testReleaseUsesCanonicalAccountAndUtf8MemoLimit() throws {
        let action = "release_stranded"
        for memo in ["", " \u{0}\n", String(repeating: "é", count: 128), String(repeating: "🦊", count: 64)] {
            var body = payload(action)
            body["memo"] = memo
            body["amount"] = "340282366920938463463374607431768211455"
            try accepted(proposal(action, body))
        }
        for invalid: Any in [
            NSNull(), 1, String(repeating: "a", count: 257), String(repeating: "é", count: 129),
            String(repeating: "🦊", count: 65),
        ] {
            try rejected(proposal(action, replacing(payload(action), ["memo"], invalid)))
        }
        for invalid: Any in [
            "0", "01", "-1", "1.0", 1, "340282366920938463463374607431768211456", String(repeating: "9", count: 10000),
        ] {
            try rejected(proposal(action, replacing(payload(action), ["amount"], invalid)))
        }
        try rejected(proposal(action, replacing(payload(action), ["recipient"], "not-an-account")))
    }

    func testDraftRejectsNoncanonicalNumberTokensAndDuplicateKeys() throws {
        let data = try wire(proposal("remove_staged"))
        let text = String(decoding: data, as: UTF8.self)
        for token in ["-0", "1.0", "1e0", "1E0", "9007199254740992"] {
            let bad = text.replacingOccurrences(of: "\"revision\":1", with: "\"revision\":\(token)")
            XCTAssertThrowsError(try ToriiParliamentProposalV1(validating: Data(bad.utf8)), token)
        }
        let duplicate = text.replacingOccurrences(of: "\"revision\":1", with: "\"revision\":1,\"revision\":1")
        XCTAssertThrowsError(try ToriiParliamentProposalV1(validating: Data(duplicate.utf8)))

        let release = String(decoding: try wire(proposal("release_stranded")), as: UTF8.self)
        for surrogate in [#"\ud800"#, #"\udc00"#] {
            let bad = release.replacingOccurrences(of: "\"memo\":\"\"", with: "\"memo\":\"\(surrogate)\"")
            XCTAssertThrowsError(try JSONDecoder().decode(ToriiGovernanceProposalKind.self, from: Data(bad.utf8)))
            XCTAssertThrowsError(try ToriiParliamentProposalV1(validating: Data(bad.utf8)))
        }
    }

    func testSharedRustGovernanceSubjectVectors() throws {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let fixture =
            try JSONSerialization.jsonObject(
                with: Data(contentsOf: root.appendingPathComponent("fixtures/sccp/governance_v1.json")))
            as! [String: Any]
        for row in fixture["rows"] as! [[String: Any]] {
            let input = row["input"] as! [String: Any]
            let bases = (input["base_revisions"] as! [[String: Any]]).map { item -> [String: Any] in
                let subject = item["subject"] as! [String: Any]
                let name = (subject["network"] as? String ?? "").replacingOccurrences(of: "-", with: "_")
                return base(subject["kind"] as! String, name, revision: (item["revision"] as! NSNumber).uint64Value)
            }
            let actions = (input["actions"] as! [[String: Any]]).map { item -> [String: Any] in
                var body = item
                body.removeValue(forKey: "kind")
                body["network"] = network((item["network"] as! String).replacingOccurrences(of: "-", with: "_"))
                return ["action": item["kind"]!, "payload": body]
            }
            let value: [String: Any] = [
                "network_id": TestNetworkIds.canonical.literal, "base_revisions": bases, "actions": actions,
            ]
            let expected = row["expected"] as! [String: Any]
            if expected["static_outcome"] as! String == "Accepted" {
                try accepted(value, row["label"] as! String)
            } else {
                try rejected(value, row["label"] as! String)
            }
        }
    }
}
