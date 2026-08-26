import Foundation

/// A locally encoded first-release Musubi registry instruction.
public protocol MusubiInstructionV1: Sendable {
    /// Stable dynamic instruction identifier carried by `InstructionBox`.
    var wireID: String { get }

    /// Compiler-emitted Rust type name whose hash identifies the concrete payload frame.
    var concreteSchemaName: String { get }

    /// Encode the concrete instruction without its Norito header.
    func barePayload() throws -> Data
}

public extension MusubiInstructionV1 {
    /// Build the dynamic instruction frame embedded directly in a transaction.
    func transactionInstructionFrame() throws -> TransactionInstructionFrame {
        let frame = noritoEncode(
            typeName: concreteSchemaName,
            payload: try barePayload(),
            flags: NoritoHeader.compactLen,
            payloadAlignment: 8
        )
        return try TransactionInstructionFrame(wireName: wireID, framedPayload: frame)
    }

    /// Encode the standalone `InstructionBox` frame returned by Torii instruction builders.
    func standaloneInstructionBoxFrame() throws -> Data {
        let instruction = try transactionInstructionFrame()
        return noritoEncode(
            typeName: MusubiInstructionNoritoV1.instructionBoxSchema,
            payload: try instruction.compactInstructionBoxPayload(),
            flags: NoritoHeader.compactLen,
            payloadAlignment: 8
        )
    }
}

/// Accept a pending package-maintainer invitation as the invited account.
public struct AcceptMusubiPackageMaintainerV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.package_member.accept"
    public static let schemaName =
        "iroha_data_model::isi::musubi::AcceptMusubiPackageMaintainerV1"

    public let package: MusubiPackageIdV1
    public let inviteID: MusubiDigest32V1
    public let expectedGovernanceRevision: UInt64

    public init(
        package: MusubiPackageIdV1,
        inviteID: MusubiDigest32V1,
        expectedGovernanceRevision: UInt64
    ) {
        self.package = package
        self.inviteID = inviteID
        self.expectedGovernanceRevision = expectedGovernanceRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        MusubiInstructionNoritoV1.packageInvitationMutation(
            package: package,
            inviteID: inviteID,
            expectedGovernanceRevision: expectedGovernanceRevision
        )
    }
}

/// Revoke a pending package-maintainer invitation as a current owner.
public struct RevokeMusubiPackageMaintainerInvitationV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.package_member.invitation.revoke"
    public static let schemaName =
        "iroha_data_model::isi::musubi::RevokeMusubiPackageMaintainerInvitationV1"

    public let package: MusubiPackageIdV1
    public let inviteID: MusubiDigest32V1
    public let expectedGovernanceRevision: UInt64

    public init(
        package: MusubiPackageIdV1,
        inviteID: MusubiDigest32V1,
        expectedGovernanceRevision: UInt64
    ) {
        self.package = package
        self.inviteID = inviteID
        self.expectedGovernanceRevision = expectedGovernanceRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        MusubiInstructionNoritoV1.packageInvitationMutation(
            package: package,
            inviteID: inviteID,
            expectedGovernanceRevision: expectedGovernanceRevision
        )
    }
}

/// Register a paid permanent global alias for one structural package identity.
public struct RegisterMusubiAliasV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.alias.register"
    public static let schemaName = "iroha_data_model::isi::musubi::RegisterMusubiAliasV1"

    public let alias: MusubiAliasNameV1
    public let target: MusubiPackageIdV1
    public let expectedPricingRevision: UInt64

    public init(
        alias: MusubiAliasNameV1,
        target: MusubiPackageIdV1,
        expectedPricingRevision: UInt64
    ) {
        self.alias = alias
        self.target = target
        self.expectedPricingRevision = expectedPricingRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.aliasName(alias))
        writer.writeField(MusubiInstructionNoritoV1.packageID(target))
        writer.writeField(CompactNorito.encodeUInt64(expectedPricingRevision))
        return writer.data
    }
}

/// Assert the immutable digest of one exact package release.
public struct AssertMusubiReleaseDigestV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.release_digest.assert"
    public static let schemaName =
        "iroha_data_model::isi::musubi::AssertMusubiReleaseDigestV1"

    public let release: MusubiReleaseIdV1
    public let expectedDigest: MusubiDigest32V1

    public init(release: MusubiReleaseIdV1, expectedDigest: MusubiDigest32V1) {
        self.release = release
        self.expectedDigest = expectedDigest
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.releaseID(release))
        writer.writeField(MusubiInstructionNoritoV1.digest32(expectedDigest))
        return writer.data
    }
}

/// Retire one exact archive location without changing archive identity.
public struct RetireMusubiArchiveLocationV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.archive_location.retire"
    public static let schemaName =
        "iroha_data_model::isi::musubi::RetireMusubiArchiveLocationV1"

    public let archiveID: MusubiDigest32V1
    public let locationID: MusubiDigest32V1
    public let expectedLocationRevision: UInt64
    public let reason: MusubiReasonV1

    public init(
        archiveID: MusubiDigest32V1,
        locationID: MusubiDigest32V1,
        expectedLocationRevision: UInt64,
        reason: MusubiReasonV1
    ) {
        self.archiveID = archiveID
        self.locationID = locationID
        self.expectedLocationRevision = expectedLocationRevision
        self.reason = reason
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.digest32(archiveID))
        writer.writeField(MusubiInstructionNoritoV1.digest32(locationID))
        writer.writeField(CompactNorito.encodeUInt64(expectedLocationRevision))
        writer.writeField(MusubiInstructionNoritoV1.reason(reason))
        return writer.data
    }
}

/// Yank or unyank one immutable release using compare-and-set governance.
public struct SetMusubiReleaseYankV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.release_yank.set"
    public static let schemaName = "iroha_data_model::isi::musubi::SetMusubiReleaseYankV1"

    public let release: MusubiReleaseIdV1
    public let yanked: Bool
    public let reason: MusubiReasonV1
    public let expectedYankRevision: UInt64

    public init(
        release: MusubiReleaseIdV1,
        yanked: Bool,
        reason: MusubiReasonV1,
        expectedYankRevision: UInt64
    ) {
        self.release = release
        self.yanked = yanked
        self.reason = reason
        self.expectedYankRevision = expectedYankRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.releaseID(release))
        writer.writeField(CompactNorito.encodeBool(yanked))
        writer.writeField(MusubiInstructionNoritoV1.reason(reason))
        writer.writeField(CompactNorito.encodeUInt64(expectedYankRevision))
        return writer.data
    }
}

/// Remove one accepted package member while preserving Core's last-owner guard.
public struct RemoveMusubiPackageMaintainerV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.package_member.remove"
    public static let schemaName =
        "iroha_data_model::isi::musubi::RemoveMusubiPackageMaintainerV1"

    public let package: MusubiPackageIdV1
    public let account: String
    public let expectedGovernanceRevision: UInt64

    public init(
        package: MusubiPackageIdV1,
        account: String,
        expectedGovernanceRevision: UInt64
    ) throws {
        _ = try CanonicalNorito.encodeCompactAccountId(account)
        self.package = package
        self.account = account
        self.expectedGovernanceRevision = expectedGovernanceRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.packageID(package))
        writer.writeField(try CanonicalNorito.encodeCompactAccountId(account))
        writer.writeField(CompactNorito.encodeUInt64(expectedGovernanceRevision))
        return writer.data
    }
}

/// Register one immutable public namespace binding.
public struct RegisterMusubiNamespaceBindingV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.namespace_binding.register"
    public static let schemaName =
        "iroha_data_model::isi::musubi::RegisterMusubiNamespaceBindingV1"

    public let binding: MusubiNamespaceBindingV1
    public let expectedPolicyRevision: UInt64

    public init(binding: MusubiNamespaceBindingV1, expectedPolicyRevision: UInt64) {
        self.binding = binding
        self.expectedPolicyRevision = expectedPolicyRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.namespaceBinding(binding))
        writer.writeField(CompactNorito.encodeUInt64(expectedPolicyRevision))
        return writer.data
    }
}

