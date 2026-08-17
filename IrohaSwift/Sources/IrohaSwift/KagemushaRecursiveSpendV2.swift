import CryptoKit
import Foundation

#if canImport(DeviceCheck)
import DeviceCheck
#endif

#if canImport(Darwin)
import Darwin
#endif

public enum KagemushaRecursiveSpendError: Error, Equatable, LocalizedError {
    case invalidField(String)
    case invalidArchive(String)
    case nativeBridgeUnavailable
    case proofBackendUnavailable
    case proofWorkerBusy
    case finalityTrustUnavailable
    case hardwareAssertionUnavailable

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Invalid Kagemusha recursive spend field: \(field)."
        case let .invalidArchive(field):
            return "Invalid Kagemusha recursive spend Norito archive: \(field)."
        case .nativeBridgeUnavailable:
            return "The ABI-22 Kagemusha recursive spend bridge is unavailable."
        case .proofBackendUnavailable:
            return "Kagemusha recursive spend V4 is unavailable until the ABI-21 proof backend is promoted."
        case .proofWorkerBusy:
            return "Another Kagemusha proof operation is active; retry after it completes."
        case .finalityTrustUnavailable:
            return "Kagemusha top-up finality requires an authenticated ABI-21 release and matching validator roster."
        case .hardwareAssertionUnavailable:
            return "The requested physical hardware assertion service is unavailable on this device."
        }
    }
}

/// Process-wide, non-blocking owner for memory-heavy Kagemusha work.
///
/// The recursive lock lets a coordinator lease call the lower-level prover or
/// artifact session on the same thread without reacquiring memory capacity.
/// A competing thread fails before encoding or copying its request.
enum KagemushaRecursiveSpendWorkerPermit {
    private static let lock = NSRecursiveLock()

    static func withPermit<T>(_ body: () throws -> T) throws -> T {
        guard lock.try() else {
            throw KagemushaRecursiveSpendError.proofWorkerBusy
        }
        defer { lock.unlock() }
        return try body()
    }
}

/// Exact capability record returned by the explicitly versioned ABI-21/V4
/// bridge. Older capability archives cannot be reinterpreted as permission to
/// invoke this prover.
public struct KagemushaRecursiveSpendNativeCapabilitiesV4: Equatable, Sendable {
    public let bridgeABIVersion: UInt32
    public let artifactManifestSchema: String
    public let proofBackend: String
    public let transcriptProfile: String
    public let proofEnvelopeVersion: UInt16
    public let stepEqCircuitID: String
    public let stepEpCircuitID: String
    public let artifactRoles: [String]
    public let maxProofBytes: UInt32
    public let proofBackendAvailable: Bool
    public let missingGates: [String]

    public init(
        bridgeABIVersion: UInt32,
        artifactManifestSchema: String,
        proofBackend: String,
        transcriptProfile: String,
        proofEnvelopeVersion: UInt16,
        stepEqCircuitID: String,
        stepEpCircuitID: String,
        artifactRoles: [String],
        maxProofBytes: UInt32,
        proofBackendAvailable: Bool,
        missingGates: [String]
    ) throws {
        let gatesAreCanonical: Bool
        if proofBackendAvailable {
            gatesAreCanonical = missingGates.isEmpty
        } else {
            gatesAreCanonical = !missingGates.isEmpty
                && missingGates.count <= 64
                && zip(missingGates, missingGates.dropFirst()).allSatisfy {
                    $0.0 < $0.1
                }
                && missingGates.allSatisfy {
                    (try? KagemushaRecursiveSpend.requirePortableArtifactIdentifier(
                        $0,
                        field: "nativeCapabilitiesV4.missingGates"
                    )) != nil
                }
        }
        guard bridgeABIVersion == KagemushaRecursiveSpend.requiredNativeBridgeAbiVersion,
              artifactManifestSchema == KagemushaRecursiveSpend.artifactManifestSchemaV4,
              proofBackend == KagemushaRecursiveSpend.pastaCycleBackendV4,
              transcriptProfile == KagemushaRecursiveSpend.pastaCycleTranscriptV4,
              proofEnvelopeVersion
                == KagemushaRecursiveSpend.pastaCycleProofEnvelopeVersionV4,
              stepEqCircuitID == KagemushaRecursiveSpend.stepEqCircuitIDV4,
              stepEpCircuitID == KagemushaRecursiveSpend.stepEpCircuitIDV4,
              artifactRoles == KagemushaRecursiveSpend.artifactRolesV4,
              maxProofBytes > 0,
              maxProofBytes <= KagemushaRecursiveSpend.absoluteMaximumProofPairBytesV4,
              gatesAreCanonical else {
            throw KagemushaRecursiveSpendError.invalidField("nativeCapabilitiesV4")
        }
        self.bridgeABIVersion = bridgeABIVersion
        self.artifactManifestSchema = artifactManifestSchema
        self.proofBackend = proofBackend
        self.transcriptProfile = transcriptProfile
        self.proofEnvelopeVersion = proofEnvelopeVersion
        self.stepEqCircuitID = stepEqCircuitID
        self.stepEpCircuitID = stepEpCircuitID
        self.artifactRoles = artifactRoles
        self.maxProofBytes = maxProofBytes
        self.proofBackendAvailable = proofBackendAvailable
        self.missingGates = missingGates
    }
}

public enum KagemushaRecursiveSpend {
    /// Exact verifier-registry roles carried by the five readiness fields.
    /// A field is valid only for its matching role and circuit; roles are not
    /// interchangeable even though Torii uses one common record shape.
    public enum VerifierRole: CaseIterable, Sendable {
        case transfer
        case topUpShield
        case unshield
        case recursiveStepEq
        case recursiveStepEp

        public var registryBackend: String { "halo2/ipa" }

        public var registryName: String {
            switch self {
            case .transfer:
                return "confidential_transfer_v2_verifier_record"
            case .topUpShield:
                return "kagemusha_topup_shield_v2_verifier_record"
            case .unshield:
                return "confidential_unshield_v3_verifier_record"
            case .recursiveStepEq:
                return "kagemusha_recursive_step_eq_v4_verifier_record"
            case .recursiveStepEp:
                return "kagemusha_recursive_step_ep_v4_verifier_record"
            }
        }

        public var circuitID: String {
            switch self {
            case .transfer:
                return "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3"
            case .topUpShield:
                return KagemushaRecursiveSpend.topUpShieldCircuitID
            case .unshield:
                return "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4"
            case .recursiveStepEq:
                return KagemushaRecursiveSpend.stepEqCircuitIDV4
            case .recursiveStepEp:
                return KagemushaRecursiveSpend.stepEpCircuitIDV4
            }
        }
    }

    public static let requiredNativeBridgeAbiVersion: UInt32 = 22
    /// Mandatory sender-final peer-cash contract advertised by Torii readiness.
    public static let cashHandoffCapabilityV1 = "cash_handoff_v1"
    public static let authorizationPreparationVersionV2: UInt16 = 2
    public static let wireVersionV4: UInt16 = 4
    public static let localWitnessVersionV4: UInt16 = 4
    /// First-release maximum number of recursive parents consumed by one transition.
    public static let maximumInputsPerTransition = 2
    /// First-release peer-hop bound advertised by Torii readiness and
    /// enforced by every recursive-spend request codec.
    public static let maximumPeerHops: UInt32 = 8
    public static let artifactManifestSchemaV4 =
        "kagemusha.offline.recursive_spend.artifact_manifest.v4"
    public static let artifactManifestVersionV4: UInt16 = 4
    public static let pastaCycleBackendV4 = "halo2/ipa-pasta-cycle-compact-v5"
    public static let pastaCycleTranscriptV4 =
        "kagemusha-pasta-cycle-poseidon-compact-v5"
    public static let pastaCycleProofEnvelopeVersionV4: UInt16 = 5
    public static let stepEqCircuitIDV4 =
        "kagemusha-recursive-spend-step-eq-compact-layout-v5"
    public static let stepEpCircuitIDV4 =
        "kagemusha-recursive-spend-step-ep-compact-lineage-v5"

