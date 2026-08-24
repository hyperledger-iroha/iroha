import CryptoKit
import Foundation

/// Stable wire/JSON/event mapping for one public Parliament lifecycle transition.
public struct ToriiParliamentTransitionLayoutV1: Sendable, Equatable {
    public let noritoIndex: UInt8
    public let jsonTag: String
    public let jsonPayloadRequired: Bool
    public let eventKindIndex: UInt8
}

/// Stable wire/JSON/event mapping for one consensus-owned automatic outcome.
///
/// These values are intentionally audit-only. `ToriiParliamentLifecycleTransitionV1`
/// has no case that can submit one of these outcomes.
public struct ToriiParliamentAutomaticOutcomeLayoutV1: Sendable, Equatable {
    public let noritoIndex: UInt8
    public let jsonTag: String
    public let jsonPayloadRequired: Bool
    public let eventKind: String
    public let eventKindIndex: UInt8
}

/// Stable Norito/JSON mapping for one closed body no-result audit class.
public struct ToriiParliamentNoResultKindLayoutV1: Sendable, Equatable {
    public let noritoIndex: UInt8
    public let jsonTag: String
}

/// One recursively validated member of the closed Parliament V1 proposal inventory.
///
/// The wire value is private and can only be created after the shared typed governance
/// decoder has validated the complete nested payload. This deliberately removes the old
/// public `ToriiJSONValue` cases that allowed callers to submit an arbitrary object under
/// an otherwise recognized outer tag.
public struct ToriiParliamentProposalV1: Sendable, Equatable, Encodable {
    /// Typed proposal projection produced by the same strict decoder used for reads.
    public let kind: ToriiGovernanceProposalKind

    private let wireValue: ToriiJSONValue

    /// Validate one exact seven-kind proposal wire value before it can enter a draft request.
    public init(validating data: Data) throws {
        do {
            try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(in: data)
        } catch {
            throw ToriiClientError.invalidPayload(
                "proposal must be valid UTF-8 JSON without duplicate object keys."
            )
        }
        let decoder = JSONDecoder()
        kind = try decoder.decode(ToriiGovernanceProposalKind.self, from: data)
        wireValue = try decoder.decode(ToriiJSONValue.self, from: data)
        try governanceRequireExactJSONIntegers(
            wireValue,
            codingPath: [],
            context: "proposal"
        )
        try ToriiParliamentAPIV1.rejectSigningMaterial(wireValue, context: "proposal")
    }

    public func encode(to encoder: Encoder) throws {
        try wireValue.encode(to: encoder)
    }
}

/// Exact release data attached to `FinalizeOpenedBallot`.
public struct ToriiParliamentFinalReleaseV1: Sendable, Equatable, Encodable {
    public let keySessionId: String
    public let identityDigest: [UInt8]
    public let signature: [UInt8]

    public init(keySessionId: String, identityDigest: [UInt8], signature: [UInt8]) {
        self.keySessionId = keySessionId
        self.identityDigest = identityDigest
        self.signature = signature
    }

    enum CodingKeys: String, CodingKey {
        case keySessionId = "key_session_id"
        case identityDigest = "identity_digest"
        case signature
    }
}

/// The closed, submit-able V1 lifecycle transition set.
///
/// Consensus-owned certificate construction and automatic execution outcomes
/// are deliberately absent from this enum.
public enum ToriiParliamentLifecycleTransitionV1: Sendable, Equatable, Encodable {
    case escalateRisk(target: ToriiJSONValue)
    case completeQualification
    case registerSortitionRequest(sequence: UInt32, request: ToriiJSONValue, candidateSnapshot: [ToriiJSONValue])
    case consumeSortitionPulseBatch(requestIds: [String], beaconSessionId: String, pulseHeight: UInt64, pulseId: String)
    case beginInvitationAcceptance(electionAttemptId: String)
    case failBodyElectionNoRoster(electionAttemptId: String)
    case sealBodyRoster(electionAttemptId: String)
    case advanceBodyPhase(bodyInstanceId: String, target: ToriiJSONValue)
    case recordAttemptAbsence(bodyInstanceId: String, assignmentId: String)
    case endorsePublicFinding(bodyInstanceId: String, resultRoot: [UInt8])
    case registerBallotAttempt(
        bodyInstanceId: String,
        ballotAttemptId: String,
        sequence: UInt32,
        tleSessionId: String,
        tleKeySessionId: String,
        releaseBeaconSessionId: String,
        releaseHeight: UInt64
    )
    case closeBallotRegistration(ballotAttemptId: String)
    case freezeBallotSurvivors(ballotAttemptId: String)
    case freezeTimedOvnCorpus(ballotAttemptId: String, ballotRecords: [[UInt8]])
    case beginBallotOpeningBatch(
        ballotAttemptIds: [String],
        releaseBeaconSessionId: String,
        releaseHeight: UInt64,
        pulseId: String
    )
    case failBallotNoResult(ballotAttemptId: String)
    case finalizeOpenedBallot(ballotAttemptId: String, finalRelease: ToriiParliamentFinalReleaseV1)
    case recordInvitationResponse(electionAttemptId: String, body: String, decision: String)
    case registerBallotParticipant(ballotAttemptId: String, registrationRecord: [UInt8])
    case recordBallotDropout(ballotAttemptId: String)
    case failPublicFindingNoResult(bodyInstanceId: String)

    public var layout: ToriiParliamentTransitionLayoutV1 {
        switch self {
        case .escalateRisk: ToriiParliamentAPIV1.publicTransitions[0]
        case .completeQualification: ToriiParliamentAPIV1.publicTransitions[1]
        case .registerSortitionRequest: ToriiParliamentAPIV1.publicTransitions[2]
        case .consumeSortitionPulseBatch: ToriiParliamentAPIV1.publicTransitions[3]
        case .beginInvitationAcceptance: ToriiParliamentAPIV1.publicTransitions[4]
        case .failBodyElectionNoRoster: ToriiParliamentAPIV1.publicTransitions[5]
        case .sealBodyRoster: ToriiParliamentAPIV1.publicTransitions[6]
        case .advanceBodyPhase: ToriiParliamentAPIV1.publicTransitions[7]
        case .recordAttemptAbsence: ToriiParliamentAPIV1.publicTransitions[8]
        case .endorsePublicFinding: ToriiParliamentAPIV1.publicTransitions[9]
        case .registerBallotAttempt: ToriiParliamentAPIV1.publicTransitions[10]
        case .closeBallotRegistration: ToriiParliamentAPIV1.publicTransitions[11]
        case .freezeBallotSurvivors: ToriiParliamentAPIV1.publicTransitions[12]
        case .freezeTimedOvnCorpus: ToriiParliamentAPIV1.publicTransitions[13]
        case .beginBallotOpeningBatch: ToriiParliamentAPIV1.publicTransitions[14]
        case .failBallotNoResult: ToriiParliamentAPIV1.publicTransitions[15]
        case .finalizeOpenedBallot: ToriiParliamentAPIV1.publicTransitions[16]
        case .recordInvitationResponse: ToriiParliamentAPIV1.publicTransitions[17]
        case .registerBallotParticipant: ToriiParliamentAPIV1.publicTransitions[18]
        case .recordBallotDropout: ToriiParliamentAPIV1.publicTransitions[19]
        case .failPublicFindingNoResult: ToriiParliamentAPIV1.publicTransitions[20]
        }
    }

    public func encode(to encoder: Encoder) throws {
        try validate()
        var root = encoder.container(keyedBy: ToriiParliamentCodingKey.self)
        try root.encode(layout.jsonTag, forKey: .init("transition"))
        guard layout.jsonPayloadRequired else { return }
        var payload = root.nestedContainer(
            keyedBy: ToriiParliamentCodingKey.self,
            forKey: .init("payload")
        )
        switch self {
        case let .escalateRisk(target):
            try payload.encode(target, forKey: .init("target"))
        case .completeQualification:
            break
        case let .registerSortitionRequest(sequence, request, candidateSnapshot):
            try payload.encode(sequence, forKey: .init("sequence"))
            try payload.encode(request, forKey: .init("request"))
            try payload.encode(candidateSnapshot, forKey: .init("candidate_snapshot"))
        case let .consumeSortitionPulseBatch(requestIds, beaconSessionId, pulseHeight, pulseId):
            try payload.encode(requestIds, forKey: .init("request_ids"))
            try payload.encode(beaconSessionId, forKey: .init("beacon_session_id"))
            try payload.encode(pulseHeight, forKey: .init("pulse_height"))
            try payload.encode(pulseId, forKey: .init("pulse_id"))
        case let .beginInvitationAcceptance(electionAttemptId),
             let .failBodyElectionNoRoster(electionAttemptId),
             let .sealBodyRoster(electionAttemptId):
            try payload.encode(electionAttemptId, forKey: .init("election_attempt_id"))
        case let .advanceBodyPhase(bodyInstanceId, target):
            try payload.encode(bodyInstanceId, forKey: .init("body_instance_id"))
            try payload.encode(target, forKey: .init("target"))
        case let .recordAttemptAbsence(bodyInstanceId, assignmentId):
            try payload.encode(bodyInstanceId, forKey: .init("body_instance_id"))
            try payload.encode(assignmentId, forKey: .init("assignment_id"))
        case let .endorsePublicFinding(bodyInstanceId, resultRoot):
            try payload.encode(bodyInstanceId, forKey: .init("body_instance_id"))
            try payload.encode(resultRoot, forKey: .init("result_root"))
        case let .registerBallotAttempt(
            bodyInstanceId,
            ballotAttemptId,
            sequence,
            tleSessionId,
            tleKeySessionId,
            releaseBeaconSessionId,
            releaseHeight
        ):
            try payload.encode(bodyInstanceId, forKey: .init("body_instance_id"))
            try payload.encode(ballotAttemptId, forKey: .init("ballot_attempt_id"))
            try payload.encode(sequence, forKey: .init("sequence"))
            try payload.encode(tleSessionId, forKey: .init("tle_session_id"))
            try payload.encode(tleKeySessionId, forKey: .init("tle_key_session_id"))
            try payload.encode(releaseBeaconSessionId, forKey: .init("release_beacon_session_id"))
            try payload.encode(releaseHeight, forKey: .init("release_height"))
        case let .closeBallotRegistration(ballotAttemptId),
             let .freezeBallotSurvivors(ballotAttemptId),
             let .failBallotNoResult(ballotAttemptId),
             let .recordBallotDropout(ballotAttemptId):
            try payload.encode(ballotAttemptId, forKey: .init("ballot_attempt_id"))
        case let .freezeTimedOvnCorpus(ballotAttemptId, ballotRecords):
            try payload.encode(ballotAttemptId, forKey: .init("ballot_attempt_id"))
            try payload.encode(ballotRecords, forKey: .init("ballot_records"))
        case let .beginBallotOpeningBatch(
            ballotAttemptIds,
            releaseBeaconSessionId,
            releaseHeight,
            pulseId
        ):
            try payload.encode(ballotAttemptIds, forKey: .init("ballot_attempt_ids"))
            try payload.encode(releaseBeaconSessionId, forKey: .init("release_beacon_session_id"))
            try payload.encode(releaseHeight, forKey: .init("release_height"))
            try payload.encode(pulseId, forKey: .init("pulse_id"))
        case let .finalizeOpenedBallot(ballotAttemptId, finalRelease):
            try payload.encode(ballotAttemptId, forKey: .init("ballot_attempt_id"))
            try payload.encode(finalRelease, forKey: .init("final_release"))
        case let .recordInvitationResponse(electionAttemptId, body, decision):
            try payload.encode(electionAttemptId, forKey: .init("election_attempt_id"))
            try payload.encode(body, forKey: .init("body"))
            var encodedDecision = payload.nestedContainer(
                keyedBy: ToriiParliamentCodingKey.self,
                forKey: .init("decision")
            )
            try encodedDecision.encode(decision, forKey: .init("decision"))
        case let .registerBallotParticipant(ballotAttemptId, registrationRecord):
            try payload.encode(ballotAttemptId, forKey: .init("ballot_attempt_id"))
            try payload.encode(registrationRecord, forKey: .init("registration_record"))
        case let .failPublicFindingNoResult(bodyInstanceId):
            try payload.encode(bodyInstanceId, forKey: .init("body_instance_id"))
        }
    }