/// Invite one canonical account to a package owner or maintainer role.
public struct InviteMusubiPackageMaintainerV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.package_member.invite"
    public static let schemaName =
        "iroha_data_model::isi::musubi::InviteMusubiPackageMaintainerV1"

    public let package: MusubiPackageIdV1
    public let inviteID: MusubiDigest32V1
    public let invitedAccount: String
    public let role: MusubiPackageRoleV1
    public let expiresAtHeight: UInt64
    public let expectedGovernanceRevision: UInt64

    public init(
        package: MusubiPackageIdV1,
        inviteID: MusubiDigest32V1,
        invitedAccount: String,
        role: MusubiPackageRoleV1,
        expiresAtHeight: UInt64,
        expectedGovernanceRevision: UInt64
    ) throws {
        guard inviteID.bytes.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi maintainer invite ID must not be zero.")
        }
        guard expiresAtHeight > 0 else {
            throw MusubiV1Error.invalidValue(
                "Musubi maintainer invitation expiry must be non-zero."
            )
        }
        _ = try CanonicalNorito.encodeCompactAccountId(invitedAccount)
        self.package = package
        self.inviteID = inviteID
        self.invitedAccount = invitedAccount
        self.role = role
        self.expiresAtHeight = expiresAtHeight
        self.expectedGovernanceRevision = expectedGovernanceRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.packageID(package))
        writer.writeField(MusubiInstructionNoritoV1.digest32(inviteID))
        writer.writeField(try CanonicalNorito.encodeCompactAccountId(invitedAccount))
        writer.writeField(MusubiInstructionNoritoV1.packageRole(role))
        writer.writeField(CompactNorito.encodeUInt64(expiresAtHeight))
        writer.writeField(CompactNorito.encodeUInt64(expectedGovernanceRevision))
        return writer.data
    }
}

/// Replace one accepted package member's role.
public struct SetMusubiPackageMaintainerRoleV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.package_member.set_role"
    public static let schemaName =
        "iroha_data_model::isi::musubi::SetMusubiPackageMaintainerRoleV1"

    public let package: MusubiPackageIdV1
    public let account: String
    public let role: MusubiPackageRoleV1
    public let expectedGovernanceRevision: UInt64

    public init(
        package: MusubiPackageIdV1,
        account: String,
        role: MusubiPackageRoleV1,
        expectedGovernanceRevision: UInt64
    ) throws {
        _ = try CanonicalNorito.encodeCompactAccountId(account)
        self.package = package
        self.account = account
        self.role = role
        self.expectedGovernanceRevision = expectedGovernanceRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.packageID(package))
        writer.writeField(try CanonicalNorito.encodeCompactAccountId(account))
        writer.writeField(MusubiInstructionNoritoV1.packageRole(role))
        writer.writeField(CompactNorito.encodeUInt64(expectedGovernanceRevision))
        return writer.data
    }
}

/// Recover package ownership under one enacted, delayed Parliament decision.
public struct RecoverMusubiPackageV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.parliament.package_recover"
    public static let schemaName =
        "iroha_data_model::isi::musubi::RecoverMusubiPackageV1"

    public let decision: MusubiGovernanceDecisionV1
    public let package: MusubiPackageIdV1
    public let owners: [String]
    public let expectedGovernanceRevision: UInt64
    private let ownerPayloads: [Data]

    public init(
        decision: MusubiGovernanceDecisionV1,
        package: MusubiPackageIdV1,
        owners: [String],
        expectedGovernanceRevision: UInt64
    ) throws {
        guard (1...64).contains(owners.count) else {
            throw MusubiV1Error.invalidValue(
                "Musubi package recovery must carry between 1 and 64 owners."
            )
        }
        guard expectedGovernanceRevision > 0 else {
            throw MusubiV1Error.invalidValue(
                "Musubi package recovery governance revision must be non-zero."
            )
        }

        var payloads: [Data] = []
        var orderKeys: [MusubiAccountOrderKeyV1] = []
        payloads.reserveCapacity(owners.count)
        orderKeys.reserveCapacity(owners.count)
        for owner in owners {
            let normalized = try musubiNormalizedAccountV1(owner)
            payloads.append(normalized.payload)
            orderKeys.append(normalized.orderKey)
        }
        for index in 1..<orderKeys.count {
            guard musubiCompareAccountOrderKeysV1(
                orderKeys[index - 1], orderKeys[index]
            ) < 0 else {
                throw MusubiV1Error.invalidValue(
                    "Musubi package recovery owners must be sorted and distinct in AccountId wire order."
                )
            }
        }

        self.decision = decision
        self.package = package
        self.owners = owners
        self.expectedGovernanceRevision = expectedGovernanceRevision
        self.ownerPayloads = payloads
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.governanceDecision(decision))
        writer.writeField(MusubiInstructionNoritoV1.packageID(package))
        writer.writeField(MusubiInstructionNoritoV1.accountIDs(ownerPayloads))
        writer.writeField(CompactNorito.encodeUInt64(expectedGovernanceRevision))
        return writer.data
    }
}

/// Retarget one permanent alias under an enacted Parliament recovery decision.
public struct RetargetMusubiAliasV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.parliament.alias_retarget"
    public static let schemaName =
        "iroha_data_model::isi::musubi::RetargetMusubiAliasV1"

    public let decision: MusubiGovernanceDecisionV1
    public let alias: MusubiAliasNameV1
    public let target: MusubiPackageIdV1
    public let expectedHistoryRevision: UInt64

    public init(
        decision: MusubiGovernanceDecisionV1,
        alias: MusubiAliasNameV1,
        target: MusubiPackageIdV1,
        expectedHistoryRevision: UInt64
    ) throws {
        guard expectedHistoryRevision > 0 else {
            throw MusubiV1Error.invalidValue(
                "Musubi alias history revision must be non-zero."
            )
        }
        self.decision = decision
        self.alias = alias
        self.target = target
        self.expectedHistoryRevision = expectedHistoryRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.governanceDecision(decision))
        writer.writeField(MusubiInstructionNoritoV1.aliasName(alias))
        writer.writeField(MusubiInstructionNoritoV1.packageID(target))
        writer.writeField(CompactNorito.encodeUInt64(expectedHistoryRevision))
        return writer.data
    }
}

/// Mark one exact release artifact unavailable under an enacted Parliament decision.
public struct SetMusubiArtifactTakedownV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.parliament.artifact_takedown"
    public static let schemaName =
        "iroha_data_model::isi::musubi::SetMusubiArtifactTakedownV1"

    public let decision: MusubiGovernanceDecisionV1
    public let release: MusubiReleaseIdV1
    public let reason: MusubiReasonV1
    public let expectedArtifactGovernanceRevision: UInt64

    public init(
        decision: MusubiGovernanceDecisionV1,
        release: MusubiReleaseIdV1,
        reason: MusubiReasonV1,
        expectedArtifactGovernanceRevision: UInt64
    ) throws {
        guard expectedArtifactGovernanceRevision > 0 else {
            throw MusubiV1Error.invalidValue(
                "Musubi artifact governance revision must be non-zero."
            )
        }
        self.decision = decision
        self.release = release
        self.reason = reason
        self.expectedArtifactGovernanceRevision = expectedArtifactGovernanceRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.governanceDecision(decision))
        writer.writeField(MusubiInstructionNoritoV1.releaseID(release))
        writer.writeField(MusubiInstructionNoritoV1.reason(reason))
        writer.writeField(CompactNorito.encodeUInt64(expectedArtifactGovernanceRevision))
        return writer.data
    }
}