    /// Exact public-bundle verifier identifier selected by an authenticated V4 manifest.
    ///
    /// StepEp is an internal recursion parity. Every public recursive-spend
    /// statement carries the release-qualified StepEq verifier identifier.
    public static func releaseQualifiedStepEqVerifierKeyIDV4(
        manifestSHA256: Data
    ) throws -> String {
        guard manifestSHA256.count == 32,
              manifestSHA256.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidField(
                "releaseQualifiedStepEqVerifierKeyIDV4.manifestSHA256"
            )
        }
        return "\(pastaCycleBackendV4):\(stepEqCircuitIDV4)-\(manifestSHA256.hexEncodedString())"
    }
    public static let artifactRolesV4 = [
        "step_eq_params_ipa",
        "step_eq_proving_key",
        "step_eq_verifying_key",
        "step_eq_bootstrap_witness",
        "step_ep_params_ipa",
        "step_ep_proving_key",
        "step_ep_verifying_key",
        "step_ep_bootstrap_witness",
    ]
    public static let artifactFileNamesV4 = [
        "step-eq.params-ipa.krv4",
        "step-eq.proving-key.krv4",
        "step-eq.verifying-key.krv4",
        "step-eq.bootstrap-witness.krv4",
        "step-ep.params-ipa.krv4",
        "step-ep.proving-key.krv4",
        "step-ep.verifying-key.krv4",
        "step-ep.bootstrap-witness.krv4",
    ]
    public static let absoluteMaximumProofPairBytesV4: UInt32 = 384 * 1024
    public static let maximumProofSteps: UInt32 = 128
    /// Maximum size of one streamed `KRV4KEY` artifact.
    public static let artifactMaximumStreamedFileBytesV4: UInt64 = 5 * 1024 * 1024 * 1024
    /// Maximum size of an archive materialized as one Swift `Data` value.
    public static let artifactMaximumInMemoryArchiveBytes = 256 * 1024 * 1024
    public static let artifactMaximumChunkBytes = 1 * 1024 * 1024
    public static let topUpFinalityProofMaximumArchiveBytes = 2 * 1_024 * 1_024
    public static let topUpFinalityRosterMaximumArchiveBytes = 2 * 1_024 * 1_024
    public static let topUpFinalityAnchorMaximumArchiveBytes = 64 * 1_024
    public static let proofAttachmentWireName =
        "iroha_data_model::proof::ProofAttachment"

    public static let scaledAmountWireName = wire("KagemushaScaledAmountV2")
    public static let noteWireName = wire("KagemushaSpendableNoteDescriptorV2")
    public static let recipientOutputDerivationRequestWireName =
        wire("KagemushaRecipientOutputDerivationRequestV2")
    public static let recipientOutputDerivationResultWireName =
        wire("KagemushaRecipientOutputDerivationResultV2")
    public static let noteOpeningWireName =
        "connect_norito_bridge::KagemushaNoteOpeningV2"
    public static let membershipWitnessWireName =
        "connect_norito_bridge::KagemushaNoteMembershipWitnessV2"
    public static let spendableMembershipWitnessWireName =
        wire("KagemushaNoteMembershipWitnessV2")
    public static let outputMembershipPathsWireNameV4 =
        "connect_norito_bridge::KagemushaOutputMembershipPathsV4"
    public static let outputMembershipFrontierWireNameV4 =
        "connect_norito_bridge::KagemushaOutputMembershipFrontierV4"
    public static let initLocalRequestWireNameV4 =
        "connect_norito_bridge::KagemushaRecursiveSpendInitLocalRequestV4"
    public static let appendLocalRequestWireNameV4 =
        "connect_norito_bridge::KagemushaRecursiveSpendAppendLocalRequestV4"
    public static let verifyLocalRequestWireNameV4 =
        "connect_norito_bridge::KagemushaRecursiveSpendVerifyLocalRequestV4"
    public static let redeemLocalRequestWireNameV4 =
        "connect_norito_bridge::KagemushaRecursiveSpendRedeemLocalRequestV4"
    public static let redemptionChangePrepareRequestWireNameV4 =
        "connect_norito_bridge::KagemushaRecursiveSpendRedemptionChangePrepareRequestV4"
    public static let redemptionChangePrepareResultWireNameV4 =
        "connect_norito_bridge::KagemushaRecursiveSpendRedemptionChangePrepareResultV4"
    public static let peerSplitChangePrepareRequestWireNameV4 =
        "connect_norito_bridge::KagemushaRecursiveSpendPeerSplitChangePrepareRequestV4"
    public static let peerSplitChangePrepareResultWireNameV4 =
        "connect_norito_bridge::KagemushaRecursiveSpendPeerSplitChangePrepareResultV4"
    public static let branchPathWireName = wire("KagemushaRecursiveSpendBranchPathV2")
    public static let branchClaimWireName = wire("KagemushaRecursiveSpendBranchClaimV2")
    public static let recipientRequestPayloadWireName =
        wire("KagemushaRecipientPaymentRequestSigningPayloadV2")
    public static let recipientRequestWireName = wire("KagemushaRecipientPaymentRequestV2")
    public static let recipientReceiveOfferWireName =
        "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2"
    public static let authorizationWireName = wire("KagemushaRequestAuthorizationV2")
    public static let authorizationPreparationWireName =
        "connect_norito_bridge::KagemushaRequestAuthorizationPreparationV2"
    public static let artifactManifestWireName =
        wire("KagemushaRecursiveSpendArtifactManifestV4")
    public static let artifactBindingWireNameV4 =
        wire("KagemushaRecursiveSpendArtifactBindingV4")
    public static let nativeCapabilitiesWireNameV4 =
        wire("KagemushaRecursiveSpendNativeCapabilitiesV4")
    public static let topUpShieldBuildRequestWireNameV4 =
        "connect_norito_bridge::KagemushaTopUpShieldBuildRequestV4"
    public static let topUpShieldEvidenceWireName = wire("KagemushaTopUpShieldEvidenceV2")
    public static let topUpUnsignedWireNameV4 = wire("KagemushaRecursiveSpendTopUpUnsignedV4")
    public static let topUpRequestWireName = "iroha.torii.v1.offline.top_up.request"
    public static let topUpFinalityProofWireName = wire("KagemushaTopUpFinalityProofV2")
    public static let topUpFinalityRosterArtifactWireName =
        wire("KagemushaTopUpFinalityRosterArtifactV2")
    public static let acknowledgementPayloadWireName =
        wire("KagemushaReceiverAcknowledgementPayloadV2")
    public static let acknowledgementWireName = wire("KagemushaReceiverAcknowledgementV2")
    public static let acknowledgementVerifyResultWireName =
        wire("KagemushaReceiverAcknowledgementVerifyResultV2")
    public static let redeemRequestWireName = "iroha.torii.v1.offline.redeem.request"

    // Canonical ABI-21 data-model carriers.
    public static let bundleWireNameV4 = wire("KagemushaRecursiveSpendBundleV4")
    public static let topUpAnchorWireNameV4 =
        wire("KagemushaRecursiveSpendTopUpAnchorV4")
    public static let topUpFinalityEvidenceWireNameV4 =
        wire("KagemushaRecursiveSpendTopUpFinalityEvidenceV4")
    public static let topUpProvenanceWireNameV4 =
        wire("KagemushaRecursiveSpendTopUpProvenanceV4")
    public static let initRequestWireNameV4 =
        wire("KagemushaRecursiveSpendInitRequestV4")
    public static let initResultWireNameV4 =
        wire("KagemushaRecursiveSpendInitResultV4")
    public static let appendInputWireNameV4 =
        wire("KagemushaRecursiveSpendAppendInputV4")
    public static let splitIntentWireNameV4 =
        wire("KagemushaRecursiveSpendSplitIntentV4")
    public static let bundleSummaryWireNameV4 =
        wire("KagemushaRecursiveSpendBundleSummaryV4")
    public static let splitResultWireNameV4 =
        wire("KagemushaRecursiveSpendSplitResultV4")
    public static let peerPaymentWireNameV4 =
        wire("KagemushaRecursiveSpendPeerPaymentV4")
    public static let verifyRequestWireNameV4 =
        wire("KagemushaRecursiveSpendVerifyRequestV4")
    public static let verifyResultWireNameV4 =
        wire("KagemushaRecursiveSpendVerifyResultV4")
    public static let redeemBuildResultWireNameV4 =
        wire("KagemushaRecursiveSpendRedeemBuildResultV4")
    public static let redeemUnsignedWireNameV4 =
        wire("KagemushaRecursiveSpendRedeemUnsignedV4")
    public static let redeemResultWireNameV4 =
        wire("KagemushaRecursiveSpendRedeemResultV4")

    /// Exact Rust `Archived<T>` alignment for every Kagemusha wire schema.
    /// The mapping is static because decoding must never infer archived layout
    /// from attacker-controlled bytes.
    static func archivedPayloadAlignment(forWireName schema: String) -> Int? {
        switch schema {
        case scaledAmountWireName,
             noteWireName,
             recipientOutputDerivationRequestWireName,
             recipientOutputDerivationResultWireName,
             recipientRequestPayloadWireName,
             recipientRequestWireName,
             recipientReceiveOfferWireName,
             topUpRequestWireName,
             redeemRequestWireName,
             bundleWireNameV4,
             topUpAnchorWireNameV4,
             topUpFinalityEvidenceWireNameV4,
             initRequestWireNameV4,
             initLocalRequestWireNameV4,
             initResultWireNameV4,
             topUpShieldBuildRequestWireNameV4,
             topUpUnsignedWireNameV4,
             appendInputWireNameV4,
             appendLocalRequestWireNameV4,
             splitIntentWireNameV4,
             bundleSummaryWireNameV4,
             splitResultWireNameV4,
             peerPaymentWireNameV4,
             verifyRequestWireNameV4,
             verifyLocalRequestWireNameV4,
             verifyResultWireNameV4,
             redeemLocalRequestWireNameV4,
             redemptionChangePrepareRequestWireNameV4,
             redemptionChangePrepareResultWireNameV4,
             redeemBuildResultWireNameV4,
             redeemUnsignedWireNameV4,
             redeemResultWireNameV4:
            // These archived types contain an inline u128, directly or through
            // an inline nested value.
            return 16
        case noteOpeningWireName,
             branchPathWireName,
             acknowledgementVerifyResultWireName:
            return 1
        case proofAttachmentWireName,
             membershipWitnessWireName,
             spendableMembershipWitnessWireName,
             outputMembershipPathsWireNameV4,
             outputMembershipFrontierWireNameV4,
             branchClaimWireName,
             authorizationPreparationWireName,
             authorizationWireName,
             artifactBindingWireNameV4,
             artifactManifestWireName,
             nativeCapabilitiesWireNameV4,
             topUpShieldEvidenceWireName,
             topUpFinalityProofWireName,
             topUpFinalityRosterArtifactWireName,
             topUpProvenanceWireNameV4,
             acknowledgementPayloadWireName,
             acknowledgementWireName:
            return 8
        default:
            return nil
        }
    }

    static func requiredHeaderPaddingLength(forWireName schema: String) -> Int? {
        guard let alignment = archivedPayloadAlignment(forWireName: schema) else {
            return nil
        }
        return noritoHeaderPaddingLength(payloadAlignment: alignment)
    }

    static func frameArchive(schema: String, payload: Data) -> Data {
        guard let payloadAlignment = archivedPayloadAlignment(forWireName: schema) else {
            preconditionFailure("Unknown Kagemusha Norito schema: \(schema)")
        }
        return noritoEncode(
            typeName: schema,
            payload: payload,
            flags: NoritoHeader.compactLen,
            payloadAlignment: payloadAlignment
        )
    }

    public static let topUpShieldCircuitID =
        "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3"
    public static let maximumPeerTextEnvelopeBytes = 12 * 1024
    /// Six-byte `PKK2?.` discriminator prepended to a direct peer text envelope.
    public static let peerTextDiscriminatorBytes = 6
    /// Largest unpadded base64url archive that fits beside the discriminator in
    /// the direct 12 KiB text envelope. Streamed QR archives use the larger raw
    /// protocol limit below.
    public static let maximumPeerTextArchiveBytes =
        (maximumPeerTextEnvelopeBytes - peerTextDiscriminatorBytes) * 3 / 4
    public static let maximumPeerArchiveBytesV2 = 32 * 1024
    /// Exact bridge ceiling for the CBOR object returned by App Attest.
    public static let maximumIosAppAttestAssertionObjectBytesV2 = 8 * 1024
    /// Fixed App Attest assertion header before the mandatory extension CBOR.
    public static let iosAppAttestAuthenticatorDataFixedHeaderBytesV2 = 37
    /// Minimum current App Attest assertion size, including extension CBOR.
    public static let minimumIosAppAttestAuthenticatorDataBytesV2 =
        iosAppAttestAuthenticatorDataFixedHeaderBytesV2 + 1
    /// Exact protocol ceiling for App Attest authenticator data.
    public static let maximumIosAppAttestAuthenticatorDataBytesV2 = 4 * 1024
    /// Consensus ceiling for one canonical recipient-only ABI-21 peer archive.
    /// Text and individual QR/APDU frames retain smaller independent bounds.
    public static let maximumPeerArchiveBytesV4 = 32 * 1024 * 1024
    public static let maximumPeerArchiveBytes = maximumPeerArchiveBytesV4
    /// Maximum canonical ABI-21 promoted-release marker accepted by native install.
    public static let maximumPromotionRecordBytesV4 = 1_024 * 1_024
    /// Exact Rust `KAGEMUSHA_RECURSIVE_SPEND_TOPUP_PROVENANCE_MAX_BYTES_V4`.
    public static let maximumTopUpProvenanceArchiveBytesV4 =
        topUpFinalityRosterMaximumArchiveBytes
            + Int(maximumInputsPerTransition)
                * (topUpFinalityProofMaximumArchiveBytes
                    + topUpFinalityAnchorMaximumArchiveBytes)
            + 64 * 1024
    public static let maximumOutputMembershipFrontierArchiveBytesV4 = 4 * 1024
    public static let maximumOutputMembershipPathsArchiveBytesV4 = 16 * 1024
    public static let maximumRedemptionChangePreparationArchiveBytesV4 = 64 * 1024
    public static let maximumPeerSplitChangePreparationArchiveBytesV4 = 64 * 1024
    public static let maximumPeerSplitChangePreparationRequestArchiveBytesV4 =
        2 * maximumPeerArchiveBytesV4 + 3 * maximumPeerArchiveBytesV2
    public static let maximumBranchClaims = 2
    public static let transitionTagBytes = 24
    public static let transitionTagDomain =
        "iroha:kagemusha:v2:transition-tag:sha256-192"
    public static let maximumAuthorizationTTLMilliseconds: UInt64 = 5 * 60 * 1_000

    public static let requiredProofSymbols = [
        "connect_norito_kagemusha_recursive_spend_init_v4",
        "connect_norito_kagemusha_recursive_spend_append_v4",
        "connect_norito_kagemusha_recursive_spend_verify_v4",
        "connect_norito_kagemusha_recursive_spend_redeem_v4",
    ]

    public static let requiredProtocolSymbols = [
        "connect_norito_kagemusha_recursive_spend_capabilities_v4",
        "connect_norito_kagemusha_topup_finality_verify_v4",
        "connect_norito_kagemusha_topup_shield_build_unsigned_v4",
        "connect_norito_kagemusha_recursive_spend_topup_v4",
        "connect_norito_kagemusha_recursive_spend_topup_unsigned_payload_digest_v4",
        "connect_norito_kagemusha_recursive_spend_topup_finalize_request_v4",
        "connect_norito_kagemusha_recursive_spend_redeem_unsigned_payload_digest_v4",
        "connect_norito_kagemusha_recursive_spend_redeem_finalize_request_v4",
        "connect_norito_kagemusha_recursive_spend_redemption_change_prepare_v4",
        "connect_norito_kagemusha_secret_free_buffer",
        "connect_norito_kagemusha_receiver_key_reference_v2",
        "connect_norito_kagemusha_recipient_output_derive_v2",
        "connect_norito_kagemusha_recipient_payment_request_signing_bytes_v2",
        "connect_norito_kagemusha_recipient_payment_request_create_v2",
        "connect_norito_kagemusha_recipient_payment_request_verify_v2",
        "connect_norito_kagemusha_recipient_lineage_query_create_v2",
        "connect_norito_kagemusha_recipient_registration_lineage_verify_v2",
        "connect_norito_kagemusha_recipient_receive_offer_create_v2",
        "connect_norito_kagemusha_recipient_receive_offer_project_v2",
        "connect_norito_kagemusha_recipient_receive_offer_verify_v2",
        "connect_norito_kagemusha_request_authorization_signing_bytes_v2",
        "connect_norito_kagemusha_request_authorization_finalize_hardware_v2",
        "connect_norito_kagemusha_request_authorization_finalize_ios_app_attest_v2",
        "connect_norito_kagemusha_receiver_acknowledgement_payload_v2",
        "connect_norito_kagemusha_receiver_acknowledgement_signing_bytes_v2",
        "connect_norito_kagemusha_receiver_acknowledgement_create_v2",
        "connect_norito_kagemusha_receiver_acknowledgement_verify_v2",
        "connect_norito_kagemusha_recursive_spend_peer_split_change_prepare_v4",
        "connect_norito_kagemusha_recursive_spend_peer_payment_from_split_v4",
        "connect_norito_kagemusha_recursive_spend_peer_payment_validate_v4",
        "connect_norito_kagemusha_recursive_spend_bundle_summary_v4",
        "connect_norito_kagemusha_output_membership_frontier_build_v4",
        "connect_norito_kagemusha_output_membership_paths_derive_v4",
        "connect_norito_kagemusha_recursive_spend_branch_validate_v4",
        "connect_norito_kagemusha_recursive_spend_topup_provenance_build_v4",
        "connect_norito_kagemusha_recursive_spend_topup_provenance_validate_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_begin_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_write_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_finalize_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_cancel_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_set_install_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_set_is_installed_v4",
        "connect_norito_kagemusha_recursive_spend_installed_manifest_sha256_v4",
        "connect_norito_kagemusha_recursive_spend_artifact_set_uninstall_v4",
    ]

    /// Complete native-symbol inventory required by first-release readiness checks.
    public static let requiredNativeSymbols = requiredProofSymbols + requiredProtocolSymbols

    public static var hasRequiredNativeSymbols: Bool {
        NoritoNativeBridge.shared.isKagemushaRecursiveSpendBridgeAvailable
            && NoritoNativeBridge.shared.hasKagemushaRecursiveSpendV4Symbols(
                requiredNativeSymbols
            )
    }

    public static func nativeCapabilitiesV4() throws
        -> KagemushaRecursiveSpendNativeCapabilitiesV4
    {
        guard let archive = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendCapabilitiesV4() else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendCodecs.decodeNativeCapabilitiesV4(archive)
    }

    /// Exact local production capability; Torii readiness remains an additional requirement.
    public static var isProductionAvailable: Bool {
        productionAvailability(
            hasRequiredNativeSymbols: hasRequiredNativeSymbols,
            probe: {
                try nativeCapabilitiesV4().proofBackendAvailable
            }
        )
    }

    /// True when the ABI-22 bridge was compiled with the audited production
    /// promotion feature, even if its authenticated artifact set has not been
    /// installed yet. Setup UI uses this non-cached probe to avoid an artifact
    /// bootstrap cycle; value-moving operations still require
    /// `isProductionAvailable` after installation.
    public static var isProductionCompiledAndLinked: Bool {
        productionCompilationAvailability(
            hasRequiredNativeSymbols: hasRequiredNativeSymbols,
            probe: {
                let capabilities = try nativeCapabilitiesV4()
                return (
                    capabilities.proofBackendAvailable,
                    capabilities.missingGates
                )
            }
        )
    }

    /// Keep ABI linkage separate from mutable artifact readiness. In
    /// particular, an unavailable response before artifact promotion must not
    /// be remembered as a missing symbol: the probe is deliberately executed
    /// afresh on every readiness check.
    static func productionAvailability(
        hasRequiredNativeSymbols: Bool,
        probe: () throws -> Bool
    ) -> Bool {
        guard hasRequiredNativeSymbols else { return false }
        return (try? probe()) == true
    }

    static func productionCompilationAvailability(
        hasRequiredNativeSymbols: Bool,
        probe: () throws -> (proofBackendAvailable: Bool, missingGates: [String])
    ) -> Bool {
        guard hasRequiredNativeSymbols,
              let capabilities = try? probe() else {
            return false
        }
        return capabilities.proofBackendAvailable
            || !capabilities.missingGates.contains("authenticated-production-promotion")
    }

    static func requireArchive(_ archive: Data, schema: String, field: String) throws {
        guard let requiredPaddingLength = requiredHeaderPaddingLength(forWireName: schema),
              !archive.isEmpty,
              archive.count <= artifactMaximumInMemoryArchiveBytes,
              let frame = noritoDecodeFrame(archive),
              frame.header.schema == noritoSchemaHash(forTypeName: schema),
              frame.header.compression == .none,
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == requiredPaddingLength,
              !frame.payload.isEmpty else {
            throw KagemushaRecursiveSpendError.invalidArchive(field)
        }
    }

    static func requireNonzeroFixed32(_ value: Data, field: String) throws {
        guard value.count == 32, value.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidField(field)
        }
    }

    static func transitionTag(for transitionBinding: Data) throws -> Data {
        try requireNonzeroFixed32(transitionBinding, field: "transitionBinding")
        var preimage = Data(transitionTagDomain.utf8)
        preimage.append(0)
        preimage.append(transitionBinding)
        let tag = Data(SHA256.hash(data: preimage).prefix(transitionTagBytes))
        guard tag.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidField("transitionTag")
        }
        return tag
    }

    static func requirePortableText(_ value: String, field: String, maximum: Int = 128) throws {
        guard !value.isEmpty,
              value.count <= maximum,
              value == value.trimmingCharacters(in: .whitespacesAndNewlines),
              !value.unicodeScalars.contains(where: CharacterSet.controlCharacters.contains)
        else {
            throw KagemushaRecursiveSpendError.invalidField(field)
        }
    }

    static func canonicalAccountAddress(
        _ value: String,
        field: String,
        expectedChainDiscriminant: UInt16? = nil
    ) throws -> (address: AccountAddress, chainDiscriminant: UInt16) {
        do {
            guard !value.isEmpty,
                  value.utf8.elementsEqual(
                      value.trimmingCharacters(in: .whitespacesAndNewlines).utf8
                  ),
                  !value.contains("@"),
                  !value.contains("#"),
                  !value.contains("$") else {
                throw KagemushaRecursiveSpendError.invalidField(field)
            }
            let chainDiscriminant = try AccountAddress
                .inspectI105NetworkPrefix(value).chainDiscriminant
            if let expectedChainDiscriminant,
               chainDiscriminant != expectedChainDiscriminant {
                throw KagemushaRecursiveSpendError.invalidField(field)
            }
            let address = try AccountAddress.parseEncodedSwiftOnly(
                value,
                expectedPrefix: chainDiscriminant
            )
            let canonical = try address.toI105(networkPrefix: chainDiscriminant)
            guard canonical.utf8.elementsEqual(value.utf8) else {
                throw KagemushaRecursiveSpendError.invalidField(field)
            }
            return (address, chainDiscriminant)
        } catch let error as KagemushaRecursiveSpendError {
            throw error
        } catch {
            throw KagemushaRecursiveSpendError.invalidField(field)
        }
    }

    /// Match the ABI-21/V4 cross-platform artifact identifier contract byte
    /// for byte. This is deliberately stricter than general portable text.
    static func requirePortableArtifactIdentifier(
        _ value: String,
        field: String
    ) throws {
        let bytes = Array(value.utf8)
        let isASCIIAlphanumeric: (UInt8) -> Bool = { byte in
            (UInt8(ascii: "0")...UInt8(ascii: "9")).contains(byte)
                || (UInt8(ascii: "A")...UInt8(ascii: "Z")).contains(byte)
                || (UInt8(ascii: "a")...UInt8(ascii: "z")).contains(byte)
        }
        let isPortable: (UInt8) -> Bool = { byte in
            isASCIIAlphanumeric(byte)
                || byte == UInt8(ascii: ".")
                || byte == UInt8(ascii: "_")
                || byte == UInt8(ascii: "-")
        }
        guard !bytes.isEmpty,
              bytes.count <= 128,
              bytes.allSatisfy(isPortable),
              bytes.first.map(isASCIIAlphanumeric) == true,
              bytes.last.map(isASCIIAlphanumeric) == true else {
            throw KagemushaRecursiveSpendError.invalidField(field)
        }
        let basename = String(decoding: bytes.prefix { $0 != UInt8(ascii: ".") }, as: UTF8.self)
            .lowercased()
        let basenameBytes = Array(basename.utf8)
        let reserved = ["con", "prn", "aux", "nul"]
        let numberedDevice = basenameBytes.count == 4
            && (basename.hasPrefix("com") || basename.hasPrefix("lpt"))
            && (UInt8(ascii: "1")...UInt8(ascii: "9")).contains(basenameBytes[3])
        guard !reserved.contains(basename), !numberedDevice else {
            throw KagemushaRecursiveSpendError.invalidField(field)
        }
    }

    static func validateBranchClaims(
        _ claims: [KagemushaRecursiveSpendBranchClaim]
    ) throws {
        guard (1...maximumBranchClaims).contains(claims.count) else {
            throw KagemushaRecursiveSpendError.invalidField("branchClaims")
        }
        for index in claims.indices where index > claims.startIndex {
            let claim = claims[index]
            guard claims[index - 1].path.canonicallyPrecedes(claim.path) else {
                throw KagemushaRecursiveSpendError.invalidField("branchClaims.order")
            }
            guard !claims[..<index].contains(where: { $0.path.conflicts(with: claim.path) }) else {
                throw KagemushaRecursiveSpendError.invalidField("branchClaims.conflict")
            }
            for previous in claims[..<index] {
                guard previous.path.lineageRoot == claim.path.lineageRoot else { continue }
                let sharedDepth = min(previous.path.depth, claim.path.depth)
                for parentDepth in 0..<sharedDepth
                    where previous.path.hasSamePrefix(as: claim.path, depth: parentDepth)
                {
                    guard previous.transitionTags[Int(parentDepth)]
                        == claim.transitionTags[Int(parentDepth)] else {
                        throw KagemushaRecursiveSpendError.invalidField(
                            "branchClaims.transitionChoice"
                        )
                    }
                }
            }
        }
    }

    private static func wire(_ type: String) -> String {
        "iroha_data_model::offline::model::\(type)"
    }

}