    private func validate() throws {
        switch self {
        case let .escalateRisk(target):
            try ToriiParliamentAPIV1.rejectSigningMaterial(target, context: "EscalateRisk.target")
        case .completeQualification:
            break
        case let .registerSortitionRequest(_, request, candidateSnapshot):
            guard !candidateSnapshot.isEmpty,
                  candidateSnapshot.count <= ToriiParliamentAPIV1.maximumCorpusEntries else {
                throw ToriiClientError.invalidPayload(
                    "candidate_snapshot must contain one through 1000 entries."
                )
            }
            try ToriiParliamentAPIV1.rejectSigningMaterial(request, context: "sortition request")
        case let .consumeSortitionPulseBatch(requestIds, beaconSessionId, _, pulseId):
            try ToriiParliamentAPIV1.requireStrictIdentifiers(requestIds, field: "request_ids")
            _ = try ToriiParliamentAPIV1.requireIdentifier(beaconSessionId, field: "beacon_session_id")
            _ = try ToriiParliamentAPIV1.requireIdentifier(pulseId, field: "pulse_id")
        case let .beginInvitationAcceptance(id),
             let .failBodyElectionNoRoster(id),
             let .sealBodyRoster(id):
            _ = try ToriiParliamentAPIV1.requireIdentifier(id, field: "election_attempt_id")
        case let .advanceBodyPhase(id, target):
            _ = try ToriiParliamentAPIV1.requireIdentifier(id, field: "body_instance_id")
            try ToriiParliamentAPIV1.rejectSigningMaterial(target, context: "body phase target")
        case let .recordAttemptAbsence(bodyId, assignmentId):
            _ = try ToriiParliamentAPIV1.requireIdentifier(bodyId, field: "body_instance_id")
            _ = try ToriiParliamentAPIV1.requireIdentifier(assignmentId, field: "assignment_id")
        case let .endorsePublicFinding(bodyId, resultRoot):
            _ = try ToriiParliamentAPIV1.requireIdentifier(bodyId, field: "body_instance_id")
            try ToriiParliamentAPIV1.requireFixedBytes(resultRoot, count: 32, nonzero: true, field: "result_root")
        case let .registerBallotAttempt(
            bodyId,
            ballotId,
            _,
            tleSessionId,
            tleKeySessionId,
            releaseBeaconSessionId,
            _
        ):
            for (value, field) in [
                (bodyId, "body_instance_id"),
                (ballotId, "ballot_attempt_id"),
                (tleSessionId, "tle_session_id"),
                (tleKeySessionId, "tle_key_session_id"),
                (releaseBeaconSessionId, "release_beacon_session_id"),
            ] {
                _ = try ToriiParliamentAPIV1.requireIdentifier(value, field: field)
            }
        case let .closeBallotRegistration(id),
             let .freezeBallotSurvivors(id),
             let .failBallotNoResult(id),
             let .recordBallotDropout(id):
            _ = try ToriiParliamentAPIV1.requireIdentifier(id, field: "ballot_attempt_id")
        case let .freezeTimedOvnCorpus(id, records):
            _ = try ToriiParliamentAPIV1.requireIdentifier(id, field: "ballot_attempt_id")
            guard !records.isEmpty,
                  records.count <= ToriiParliamentAPIV1.maximumCorpusEntries else {
                throw ToriiClientError.invalidPayload(
                    "ballot_records must contain one through 1000 records."
                )
            }
            for (index, record) in records.enumerated() {
                try ToriiParliamentAPIV1.requireFixedBytes(
                    record,
                    count: ToriiParliamentAPIV1.timedOvnBallotRecordBytes,
                    nonzero: false,
                    field: "ballot_records[\(index)]"
                )
            }
        case let .beginBallotOpeningBatch(ids, releaseBeaconSessionId, _, pulseId):
            try ToriiParliamentAPIV1.requireStrictIdentifiers(ids, field: "ballot_attempt_ids")
            _ = try ToriiParliamentAPIV1.requireIdentifier(
                releaseBeaconSessionId,
                field: "release_beacon_session_id"
            )
            _ = try ToriiParliamentAPIV1.requireIdentifier(pulseId, field: "pulse_id")
        case let .finalizeOpenedBallot(id, release):
            _ = try ToriiParliamentAPIV1.requireIdentifier(id, field: "ballot_attempt_id")
            _ = try ToriiParliamentAPIV1.requireIdentifier(
                release.keySessionId,
                field: "final_release.key_session_id"
            )
            try ToriiParliamentAPIV1.requireFixedBytes(
                release.identityDigest,
                count: 32,
                nonzero: true,
                field: "final_release.identity_digest"
            )
            try ToriiParliamentAPIV1.requireFixedBytes(
                release.signature,
                count: 48,
                nonzero: true,
                field: "final_release.signature"
            )
        case let .recordInvitationResponse(id, body, decision):
            _ = try ToriiParliamentAPIV1.requireIdentifier(id, field: "election_attempt_id")
            try ToriiParliamentAPIV1.requireBody(body, field: "body")
            guard decision == "Accept" || decision == "Decline" else {
                throw ToriiClientError.invalidPayload("decision must be Accept or Decline.")
            }
        case let .registerBallotParticipant(id, record):
            _ = try ToriiParliamentAPIV1.requireIdentifier(id, field: "ballot_attempt_id")
            try ToriiParliamentAPIV1.requireFixedBytes(
                record,
                count: ToriiParliamentAPIV1.timedOvnRegistrationRecordBytes,
                nonzero: false,
                field: "registration_record"
            )
        case let .failPublicFindingNoResult(id):
            _ = try ToriiParliamentAPIV1.requireIdentifier(id, field: "body_instance_id")
        }
    }
}

/// One canonical native instruction returned by a Parliament draft route.
public struct ToriiParliamentInstructionDraftV1: Sendable, Equatable {
    public let wireId: String
    public let payloadHex: String
}

/// Strict result from the attempt-draft route.
public struct ToriiParliamentAttemptDraftResponseV1: Sendable, Equatable {
    public let proposalContentId: String
    public let governanceAttemptId: String
    public let instruction: ToriiParliamentInstructionDraftV1
}

/// Strict result from the lifecycle-transition draft route.
public struct ToriiParliamentTransitionDraftResponseV1: Sendable, Equatable {
    public let governanceAttemptId: String
    public let transitionKind: String
    public let transitionDigest: [UInt8]
    public let instruction: ToriiParliamentInstructionDraftV1
}

/// Exact authority-authenticated supporter list included in a public-finding certificate.
public struct ToriiParliamentPublicFindingCertificateBindingV1: Sendable, Equatable {
    public let endorsementRoot: [UInt8]
    public let endorsingAssignments: [String]
    public let endorsements: UInt32
    public let quorum: UInt32
}

/// Bounded public lifecycle/deadline projection for one required Parliament body.
///
/// Private-ballot registrations, records, roots, shares, and openings are never
/// represented here; only the lifecycle and closed terminal failure class are public.
public struct ToriiParliamentBodyStateProjectionV1: Sendable, Equatable {
    public let body: String
    public let bodyInstanceId: String?
    public let status: String?
    public let deliberationPhase: String?
    public let publicFindingOpenedAtHeight: UInt64?
    public let publicFindingPhaseBlocks: UInt64?
    public let publicFindingDeadlineHeight: UInt64?
    public let noResultKind: String?
    public let noResultHeight: UInt64?
}

/// Strict outer projection from the authenticated attempt-read route.
public struct ToriiParliamentAttemptReadResponseV1: Sendable, Equatable {
    public let governanceAttemptId: String
    public let currentHeight: UInt64
    public let policyVersion: UInt64
    public let terminalHeight: UInt64?
    public let executionFailureRoot: [UInt8]?
    public let statePayloadHex: String
    public let bodyStates: [ToriiParliamentBodyStateProjectionV1]
    public let publicFindingBindings: [ToriiParliamentPublicFindingCertificateBindingV1]
    public let rawJSON: Data
}

/// Proof-carrying public broadcast for one qualified adaptive TLE dealer.
public struct ToriiParliamentTleAdaptiveDealerCommitmentV1: Sendable, Equatable {
    public let dealerIndex: UInt32
    public let coefficientCommitments: [[UInt8]]
    public let constantPokCommitment: [UInt8]
    public let constantPokResponse: [UInt8]
}

/// Public composite verification share for one threshold participant.
public struct ToriiParliamentTleAdaptivePublicShareV1: Sendable, Equatable {
    public let index: UInt32
    public let participantHash: [UInt8]
    public let publicKeyShare: [UInt8]
}

/// Complete bounded public transcript required to verify adaptive partial releases.
public struct ToriiParliamentTleKeySessionPublicStateV1: Sendable, Equatable {
    public let keySessionId: String
    public let networkId: [UInt8]
    public let rosterHash: [UInt8]
    public let committeeSize: UInt32
    public let threshold: UInt32
    public let generatorH: [UInt8]
    public let generatorV: [UInt8]
    public let qualifiedDealers: [UInt32]
    public let qualifiedDealerCommitments: [ToriiParliamentTleAdaptiveDealerCommitmentV1]
    public let dkgEventHash: [UInt8]
    public let groupPublicKey: [UInt8]
    public let publicShares: [ToriiParliamentTleAdaptivePublicShareV1]
    public let transcriptHash: [UInt8]
}

/// Exact frozen public timed-OVN future release identity.
public struct ToriiParliamentTimedOvnReleaseIdentityProjectionV1: Sendable, Equatable {
    public let tleKeySessionId: String
    public let governanceAttemptId: String
    public let bodyInstanceId: String
    public let ballotAttemptId: String
    public let survivorCorpusRoot: [UInt8]
    public let noRecoveryRoot: [UInt8]
    public let targetFinalizedHeight: UInt64
    public let parameterHash: [UInt8]
}

/// Exact cast-capable public timed-OVN phase.
public enum ToriiParliamentTimedOvnCastingPhaseV1: String, Sendable, Equatable {
    case registered = "Registered"
    case registrationClosed = "RegistrationClosed"
    case survivorsFrozen = "SurvivorsFrozen"
}

/// Immutable public timed-OVN wallet-session bindings.
public struct ToriiParliamentTimedOvnSessionProjectionV1: Sendable, Equatable {
    public let networkId: [UInt8]
    public let proposalContentId: String
    public let governanceAttemptId: String
    public let bodyInstanceId: String
    public let ballotAttemptId: String
    public let parameterHash: [UInt8]
    public let tleKeySessionId: String
    public let tleKeyTranscriptHash: [UInt8]
    public let tleMasterPublicKey: [UInt8]
}

/// Replay-validated public context consumed by a secret-local native wallet bridge.
public struct ToriiParliamentTimedOvnCastingContextResponseV1: Sendable, Equatable {
    public let currentHeight: UInt64
    public let phase: ToriiParliamentTimedOvnCastingPhaseV1
    public let session: ToriiParliamentTimedOvnSessionProjectionV1
    public let registrationOpenedAtFinalizedHeight: UInt64
    public let targetFinalizedHeight: UInt64
    public let keySession: ToriiParliamentTleKeySessionPublicStateV1
    public let registrationRecordsHex: [String]
    public let survivorParticipantHashes: [[UInt8]]?
    public let releaseIdentity: ToriiParliamentTimedOvnReleaseIdentityProjectionV1?
    /// Complete canonical framed `ParliamentTimedOvnCastingContextArchiveV1`.
    public let archiveNorito: Data
}

/// Core-authorized release context available only during the inclusive Opening window.
public struct ToriiParliamentTleReleaseContextResponseV1: Sendable, Equatable {
    public let currentHeight: UInt64
    public let ballotAttemptId: String
    public let governanceAttemptId: String
    public let bodyInstanceId: String
    public let releaseHeight: UInt64
    public let openingDeadlineHeight: UInt64
    public let keySession: ToriiParliamentTleKeySessionPublicStateV1
    public let releaseIdentity: ToriiParliamentTimedOvnReleaseIdentityProjectionV1
    public let identityDigest: [UInt8]
    public let identityPayloadHex: String
}

/// One independently verifiable public adaptive partial release.
public struct ToriiParliamentTlePartialReleaseShareV1: Sendable, Equatable {
    public let keySessionId: String
    public let identityDigest: [UInt8]
    public let participantIndex: UInt32
    public let sigma: [UInt8]
    public let proofX: [UInt8]
    public let proofY: [UInt8]
    public let zS: [UInt8]
    public let zR: [UInt8]
    public let zU: [UInt8]
}

private struct ToriiParliamentCodingKey: CodingKey {
    let stringValue: String
    let intValue: Int? = nil