/// Register one immutable source archive commitment and signed ingress receipt.
public struct RegisterMusubiArchiveV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.archive.register"
    public static let schemaName =
        "iroha_data_model::isi::musubi::RegisterMusubiArchiveV1"

    public let commitment: MusubiArchiveCommitmentV1
    public let stagingReceipt: MusubiSeedIngressReceiptV1
    public let expectedPolicyRevision: UInt64

    public init(
        commitment: MusubiArchiveCommitmentV1,
        stagingReceipt: MusubiSeedIngressReceiptV1,
        expectedPolicyRevision: UInt64
    ) throws {
        guard expectedPolicyRevision > 0 else {
            throw MusubiV1Error.invalidValue(
                "Musubi archive registration policy revision must be non-zero."
            )
        }
        let binding = stagingReceipt.payload.binding
        let commitmentPayload = MusubiInstructionNoritoV1.archiveCommitment(commitment)
        let archiveID = try MusubiInstructionNoritoV1.domainHash(
            domain: "iroha.musubi.archive-id.v1",
            payload: commitmentPayload
        )
        guard binding.archiveId.bytes == [UInt8](archiveID),
              binding.carBodyDigest == commitment.carDigest,
              binding.carBodyLength == commitment.carSize else {
            throw MusubiV1Error.invalidValue(
                "Musubi staging receipt does not bind the archive commitment."
            )
        }
        let approvals = try stagingReceipt.approvals.map {
            try MusubiControllerApprovalV1(publicKey: $0.publicKey, signature: $0.signature)
        }
        try musubiValidateControllerApprovalsV1(
            approvals,
            account: binding.ingressBroker,
            field: "seed-ingress broker"
        )
        self.commitment = commitment
        self.stagingReceipt = stagingReceipt
        self.expectedPolicyRevision = expectedPolicyRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.archiveCommitment(commitment))
        writer.writeField(try MusubiInstructionNoritoV1.seedIngressReceipt(stagingReceipt))
        writer.writeField(CompactNorito.encodeUInt64(expectedPolicyRevision))
        return writer.data
    }
}

/// Register one immutable provider attestation for later location-set commitments.
public struct RegisterMusubiProviderBundleAttestationV1: MusubiInstructionV1 {
    public static let stableWireID =
        "iroha.musubi.v1.provider_bundle_attestation.register"
    public static let schemaName =
        "iroha_data_model::isi::musubi::RegisterMusubiProviderBundleAttestationV1"

    public let attestation: MusubiProviderBundleVerificationAttestationV1
    public let expectedLocationRevision: UInt64

    public init(
        attestation: MusubiProviderBundleVerificationAttestationV1,
        expectedLocationRevision: UInt64
    ) throws {
        guard expectedLocationRevision > 0 else {
            throw MusubiV1Error.invalidValue(
                "Musubi provider attestation location revision must be non-zero."
            )
        }
        try musubiValidateControllerApprovalsV1(
            attestation.approvals,
            account: attestation.payload.binding.completionAuthority.providerOwner,
            field: "provider owner"
        )
        self.attestation = attestation
        self.expectedLocationRevision = expectedLocationRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try MusubiInstructionNoritoV1.providerAttestation(attestation))
        writer.writeField(CompactNorito.encodeUInt64(expectedLocationRevision))
        return writer.data
    }
}

/// Add or renew one finalized SoraFS location for a registered archive.
public struct AddMusubiArchiveLocationV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.archive_location.add"
    public static let schemaName =
        "iroha_data_model::isi::musubi::AddMusubiArchiveLocationV1"

    public let archiveID: MusubiDigest32V1
    public let locationID: MusubiDigest32V1
    public let pinManifest: MusubiDigest32V1
    public let replicationOrder: MusubiDigest32V1
    public let providerAttestationSetDigest: MusubiProviderBundleAttestationSetDigestV1
    public let renewAfterEpoch: UInt64
    public let expiresAtEpoch: UInt64
    public let expectedLocationRevision: UInt64

    public init(
        archiveID: MusubiDigest32V1,
        locationID: MusubiDigest32V1,
        pinManifest: MusubiDigest32V1,
        replicationOrder: MusubiDigest32V1,
        providerAttestationSetDigest: MusubiProviderBundleAttestationSetDigestV1,
        renewAfterEpoch: UInt64,
        expiresAtEpoch: UInt64,
        expectedLocationRevision: UInt64
    ) throws {
        guard [archiveID, locationID, pinManifest, replicationOrder]
            .allSatisfy({ $0.bytes.contains(where: { $0 != 0 }) }),
            providerAttestationSetDigest.bytes.contains(where: { $0 != 0 }),
            renewAfterEpoch < expiresAtEpoch,
            expectedLocationRevision > 0 else {
            throw MusubiV1Error.invalidValue("Musubi archive location request is invalid.")
        }
        self.archiveID = archiveID
        self.locationID = locationID
        self.pinManifest = pinManifest
        self.replicationOrder = replicationOrder
        self.providerAttestationSetDigest = providerAttestationSetDigest
        self.renewAfterEpoch = renewAfterEpoch
        self.expiresAtEpoch = expiresAtEpoch
        self.expectedLocationRevision = expectedLocationRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.digest32(archiveID))
        writer.writeField(MusubiInstructionNoritoV1.digest32(locationID))
        writer.writeField(MusubiInstructionNoritoV1.digest32(pinManifest))
        writer.writeField(MusubiInstructionNoritoV1.digest32(replicationOrder))
        writer.writeField(MusubiInstructionNoritoV1.digest32(providerAttestationSetDigest))
        writer.writeField(CompactNorito.encodeUInt64(renewAfterEpoch))
        writer.writeField(CompactNorito.encodeUInt64(expiresAtEpoch))
        writer.writeField(CompactNorito.encodeUInt64(expectedLocationRevision))
        return writer.data
    }
}

/// Claim or update one package and publish an immutable release.
public struct PublishMusubiReleaseV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.release.publish"
    public static let schemaName =
        "iroha_data_model::isi::musubi::PublishMusubiReleaseV1"

    public let namespace: MusubiNamespaceV1
    public let publication: MusubiPublicationV1
    public let namespaceDelegation: MusubiNamespaceDelegationV1?
    public let expectedPolicyRevision: UInt64
    public let expectedGovernanceRevision: UInt64?

    public init(
        namespace: MusubiNamespaceV1,
        publication: MusubiPublicationV1,
        namespaceDelegation: MusubiNamespaceDelegationV1?,
        expectedPolicyRevision: UInt64,
        expectedGovernanceRevision: UInt64?
    ) throws {
        guard expectedPolicyRevision > 0,
              expectedGovernanceRevision == nil || expectedGovernanceRevision! > 0 else {
            throw MusubiV1Error.invalidValue("Musubi publication revision is invalid.")
        }
        let namespaceSegments = namespace.value.split(
            separator: ".", omittingEmptySubsequences: false
        )
        switch publication.manifest.release.package.scope {
        case .dataspaceRoot:
            guard namespaceSegments.count == 1 else {
                throw MusubiV1Error.invalidValue(
                    "Musubi publication namespace and package scope disagree."
                )
            }
        case .domain(let domain):
            guard namespaceSegments.count == 2, namespaceSegments[0] == domain else {
                throw MusubiV1Error.invalidValue(
                    "Musubi publication namespace and package scope disagree."
                )
            }
        }
        let lockPayload = try MusubiInstructionNoritoV1.verificationLock(
            publication.resolution.lock
        )
        let lockDigest = try MusubiInstructionNoritoV1.domainHash(
            domain: "iroha.musubi.verification-lock.v1",
            payload: lockPayload
        )
        guard publication.manifest.verificationLockDigest.bytes == [UInt8](lockDigest) else {
            throw MusubiV1Error.invalidValue(
                "Musubi publication verification-lock digest does not match the exact graph."
            )
        }
        if let namespaceDelegation {
            try musubiValidateControllerApprovalsV1(
                namespaceDelegation.approvals,
                account: namespaceDelegation.payload.owner,
                field: "namespace owner"
            )
        }
        self.namespace = namespace
        self.publication = publication
        self.namespaceDelegation = namespaceDelegation
        self.expectedPolicyRevision = expectedPolicyRevision
        self.expectedGovernanceRevision = expectedGovernanceRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.namespace(namespace))
        writer.writeField(try MusubiInstructionNoritoV1.publication(publication))
        writer.writeField(
            try CompactNorito.encodeOption(
                namespaceDelegation,
                encode: MusubiInstructionNoritoV1.namespaceDelegation
            )
        )
        writer.writeField(CompactNorito.encodeUInt64(expectedPolicyRevision))
        writer.writeField(
            try CompactNorito.encodeOption(
                expectedGovernanceRevision,
                encode: CompactNorito.encodeUInt64
            )
        )
        return writer.data
    }
}

