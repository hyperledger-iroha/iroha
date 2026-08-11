import Foundation

// Governance ballot wire models and validation shared by Torii submission APIs.

/// A governance vote direction as encoded by Torii ballot routes.
public enum ToriiGovernanceBallotDirection: String, Codable, Sendable {
    case aye = "Aye"
    case nay = "Nay"
    case abstain = "Abstain"
}
fileprivate struct NormalizedGovernanceZkPublicInputs {
    var rootHint: String?
    var owner: String?
    var amount: String?
    var durationBlocks: UInt64?
    var direction: ToriiGovernanceBallotDirection?
    var nullifier: String?
}

/// The complete public-input surface accepted by governance ZK ballot routes.
///
/// Signing keys, witnesses, and arbitrary extension fields are intentionally not
/// representable. The three lock hints are atomic: callers either omit all of
/// `owner`, `amount`, and `durationBlocks`, or provide all three.
public struct GovernanceZkBallotPublicInputs: Encodable, Sendable {
    public var rootHint: String?
    public var owner: String?
    public var amount: String?
    public var durationBlocks: UInt64?
    public var direction: ToriiGovernanceBallotDirection?
    public var nullifier: String?

    private enum CodingKeys: String, CodingKey {
        case rootHint = "root_hint"
        case owner
        case amount
        case durationBlocks = "duration_blocks"
        case direction
        case nullifier
    }

    public init(rootHint: String? = nil,
                owner: String? = nil,
                amount: String? = nil,
                durationBlocks: UInt64? = nil,
                direction: ToriiGovernanceBallotDirection? = nil,
                nullifier: String? = nil) {
        self.rootHint = rootHint
        self.owner = owner
        self.amount = amount
        self.durationBlocks = durationBlocks
        self.direction = direction
        self.nullifier = nullifier
    }

    fileprivate func normalized(field: String) throws -> NormalizedGovernanceZkPublicInputs {
        let hasOwner = owner != nil
        let hasAmount = amount != nil
        let hasDuration = durationBlocks != nil
        if (hasOwner || hasAmount || hasDuration) && !(hasOwner && hasAmount && hasDuration) {
            throw ToriiClientError.invalidPayload(
                "\(field) must include owner, amount, and duration_blocks when providing lock hints."
            )
        }

        let normalizedOwner: String? = try owner.map { raw in
            let canonical = try canonicalizeGovernanceZkOwnerLiteral(raw, field: field)
            guard canonical == raw else {
                throw ToriiClientError.invalidPayload(
                    "\(field).owner must use canonical I105 account id form."
                )
            }
            return canonical
        }
        return try NormalizedGovernanceZkPublicInputs(
            rootHint: rootHint.map {
                try canonicalizeGovernanceHex32($0, field: "\(field).root_hint")
            },
            owner: normalizedOwner,
            amount: amount.map {
                try canonicalGovernanceQuantity($0, field: "\(field).amount")
            },
            durationBlocks: durationBlocks,
            direction: direction,
            nullifier: nullifier.map {
                try canonicalizeGovernanceHex32($0, field: "\(field).nullifier")
            }
        )
    }

    public func encode(to encoder: Encoder) throws {
        let normalized = try normalized(field: "governance ZK public inputs")
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encodeIfPresent(normalized.rootHint, forKey: .rootHint)
        try container.encodeIfPresent(normalized.owner, forKey: .owner)
        try container.encodeIfPresent(normalized.amount, forKey: .amount)
        try container.encodeIfPresent(normalized.durationBlocks, forKey: .durationBlocks)
        try container.encodeIfPresent(normalized.direction, forKey: .direction)
        try container.encodeIfPresent(normalized.nullifier, forKey: .nullifier)
    }
}

func canonicalizeGovernanceHex32(_ raw: String, field: String) throws -> String {
    var body = raw
    if let colonIndex = body.firstIndex(of: ":") {
        let scheme = String(body[..<colonIndex])
        let rest = String(body[body.index(after: colonIndex)...])
        if scheme.isEmpty || scheme.lowercased() != "blake2b32" {
            throw ToriiClientError.invalidPayload("\(field) must be a 32-byte hex string.")
        }
        body = rest
    }
    if body.hasPrefix("0x") || body.hasPrefix("0X") {
        body = String(body.dropFirst(2))
    }
    guard body.count == 64, Data(hexString: body) != nil else {
        throw ToriiClientError.invalidPayload("\(field) must be a 32-byte hex string.")
    }
    return body.lowercased()
}