public struct KagemushaPublicKey: Equatable, Hashable, Sendable {
    public let algorithm: UInt8
    public let payload: Data

    public init(algorithm: UInt8 = 0, payload: Data) throws {
        guard !payload.isEmpty, payload.count <= 8_192 else {
            throw KagemushaRecursiveSpendError.invalidField("receiverPublicKey")
        }
        if algorithm == 0, payload.count != 32 {
            throw KagemushaRecursiveSpendError.invalidField("receiverPublicKey.ed25519")
        }
        self.algorithm = algorithm
        self.payload = Data(payload)
    }

}

/// Sole first-release Kagemusha device authority key.
///
/// The protocol accepts exactly one canonical uncompressed SEC1 NIST P-256
/// point (`0x04 || x || y`). It intentionally has no algorithm selector.
public struct KagemushaDevicePublicKeyV2: Equatable, Hashable, Sendable {
    public static let sec1ByteCount = 65
    public let sec1Bytes: Data

    public init(sec1Bytes: Data) throws {
        guard sec1Bytes.count == Self.sec1ByteCount,
              sec1Bytes.first == 0x04,
              let key = try? P256.Signing.PublicKey(x963Representation: sec1Bytes),
              key.x963Representation == sec1Bytes else {
            throw KagemushaRecursiveSpendError.invalidField("devicePublicKey")
        }
        self.sec1Bytes = Data(sec1Bytes)
    }

    public func receiverKeyReference() throws -> Data {
        guard let reference = try NoritoNativeBridge.shared
            .kagemushaReceiverKeyReferenceV2(publicKey: sec1Bytes) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            reference,
            field: "recipientKeyReference"
        )
        return reference
    }

    public func isValidSignature(
        _ signature: KagemushaDeviceSignatureV2,
        for message: Data
    ) -> Bool {
        guard let key = try? P256.Signing.PublicKey(x963Representation: sec1Bytes),
              let parsed = try? P256.Signing.ECDSASignature(
                  rawRepresentation: signature.rawBytes
              ) else {
            return false
        }
        return key.isValidSignature(parsed, for: message)
    }
}

/// Canonical fixed-width low-S ECDSA-P256-SHA256 device signature (`r || s`).
public struct KagemushaDeviceSignatureV2: Equatable, Hashable, Sendable {
    public static let rawByteCount = 64
    private static let scalarByteCount = 32
    private static let order = Data([
        0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84,
        0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
    ])
    private static let halfOrder = Data([
        0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00,
        0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42,
        0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa8,
    ])

    public let rawBytes: Data

    /// Parse a strict ASN.1 DER ECDSA signature and normalize it to the unique
    /// low-S fixed-width representation used by the hardware-assertion wire protocol.
    public init(derBytes: Data) throws {
        guard let parsed = try? P256.Signing.ECDSASignature(
            derRepresentation: derBytes
        ), parsed.derRepresentation == derBytes else {
            throw KagemushaRecursiveSpendError.invalidField("deviceSignature.der")
        }
        var raw = parsed.rawRepresentation
        let s = Data(raw.suffix(Self.scalarByteCount))
        if Self.halfOrder.lexicographicallyPrecedes(s) {
            raw.replaceSubrange(
                Self.scalarByteCount..<Self.rawByteCount,
                with: Self.subtract(s, from: Self.order)
            )
        }
        try self.init(rawBytes: raw)
    }