/// Replace one package's mutable metadata projection.
public struct SetMusubiPackageMetadataV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.package_metadata.set"
    public static let schemaName =
        "iroha_data_model::isi::musubi::SetMusubiPackageMetadataV1"

    public let package: MusubiPackageIdV1
    public let metadata: MusubiReleaseMetadataV1
    public let expectedMetadataRevision: UInt64

    public init(
        package: MusubiPackageIdV1,
        metadata: MusubiReleaseMetadataV1,
        expectedMetadataRevision: UInt64
    ) throws {
        guard expectedMetadataRevision > 0 else {
            throw MusubiV1Error.invalidValue(
                "Musubi package metadata revision must be non-zero."
            )
        }
        self.package = package
        self.metadata = metadata
        self.expectedMetadataRevision = expectedMetadataRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.packageID(package))
        writer.writeField(MusubiInstructionNoritoV1.releaseMetadata(metadata))
        writer.writeField(CompactNorito.encodeUInt64(expectedMetadataRevision))
        return writer.data
    }
}

/// Replace registry admission and future alias pricing under Parliament authority.
public struct SetMusubiRegistryPolicyV1: MusubiInstructionV1 {
    public static let stableWireID = "iroha.musubi.v1.parliament.registry_policy.set"
    public static let schemaName =
        "iroha_data_model::isi::musubi::SetMusubiRegistryPolicyV1"

    public let decision: MusubiGovernanceDecisionV1
    public let policy: MusubiRegistryPolicyV1
    public let expectedPolicyRevision: UInt64

    public init(
        decision: MusubiGovernanceDecisionV1,
        policy: MusubiRegistryPolicyV1,
        expectedPolicyRevision: UInt64
    ) throws {
        guard expectedPolicyRevision > 0,
              expectedPolicyRevision < UInt64.max,
              policy.revision == expectedPolicyRevision + 1 else {
            throw MusubiV1Error.invalidValue(
                "Musubi replacement policy must be the exact revision successor."
            )
        }
        let action = MusubiInstructionNoritoV1.setRegistryPolicyAction(
            policy: policy,
            expectedRevision: expectedPolicyRevision
        )
        let expectedDigest = try MusubiInstructionNoritoV1.domainHash(
            domain: "iroha.musubi.parliament-action.v1",
            payload: action
        )
        guard decision.actionDigest.bytes == [UInt8](expectedDigest) else {
            throw MusubiV1Error.invalidValue(
                "Musubi governance decision does not bind the registry-policy action."
            )
        }
        self.decision = decision
        self.policy = policy
        self.expectedPolicyRevision = expectedPolicyRevision
    }

    public var wireID: String { Self.stableWireID }
    public var concreteSchemaName: String { Self.schemaName }

    public func barePayload() throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(MusubiInstructionNoritoV1.governanceDecision(decision))
        writer.writeField(MusubiInstructionNoritoV1.registryPolicy(policy))
        writer.writeField(CompactNorito.encodeUInt64(expectedPolicyRevision))
        return writer.data
    }
}

private enum MusubiInstructionNoritoV1 {
    static let instructionBoxSchema = "(alloc::string::String, alloc::vec::Vec<u8>)"