fileprivate func canonicalizeGovernanceZkOwnerLiteral(_ raw: String, field: String) throws -> String {
    let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !trimmed.isEmpty, trimmed == raw else {
        throw ToriiClientError.invalidPayload("\(field).owner must be a canonical I105 account id.")
    }
    if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
        throw ToriiClientError.invalidPayload("\(field).owner must be a canonical I105 account id.")
    }
    if trimmed.contains("@") {
        throw ToriiClientError.invalidPayload("\(field).owner must be a canonical I105 account id.")
    }
    do {
        let canonical = try exactCanonicalToriiAccountAddress(trimmed)
        return try canonical.address.toI105(
            networkPrefix: canonical.chainDiscriminant
        )
    } catch {
        throw ToriiClientError.invalidPayload("\(field).owner must be a canonical I105 account id.")
    }
}

fileprivate func canonicalGovernanceQuantity(_ value: String, field: String) throws -> String {
    do {
        return try KotodamaNumericV1Codec.decodeQuantityJSON(value).canonicalString
    } catch {
        throw ToriiClientError.invalidPayload(
            "\(field) must be a canonical non-negative Kotodama V1 Quantity string."
        )
    }
}

/// A plain governance ballot bound to one exact network.
public struct ToriiGovernancePlainBallotRequest: Encodable, Sendable {
    public var authority: String
    public var networkId: NetworkId
    public var referendumId: String
    public var owner: String
    public var amount: String
    public var durationBlocks: UInt64
    public var direction: ToriiGovernanceBallotDirection

    private enum CodingKeys: String, CodingKey {
        case authority
        case networkId = "network_id"
        case referendumId = "referendum_id"
        case owner
        case amount
        case durationBlocks = "duration_blocks"
        case direction
    }

    public init(authority: String,
                networkId: NetworkId,
                referendumId: String,
                owner: String,
                amount: String,
                durationBlocks: UInt64,
                direction: ToriiGovernanceBallotDirection) {
        self.authority = authority
        self.networkId = networkId
        self.referendumId = referendumId
        self.owner = owner
        self.amount = amount
        self.durationBlocks = durationBlocks
        self.direction = direction
    }

    public func encode(to encoder: Encoder) throws {
        let canonicalAmount = try canonicalGovernanceQuantity(
            amount,
            field: "governance plain ballot amount"
        )
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(
            ToriiRequestValidation.exactToken(authority, field: "authority"),
            forKey: .authority
        )
        try container.encode(
            networkId,
            forKey: .networkId
        )
        try container.encode(
            ToriiRequestValidation.governanceSelector(referendumId, field: "referendum_id"),
            forKey: .referendumId
        )
        try container.encode(
            ToriiRequestValidation.exactToken(owner, field: "owner"),
            forKey: .owner
        )
        try container.encode(canonicalAmount, forKey: .amount)
        try container.encode(String(durationBlocks), forKey: .durationBlocks)
        try container.encode(direction, forKey: .direction)
    }
}

/// A version-one ZK ballot envelope bound to one exact network.
public struct ToriiGovernanceZkBallotV1Request: Encodable, Sendable {
    public var authority: String
    public var networkId: NetworkId
    public var electionId: String
    public var backend: String
    public var envelopeB64: String
    public var publicInputs: GovernanceZkBallotPublicInputs

    private enum CodingKeys: String, CodingKey {
        case authority
        case networkId = "network_id"
        case electionId = "election_id"
        case backend
        case envelopeB64 = "envelope_b64"
        case rootHint = "root_hint"
        case owner
        case amount
        case durationBlocks = "duration_blocks"
        case direction
        case nullifier
    }

    public init(authority: String,
                networkId: NetworkId,
                electionId: String,
                backend: String,
                envelopeB64: String,
                publicInputs: GovernanceZkBallotPublicInputs = .init()) {
        self.authority = authority
        self.networkId = networkId
        self.electionId = electionId
        self.backend = backend
        self.envelopeB64 = envelopeB64
        self.publicInputs = publicInputs
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedPublic = try publicInputs.normalized(
            field: "governance ZK ballot V1 public inputs"
        )
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(
            ToriiRequestValidation.exactToken(authority, field: "authority"),
            forKey: .authority
        )
        try container.encode(
            networkId,
            forKey: .networkId
        )
        try container.encode(
            ToriiRequestValidation.governanceSelector(electionId, field: "election_id"),
            forKey: .electionId
        )
        try container.encode(
            ToriiRequestValidation.exactToken(backend, field: "backend"),
            forKey: .backend
        )
        try container.encode(
            ToriiRequestValidation.normalizedExactBase64(envelopeB64, field: "envelope_b64"),
            forKey: .envelopeB64
        )
        try container.encodeIfPresent(normalizedPublic.rootHint, forKey: .rootHint)
        try container.encodeIfPresent(normalizedPublic.owner, forKey: .owner)
        try container.encodeIfPresent(normalizedPublic.amount, forKey: .amount)
        try container.encodeIfPresent(normalizedPublic.durationBlocks, forKey: .durationBlocks)
        try container.encodeIfPresent(normalizedPublic.direction, forKey: .direction)
        try container.encodeIfPresent(normalizedPublic.nullifier, forKey: .nullifier)
    }
}