    public init(rawBytes: Data) throws {
        guard rawBytes.count == Self.rawByteCount else {
            throw KagemushaRecursiveSpendError.invalidField("deviceSignature")
        }
        let r = Data(rawBytes.prefix(Self.scalarByteCount))
        let s = Data(rawBytes.suffix(Self.scalarByteCount))
        guard Self.isNonzeroScalarBelowOrder(r),
              Self.isNonzeroScalarBelowOrder(s),
              !Self.halfOrder.lexicographicallyPrecedes(s),
              (try? P256.Signing.ECDSASignature(rawRepresentation: rawBytes)) != nil else {
            throw KagemushaRecursiveSpendError.invalidField("deviceSignature")
        }
        self.rawBytes = Data(rawBytes)
    }

    public func strictDERBytes() throws -> Data {
        guard let signature = try? P256.Signing.ECDSASignature(
            rawRepresentation: rawBytes
        ) else {
            throw KagemushaRecursiveSpendError.invalidField("deviceSignature")
        }
        return signature.derRepresentation
    }

    private static func isNonzeroScalarBelowOrder(_ scalar: Data) -> Bool {
        scalar.count == scalarByteCount
            && scalar.contains(where: { $0 != 0 })
            && scalar.lexicographicallyPrecedes(order)
    }

    private static func subtract(_ value: Data, from minuend: Data) -> Data {
        let lhs = [UInt8](minuend)
        let rhs = [UInt8](value)
        var result = [UInt8](repeating: 0, count: scalarByteCount)
        var borrow = 0
        for index in stride(from: scalarByteCount - 1, through: 0, by: -1) {
            var difference = Int(lhs[index]) - Int(rhs[index]) - borrow
            if difference < 0 {
                difference += 256
                borrow = 1
            } else {
                borrow = 0
            }
            result[index] = UInt8(difference)
        }
        return Data(result)
    }
}

public struct KagemushaSpendableNoteDescriptor: Equatable, Hashable, Sendable {
    public let networkID: NetworkId
    public let assetDefinitionID: String
    public let noteCommitment: Data
    public let spendNullifier: Data
    public let amount: KagemushaScaledAmount

    public init(
        networkID: NetworkId,
        assetDefinitionID: String,
        noteCommitment: Data,
        spendNullifier: Data,
        amount: KagemushaScaledAmount
    ) throws {
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil else {
            throw KagemushaRecursiveSpendError.invalidField("assetDefinitionID")
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            noteCommitment,
            field: "noteCommitment"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            spendNullifier,
            field: "spendNullifier"
        )
        guard noteCommitment != spendNullifier else {
            throw KagemushaRecursiveSpendError.invalidField("spendNullifier")
        }
        self.networkID = networkID
        self.assetDefinitionID = assetDefinitionID
        self.noteCommitment = Data(noteCommitment)
        self.spendNullifier = Data(spendNullifier)
        self.amount = amount
    }
}

/// Uniform local opening for every Kagemusha note created by top-up, receive,
/// sender change, or partial redemption.
///
/// This type is secret-bearing. Its Norito archive is accepted only by local
/// native proof entrypoints and must be encrypted at rest, wiped after bridge
/// calls, and never included in a peer or Torii payload.
///
/// The spend key is wallet-scoped: every note owned by one wallet must resolve
/// to the same device-bound key. Wallet
/// state should persist that key's secure reference once and store only the
/// note-specific rho and diversifier beside each note; this value is the
/// transient bridge-bound reconstruction.
public struct KagemushaNoteOpening: Equatable, Sendable {
    public let spendKey: Data
    public let rho: Data
    public let diversifier: Data

    public init(spendKey: Data, rho: Data, diversifier: Data) throws {
        for (field, value) in [
            ("spendKey", spendKey),
            ("rho", rho),
            ("diversifier", diversifier),
        ] {
            try KagemushaRecursiveSpend.requireNonzeroFixed32(value, field: field)
        }
        self.spendKey = Data(spendKey)
        self.rho = Data(rho)
        self.diversifier = Data(diversifier)
    }

    func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecs.encodeNoteOpening(self)
    }
}

/// Encrypted local membership witness retained with an owned recursive note.
///
/// The real path authenticates the note at `leafIndex`. The dummy path
/// authenticates an empty leaf at the same root for the fixed-width circuit's
/// mandatory dummy slot and never authorizes a second parent. This local
/// archive is never sent to Torii; the public data-model form is embedded beside
/// a recipient bundle so the receiver can persist independently spendable cash.
public struct KagemushaNoteMembershipWitness: Equatable, Sendable {
    public let leafIndex: UInt32
    public let inputPath: PrivacyConfidentialMerklePathWitnessV2
    public let dummyInputPath: PrivacyConfidentialMerklePathWitnessV2

    public var dummyLeafIndex: UInt32 {
        dummyInputPath.directions.enumerated().reduce(UInt32(0)) {
            $0 | (UInt32($1.element) << UInt32($1.offset))
        }
    }

    public init(
        leafIndex: UInt32,
        inputPath: PrivacyConfidentialMerklePathWitnessV2,
        dummyInputPath: PrivacyConfidentialMerklePathWitnessV2
    ) throws {
        guard inputPath.root == dummyInputPath.root,
              inputPath.root.contains(where: { $0 != 0 }),
              leafIndex < UInt32(PrivacyConfidentialWitnessCodecs.confidentialTreeCapacityV2)
        else {
            throw KagemushaRecursiveSpendError.invalidField("membershipWitness")
        }
        for (index, direction) in inputPath.directions.enumerated() {
            guard direction == UInt8((UInt64(leafIndex) >> UInt64(index)) & 1) else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "membershipWitness.inputPath.directions"
                )
            }
        }
        let dummyLeafIndex = dummyInputPath.directions.enumerated().reduce(UInt32(0)) {
            $0 | (UInt32($1.element) << UInt32($1.offset))
        }
        guard dummyLeafIndex != leafIndex else {
            throw KagemushaRecursiveSpendError.invalidField(
                "membershipWitness.dummyInputPath"
            )
        }
        self.leafIndex = leafIndex
        self.inputPath = inputPath
        self.dummyInputPath = dummyInputPath
    }

    public func noritoEncoded() throws -> Data {
        try KagemushaRecursiveSpendCodecs.encodeMembershipWitness(self)
    }

    public static func decode(_ archive: Data) throws -> Self {
        try KagemushaRecursiveSpendCodecs.decodeMembershipWitness(archive)
    }
}

/// Paths proving that one confidential output was inserted and remains a
/// member of the tree after the complete operation. Commitments are omitted:
/// native code derives and binds them from the finalized operation.
public struct KagemushaConfidentialVerifierBinding: Equatable, Sendable {
    public let role: KagemushaRecursiveSpend.VerifierRole
    public let backend: String
    public let name: String
    public let commitment: Data
    public let blockHeight: UInt64

    public init(
        role: KagemushaRecursiveSpend.VerifierRole,
        verifier: ToriiKagemushaActiveTransferVerifier,
        blockHeight: UInt64
    ) throws {
        try self.init(
            role: role,
            backend: verifier.id.backend,
            name: verifier.id.name,
            circuitID: verifier.circuitId,
            commitmentHex: verifier.commitment,
            activationHeight: verifier.activationHeight,
            withdrawalHeight: verifier.withdrawalHeight,
            blockHeight: blockHeight
        )
    }

    public init(
        role: KagemushaRecursiveSpend.VerifierRole,
        backend: String,
        name: String,
        circuitID: String,
        commitmentHex: String,
        activationHeight: UInt64,
        withdrawalHeight: UInt64?,
        blockHeight: UInt64
    ) throws {
        guard role == .transfer || role == .unshield,
              backend == role.registryBackend,
              name == role.registryName,
              circuitID == role.circuitID,
              activationHeight <= blockHeight,
              withdrawalHeight.map({ blockHeight < $0 }) != false,
              commitmentHex == commitmentHex.lowercased(),
              let commitment = Data(hexString: commitmentHex),
              commitment.count == 32,
              commitment.contains(where: { $0 != 0 }) else {
            throw KagemushaRecursiveSpendError.invalidField("confidentialVerifier")
        }
        self.role = role
        self.backend = backend
        self.name = name
        self.commitment = commitment
        self.blockHeight = blockHeight
    }

    var identifier: String { "\(backend):\(name)" }
}

public struct KagemushaRecipientOutputDerivationRequest: Equatable, Sendable {
    public let networkID: NetworkId
    public let assetDefinitionID: String
    public let amount: KagemushaScaledAmount
    public let requestID: Data

    public init(
        networkID: NetworkId,
        assetDefinitionID: String,
        amount: KagemushaScaledAmount,
        requestID: Data
    ) throws {
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil else {
            throw KagemushaRecursiveSpendError.invalidField("assetDefinitionID")
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(requestID, field: "requestID")
        self.networkID = networkID
        self.assetDefinitionID = assetDefinitionID
        self.amount = amount
        self.requestID = Data(requestID)
    }

    public func derive(
        opening: KagemushaNoteOpening
    ) throws -> KagemushaRecipientOutputDerivationResult {
        let requestArchive = try KagemushaRecursiveSpendCodecs
            .encodeRecipientOutputDerivationRequest(self)
        var openingArchive = try opening.noritoEncoded()
        defer { openingArchive.resetBytes(in: 0..<openingArchive.count) }
        guard let resultArchive = try NoritoNativeBridge.shared
            .kagemushaRecipientOutputDeriveV2(
                requestArchive: requestArchive,
                noteOpeningArchive: openingArchive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendCodecs.decodeRecipientOutputDerivationResult(
            resultArchive,
            request: self,
            opening: opening
        )
    }
}

public struct KagemushaRecipientOutputDerivationResult: Equatable, Sendable {
    public let recipientOutput: KagemushaSpendableNoteDescriptor
    /// Opaque sender-prover archive containing only amount, rho, and owner tag.
    /// Carry this unchanged in the signed peer request; never interpret it in wallet code.
    public let senderOutputProverMaterial: Data
    public let opening: KagemushaNoteOpening

    init(
        recipientOutput: KagemushaSpendableNoteDescriptor,
        senderOutputProverMaterial: Data,
        request: KagemushaRecipientOutputDerivationRequest,
        opening: KagemushaNoteOpening
    ) throws {
        guard recipientOutput.networkID == request.networkID,
              recipientOutput.assetDefinitionID == request.assetDefinitionID,
              recipientOutput.amount == request.amount,
              !senderOutputProverMaterial.isEmpty,
              senderOutputProverMaterial.count <= 4 * 1_024 else {
            throw KagemushaRecursiveSpendError.invalidField(
                "senderOutputProverMaterial"
            )
        }
        self.recipientOutput = recipientOutput
        self.senderOutputProverMaterial = Data(senderOutputProverMaterial)
        self.opening = opening
    }
}

public struct KagemushaRecursiveSpendBranchPath: Equatable, Hashable, Sendable {
    public static let maximumDepth: UInt8 = 64
    public let lineageRoot: Data
    public let depth: UInt8
    public let pathBits: Data

    public init(lineageRoot: Data, depth: UInt8, pathBits: Data) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(lineageRoot, field: "lineageRoot")
        guard depth <= Self.maximumDepth, pathBits.count == 8 else {
            throw KagemushaRecursiveSpendError.invalidField("branchPath")
        }
        let unused = 64 - Int(depth)
        if unused > 0 {
            let value = pathBits.reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
            let mask = unused == 64 ? UInt64.max : (UInt64(1) << UInt64(unused)) - 1
            guard value & mask == 0 else {
                throw KagemushaRecursiveSpendError.invalidField("branchPath.pathBits")
            }
        }
        self.lineageRoot = Data(lineageRoot)
        self.depth = depth
        self.pathBits = Data(pathBits)
    }

    public static func root(_ lineageRoot: Data) throws -> Self {
        try Self(lineageRoot: lineageRoot, depth: 0, pathBits: Data(repeating: 0, count: 8))
    }

    var parent: Self? {
        guard depth > 0 else { return nil }
        var bytes = Array(pathBits)
        let bitIndex = Int(depth - 1)
        bytes[bitIndex / 8] &= ~(1 << UInt8(7 - bitIndex % 8))
        return try? Self(
            lineageRoot: lineageRoot,
            depth: depth - 1,
            pathBits: Data(bytes)
        )
    }

    func isExactSibling(of other: Self) -> Bool {
        guard depth > 0,
              depth == other.depth,
              lineageRoot == other.lineageRoot,
              parent == other.parent else {
            return false
        }
        let bitIndex = Int(depth - 1)
        let mask = UInt8(1 << UInt8(7 - bitIndex % 8))
        return (pathBits[bitIndex / 8] & mask) != (other.pathBits[bitIndex / 8] & mask)
    }

    func canonicallyPrecedes(_ other: Self) -> Bool {
        if lineageRoot != other.lineageRoot {
            return lineageRoot.lexicographicallyPrecedes(other.lineageRoot)
        }
        if depth != other.depth { return depth < other.depth }
        return pathBits.lexicographicallyPrecedes(other.pathBits)
    }

    func isPrefix(of other: Self) -> Bool {
        guard lineageRoot == other.lineageRoot, depth <= other.depth else { return false }
        let fullBytes = Int(depth / 8)
        guard pathBits.prefix(fullBytes) == other.pathBits.prefix(fullBytes) else { return false }
        let partialBits = Int(depth % 8)
        guard partialBits > 0 else { return true }
        let mask = UInt8.max << UInt8(8 - partialBits)
        return pathBits[fullBytes] & mask == other.pathBits[fullBytes] & mask
    }

    func conflicts(with other: Self) -> Bool {
        isPrefix(of: other) || other.isPrefix(of: self)
    }

    func hasSamePrefix(as other: Self, depth prefixDepth: UInt8) -> Bool {
        guard lineageRoot == other.lineageRoot,
              prefixDepth <= depth,
              prefixDepth <= other.depth else {
            return false
        }
        let fullBytes = Int(prefixDepth / 8)
        guard pathBits.prefix(fullBytes) == other.pathBits.prefix(fullBytes) else {
            return false
        }
        let partialBits = Int(prefixDepth % 8)
        guard partialBits > 0 else { return true }
        let mask = UInt8.max << UInt8(8 - partialBits)
        return pathBits[fullBytes] & mask == other.pathBits[fullBytes] & mask
    }
}

/// Replay-safe claim for one independently spendable lineage leaf.
///
/// The exact `path.depth` tags select the transition used at every ancestor
/// edge. Each tag is the domain-separated 192-bit prefix of the producing
/// proof-bound 256-bit transition digest. Swift exposes one `Data` per tag;
/// Norito concatenates them into a single exact-depth byte vector.
public struct KagemushaRecursiveSpendBranchClaim: Equatable, Hashable, Sendable {
    public let path: KagemushaRecursiveSpendBranchPath
    public let transitionTags: [Data]

