import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Pins the authenticated Parliament V1 SDK surface to the cross-SDK fixture.
final class ToriiParliamentAPIV1Tests: XCTestCase {
    private static let contractAddress =
        "irohac1qyqqqqqqqqqqqqputuv64zhf0a0a4hhlqdj2lhnwuzq4xjq3qexfh"
    private let attemptID = String(repeating: "ab", count: 32)
    private let proposalID = String(repeating: "cd", count: 32)
    private let root = [UInt8](repeating: 0x55, count: 32)

    private func deployProposal() throws -> ToriiParliamentProposalV1 {
        try ToriiParliamentProposalV1(
            validating: Data(
                """
                {"kind":"DeployContract","payload":{"contract_address":"\(Self.contractAddress)","code_hash":"\(String(repeating: "11", count: 32))","abi_hash":"\(String(repeating: "22", count: 32))","abi_version":1,"manifest_provenance":null}}
                """.utf8
            )
        )
    }

    func testSharedFixturePinsCompletePublicAndAuditInventories() throws {
        let fixture = try fixtureObject()
        XCTAssertEqual(fixture["api_version"] as? Int, Int(ToriiParliamentAPIV1.version))
        let routes = try XCTUnwrap(fixture["routes"] as? [String: String])
        XCTAssertEqual(routes["attempt_draft"], ToriiParliamentAPIV1.attemptDraftPath)
        XCTAssertEqual(routes["attempt_read"], ToriiParliamentAPIV1.attemptReadPathTemplate)
        XCTAssertEqual(
            routes["timed_ovn_casting_context_read"],
            ToriiParliamentAPIV1.timedOvnCastingContextReadPathTemplate
        )
        XCTAssertEqual(
            routes["timed_ovn_casting_proof"],
            ToriiParliamentAPIV1.timedOvnCastingProofPathTemplate
        )
        XCTAssertEqual(
            routes["tle_release_context_read"],
            ToriiParliamentAPIV1.tleReleaseContextReadPathTemplate
        )
        XCTAssertEqual(
            routes["tle_partial_release"],
            ToriiParliamentAPIV1.tlePartialReleasePathTemplate
        )
        XCTAssertEqual(routes["transition_draft"], ToriiParliamentAPIV1.transitionDraftPath)

        let transitions = try XCTUnwrap(fixture["public_transitions"] as? [[String: Any]])
        XCTAssertEqual(transitions.count, 21)
        XCTAssertEqual(transitions.count, ToriiParliamentAPIV1.publicTransitions.count)
        for (fixtureEntry, sdkEntry) in zip(transitions, ToriiParliamentAPIV1.publicTransitions) {
            XCTAssertEqual(fixtureEntry["norito_index"] as? Int, Int(sdkEntry.noritoIndex))
            XCTAssertEqual(fixtureEntry["json_tag"] as? String, sdkEntry.jsonTag)
            XCTAssertEqual(fixtureEntry["event_kind_index"] as? Int, Int(sdkEntry.eventKindIndex))
            XCTAssertEqual(
                fixtureEntry["json_payload"] as? String == "required",
                sdkEntry.jsonPayloadRequired
            )
        }

        let outcomes = try XCTUnwrap(
            fixture["automatic_execution_outcomes"] as? [[String: Any]]
        )
        XCTAssertEqual(outcomes.count, ToriiParliamentAPIV1.automaticExecutionOutcomes.count)
        let reasons = try XCTUnwrap(fixture["no_result_kinds"] as? [[String: Any]])
        XCTAssertEqual(
            reasons.compactMap { $0["json_tag"] as? String },
            ToriiParliamentAPIV1.noResultKinds.map(\.jsonTag)
        )
        let bodyState = try XCTUnwrap(
            fixture["attempt_read_body_state"] as? [String: Any]
        )
        XCTAssertEqual(
            bodyState["json_fields"] as? [String],
            ToriiParliamentAPIV1.bodyStateFields
        )
        let binding = try XCTUnwrap(fixture["certificate_body_binding"] as? [String: Any])
        XCTAssertEqual(
            binding["norito_field_order"] as? [String],
            ToriiParliamentAPIV1.certificateBodyBindingNoritoFields
        )
        let publicBody = try XCTUnwrap(binding["public_nonbinding_body"] as? [String: Any])
        XCTAssertEqual(
            publicBody["public_finding_norito_field_order"] as? [String],
            ToriiParliamentAPIV1.publicFindingCertificateNoritoFields
        )
        let release = try XCTUnwrap(fixture["tle_release_context"] as? [String: Any])
        XCTAssertEqual(
            release["transcript_public_state_fields"] as? [String],
            [
                "version", "key_session_id", "network_id", "roster_hash", "committee_size",
                "threshold", "generator_h", "generator_v", "qualified_dealers",
                "qualified_dealer_commitments", "dkg_event_hash", "group_public_key",
                "public_shares", "transcript_hash",
            ]
        )
        let partial = try XCTUnwrap(fixture["tle_partial_release"] as? [String: Any])
        XCTAssertEqual((partial["response_fields"] as? [String])?.count, 9)
    }