    init(_ stringValue: String) {
        self.stringValue = stringValue
    }

    init?(stringValue: String) {
        self.init(stringValue)
    }

    init?(intValue: Int) {
        return nil
    }
}

/// Canonical constants, request builders, and strict response admission for Parliament API V1.
public enum ToriiParliamentAPIV1 {
    public static let version: UInt16 = 1
    public static let attemptDraftPath = "/v1/gov/parliament/attempts/draft"
    public static let attemptReadPathTemplate =
        "/v1/gov/parliament/attempts/{governance_attempt_id}"
    public static let timedOvnCastingContextReadPathTemplate =
        "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-context"
    public static let timedOvnCastingProofPathTemplate =
        "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-proof"
    public static let tleReleaseContextReadPathTemplate =
        "/v1/gov/parliament/ballots/{ballot_attempt_id}/release-context"
    public static let tlePartialReleasePathTemplate =
        "/v1/gov/parliament/ballots/{ballot_attempt_id}/partial-release"
    public static let transitionDraftPath = "/v1/gov/parliament/transitions/draft"
    public static let attemptCreateWireId =
        "iroha.governance.parliament.attempt.create.v1"
    public static let transitionSubmitWireId =
        "iroha.governance.parliament.transition.submit.v1"
    public static let maximumAttemptStateBytes = 16 * 1024 * 1024
    public static let maximumTimedOvnCastingContextArchiveBytes = 4 * 1024 * 1024
    public static let maximumTimedOvnCastingProofResponseBytes = 8 * 1024 * 1024
    public static let maximumTleCommitteeSize: UInt32 = 31
    public static let timedOvnRegistrationRecordBytes = 3_624
    public static let timedOvnBallotRecordBytes = 2_858
    public static let maximumCorpusEntries = 1_000

    public static let publicTransitionDigestDomain =
        "iroha.governance.parliament.lifecycle_transition.digest.v1"
    public static let automaticOutcomeDigestDomain =
        "iroha.governance.parliament.automatic_execution_outcome.digest.v1"

    public static let publicTransitions: [ToriiParliamentTransitionLayoutV1] = [
        .init(noritoIndex: 0, jsonTag: "EscalateRisk", jsonPayloadRequired: true, eventKindIndex: 0),
        .init(noritoIndex: 1, jsonTag: "CompleteQualification", jsonPayloadRequired: false, eventKindIndex: 1),
        .init(noritoIndex: 2, jsonTag: "RegisterSortitionRequest", jsonPayloadRequired: true, eventKindIndex: 2),
        .init(noritoIndex: 3, jsonTag: "ConsumeSortitionPulseBatch", jsonPayloadRequired: true, eventKindIndex: 3),
        .init(noritoIndex: 4, jsonTag: "BeginInvitationAcceptance", jsonPayloadRequired: true, eventKindIndex: 4),
        .init(noritoIndex: 5, jsonTag: "FailBodyElectionNoRoster", jsonPayloadRequired: true, eventKindIndex: 5),
        .init(noritoIndex: 6, jsonTag: "SealBodyRoster", jsonPayloadRequired: true, eventKindIndex: 6),
        .init(noritoIndex: 7, jsonTag: "AdvanceBodyPhase", jsonPayloadRequired: true, eventKindIndex: 7),
        .init(noritoIndex: 8, jsonTag: "RecordAttemptAbsence", jsonPayloadRequired: true, eventKindIndex: 8),
        .init(noritoIndex: 9, jsonTag: "EndorsePublicFinding", jsonPayloadRequired: true, eventKindIndex: 9),
        .init(noritoIndex: 10, jsonTag: "RegisterBallotAttempt", jsonPayloadRequired: true, eventKindIndex: 10),
        .init(noritoIndex: 11, jsonTag: "CloseBallotRegistration", jsonPayloadRequired: true, eventKindIndex: 11),
        .init(noritoIndex: 12, jsonTag: "FreezeBallotSurvivors", jsonPayloadRequired: true, eventKindIndex: 12),
        .init(noritoIndex: 13, jsonTag: "FreezeTimedOvnCorpus", jsonPayloadRequired: true, eventKindIndex: 13),
        .init(noritoIndex: 14, jsonTag: "BeginBallotOpeningBatch", jsonPayloadRequired: true, eventKindIndex: 14),
        .init(noritoIndex: 15, jsonTag: "FailBallotNoResult", jsonPayloadRequired: true, eventKindIndex: 15),
        .init(noritoIndex: 16, jsonTag: "FinalizeOpenedBallot", jsonPayloadRequired: true, eventKindIndex: 16),
        .init(noritoIndex: 17, jsonTag: "RecordInvitationResponse", jsonPayloadRequired: true, eventKindIndex: 20),
        .init(noritoIndex: 18, jsonTag: "RegisterBallotParticipant", jsonPayloadRequired: true, eventKindIndex: 21),
        .init(noritoIndex: 19, jsonTag: "RecordBallotDropout", jsonPayloadRequired: true, eventKindIndex: 22),
        .init(noritoIndex: 20, jsonTag: "FailPublicFindingNoResult", jsonPayloadRequired: true, eventKindIndex: 23),
    ]

    public static let automaticExecutionOutcomes: [ToriiParliamentAutomaticOutcomeLayoutV1] = [
        .init(
            noritoIndex: 0,
            jsonTag: "Enacted",
            jsonPayloadRequired: false,
            eventKind: "MarkEnacted",
            eventKindIndex: 17
        ),
        .init(
            noritoIndex: 1,
            jsonTag: "Superseded",
            jsonPayloadRequired: true,
            eventKind: "MarkSuperseded",
            eventKindIndex: 18
        ),
        .init(
            noritoIndex: 2,
            jsonTag: "ExecutionFailed",
            jsonPayloadRequired: true,
            eventKind: "MarkExecutionFailed",
            eventKindIndex: 19
        ),
    ]

    public static let noResultKinds: [ToriiParliamentNoResultKindLayoutV1] = [
        .init(noritoIndex: 0, jsonTag: "PublicFindingQuorumUnreachable"),
        .init(noritoIndex: 1, jsonTag: "PublicFindingDeadlineExpired"),
        .init(noritoIndex: 2, jsonTag: "BallotRegistrationDeadlineExpired"),
        .init(noritoIndex: 3, jsonTag: "BallotSurvivorDeadlineExpired"),
        .init(noritoIndex: 4, jsonTag: "BallotCommitmentDeadlineExpired"),
        .init(noritoIndex: 5, jsonTag: "BallotReleasePulseUnavailable"),
        .init(noritoIndex: 6, jsonTag: "BallotOpeningDeadlineExpired"),
    ]

    public static let bodyStateFields = [
        "body",
        "body_instance_id",
        "status",
        "public_finding_opened_at_height",
        "public_finding_phase_blocks",
        "public_finding_deadline_height",
        "no_result_kind",
        "no_result_height",
    ]

    public static let certificateBodyBindingNoritoFields = [
        "body_instance_id",
        "election_attempt_id",
        "election_attempt_sequence",
        "sortition_request_id",
        "sortition_request",
        "body",
        "original_seats",
        "beacon_session_id",
        "beacon_pulse_id",
        "roster_root",
        "assignment_root",
        "result_root",
        "result_height",
        "public_finding",
        "ballot",
    ]

    public static let publicFindingCertificateNoritoFields = [
        "endorsement_root",
        "endorsing_assignments",
        "endorsements",
        "quorum",
    ]

    private static let bodies: Set<String> = [
        "rules-committee",
        "agenda-council",
        "interest-panel",
        "review-panel",
        "coordination-council",
        "mpc-committee",
        "fma-committee",
        "oversight-committee",
        "policy-jury",
        "confirmation-jury",
    ]
    private static let privateBodies: Set<String> = ["policy-jury", "confirmation-jury"]
    private static let riskTiers: Set<String> = [
        "Routine", "Standard", "Constitutional", "Emergency",
    ]
    private static let stages: Set<String> = [
        "Qualification", "Rules", "Agenda", "Interest", "Review", "Coordination",
        "Mpc", "Fma", "Oversight", "PolicyJury", "ConfirmationJury",
        "Certification", "Enactment",
    ]
    private static let statuses: Set<String> = [
        "Active", "Certified", "Rejected", "Enacted", "Superseded", "ExecutionFailed",
    ]
    private static let bodyStatuses: Set<String> = [
        "AwaitingSortition", "AcceptingInvitations", "RosterSealed", "Deliberating",
        "Balloting", "Approved", "Rejected", "NoQuorum", "NoResult", "Superseded",
    ]
    private static let deliberationPhases: Set<String> = [
        "Orientation", "Evidence", "Questions", "Responses", "Deliberation", "Reflection", "Vote",
    ]
    private static let privateKeyFields: Set<String> = [
        "private_key", "privateKey", "seed", "mnemonic", "private_key_seed",
    ]

    /// Replace the sole path parameter after exact nonzero identifier validation.
    public static func attemptReadPath(governanceAttemptId: String) throws -> String {
        let identifier = try requireIdentifier(
            governanceAttemptId,
            field: "governanceAttemptId"
        )
        return attemptReadPathTemplate.replacingOccurrences(
            of: "{governance_attempt_id}",
            with: identifier
        )
    }

    /// Replace the casting-context ballot parameter after exact identifier validation.
    public static func timedOvnCastingContextReadPath(ballotAttemptId: String) throws -> String {
        let identifier = try requireIdentifier(ballotAttemptId, field: "ballotAttemptId")
        return timedOvnCastingContextReadPathTemplate.replacingOccurrences(
            of: "{ballot_attempt_id}",
            with: identifier
        )
    }

    /// Replace the casting-proof ballot parameter after exact identifier validation.
    public static func timedOvnCastingProofPath(ballotAttemptId: String) throws -> String {
        let identifier = try requireIdentifier(ballotAttemptId, field: "ballotAttemptId")
        return timedOvnCastingProofPathTemplate.replacingOccurrences(
            of: "{ballot_attempt_id}",
            with: identifier
        )
    }

    /// Replace the release-context ballot parameter after exact identifier validation.
    public static func tleReleaseContextReadPath(ballotAttemptId: String) throws -> String {
        let identifier = try requireIdentifier(ballotAttemptId, field: "ballotAttemptId")
        return tleReleaseContextReadPathTemplate.replacingOccurrences(
            of: "{ballot_attempt_id}",
            with: identifier
        )
    }

    /// Replace the local partial-release ballot parameter after exact identifier validation.
    public static func tlePartialReleasePath(ballotAttemptId: String) throws -> String {
        let identifier = try requireIdentifier(ballotAttemptId, field: "ballotAttemptId")
        return tlePartialReleasePathTemplate.replacingOccurrences(
            of: "{ballot_attempt_id}",
            with: identifier
        )
    }

    /// Encode the exact, versioned attempt-draft request body.
    public static func attemptDraftRequestData(
        proposal: ToriiParliamentProposalV1,
        attemptSequence: UInt32
    ) throws -> Data {
        try JSONEncoder().encode(
            ToriiParliamentAttemptDraftEnvelopeV1(
                version: version,
                proposal: proposal,
                attemptSequence: attemptSequence
            )
        )
    }

    /// Encode the exact, versioned lifecycle-transition draft request body.
    public static func transitionDraftRequestData(
        governanceAttemptId: String,
        transition: ToriiParliamentLifecycleTransitionV1
    ) throws -> Data {
        let identifier = try requireIdentifier(
            governanceAttemptId,
            field: "governanceAttemptId"
        )
        return try JSONEncoder().encode(
            ToriiParliamentTransitionDraftEnvelopeV1(
                version: version,
                governanceAttemptId: identifier,
                transition: transition
            )
        )
    }

    /// Strictly decode and bind one attempt-draft response.
    public static func decodeAttemptDraftResponse(
        _ data: Data,
        expectedProposalContentId: String,
        expectedGovernanceAttemptId: String
    ) throws -> ToriiParliamentAttemptDraftResponseV1 {
        let root = try exactRoot(
            data,
            fields: ["version", "proposal_content_id", "governance_attempt_id", "tx_instructions"],
            context: "Parliament attempt draft response"
        )
        try requireVersion(root["version"])
        let proposalId = try requireIdentifier(
            root["proposal_content_id"],
            field: "proposal_content_id"
        )
        let attemptId = try requireIdentifier(
            root["governance_attempt_id"],
            field: "governance_attempt_id"
        )
        guard proposalId == (try requireIdentifier(
            expectedProposalContentId,
            field: "expectedProposalContentId"
        )) else {
            throw ToriiClientError.invalidPayload(
                "proposal_content_id differs from the exact request binding."
            )
        }
        guard attemptId == (try requireIdentifier(
            expectedGovernanceAttemptId,
            field: "expectedGovernanceAttemptId"
        )) else {
            throw ToriiClientError.invalidPayload(
                "governance_attempt_id differs from the exact request binding."
            )
        }
        return .init(
            proposalContentId: proposalId,
            governanceAttemptId: attemptId,
            instruction: try instruction(
                root["tx_instructions"],
                expectedWireId: attemptCreateWireId
            )
        )
    }