    static func packageInvitationMutation(
        package: MusubiPackageIdV1,
        inviteID: MusubiDigest32V1,
        expectedGovernanceRevision: UInt64
    ) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(packageID(package))
        writer.writeField(digest32(inviteID))
        writer.writeField(CompactNorito.encodeUInt64(expectedGovernanceRevision))
        return writer.data
    }

    static func packageID(_ value: MusubiPackageIdV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(newtype(CompactNorito.encodeUInt64(value.homeDataspace)))
        writer.writeField(packageScope(value.scope))
        writer.writeField(packageName(value.name))
        return writer.data
    }

    static func namespaceBinding(_ value: MusubiNamespaceBindingV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(newtype(CompactNorito.encodeString(value.namespace.value)))
        writer.writeField(newtype(CompactNorito.encodeUInt64(value.homeDataspace)))
        writer.writeField(packageScope(value.scope))
        writer.writeField(CompactNorito.encodeUInt64(value.generation))
        return writer.data
    }

    static func packageScope(_ value: MusubiPackageScopeV1) -> Data {
        var writer = CompactNoritoWriter()
        switch value {
        case .dataspaceRoot:
            writer.writeUInt32LE(0)
        case .domain(let domain):
            writer.writeUInt32LE(1)
            writer.writeField(CompactNorito.encodeString(domain))
        }
        return writer.data
    }

    static func packageName(_ value: MusubiPackageNameV1) -> Data {
        newtype(CompactNorito.encodeString(value.value))
    }

    static func aliasName(_ value: MusubiAliasNameV1) -> Data {
        newtype(CompactNorito.encodeString(value.value))
    }

    static func reason(_ value: MusubiReasonV1) -> Data {
        newtype(CompactNorito.encodeString(value.value))
    }

    static func governanceDecision(_ value: MusubiGovernanceDecisionV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(Data(value.decisionID))
        writer.writeField(digest32(value.actionDigest))
        writer.writeField(CompactNorito.encodeUInt64(value.enactedAtHeight))
        writer.writeField(CompactNorito.encodeUInt64(value.executeAfterHeight))
        return writer.data
    }

    static func accountIDs(_ payloads: [Data]) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(payloads.count))
        for payload in payloads {
            writer.writeField(payload)
        }
        return writer.data
    }

    static func packageRole(_ value: MusubiPackageRoleV1) -> Data {
        var writer = CompactNoritoWriter()
        switch value {
        case .owner:
            writer.writeUInt32LE(0)
        case .maintainer(let permissions):
            writer.writeUInt32LE(1)
            writer.writeField(maintainerPermissions(permissions))
        }
        return writer.data
    }

    static func maintainerPermissions(_ value: MusubiMaintainerPermissionsV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeBool(value.publish))
        writer.writeField(CompactNorito.encodeBool(value.yank))
        writer.writeField(CompactNorito.encodeBool(value.metadata))
        writer.writeField(CompactNorito.encodeBool(value.archiveLocations))
        return writer.data
    }

    static func releaseID(_ value: MusubiReleaseIdV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(packageID(value.package))
        writer.writeField(version(value.version))
        return writer.data
    }

    static func version(_ value: MusubiVersionV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt64(value.major))
        writer.writeField(CompactNorito.encodeUInt64(value.minor))
        writer.writeField(CompactNorito.encodeUInt64(value.patch))
        writer.writeField(prereleaseIdentifiers(value.prerelease))
        return writer.data
    }

    static func prereleaseIdentifiers(_ values: [MusubiPrereleaseIdentifierV1]) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(values.count))
        for value in values {
            writer.writeField(prereleaseIdentifier(value))
        }
        return writer.data
    }

    static func prereleaseIdentifier(_ value: MusubiPrereleaseIdentifierV1) -> Data {
        var writer = CompactNoritoWriter()
        switch value {
        case .numeric(let numeric):
            writer.writeUInt32LE(0)
            writer.writeField(CompactNorito.encodeUInt64(numeric))
        case .alphaNumeric(let text):
            writer.writeUInt32LE(1)
            writer.writeField(CompactNorito.encodeString(text))
        }
        return writer.data
    }

    static func namespace(_ value: MusubiNamespaceV1) -> Data {
        newtype(CompactNorito.encodeString(value.value))
    }

    static func archiveCommitment(_ value: MusubiArchiveCommitmentV1) -> Data {
        var writer = CompactNoritoWriter()
        // Rust's custom ManifestRootCid serializer delegates to `[u8; 36]`, whose
        // compact layout frames every element. The optimized 32-byte fixed-array
        // shortcut used by digest fields must not be applied here.
        writer.writeField(fixedArray(value.rootCid))
        writer.writeField(chunkerProfile(value.chunker))
        writer.writeField(digest32(value.chunkPlanDigest))
        writer.writeField(digest32(value.porRoot))
        writer.writeField(CompactNorito.encodeUInt64(value.contentLength))
        writer.writeField(digest32(value.carDigest))
        writer.writeField(CompactNorito.encodeUInt64(value.carSize))
        writer.writeField(digest32(value.bundleDigest))
        writer.writeField(digest32(value.sourceTreeDigest))
        writer.writeField(digest32(value.descriptorDigest))
        writer.writeField(CompactNorito.encodeUInt32(value.fileCount))
        writer.writeField(CompactNorito.encodeUInt32(value.chunkCount))
        return writer.data
    }

    static func chunkerProfile(_ value: MusubiChunkerProfileHandleV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt32(value.profileId))
        writer.writeField(CompactNorito.encodeString(value.namespace))
        writer.writeField(CompactNorito.encodeString(value.name))
        writer.writeField(CompactNorito.encodeString(value.semver))
        writer.writeField(CompactNorito.encodeUInt64(value.multihashCode))
        return writer.data
    }

    static func seedIngressReceipt(_ value: MusubiSeedIngressReceiptV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try seedIngressPayload(value.payload))
        writer.writeField(
            try CompactNorito.encodeVec(value.approvals) { approval in
                try controllerApproval(
                    MusubiControllerApprovalV1(
                        publicKey: approval.publicKey,
                        signature: approval.signature
                    )
                )
            }
        )
        return writer.data
    }

    static func seedIngressPayload(_ value: MusubiSeedIngressReceiptPayloadV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt8(value.version))
        writer.writeField(try seedIngressBinding(value.binding))
        writer.writeField(CompactNorito.encodeUInt64(value.issuedAtMs))
        writer.writeField(CompactNorito.encodeUInt64(value.expiresAtMs))
        return writer.data
    }

    static func seedIngressBinding(_ value: MusubiSeedIngressReceiptBindingV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(value.networkId.bytes)
        writer.writeField(try CanonicalNorito.encodeCompactAccountId(value.publisher))
        writer.writeField(try CanonicalNorito.encodeCompactAccountId(value.ingressBroker))
        writer.writeField(try providerID(value.seedProvider))
        writer.writeField(digest32(value.semanticReleaseManifestDigest))
        writer.writeField(digest32(value.archiveId))
        writer.writeField(digest32(value.carBodyDigest))
        writer.writeField(CompactNorito.encodeUInt64(value.carBodyLength))
        writer.writeField(Data(value.nonce))
        return writer.data
    }

    static func providerAttestation(
        _ value: MusubiProviderBundleVerificationAttestationV1
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try providerPayload(value.payload))
        writer.writeField(try CompactNorito.encodeVec(value.approvals, encode: controllerApproval))
        return writer.data
    }

    static func providerPayload(
        _ value: MusubiProviderBundleVerificationPayloadV1
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt8(value.version))
        writer.writeField(try providerBinding(value.binding))
        return writer.data
    }

    static func providerBinding(
        _ value: MusubiProviderBundleVerificationBindingV1
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(value.networkId.bytes)
        writer.writeField(digest32(value.providerID))
        writer.writeField(try CanonicalNorito.encodeCompactAccountId(value.completedBy))
        writer.writeField(try completionAuthority(value.completionAuthority))
        writer.writeField(digest32(value.replicationOrder))
        writer.writeField(CompactNorito.encodeUInt64(value.assignmentRevision))
        writer.writeField(CompactNorito.encodeUInt64(value.completionEpoch))
        writer.writeField(finalizedAnchor(value.finalizedAnchor))
        writer.writeField(digest32(value.archiveID))
        writer.writeField(digest32(value.bundleDigest))
        writer.writeField(digest32(value.descriptorDigest))
        writer.writeField(digest32(value.semanticReleaseManifestDigest))
        writer.writeField(digest32(value.verificationLockDigest))
        writer.writeField(digest32(value.sourceTreeDigest))
        return writer.data
    }

    static func completionAuthority(
        _ value: MusubiProviderIngestCompletionAuthorityV1
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try CanonicalNorito.encodeCompactAccountId(value.providerOwner))
        writer.writeField(completionSignerPolicy(value.signerPolicy))
        return writer.data
    }

    static func completionSignerPolicy(
        _ value: MusubiProviderIngestCompletionSignerPolicyV1
    ) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(Data(value.policyID))
        writer.writeField(CompactNorito.encodeUInt64(value.revision))
        writer.writeField(optional(value.predecessorDigest, encode: fixedArray))
        writer.writeField(Data(value.policyDigest))
        return writer.data
    }

    static func finalizedAnchor(_ value: MusubiProviderIngestFinalizedAnchorV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt64(value.height))
        writer.writeField(Data(value.blockHash))
        return writer.data
    }

    static func controllerApproval(_ value: MusubiControllerApprovalV1) throws -> Data {
        var publicKey = Data([value.algorithm.noritoDiscriminant])
        publicKey.append(contentsOf: value.publicKeyPayload)
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeConstVec(publicKey))
        writer.writeField(CompactNorito.encodeConstVec(Data(value.signaturePayload)))
        return writer.data
    }

    static func publication(_ value: MusubiPublicationV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try releaseManifest(value.manifest))
        writer.writeField(try resolutionProof(value.resolution))
        return writer.data
    }

    static func releaseManifest(_ value: MusubiReleaseManifestV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(releaseID(value.release))
        writer.writeField(unitEnum(value.edition.rawValue))
        writer.writeField(abiBinding(value.abi))
        writer.writeField(try CompactNorito.encodeVec(value.dependencies, encode: dependencyReq))
        writer.writeField(try CompactNorito.encodeVec(value.exports, encode: name))
        writer.writeField(digest32(value.interfaceDigest))
        writer.writeField(releaseMetadata(value.metadata))
        writer.writeField(digest32(value.archiveID))
        writer.writeField(digest32(value.verificationLockDigest))
        return writer.data
    }

    static func abiBinding(_ value: MusubiAbiBindingV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt16(value.abiVersion))
        writer.writeField(Data(value.abiHash))
        return writer.data
    }

    static func dependencyReq(_ value: MusubiDependencyReqV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(name(value.alias))
        writer.writeField(packageID(value.package))
        writer.writeField(try versionRequirement(value.requirement))
        return writer.data
    }

    static func versionRequirement(_ value: MusubiVersionReqV1) throws -> Data {
        var writer = CompactNoritoWriter()
        switch value {
        case .any:
            writer.writeUInt32LE(0)
        case .caret(let versionValue):
            writer.writeUInt32LE(1)
            writer.writeField(version(versionValue))
        case .tilde(let versionValue):
            writer.writeUInt32LE(2)
            writer.writeField(version(versionValue))
        case .majorWildcard(let major):
            writer.writeUInt32LE(3)
            writer.writeField(CompactNorito.encodeUInt64(major))
        case .minorWildcard(let major, let minor):
            writer.writeUInt32LE(4)
            var wildcard = CompactNoritoWriter()
            wildcard.writeField(CompactNorito.encodeUInt64(major))
            wildcard.writeField(CompactNorito.encodeUInt64(minor))
            writer.writeField(wildcard.data)
        case .exact(let versionValue):
            writer.writeUInt32LE(5)
            writer.writeField(version(versionValue))
        case .comparators(let comparators):
            writer.writeUInt32LE(6)
            writer.writeField(try CompactNorito.encodeVec(comparators, encode: versionComparator))
        }
        return writer.data
    }

    static func versionComparator(_ value: MusubiVersionComparatorV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(unitEnum(UInt32(value.op.rawValue)))
        writer.writeField(version(value.version))
        return writer.data
    }

    static func releaseMetadata(_ value: MusubiReleaseMetadataV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(optional(value.description, encode: boundedText))
        writer.writeField(optional(value.readme, encode: boundedText))
        writer.writeField(optional(value.license, encode: boundedText))
        writer.writeField(optional(value.repository, encode: boundedText))
        writer.writeField(vector(value.keywords, encode: boundedText))
        return writer.data
    }

    static func resolutionProof(_ value: MusubiResolutionProofV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(registrySnapshot(value.snapshot))
        writer.writeField(try verificationLock(value.lock))
        return writer.data
    }

    static func registrySnapshot(_ value: MusubiRegistrySnapshotV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt64(value.finalizedHeight))
        writer.writeField(Data(value.finalizedBlockHash))
        writer.writeField(CompactNorito.encodeUInt64(value.indexRevision))
        return writer.data
    }

    static func verificationLock(_ value: MusubiVerificationLockV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeString(value.schema))
        writer.writeField(CompactNorito.encodeUInt8(value.version))
        writer.writeField(releaseID(value.root))
        writer.writeField(
            try CompactNorito.encodeVec(value.rootDependencies, encode: exactDependency)
        )
        writer.writeField(try CompactNorito.encodeVec(value.nodes, encode: verificationNode))
        return writer.data
    }

    static func exactDependency(_ value: MusubiExactDependencyEdgeV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(name(value.alias))
        writer.writeField(unitEnum(value.kind.rawValue))
        writer.writeField(packageID(value.package))
        writer.writeField(try versionRequirement(value.requirement))
        writer.writeField(releaseID(value.selected))
        return writer.data
    }

    static func verificationNode(_ value: MusubiVerificationNodeV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(releaseID(value.release))
        writer.writeField(digest32(value.releaseDigest))
        writer.writeField(digest32(value.archiveID))
        writer.writeField(digest32(value.sourceDigest))
        writer.writeField(digest32(value.interfaceDigest))
        writer.writeField(abiBinding(value.abi))
        writer.writeField(try CompactNorito.encodeVec(value.dependencies, encode: exactDependency))
        return writer.data
    }

    static func namespaceDelegation(_ value: MusubiNamespaceDelegationV1) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(try namespaceDelegationPayload(value.payload))
        writer.writeField(try CompactNorito.encodeVec(value.approvals, encode: controllerApproval))
        return writer.data
    }

    static func namespaceDelegationPayload(
        _ value: MusubiNamespaceDelegationPayloadV1
    ) throws -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt8(value.version))
        writer.writeField(digest32(value.namespaceBinding))
        writer.writeField(CompactNorito.encodeUInt64(value.ownerGeneration))
        writer.writeField(try CanonicalNorito.encodeCompactAccountId(value.owner))
        writer.writeField(try CanonicalNorito.encodeCompactAccountId(value.delegate))
        writer.writeField(CompactNorito.encodeUInt64(value.expiresAtHeight))
        return writer.data
    }

    static func registryPolicy(_ value: MusubiRegistryPolicyV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt8(value.version))
        writer.writeField(CompactNorito.encodeUInt64(value.revision))
        writer.writeField(unitEnum(value.mode.rawValue))
        writer.writeField(vector(value.allowlistedDataspaces) {
            newtype(CompactNorito.encodeUInt64($0))
        })
        writer.writeField(aliasPricing(value.aliasPricing))
        return writer.data
    }

    static func aliasPricing(_ value: MusubiAliasPricingPolicyV1) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(CompactNorito.encodeUInt64(value.revision))
        writer.writeField(CompactNorito.encodeUInt64(value.length1Xor))
        writer.writeField(CompactNorito.encodeUInt64(value.length2Xor))
        writer.writeField(CompactNorito.encodeUInt64(value.length3Xor))
        writer.writeField(CompactNorito.encodeUInt64(value.length4Xor))
        writer.writeField(CompactNorito.encodeUInt64(value.length5To32Xor))
        return writer.data
    }

    static func setRegistryPolicyAction(
        policy: MusubiRegistryPolicyV1,
        expectedRevision: UInt64
    ) -> Data {
        var replacement = CompactNoritoWriter()
        replacement.writeField(registryPolicy(policy))
        replacement.writeField(CompactNorito.encodeUInt64(expectedRevision))
        var action = CompactNoritoWriter()
        action.writeUInt32LE(3)
        action.writeField(replacement.data)
        return action.data
    }

    static func providerID(_ canonicalHex: String) throws -> Data {
        guard canonicalHex == canonicalHex.uppercased(),
              let bytes = Data(hexString: canonicalHex), bytes.count == 32 else {
            throw MusubiV1Error.invalidValue("Musubi provider ID is not canonical hex.")
        }
        return newtype(bytes)
    }

    static func name(_ value: String) -> Data {
        CompactNorito.encodeString(value)
    }

    static func boundedText(_ value: String) -> Data {
        newtype(CompactNorito.encodeString(value))
    }

    static func unitEnum(_ discriminant: UInt32) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt32LE(discriminant)
        return writer.data
    }

    static func fixedArray(_ bytes: [UInt8]) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeByteFields(bytes)
        return writer.data
    }

    static func optional<T>(_ value: T?, encode: (T) -> Data) -> Data {
        var writer = CompactNoritoWriter()
        guard let value else {
            writer.writeUInt8(0)
            return writer.data
        }
        writer.writeUInt8(1)
        writer.writeField(encode(value))
        return writer.data
    }

    static func vector<T>(_ values: [T], encode: (T) -> Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(values.count))
        for value in values { writer.writeField(encode(value)) }
        return writer.data
    }

    static func domainHash(domain: String, payload: Data) throws -> Data {
        var preimage = Data()
        var domainLength = UInt64(domain.utf8.count).littleEndian
        withUnsafeBytes(of: &domainLength) { preimage.append(contentsOf: $0) }
        preimage.append(contentsOf: domain.utf8)
        var payloadLength = UInt64(payload.count).littleEndian
        withUnsafeBytes(of: &payloadLength) { preimage.append(contentsOf: $0) }
        preimage.append(payload)
        guard let digest = MusubiBlake3V1.hash(preimage), digest.count == 32 else {
            throw MusubiV1Error.invalidValue("Musubi BLAKE3 hashing failed.")
        }
        return digest
    }

    static func digest32(_ value: MusubiDigest32V1) -> Data {
        newtype(Data(value.bytes))
    }

    static func digest32(_ value: MusubiProviderBundleAttestationSetDigestV1) -> Data {
        newtype(Data(value.bytes))
    }

    static func newtype(_ payload: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeField(payload)
        return writer.data
    }

}