    func testBuildersEmitOnlyCanonicalVersionedFields() throws {
        let attempt = try ToriiParliamentAPIV1.attemptDraftRequestData(
            proposal: deployProposal(),
            attemptSequence: 7
        )
        let attemptObject = try XCTUnwrap(
            JSONSerialization.jsonObject(with: attempt) as? [String: Any]
        )
        XCTAssertEqual(Set(attemptObject.keys), ["version", "proposal", "attempt_sequence"])
        XCTAssertEqual(
            try ToriiParliamentAPIV1.attemptReadPath(governanceAttemptId: attemptID),
            "/v1/gov/parliament/attempts/\(attemptID)"
        )
        let ballotID = identifier(0x33)
        XCTAssertEqual(
            try ToriiParliamentAPIV1.timedOvnCastingContextReadPath(
                ballotAttemptId: ballotID
            ),
            "/v1/gov/parliament/ballots/\(ballotID)/casting-context"
        )
        XCTAssertEqual(
            try ToriiParliamentAPIV1.timedOvnCastingProofPath(ballotAttemptId: ballotID),
            "/v1/gov/parliament/ballots/\(ballotID)/casting-proof"
        )
        XCTAssertEqual(
            try ToriiParliamentAPIV1.tleReleaseContextReadPath(ballotAttemptId: ballotID),
            "/v1/gov/parliament/ballots/\(ballotID)/release-context"
        )
        XCTAssertEqual(
            try ToriiParliamentAPIV1.tlePartialReleasePath(ballotAttemptId: ballotID),
            "/v1/gov/parliament/ballots/\(ballotID)/partial-release"
        )
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.attemptReadPath(
                governanceAttemptId: attemptID.uppercased()
            )
        )