    public init(
        path: KagemushaRecursiveSpendBranchPath,
        transitionTags: [Data]
    ) throws {
        guard transitionTags.count == Int(path.depth),
              transitionTags.allSatisfy({
                  $0.count == KagemushaRecursiveSpend.transitionTagBytes
                    && $0.contains(where: { $0 != 0 })
              }) else {
            throw KagemushaRecursiveSpendError.invalidField(
                "branchClaim.transitionTags"
            )
        }
        self.path = path
        self.transitionTags = transitionTags.map { Data($0) }
    }

    public static func root(lineageRoot: Data) throws -> Self {
        try Self(
            path: KagemushaRecursiveSpendBranchPath.root(lineageRoot),
            transitionTags: []
        )
    }

    /// Returns whether two validated claims can represent competing spends.
    /// Exact replays, ancestor/descendant pairs, and alternative transitions
    /// from a shared parent conflict. Siblings produced by the same transition
    /// and claims funded by different top-ups remain independently spendable.
    public func conflicts(with other: Self) -> Bool {
        if path.conflicts(with: other.path) {
            return true
        }
        guard path.lineageRoot == other.path.lineageRoot else {
            return false
        }
        let sharedDepth = min(path.depth, other.path.depth)
        for parentDepth in 0..<sharedDepth
            where path.hasSamePrefix(as: other.path, depth: parentDepth)
        {
            if transitionTags[Int(parentDepth)]
                != other.transitionTags[Int(parentDepth)] {
                return true
            }
        }
        return false
    }
}

public struct KagemushaRecipientPaymentRequestSigningPayload: Equatable, Sendable {
    public let networkID: NetworkId
    public let assetDefinitionID: String
    public let amount: KagemushaScaledAmount
    public let recipient: String
    public let recipientKeyReference: Data
    public let receiverDeviceID: String
    public let receiverPublicKey: KagemushaDevicePublicKeyV2
    public let requestID: Data
    public let issuedAtMilliseconds: UInt64
    public let expiresAtMilliseconds: UInt64
    public let recipientOutput: KagemushaSpendableNoteDescriptor
    /// Signed, peer-carried archive consumed only by the sender proof builder.
    /// It must never contain the receiver spend key or output diversifier.
    public let senderOutputProverMaterial: Data

    public init(
        networkID: NetworkId,
        assetDefinitionID: String,
        amount: KagemushaScaledAmount,
        recipient: String,
        recipientKeyReference: Data,
        receiverDeviceID: String,
        receiverPublicKey: KagemushaDevicePublicKeyV2,
        requestID: Data,
        issuedAtMilliseconds: UInt64,
        expiresAtMilliseconds: UInt64,
        recipientOutput: KagemushaSpendableNoteDescriptor,
        senderOutputProverMaterial: Data
    ) throws {
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil else {
            throw KagemushaRecursiveSpendError.invalidField("assetDefinitionID")
        }
        _ = try KagemushaRecursiveSpend.canonicalAccountAddress(
            recipient,
            field: "recipient"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            recipientKeyReference,
            field: "recipientKeyReference"
        )
        try KagemushaRecursiveSpend.requirePortableText(
            receiverDeviceID,
            field: "receiverDeviceID"
        )
        try KagemushaRecursiveSpend.requireNonzeroFixed32(requestID, field: "requestID")
        guard issuedAtMilliseconds > 0,
              expiresAtMilliseconds > issuedAtMilliseconds,
              expiresAtMilliseconds - issuedAtMilliseconds
                <= KagemushaRecursiveSpend.maximumAuthorizationTTLMilliseconds,
              recipientOutput.networkID == networkID,
              recipientOutput.assetDefinitionID == assetDefinitionID,
              recipientOutput.amount == amount,
              !senderOutputProverMaterial.isEmpty,
              senderOutputProverMaterial.count <= 4 * 1024 else {
            throw KagemushaRecursiveSpendError.invalidField("recipientRequest")
        }
        self.networkID = networkID
        self.assetDefinitionID = assetDefinitionID
        self.amount = amount
        self.recipient = recipient
        self.recipientKeyReference = Data(recipientKeyReference)
        self.receiverDeviceID = receiverDeviceID
        self.receiverPublicKey = receiverPublicKey
        self.requestID = Data(requestID)
        self.issuedAtMilliseconds = issuedAtMilliseconds
        self.expiresAtMilliseconds = expiresAtMilliseconds
        self.recipientOutput = recipientOutput
        self.senderOutputProverMaterial = Data(senderOutputProverMaterial)
    }

    public func signingBytes() throws -> Data {
        let archive = try KagemushaRecursiveSpendCodecs.encodeRecipientRequestPayload(self)
        guard let bytes = try NoritoNativeBridge.shared
            .kagemushaRecipientPaymentRequestSigningBytesV2(payloadArchive: archive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return bytes
    }

    public func signed(
        signature: KagemushaDeviceSignatureV2
    ) throws -> KagemushaRecipientPaymentRequest {
        let payloadArchive = try KagemushaRecursiveSpendCodecs.encodeRecipientRequestPayload(self)
        guard let requestArchive = try NoritoNativeBridge.shared
            .kagemushaRecipientPaymentRequestCreateV2(
                payloadArchive: payloadArchive,
                signature: signature.rawBytes
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecipientPaymentRequest(
            payload: self,
            signature: signature,
            archive: requestArchive
        )
    }
}

public struct KagemushaRecipientPaymentRequest: Equatable, Sendable {
    public let payload: KagemushaRecipientPaymentRequestSigningPayload
    public let signature: KagemushaDeviceSignatureV2
    public let archive: Data

    init(
        payload: KagemushaRecipientPaymentRequestSigningPayload,
        signature: KagemushaDeviceSignatureV2,
        archive: Data
    ) throws {
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.recipientRequestWireName,
            field: "recipientRequest"
        )
        self.payload = payload
        self.signature = signature
        self.archive = Data(archive)
    }

    public func verified(atMilliseconds: UInt64) throws -> KagemushaVerifiedRecipientPaymentRequest {
        guard let digest = try NoritoNativeBridge.shared.kagemushaRecipientPaymentRequestVerifyV2(
            requestArchive: archive,
            verifiedAtMilliseconds: atMilliseconds
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(digest, field: "requestDigest")
        return KagemushaVerifiedRecipientPaymentRequest(
            request: self,
            digest: digest,
            verifiedAtMilliseconds: atMilliseconds
        )
    }
}

public struct KagemushaVerifiedRecipientPaymentRequest: Equatable, Sendable {
    public let request: KagemushaRecipientPaymentRequest
    public let digest: Data
    public let verifiedAtMilliseconds: UInt64

    init(
        request: KagemushaRecipientPaymentRequest,
        digest: Data,
        verifiedAtMilliseconds: UInt64
    ) {
        self.request = request
        self.digest = Data(digest)
        self.verifiedAtMilliseconds = verifiedAtMilliseconds
    }
}

/// Exact online hardware assertion profile carried by a V2 authorization.
///
/// The discriminants are part of the local preparation ABI and must stay in
/// lockstep with `KagemushaRequestAuthorizationPlatformV2` in the native
/// bridge. There is intentionally no stringly-typed fallback variant.
public enum KagemushaOnlineHardwareAssertionPlatform: UInt32, Equatable, Hashable, Sendable {
    case androidKeyMint = 0
    case iosAppAttest = 1

    public var wireName: String {
        switch self {
        case .androidKeyMint:
            return KagemushaDeviceAttestation.androidKeyMintPlatform
        case .iosAppAttest:
            return KagemushaDeviceAttestation.iosAppAttestPlatform
        }
    }
}

/// Canonical hardware assertion returned by the native authorization finalizer.
/// Platform APIs emit strict ASN.1 DER; the native boundary converts that DER
/// to the unique low-S `r || s` representation stored here and on the wire.
public enum KagemushaOnlineHardwareAssertion: Equatable, Sendable {
    case androidKeyMint(signature: KagemushaDeviceSignatureV2)
    case iosAppAttest(
        authenticatorData: Data,
        signature: KagemushaDeviceSignatureV2
    )

    public var platform: KagemushaOnlineHardwareAssertionPlatform {
        switch self {
        case .androidKeyMint:
            return .androidKeyMint
        case .iosAppAttest:
            return .iosAppAttest
        }
    }

    public var signature: KagemushaDeviceSignatureV2 {
        switch self {
        case let .androidKeyMint(signature):
            return signature
        case let .iosAppAttest(_, signature):
            return signature
        }
    }

    public var authenticatorData: Data? {
        switch self {
        case .androidKeyMint:
            return nil
        case let .iosAppAttest(authenticatorData, _):
            return Data(authenticatorData)
        }
    }
}

/// Unsigned public fields of the self-contained hardware authorization used by
/// online top-up and redemption. Private keys, DER signatures, and App Attest
/// assertion bytes are deliberately absent from this value.
public struct KagemushaRequestAuthorizationFields: Equatable, Sendable {
    public let authority: String
    public let deviceID: String
    public let assetDefinitionID: String
    public let operationID: Data
    public let issuedAtMilliseconds: UInt64
    public let expiresAtMilliseconds: UInt64
    public let nonce: Data
    public let payloadDigest: Data
    /// Canonical Iroha hash of the exact Norito registration admitted on-chain.
    public let registrationHash: Data
    public let platform: KagemushaOnlineHardwareAssertionPlatform

    public init(
        authority: String,
        deviceID: String,
        assetDefinitionID: String,
        operationID: Data,
        issuedAtMilliseconds: UInt64,
        expiresAtMilliseconds: UInt64,
        nonce: Data,
        payloadDigest: Data,
        registrationHash: Data,
        platform: KagemushaOnlineHardwareAssertionPlatform
    ) throws {
        _ = try KagemushaRecursiveSpend.canonicalAccountAddress(
            authority,
            field: "authority"
        )
        try KagemushaRecursiveSpend.requirePortableText(deviceID, field: "deviceID")
        guard AssetDefinitionAddress.decode(assetDefinitionID) != nil else {
            throw KagemushaRecursiveSpendError.invalidField("assetDefinitionID")
        }
        try KagemushaRecursiveSpend.requireNonzeroFixed32(operationID, field: "operationID")
        try KagemushaRecursiveSpend.requireNonzeroFixed32(nonce, field: "nonce")
        try KagemushaRecursiveSpend.requireNonzeroFixed32(payloadDigest, field: "payloadDigest")
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            registrationHash,
            field: "registrationHash"
        )
        guard issuedAtMilliseconds > 0,
              expiresAtMilliseconds > issuedAtMilliseconds,
              expiresAtMilliseconds - issuedAtMilliseconds
                <= KagemushaRecursiveSpend.maximumAuthorizationTTLMilliseconds else {
            throw KagemushaRecursiveSpendError.invalidField("authorization.expiry")
        }
        self.authority = authority
        self.deviceID = deviceID
        self.assetDefinitionID = assetDefinitionID
        self.operationID = Data(operationID)
        self.issuedAtMilliseconds = issuedAtMilliseconds
        self.expiresAtMilliseconds = expiresAtMilliseconds
        self.nonce = Data(nonce)
        self.payloadDigest = Data(payloadDigest)
        self.registrationHash = Data(registrationHash)
        self.platform = platform
    }

    /// Prepare the exact bytes supplied to the selected hardware API.
    ///
    /// Android signs `signingBytes` with KeyMint `SHA256withECDSA`. For iOS,
    /// `signingBytes` is the 32-byte `clientDataHash` passed to
    /// `DCAppAttestService.generateAssertion`.
    public func prepare() throws -> KagemushaRequestAuthorizationPreparation {
        let preparationArchive = try KagemushaRecursiveSpendCodecs
            .encodeAuthorizationPreparation(self)
        guard let signingBytes = try NoritoNativeBridge.shared
            .kagemushaRequestAuthorizationSigningBytesV2(
                preparationArchive: preparationArchive
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRequestAuthorizationPreparation(
            fields: self,
            preparationArchive: preparationArchive,
            signingBytes: signingBytes
        )
    }
}

/// Opaque native-checked unsigned authorization retained across one hardware
/// signing call. It is not a `KagemushaRequestAuthorizationV2` archive and can
/// never be submitted as a signed authorization.
public struct KagemushaRequestAuthorizationPreparation: Equatable, Sendable {
    public let fields: KagemushaRequestAuthorizationFields
    public let signingBytes: Data
    let preparationArchive: Data

    init(
        fields: KagemushaRequestAuthorizationFields,
        preparationArchive: Data,
        signingBytes: Data
    ) throws {
        try KagemushaRecursiveSpend.requireArchive(
            preparationArchive,
            schema: KagemushaRecursiveSpend.authorizationPreparationWireName,
            field: "authorizationPreparation"
        )
        guard !signingBytes.isEmpty,
              signingBytes.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV2,
              fields.platform != .iosAppAttest || signingBytes.count == 32 else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "authorizationPreparation.signingBytes"
            )
        }
        self.fields = fields
        self.preparationArchive = Data(preparationArchive)
        self.signingBytes = Data(signingBytes)
    }