func musubiProviderBundleAttestationDigestV1(
    _ attestation: MusubiProviderBundleVerificationAttestationV1
) throws -> MusubiProviderBundleAttestationDigestV1 {
    let canonical = try MusubiInstructionNoritoV1.providerAttestation(attestation)
    let digest = try MusubiInstructionNoritoV1.domainHash(
        domain: "iroha.musubi.provider-bundle-attestation.digest.v1",
        payload: canonical
    )
    return try MusubiProviderBundleAttestationDigestV1(bytes: Array(digest))
}

func musubiReleaseManifestDigestV1(
    _ manifest: MusubiReleaseManifestV1
) throws -> MusubiDigest32V1 {
    let canonical = try MusubiInstructionNoritoV1.releaseManifest(manifest)
    let digest = try MusubiInstructionNoritoV1.domainHash(
        domain: "iroha.musubi.release-digest.v1",
        payload: canonical
    )
    return try MusubiDigest32V1(bytes: Array(digest))
}

private func musubiValidateControllerApprovalsV1(
    _ approvals: [MusubiControllerApprovalV1],
    account: String,
    field: String
) throws {
    guard !approvals.isEmpty,
          zip(approvals, approvals.dropFirst()).allSatisfy({ $0.0 < $0.1 }) else {
        throw MusubiV1Error.invalidValue(
            "Musubi \(field) approvals must be sorted and distinct."
        )
    }
    let prefix = try AccountAddress.inspectI105NetworkPrefix(account).chainDiscriminant
    let address = try AccountAddress.fromI105(account, expectedPrefix: prefix)
    if let single = address.singleControllerInfo() {
        guard approvals.count == 1,
              approvals[0].algorithm == single.algorithm,
              approvals[0].publicKeyPayload == [UInt8](single.publicKey) else {
            throw MusubiV1Error.invalidValue(
                "Musubi \(field) approval is not the account controller key."
            )
        }
        return
    }
    guard let policy = try address.multisigPolicyInfo(), policy.version == 1,
          policy.threshold > 0 else {
        throw MusubiV1Error.invalidValue("Musubi \(field) has no valid controller policy.")
    }
    var approvedWeight: UInt32 = 0
    for approval in approvals {
        guard let member = try policy.members.first(where: { member in
            let body = member.publicKeyHex.hasPrefix("0x")
                ? String(member.publicKeyHex.dropFirst(2))
                : member.publicKeyHex
            guard let key = Data(hexString: body) else { return false }
            return try musubiAlgorithmOrderV1(member.algorithm)
                == approval.algorithm.noritoDiscriminant
                && [UInt8](key) == approval.publicKeyPayload
        }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi \(field) approval is not a multisig controller key."
            )
        }
        approvedWeight = approvedWeight.addingReportingOverflow(UInt32(member.weight)).partialValue
    }
    guard approvedWeight >= UInt32(policy.threshold) else {
        throw MusubiV1Error.invalidValue(
            "Musubi \(field) approvals do not meet the controller threshold."
        )
    }
}