/// The typed proof nested in a version-one ZK ballot-proof request.
public struct ToriiGovernanceBallotProof: Encodable, Sendable {
    public var backend: String
    public var envelopeBytesB64: String
    public var publicInputs: GovernanceZkBallotPublicInputs

    private enum CodingKeys: String, CodingKey {
        case backend
        case envelopeBytesB64 = "envelope_bytes"
        case rootHint = "root_hint"
        case owner
        case nullifier
        case amount
        case durationBlocks = "duration_blocks"
        case direction
    }

    public init(backend: String,
                envelopeBytesB64: String,
                publicInputs: GovernanceZkBallotPublicInputs = .init()) {
        self.backend = backend
        self.envelopeBytesB64 = envelopeBytesB64
        self.publicInputs = publicInputs
    }

    public func encode(to encoder: Encoder) throws {
        let normalizedPublic = try publicInputs.normalized(
            field: "governance ballot proof public inputs"
        )
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(
            ToriiRequestValidation.exactToken(backend, field: "backend"),
            forKey: .backend
        )
        try container.encode(
            ToriiRequestValidation.normalizedExactBase64(
                envelopeBytesB64,
                field: "envelope_bytes"
            ),
            forKey: .envelopeBytesB64
        )
        try container.encodeIfPresent(normalizedPublic.rootHint, forKey: .rootHint)
        try container.encodeIfPresent(normalizedPublic.owner, forKey: .owner)
        try container.encodeIfPresent(normalizedPublic.nullifier, forKey: .nullifier)
        try container.encodeIfPresent(normalizedPublic.amount, forKey: .amount)
        try container.encodeIfPresent(normalizedPublic.durationBlocks, forKey: .durationBlocks)
        try container.encodeIfPresent(normalizedPublic.direction, forKey: .direction)
    }
}

/// A version-one ZK ballot proof bound to one exact network.
public struct ToriiGovernanceZkBallotProofRequest: Encodable, Sendable {
    public var authority: String
    public var networkId: NetworkId
    public var electionId: String
    public var ballot: ToriiGovernanceBallotProof

    private enum CodingKeys: String, CodingKey {
        case authority
        case networkId = "network_id"
        case electionId = "election_id"
        case ballot
    }

    public init(authority: String,
                networkId: NetworkId,
                electionId: String,
                ballot: ToriiGovernanceBallotProof) {
        self.authority = authority
        self.networkId = networkId
        self.electionId = electionId
        self.ballot = ballot
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(
            ToriiRequestValidation.exactToken(authority, field: "authority"),
            forKey: .authority
        )
        try container.encode(
            networkId,
            forKey: .networkId
        )
        try container.encode(
            ToriiRequestValidation.governanceSelector(electionId, field: "election_id"),
            forKey: .electionId
        )
        try container.encode(ballot, forKey: .ballot)
    }
}

/// A parliament body eligible to cast a governance ballot.
public enum ToriiGovernanceParliamentBody: String, Codable, Sendable {
    case rulesCommittee = "rules-committee"
    case agendaCouncil = "agenda-council"
    case interestPanel = "interest-panel"
    case reviewPanel = "review-panel"
    case policyJury = "policy-jury"
    case oversightCommittee = "oversight-committee"
    case fmaCommittee = "fma-committee"
}

/// A parliament ballot decision.
public enum ToriiGovernanceParliamentDecision: String, Codable, Sendable {
    case approve
    case reject
    case abstain
}

/// A parliament ballot bound to one exact network.
public struct ToriiGovernanceParliamentBallotRequest: Encodable, Sendable {
    public var authority: String
    public var networkId: NetworkId
    public var proposalId: String
    public var body: ToriiGovernanceParliamentBody
    public var decision: ToriiGovernanceParliamentDecision

    private enum CodingKeys: String, CodingKey {
        case authority
        case networkId = "network_id"
        case proposalId = "proposal_id"
        case body
        case decision
    }

    public init(authority: String,
                networkId: NetworkId,
                proposalId: String,
                body: ToriiGovernanceParliamentBody,
                decision: ToriiGovernanceParliamentDecision) {
        self.authority = authority
        self.networkId = networkId
        self.proposalId = proposalId
        self.body = body
        self.decision = decision
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(
            ToriiRequestValidation.exactToken(authority, field: "authority"),
            forKey: .authority
        )
        try container.encode(
            networkId,
            forKey: .networkId
        )
        try container.encode(
            canonicalizeGovernanceHex32(proposalId, field: "proposal_id"),
            forKey: .proposalId
        )
        try container.encode(body, forKey: .body)
        try container.encode(decision, forKey: .decision)
    }
}