    /// Finalize an Android KeyMint `SHA256withECDSA` result.
    public func finalizeAndroidKeyMint(
        derSignature: Data
    ) throws -> KagemushaRequestAuthorization {
        guard fields.platform == .androidKeyMint else {
            throw KagemushaRecursiveSpendError.invalidField("authorization.platform")
        }
        return try finalize(authenticatorData: Data(), derSignature: derSignature)
    }

    /// Finalize the exact CBOR object returned by
    /// `DCAppAttestService.generateAssertion`.
    public func finalizeIosAppAttest(
        assertionObject: Data
    ) throws -> KagemushaRequestAuthorization {
        guard fields.platform == .iosAppAttest else {
            throw KagemushaRecursiveSpendError.invalidField("authorization.platform")
        }
        guard !assertionObject.isEmpty,
              assertionObject.count
                <= KagemushaRecursiveSpend.maximumIosAppAttestAssertionObjectBytesV2 else {
            throw KagemushaRecursiveSpendError.invalidField(
                "authorization.assertionObject"
            )
        }
        guard let result = try NoritoNativeBridge.shared
            .kagemushaRequestAuthorizationFinalizeIosAppAttestV2(
                preparationArchive: preparationArchive,
                assertionObject: assertionObject
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        try Self.validateIosAuthenticatorData(result.authenticatorData)
        let nativeSignature = try KagemushaDeviceSignatureV2(
            rawBytes: result.rawSignature
        )
        return try KagemushaRequestAuthorization(
            fields: fields,
            hardwareAssertion: .iosAppAttest(
                authenticatorData: result.authenticatorData,
                signature: nativeSignature
            ),
            archive: result.authorizationArchive
        )
    }

    #if canImport(DeviceCheck)
    /// Ask the physical App Attest service to sign this exact authorization
    /// preparation, then finalize its returned CBOR assertion without allowing
    /// the caller to substitute a different client-data hash.
    @available(iOS 15.0, macOS 12.0, *)
    public func authorizeWithIosAppAttest(
        keyId: String,
        service: DCAppAttestService = .shared
    ) async throws -> KagemushaRequestAuthorization {
        guard fields.platform == .iosAppAttest,
              signingBytes.count == 32 else {
            throw KagemushaRecursiveSpendError.invalidField(
                "authorization.platform"
            )
        }
        guard let decodedKeyId = Data(base64Encoded: keyId),
              !decodedKeyId.isEmpty,
              decodedKeyId.base64EncodedString() == keyId else {
            throw KagemushaRecursiveSpendError.invalidField(
                "authorization.appAttest.keyId"
            )
        }
        guard service.isSupported else {
            throw KagemushaRecursiveSpendError.hardwareAssertionUnavailable
        }
        let assertionObject: Data = try await withCheckedThrowingContinuation {
            continuation in
            service.generateAssertion(
                keyId,
                clientDataHash: signingBytes
            ) { assertionObject, error in
                if let error {
                    continuation.resume(throwing: error)
                } else if let assertionObject {
                    continuation.resume(returning: assertionObject)
                } else {
                    continuation.resume(
                        throwing: KagemushaRecursiveSpendError
                            .hardwareAssertionUnavailable
                    )
                }
            }
        }
        return try finalizeIosAppAttest(assertionObject: assertionObject)
    }
    #endif

    /// Advanced finalization from already separated App Attest fields.
    /// Prefer the assertion-object overload for normal `generateAssertion` use.
    public func finalizeIosAppAttest(
        authenticatorData: Data,
        derSignature: Data
    ) throws -> KagemushaRequestAuthorization {
        guard fields.platform == .iosAppAttest else {
            throw KagemushaRecursiveSpendError.invalidField("authorization.platform")
        }
        try Self.validateIosAuthenticatorData(authenticatorData)
        return try finalize(
            authenticatorData: authenticatorData,
            derSignature: derSignature
        )
    }

    private func finalize(
        authenticatorData: Data,
        derSignature: Data
    ) throws -> KagemushaRequestAuthorization {
        // Independently normalize DER in Swift, then require the native result
        // to agree byte-for-byte. This catches ABI mix-ups without making Swift
        // the authority that constructs the on-wire assertion.
        let swiftSignature = try KagemushaDeviceSignatureV2(derBytes: derSignature)
        guard let result = try NoritoNativeBridge.shared
            .kagemushaRequestAuthorizationFinalizeHardwareV2(
                preparationArchive: preparationArchive,
                authenticatorData: authenticatorData,
                derSignature: derSignature
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        let nativeSignature = try KagemushaDeviceSignatureV2(
            rawBytes: result.rawSignature
        )
        guard nativeSignature == swiftSignature else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "authorization.signature.normalization"
            )
        }
        let assertion: KagemushaOnlineHardwareAssertion
        switch fields.platform {
        case .androidKeyMint:
            guard authenticatorData.isEmpty else {
                throw KagemushaRecursiveSpendError.invalidField(
                    "authorization.authenticatorData"
                )
            }
            assertion = .androidKeyMint(signature: nativeSignature)
        case .iosAppAttest:
            try Self.validateIosAuthenticatorData(authenticatorData)
            assertion = .iosAppAttest(
                authenticatorData: Data(authenticatorData),
                signature: nativeSignature
            )
        }
        return try KagemushaRequestAuthorization(
            fields: fields,
            hardwareAssertion: assertion,
            archive: result.authorizationArchive
        )
    }

    private static func validateIosAuthenticatorData(_ authenticatorData: Data) throws {
        let minimumLength =
            KagemushaRecursiveSpend.minimumIosAppAttestAuthenticatorDataBytesV2
        let maximumLength =
            KagemushaRecursiveSpend.maximumIosAppAttestAuthenticatorDataBytesV2
        guard (minimumLength...maximumLength).contains(authenticatorData.count) else {
            throw KagemushaRecursiveSpendError.invalidField(
                "authorization.authenticatorData"
            )
        }
        let flags = authenticatorData[authenticatorData.startIndex + 32]
        let extensionDataFlag: UInt8 = 0x80
        guard flags == extensionDataFlag else {
            throw KagemushaRecursiveSpendError.invalidField(
                "authorization.authenticatorData.flags"
            )
        }
    }
}

public struct KagemushaRequestAuthorization: Equatable, Sendable {
    public let fields: KagemushaRequestAuthorizationFields
    public let hardwareAssertion: KagemushaOnlineHardwareAssertion
    public let archive: Data

    /// Canonical raw low-S signature retained for source compatibility.
    public var signature: Data {
        hardwareAssertion.signature.rawBytes
    }

    init(
        fields: KagemushaRequestAuthorizationFields,
        hardwareAssertion: KagemushaOnlineHardwareAssertion,
        archive: Data
    ) throws {
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.authorizationWireName,
            field: "authorization"
        )
        guard fields.platform == hardwareAssertion.platform,
              archive.count <= KagemushaRecursiveSpend.maximumPeerArchiveBytesV2 else {
            throw KagemushaRecursiveSpendError.invalidArchive("authorization.binding")
        }
        self.fields = fields
        self.hardwareAssertion = hardwareAssertion
        self.archive = Data(archive)
    }
}

/// Local-only proof request for a zero-input public-to-confidential top-up.
///
/// The request must be built from Torii's verified `next_zero_path` snapshot.
/// It contains note secrets and therefore must never be persisted, logged, or
/// submitted to Torii; native code returns only the canonical unsigned top-up.
public struct KagemushaTopUpShieldEvidence: Equatable, Sendable {
    public let initialRoot: Data
    public let finalizedRoot: Data
    public let leafIndex: UInt32
    public let proofAttachment: Data

    public init(
        initialRoot: Data,
        finalizedRoot: Data,
        leafIndex: UInt32,
        proofAttachment: Data
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(initialRoot, field: "initialRoot")
        try KagemushaRecursiveSpend.requireNonzeroFixed32(finalizedRoot, field: "finalizedRoot")
        guard initialRoot != finalizedRoot, leafIndex < (1 << 16) else {
            throw KagemushaRecursiveSpendError.invalidField("shieldEvidence")
        }
        try KagemushaRecursiveSpend.requireArchive(
            proofAttachment,
            schema: KagemushaRecursiveSpend.proofAttachmentWireName,
            field: "shieldEvidence.proofAttachment"
        )
        self.initialRoot = Data(initialRoot)
        self.finalizedRoot = Data(finalizedRoot)
        self.leafIndex = leafIndex
        self.proofAttachment = Data(proofAttachment)
    }
}

public struct KagemushaTopUpFinalityProofArchive: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count
                <= KagemushaRecursiveSpend.topUpFinalityProofMaximumArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive("topUpFinalityProof")
        }
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.topUpFinalityProofWireName,
            field: "topUpFinalityProof"
        )
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Content-addressed validator-roster trust artifact prefetched while online.
///
/// The artifact is opaque to application code. Native verification validates
/// its exact authenticated SHA-256, network id, activation windows, ordered BLS
/// keys, PoPs, and generation.
public struct KagemushaTopUpFinalityRosterArtifactArchive: Equatable, Sendable {
    public let noritoArchive: Data

    public init(noritoArchive: Data) throws {
        guard !noritoArchive.isEmpty,
              noritoArchive.count
                <= KagemushaRecursiveSpend.topUpFinalityRosterMaximumArchiveBytes else {
            throw KagemushaRecursiveSpendError.invalidArchive(
                "topUpFinalityRosterArtifact"
            )
        }
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.topUpFinalityRosterArtifactWireName,
            field: "topUpFinalityRosterArtifact"
        )
        self.noritoArchive = Data(noritoArchive)
    }
}

/// Canonical authenticated V4 release manifest passed opaquely to the native
/// artifact loader. Application code never derives proof parameters from it.
public struct KagemushaRecursiveSpendArtifactManifestArchive: Equatable, Sendable {
    public let noritoArchive: Data
    public let sha256: Data

    public init(noritoArchive: Data, expectedSHA256: Data) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            expectedSHA256,
            field: "artifactManifest.sha256"
        )
        try KagemushaRecursiveSpend.requireArchive(
            noritoArchive,
            schema: KagemushaRecursiveSpend.artifactManifestWireName,
            field: "artifactManifest"
        )
        guard Data(SHA256.hash(data: noritoArchive)) == expectedSHA256 else {
            throw KagemushaRecursiveSpendError.invalidField("artifactManifest.sha256")
        }
        self.noritoArchive = Data(noritoArchive)
        self.sha256 = Data(expectedSHA256)
    }
}