/// Small dependency-free BLAKE3 implementation for exact Musubi domain hashes.
enum MusubiBlake3V1 {
    private static let iv: [UInt32] = [
        0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
        0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
    ]
    private static let permutation = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8]
    private static let chunkStart: UInt32 = 1
    private static let chunkEnd: UInt32 = 2
    private static let parent: UInt32 = 4
    private static let root: UInt32 = 8
    private static let blockLength = 64
    private static let chunkLength = 1_024

    private struct Output {
        let inputCV: [UInt32]
        let blockWords: [UInt32]
        let counter: UInt64
        let blockLength: UInt32
        let flags: UInt32

        func chainingValue() -> [UInt32] {
            Array(MusubiBlake3V1.compress(
                cv: inputCV,
                blockWords: blockWords,
                counter: counter,
                blockLength: blockLength,
                flags: flags
            ).prefix(8))
        }

        func rootBytes() -> Data {
            let words = MusubiBlake3V1.compress(
                cv: inputCV,
                blockWords: blockWords,
                counter: 0,
                blockLength: blockLength,
                flags: flags | MusubiBlake3V1.root
            )
            var data = Data(capacity: 32)
            for word in words.prefix(8) {
                var littleEndian = word.littleEndian
                withUnsafeBytes(of: &littleEndian) { data.append(contentsOf: $0) }
            }
            return data
        }
    }

    static func hash(_ data: Data) -> Data? {
        let bytes = [UInt8](data)
        let chunkCount = max(1, (bytes.count + chunkLength - 1) / chunkLength)
        var cvStack: [[UInt32]] = []
        if chunkCount > 1 {
            for chunkIndex in 0..<(chunkCount - 1) {
                let start = chunkIndex * chunkLength
                let chunk = Array(bytes[start..<(start + chunkLength)])
                var cv = chunkOutput(chunk, counter: UInt64(chunkIndex)).chainingValue()
                var completedChunks = chunkIndex + 1
                while completedChunks & 1 == 0 {
                    guard let left = cvStack.popLast() else { return nil }
                    cv = parentOutput(left: left, right: cv).chainingValue()
                    completedChunks >>= 1
                }
                cvStack.append(cv)
            }
        }
        let finalStart = (chunkCount - 1) * chunkLength
        let finalChunk = finalStart < bytes.count ? Array(bytes[finalStart...]) : []
        var output = chunkOutput(finalChunk, counter: UInt64(chunkCount - 1))
        while let left = cvStack.popLast() {
            output = parentOutput(left: left, right: output.chainingValue())
        }
        return output.rootBytes()
    }

    private static func chunkOutput(_ bytes: [UInt8], counter: UInt64) -> Output {
        let blockCount = max(1, (bytes.count + blockLength - 1) / blockLength)
        var cv = iv
        if blockCount > 1 {
            for blockIndex in 0..<(blockCount - 1) {
                let start = blockIndex * blockLength
                let block = Array(bytes[start..<(start + blockLength)])
                let flags = blockIndex == 0 ? chunkStart : 0
                cv = Array(compress(
                    cv: cv,
                    blockWords: words(block),
                    counter: counter,
                    blockLength: UInt32(blockLength),
                    flags: flags
                ).prefix(8))
            }
        }
        let finalStart = (blockCount - 1) * blockLength
        let final = finalStart < bytes.count ? Array(bytes[finalStart...]) : []
        var flags = chunkEnd
        if blockCount == 1 { flags |= chunkStart }
        return Output(
            inputCV: cv,
            blockWords: words(final),
            counter: counter,
            blockLength: UInt32(final.count),
            flags: flags
        )
    }

    private static func parentOutput(left: [UInt32], right: [UInt32]) -> Output {
        Output(
            inputCV: iv,
            blockWords: left + right,
            counter: 0,
            blockLength: UInt32(blockLength),
            flags: parent
        )
    }

    private static func words(_ bytes: [UInt8]) -> [UInt32] {
        var padded = bytes
        padded.append(contentsOf: repeatElement(0, count: max(0, blockLength - padded.count)))
        return (0..<16).map { index in
            let start = index * 4
            return UInt32(padded[start])
                | (UInt32(padded[start + 1]) << 8)
                | (UInt32(padded[start + 2]) << 16)
                | (UInt32(padded[start + 3]) << 24)
        }
    }

    private static func compress(
        cv: [UInt32],
        blockWords: [UInt32],
        counter: UInt64,
        blockLength: UInt32,
        flags: UInt32
    ) -> [UInt32] {
        var state = cv + Array(iv.prefix(4)) + [
            UInt32(truncatingIfNeeded: counter),
            UInt32(truncatingIfNeeded: counter >> 32),
            blockLength,
            flags,
        ]
        var message = blockWords
        for _ in 0..<7 {
            round(&state, message)
            message = permutation.map { message[$0] }
        }
        var output = [UInt32](repeating: 0, count: 16)
        for index in 0..<8 {
            output[index] = state[index] ^ state[index + 8]
            output[index + 8] = state[index + 8] ^ cv[index]
        }
        return output
    }

    private static func round(_ state: inout [UInt32], _ message: [UInt32]) {
        mix(&state, 0, 4, 8, 12, message[0], message[1])
        mix(&state, 1, 5, 9, 13, message[2], message[3])
        mix(&state, 2, 6, 10, 14, message[4], message[5])
        mix(&state, 3, 7, 11, 15, message[6], message[7])
        mix(&state, 0, 5, 10, 15, message[8], message[9])
        mix(&state, 1, 6, 11, 12, message[10], message[11])
        mix(&state, 2, 7, 8, 13, message[12], message[13])
        mix(&state, 3, 4, 9, 14, message[14], message[15])
    }

    private static func mix(
        _ state: inout [UInt32],
        _ a: Int,
        _ b: Int,
        _ c: Int,
        _ d: Int,
        _ x: UInt32,
        _ y: UInt32
    ) {
        state[a] = state[a] &+ state[b] &+ x
        state[d] = rotateRight(state[d] ^ state[a], by: 16)
        state[c] = state[c] &+ state[d]
        state[b] = rotateRight(state[b] ^ state[c], by: 12)
        state[a] = state[a] &+ state[b] &+ y
        state[d] = rotateRight(state[d] ^ state[a], by: 8)
        state[c] = state[c] &+ state[d]
        state[b] = rotateRight(state[b] ^ state[c], by: 7)
    }

    private static func rotateRight(_ value: UInt32, by amount: UInt32) -> UInt32 {
        (value >> amount) | (value << (32 - amount))
    }
}

private struct MusubiPublicKeyOrderKeyV1 {
    let algorithm: UInt8
    let payload: [UInt8]
}

private struct MusubiMultisigMemberOrderKeyV1 {
    let publicKey: MusubiPublicKeyOrderKeyV1
    let weight: UInt16
    let canonicalSortKey: [UInt8]
}

private enum MusubiAccountOrderKeyV1 {
    case single(MusubiPublicKeyOrderKeyV1)
    case multisig(version: UInt8, threshold: UInt16, members: [MusubiMultisigMemberOrderKeyV1])
}

private struct MusubiNormalizedAccountV1 {
    let payload: Data
    let orderKey: MusubiAccountOrderKeyV1
}