    /// Strictly decode and bind one transition-draft response.
    public static func decodeTransitionDraftResponse(
        _ data: Data,
        expectedGovernanceAttemptId: String,
        expectedTransitionKind: String,
        expectedTransitionDigest: [UInt8]
    ) throws -> ToriiParliamentTransitionDraftResponseV1 {
        guard publicTransitions.contains(where: { $0.jsonTag == expectedTransitionKind }) else {
            throw ToriiClientError.invalidPayload(
                "expected transition kind is unknown or automatic-only."
            )
        }
        try requireFixedBytes(
            expectedTransitionDigest,
            count: 32,
            nonzero: true,
            field: "expectedTransitionDigest"
        )
        let root = try exactRoot(
            data,
            fields: [
                "version", "governance_attempt_id", "transition_kind",
                "transition_digest", "tx_instructions",
            ],
            context: "Parliament transition draft response"
        )
        try requireVersion(root["version"])
        let attemptId = try requireIdentifier(
            root["governance_attempt_id"],
            field: "governance_attempt_id"
        )
        guard attemptId == (try requireIdentifier(
            expectedGovernanceAttemptId,
            field: "expectedGovernanceAttemptId"
        )) else {
            throw ToriiClientError.invalidPayload(
                "governance_attempt_id differs from the exact request binding."
            )
        }
        let transitionKind = try taggedUnit(
            root["transition_kind"],
            tag: "kind",
            admitted: Set(publicTransitions.map(\.jsonTag)),
            context: "transition_kind"
        )
        guard transitionKind == expectedTransitionKind else {
            throw ToriiClientError.invalidPayload(
                "transition_kind differs from the exact request binding."
            )
        }
        let digest = try fixedBytes(
            root["transition_digest"],
            count: 32,
            nonzero: true,
            field: "transition_digest"
        )
        guard digest == expectedTransitionDigest else {
            throw ToriiClientError.invalidPayload(
                "transition_digest differs from the exact request binding."
            )
        }
        return .init(
            governanceAttemptId: attemptId,
            transitionKind: transitionKind,
            transitionDigest: digest,
            instruction: try instruction(
                root["tx_instructions"],
                expectedWireId: transitionSubmitWireId
            )
        )
    }

    /// Strictly decode one bounded attempt-read response and its public supporters.
    public static func decodeAttemptReadResponse(
        _ data: Data,
        expectedGovernanceAttemptId: String
    ) throws -> ToriiParliamentAttemptReadResponseV1 {
        let root = try exactRoot(
            data,
            fields: [
                "version", "current_height", "attempt", "policy_version", "required_bodies",
                "body_states", "certificate", "terminal_height", "execution_failure_root",
                "superseding_head", "state_payload_hex",
            ],
            context: "Parliament attempt read response"
        )
        try requireVersion(root["version"])
        let currentHeight = try unsigned(root["current_height"], field: "current_height")
        let policyVersion = try unsigned(root["policy_version"], field: "policy_version")
        let terminalHeight = try optionalUnsigned(root["terminal_height"], field: "terminal_height")
        let executionFailureRoot = try optionalFixedBytes(
            root["execution_failure_root"],
            count: 32,
            nonzero: true,
            field: "execution_failure_root"
        )

        let attempt = try exactObject(
            root["attempt"],
            fields: ["id", "proposal_content_id", "sequence", "risk_tier", "stage", "status"],
            context: "attempt"
        )
        let attemptId = try requireIdentifier(attempt["id"], field: "attempt.id")
        guard attemptId == (try requireIdentifier(
            expectedGovernanceAttemptId,
            field: "expectedGovernanceAttemptId"
        )) else {
            throw ToriiClientError.invalidPayload(
                "attempt.id differs from the requested canonical identifier."
            )
        }
        _ = try requireIdentifier(
            attempt["proposal_content_id"],
            field: "attempt.proposal_content_id"
        )
        _ = try unsigned32(attempt["sequence"], field: "attempt.sequence")
        _ = try taggedUnit(
            attempt["risk_tier"], tag: "tier", admitted: riskTiers, context: "attempt.risk_tier"
        )
        _ = try taggedUnit(
            attempt["stage"], tag: "stage", admitted: stages, context: "attempt.stage"
        )
        _ = try taggedUnit(
            attempt["status"], tag: "status", admitted: statuses, context: "attempt.status"
        )

        let requiredBodies = try validateRequiredBodies(root["required_bodies"])
        let bodyStates = try validateBodyStates(
            root["body_states"],
            requiredBodies: requiredBodies
        )
        let publicFindings = try validateCertificate(
            root["certificate"],
            expectedAttemptId: attemptId
        )
        if !(root["superseding_head"] is NSNull) {
            try validateExpectedHead(root["superseding_head"], context: "superseding_head")
        }
        let statePayloadHex = try requireFramedHex(
            root["state_payload_hex"],
            maximumBytes: maximumAttemptStateBytes,
            field: "state_payload_hex"
        )
        return .init(
            governanceAttemptId: attemptId,
            currentHeight: currentHeight,
            policyVersion: policyVersion,
            terminalHeight: terminalHeight,
            executionFailureRoot: executionFailureRoot,
            statePayloadHex: statePayloadHex,
            bodyStates: bodyStates,
            publicFindingBindings: publicFindings,
            rawJSON: data
        )
    }

    /// Strictly admit one replay-validated public timed-OVN wallet context.
    public static func decodeTimedOvnCastingContextResponse(
        _ data: Data,
        expectedBallotAttemptId: String
    ) throws -> ToriiParliamentTimedOvnCastingContextResponseV1 {
        guard !data.isEmpty, data.count <= 16 * 1024 * 1024 else {
            throw ToriiClientError.invalidPayload(
                "Parliament timed-OVN casting context exceeds its response bound."
            )
        }
        let root = try exactRoot(
            data,
            fields: [
                "version", "current_height", "phase", "session",
                "registration_opened_at_finalized_height", "target_finalized_height",
                "tle_key_session", "registration_records_hex", "survivor_participant_hashes",
                "release_identity", "archive_norito_base64",
            ],
            context: "Parliament timed-OVN casting context"
        )
        try requireVersion(root["version"])
        let currentHeight = try unsigned(root["current_height"], field: "current_height")
        guard currentHeight > 0,
              let phaseRaw = root["phase"] as? String,
              let phase = ToriiParliamentTimedOvnCastingPhaseV1(rawValue: phaseRaw) else {
            throw ToriiClientError.invalidPayload(
                "Casting context height or cast-capable phase is invalid."
            )
        }
        let sessionRoot = try exactObject(
            root["session"],
            fields: [
                "network_id", "proposal_content_id", "governance_attempt_id",
                "body_instance_id", "ballot_attempt_id", "parameter_hash",
                "tle_key_session_id", "tle_key_transcript_hash", "tle_master_public_key",
            ],
            context: "session"
        )
        let session = ToriiParliamentTimedOvnSessionProjectionV1(
            networkId: try fixedBytes(
                sessionRoot["network_id"], count: 32, nonzero: true, field: "session.network_id"
            ),
            proposalContentId: try requireIdentifier(
                sessionRoot["proposal_content_id"], field: "session.proposal_content_id"
            ),
            governanceAttemptId: try requireIdentifier(
                sessionRoot["governance_attempt_id"], field: "session.governance_attempt_id"
            ),
            bodyInstanceId: try requireIdentifier(
                sessionRoot["body_instance_id"], field: "session.body_instance_id"
            ),
            ballotAttemptId: try requireIdentifier(
                sessionRoot["ballot_attempt_id"], field: "session.ballot_attempt_id"
            ),
            parameterHash: try fixedBytes(
                sessionRoot["parameter_hash"],
                count: 32,
                nonzero: true,
                field: "session.parameter_hash"
            ),
            tleKeySessionId: try requireIdentifier(
                sessionRoot["tle_key_session_id"], field: "session.tle_key_session_id"
            ),
            tleKeyTranscriptHash: try fixedBytes(
                sessionRoot["tle_key_transcript_hash"],
                count: 32,
                nonzero: true,
                field: "session.tle_key_transcript_hash"
            ),
            tleMasterPublicKey: try fixedBytes(
                sessionRoot["tle_master_public_key"],
                count: 96,
                nonzero: true,
                field: "session.tle_master_public_key"
            )
        )
        guard session.ballotAttemptId == (try requireIdentifier(
            expectedBallotAttemptId,
            field: "expectedBallotAttemptId"
        )) else {
            throw ToriiClientError.invalidPayload(
                "session.ballot_attempt_id differs from the requested identifier."
            )
        }
        let registrationOpened = try unsigned(
            root["registration_opened_at_finalized_height"],
            field: "registration_opened_at_finalized_height"
        )
        let targetHeight = try unsigned(
            root["target_finalized_height"], field: "target_finalized_height"
        )
        guard registrationOpened > 0,
              registrationOpened <= currentHeight,
              targetHeight > registrationOpened else {
            throw ToriiClientError.invalidPayload(
                "Casting-context height schedule is inconsistent."
            )
        }
        let keySession = try parseTleKeySession(root["tle_key_session"])
        guard session.tleKeySessionId == keySession.keySessionId,
              session.tleKeyTranscriptHash == keySession.transcriptHash,
              session.tleMasterPublicKey == keySession.groupPublicKey else {
            throw ToriiClientError.invalidPayload(
                "Timed-OVN session differs from the complete public TLE transcript."
            )
        }
        guard let recordValues = root["registration_records_hex"] as? [Any],
              recordValues.count <= maximumCorpusEntries,
              phase == .registered || !recordValues.isEmpty else {
            throw ToriiClientError.invalidPayload(
                "Registration corpus violates its casting-phase bound."
            )
        }
        let registrationRecords = try recordValues.enumerated().map { index, value in
            try canonicalHex(
                value,
                exactBytes: timedOvnRegistrationRecordBytes,
                field: "registration_records_hex[\(index)]"
            )
        }
        guard Set(registrationRecords).count == registrationRecords.count else {
            throw ToriiClientError.invalidPayload("Registration records must be unique.")
        }

        let survivorHashes: [[UInt8]]?
        if root["survivor_participant_hashes"] is NSNull {
            survivorHashes = nil
        } else if let values = root["survivor_participant_hashes"] as? [Any] {
            survivorHashes = try values.enumerated().map { index, value in
                try fixedBytes(
                    value,
                    count: 32,
                    nonzero: true,
                    field: "survivor_participant_hashes[\(index)]"
                )
            }
            guard let survivorHashes,
                  Set(survivorHashes.map { Data($0).base64EncodedString() }).count
                    == survivorHashes.count else {
                throw ToriiClientError.invalidPayload(
                    "Survivor participant hashes must be unique."
                )
            }
        } else {
            throw ToriiClientError.invalidPayload(
                "survivor_participant_hashes must be null or an array."
            )
        }

        let releaseIdentity: ToriiParliamentTimedOvnReleaseIdentityProjectionV1?
        if root["release_identity"] is NSNull {
            releaseIdentity = nil
        } else {
            releaseIdentity = try parseTimedOvnReleaseIdentity(root["release_identity"])
        }
        if phase == .survivorsFrozen {
            guard let survivors = survivorHashes,
                  !survivors.isEmpty,
                  survivors.count <= registrationRecords.count,
                  let identity = releaseIdentity,
                  identity.tleKeySessionId == session.tleKeySessionId,
                  identity.governanceAttemptId == session.governanceAttemptId,
                  identity.bodyInstanceId == session.bodyInstanceId,
                  identity.ballotAttemptId == session.ballotAttemptId,
                  identity.targetFinalizedHeight == targetHeight,
                  identity.parameterHash == session.parameterHash else {
                throw ToriiClientError.invalidPayload(
                    "SurvivorsFrozen fields differ from the timed-OVN session."
                )
            }
        } else if survivorHashes != nil || releaseIdentity != nil {
            throw ToriiClientError.invalidPayload(
                "Pre-freeze casting context must not expose frozen state."
            )
        }
        guard let archiveLiteral = root["archive_norito_base64"] as? String,
              let archive = Data(base64Encoded: archiveLiteral),
              !archive.isEmpty,
              archive.count <= maximumTimedOvnCastingContextArchiveBytes,
              archive.base64EncodedString() == archiveLiteral else {
            throw ToriiClientError.invalidPayload(
                "archive_norito_base64 is oversized or noncanonical."
            )
        }
        return .init(
            currentHeight: currentHeight,
            phase: phase,
            session: session,
            registrationOpenedAtFinalizedHeight: registrationOpened,
            targetFinalizedHeight: targetHeight,
            keySession: keySession,
            registrationRecordsHex: registrationRecords,
            survivorParticipantHashes: survivorHashes,
            releaseIdentity: releaseIdentity,
            archiveNorito: archive
        )
    }