/// Installed ABI-21/V4 generation bound to its authenticated manifest.
public struct KagemushaRecursiveSpendInstalledArtifactSetV4: Equatable, Sendable {
    public let binding: KagemushaRecursiveSpendArtifactBindingV4
    public let manifest: KagemushaRecursiveSpendArtifactManifestArchive

    init(
        binding: KagemushaRecursiveSpendArtifactBindingV4,
        manifest: KagemushaRecursiveSpendArtifactManifestArchive
    ) throws {
        guard binding.manifestSHA256 == manifest.sha256 else {
            throw KagemushaRecursiveSpendError.invalidField("artifactBinding.manifestSHA256")
        }
        self.binding = binding
        self.manifest = manifest
    }

    func requireInstalled() throws {
        guard let installed = try NoritoNativeBridge.shared
            .kagemushaRecursiveSpendArtifactSetIsInstalledV4(
                manifestArchive: manifest.noritoArchive,
                expectedManifestSHA256: binding.manifestSHA256
            ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        guard installed else {
            throw KagemushaRecursiveSpendError.proofBackendUnavailable
        }
    }
}

/// Receiver acknowledgement shared unchanged by the V4 peer-payment flow.
public struct KagemushaReceiverAcknowledgementPayload: Equatable, Sendable {
    public let operationID: Data
    public let recipientRequestDigest: Data
    public let paymentBundleDigest: Data
    public let recipientCommitment: Data
    public let acceptedAtMilliseconds: UInt64
    public let receiverDeviceID: String
    public let receiverKeyReference: Data
    public let receiverPublicKey: KagemushaDevicePublicKeyV2
    public let archive: Data

    init(
        operationID: Data,
        recipientRequestDigest: Data,
        paymentBundleDigest: Data,
        recipientCommitment: Data,
        acceptedAtMilliseconds: UInt64,
        receiverDeviceID: String,
        receiverKeyReference: Data,
        receiverPublicKey: KagemushaDevicePublicKeyV2,
        archive: Data
    ) throws {
        for (field, value) in [
            ("operationID", operationID),
            ("recipientRequestDigest", recipientRequestDigest),
            ("paymentBundleDigest", paymentBundleDigest),
            ("recipientCommitment", recipientCommitment),
            ("receiverKeyReference", receiverKeyReference),
        ] {
            try KagemushaRecursiveSpend.requireNonzeroFixed32(value, field: field)
        }
        guard acceptedAtMilliseconds > 0 else {
            throw KagemushaRecursiveSpendError.invalidField("acceptedAtMilliseconds")
        }
        try KagemushaRecursiveSpend.requirePortableText(
            receiverDeviceID,
            field: "receiverDeviceID"
        )
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.acknowledgementPayloadWireName,
            field: "acknowledgementPayload"
        )
        self.operationID = Data(operationID)
        self.recipientRequestDigest = Data(recipientRequestDigest)
        self.paymentBundleDigest = Data(paymentBundleDigest)
        self.recipientCommitment = Data(recipientCommitment)
        self.acceptedAtMilliseconds = acceptedAtMilliseconds
        self.receiverDeviceID = receiverDeviceID
        self.receiverKeyReference = Data(receiverKeyReference)
        self.receiverPublicKey = receiverPublicKey
        self.archive = Data(archive)
    }

    public func signingBytes() throws -> Data {
        guard let bytes = try NoritoNativeBridge.shared
            .kagemushaReceiverAcknowledgementSigningBytesV2(payloadArchive: archive) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return bytes
    }
}

public struct KagemushaReceiverAcknowledgement: Equatable, Sendable {
    public let payload: KagemushaReceiverAcknowledgementPayload
    public let signature: KagemushaDeviceSignatureV2
    public let archive: Data

    public static func prepare(
        request: KagemushaRecipientPaymentRequest,
        payment: KagemushaRecursiveSpendPeerPaymentV4,
        acceptedAtMilliseconds: UInt64
    ) throws -> KagemushaReceiverAcknowledgementPayload {
        guard let archive = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementPayloadV2(
            requestArchive: request.archive,
            peerPaymentArchive: payment.archive,
            acceptedAtMilliseconds: acceptedAtMilliseconds
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendCodecs.decodeAcknowledgementPayload(archive)
    }

    public static func create(
        payload: KagemushaReceiverAcknowledgementPayload,
        signature: KagemushaDeviceSignatureV2,
        request: KagemushaRecipientPaymentRequest,
        payment: KagemushaRecursiveSpendPeerPaymentV4
    ) throws -> Self {
        guard let archive = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementCreateV2(
            payloadArchive: payload.archive,
            signature: signature.rawBytes,
            requestArchive: request.archive,
            peerPaymentArchive: payment.archive
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try Self(payload: payload, signature: signature, archive: archive)
    }

    init(
        payload: KagemushaReceiverAcknowledgementPayload,
        signature: KagemushaDeviceSignatureV2,
        archive: Data
    ) throws {
        try KagemushaRecursiveSpend.requireArchive(
            archive,
            schema: KagemushaRecursiveSpend.acknowledgementWireName,
            field: "acknowledgement"
        )
        self.payload = payload
        self.signature = signature
        self.archive = Data(archive)
    }

    /// Verify a receiver-signed delivery receipt.
    ///
    /// Under `cash_handoff_v1` this is evidence only. The sender has already
    /// irreversibly consumed its inputs and committed the exact payment before
    /// transport handoff; failure, absence, or rejection of this receipt must
    /// never unspend, roll back, replace, or claw back that payment.
    public func verifiedForSender(
        request: KagemushaRecipientPaymentRequest,
        payment: KagemushaRecursiveSpendPeerPaymentV4
    ) throws -> KagemushaReceiverAcknowledgementVerifyResult {
        guard let result = try NoritoNativeBridge.shared.kagemushaReceiverAcknowledgementVerifyV2(
            acknowledgementArchive: archive,
            requestArchive: request.archive,
            peerPaymentArchive: payment.archive
        ) else {
            throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
        }
        return try KagemushaRecursiveSpendCodecs.decodeAcknowledgementVerifyResult(result)
    }
}

public struct KagemushaReceiverAcknowledgementVerifyResult: Equatable, Sendable {
    public let valid: Bool
    public let operationID: Data
    public let recipientRequestDigest: Data
    public let paymentBundleDigest: Data
    public let acknowledgementDigest: Data
}

public enum KagemushaRecursiveSpendArtifactRoleV4: Int, CaseIterable, Sendable {
    case stepEqParamsIpa
    case stepEqProvingKey
    case stepEqVerifyingKey
    case stepEqBootstrapWitness
    case stepEpParamsIpa
    case stepEpProvingKey
    case stepEpVerifyingKey
    case stepEpBootstrapWitness

    public var fileName: String {
        KagemushaRecursiveSpend.artifactFileNamesV4[rawValue]
    }
}

public final class KagemushaRecursiveSpendArtifactIngest: @unchecked Sendable {
    public let role: KagemushaRecursiveSpendArtifactRoleV4
    public let manifest: KagemushaRecursiveSpendArtifactManifestArchive
    public let artifactSHA256: Data
    private var handle: UInt64?
    private var finalized = false
    private let lock = NSLock()

    public init(
        role: KagemushaRecursiveSpendArtifactRoleV4,
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        expectedArtifactSHA256: Data
    ) throws {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            expectedArtifactSHA256,
            field: "artifact.sha256"
        )
        let handle = try KagemushaRecursiveSpendWorkerPermit.withPermit {
            guard let handle = try NoritoNativeBridge.shared
                .kagemushaRecursiveSpendArtifactBeginV4(
                    manifestArchive: manifest.noritoArchive,
                    expectedManifestSHA256: manifest.sha256,
                    expectedArtifactSHA256: expectedArtifactSHA256
                ) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            return handle
        }
        self.role = role
        self.manifest = manifest
        self.artifactSHA256 = Data(expectedArtifactSHA256)
        self.handle = handle
    }

    deinit {
        lock.lock()
        let active = handle
        handle = nil
        lock.unlock()
        if let active {
            _ = try? NoritoNativeBridge.shared
                .kagemushaRecursiveSpendArtifactCancelV4(handle: active)
        }
    }

    public func write(_ chunk: Data) throws {
        guard !chunk.isEmpty,
              chunk.count <= KagemushaRecursiveSpend.artifactMaximumChunkBytes else {
            throw KagemushaRecursiveSpendError.invalidField("artifact.chunk")
        }
        try KagemushaRecursiveSpendWorkerPermit.withPermit {
            lock.lock()
            guard let active = handle else {
                lock.unlock()
                throw KagemushaRecursiveSpendError.invalidField("artifact.handle")
            }
            guard !finalized else {
                lock.unlock()
                throw KagemushaRecursiveSpendError.invalidField("artifact.finalized")
            }
            lock.unlock()
            do {
                guard try NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactWriteV4(
                    handle: active,
                    chunk: chunk
                ) else {
                    throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
                }
            } catch {
                _ = try? NoritoNativeBridge.shared
                    .kagemushaRecursiveSpendArtifactCancelV4(handle: active)
                lock.lock()
                if handle == active { handle = nil }
                lock.unlock()
                throw error
            }
        }
    }

    public func finalize() throws {
        try KagemushaRecursiveSpendWorkerPermit.withPermit {
            lock.lock()
            guard let active = handle else {
                lock.unlock()
                throw KagemushaRecursiveSpendError.invalidField("artifact.handle")
            }
            guard !finalized else {
                lock.unlock()
                throw KagemushaRecursiveSpendError.invalidField("artifact.finalized")
            }
            lock.unlock()
            do {
                guard try NoritoNativeBridge.shared
                    .kagemushaRecursiveSpendArtifactFinalizeV4(handle: active) else {
                    throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
                }
                lock.lock()
                if handle == active { finalized = true }
                lock.unlock()
            } catch NativeBridgeError.kagemushaBusy {
                // Native leaves a busy spool intact. Preserve the handle and
                // unfinished state so this exact finalization can be retried.
                throw KagemushaRecursiveSpendError.proofWorkerBusy
            } catch {
                // Native finalization removes a corrupt spool. Clear the Swift
                // owner as well so cancellation remains idempotent.
                _ = try? NoritoNativeBridge.shared
                    .kagemushaRecursiveSpendArtifactCancelV4(handle: active)
                lock.lock()
                if handle == active { handle = nil }
                lock.unlock()
                throw error
            }
        }
    }

    public func cancel() throws {
        try KagemushaRecursiveSpendWorkerPermit.withPermit {
            lock.lock()
            let active = handle
            lock.unlock()
            guard let active else { return }
            guard try NoritoNativeBridge.shared
                .kagemushaRecursiveSpendArtifactCancelV4(handle: active) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            lock.lock()
            if handle == active {
                handle = nil
                finalized = false
            }
            lock.unlock()
        }
    }

    fileprivate func finalizedHandle(
        for expectedManifest: KagemushaRecursiveSpendArtifactManifestArchive
    ) throws -> UInt64 {
        lock.lock()
        defer { lock.unlock() }
        guard manifest == expectedManifest else {
            throw KagemushaRecursiveSpendError.invalidField("artifact.manifest")
        }
        guard finalized, let handle else {
            throw KagemushaRecursiveSpendError.invalidField("artifact.finalized")
        }
        return handle
    }

    fileprivate func relinquishInstalledHandle(_ expectedHandle: UInt64) {
        lock.lock()
        defer { lock.unlock() }
        if handle == expectedHandle, finalized {
            handle = nil
            finalized = false
        }
    }
}

/// Local trust material required to authenticate one V4 proving release.
///
/// The policy must be provisioned from the application/deployment trust root;
/// it must never be accepted from the same downloaded bundle as the manifest.
public struct KagemushaRecursiveSpendReleaseAuthenticationV4: Sendable {
    public let trustedPolicyNorito: Data
    public let releaseAttestationNorito: Data
    public let benchmarkEvidence: Data
    public let cryptographicReview: Data
    public let promotionRecordNorito: Data