private func musubiNormalizedAccountV1(_ canonicalOwner: String) throws -> MusubiNormalizedAccountV1 {
    let prefix = try AccountAddress.inspectI105NetworkPrefix(canonicalOwner).chainDiscriminant
    let address = try AccountAddress.fromI105(canonicalOwner, expectedPrefix: prefix)
    if let single = address.singleControllerInfo() {
        return MusubiNormalizedAccountV1(
            payload: try CanonicalNorito.encodeCompactAccountId(canonicalOwner),
            orderKey: .single(MusubiPublicKeyOrderKeyV1(
                algorithm: single.algorithm.noritoDiscriminant,
                payload: [UInt8](single.publicKey)
            ))
        )
    }
    guard let policy = try address.multisigPolicyInfo() else {
        throw MusubiV1Error.invalidValue(
            "Musubi package recovery owner has no canonical account controller."
        )
    }
    guard policy.version == 1,
          policy.threshold > 0,
          !policy.members.isEmpty else {
        throw MusubiV1Error.invalidValue(
            "Musubi package recovery owner contains an invalid multisig policy."
        )
    }
    var totalWeight: UInt32 = 0
    var members = try policy.members.map { member -> MusubiMultisigMemberOrderKeyV1 in
        let body = member.publicKeyHex.hasPrefix("0x")
            ? String(member.publicKeyHex.dropFirst(2))
            : member.publicKeyHex
        guard let publicKey = Data(hexString: body) else {
            throw MusubiV1Error.invalidValue(
                "Musubi package recovery owner contains an invalid multisig public key."
            )
        }
        guard member.weight > 0 else {
            throw MusubiV1Error.invalidValue(
                "Musubi package recovery owner contains an invalid multisig policy."
            )
        }
        totalWeight += UInt32(member.weight)
        let algorithm = try musubiAlgorithmOrderV1(member.algorithm)
        let publicKeyOrder = MusubiPublicKeyOrderKeyV1(
            algorithm: algorithm,
            payload: [UInt8](publicKey)
        )
        return MusubiMultisigMemberOrderKeyV1(
            publicKey: publicKeyOrder,
            weight: member.weight,
            canonicalSortKey: Array(try musubiAlgorithmStaticNameV1(member.algorithm).utf8)
                + [0]
                + publicKeyOrder.payload
        )
    }
    guard totalWeight >= UInt32(policy.threshold) else {
        throw MusubiV1Error.invalidValue(
            "Musubi package recovery owner contains an invalid multisig policy."
        )
    }
    members.sort {
        musubiCompareUnsignedBytesV1($0.canonicalSortKey, $1.canonicalSortKey) < 0
    }
    for index in 1..<members.count {
        guard members[index - 1].canonicalSortKey != members[index].canonicalSortKey else {
            throw MusubiV1Error.invalidValue(
                "Musubi package recovery owner contains duplicate multisig members."
            )
        }
    }
    return MusubiNormalizedAccountV1(
        payload: musubiCompactMultisigAccountPayloadV1(
            version: policy.version,
            threshold: policy.threshold,
            members: members
        ),
        orderKey: .multisig(
            version: policy.version,
            threshold: policy.threshold,
            members: members
        )
    )
}

private func musubiAlgorithmOrderV1(_ algorithm: String) throws -> UInt8 {
    switch algorithm {
    case "ed25519": return 0
    case "secp256k1": return 1
    case "bls_normal": return 2
    case "bls_small": return 3
    case "mldsa", "ml-dsa": return 4
    case "gost3410-2012-256-paramset-a": return 5
    case "gost3410-2012-256-paramset-b": return 6
    case "gost3410-2012-256-paramset-c": return 7
    case "gost3410-2012-512-paramset-a": return 8
    case "gost3410-2012-512-paramset-b": return 9
    case "sm2": return 10
    default:
        throw MusubiV1Error.invalidValue(
            "Musubi package recovery owner uses an unsupported signing algorithm."
        )
    }
}

private func musubiAlgorithmStaticNameV1(_ algorithm: String) throws -> String {
    switch algorithm {
    case "ed25519": return "ed25519"
    case "secp256k1": return "secp256k1"
    case "bls_normal": return "bls_normal"
    case "bls_small": return "bls_small"
    case "mldsa", "ml-dsa": return "ml-dsa"
    case "gost3410-2012-256-paramset-a": return "gost3410-2012-256-paramset-a"
    case "gost3410-2012-256-paramset-b": return "gost3410-2012-256-paramset-b"
    case "gost3410-2012-256-paramset-c": return "gost3410-2012-256-paramset-c"
    case "gost3410-2012-512-paramset-a": return "gost3410-2012-512-paramset-a"
    case "gost3410-2012-512-paramset-b": return "gost3410-2012-512-paramset-b"
    case "sm2": return "sm2"
    default:
        throw MusubiV1Error.invalidValue(
            "Musubi package recovery owner uses an unsupported signing algorithm."
        )
    }
}

private func musubiCompactMultisigAccountPayloadV1(
    version: UInt8,
    threshold: UInt16,
    members: [MusubiMultisigMemberOrderKeyV1]
) -> Data {
    var policyWriter = CompactNoritoWriter()
    policyWriter.writeField(CompactNorito.encodeUInt8(version))
    policyWriter.writeField(CompactNorito.encodeUInt16(threshold))

    var membersWriter = CompactNoritoWriter()
    membersWriter.writeUInt64LE(UInt64(members.count))
    for member in members {
        var memberWriter = CompactNoritoWriter()
        memberWriter.writeField(musubiCompactPublicKeyPayloadV1(member.publicKey))
        memberWriter.writeField(CompactNorito.encodeUInt16(member.weight))
        membersWriter.writeField(memberWriter.data)
    }
    policyWriter.writeField(membersWriter.data)

    var accountWriter = CompactNoritoWriter()
    accountWriter.writeUInt32LE(1)
    accountWriter.writeField(policyWriter.data)
    return accountWriter.data
}

private func musubiCompactPublicKeyPayloadV1(_ publicKey: MusubiPublicKeyOrderKeyV1) -> Data {
    let bytes = [publicKey.algorithm] + publicKey.payload
    var writer = CompactNoritoWriter()
    writer.writeUInt64LE(UInt64(bytes.count))
    writer.writeByteFields(bytes)
    return writer.data
}

private func musubiCompareUnsignedBytesV1(_ left: [UInt8], _ right: [UInt8]) -> Int {
    for index in 0..<min(left.count, right.count) {
        if left[index] != right[index] {
            return left[index] < right[index] ? -1 : 1
        }
    }
    if left.count == right.count { return 0 }
    return left.count < right.count ? -1 : 1
}

private func musubiCompareAccountOrderKeysV1(
    _ left: MusubiAccountOrderKeyV1,
    _ right: MusubiAccountOrderKeyV1
) -> Int {
    switch (left, right) {
    case (.single(let leftKey), .single(let rightKey)):
        return musubiComparePublicKeyOrderKeysV1(leftKey, rightKey)
    case (.single, .multisig):
        return -1
    case (.multisig, .single):
        return 1
    case let (
        .multisig(leftVersion, leftThreshold, leftMembers),
        .multisig(rightVersion, rightThreshold, rightMembers)
    ):
        if leftVersion != rightVersion { return leftVersion < rightVersion ? -1 : 1 }
        if leftThreshold != rightThreshold { return leftThreshold < rightThreshold ? -1 : 1 }
        for index in 0..<min(leftMembers.count, rightMembers.count) {
            let keyComparison = musubiComparePublicKeyOrderKeysV1(
                leftMembers[index].publicKey,
                rightMembers[index].publicKey
            )
            if keyComparison != 0 { return keyComparison }
            if leftMembers[index].weight != rightMembers[index].weight {
                return leftMembers[index].weight < rightMembers[index].weight ? -1 : 1
            }
        }
        if leftMembers.count == rightMembers.count { return 0 }
        return leftMembers.count < rightMembers.count ? -1 : 1
    }
}

private func musubiComparePublicKeyOrderKeysV1(
    _ left: MusubiPublicKeyOrderKeyV1,
    _ right: MusubiPublicKeyOrderKeyV1
) -> Int {
    if left.algorithm != right.algorithm { return left.algorithm < right.algorithm ? -1 : 1 }
    for index in 0..<min(left.payload.count, right.payload.count) {
        if left.payload[index] != right.payload[index] {
            return left.payload[index] < right.payload[index] ? -1 : 1
        }
    }
    if left.payload.count == right.payload.count { return 0 }
    return left.payload.count < right.payload.count ? -1 : 1
}