        let transition = try ToriiParliamentAPIV1.transitionDraftRequestData(
            governanceAttemptId: attemptID,
            transition: .failPublicFindingNoResult(bodyInstanceId: identifier(1))
        )
        let transitionObject = try XCTUnwrap(
            JSONSerialization.jsonObject(with: transition) as? [String: Any]
        )
        XCTAssertEqual(
            Set(transitionObject.keys),
            ["version", "governance_attempt_id", "transition"]
        )
        let tagged = try XCTUnwrap(transitionObject["transition"] as? [String: Any])
        XCTAssertEqual(tagged["transition"] as? String, "FailPublicFindingNoResult")
    }

    func testAttemptDraftRejectsRecognizedTagWithArbitraryPayload() {
        XCTAssertThrowsError(
            try ToriiParliamentProposalV1(
                validating: Data(
                    "{\"kind\":\"RuntimeUpgrade\",\"payload\":{}}".utf8
                )
            )
        )
        XCTAssertThrowsError(
            try ToriiParliamentProposalV1(
                validating: Data(
                    """
                    {"kind":"DeployContract","payload":{"contract_address":"\(Self.contractAddress)","code_hash":"\(String(repeating: "11", count: 32))","abi_hash":"\(String(repeating: "22", count: 32))","abi_version":1,"abi_version":1,"manifest_provenance":null}}
                    """.utf8
                )
            )
        )
    }

    func testReadProjectionValidatesScheduleNoResultAndExactSupporters() throws {
        let valid = try jsonData(makeReadResponse())
        let parsed = try ToriiParliamentAPIV1.decodeAttemptReadResponse(
            valid,
            expectedGovernanceAttemptId: attemptID
        )
        XCTAssertEqual(parsed.bodyStates.count, 1)
        XCTAssertEqual(parsed.bodyStates[0].body, "rules-committee")
        XCTAssertEqual(parsed.bodyStates[0].publicFindingDeadlineHeight, 8)
        XCTAssertEqual(
            parsed.publicFindingBindings[0].endorsingAssignments,
            [identifier(0x11), identifier(0x12)]
        )

        var wrongDeadline = makeReadResponse()
        var states = try XCTUnwrap(wrongDeadline["body_states"] as? [[String: Any]])
        states[0]["public_finding_deadline_height"] = 9
        wrongDeadline["body_states"] = states
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeAttemptReadResponse(
                jsonData(wrongDeadline),
                expectedGovernanceAttemptId: attemptID
            )
        )

        var automaticFailure = makeReadResponse()
        states = try XCTUnwrap(automaticFailure["body_states"] as? [[String: Any]])
        states[0]["status"] = ["status": "NoResult"]
        states[0]["no_result_kind"] = ["reason": "ExecutionFailed"]
        states[0]["no_result_height"] = 9
        automaticFailure["body_states"] = states
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeAttemptReadResponse(
                jsonData(automaticFailure),
                expectedGovernanceAttemptId: attemptID
            )
        )

        var unsorted = makeReadResponse(supporters: [identifier(0x12), identifier(0x11)])
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeAttemptReadResponse(
                jsonData(unsorted),
                expectedGovernanceAttemptId: attemptID
            )
        )
        unsorted["version"] = 2
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeAttemptReadResponse(
                jsonData(unsorted),
                expectedGovernanceAttemptId: attemptID
            )
        )
    }

    func testReleaseContextRequiresCompleteTranscriptAndBindsEveryPartial() throws {
        let ballotID = identifier(0x33)
        let response = makeTleReleaseContextResponse()
        let context = try ToriiParliamentAPIV1.decodeTleReleaseContextResponse(
            jsonData(response),
            expectedBallotAttemptId: ballotID
        )
        XCTAssertEqual(context.keySession.publicShares.count, 4)
        XCTAssertEqual(context.keySession.qualifiedDealerCommitments.count, 2)

        let partial = makeTlePartialReleaseResponse(context: response)
        let parsedPartial = try ToriiParliamentAPIV1.decodeTlePartialReleaseResponse(
            jsonData(partial),
            expectedKeySessionId: context.keySession.keySessionId,
            expectedIdentityDigest: context.identityDigest,
            committeeSize: context.keySession.committeeSize
        )
        XCTAssertEqual(parsedPartial.participantIndex, 1)

        var missingShare = makeTleReleaseContextResponse()
        var session = try XCTUnwrap(missingShare["tle_key_session"] as? [String: Any])
        var shares = try XCTUnwrap(session["public_shares"] as? [[String: Any]])
        shares.removeLast()
        session["public_shares"] = shares
        missingShare["tle_key_session"] = session
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeTleReleaseContextResponse(
                jsonData(missingShare),
                expectedBallotAttemptId: ballotID
            )
        )

        var wrongDigest = makeTleReleaseContextResponse()
        var digest = [UInt8](repeating: 0x88, count: 32)
        digest[0] = 1
        wrongDigest["identity_digest"] = digest
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeTleReleaseContextResponse(
                jsonData(wrongDigest),
                expectedBallotAttemptId: ballotID
            )
        )

        var crossBound = makeTlePartialReleaseResponse(context: response)
        crossBound["key_session_id"] = identifier(0x77)
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeTlePartialReleaseResponse(
                jsonData(crossBound),
                expectedKeySessionId: context.keySession.keySessionId,
                expectedIdentityDigest: context.identityDigest,
                committeeSize: context.keySession.committeeSize
            )
        )
    }

    func testCastingContextRequiresExactPublicCorpusAndCanonicalArchive() throws {
        let ballotID = identifier(0x33)
        let response = makeTimedOvnCastingContextResponse()
        let parsed = try ToriiParliamentAPIV1.decodeTimedOvnCastingContextResponse(
            jsonData(response),
            expectedBallotAttemptId: ballotID
        )
        XCTAssertEqual(parsed.phase, .registered)
        XCTAssertEqual(parsed.registrationRecordsHex.count, 1)
        XCTAssertEqual(parsed.archiveNorito, Data("NRT0".utf8))

        var unknown = response
        unknown["seed"] = [UInt8](repeating: 0x11, count: 32)
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeTimedOvnCastingContextResponse(
                jsonData(unknown),
                expectedBallotAttemptId: ballotID
            )
        )

        var emptyClosed = response
        emptyClosed["phase"] = "RegistrationClosed"
        emptyClosed["registration_records_hex"] = []
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeTimedOvnCastingContextResponse(
                jsonData(emptyClosed),
                expectedBallotAttemptId: ballotID
            )
        )

        var emptyFrozen = response
        let release = makeTleReleaseContextResponse()
        emptyFrozen["phase"] = "SurvivorsFrozen"
        emptyFrozen["survivor_participant_hashes"] = []
        emptyFrozen["release_identity"] = release["release_identity"]
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeTimedOvnCastingContextResponse(
                jsonData(emptyFrozen),
                expectedBallotAttemptId: ballotID
            )
        )

        var noncanonicalArchive = response
        noncanonicalArchive["archive_norito_base64"] = "TlJUMA"
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeTimedOvnCastingContextResponse(
                jsonData(noncanonicalArchive),
                expectedBallotAttemptId: ballotID
            )
        )
    }

    func testAutomaticOutcomeCannotMasqueradeAsPublicTransitionResponse() throws {
        let response: [String: Any] = [
            "version": 1,
            "governance_attempt_id": attemptID,
            "transition_kind": ["kind": "MarkEnacted"],
            "transition_digest": root,
            "tx_instructions": [[
                "wire_id": ToriiParliamentAPIV1.transitionSubmitWireId,
                "payload_hex": framedHex(),
            ]],
        ]
        XCTAssertThrowsError(
            try ToriiParliamentAPIV1.decodeTransitionDraftResponse(
                jsonData(response),
                expectedGovernanceAttemptId: attemptID,
                expectedTransitionKind: "CompleteQualification",
                expectedTransitionDigest: root
            )
        )
    }

    private func makeReadResponse(
        supporters: [String]? = nil
    ) -> [String: Any] {
        let exactSupporters = supporters ?? [identifier(0x11), identifier(0x12)]
        return [
            "version": 1,
            "current_height": 9,
            "attempt": [
                "id": attemptID,
                "proposal_content_id": proposalID,
                "sequence": 0,
                "risk_tier": ["tier": "Standard"],
                "stage": ["stage": "Rules"],
                "status": ["status": "Certified"],
            ],
            "policy_version": 1,
            "required_bodies": [[
                "body": "rules-committee",
                "decision_mode": ["mode": "PublicFinding"],
            ]],
            "body_states": [[
                "body": "rules-committee",
                "body_instance_id": identifier(1),
                "status": ["status": "Approved"],
                "public_finding_opened_at_height": 1,
                "public_finding_phase_blocks": 7,
                "public_finding_deadline_height": 8,
                "no_result_kind": NSNull(),
                "no_result_height": NSNull(),
            ]],
            "certificate": [
                "proposal_content_id": proposalID,
                "governance_attempt_id": attemptID,
                "governance_attempt_sequence": 0,
                "risk_tier": ["tier": "Standard"],
                "body_bindings": [[
                    "body_instance_id": identifier(1),
                    "election_attempt_id": identifier(2),
                    "election_attempt_sequence": 0,
                    "sortition_request_id": identifier(3),
                    "sortition_request": [
                        "id": identifier(3),
                        "governance_attempt_id": attemptID,
                        "body_election_attempt_id": identifier(2),
                        "body": "rules-committee",
                        "candidate_root": root,
                        "candidate_count": 3,
                        "target_seats": 3,
                        "request_height": 1,
                        "pulse_height": 2,
                        "beacon_session_id": identifier(4),
                    ],
                    "body": "rules-committee",
                    "original_seats": 3,
                    "beacon_session_id": identifier(4),
                    "beacon_pulse_id": identifier(5),
                    "roster_root": root,
                    "assignment_root": root,
                    "result_root": root,
                    "result_height": 8,
                    "public_finding": [
                        "endorsement_root": root,
                        "endorsing_assignments": exactSupporters,
                        "endorsements": 2,
                        "quorum": 2,
                    ],
                    "ballot": NSNull(),
                ]],
                "policy_version": 1,
                "effect_preimage_hash": root,
                "expected_head": ["state": "Absent", "head": ["subject_id": root]],
                "certified_at_height": 8,
                "enact_at_height": 10,
            ],
            "terminal_height": NSNull(),
            "execution_failure_root": NSNull(),
            "superseding_head": NSNull(),
            "state_payload_hex": framedHex(),
        ]
    }

    private func makeTleReleaseContextResponse() -> [String: Any] {
        let ballotID = identifier(0x33)
        let bodyID = identifier(0x22)
        let keySessionID = identifier(0x44)
        let survivorRoot = [UInt8](repeating: 0x61, count: 32)
        let noRecoveryRoot = [UInt8](repeating: 0x62, count: 32)
        let parameterHash = [UInt8](repeating: 0x63, count: 32)
        var identityPayload = Data("iroha.parliament.tle.identity-payload.v1\0".utf8)
        identityPayload.append(contentsOf: bigEndian16(1))
        identityPayload.append(Data(hexString: attemptID)!)
        identityPayload.append(Data(hexString: bodyID)!)
        identityPayload.append(Data(hexString: ballotID)!)
        identityPayload.append(contentsOf: survivorRoot)
        identityPayload.append(contentsOf: noRecoveryRoot)
        identityPayload.append(contentsOf: bigEndian64(40))
        identityPayload.append(contentsOf: parameterHash)
        XCTAssertEqual(identityPayload.count, 243)

        let networkID = [UInt8](repeating: 0x45, count: 32)
        let rosterHash = [UInt8](repeating: 0x46, count: 32)
        var dealers = [[String: Any]]()
        for dealer in [1, 2] {
            let entry: [String: Any] = [
                "dealer_index": dealer,
                "coefficient_commitments": [
                    [UInt8](repeating: UInt8(0x50 + dealer), count: 96),
                    [UInt8](repeating: UInt8(0x60 + dealer), count: 96),
                ],
                "constant_pok_commitment": [UInt8](
                    repeating: UInt8(0x70 + dealer), count: 96
                ),
                "constant_pok_response": [UInt8](
                    repeating: UInt8(0x80 + dealer), count: 32
                ),
            ]
            dealers.append(entry)
        }
        var shares = [[String: Any]]()
        for index in 1...4 {
            let entry: [String: Any] = [
                "index": index,
                "participant_hash": [UInt8](
                    repeating: UInt8(0x20 + index), count: 32
                ),
                "public_key_share": [UInt8](
                    repeating: UInt8(0x30 + index), count: 96
                ),
            ]
            shares.append(entry)
        }
        let keySession: [String: Any] = [
            "version": 1,
            "key_session_id": keySessionID,
            "network_id": networkID,
            "roster_hash": rosterHash,
            "committee_size": 4,
            "threshold": 2,
            "generator_h": [UInt8](repeating: 0x47, count: 96),
            "generator_v": [UInt8](repeating: 0x48, count: 96),
            "qualified_dealers": [1, 2],
            "qualified_dealer_commitments": dealers,
            "dkg_event_hash": [UInt8](repeating: 0x49, count: 32),
            "group_public_key": [UInt8](repeating: 0x4a, count: 96),
            "public_shares": shares,
            "transcript_hash": [UInt8](repeating: 0x4b, count: 32),
        ]
        var framed = Data("iroha.threshold-bls.message.v1\0".utf8)
        framed.append(contentsOf: "iroha.threshold-bls.session.v1\0".utf8)
        framed.append(contentsOf: bigEndian16(1))
        framed.append(2)
        framed.append(contentsOf: networkID)
        framed.append(Data(hexString: keySessionID)!)
        framed.append(contentsOf: rosterHash)
        framed.append(contentsOf: bigEndian16(4))
        framed.append(contentsOf: bigEndian16(2))
        framed.append(contentsOf: bigEndian32(UInt32(identityPayload.count)))
        framed.append(identityPayload)
        return [
            "version": 1,
            "current_height": 42,
            "ballot_attempt_id": ballotID,
            "governance_attempt_id": attemptID,
            "body_instance_id": bodyID,
            "status": ["status": "Opening"],
            "release_height": 40,
            "opening_deadline_height": 45,
            "tle_key_session": keySession,
            "release_identity": [
                "tle_key_session_id": keySessionID,
                "governance_attempt_id": attemptID,
                "body_instance_id": bodyID,
                "ballot_attempt_id": ballotID,
                "survivor_corpus_root": survivorRoot,
                "no_recovery_root": noRecoveryRoot,
                "target_finalized_height": 40,
                "parameter_hash": parameterHash,
            ],
            "identity_digest": [UInt8](SHA256.hash(data: framed)),
            "identity_payload_hex": identityPayload.hexEncodedString(),
        ]
    }

    private func makeTlePartialReleaseResponse(
        context: [String: Any]
    ) -> [String: Any] {
        let session = context["tle_key_session"] as! [String: Any]
        return [
            "key_session_id": session["key_session_id"]!,
            "identity_digest": context["identity_digest"]!,
            "participant_index": 1,
            "sigma": [UInt8](repeating: 0x91, count: 48),
            "proof_x": [UInt8](repeating: 0x92, count: 96),
            "proof_y": [UInt8](repeating: 0x93, count: 48),
            "z_s": [UInt8](repeating: 0x94, count: 32),
            "z_r": [UInt8](repeating: 0x95, count: 32),
            "z_u": [UInt8](repeating: 0x96, count: 32),
        ]
    }

    private func makeTimedOvnCastingContextResponse() -> [String: Any] {
        let release = makeTleReleaseContextResponse()
        let keySession = release["tle_key_session"] as! [String: Any]
        let identity = release["release_identity"] as! [String: Any]
        return [
            "version": 1,
            "current_height": 30,
            "phase": "Registered",
            "session": [
                "network_id": keySession["network_id"]!,
                "proposal_content_id": proposalID,
                "governance_attempt_id": attemptID,
                "body_instance_id": release["body_instance_id"]!,
                "ballot_attempt_id": release["ballot_attempt_id"]!,
                "parameter_hash": identity["parameter_hash"]!,
                "tle_key_session_id": keySession["key_session_id"]!,
                "tle_key_transcript_hash": keySession["transcript_hash"]!,
                "tle_master_public_key": keySession["group_public_key"]!,
            ],
            "registration_opened_at_finalized_height": 20,
            "target_finalized_height": 40,
            "tle_key_session": keySession,
            "registration_records_hex": [String(repeating: "81", count: 3_624)],
            "survivor_participant_hashes": NSNull(),
            "release_identity": NSNull(),
            "archive_norito_base64": Data("NRT0".utf8).base64EncodedString(),
        ]
    }

    private func bigEndian16(_ value: UInt16) -> [UInt8] {
        [UInt8(value >> 8), UInt8(value & 0xff)]
    }

    private func bigEndian32(_ value: UInt32) -> [UInt8] {
        [
            UInt8((value >> 24) & 0xff), UInt8((value >> 16) & 0xff),
            UInt8((value >> 8) & 0xff), UInt8(value & 0xff),
        ]
    }

    private func bigEndian64(_ value: UInt64) -> [UInt8] {
        (0 ..< 8).map { offset in UInt8((value >> UInt64(8 * (7 - offset))) & 0xff) }
    }

    private func framedHex() -> String {
        noritoEncode(
            typeName: "iroha.governance.parliament.swift.fixture.v1",
            payload: Data([1, 2])
        ).hexEncodedString()
    }

    private func identifier(_ byte: UInt8) -> String {
        String(repeating: String(format: "%02x", byte), count: 32)
    }

    private func jsonData(_ value: Any) throws -> Data {
        try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    }

    private func fixtureObject() throws -> [String: Any] {
        var root = URL(fileURLWithPath: #filePath)
        for _ in 0..<4 {
            root.deleteLastPathComponent()
        }
        let fixture = root.appendingPathComponent(
            "fixtures/governance/parliament_api_v1.json"
        )
        return try XCTUnwrap(
            JSONSerialization.jsonObject(with: Data(contentsOf: fixture)) as? [String: Any]
        )
    }
}