    /// Strictly admit one complete public adaptive-TLE transcript and release identity.
    public static func decodeTleReleaseContextResponse(
        _ data: Data,
        expectedBallotAttemptId: String
    ) throws -> ToriiParliamentTleReleaseContextResponseV1 {
        guard !data.isEmpty, data.count <= 1024 * 1024 else {
            throw ToriiClientError.invalidPayload(
                "Parliament TLE release context exceeds its response bound."
            )
        }
        let root = try exactRoot(
            data,
            fields: [
                "version", "current_height", "ballot_attempt_id", "governance_attempt_id",
                "body_instance_id", "status", "release_height", "opening_deadline_height",
                "tle_key_session", "release_identity", "identity_digest", "identity_payload_hex",
            ],
            context: "Parliament TLE release context"
        )
        try requireVersion(root["version"])
        let currentHeight = try unsigned(root["current_height"], field: "current_height")
        let ballotAttemptId = try requireIdentifier(
            root["ballot_attempt_id"], field: "ballot_attempt_id"
        )
        guard ballotAttemptId == (try requireIdentifier(
            expectedBallotAttemptId, field: "expectedBallotAttemptId"
        )) else {
            throw ToriiClientError.invalidPayload(
                "ballot_attempt_id differs from the requested canonical identifier."
            )
        }
        let governanceAttemptId = try requireIdentifier(
            root["governance_attempt_id"], field: "governance_attempt_id"
        )
        let bodyInstanceId = try requireIdentifier(
            root["body_instance_id"], field: "body_instance_id"
        )
        guard try taggedUnit(
            root["status"], tag: "status", admitted: ["Opening"], context: "status"
        ) == "Opening" else {
            throw ToriiClientError.invalidPayload("release context status must be Opening.")
        }
        let releaseHeight = try unsigned(root["release_height"], field: "release_height")
        let openingDeadlineHeight = try unsigned(
            root["opening_deadline_height"], field: "opening_deadline_height"
        )
        guard currentHeight >= releaseHeight, currentHeight <= openingDeadlineHeight else {
            throw ToriiClientError.invalidPayload(
                "release context lies outside its inclusive opening window."
            )
        }

        let keySession = try parseTleKeySession(root["tle_key_session"])
        let identityRoot = try exactObject(
            root["release_identity"],
            fields: [
                "tle_key_session_id", "governance_attempt_id", "body_instance_id",
                "ballot_attempt_id", "survivor_corpus_root", "no_recovery_root",
                "target_finalized_height", "parameter_hash",
            ],
            context: "release_identity"
        )
        let identity = ToriiParliamentTimedOvnReleaseIdentityProjectionV1(
            tleKeySessionId: try requireIdentifier(
                identityRoot["tle_key_session_id"], field: "release_identity.tle_key_session_id"
            ),
            governanceAttemptId: try requireIdentifier(
                identityRoot["governance_attempt_id"],
                field: "release_identity.governance_attempt_id"
            ),
            bodyInstanceId: try requireIdentifier(
                identityRoot["body_instance_id"], field: "release_identity.body_instance_id"
            ),
            ballotAttemptId: try requireIdentifier(
                identityRoot["ballot_attempt_id"], field: "release_identity.ballot_attempt_id"
            ),
            survivorCorpusRoot: try fixedBytes(
                identityRoot["survivor_corpus_root"],
                count: 32,
                nonzero: true,
                field: "release_identity.survivor_corpus_root"
            ),
            noRecoveryRoot: try fixedBytes(
                identityRoot["no_recovery_root"],
                count: 32,
                nonzero: true,
                field: "release_identity.no_recovery_root"
            ),
            targetFinalizedHeight: try unsigned(
                identityRoot["target_finalized_height"],
                field: "release_identity.target_finalized_height"
            ),
            parameterHash: try fixedBytes(
                identityRoot["parameter_hash"],
                count: 32,
                nonzero: true,
                field: "release_identity.parameter_hash"
            )
        )
        guard identity.tleKeySessionId == keySession.keySessionId,
              identity.governanceAttemptId == governanceAttemptId,
              identity.bodyInstanceId == bodyInstanceId,
              identity.ballotAttemptId == ballotAttemptId,
              identity.targetFinalizedHeight == releaseHeight else {
            throw ToriiClientError.invalidPayload(
                "release_identity differs from the top-level Parliament/TLE bindings."
            )
        }
        let identityPayloadHex = try canonicalHex(
            root["identity_payload_hex"],
            exactBytes: 243,
            field: "identity_payload_hex"
        )
        guard let identityPayload = Data(hexString: identityPayloadHex) else {
            throw ToriiClientError.invalidPayload("identity_payload_hex is malformed.")
        }
        try validateTleIdentityPayload(
            [UInt8](identityPayload),
            governanceAttemptId: governanceAttemptId,
            bodyInstanceId: bodyInstanceId,
            ballotAttemptId: ballotAttemptId,
            identity: identity
        )
        let identityDigest = try fixedBytes(
            root["identity_digest"], count: 32, nonzero: true, field: "identity_digest"
        )
        guard identityDigest == (try tleReleaseMessageDigest(
            session: keySession,
            identityPayload: [UInt8](identityPayload)
        )) else {
            throw ToriiClientError.invalidPayload(
                "identity_digest differs from the exact threshold-session-framed release message."
            )
        }
        return .init(
            currentHeight: currentHeight,
            ballotAttemptId: ballotAttemptId,
            governanceAttemptId: governanceAttemptId,
            bodyInstanceId: bodyInstanceId,
            releaseHeight: releaseHeight,
            openingDeadlineHeight: openingDeadlineHeight,
            keySession: keySession,
            releaseIdentity: identity,
            identityDigest: identityDigest,
            identityPayloadHex: identityPayloadHex
        )
    }

    /// Strictly bind one public partial release to an already admitted release context.
    public static func decodeTlePartialReleaseResponse(
        _ data: Data,
        expectedKeySessionId: String,
        expectedIdentityDigest: [UInt8],
        committeeSize: UInt32
    ) throws -> ToriiParliamentTlePartialReleaseShareV1 {
        guard !data.isEmpty, data.count <= 16 * 1024 else {
            throw ToriiClientError.invalidPayload(
                "Parliament TLE partial release exceeds its response bound."
            )
        }
        try requireFixedBytes(
            expectedIdentityDigest,
            count: 32,
            nonzero: true,
            field: "expectedIdentityDigest"
        )
        guard committeeSize >= 4,
              committeeSize <= maximumTleCommitteeSize,
              (committeeSize - 1).isMultiple(of: 3) else {
            throw ToriiClientError.invalidPayload(
                "committeeSize must be an exact supported 3f+1 size."
            )
        }
        let root = try exactRoot(
            data,
            fields: [
                "key_session_id", "identity_digest", "participant_index", "sigma",
                "proof_x", "proof_y", "z_s", "z_r", "z_u",
            ],
            context: "Parliament TLE partial release"
        )
        let keySessionId = try requireIdentifier(root["key_session_id"], field: "key_session_id")
        guard keySessionId == (try requireIdentifier(
            expectedKeySessionId, field: "expectedKeySessionId"
        )) else {
            throw ToriiClientError.invalidPayload(
                "partial key_session_id differs from the authorized release context."
            )
        }
        let identityDigest = try fixedBytes(
            root["identity_digest"], count: 32, nonzero: true, field: "identity_digest"
        )
        guard identityDigest == expectedIdentityDigest else {
            throw ToriiClientError.invalidPayload(
                "partial identity_digest differs from the authorized release context."
            )
        }
        return .init(
            keySessionId: keySessionId,
            identityDigest: identityDigest,
            participantIndex: try boundedUInt32(
                root["participant_index"],
                minimum: 1,
                maximum: committeeSize,
                field: "participant_index"
            ),
            sigma: try fixedBytes(root["sigma"], count: 48, nonzero: true, field: "sigma"),
            proofX: try fixedBytes(root["proof_x"], count: 96, nonzero: true, field: "proof_x"),
            proofY: try fixedBytes(root["proof_y"], count: 48, nonzero: true, field: "proof_y"),
            zS: try fixedBytes(root["z_s"], count: 32, nonzero: false, field: "z_s"),
            zR: try fixedBytes(root["z_r"], count: 32, nonzero: false, field: "z_r"),
            zU: try fixedBytes(root["z_u"], count: 32, nonzero: false, field: "z_u")
        )
    }
}

private struct ToriiParliamentAttemptDraftEnvelopeV1: Encodable {
    let version: UInt16
    let proposal: ToriiParliamentProposalV1
    let attemptSequence: UInt32

    enum CodingKeys: String, CodingKey {
        case version
        case proposal
        case attemptSequence = "attempt_sequence"
    }
}

private struct ToriiParliamentTransitionDraftEnvelopeV1: Encodable {
    let version: UInt16
    let governanceAttemptId: String
    let transition: ToriiParliamentLifecycleTransitionV1

    enum CodingKeys: String, CodingKey {
        case version
        case governanceAttemptId = "governance_attempt_id"
        case transition
    }
}