    public init(
        trustedPolicyNorito: Data,
        releaseAttestationNorito: Data,
        benchmarkEvidence: Data,
        cryptographicReview: Data,
        promotionRecordNorito: Data
    ) throws {
        guard !trustedPolicyNorito.isEmpty, trustedPolicyNorito.count <= 64 * 1_024 else {
            throw KagemushaRecursiveSpendError.invalidField("release.trustedPolicy")
        }
        guard !releaseAttestationNorito.isEmpty,
              releaseAttestationNorito.count <= 1_024 * 1_024 else {
            throw KagemushaRecursiveSpendError.invalidField("release.attestation")
        }
        for (field, evidence) in [
            ("release.benchmarkEvidence", benchmarkEvidence),
            ("release.cryptographicReview", cryptographicReview),
        ] {
            guard !evidence.isEmpty, evidence.count <= 16 * 1_024 * 1_024 else {
                throw KagemushaRecursiveSpendError.invalidField(field)
            }
        }
        guard !promotionRecordNorito.isEmpty,
              promotionRecordNorito.count
                <= KagemushaRecursiveSpend.maximumPromotionRecordBytesV4 else {
            throw KagemushaRecursiveSpendError.invalidField("release.promotionRecord")
        }
        self.trustedPolicyNorito = Data(trustedPolicyNorito)
        self.releaseAttestationNorito = Data(releaseAttestationNorito)
        self.benchmarkEvidence = Data(benchmarkEvidence)
        self.cryptographicReview = Data(cryptographicReview)
        self.promotionRecordNorito = Data(promotionRecordNorito)
    }
}

/// Coordinates one complete content-addressed V4 release installation.
///
/// Each artifact is still streamed independently, but `install()` is the only
/// operation that transfers ownership to the prover. Native resolves the
/// required files from the manifest and either consumes the complete set or none.
public final class KagemushaRecursiveSpendArtifactInstallSessionV4: @unchecked Sendable {
    public let manifest: KagemushaRecursiveSpendArtifactManifestArchive
    public let binding: KagemushaRecursiveSpendArtifactBindingV4
    public let authentication: KagemushaRecursiveSpendReleaseAuthenticationV4
    private var artifacts: [KagemushaRecursiveSpendArtifactRoleV4:
        KagemushaRecursiveSpendArtifactIngest] = [:]
    private var installed = false
    private var closed = false
    private let lock = NSLock()

    public init(
        manifest: KagemushaRecursiveSpendArtifactManifestArchive,
        binding: KagemushaRecursiveSpendArtifactBindingV4,
        authentication: KagemushaRecursiveSpendReleaseAuthenticationV4
    ) throws {
        guard binding.manifestSHA256 == manifest.sha256 else {
            throw KagemushaRecursiveSpendError.invalidField("artifactBinding.manifestSHA256")
        }
        self.manifest = manifest
        self.binding = binding
        self.authentication = authentication
    }

    deinit {
        lock.lock()
        let pending = installed ? [] : Array(artifacts.values)
        artifacts.removeAll()
        closed = true
        lock.unlock()
        for artifact in pending {
            try? artifact.cancel()
        }
    }

    /// Start one manifest-selected artifact stream. Native begin rejects a
    /// digest not present exactly once in the canonical manifest.
    public func beginArtifact(
        role: KagemushaRecursiveSpendArtifactRoleV4,
        expectedArtifactSHA256: Data
    ) throws -> KagemushaRecursiveSpendArtifactIngest {
        try KagemushaRecursiveSpend.requireNonzeroFixed32(
            expectedArtifactSHA256,
            field: "artifact.sha256"
        )
        return try KagemushaRecursiveSpendWorkerPermit.withPermit {
            lock.lock()
            guard !closed, !installed else {
                lock.unlock()
                throw KagemushaRecursiveSpendError.invalidField("artifactSet.state")
            }
            guard artifacts.count < KagemushaRecursiveSpendArtifactRoleV4.allCases.count,
                  role == KagemushaRecursiveSpendArtifactRoleV4.allCases[artifacts.count] else {
                lock.unlock()
                throw KagemushaRecursiveSpendError.invalidField("artifactSet.roleOrder")
            }
            guard artifacts[role] == nil,
                  artifacts.values.allSatisfy({
                      $0.artifactSHA256 != expectedArtifactSHA256
                  }) else {
                lock.unlock()
                throw KagemushaRecursiveSpendError.invalidField(
                    "artifactSet.duplicateRoleOrDigest"
                )
            }
            lock.unlock()
            let artifact = try KagemushaRecursiveSpendArtifactIngest(
                role: role,
                manifest: manifest,
                expectedArtifactSHA256: expectedArtifactSHA256
            )
            lock.lock()
            artifacts[role] = artifact
            lock.unlock()
            return artifact
        }
    }

    /// Atomically transfer the complete manifest-selected file set into native.
    /// Native, not the wallet, resolves each circuit/key purpose.
    @discardableResult
    public func install() throws -> KagemushaRecursiveSpendInstalledArtifactSetV4 {
        return try KagemushaRecursiveSpendWorkerPermit.withPermit {
            lock.lock()
            guard !closed, !installed,
                  artifacts.count == KagemushaRecursiveSpendArtifactRoleV4.allCases.count else {
                lock.unlock()
                throw KagemushaRecursiveSpendError.invalidField("artifactSet.count")
            }
            let orderedArtifacts = try KagemushaRecursiveSpendArtifactRoleV4.allCases.map {
                role in
                guard let artifact = artifacts[role] else {
                    lock.unlock()
                    throw KagemushaRecursiveSpendError.invalidField("artifactSet.roleOrder")
                }
                return artifact
            }
            lock.unlock()
            let handles = try orderedArtifacts.map { try $0.finalizedHandle(for: manifest) }
            do {
                guard try NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactSetInstallV4(
                    manifestArchive: manifest.noritoArchive,
                    expectedManifestSHA256: manifest.sha256,
                    trustedPolicyArchive: authentication.trustedPolicyNorito,
                    releaseAttestationArchive: authentication.releaseAttestationNorito,
                    benchmarkEvidence: authentication.benchmarkEvidence,
                    cryptographicReview: authentication.cryptographicReview,
                    promotionRecordArchive: authentication.promotionRecordNorito,
                    handles: handles
                ) else {
                    throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
                }
            } catch NativeBridgeError.kagemushaBusy {
                // Native does not consume any handle on a busy install. Keep the
                // complete finalized set owned by this session for a later retry.
                throw KagemushaRecursiveSpendError.proofWorkerBusy
            }
            for (artifact, handle) in zip(orderedArtifacts, handles) {
                artifact.relinquishInstalledHandle(handle)
            }
            lock.lock()
            artifacts.removeAll()
            installed = true
            lock.unlock()
            return try KagemushaRecursiveSpendInstalledArtifactSetV4(
                binding: binding,
                manifest: manifest
            )
        }
    }

    public func isInstalled() throws -> Bool {
        try KagemushaRecursiveSpendWorkerPermit.withPermit {
            guard let result = try NoritoNativeBridge.shared
                .kagemushaRecursiveSpendArtifactSetIsInstalledV4(
                    manifestArchive: manifest.noritoArchive,
                    expectedManifestSHA256: manifest.sha256
                ) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            return result
        }
    }

    public func installedArtifactSet() throws -> KagemushaRecursiveSpendInstalledArtifactSetV4 {
        guard try isInstalled() else {
            throw KagemushaRecursiveSpendError.proofBackendUnavailable
        }
        return try KagemushaRecursiveSpendInstalledArtifactSetV4(
            binding: binding,
            manifest: manifest
        )
    }

    /// Cancel only pending streams. An installed generation remains active
    /// until `uninstall()` is explicitly requested.
    public func cancel() throws {
        try KagemushaRecursiveSpendWorkerPermit.withPermit {
            lock.lock()
            guard !installed else {
                lock.unlock()
                return
            }
            let pending = Array(artifacts.values)
            artifacts.removeAll()
            closed = true
            lock.unlock()
            var firstError: Error?
            for artifact in pending {
                do {
                    try artifact.cancel()
                } catch where firstError == nil {
                    firstError = error
                } catch {}
            }
            if let firstError { throw firstError }
        }
    }

    /// Release this exact installed generation. The native digest guard makes
    /// a stale session incapable of removing a newer generation.
    public func uninstall() throws {
        try KagemushaRecursiveSpendWorkerPermit.withPermit {
            lock.lock()
            let shouldUninstall = !closed
            lock.unlock()
            guard shouldUninstall else { return }
            // The native digest guard is the source of truth. This deliberately
            // supports reconstructing a coordinator after an app-layer owner was
            // lost while the process stayed alive; an explicit uninstall can then
            // release the exact active generation without being able to remove a
            // newer one.
            guard try NoritoNativeBridge.shared.kagemushaRecursiveSpendArtifactSetUninstallV4(
                expectedManifestSHA256: manifest.sha256
            ) else {
                throw KagemushaRecursiveSpendError.nativeBridgeUnavailable
            }
            lock.lock()
            installed = false
            closed = true
            lock.unlock()
        }
    }
}

#if canImport(Darwin)
private typealias KagemushaV4FreeFn = @convention(c) (UnsafeMutablePointer<UInt8>?) -> Void
private typealias KagemushaV4ArchiveOnlyOutFn = @convention(c) (
    UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
    UnsafeMutablePointer<CUnsignedLong>?
) -> Int32
private typealias KagemushaV4ArchiveOutFn = @convention(c) (
    UnsafePointer<UInt8>?,
    CUnsignedLong,
    UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
    UnsafeMutablePointer<CUnsignedLong>?
) -> Int32
private typealias KagemushaV4TwoArchiveTimeOutFn = @convention(c) (
    UnsafePointer<UInt8>?,
    CUnsignedLong,
    UnsafePointer<UInt8>?,
    CUnsignedLong,
    UInt64,
    UnsafeMutablePointer<UnsafeMutablePointer<UInt8>?>?,
    UnsafeMutablePointer<CUnsignedLong>?
) -> Int32
#endif

/// ABI-21 lifecycle calls resolve explicitly suffixed V4 symbols without a
/// legacy recursive-lifecycle fallback.
extension NoritoNativeBridge {
    func kagemushaRecursiveSpendCapabilitiesV4() throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_capabilities_v4",
            as: KagemushaV4ArchiveOnlyOutFn.self
        ) else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = function(&output, &outputLength)
        return try copyKagemushaV4Output(
            status: status,
            pointer: output,
            length: outputLength
        )
        #else
        return nil
        #endif
    }

    func kagemushaRecursiveSpendInitV4(requestArchive: Data) throws -> Data? {
        try callKagemushaV4Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_init_v4",
            archive: requestArchive
        )
    }

    func kagemushaRecursiveSpendVerifyV4(requestArchive: Data) throws -> Data? {
        try callKagemushaV4Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_verify_v4",
            archive: requestArchive
        )
    }

    func kagemushaRecursiveSpendRedeemV4(requestArchive: Data) throws -> Data? {
        try callKagemushaV4Archive(
            symbol: "connect_norito_kagemusha_recursive_spend_redeem_v4",
            archive: requestArchive
        )
    }

    func kagemushaRecursiveSpendAppendV4(
        requestArchive: Data,
        recipientRequestArchive: Data,
        verifiedAtMilliseconds: UInt64
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            "connect_norito_kagemusha_recursive_spend_append_v4",
            as: KagemushaV4TwoArchiveTimeOutFn.self
        ) else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = requestArchive.withUnsafeBytes { requestBuffer in
            recipientRequestArchive.withUnsafeBytes { recipientBuffer in
                function(
                    requestBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(requestBuffer.count),
                    recipientBuffer.bindMemory(to: UInt8.self).baseAddress,
                    CUnsignedLong(recipientBuffer.count),
                    verifiedAtMilliseconds,
                    &output,
                    &outputLength
                )
            }
        }
        return try copyKagemushaV4Output(
            status: status,
            pointer: output,
            length: outputLength
        )
        #else
        return nil
        #endif
    }

    private func callKagemushaV4Archive(
        symbol: String,
        archive: Data
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let function = resolveKagemushaV2Symbol(
            symbol,
            as: KagemushaV4ArchiveOutFn.self
        ) else { return nil }
        var output: UnsafeMutablePointer<UInt8>?
        var outputLength: CUnsignedLong = 0
        let status = archive.withUnsafeBytes { buffer in
            function(
                buffer.bindMemory(to: UInt8.self).baseAddress,
                CUnsignedLong(buffer.count),
                &output,
                &outputLength
            )
        }
        return try copyKagemushaV4Output(
            status: status,
            pointer: output,
            length: outputLength
        )
        #else
        return nil
        #endif
    }

    private func copyKagemushaV4Output(
        status: Int32,
        pointer: UnsafeMutablePointer<UInt8>?,
        length: CUnsignedLong
    ) throws -> Data? {
        #if canImport(Darwin)
        guard let freeFunction = resolveKagemushaV2Symbol(
            "connect_norito_free",
            as: KagemushaV4FreeFn.self
        ) else { return nil }
        if let error = NativeBridgeError.fromStatus(status) {
            if let pointer { freeFunction(pointer) }
            throw error
        }
        return try Self.copyKagemushaNativeArchiveOutput(
            pointer: pointer,
            length: length,
            free: freeFunction
        )
        #else
        _ = status
        _ = pointer
        _ = length
        return nil
        #endif
    }
}