fileprivate extension ToriiParliamentAPIV1 {
    static func exactRoot(
        _ data: Data,
        fields: Set<String>,
        context: String
    ) throws -> [String: Any] {
        guard !data.isEmpty, data.count <= 64 * 1024 * 1024 else {
            throw ToriiClientError.invalidPayload("\(context) exceeds its response bound.")
        }
        let decoded: Any
        do {
            decoded = try JSONSerialization.jsonObject(with: data)
        } catch {
            throw ToriiClientError.decoding(error)
        }
        return try exactObject(decoded, fields: fields, context: context)
    }

    static func exactObject(
        _ value: Any?,
        fields: Set<String>,
        context: String
    ) throws -> [String: Any] {
        guard let object = value as? [String: Any], Set(object.keys) == fields else {
            throw ToriiClientError.invalidPayload(
                "\(context) contains unknown, aliased, or missing fields."
            )
        }
        return object
    }

    static func requireVersion(_ value: Any?) throws {
        guard try unsigned(value, field: "version") == UInt64(version) else {
            throw ToriiClientError.invalidPayload("unsupported Parliament API version.")
        }
    }

    static func requireIdentifier(_ value: Any?, field: String) throws -> String {
        guard let string = value as? String else {
            throw ToriiClientError.invalidPayload(
                "\(field) must be exactly 64 lowercase nonzero hexadecimal characters."
            )
        }
        return try requireIdentifier(string, field: field)
    }

    static func requireIdentifier(_ value: String, field: String) throws -> String {
        let bytes = Array(value.utf8)
        guard bytes.count == 64,
              bytes.allSatisfy({ (0x30...0x39).contains($0) || (0x61...0x66).contains($0) }),
              bytes.contains(where: { $0 != 0x30 }) else {
            throw ToriiClientError.invalidPayload(
                "\(field) must be exactly 64 lowercase nonzero hexadecimal characters."
            )
        }
        return value
    }

    static func requireBody(_ value: String, field: String) throws {
        guard bodies.contains(value) else {
            throw ToriiClientError.invalidPayload("\(field) is not a Parliament body.")
        }
    }

    static func requireFixedBytes(
        _ value: [UInt8],
        count: Int,
        nonzero: Bool,
        field: String
    ) throws {
        guard value.count == count,
              !nonzero || value.contains(where: { $0 != 0 }) else {
            let suffix = nonzero ? " nonzero" : ""
            throw ToriiClientError.invalidPayload(
                "\(field) must contain exactly \(count)\(suffix) bytes."
            )
        }
    }

    static func fixedBytes(
        _ value: Any?,
        count: Int,
        nonzero: Bool,
        field: String
    ) throws -> [UInt8] {
        guard let raw = value as? [Any], raw.count == count else {
            throw ToriiClientError.invalidPayload(
                "\(field) must contain exactly \(count) bytes."
            )
        }
        var result = [UInt8]()
        result.reserveCapacity(count)
        for (index, item) in raw.enumerated() {
            let byte = try unsigned(item, field: "\(field)[\(index)]")
            guard byte <= UInt64(UInt8.max) else {
                throw ToriiClientError.invalidPayload("\(field)[\(index)] is not a byte.")
            }
            result.append(UInt8(byte))
        }
        try requireFixedBytes(result, count: count, nonzero: nonzero, field: field)
        return result
    }

    static func optionalFixedBytes(
        _ value: Any?,
        count: Int,
        nonzero: Bool,
        field: String
    ) throws -> [UInt8]? {
        guard let value else {
            throw ToriiClientError.invalidPayload("\(field) is missing.")
        }
        if value is NSNull { return nil }
        return try fixedBytes(value, count: count, nonzero: nonzero, field: field)
    }

    private static func parseTimedOvnReleaseIdentity(
        _ value: Any?
    ) throws -> ToriiParliamentTimedOvnReleaseIdentityProjectionV1 {
        let root = try exactObject(
            value,
            fields: [
                "tle_key_session_id", "governance_attempt_id", "body_instance_id",
                "ballot_attempt_id", "survivor_corpus_root", "no_recovery_root",
                "target_finalized_height", "parameter_hash",
            ],
            context: "release_identity"
        )
        return .init(
            tleKeySessionId: try requireIdentifier(
                root["tle_key_session_id"], field: "release_identity.tle_key_session_id"
            ),
            governanceAttemptId: try requireIdentifier(
                root["governance_attempt_id"], field: "release_identity.governance_attempt_id"
            ),
            bodyInstanceId: try requireIdentifier(
                root["body_instance_id"], field: "release_identity.body_instance_id"
            ),
            ballotAttemptId: try requireIdentifier(
                root["ballot_attempt_id"], field: "release_identity.ballot_attempt_id"
            ),
            survivorCorpusRoot: try fixedBytes(
                root["survivor_corpus_root"],
                count: 32,
                nonzero: true,
                field: "release_identity.survivor_corpus_root"
            ),
            noRecoveryRoot: try fixedBytes(
                root["no_recovery_root"],
                count: 32,
                nonzero: true,
                field: "release_identity.no_recovery_root"
            ),
            targetFinalizedHeight: try unsigned(
                root["target_finalized_height"],
                field: "release_identity.target_finalized_height"
            ),
            parameterHash: try fixedBytes(
                root["parameter_hash"],
                count: 32,
                nonzero: true,
                field: "release_identity.parameter_hash"
            )
        )
    }

    static func parseTleKeySession(
        _ value: Any?
    ) throws -> ToriiParliamentTleKeySessionPublicStateV1 {
        let root = try exactObject(
            value,
            fields: [
                "version", "key_session_id", "network_id", "roster_hash", "committee_size",
                "threshold", "generator_h", "generator_v", "qualified_dealers",
                "qualified_dealer_commitments", "dkg_event_hash", "group_public_key",
                "public_shares", "transcript_hash",
            ],
            context: "tle_key_session"
        )
        try requireVersion(root["version"])
        let keySessionId = try requireIdentifier(
            root["key_session_id"], field: "tle_key_session.key_session_id"
        )
        let committeeSize = try boundedUInt32(
            root["committee_size"],
            minimum: 4,
            maximum: maximumTleCommitteeSize,
            field: "tle_key_session.committee_size"
        )
        let threshold = try boundedUInt32(
            root["threshold"],
            minimum: 2,
            maximum: 11,
            field: "tle_key_session.threshold"
        )
        guard (committeeSize - 1).isMultiple(of: 3),
              threshold == (committeeSize - 1) / 3 + 1 else {
            throw ToriiClientError.invalidPayload(
                "TLE committee_size/threshold is not an exact 3f+1/f+1 binding."
            )
        }
        guard let qualifiedValues = root["qualified_dealers"] as? [Any] else {
            throw ToriiClientError.invalidPayload(
                "tle_key_session.qualified_dealers must be an array."
            )
        }
        var qualifiedDealers = [UInt32]()
        qualifiedDealers.reserveCapacity(qualifiedValues.count)
        var previous: UInt32?
        for (index, value) in qualifiedValues.enumerated() {
            let dealer = try boundedUInt32(
                value,
                minimum: 1,
                maximum: committeeSize,
                field: "tle_key_session.qualified_dealers[\(index)]"
            )
            if let previous, dealer <= previous {
                throw ToriiClientError.invalidPayload(
                    "qualified dealer indices must be strictly increasing and distinct."
                )
            }
            qualifiedDealers.append(dealer)
            previous = dealer
        }
        guard qualifiedDealers.count >= Int(threshold),
              qualifiedDealers.count <= Int(committeeSize) else {
            throw ToriiClientError.invalidPayload(
                "qualified dealer indices violate the threshold bound."
            )
        }

        guard let dealerValues = root["qualified_dealer_commitments"] as? [Any],
              dealerValues.count == qualifiedDealers.count else {
            throw ToriiClientError.invalidPayload(
                "qualified dealer commitments must align exactly with qualified_dealers."
            )
        }
        var dealers = [ToriiParliamentTleAdaptiveDealerCommitmentV1]()
        dealers.reserveCapacity(dealerValues.count)
        for (index, value) in dealerValues.enumerated() {
            let context = "qualified_dealer_commitments[\(index)]"
            let dealer = try exactObject(
                value,
                fields: [
                    "dealer_index", "coefficient_commitments", "constant_pok_commitment",
                    "constant_pok_response",
                ],
                context: context
            )
            let dealerIndex = try boundedUInt32(
                dealer["dealer_index"],
                minimum: 1,
                maximum: committeeSize,
                field: "\(context).dealer_index"
            )
            guard dealerIndex == qualifiedDealers[index] else {
                throw ToriiClientError.invalidPayload(
                    "dealer commitment index differs from the canonical qualified set."
                )
            }
            guard let coefficients = dealer["coefficient_commitments"] as? [Any],
                  coefficients.count == Int(threshold) else {
                throw ToriiClientError.invalidPayload(
                    "each dealer must carry the exact degree-f coefficient set."
                )
            }
            let commitments = try coefficients.enumerated().map { coefficientIndex, item in
                try fixedBytes(
                    item,
                    count: 96,
                    nonzero: true,
                    field: "\(context).coefficient_commitments[\(coefficientIndex)]"
                )
            }
            dealers.append(.init(
                dealerIndex: dealerIndex,
                coefficientCommitments: commitments,
                constantPokCommitment: try fixedBytes(
                    dealer["constant_pok_commitment"],
                    count: 96,
                    nonzero: true,
                    field: "\(context).constant_pok_commitment"
                ),
                constantPokResponse: try fixedBytes(
                    dealer["constant_pok_response"],
                    count: 32,
                    nonzero: false,
                    field: "\(context).constant_pok_response"
                )
            ))
        }

        guard let shareValues = root["public_shares"] as? [Any],
              shareValues.count == Int(committeeSize) else {
            throw ToriiClientError.invalidPayload(
                "public_shares must contain the complete ordered committee."
            )
        }
        var publicShares = [ToriiParliamentTleAdaptivePublicShareV1]()
        publicShares.reserveCapacity(shareValues.count)
        for (offset, value) in shareValues.enumerated() {
            let context = "public_shares[\(offset)]"
            let share = try exactObject(
                value,
                fields: ["index", "participant_hash", "public_key_share"],
                context: context
            )
            let index = try boundedUInt32(
                share["index"],
                minimum: 1,
                maximum: committeeSize,
                field: "\(context).index"
            )
            guard index == UInt32(offset + 1) else {
                throw ToriiClientError.invalidPayload(
                    "public share indices must be the exact one-based committee sequence."
                )
            }
            publicShares.append(.init(
                index: index,
                participantHash: try fixedBytes(
                    share["participant_hash"],
                    count: 32,
                    nonzero: true,
                    field: "\(context).participant_hash"
                ),
                publicKeyShare: try fixedBytes(
                    share["public_key_share"],
                    count: 96,
                    nonzero: true,
                    field: "\(context).public_key_share"
                )
            ))
        }
        return .init(
            keySessionId: keySessionId,
            networkId: try fixedBytes(
                root["network_id"], count: 32, nonzero: true, field: "tle_key_session.network_id"
            ),
            rosterHash: try fixedBytes(
                root["roster_hash"], count: 32, nonzero: true, field: "tle_key_session.roster_hash"
            ),
            committeeSize: committeeSize,
            threshold: threshold,
            generatorH: try fixedBytes(
                root["generator_h"], count: 96, nonzero: true, field: "tle_key_session.generator_h"
            ),
            generatorV: try fixedBytes(
                root["generator_v"], count: 96, nonzero: true, field: "tle_key_session.generator_v"
            ),
            qualifiedDealers: qualifiedDealers,
            qualifiedDealerCommitments: dealers,
            dkgEventHash: try fixedBytes(
                root["dkg_event_hash"], count: 32, nonzero: true, field: "tle_key_session.dkg_event_hash"
            ),
            groupPublicKey: try fixedBytes(
                root["group_public_key"], count: 96, nonzero: true, field: "tle_key_session.group_public_key"
            ),
            publicShares: publicShares,
            transcriptHash: try fixedBytes(
                root["transcript_hash"], count: 32, nonzero: true, field: "tle_key_session.transcript_hash"
            )
        )
    }

    static func canonicalHex(
        _ value: Any?,
        exactBytes: Int,
        field: String
    ) throws -> String {
        guard let string = value as? String,
              string.utf8.count == 2 * exactBytes,
              string.utf8.allSatisfy({
                  (0x30...0x39).contains($0) || (0x61...0x66).contains($0)
              }),
              Data(hexString: string)?.count == exactBytes else {
            throw ToriiClientError.invalidPayload(
                "\(field) must contain exactly \(exactBytes) lowercase hexadecimal bytes."
            )
        }
        return string
    }

    static func validateTleIdentityPayload(
        _ payload: [UInt8],
        governanceAttemptId: String,
        bodyInstanceId: String,
        ballotAttemptId: String,
        identity: ToriiParliamentTimedOvnReleaseIdentityProjectionV1
    ) throws {
        let domain = Array("iroha.parliament.tle.identity-payload.v1\0".utf8)
        guard payload.count == 243, Array(payload.prefix(domain.count)) == domain else {
            throw ToriiClientError.invalidPayload(
                "identity_payload_hex has the wrong domain or canonical width."
            )
        }
        var offset = domain.count
        guard Array(payload[offset ..< offset + 2]) == bigEndian16(1) else {
            throw ToriiClientError.invalidPayload("identity payload version must equal one.")
        }
        offset += 2
        let identifiers = [governanceAttemptId, bodyInstanceId, ballotAttemptId]
        var expectedBindings = try identifiers.map { identifier -> [UInt8] in
            guard let bytes = Data(hexString: identifier) else {
                throw ToriiClientError.invalidPayload("identity payload identifier is malformed.")
            }
            return [UInt8](bytes)
        }
        expectedBindings.append(identity.survivorCorpusRoot)
        expectedBindings.append(identity.noRecoveryRoot)
        let fields = [
            "governance_attempt_id", "body_instance_id", "ballot_attempt_id",
            "survivor_corpus_root", "no_recovery_root",
        ]
        for (index, expected) in expectedBindings.enumerated() {
            guard Array(payload[offset ..< offset + 32]) == expected else {
                throw ToriiClientError.invalidPayload(
                    "identity payload \(fields[index]) binding differs."
                )
            }
            offset += 32
        }
        guard Array(payload[offset ..< offset + 8]) == bigEndian64(
            identity.targetFinalizedHeight
        ) else {
            throw ToriiClientError.invalidPayload("identity payload release height differs.")
        }
        offset += 8
        guard Array(payload[offset ..< offset + 32]) == identity.parameterHash else {
            throw ToriiClientError.invalidPayload(
                "identity payload parameter_hash binding differs."
            )
        }
    }

    static func tleReleaseMessageDigest(
        session: ToriiParliamentTleKeySessionPublicStateV1,
        identityPayload: [UInt8]
    ) throws -> [UInt8] {
        guard let keySessionId = Data(hexString: session.keySessionId) else {
            throw ToriiClientError.invalidPayload("TLE key session identifier is malformed.")
        }
        var message = [UInt8]()
        message.append(contentsOf: "iroha.threshold-bls.message.v1\0".utf8)
        message.append(contentsOf: "iroha.threshold-bls.session.v1\0".utf8)
        message.append(contentsOf: bigEndian16(1))
        message.append(2)
        message.append(contentsOf: session.networkId)
        message.append(contentsOf: keySessionId)
        message.append(contentsOf: session.rosterHash)
        message.append(contentsOf: bigEndian16(UInt16(session.committeeSize)))
        message.append(contentsOf: bigEndian16(UInt16(session.threshold)))
        message.append(contentsOf: bigEndian32(UInt32(identityPayload.count)))
        message.append(contentsOf: identityPayload)
        return [UInt8](SHA256.hash(data: Data(message)))
    }

    static func bigEndian16(_ value: UInt16) -> [UInt8] {
        [UInt8(value >> 8), UInt8(value & 0xff)]
    }

    static func bigEndian32(_ value: UInt32) -> [UInt8] {
        [
            UInt8((value >> 24) & 0xff), UInt8((value >> 16) & 0xff),
            UInt8((value >> 8) & 0xff), UInt8(value & 0xff),
        ]
    }

    static func bigEndian64(_ value: UInt64) -> [UInt8] {
        (0 ..< 8).map { offset in UInt8((value >> UInt64(8 * (7 - offset))) & 0xff) }
    }

    static func unsigned(_ value: Any?, field: String) throws -> UInt64 {
        guard let value, !(value is Bool), let number = value as? NSNumber else {
            throw ToriiClientError.invalidPayload("\(field) must be an unsigned integer.")
        }
        let decimal = number.stringValue
        guard !decimal.isEmpty,
              decimal.allSatisfy(\.isNumber),
              let parsed = UInt64(decimal),
              String(parsed) == decimal else {
            throw ToriiClientError.invalidPayload("\(field) must be an unsigned integer.")
        }
        return parsed
    }

    static func unsigned32(_ value: Any?, field: String) throws -> UInt32 {
        let parsed = try unsigned(value, field: field)
        guard parsed <= UInt64(UInt32.max) else {
            throw ToriiClientError.invalidPayload("\(field) is outside u32.")
        }
        return UInt32(parsed)
    }

    static func boundedUInt32(
        _ value: Any?,
        minimum: UInt32,
        maximum: UInt32,
        field: String
    ) throws -> UInt32 {
        let parsed = try unsigned32(value, field: field)
        guard parsed >= minimum, parsed <= maximum else {
            throw ToriiClientError.invalidPayload(
                "\(field) must be in \(minimum)...\(maximum)."
            )
        }
        return parsed
    }

    static func optionalUnsigned(_ value: Any?, field: String) throws -> UInt64? {
        guard let value else {
            throw ToriiClientError.invalidPayload("\(field) is missing.")
        }
        if value is NSNull { return nil }
        return try unsigned(value, field: field)
    }

    static func taggedUnit(
        _ value: Any?,
        tag: String,
        admitted: Set<String>,
        context: String
    ) throws -> String {
        let object = try exactObject(value, fields: [tag], context: context)
        guard let result = object[tag] as? String, admitted.contains(result) else {
            throw ToriiClientError.invalidPayload("\(context).\(tag) is unknown.")
        }
        return result
    }

    static func requireStrictIdentifiers(_ values: [String], field: String) throws {
        guard !values.isEmpty, values.count <= maximumCorpusEntries else {
            throw ToriiClientError.invalidPayload(
                "\(field) must contain one through 1000 identifiers."
            )
        }
        var previous: String?
        for (index, value) in values.enumerated() {
            let identifier = try requireIdentifier(value, field: "\(field)[\(index)]")
            if let previous, previous >= identifier {
                throw ToriiClientError.invalidPayload(
                    "\(field) must be strictly increasing and distinct."
                )
            }
            previous = identifier
        }
    }

    static func rejectSigningMaterial(_ value: ToriiJSONValue, context: String) throws {
        switch value {
        case let .array(values):
            for (index, item) in values.enumerated() {
                try rejectSigningMaterial(item, context: "\(context)[\(index)]")
            }
        case let .object(object):
            for (key, item) in object {
                guard !privateKeyFields.contains(key) else {
                    throw ToriiClientError.invalidPayload(
                        "\(context).\(key) is forbidden; sign the draft locally."
                    )
                }
                try rejectSigningMaterial(item, context: "\(context).\(key)")
            }
        case .string, .number, .bool, .null:
            break
        }
    }

    static func instruction(
        _ value: Any?,
        expectedWireId: String
    ) throws -> ToriiParliamentInstructionDraftV1 {
        guard let instructions = value as? [Any], instructions.count == 1 else {
            throw ToriiClientError.invalidPayload(
                "Parliament draft response must contain exactly one instruction."
            )
        }
        let instruction = try exactObject(
            instructions[0],
            fields: ["wire_id", "payload_hex"],
            context: "tx_instructions[0]"
        )
        guard instruction["wire_id"] as? String == expectedWireId else {
            throw ToriiClientError.invalidPayload("instruction draft has the wrong wire_id.")
        }
        return .init(
            wireId: expectedWireId,
            payloadHex: try requireFramedHex(
                instruction["payload_hex"],
                maximumBytes: maximumAttemptStateBytes,
                field: "tx_instructions[0].payload_hex"
            )
        )
    }

    static func requireFramedHex(
        _ value: Any?,
        maximumBytes: Int,
        field: String
    ) throws -> String {
        guard let string = value as? String,
              !string.isEmpty,
              string.utf8.count <= 2 * maximumBytes,
              string.utf8.count.isMultiple(of: 2),
              string.utf8.allSatisfy({
                  (0x30...0x39).contains($0) || (0x61...0x66).contains($0)
              }),
              let data = Data(hexString: string),
              !data.isEmpty,
              data.count <= maximumBytes,
              let frame = noritoDecodeFrame(data),
              frame.header.compression == .none,
              data.prefix(NoritoHeader.encodedLength) == frame.header.encode() else {
            throw ToriiClientError.invalidPayload(
                "\(field) must contain one bounded canonical uncompressed NRT0 frame."
            )
        }
        return string
    }

    static func validateRequiredBodies(_ value: Any?) throws -> [String] {
        guard let entries = value as? [Any], (1...10).contains(entries.count) else {
            throw ToriiClientError.invalidPayload(
                "required_bodies must contain one through ten entries."
            )
        }
        var seen = Set<String>()
        var bodiesInOrder = [String]()
        bodiesInOrder.reserveCapacity(entries.count)
        for (index, raw) in entries.enumerated() {
            let entry = try exactObject(
                raw,
                fields: ["body", "decision_mode"],
                context: "required_bodies[\(index)]"
            )
            guard let body = entry["body"] as? String else {
                throw ToriiClientError.invalidPayload(
                    "required_bodies[\(index)].body must be a string."
                )
            }
            try requireBody(body, field: "required_bodies[\(index)].body")
            guard seen.insert(body).inserted else {
                throw ToriiClientError.invalidPayload("required_bodies contains a duplicate body.")
            }
            bodiesInOrder.append(body)
            let mode = try taggedUnit(
                entry["decision_mode"],
                tag: "mode",
                admitted: ["PublicFinding", "HiddenBindingBallot"],
                context: "required_bodies[\(index)].decision_mode"
            )
            let expected = privateBodies.contains(body) ? "HiddenBindingBallot" : "PublicFinding"
            guard mode == expected else {
                throw ToriiClientError.invalidPayload(
                    "required_bodies[\(index)] uses the wrong decision mode."
                )
            }
        }
        return bodiesInOrder
    }

    static func validateBodyStates(
        _ value: Any?,
        requiredBodies: [String]
    ) throws -> [ToriiParliamentBodyStateProjectionV1] {
        guard let entries = value as? [Any],
              entries.count == requiredBodies.count,
              (1...10).contains(entries.count) else {
            throw ToriiClientError.invalidPayload(
                "body_states must exactly match the required body pipeline."
            )
        }
        let noResultTags = Set(noResultKinds.map(\.jsonTag))
        let publicNoResultTags: Set<String> = [
            "PublicFindingQuorumUnreachable", "PublicFindingDeadlineExpired",
        ]
        var result = [ToriiParliamentBodyStateProjectionV1]()
        result.reserveCapacity(entries.count)
        for (index, raw) in entries.enumerated() {
            let context = "body_states[\(index)]"
            let entry = try exactObject(
                raw,
                fields: Set(bodyStateFields),
                context: context
            )
            guard let body = entry["body"] as? String, body == requiredBodies[index] else {
                throw ToriiClientError.invalidPayload(
                    "\(context).body differs from required_bodies order."
                )
            }

            let instanceIsNull = entry["body_instance_id"] is NSNull
            let statusIsNull = entry["status"] is NSNull
            guard instanceIsNull == statusIsNull else {
                throw ToriiClientError.invalidPayload(
                    "\(context) must bind body_instance_id and status together."
                )
            }
            let bodyInstanceId = instanceIsNull
                ? nil
                : try requireIdentifier(
                    entry["body_instance_id"],
                    field: "\(context).body_instance_id"
                )

            var status: String?
            var phase: String?
            if !statusIsNull {
                guard let object = entry["status"] as? [String: Any],
                      let parsedStatus = object["status"] as? String,
                      bodyStatuses.contains(parsedStatus) else {
                    throw ToriiClientError.invalidPayload("\(context).status is unknown.")
                }
                if parsedStatus == "Deliberating" {
                    guard Set(object.keys) == ["status", "phase"] else {
                        throw ToriiClientError.invalidPayload(
                            "\(context).status contains unknown, aliased, or missing fields."
                        )
                    }
                    phase = try taggedUnit(
                        object["phase"],
                        tag: "phase",
                        admitted: deliberationPhases,
                        context: "\(context).status.phase"
                    )
                } else {
                    guard Set(object.keys) == ["status"] else {
                        throw ToriiClientError.invalidPayload(
                            "\(context).status contains unknown, aliased, or missing fields."
                        )
                    }
                }
                status = parsedStatus
            }

            let opened = try optionalUnsigned(
                entry["public_finding_opened_at_height"],
                field: "\(context).public_finding_opened_at_height"
            )
            let phaseBlocks = try optionalUnsigned(
                entry["public_finding_phase_blocks"],
                field: "\(context).public_finding_phase_blocks"
            )
            let deadline = try optionalUnsigned(
                entry["public_finding_deadline_height"],
                field: "\(context).public_finding_deadline_height"
            )
            guard (opened == nil) == (phaseBlocks == nil),
                  (opened == nil) == (deadline == nil) else {
                throw ToriiClientError.invalidPayload(
                    "\(context) must expose the complete public-finding schedule or none."
                )
            }
            if let opened, let phaseBlocks, let deadline {
                let (expectedDeadline, overflow) = opened.addingReportingOverflow(phaseBlocks)
                guard phaseBlocks > 0,
                      !privateBodies.contains(body),
                      !overflow,
                      deadline == expectedDeadline else {
                    throw ToriiClientError.invalidPayload(
                        "\(context) public-finding deadline does not match its frozen schedule."
                    )
                }
            }

            let kindIsNull = entry["no_result_kind"] is NSNull
            let heightIsNull = entry["no_result_height"] is NSNull
            guard kindIsNull == heightIsNull else {
                throw ToriiClientError.invalidPayload(
                    "\(context) must bind no-result kind and height together."
                )
            }
            let noResultKind = kindIsNull
                ? nil
                : try taggedUnit(
                    entry["no_result_kind"],
                    tag: "reason",
                    admitted: noResultTags,
                    context: "\(context).no_result_kind"
                )
            let noResultHeight = try optionalUnsigned(
                entry["no_result_height"],
                field: "\(context).no_result_height"
            )
            if let noResultKind {
                guard status == "NoResult",
                      publicNoResultTags.contains(noResultKind) != privateBodies.contains(body) else {
                    throw ToriiClientError.invalidPayload(
                        "\(context) no-result facts do not match its lifecycle and decision protocol."
                    )
                }
            }
            result.append(.init(
                body: body,
                bodyInstanceId: bodyInstanceId,
                status: status,
                deliberationPhase: phase,
                publicFindingOpenedAtHeight: opened,
                publicFindingPhaseBlocks: phaseBlocks,
                publicFindingDeadlineHeight: deadline,
                noResultKind: noResultKind,
                noResultHeight: noResultHeight
            ))
        }
        return result
    }

    static func validateCertificate(
        _ value: Any?,
        expectedAttemptId: String
    ) throws -> [ToriiParliamentPublicFindingCertificateBindingV1] {
        guard let value else {
            throw ToriiClientError.invalidPayload("certificate is missing.")
        }
        if value is NSNull { return [] }
        let certificate = try exactObject(
            value,
            fields: [
                "proposal_content_id", "governance_attempt_id", "governance_attempt_sequence",
                "risk_tier", "body_bindings", "policy_version", "effect_preimage_hash",
                "expected_head", "certified_at_height", "enact_at_height",
            ],
            context: "certificate"
        )
        _ = try requireIdentifier(
            certificate["proposal_content_id"],
            field: "certificate.proposal_content_id"
        )
        guard try requireIdentifier(
            certificate["governance_attempt_id"],
            field: "certificate.governance_attempt_id"
        ) == expectedAttemptId else {
            throw ToriiClientError.invalidPayload(
                "certificate.governance_attempt_id differs from attempt.id."
            )
        }
        _ = try unsigned32(
            certificate["governance_attempt_sequence"],
            field: "certificate.governance_attempt_sequence"
        )
        _ = try taggedUnit(
            certificate["risk_tier"],
            tag: "tier",
            admitted: riskTiers,
            context: "certificate.risk_tier"
        )
        _ = try unsigned(certificate["policy_version"], field: "certificate.policy_version")
        _ = try fixedBytes(
            certificate["effect_preimage_hash"],
            count: 32,
            nonzero: true,
            field: "certificate.effect_preimage_hash"
        )
        try validateExpectedHead(certificate["expected_head"], context: "certificate.expected_head")
        _ = try unsigned(
            certificate["certified_at_height"],
            field: "certificate.certified_at_height"
        )
        _ = try unsigned(
            certificate["enact_at_height"],
            field: "certificate.enact_at_height"
        )
        guard let bindings = certificate["body_bindings"] as? [Any],
              (1...10).contains(bindings.count) else {
            throw ToriiClientError.invalidPayload(
                "certificate.body_bindings must contain one through ten entries."
            )
        }
        var findings = [ToriiParliamentPublicFindingCertificateBindingV1]()
        for (index, binding) in bindings.enumerated() {
            if let finding = try validateBodyBinding(binding, index: index) {
                findings.append(finding)
            }
        }
        return findings
    }

    static func validateBodyBinding(
        _ value: Any,
        index: Int
    ) throws -> ToriiParliamentPublicFindingCertificateBindingV1? {
        let context = "certificate.body_bindings[\(index)]"
        let binding = try exactObject(
            value,
            fields: Set(certificateBodyBindingNoritoFields),
            context: context
        )
        for field in [
            "body_instance_id", "election_attempt_id", "sortition_request_id",
            "beacon_session_id", "beacon_pulse_id",
        ] {
            _ = try requireIdentifier(binding[field], field: "\(context).\(field)")
        }
        for field in ["roster_root", "assignment_root", "result_root"] {
            _ = try fixedBytes(
                binding[field],
                count: 32,
                nonzero: true,
                field: "\(context).\(field)"
            )
        }
        _ = try unsigned32(
            binding["election_attempt_sequence"],
            field: "\(context).election_attempt_sequence"
        )
        let seats = try boundedUInt32(
            binding["original_seats"],
            minimum: 1,
            maximum: UInt32(maximumCorpusEntries),
            field: "\(context).original_seats"
        )
        _ = try unsigned(binding["result_height"], field: "\(context).result_height")
        guard let body = binding["body"] as? String else {
            throw ToriiClientError.invalidPayload("\(context).body must be a string.")
        }
        try requireBody(body, field: "\(context).body")
        try validateSortitionRequest(
            binding["sortition_request"],
            expectedBody: body,
            context: "\(context).sortition_request"
        )
        let publicFinding = binding["public_finding"]
        let ballot = binding["ballot"]
        if privateBodies.contains(body) {
            guard publicFinding is NSNull, !(ballot is NSNull), let ballot else {
                throw ToriiClientError.invalidPayload(
                    "\(context) private jury must carry ballot only."
                )
            }
            try validateBallotBinding(ballot, originalSeats: seats, context: "\(context).ballot")
            return nil
        }
        guard !(publicFinding is NSNull), let publicFinding, ballot is NSNull else {
            throw ToriiClientError.invalidPayload(
                "\(context) public body must carry public_finding only."
            )
        }
        return try validatePublicFinding(
            publicFinding,
            originalSeats: seats,
            context: "\(context).public_finding"
        )
    }

    static func validateSortitionRequest(
        _ value: Any?,
        expectedBody: String,
        context: String
    ) throws {
        let request = try exactObject(
            value,
            fields: [
                "id", "governance_attempt_id", "body_election_attempt_id", "body",
                "candidate_root", "candidate_count", "target_seats", "request_height",
                "pulse_height", "beacon_session_id",
            ],
            context: context
        )
        for field in ["id", "governance_attempt_id", "body_election_attempt_id", "beacon_session_id"] {
            _ = try requireIdentifier(request[field], field: "\(context).\(field)")
        }
        _ = try fixedBytes(
            request["candidate_root"],
            count: 32,
            nonzero: true,
            field: "\(context).candidate_root"
        )
        _ = try boundedUInt32(
            request["candidate_count"],
            minimum: 1,
            maximum: UInt32(maximumCorpusEntries),
            field: "\(context).candidate_count"
        )
        _ = try boundedUInt32(
            request["target_seats"],
            minimum: 1,
            maximum: UInt32(maximumCorpusEntries),
            field: "\(context).target_seats"
        )
        _ = try unsigned(request["request_height"], field: "\(context).request_height")
        _ = try unsigned(request["pulse_height"], field: "\(context).pulse_height")
        guard request["body"] as? String == expectedBody else {
            throw ToriiClientError.invalidPayload("\(context).body differs from its binding.")
        }
    }

    static func validatePublicFinding(
        _ value: Any,
        originalSeats: UInt32,
        context: String
    ) throws -> ToriiParliamentPublicFindingCertificateBindingV1 {
        let finding = try exactObject(
            value,
            fields: Set(publicFindingCertificateNoritoFields),
            context: context
        )
        let root = try fixedBytes(
            finding["endorsement_root"],
            count: 32,
            nonzero: true,
            field: "\(context).endorsement_root"
        )
        guard let assignments = finding["endorsing_assignments"] as? [String] else {
            throw ToriiClientError.invalidPayload(
                "\(context).endorsing_assignments must be an identifier array."
            )
        }
        try requireStrictIdentifiers(assignments, field: "\(context).endorsing_assignments")
        let endorsements = try boundedUInt32(
            finding["endorsements"],
            minimum: 1,
            maximum: UInt32(maximumCorpusEntries),
            field: "\(context).endorsements"
        )
        let quorum = try boundedUInt32(
            finding["quorum"],
            minimum: 1,
            maximum: UInt32(maximumCorpusEntries),
            field: "\(context).quorum"
        )
        let expectedQuorum = (2 * originalSeats + 2) / 3
        guard UInt32(assignments.count) == endorsements,
              endorsements == quorum,
              quorum == expectedQuorum else {
            throw ToriiClientError.invalidPayload(
                "\(context) must contain the exact canonical two-thirds supporter list."
            )
        }
        return .init(
            endorsementRoot: root,
            endorsingAssignments: assignments,
            endorsements: endorsements,
            quorum: quorum
        )
    }

    static func validateBallotBinding(
        _ value: Any,
        originalSeats: UInt32,
        context: String
    ) throws {
        let ballot = try exactObject(
            value,
            fields: [
                "ballot_attempt_id", "ballot_attempt_sequence", "tle_session_id",
                "tle_key_session_id", "registration_root", "dropout_root", "survivor_root",
                "corpus_root", "no_recovery_root", "timed_commitment_root",
                "release_beacon_session_id", "registered_at_height", "registration_close_height",
                "survivor_freeze_height", "commitment_close_height",
                "registration_closed_at_height", "survivors_frozen_at_height",
                "commitment_closed_at_height", "max_ballot_retries", "max_corpus_entries",
                "release_height", "opening_deadline_height", "release_pulse_id",
                "opening_height", "opening_root", "tally", "outcome",
            ],
            context: context
        )
        for field in [
            "ballot_attempt_id", "tle_session_id", "tle_key_session_id",
            "release_beacon_session_id", "release_pulse_id",
        ] {
            _ = try requireIdentifier(ballot[field], field: "\(context).\(field)")
        }
        for field in [
            "registration_root", "dropout_root", "survivor_root", "corpus_root",
            "no_recovery_root", "timed_commitment_root", "opening_root",
        ] {
            _ = try fixedBytes(
                ballot[field],
                count: 32,
                nonzero: true,
                field: "\(context).\(field)"
            )
        }
        _ = try unsigned32(
            ballot["ballot_attempt_sequence"],
            field: "\(context).ballot_attempt_sequence"
        )
        for field in [
            "registered_at_height", "registration_close_height", "survivor_freeze_height",
            "commitment_close_height", "registration_closed_at_height",
            "survivors_frozen_at_height", "commitment_closed_at_height", "release_height",
            "opening_deadline_height", "opening_height",
        ] {
            _ = try unsigned(ballot[field], field: "\(context).\(field)")
        }
        _ = try boundedUInt32(
            ballot["max_ballot_retries"],
            minimum: 1,
            maximum: 16,
            field: "\(context).max_ballot_retries"
        )
        _ = try boundedUInt32(
            ballot["max_corpus_entries"],
            minimum: 1,
            maximum: UInt32(maximumCorpusEntries),
            field: "\(context).max_corpus_entries"
        )
        let tally = try exactObject(
            ballot["tally"],
            fields: ["original_seats", "accepted_ballots", "aye", "nay", "abstain"],
            context: "\(context).tally"
        )
        guard try boundedUInt32(
            tally["original_seats"],
            minimum: 1,
            maximum: UInt32(maximumCorpusEntries),
            field: "\(context).tally.original_seats"
        ) == originalSeats else {
            throw ToriiClientError.invalidPayload(
                "\(context).tally.original_seats differs from its body binding."
            )
        }
        let accepted = try boundedUInt32(
            tally["accepted_ballots"],
            minimum: 0,
            maximum: UInt32(maximumCorpusEntries),
            field: "\(context).tally.accepted_ballots"
        )
        let aye = try unsigned32(tally["aye"], field: "\(context).tally.aye")
        let nay = try unsigned32(tally["nay"], field: "\(context).tally.nay")
        let abstain = try unsigned32(tally["abstain"], field: "\(context).tally.abstain")
        guard UInt64(aye) + UInt64(nay) + UInt64(abstain) == UInt64(accepted),
              accepted <= originalSeats else {
            throw ToriiClientError.invalidPayload("\(context).tally is internally inconsistent.")
        }
        _ = try taggedUnit(
            ballot["outcome"],
            tag: "outcome",
            admitted: ["Approved", "Rejected", "NoQuorum", "NoResult"],
            context: "\(context).outcome"
        )
    }

    static func validateExpectedHead(_ value: Any?, context: String) throws {
        let root = try exactObject(value, fields: ["state", "head"], context: context)
        guard let state = root["state"] as? String else {
            throw ToriiClientError.invalidPayload("\(context).state must be a string.")
        }
        switch state {
        case "Absent":
            let head = try exactObject(
                root["head"], fields: ["subject_id"], context: "\(context).head"
            )
            _ = try fixedBytes(
                head["subject_id"],
                count: 32,
                nonzero: true,
                field: "\(context).head.subject_id"
            )
        case "Present":
            let head = try exactObject(
                root["head"],
                fields: ["subject_id", "version", "head_root"],
                context: "\(context).head"
            )
            _ = try fixedBytes(
                head["subject_id"],
                count: 32,
                nonzero: true,
                field: "\(context).head.subject_id"
            )
            _ = try unsigned(head["version"], field: "\(context).head.version")
            _ = try fixedBytes(
                head["head_root"],
                count: 32,
                nonzero: true,
                field: "\(context).head.head_root"
            )
        default:
            throw ToriiClientError.invalidPayload("\(context).state is unknown.")
        }
    }
}
