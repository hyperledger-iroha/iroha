package org.hyperledger.iroha.sdk.offline

import java.net.URI
import java.nio.charset.StandardCharsets
import java.time.Duration
import java.util.ArrayList
import java.util.Collections
import java.util.concurrent.CompletableFuture
import java.util.concurrent.locks.ReentrantLock
import org.bouncycastle.crypto.digests.Blake2bDigest
import org.hyperledger.iroha.sdk.client.CanonicalRequestSigner
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.JsonParser
import org.hyperledger.iroha.sdk.client.LocalSigningContext
import org.hyperledger.iroha.sdk.client.ToriiCanonicalRequestAuth
import org.hyperledger.iroha.sdk.client.ZkMerklePathEntry
import org.hyperledger.iroha.sdk.client.ZkMerklePathResponse
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

/**
 * Native bridge ABI 23 for Kagemusha ABI-21/V4 artifact streaming and capabilities.
 *
 * This is the sole first-release offline-cash surface. It authenticates the opaque eight-file proof
 * artifact set and validates exact typed request/payment/acknowledgement and proof-bound membership
 * archives. Proof execution remains fail-closed while the native backend reports unavailable.
 * Every recursive lifecycle result is projected only through an ABI-21/V4 native decoder.
 */
class KagemushaRecursiveSpendProver private constructor() {
    /** Retryable contention signal raised before a second proof request is copied. */
    class ProofWorkerBusyException internal constructor(
        message: String,
        cause: Throwable? = null,
    ) : IllegalStateException(message, cause)

    /** Canonical ABI-21 artifact roles. Declaration order is part of the native contract. */
    enum class ArtifactRoleV4(val fileName: String) {
        STEP_EQ_PARAMS_IPA("step-eq.params-ipa.krv4"),
        STEP_EQ_PROVING_KEY("step-eq.proving-key.krv4"),
        STEP_EQ_VERIFYING_KEY("step-eq.verifying-key.krv4"),
        STEP_EQ_BOOTSTRAP_WITNESS("step-eq.bootstrap-witness.krv4"),
        STEP_EP_PARAMS_IPA("step-ep.params-ipa.krv4"),
        STEP_EP_PROVING_KEY("step-ep.proving-key.krv4"),
        STEP_EP_VERIFYING_KEY("step-ep.verifying-key.krv4"),
        STEP_EP_BOOTSTRAP_WITNESS("step-ep.bootstrap-witness.krv4"),
    }

    /** Closed first-release hardware assertion profiles for online operations. */
    enum class OnlineHardwareAssertionPlatform(val wireName: String) {
        ANDROID_KEYMINT(DeviceAttestationRegistration.ANDROID_KEYMINT_PLATFORM),
        IOS_APP_ATTEST(DeviceAttestationRegistration.IOS_APP_ATTEST_PLATFORM),
    }

    companion object {
        const val V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 23
        const val REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION
        const val REQUIRED_KAGEMUSHA_NATIVE_CONTRACT_REVISION: Int = 1
        /** Mandatory sender-final peer-cash handoff/finality contract. */
        const val CASH_HANDOFF_CAPABILITY_V1: String = "cash_handoff_v1"
        const val V4_ARTIFACT_MANIFEST_SCHEMA: String =
            "kagemusha.offline.recursive_spend.artifact_manifest.v4"
        const val ARTIFACT_MANIFEST_SCHEMA: String = V4_ARTIFACT_MANIFEST_SCHEMA
        val V4_ARTIFACT_FILES: List<String> = listOf(
            "step-eq.params-ipa.krv4",
            "step-eq.proving-key.krv4",
            "step-eq.verifying-key.krv4",
            "step-eq.bootstrap-witness.krv4",
            "step-ep.params-ipa.krv4",
            "step-ep.proving-key.krv4",
            "step-ep.verifying-key.krv4",
            "step-ep.bootstrap-witness.krv4",
        )
        val ARTIFACT_FILES: List<String> = V4_ARTIFACT_FILES
        const val V4_ARTIFACT_COUNT: Int = 8
        const val ARTIFACT_COUNT: Int = V4_ARTIFACT_COUNT
        const val MAX_MANIFEST_BYTES: Int = 1024 * 1024
        const val MAX_ARTIFACT_CHUNK_BYTES: Int = 1024 * 1024
        const val MAX_TRUSTED_RELEASE_POLICY_BYTES: Int = 64 * 1024
        const val MAX_RELEASE_ATTESTATION_BYTES: Int = 1024 * 1024
        const val MAX_INTERNAL_VALIDATION_RECEIPT_BYTES: Int = 1024 * 1024
        const val MAX_RELEASE_EVIDENCE_BYTES: Int = 16 * 1024 * 1024
        const val MAX_CRYPTOGRAPHIC_REVIEW_BYTES: Int = 1024 * 1024
        const val MAX_PROMOTION_RECORD_BYTES: Int = 1024 * 1024
        const val MAX_PEER_TEXT_ENVELOPE_BYTES: Int = 12 * 1024
        const val MAX_PEER_TEXT_ARCHIVE_BYTES: Int =
            (MAX_PEER_TEXT_ENVELOPE_BYTES - 6) * 3 / 4
        const val MAX_PEER_ARCHIVE_BYTES_V2: Int = 32 * 1024
        const val MAX_RECIPIENT_RECEIVE_OFFER_BYTES_V2: Int = 24_576
        const val MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1: Int = 2 * 1024
        const val PROMOTED_FINALITY_CHECKPOINT_BYTES_V2: Int = 40
        /** Consensus ceiling for one canonical recipient-only ABI-21 peer archive. */
        const val MAX_PEER_ARCHIVE_BYTES_V4: Int = 32 * 1024 * 1024
        const val MAX_PEER_ARCHIVE_BYTES: Int = MAX_PEER_ARCHIVE_BYTES_V4
        /** Consensus-derived ceiling for one canonical ABI-21 top-up provenance archive. */
        const val MAX_TOP_UP_PROVENANCE_ARCHIVE_BYTES_V4: Int = 6_488_064
        /** Largest V4 local verify carrier accepted by native, plus framing headroom. */
        const val MAX_LOCAL_REQUEST_ARCHIVE_BYTES_V4: Int = 64 * 1024 * 1024 + 64
        const val MAX_LOCAL_RESULT_ARCHIVE_BYTES_V4: Int = 64 * 1024 * 1024 + 64
        const val MAX_LOCAL_REQUEST_ARCHIVE_BYTES: Int = MAX_LOCAL_REQUEST_ARCHIVE_BYTES_V4
        const val MAX_LOCAL_RESULT_ARCHIVE_BYTES: Int = MAX_LOCAL_RESULT_ARCHIVE_BYTES_V4
        /** Exact Torii body ceiling for the ABI-21/V4 top-up route. */
        const val MAX_TORII_TOP_UP_REQUEST_BYTES_V4: Int = 512 * 1024

        /** Exact Torii body ceiling for the ABI-21/V4 redemption route. */
        const val MAX_TORII_REDEEM_REQUEST_BYTES_V4: Int = 48 * 1024 * 1024

        private const val MAX_REQUEST_AUTHORIZATION_BYTES: Int = 512 * 1024
        private const val KAGEMUSHA_REQUEST_WIRE_VERSION_V4: Int = 4
        private const val KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2: Long = 300_000
        private val KAGEMUSHA_OPERATION_ID_DOMAIN_V4 =
            "iroha:offline:kagemusha:operation-id:v4\u0000".toByteArray(StandardCharsets.UTF_8)
        private val KAGEMUSHA_OPERATION_AUTHORITY_DIGEST_DOMAIN_V4 =
            "iroha:offline:kagemusha:operation-outcome-authority:v4\u0000"
                .toByteArray(StandardCharsets.UTF_8)
        private val KAGEMUSHA_OPERATION_REQUEST_DIGEST_DOMAIN_V4 =
            "iroha:offline:kagemusha:operation-request:v4\u0000"
                .toByteArray(StandardCharsets.UTF_8)
        private const val IOS_APP_ATTEST_ASSERTION_OBJECT_MAX_BYTES: Int = 8 * 1024
        private const val IOS_APP_ATTEST_AUTHENTICATOR_DATA_FIXED_HEADER_BYTES: Int = 37
        private const val IOS_APP_ATTEST_AUTHENTICATOR_DATA_MIN_BYTES: Int =
            IOS_APP_ATTEST_AUTHENTICATOR_DATA_FIXED_HEADER_BYTES + 1
        private const val IOS_APP_ATTEST_AUTHENTICATOR_DATA_MAX_BYTES: Int = 4 * 1024
        private const val IOS_APP_ATTEST_EXTENSION_DATA_FLAG: Int = 0x80
        /** Exact JSON response ceiling for the offline-readiness route. */
        const val MAX_TORII_READINESS_RESPONSE_BYTES: Int = 4 * 1024
        /** Exact Norito response ceiling for accepted operation references. */
        const val MAX_TORII_OPERATION_REFERENCE_BYTES: Int = 4 * 1024
        /** Exact Norito response ceiling for operation-status resources. */
        const val MAX_TORII_OPERATION_STATUS_BYTES: Int = 4 * 1024 * 1024
        /** Exact Norito response ceiling for recipient-lineage proofs. */
        const val MAX_TORII_RECIPIENT_LINEAGE_RESPONSE_BYTES: Int = 4 * 1024 * 1024
        /** Exact archive ceiling for standalone Torii-sourced proof material. */
        const val MAX_TORII_PROOF_ARCHIVE_BYTES: Int = 4 * 1024 * 1024
        const val MAXIMUM_INPUTS_PER_TRANSITION: Int = 2
        const val MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS: Int = MAXIMUM_INPUTS_PER_TRANSITION
        const val MAXIMUM_BRANCH_CLAIMS: Int = 2
        const val MAXIMUM_PEER_HOPS: Int = 8
        const val MAXIMUM_PROOF_STEPS: Int = 128
        /** Exact recursive proof-pair maximum selected by the sole first-release profile. */
        const val RELEASE_MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4: Int = 191_862

        /** Defensive raw archive ceiling; this is not an accepted release-profile maximum. */
        const val ABSOLUTE_MAXIMUM_RECURSIVE_PROOF_PAIR_BYTES_V4: Int = 384 * 1024
        const val CONFIDENTIAL_TREE_DEPTH: Int = 16
        /**
         * Exclusive top-up insertion capacity. The tail reserves 64 branch-depth outputs, eight
         * optional peer-change outputs, and the final dummy leaf required by the proof circuit.
         */
        const val TOP_UP_SHIELD_INSERTION_CAPACITY: Int = 65_463
        const val MAX_OUTPUT_MEMBERSHIP_FRONTIER_ARCHIVE_BYTES_V4: Int = 4 * 1024
        const val MAX_OUTPUT_MEMBERSHIP_PATHS_ARCHIVE_BYTES_V4: Int = 16 * 1024

        private const val EXACT_STATE_PROJECTION_VERSION: Int = 1

        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val artifactBridgeAvailable = loadArtifactBridge()
        private val heavyProofPermit = ReentrantLock()
        private const val NATIVE_BUSY_MESSAGE = " is busy; retry after the active proof completes"

        private inline fun <T> withHeavyProofPermit(label: String, action: () -> T): T {
            if (!heavyProofPermit.tryLock()) {
                throw ProofWorkerBusyException(
                    "Kagemusha $label is busy; retry after the active proof completes",
                )
            }
            try {
                return action()
            } catch (failure: ProofWorkerBusyException) {
                throw failure
            } catch (failure: IllegalStateException) {
                if (failure.message.orEmpty().contains(NATIVE_BUSY_MESSAGE)) {
                    throw ProofWorkerBusyException(
                        "Kagemusha $label is busy; retry after the active proof completes",
                        failure,
                    )
                }
                throw failure
            } finally {
                heavyProofPermit.unlock()
            }
        }

        internal fun withHeavyProofPermitForTest(action: () -> Unit) {
            withHeavyProofPermit("test", action)
        }

        private inline fun <T> transferChangeOpeningOwnership(
            changeOpening: NoteOpening?,
            transfer: (NoteOpening?) -> T,
        ): T {
            var locallyOwned = changeOpening
            return try {
                transfer(locallyOwned).also { locallyOwned = null }
            } finally {
                locallyOwned?.destroy()
            }
        }

        internal fun requireCanonicalV4ArtifactRoleInventory(roles: List<ArtifactRoleV4>) {
            require(roles.size == ArtifactRoleV4.entries.size) {
                "artifact roles must contain exactly eight entries"
            }
            require(roles == ArtifactRoleV4.entries) {
                "artifact roles are not in canonical V4 order"
            }
        }

        @JvmStatic
        fun isArtifactStreamingAvailable(): Boolean = artifactBridgeAvailable

        /**
         * True only when the linked bridge was compiled with the non-default production
         * Kagemusha capability. Unlike [isProofBackendAvailable], this remains true before an
         * authenticated artifact set is installed and is therefore safe for setup bootstrapping.
         */
        @JvmStatic
        fun isProductionProofBackendCompiled(): Boolean =
            artifactBridgeAvailable && detectProductionProofBackendCompilation {
                nativeArtifactBeginV4(byteArrayOf(0), ByteArray(32), ByteArray(32))
            }

        @JvmStatic
        fun isProofBackendAvailable(): Boolean =
            artifactBridgeAvailable && runCatching { nativePastaCycleV4BackendAvailable() }
                .getOrDefault(false)

        /** SHA-256 of the exact authenticated release held by native, or null when absent. */
        @JvmStatic
        fun installedArtifactManifestSha256V4(): ByteArray? =
            if (!artifactBridgeAvailable) {
                null
            } else {
                runCatching {
                    requireDigest(nativeInstalledManifestSha256V4(), "installedManifestSha256")
                }.getOrNull()
            }

        @JvmStatic
        fun beginArtifactIngest(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            expectedArtifactSha256: ByteArray,
        ): ArtifactIngest {
            requireArtifactBridge()
            val manifest = requireManifest(manifestNorito)
            val manifestDigest = requireDigest(manifestSha256, "manifestSha256")
            val artifactDigest = requireDigest(expectedArtifactSha256, "expectedArtifactSha256")
            val handle = nativeArtifactBeginV4(manifest, manifestDigest, artifactDigest)
            check(handle > 0) { "native Kagemusha artifact ingest returned no handle" }
            return ArtifactIngest(handle)
        }

        @JvmStatic
        fun beginArtifactInstallSession(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            releaseAuthentication: ReleaseAuthentication,
        ): ArtifactInstallSession {
            requireArtifactBridge()
            return ArtifactInstallSession(
                requireManifest(manifestNorito),
                requireDigest(manifestSha256, "manifestSha256"),
                releaseAuthentication,
            )
        }

        @JvmStatic
        fun decodeRecipientPaymentRequest(archive: ByteArray): RecipientPaymentRequest =
            RecipientPaymentRequest(archive)

        @JvmStatic
        fun decodeRecipientRegistrationLineageV2(
            archive: ByteArray,
        ): RecipientRegistrationLineage = RecipientRegistrationLineage(archive)

        @JvmStatic
        fun decodeRecipientReceiveOfferV2(archive: ByteArray): RecipientReceiveOfferV2 =
            RecipientReceiveOfferV2(archive).also(::projectRecipientReceiveOfferV2)

        @JvmStatic
        fun decodePeerPayment(archive: ByteArray): PeerPayment = PeerPayment(archive)

        @JvmStatic
        fun decodeReceiverAcknowledgement(archive: ByteArray): ReceiverAcknowledgement =
            ReceiverAcknowledgement(archive)

        @JvmStatic
        fun decodeNoteMembershipWitness(archive: ByteArray): NoteMembershipWitness =
            NoteMembershipWitness(archive)

        /** Restore the opaque note opening retained for a finalized top-up or staged output. */
        @JvmStatic
        fun decodeNoteOpening(archive: ByteArray): NoteOpening = NoteOpening(archive)

        @JvmStatic
        fun decodeInitRequestV4(archive: ByteArray): InitRequestV4 = InitRequestV4(archive)

        @JvmStatic
        fun decodeAppendRequestV4(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): AppendRequestV4 = transferChangeOpeningOwnership(changeOpening) {
            AppendRequestV4(archive, it)
        }

        @JvmStatic
        fun decodeVerifyRequestV4(archive: ByteArray): VerifyRequestV4 = VerifyRequestV4(archive)

        @JvmStatic
        fun decodeTopUpAnchorV4(archive: ByteArray): TopUpAnchorV4 = TopUpAnchorV4(archive)

        @JvmStatic
        fun decodeBundleV4(archive: ByteArray): BundleV4 = BundleV4(archive)

        @JvmStatic
        fun decodeTopUpFinalityEvidenceV4(archive: ByteArray): TopUpFinalityEvidenceV4 =
            TopUpFinalityEvidenceV4(archive)

        @JvmStatic
        fun decodeTopUpProvenanceV4(archive: ByteArray): TopUpProvenanceV4 =
            TopUpProvenanceV4(archive)

        /** Restores canonical persisted frontier bytes without making a branch spendable. */
        @JvmStatic
        fun decodeOutputMembershipFrontierV4(archive: ByteArray): OutputMembershipFrontierV4 =
            OutputMembershipFrontierV4(archive)

        /** Builds the canonical next-zero frontier persisted atomically with a branch. */
        @JvmStatic
        fun buildOutputMembershipFrontierV4(
            zeroPath: OutputMembershipPath,
        ): OutputMembershipFrontierV4 {
            requireArtifactBridge()
            val siblings = zeroPath.flattenedSiblings()
            val directions = zeroPath.directions()
            val root = zeroPath.root()
            return try {
                OutputMembershipFrontierV4(nativeBuildOutputMembershipFrontierV4(
                    zeroPath.leafIndex,
                    siblings,
                    directions,
                    root,
                ))
            } finally {
                siblings.fill(0)
                directions.fill(0)
                root.fill(0)
            }
        }

        /** Derives the only valid consecutive output paths from one authenticated frontier. */
        @JvmStatic
        fun deriveOutputMembershipPathsV4(
            frontier: OutputMembershipFrontierV4,
            recipientCommitment: ByteArray?,
            changeCommitment: ByteArray?,
        ): OutputMembershipPaths {
            require(recipientCommitment != null || changeCommitment != null) {
                "recipientCommitment or changeCommitment must be present"
            }
            requireArtifactBridge()
            val frontierArchive = frontier.noritoEncoded()
            val recipient = recipientCommitment?.let {
                requireDigest(it, "recipientCommitment")
            } ?: byteArrayOf()
            val change = changeCommitment?.let {
                requireDigest(it, "changeCommitment")
            } ?: byteArrayOf()
            return try {
                outputMembershipPathsFromNativeProjection(
                    nativeDeriveOutputMembershipPathsV4(frontierArchive, recipient, change),
                )
            } finally {
                frontierArchive.fill(0)
                recipient.fill(0)
                change.fill(0)
            }
        }

        /**
         * Restore one secret-bearing V4 branch only after native revalidates its provenance
         * against the bundle and the release installed at the current block height.
         *
         * Ownership of [opening] transfers at call entry. A failed restore destroys it; a
         * successful restore transfers it to the returned [SpendableBranchV4], which the caller
         * must close after the local builder has consumed the branch.
         */
        @JvmStatic
        fun restoreSpendableBranchV4(
            bundle: BundleV4,
            membershipWitness: NoteMembershipWitness,
            opening: NoteOpening,
            topUpProvenance: TopUpProvenanceV4,
            blockHeight: Long,
        ): SpendableBranchV4 = transferChangeOpeningOwnership(opening) { ownedOpening ->
            restoreSpendableBranchV4Owned(
                bundle,
                membershipWitness,
                checkNotNull(ownedOpening),
                topUpProvenance,
                blockHeight,
            )
        }

        private fun restoreSpendableBranchV4Owned(
            bundle: BundleV4,
            membershipWitness: NoteMembershipWitness,
            opening: NoteOpening,
            topUpProvenance: TopUpProvenanceV4,
            blockHeight: Long,
        ): SpendableBranchV4 {
            require(blockHeight > 0) { "blockHeight must be positive" }
            requireArtifactBridge()
            requireV4ProofBackend()
            val bundleArchive = bundle.noritoEncoded()
            val provenanceArchive = topUpProvenance.noritoEncoded()
            val witnessArchive = membershipWitness.noritoEncoded()
            val openingArchive = opening.noritoEncoded()
            return try {
                val frontier = OutputMembershipFrontierV4(nativeValidateSpendableBranchV4(
                    bundleArchive,
                    provenanceArchive,
                    witnessArchive,
                    openingArchive,
                    blockHeight,
                ))
                SpendableBranchV4(
                    bundle,
                    membershipWitness,
                    opening,
                    topUpProvenance,
                    frontier,
                )
            } finally {
                bundleArchive.fill(0)
                provenanceArchive.fill(0)
                witnessArchive.fill(0)
                openingArchive.fill(0)
            }
        }

        /** Restore finalized top-up state with its caller-retained, local-only note opening. */
        @JvmStatic
        fun restoreInitBranchV4(
            result: InitResultV4,
            opening: NoteOpening,
            blockHeight: Long,
        ): SpendableBranchV4 = transferChangeOpeningOwnership(opening) { ownedOpening ->
            require(blockHeight > 0) { "blockHeight must be positive" }
            val projection = projectInitResultV4(result)
            restoreSpendableBranchV4Owned(
                projection.branch.bundle,
                projection.branch.membershipWitness,
                checkNotNull(ownedOpening),
                projection.topUpProvenance,
                blockHeight,
            )
        }

        /** Restore a received offline payment with the receiver's local-only note opening. */
        @JvmStatic
        fun restorePeerPaymentBranchV4(
            payment: PeerPayment,
            opening: NoteOpening,
            blockHeight: Long,
        ): SpendableBranchV4 = transferChangeOpeningOwnership(opening) { ownedOpening ->
            require(blockHeight > 0) { "blockHeight must be positive" }
            val projection = projectPeerPayment(payment)
            restoreSpendableBranchV4Owned(
                projection.branch.bundle,
                projection.branch.membershipWitness,
                checkNotNull(ownedOpening),
                projection.topUpProvenance,
                blockHeight,
            )
        }

        /** Restore sender change retained locally after a successful offline split. */
        @JvmStatic
        fun restoreSplitChangeBranchV4(
            result: SplitResultV4,
            blockHeight: Long,
        ): SpendableBranchV4 {
            require(blockHeight > 0) { "blockHeight must be positive" }
            val opening = checkNotNull(result.takeChangeOpening()) {
                "split result has no local change opening"
            }
            return transferChangeOpeningOwnership(opening) { ownedOpening ->
                val projection = projectSplitResultV4(result)
                val change = checkNotNull(projection.change) {
                    "split result has no spendable change branch"
                }
                val provenance = checkNotNull(projection.changeTopUpProvenance) {
                    "split result has no spendable change provenance"
                }
                restoreSpendableBranchV4Owned(
                    change.bundle,
                    change.membershipWitness,
                    checkNotNull(ownedOpening),
                    provenance,
                    blockHeight,
                )
            }
        }

        /** Restore offline change retained locally after building a partial redemption. */
        @JvmStatic
        fun restoreRedeemChangeBranchV4(
            result: RedeemBuildResultV4,
            blockHeight: Long,
        ): SpendableBranchV4 {
            require(blockHeight > 0) { "blockHeight must be positive" }
            val opening = checkNotNull(result.takeChangeOpening()) {
                "redeem result has no local change opening"
            }
            return transferChangeOpeningOwnership(opening) { ownedOpening ->
                val projection = projectRedeemBuildResultV4(result)
                val change = checkNotNull(projection.change) {
                    "redeem result has no spendable change branch"
                }
                val provenance = checkNotNull(projection.changeTopUpProvenance) {
                    "redeem result has no spendable change provenance"
                }
                restoreSpendableBranchV4Owned(
                    change.bundle,
                    change.membershipWitness,
                    checkNotNull(ownedOpening),
                    provenance,
                    blockHeight,
                )
            }
        }

        @JvmStatic
        fun decodeRedeemRequestV5(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): RedeemRequestV5 = transferChangeOpeningOwnership(changeOpening) {
            RedeemRequestV5(archive, it)
        }

        @JvmStatic
        fun decodeInitResultV4(archive: ByteArray): InitResultV4 = InitResultV4(archive)

        @JvmStatic
        fun decodeSplitResultV4(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): SplitResultV4 = transferChangeOpeningOwnership(changeOpening) {
            SplitResultV4(archive, it)
        }

        @JvmStatic
        fun decodeVerifyResultV4(archive: ByteArray): VerifyResultV4 = VerifyResultV4(archive)

        @JvmStatic
        fun decodeRedeemBuildResultV4(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): RedeemBuildResultV4 = transferChangeOpeningOwnership(changeOpening) {
            RedeemBuildResultV4(archive, it)
        }

        @JvmStatic
        fun decodeTopUpFinalityRosterArtifact(archive: ByteArray): TopUpFinalityRosterArtifact =
            TopUpFinalityRosterArtifact(archive)

        /** Restore the exact canonical Torii request retained for an idempotent top-up retry. */
        @JvmStatic
        fun decodeTopUpRequest(archive: ByteArray): TopUpRequest = TopUpRequest(archive)

        /** Restore the exact canonical Torii request retained for an idempotent redemption retry. */
        @JvmStatic
        fun decodeRedeemSubmissionRequest(archive: ByteArray): RedeemSubmissionRequest =
            RedeemSubmissionRequest(archive)

        @JvmStatic
        fun projectTopUpRequestIdentity(request: TopUpRequest): OperationIdentity {
            requireArtifactBridge()
            val archive = request.noritoEncoded()
            return try {
                val parsed = operationIdentityFromCanonicalRequest(
                    archive,
                    "iroha.torii.v1.offline.top_up.request",
                    OperationKind.TOP_UP,
                    8,
                    6,
                )
                val projected = operationRequestIdentityFromNativeProjection(
                    nativeProjectTopUpRequestIdentityV4(archive),
                )
                check(projected == parsed) {
                    "native top-up request identity does not match the canonical request"
                }
                parsed
            } finally {
                archive.fill(0)
            }
        }

        @JvmStatic
        fun projectRedeemRequestIdentity(request: RedeemSubmissionRequest): OperationIdentity {
            requireArtifactBridge()
            val archive = request.noritoEncoded()
            return try {
                val parsed = operationIdentityFromCanonicalRequest(
                    archive,
                    "iroha.torii.v1.offline.redeem.request",
                    OperationKind.REDEEM,
                    10,
                    8,
                )
                val projected = operationRequestIdentityFromNativeProjection(
                    nativeProjectRedeemRequestIdentityV4(archive),
                )
                check(projected == parsed) {
                    "native redemption request identity does not match the canonical request"
                }
                parsed
            } finally {
                archive.fill(0)
            }
        }

        private fun operationRequestIdentityFromNativeProjection(
            fields: Array<ByteArray>,
        ): OperationIdentity {
            requireFieldCount(fields, 6, "operation request identity projection")
            return try {
                OperationIdentity(
                    requireMarkedDigest(fields[0], "operationId"),
                    requireMarkedDigest(fields[1], "requestAuthorityDigest"),
                    requireMarkedDigest(fields[2], "canonicalRequestDigest"),
                    operationKind(canonicalText(fields[3], "operationKind")),
                    longInteger(fields[4], "issuedAtMilliseconds"),
                    longInteger(fields[5], "expiresAtMilliseconds"),
                )
            } finally {
                fields.forEach { it.fill(0) }
            }
        }

        private fun operationKind(value: String): OperationKind = when (value) {
            "top_up" -> OperationKind.TOP_UP
            "redeem" -> OperationKind.REDEEM
            else -> error("native Kagemusha operation kind is invalid")
        }

        @JvmStatic
        fun projectOperationReference(reference: OperationReference): OperationReferenceProjection {
            requireArtifactBridge()
            val fields = nativeProjectOperationReferenceV2(reference.noritoEncoded())
            requireFieldCount(fields, 9, "operation reference projection")
            return try {
                OperationReferenceProjection(
                    operationIdentityFromProjection(fields, 0),
                    when (canonicalText(fields[6], "operationState")) {
                        "pending" -> OperationState.PENDING
                        else -> error("native Kagemusha operation reference state is invalid")
                    },
                    requireTransactionHash(fields[7], "transactionHash"),
                    canonicalText(fields[8], "statusUri"),
                )
            } finally {
                fields.forEach { it.fill(0) }
            }
        }

        @JvmStatic
        fun projectOperationStatus(status: OperationStatus): OperationStatusProjection {
            requireArtifactBridge()
            val fields = nativeProjectOperationStatusV2(status.noritoEncoded())
            requireFieldCount(fields, 13, "operation status projection")
            val identity = operationIdentityFromProjection(fields, 0)
            val state = when (canonicalText(fields[6], "operationState")) {
                "pending" -> OperationState.PENDING
                "applied" -> OperationState.APPLIED
                "rejected" -> OperationState.REJECTED
                else -> error("native Kagemusha operation state is invalid")
            }
            val finalizedHeight = fields[8].takeIf { it.isNotEmpty() }
                ?.let { longInteger(it, "finalizedBlockHeight") }
            val finalizedTopUp = if (fields[9].isNotEmpty() || fields[10].isNotEmpty()) {
                check(state == OperationState.APPLIED && identity.kind == OperationKind.TOP_UP &&
                    fields[9].isNotEmpty() && fields[10].isNotEmpty() &&
                    finalizedHeight != null) {
                    "native Kagemusha finalized top-up fields are invalid"
                }
                FinalizedTopUp(
                    TopUpAnchorV4(fields[9]),
                    TopUpFinalityProof(fields[10]),
                    finalizedHeight,
                )
            } else {
                null
            }
            val rejection = if (fields[11].isNotEmpty() || fields[12].isNotEmpty()) {
                check(state == OperationState.REJECTED && fields[11].isNotEmpty() && fields[12].isNotEmpty()) {
                    "native Kagemusha rejection fields are invalid"
                }
                OperationRejection(
                    canonicalText(fields[11], "rejectionCode"),
                    canonicalText(fields[12], "rejectionMessage"),
                )
            } else {
                null
            }
            return try {
                OperationStatusProjection(
                    state,
                    identity,
                    requireTransactionHash(fields[7], "transactionHash"),
                    finalizedHeight,
                    finalizedTopUp,
                    rejection,
                )
            } finally {
                fields.forEach { it.fill(0) }
            }
        }

        @JvmStatic
        fun prepareRequestAuthorization(
            authority: String,
            chainDiscriminant: Int,
            deviceId: String,
            assetDefinitionId: String,
            issuedAtMilliseconds: Long,
            expiresAtMilliseconds: Long,
            nonce: ByteArray,
            payloadDigest: ByteArray,
            registrationHash: ByteArray,
            platform: OnlineHardwareAssertionPlatform,
        ): RequestAuthorizationPreparation {
            requireArtifactBridge()
            val fields = nativePrepareAuthorizationV3(
                utf8(authority, "authority"),
                requireChainDiscriminant(chainDiscriminant),
                utf8(deviceId, "deviceId"),
                utf8(assetDefinitionId, "assetDefinitionId"),
                issuedAtMilliseconds,
                expiresAtMilliseconds,
                requireDigest(nonce, "nonce"),
                requireDigest(payloadDigest, "payloadDigest"),
                requireDigest(registrationHash, "registrationHash"),
                utf8(platform.wireName, "hardwareAssertionPlatform"),
            )
            requireFieldCount(fields, 5, "authorization preparation")
            return RequestAuthorizationPreparation(
                RequestAuthorizationPreparationArchive(fields[0]),
                fields[1],
                fields[2],
                fields[3],
                fields[4],
            )
        }

        @JvmStatic
        fun finalizeRequestAuthorization(
            preparation: RequestAuthorizationPreparation,
            platformSignatureDer: ByteArray,
            authenticatorData: ByteArray? = null,
        ): RequestAuthorization {
            requireArtifactBridge()
            val expectedRawSignature =
                KagemushaP256Codec.rawLowSFromStrictDer(platformSignatureDer)
            val fields = nativeFinalizeHardwareAuthorizationV3(
                preparation.archive.noritoEncoded(),
                authenticatorData?.copyOf() ?: ByteArray(0),
                platformSignatureDer.copyOf(),
            )
            requireFieldCount(fields, 2, "authorization finalization")
            check(fields[1].contentEquals(expectedRawSignature)) {
                "native authorization signature normalization drifted from the SDK"
            }
            return RequestAuthorization(
                fields[0],
            )
        }

        /** Finalize directly from the CBOR returned by DCAppAttestService.generateAssertion. */
        @JvmStatic
        fun finalizeIosAppAttest(
            preparation: RequestAuthorizationPreparation,
            assertionObject: ByteArray,
        ): RequestAuthorization {
            requireArtifactBridge()
            val boundedAssertionObject = requiredBytes(assertionObject, "assertionObject")
            require(boundedAssertionObject.size <= IOS_APP_ATTEST_ASSERTION_OBJECT_MAX_BYTES) {
                "assertionObject exceeds the App Attest response bound"
            }
            val fields = nativeFinalizeIosAppAttestAuthorizationV3(
                preparation.archive.noritoEncoded(),
                boundedAssertionObject,
            )
            return requestAuthorizationFromIosAppAttestNativeProjection(fields)
        }

        internal fun requestAuthorizationFromIosAppAttestNativeProjection(
            fields: Array<ByteArray>?,
        ): RequestAuthorization {
            requireFieldCount(fields, 3, "App Attest authorization finalization")
            val projection = checkNotNull(fields)
            try {
                KagemushaP256Codec.requireRawLowSSignature(projection[1])
            } catch (failure: IllegalArgumentException) {
                throw IllegalStateException(
                    "native Kagemusha App Attest finalization returned an invalid raw signature",
                    failure,
                )
            }
            requireIosAppAttestAuthenticatorDataProjection(projection[2])
            return RequestAuthorization(projection[0])
        }

        @JvmStatic
        fun finalizeTopUp(
            unsigned: TopUpUnsigned,
            authorization: RequestAuthorization,
        ): TopUpRequest {
            requireArtifactBridge()
            return TopUpRequest(
                nativeFinalizeTopUpV5(unsigned.noritoEncoded(), authorization.noritoEncoded()),
            )
        }

        @JvmStatic
        fun finalizeTopUp(
            preparation: TopUpPreparation,
            authorization: RequestAuthorization,
        ): TopUpRequest = finalizeTopUp(preparation.unsigned, authorization)

        @JvmStatic
        fun prepareTopUp(
            networkId: NetworkId,
            chainDiscriminant: Int,
            assetDefinitionId: String,
            payerAccountId: String,
            amount: KagemushaScaledAmount,
            nonce: ByteArray,
            openingSpendKey: ByteArray,
            openingRho: ByteArray,
            openingDiversifier: ByteArray,
            zeroPath: TopUpZeroPath,
            shieldVerifierCommitment: ByteArray,
            artifactBinding: ArtifactBindingV4,
        ): TopUpPreparation {
            requireArtifactBridge()
            return SecretArchiveWiper.withOpeningDigests(
                openingSpendKey,
                "openingSpendKey",
                openingRho,
                "openingRho",
                openingDiversifier,
                "openingDiversifier",
            ) { spendKeyCopy, rhoCopy, diversifierCopy ->
                var fields: Array<ByteArray>? = null
                var locallyOwnedOpening: NoteOpening? = null
                try {
                    val nativeFields = nativePrepareTopUpV5(
                        networkId.bytes(),
                        requireChainDiscriminant(chainDiscriminant),
                        utf8(assetDefinitionId, "assetDefinitionId"),
                        utf8(payerAccountId, "payerAccountId"),
                        utf8(amount.atomicUnits, "atomicUnits"),
                        amount.scale,
                        requireDigest(nonce, "nonce"),
                        spendKeyCopy,
                        rhoCopy,
                        diversifierCopy,
                        zeroPath.leafIndex,
                        zeroPath.flattenedSiblings(),
                        zeroPath.directions(),
                        zeroPath.root(),
                        requireDigest(shieldVerifierCommitment, "shieldVerifierCommitment"),
                        artifactBinding.noritoEncoded(),
                    ).also { fields = it }
                    requireFieldCount(nativeFields, 11, "top-up preparation")
                    val opening = NoteOpening(nativeFields[2]).also {
                        locallyOwnedOpening = it
                    }
                    val preparation = TopUpPreparation(
                        TopUpUnsigned(nativeFields[0]),
                        nativeFields[1],
                        opening,
                        nativeFields[3],
                        nativeFields[4],
                        nativeFields[5],
                        nativeFields[6],
                        nativeFields[7],
                        amount(nativeFields[8], nativeFields[9]),
                        integer(nativeFields[10], "leafIndex"),
                    )
                    locallyOwnedOpening = null
                    preparation
                } finally {
                    locallyOwnedOpening?.close()
                    SecretArchiveWiper.wipeAll(fields)
                }
            }
        }

        @JvmStatic
        fun finalizeRedeemV4(
            buildResult: RedeemBuildResultV4,
            authorization: RequestAuthorization,
        ): RedeemFinalization {
            requireArtifactBridge()
            val fields = nativeFinalizeRedeemV5(
                buildResult.noritoEncoded(),
                authorization.noritoEncoded(),
            )
            requireFieldCount(fields, 2, "V4 redeem finalization")
            return RedeemFinalization(
                RedeemSubmissionRequest(fields[0]),
                requireDigest(fields[1], "operationId"),
            )
        }

        @JvmStatic
        fun prepareRecipientPaymentRequest(
            networkId: NetworkId,
            chainDiscriminant: Int,
            assetDefinitionId: String,
            amount: KagemushaScaledAmount,
            recipientAccountId: String,
            receiverDeviceId: String,
            receiverPublicKey: KagemushaDevicePublicKeyV2,
            requestId: ByteArray,
            issuedAtMilliseconds: Long,
            expiresAtMilliseconds: Long,
            spendKey: ByteArray,
            rho: ByteArray,
            diversifier: ByteArray,
        ): RecipientRequestPreparation {
            requireArtifactBridge()
            return SecretArchiveWiper.withOpeningDigests(
                spendKey,
                "spendKey",
                rho,
                "rho",
                diversifier,
                "diversifier",
            ) { spendKeyCopy, rhoCopy, diversifierCopy ->
                var fields: Array<ByteArray>? = null
                var locallyOwnedOpening: NoteOpening? = null
                try {
                    val nativeFields = nativePrepareRecipientRequestV2(
                        networkId.bytes(),
                        requireChainDiscriminant(chainDiscriminant),
                        utf8(assetDefinitionId, "assetDefinitionId"),
                        utf8(amount.atomicUnits, "atomicUnits"),
                        amount.scale,
                        utf8(recipientAccountId, "recipientAccountId"),
                        utf8(receiverDeviceId, "receiverDeviceId"),
                        receiverPublicKey.sec1Bytes(),
                        requireDigest(requestId, "requestId"),
                        issuedAtMilliseconds,
                        expiresAtMilliseconds,
                        spendKeyCopy,
                        rhoCopy,
                        diversifierCopy,
                    ).also { fields = it }
                    requireFieldCount(nativeFields, 5, "recipient request preparation")
                    val opening = NoteOpening(nativeFields[2]).also {
                        locallyOwnedOpening = it
                    }
                    val preparation = RecipientRequestPreparation(
                        RecipientRequestPayload(nativeFields[0]),
                        nativeFields[1],
                        opening,
                        nativeFields[3],
                        nativeFields[4],
                        amount,
                    )
                    locallyOwnedOpening = null
                    preparation
                } finally {
                    locallyOwnedOpening?.close()
                    SecretArchiveWiper.wipeAll(fields)
                }
            }
        }

        /** Prepare one local-only opening for sender change or partial redemption change. */
        @JvmStatic
        fun prepareNoteOpening(
            spendKey: ByteArray,
            rho: ByteArray,
            diversifier: ByteArray,
        ): NoteOpening {
            requireArtifactBridge()
            return SecretArchiveWiper.withOpeningDigests(
                spendKey,
                "spendKey",
                rho,
                "rho",
                diversifier,
                "diversifier",
            ) { spendKeyCopy, rhoCopy, diversifierCopy ->
                var nativeArchive: ByteArray? = null
                try {
                    NoteOpening(
                        nativePrepareNoteOpeningV2(spendKeyCopy, rhoCopy, diversifierCopy)
                            .also { nativeArchive = it },
                    )
                } finally {
                    SecretArchiveWiper.wipe(nativeArchive)
                }
            }
        }

        /**
         * Prepare partial-redemption change inside the native secret boundary.
         *
         * Native code revalidates the exact input note/opening and derives a fresh opening from a
         * domain-separated binding over that input, [changeAmount], canonical [recipientAccountId],
         * [nonce], and caller entropy. The operation id is derived inside native code.
         * The authoritative confidential diversifier is selected natively; wallet code never
         * fabricates it. Returned coordinates exist only so encrypted wallet state can restore the
         * proof-bound change after finality.
         */
        @JvmStatic
        fun prepareRedemptionChangeV5(
            input: SpendableBranchV4,
            changeAmount: KagemushaScaledAmount,
            recipientAccountId: String,
            chainDiscriminant: Int,
            nonce: ByteArray,
            entropy: ByteArray,
        ): RedemptionChangePreparationV4 {
            requireArtifactBridge()
            var recipient: ByteArray? = null
            var freshNonce: ByteArray? = null
            var freshEntropy: ByteArray? = null
            var bundleArchive: ByteArray? = null
            var openingArchive: ByteArray? = null
            var atomicUnits: ByteArray? = null
            var fields: Array<ByteArray>? = null
            var opening: NoteOpening? = null
            return try {
                val recipientBytes = utf8(recipientAccountId, "recipientAccountId")
                    .also { recipient = it }
                val nonceCopy = requireDigest(nonce, "nonce")
                    .also { freshNonce = it }
                val entropyCopy = requireDigest(entropy, "entropy")
                    .also { freshEntropy = it }
                require(!nonceCopy.contentEquals(entropyCopy)) {
                    "entropy must be distinct from nonce"
                }
                val bundleBytes = input.bundle.noritoEncoded().also { bundleArchive = it }
                val openingBytes = input.opening.noritoEncoded().also { openingArchive = it }
                val amountBytes = utf8(changeAmount.atomicUnits, "atomicUnits")
                    .also { atomicUnits = it }
                val nativeFields = nativePrepareRedemptionChangeV5(
                    bundleBytes,
                    openingBytes,
                    amountBytes,
                    changeAmount.scale,
                    recipientBytes,
                    requireChainDiscriminant(chainDiscriminant),
                    nonceCopy,
                    entropyCopy,
                ).also { fields = it }
                requireFieldCount(nativeFields, 7, "V5 redemption change preparation")
                for ((index, name) in listOf(
                    1 to "rho",
                    2 to "diversifier",
                    3 to "commitment",
                    4 to "spendNullifier",
                )) {
                    require(
                        nativeFields[index].size == 32 &&
                            nativeFields[index].any { it.toInt() != 0 },
                    ) { "$name must be a non-zero 32-byte native field" }
                }
                require(!nativeFields[1].contentEquals(nativeFields[2])) {
                    "native Kagemusha redemption opening coordinates collide"
                }
                val projectedAmount = amount(nativeFields[5], nativeFields[6])
                check(projectedAmount == changeAmount) {
                    "native Kagemusha redemption change amount changed"
                }
                val preparedOpening = NoteOpening(nativeFields[0]).also { opening = it }
                opening = null
                RedemptionChangePreparationV4(
                    preparedOpening,
                    nativeFields[1],
                    nativeFields[2],
                    nativeFields[3],
                    nativeFields[4],
                    projectedAmount,
                )
            } finally {
                opening?.destroy()
                fields?.forEach { field ->
                    @Suppress("SENSELESS_COMPARISON")
                    if (field != null) field.fill(0)
                }
                atomicUnits?.fill(0)
                openingArchive?.fill(0)
                bundleArchive?.fill(0)
                freshEntropy?.fill(0)
                freshNonce?.fill(0)
                recipient?.fill(0)
            }
        }

        /**
         * Prepare sender change for an ordinary one- or two-input peer split.
         *
         * Native reauthenticates every ordered bundle/opening pair, enforces shared
         * chain/asset/root/artifact context and exact value conservation against [recipientRequest],
         * then derives an owned opening under a peer-split-only domain.
         */
        @JvmStatic
        fun preparePeerSplitChangeV4(
            inputs: List<SpendableBranchV4>,
            recipientRequest: VerifiedRecipientPaymentRequest,
            changeAmount: KagemushaScaledAmount,
            operationId: ByteArray,
            entropy: ByteArray,
        ): PeerSplitChangePreparationV4 {
            requireArtifactBridge()
            require(inputs.size in 1..MAXIMUM_INPUTS_PER_TRANSITION) {
                "inputs must contain one or two spendable branches"
            }
            val operation = requireDigest(operationId, "operationId")
            val freshEntropy = requireDigest(entropy, "entropy")
            require(!operation.contentEquals(freshEntropy)) {
                "entropy must be distinct from operationId"
            }
            val bundles = inputs.map { it.bundle.noritoEncoded() }.toTypedArray()
            val openings = inputs.map { it.opening.noritoEncoded() }.toTypedArray()
            val signedRequest = recipientRequest.request.noritoEncoded()
            val atomicUnits = utf8(changeAmount.atomicUnits, "atomicUnits")
            var fields: Array<ByteArray>? = null
            var opening: NoteOpening? = null
            return try {
                val nativeFields = nativePreparePeerSplitChangeV4(
                    bundles,
                    openings,
                    signedRequest,
                    atomicUnits,
                    changeAmount.scale,
                    operation,
                    freshEntropy,
                ).also { fields = it }
                requireFieldCount(nativeFields, 7, "V4 peer-split change preparation")
                for ((index, name) in listOf(
                    1 to "rho",
                    2 to "diversifier",
                    3 to "commitment",
                    4 to "spendNullifier",
                )) {
                    require(nativeFields[index].size == 32 && nativeFields[index].any { it != 0.toByte() }) {
                        "$name must be a non-zero 32-byte native field"
                    }
                }
                val projectedAmount = amount(nativeFields[5], nativeFields[6])
                check(projectedAmount == changeAmount) {
                    "native Kagemusha peer-split change amount changed"
                }
                val preparedOpening = NoteOpening(nativeFields[0]).also { opening = it }
                opening = null
                PeerSplitChangePreparationV4(
                    preparedOpening,
                    nativeFields[1],
                    nativeFields[2],
                    nativeFields[3],
                    nativeFields[4],
                    projectedAmount,
                )
            } finally {
                opening?.destroy()
                fields?.forEach { it.fill(0) }
                atomicUnits.fill(0)
                signedRequest.fill(0)
                openings.forEach { it.fill(0) }
                bundles.forEach { it.fill(0) }
                freshEntropy.fill(0)
                operation.fill(0)
            }
        }

        @JvmStatic
        fun signRecipientPaymentRequest(
            preparation: RecipientRequestPreparation,
            signature: KagemushaDeviceSignatureV2,
        ): RecipientPaymentRequest {
            requireArtifactBridge()
            return RecipientPaymentRequest(
                nativeCreateRecipientRequestV2(
                    preparation.payload.noritoEncoded(),
                    signature.rawBytes(),
                ),
            )
        }

        @JvmStatic
        fun verifyRecipientPaymentRequest(
            request: RecipientPaymentRequest,
            verifiedAtMilliseconds: Long,
        ): VerifiedRecipientPaymentRequest {
            requireArtifactBridge()
            require(verifiedAtMilliseconds > 0) { "verifiedAtMilliseconds must be positive" }
            val projection = projectRecipientPaymentRequest(request)
            return VerifiedRecipientPaymentRequest(
                request,
                requireDigest(
                    nativeVerifyRecipientRequestV2(request.noritoEncoded(), verifiedAtMilliseconds),
                    "requestDigest",
                ),
                verifiedAtMilliseconds,
                projection,
            )
        }

        /** Create the request-independent selector used to prefetch portable receiver lineage. */
        @JvmStatic
        fun createRecipientLineageQueryV2(
            networkId: NetworkId,
            chainDiscriminant: Int,
            recipientAccountId: String,
            receiverDeviceId: String,
            assetDefinitionId: String,
            trustedCheckpointHeight: Long,
        ): RecipientLineageQueryV2 {
            requireArtifactBridge()
            require(trustedCheckpointHeight > 0) { "trustedCheckpointHeight must be positive" }
            return RecipientLineageQueryV2(
                nativeCreateRecipientLineageQueryV2(
                    networkId.bytes(),
                    requireChainDiscriminant(chainDiscriminant),
                    utf8(recipientAccountId, "recipientAccountId"),
                    utf8(receiverDeviceId, "receiverDeviceId"),
                    utf8(assetDefinitionId, "assetDefinitionId"),
                    trustedCheckpointHeight,
                ),
            )
        }

        /** Verify signed request, active-state lineage and a bounded finality suffix locally. */
        @JvmStatic
        fun verifyRecipientRegistrationLineageV2(
            request: RecipientPaymentRequest,
            lineage: RecipientRegistrationLineage,
            verifiedAtMilliseconds: Long,
            trustedCheckpointHeight: Long,
            trustedCheckpointContextId: ByteArray,
        ): VerifiedRecipientRegistrationLineageV2 {
            requireArtifactBridge()
            require(verifiedAtMilliseconds > 0) { "verifiedAtMilliseconds must be positive" }
            require(trustedCheckpointHeight > 0) {
                "trustedCheckpointHeight must be positive"
            }
            val trustedContext = requireFinalityCheckpointContext(
                trustedCheckpointContextId,
                "trustedCheckpointContextId",
            )
            return try {
                val fields = nativeVerifyRecipientRegistrationLineageV2(
                    request.noritoEncoded(),
                    lineage.noritoEncoded(),
                    verifiedAtMilliseconds,
                    trustedCheckpointHeight,
                    trustedContext,
                )
                requireFieldCount(fields, 2, "verified recipient lineage")
                VerifiedRecipientRegistrationLineageV2(
                    RecipientRegistrationLineage(fields[0]),
                    FinalityCheckpointPromotionV2(fields[1]),
                )
            } finally {
                trustedContext.fill(0)
            }
        }

        /** Build one canonical receive offer carrying request, lineage and publisher envelope. */
        @JvmStatic
        fun createRecipientReceiveOfferV2(
            request: RecipientPaymentRequest,
            lineage: RecipientRegistrationLineage,
            publisherCheckpointEnvelope: ByteArray,
        ): RecipientReceiveOfferV2 {
            requireArtifactBridge()
            val envelope = requireBoundedBytes(
                publisherCheckpointEnvelope,
                "publisherCheckpointEnvelope",
                MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1,
            )
            return try {
                RecipientReceiveOfferV2(
                    nativeCreateRecipientReceiveOfferV2(
                        request.noritoEncoded(),
                        lineage.noritoEncoded(),
                        envelope,
                    ),
                )
            } finally {
                envelope.fill(0)
            }
        }

        @JvmStatic
        fun projectRecipientReceiveOfferV2(
            offer: RecipientReceiveOfferV2,
        ): RecipientReceiveOfferProjectionV2 {
            requireArtifactBridge()
            val fields = nativeProjectRecipientReceiveOfferV2(offer.noritoEncoded())
            requireFieldCount(fields, 3, "recipient receive offer projection")
            return RecipientReceiveOfferProjectionV2(
                request = RecipientPaymentRequest(fields[0]),
                lineage = RecipientRegistrationLineage(fields[1]),
                publisherCheckpointEnvelope = fields[2],
            )
        }

        /** Verify the exact whole offer locally against one durable trusted checkpoint. */
        @JvmStatic
        fun verifyRecipientReceiveOfferV2(
            offer: RecipientReceiveOfferV2,
            verifiedAtMilliseconds: Long,
            trustedCheckpointHeight: Long,
            trustedCheckpointContextId: ByteArray,
        ): VerifiedRecipientReceiveOfferV2 {
            requireArtifactBridge()
            require(verifiedAtMilliseconds > 0) { "verifiedAtMilliseconds must be positive" }
            require(trustedCheckpointHeight > 0) {
                "trustedCheckpointHeight must be positive"
            }
            val trustedContext = requireFinalityCheckpointContext(
                trustedCheckpointContextId,
                "trustedCheckpointContextId",
            )
            return try {
                val fields = nativeVerifyRecipientReceiveOfferV2(
                    offer.noritoEncoded(),
                    verifiedAtMilliseconds,
                    trustedCheckpointHeight,
                    trustedContext,
                )
                requireFieldCount(fields, 4, "verified recipient receive offer")
                VerifiedRecipientReceiveOfferV2(
                    request = RecipientPaymentRequest(fields[0]),
                    lineage = RecipientRegistrationLineage(fields[1]),
                    publisherCheckpointEnvelope = fields[2],
                    promotedCheckpoint = FinalityCheckpointPromotionV2(fields[3]),
                    verifiedAtMilliseconds = verifiedAtMilliseconds,
                )
            } finally {
                trustedContext.fill(0)
            }
        }

        @JvmStatic
        fun projectRecipientPaymentRequest(
            request: RecipientPaymentRequest,
        ): RecipientRequestProjection {
            requireArtifactBridge()
            val fields = nativeProjectRecipientRequestV2(request.noritoEncoded())
            requireFieldCount(fields, 14, "recipient request projection")
            return RecipientRequestProjection(
                networkId = NetworkId.fromBytes(requireDigest(fields[0], "networkId")),
                assetDefinitionId = canonicalText(fields[1], "assetDefinitionId"),
                amount = amount(fields[2], fields[3]),
                recipientAccountId = canonicalText(fields[4], "recipientAccountId"),
                receiverDeviceId = canonicalText(fields[5], "receiverDeviceId"),
                requestId = fields[6],
                issuedAtMilliseconds = longInteger(fields[7], "issuedAtMilliseconds"),
                expiresAtMilliseconds = longInteger(fields[8], "expiresAtMilliseconds"),
                outputCommitment = fields[9],
                outputNullifier = fields[10],
                receiverKeyReference = fields[11],
                receiverPublicKey = fields[12],
                digest = fields[13],
            )
        }

        @JvmStatic
        fun buildInitRequestV4(
            topUpAnchor: TopUpAnchorV4,
            topUpFinalityProof: TopUpFinalityProof,
            topUpFinalityRosterArtifact: TopUpFinalityRosterArtifact,
            opening: NoteOpening,
            outputMembershipPaths: OutputMembershipPaths,
        ): InitRequestV4 {
            requireArtifactBridge()
            requireV4ProofBackend()
            require(outputMembershipPaths.recipient != null && outputMembershipPaths.change == null) {
                "initialization requires exactly one recipient output path"
            }
            var openingArchive: ByteArray? = null
            var membershipArchive: ByteArray? = null
            var nativeArchive: ByteArray? = null
            return try {
                val openingBytes = opening.noritoEncoded().also { openingArchive = it }
                val membershipBytes = outputMembershipPaths.nativeArchive()
                    .also { membershipArchive = it }
                InitRequestV4(
                    nativeBuildInitRequestV4(
                        topUpAnchor.noritoEncoded(),
                        topUpFinalityProof.noritoEncoded(),
                        topUpFinalityRosterArtifact.noritoEncoded(),
                        openingBytes,
                        membershipBytes,
                    ).also { nativeArchive = it },
                )
            } finally {
                SecretArchiveWiper.wipe(nativeArchive)
                SecretArchiveWiper.wipe(membershipArchive)
                SecretArchiveWiper.wipe(openingArchive)
            }
        }

        /** Build and validate the complete origin-finality inventory for one V4 bundle. */
        @JvmStatic
        fun buildTopUpProvenanceV4(
            bundle: BundleV4,
            topUpFinalityRosterArtifact: TopUpFinalityRosterArtifact,
            topUpAnchors: List<TopUpAnchorV4>,
            topUpFinalityProofs: List<TopUpFinalityProof>,
            blockHeight: Long,
        ): TopUpProvenanceV4 {
            require(topUpAnchors.size in 1..MAXIMUM_INPUTS_PER_TRANSITION &&
                topUpFinalityProofs.size == topUpAnchors.size) {
                "topUpAnchors and topUpFinalityProofs must have the same 1..2 count"
            }
            requireArtifactBridge()
            val anchors = topUpAnchors.map { it.noritoEncoded() }.toTypedArray()
            val proofs = topUpFinalityProofs.map { it.noritoEncoded() }.toTypedArray()
            return try {
                TopUpProvenanceV4(nativeBuildTopUpProvenanceV4(
                    bundle.noritoEncoded(),
                    topUpFinalityRosterArtifact.noritoEncoded(),
                    anchors,
                    proofs,
                    blockHeight,
                ))
            } finally {
                anchors.forEach { it.fill(0) }
                proofs.forEach { it.fill(0) }
            }
        }

        /** Revalidate persisted provenance against the bundle and current installed release. */
        @JvmStatic
        fun validateTopUpProvenanceV4(
            bundle: BundleV4,
            topUpProvenance: TopUpProvenanceV4,
            blockHeight: Long,
        ): TopUpProvenanceV4 {
            requireArtifactBridge()
            return TopUpProvenanceV4(nativeValidateTopUpProvenanceV4(
                bundle.noritoEncoded(),
                topUpProvenance.noritoEncoded(),
                blockHeight,
            ))
        }

        /** Build one canonical append request from one or two independently spendable inputs. */
        @JvmStatic
        fun buildAppendRequestV4(
            inputs: List<SpendableBranchV4>,
            changeOpening: NoteOpening?,
            outputMembershipPaths: OutputMembershipPaths,
            transferVerifierCommitment: ByteArray,
            operationId: ByteArray,
            blockHeight: Long,
        ): AppendRequestV4 = transferChangeOpeningOwnership(changeOpening) { ownedChangeOpening ->
            require(inputs.size in 1..MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS) {
                "inputs must contain one or two spendable branches"
            }
            require(inputs.map { it.bundle }.distinct().size == inputs.size) {
                "inputs must refer to distinct V4 bundles"
            }
            require(outputMembershipPaths.recipient != null) {
                "append requires a recipient output path"
            }
            require((outputMembershipPaths.change != null) == (ownedChangeOpening != null)) {
                "change output membership must be present exactly when changeOpening is present"
            }
            requireArtifactBridge()
            requireV4ProofBackend()
            var bundles: Array<ByteArray?>? = null
            var topUpProvenances: Array<ByteArray?>? = null
            var openings: Array<ByteArray?>? = null
            var witnesses: Array<ByteArray?>? = null
            var change: ByteArray? = null
            var outputMembership: ByteArray? = null
            var verifier: ByteArray? = null
            var operation: ByteArray? = null
            var archive: ByteArray? = null
            try {
                val bundleCopies = arrayOfNulls<ByteArray>(inputs.size)
                val provenanceCopies = arrayOfNulls<ByteArray>(inputs.size)
                val openingCopies = arrayOfNulls<ByteArray>(inputs.size)
                val witnessCopies = arrayOfNulls<ByteArray>(inputs.size)
                bundles = bundleCopies
                topUpProvenances = provenanceCopies
                openings = openingCopies
                witnesses = witnessCopies
                for (index in inputs.indices) {
                    val input = inputs[index]
                    bundleCopies[index] = input.bundle.noritoEncoded()
                    provenanceCopies[index] = input.topUpProvenance.noritoEncoded()
                    openingCopies[index] = input.opening.noritoEncoded()
                    witnessCopies[index] = input.membershipWitness.noritoEncoded()
                }
                change = ownedChangeOpening?.noritoEncoded() ?: byteArrayOf()
                outputMembership = outputMembershipPaths.nativeArchive()
                verifier = requireDigest(
                    transferVerifierCommitment,
                    "transferVerifierCommitment",
                )
                operation = requireDigest(operationId, "operationId")
                archive = nativeBuildAppendRequestV4(
                    Array(inputs.size) { checkNotNull(bundleCopies[it]) },
                    Array(inputs.size) { checkNotNull(provenanceCopies[it]) },
                    Array(inputs.size) { checkNotNull(openingCopies[it]) },
                    Array(inputs.size) { checkNotNull(witnessCopies[it]) },
                    checkNotNull(change),
                    checkNotNull(outputMembership),
                    checkNotNull(verifier),
                    checkNotNull(operation),
                    blockHeight,
                )
                return@transferChangeOpeningOwnership AppendRequestV4(
                    checkNotNull(archive),
                    ownedChangeOpening,
                )
            } finally {
                SecretArchiveWiper.wipeAll(bundles)
                SecretArchiveWiper.wipeAll(topUpProvenances)
                SecretArchiveWiper.wipeAll(openings)
                SecretArchiveWiper.wipeAll(witnesses)
                SecretArchiveWiper.wipe(change)
                SecretArchiveWiper.wipe(outputMembership)
                SecretArchiveWiper.wipe(verifier)
                SecretArchiveWiper.wipe(operation)
                SecretArchiveWiper.wipe(archive)
            }
        }

        @JvmStatic
        fun projectPeerPayment(payment: PeerPayment): PeerPaymentProjection {
            requireArtifactBridge()
            val fields = nativeProjectPeerPaymentV4(payment.noritoEncoded())
            val cursor = ProjectionCursor(fields, "peer payment projection")
            projectionVersion(cursor.next("version"), "peer payment projection")
            val operationId = requireDigest(cursor.next("operationId"), "operationId")
            val requestDigest = requireDigest(cursor.next("requestDigest"), "requestDigest")
            val topUpProvenance = TopUpProvenanceV4(cursor.next("topUpProvenance"))
            val projection = branchProjection(cursor)
            cursor.finish()
            val result = PeerPaymentProjection(
                projection,
                topUpProvenance,
                operationId,
                requestDigest,
            )
            operationId.fill(0)
            requestDigest.fill(0)
            return result
        }

        @JvmStatic
        fun projectInitResultV4(result: InitResultV4): InitProjectionV4 {
            requireArtifactBridge()
            val cursor = ProjectionCursor(
                nativeProjectInitResultV4(result.noritoEncoded()),
                "V4 init result projection",
            )
            projectionVersion(cursor.next("version"), "V4 init result projection")
            val topUpProvenance = TopUpProvenanceV4(cursor.next("topUpProvenance"))
            val branch = branchProjection(cursor)
            val publicStatementDigest =
                requireDigest(cursor.next("publicStatementDigest"), "publicStatementDigest")
            cursor.finish()
            return InitProjectionV4(
                branch,
                topUpProvenance,
                publicStatementDigest,
            )
        }

        /** Decode every wallet-safe field of an ABI-21 append result. */
        @JvmStatic
        fun projectSplitResultV4(result: SplitResultV4): SplitProjection {
            requireArtifactBridge()
            val cursor = ProjectionCursor(
                nativeProjectSplitResultV4(result.noritoEncoded()),
                "V4 split result projection",
            )
            projectionVersion(cursor.next("version"), "V4 split result projection")
            val payment = PeerPayment(cursor.next("peerPayment"))
            val operationId = requireDigest(cursor.next("operationId"), "operationId")
            val requestDigest = requireDigest(cursor.next("requestDigest"), "requestDigest")
            val splitBindingDigest =
                requireDigest(cursor.next("splitBindingDigest"), "splitBindingDigest")
            val recipientTopUpProvenance =
                TopUpProvenanceV4(cursor.next("recipientTopUpProvenance"))
            val recipient = branchProjection(cursor)
            val change = if (bool(cursor.next("changePresent"), "changePresent")) {
                Pair(
                    TopUpProvenanceV4(cursor.next("changeTopUpProvenance")),
                    branchProjection(cursor),
                )
            } else {
                null
            }
            cursor.finish()
            return SplitProjection(
                payment,
                recipient,
                change?.second,
                recipientTopUpProvenance,
                change?.first,
                operationId,
                requestDigest,
                splitBindingDigest,
            )
        }

        /** Decode the terminal decision and exact verified ABI-21 state. */
        @JvmStatic
        fun projectVerifyResultV4(result: VerifyResultV4): VerifyProjection {
            requireArtifactBridge()
            val cursor = ProjectionCursor(
                nativeProjectVerifyResultV4(result.noritoEncoded()),
                "V4 verify result projection",
            )
            projectionVersion(cursor.next("version"), "V4 verify result projection")
            val valid = bool(cursor.next("valid"), "valid")
            val chainAdmissible = bool(cursor.next("chainAdmissible"), "chainAdmissible")
            val lineageRedeemable = bool(cursor.next("lineageRedeemable"), "lineageRedeemable")
            val witnesslessRedemptionSupported = bool(
                cursor.next("witnesslessRedemptionSupported"),
                "witnesslessRedemptionSupported",
            )
            val commitment = cursor.next("commitment")
            val spendNullifier = cursor.next("spendNullifier")
            val amount = amount(cursor.next("atomicUnits"), cursor.next("scale"))
            val hopCount = integer(cursor.next("hopCount"), "hopCount")
            val proofStepCount = integer(cursor.next("proofStepCount"), "proofStepCount")
            val bundleDigest = cursor.next("bundleDigest")
            val assetDefinitionId = canonicalText(
                cursor.next("assetDefinitionId"),
                "assetDefinitionId",
            )
            val artifactBinding = ArtifactBindingV4(cursor.next("artifactBinding"))
            val requestDigest = cursor.next("requestDigest")
            val outputBindingDigest = cursor.next("outputBindingDigest")
            val verifierBackend = canonicalText(cursor.next("verifierBackend"), "verifierBackend")
            val verifierName = canonicalText(cursor.next("verifierName"), "verifierName")
            val verifierCircuitId =
                canonicalText(cursor.next("verifierCircuitId"), "verifierCircuitId")
            val activation = cursor.next("verifierActivationHeight").takeIf { it.isNotEmpty() }
                ?.let { longInteger(it, "verifierActivationHeight") }
            val withdrawal = cursor.next("verifierWithdrawalHeight").takeIf { it.isNotEmpty() }
                ?.let { longInteger(it, "verifierWithdrawalHeight") }
            val verifiedAtBlockHeight =
                longInteger(cursor.next("verifiedAtBlockHeight"), "verifiedAtBlockHeight")
            val verifiedAtMilliseconds =
                longInteger(cursor.next("verifiedAtMilliseconds"), "verifiedAtMilliseconds")
            val claimCount = projectionCount(cursor.next("branchClaimCount"), "branchClaim")
            val claims = List(claimCount) { BranchClaim(cursor.next("branchClaim[$it]")) }
            cursor.finish()
            return VerifyProjection(
                valid,
                chainAdmissible,
                lineageRedeemable,
                witnesslessRedemptionSupported,
                commitment,
                spendNullifier,
                amount,
                hopCount,
                proofStepCount,
                bundleDigest,
                assetDefinitionId,
                artifactBinding,
                requestDigest,
                outputBindingDigest,
                verifierBackend,
                verifierName,
                verifierCircuitId,
                activation,
                withdrawal,
                verifiedAtBlockHeight,
                verifiedAtMilliseconds,
                claims,
            )
        }

        /** Decode the authorization payload and optional spendable change of a V4 redemption. */
        @JvmStatic
        fun projectRedeemBuildResultV4(result: RedeemBuildResultV4): RedeemBuildProjection {
            requireArtifactBridge()
            val cursor = ProjectionCursor(
                nativeProjectRedeemBuildResultV4(result.noritoEncoded()),
                "V4 redeem build projection",
            )
            projectionVersion(cursor.next("version"), "V4 redeem build projection")
            val unsigned = RedeemUnsignedV4(cursor.next("unsigned"))
            val authorizationDigest = cursor.next("authorizationDigest")
            val operationId = cursor.next("operationId")
            val change = if (bool(cursor.next("changePresent"), "changePresent")) {
                Pair(
                    TopUpProvenanceV4(cursor.next("changeTopUpProvenance")),
                    branchProjection(cursor),
                )
            } else {
                null
            }
            cursor.finish()
            return RedeemBuildProjection(
                unsigned,
                authorizationDigest,
                change?.second,
                change?.first,
                operationId,
            )
        }

        @JvmStatic
        fun buildVerifyRequestV4(
            bundle: BundleV4,
            recipientRequest: RecipientPaymentRequest,
            topUpProvenance: TopUpProvenanceV4,
            maximumHops: Int,
            blockHeight: Long,
            verifiedAtMilliseconds: Long,
        ): VerifyRequestV4 {
            requireArtifactBridge()
            requireV4ProofBackend()
            return VerifyRequestV4(nativeBuildVerifyRequestV4(
                bundle.noritoEncoded(),
                recipientRequest.noritoEncoded(),
                topUpProvenance.noritoEncoded(),
                maximumHops,
                blockHeight,
                verifiedAtMilliseconds,
            ))
        }

        @JvmStatic
        fun buildRedeemRequestV5(
            input: SpendableBranchV4,
            recipientAccountId: String,
            chainDiscriminant: Int,
            amount: KagemushaScaledAmount,
            changeOpening: NoteOpening?,
            changeOutputMembershipPaths: OutputMembershipPaths?,
            unshieldVerifierCommitment: ByteArray,
            nonce: ByteArray,
            blockHeight: Long,
        ): RedeemRequestV5 = transferChangeOpeningOwnership(changeOpening) { ownedChangeOpening ->
            requireArtifactBridge()
            requireV4ProofBackend()
            require((ownedChangeOpening != null) == (changeOutputMembershipPaths != null)) {
                "change output membership must be present exactly when changeOpening is present"
            }
            changeOutputMembershipPaths?.let {
                require(it.recipient == null && it.change != null) {
                    "redemption change requires exactly one change output path"
                }
            }
            var change: ByteArray? = null
            var outputMembership: ByteArray? = null
            var verifier: ByteArray? = null
            var nonceBytes: ByteArray? = null
            var bundleArchive: ByteArray? = null
            var topUpProvenanceArchive: ByteArray? = null
            var openingArchive: ByteArray? = null
            var witnessArchive: ByteArray? = null
            var recipient: ByteArray? = null
            var atomicUnits: ByteArray? = null
            var archive: ByteArray? = null
            try {
                change = ownedChangeOpening?.noritoEncoded() ?: byteArrayOf()
                outputMembership = changeOutputMembershipPaths?.nativeArchive() ?: byteArrayOf()
                verifier = requireDigest(
                    unshieldVerifierCommitment,
                    "unshieldVerifierCommitment",
                )
                nonceBytes = requireDigest(nonce, "nonce")
                bundleArchive = input.bundle.noritoEncoded()
                topUpProvenanceArchive = input.topUpProvenance.noritoEncoded()
                openingArchive = input.opening.noritoEncoded()
                witnessArchive = input.membershipWitness.noritoEncoded()
                recipient = utf8(recipientAccountId, "recipientAccountId")
                atomicUnits = utf8(amount.atomicUnits, "atomicUnits")
                archive = nativeBuildRedeemRequestV5(
                    checkNotNull(bundleArchive),
                    checkNotNull(topUpProvenanceArchive),
                    checkNotNull(openingArchive),
                    checkNotNull(witnessArchive),
                    checkNotNull(recipient),
                    requireChainDiscriminant(chainDiscriminant),
                    checkNotNull(atomicUnits),
                    amount.scale,
                    checkNotNull(change),
                    checkNotNull(outputMembership),
                    checkNotNull(verifier),
                    checkNotNull(nonceBytes),
                    blockHeight,
                )
                return@transferChangeOpeningOwnership RedeemRequestV5(
                    checkNotNull(archive),
                    ownedChangeOpening,
                )
            } finally {
                SecretArchiveWiper.wipe(change)
                SecretArchiveWiper.wipe(outputMembership)
                SecretArchiveWiper.wipe(verifier)
                SecretArchiveWiper.wipe(nonceBytes)
                SecretArchiveWiper.wipe(bundleArchive)
                SecretArchiveWiper.wipe(topUpProvenanceArchive)
                SecretArchiveWiper.wipe(openingArchive)
                SecretArchiveWiper.wipe(witnessArchive)
                SecretArchiveWiper.wipe(recipient)
                SecretArchiveWiper.wipe(atomicUnits)
                SecretArchiveWiper.wipe(archive)
            }
        }

        @JvmStatic
        fun prepareAcknowledgement(
            request: RecipientPaymentRequest,
            payment: PeerPayment,
            acceptedAtMilliseconds: Long,
        ): AcknowledgementPreparation {
            requireArtifactBridge()
            val fields = nativePrepareAcknowledgementV2(
                request.noritoEncoded(), payment.noritoEncoded(), acceptedAtMilliseconds,
            )
            requireFieldCount(fields, 6, "acknowledgement preparation")
            return AcknowledgementPreparation(
                AcknowledgementPayload(fields[0]), fields[1], fields[2], fields[3], fields[4], fields[5],
            )
        }

        @JvmStatic
        fun signAcknowledgement(
            preparation: AcknowledgementPreparation,
            signature: KagemushaDeviceSignatureV2,
            request: RecipientPaymentRequest,
            payment: PeerPayment,
        ): ReceiverAcknowledgement {
            requireArtifactBridge()
            return ReceiverAcknowledgement(
                nativeCreateAcknowledgementV2(
                    preparation.payload.noritoEncoded(), signature.rawBytes(),
                    request.noritoEncoded(), payment.noritoEncoded(),
                ),
            )
        }

        /**
         * Verifies a receiver-signed delivery receipt. Under cash_handoff_v1
         * this is never a sender commit, acceptance, rollback, or clawback gate.
         */
        @JvmStatic
        fun verifyAcknowledgement(
            acknowledgement: ReceiverAcknowledgement,
            request: RecipientPaymentRequest,
            payment: PeerPayment,
        ): AcknowledgementVerification {
            requireArtifactBridge()
            val fields = nativeVerifyAcknowledgementV2(
                acknowledgement.noritoEncoded(), request.noritoEncoded(), payment.noritoEncoded(),
            )
            requireFieldCount(fields, 5, "acknowledgement verification")
            return AcknowledgementVerification(
                bool(fields[0], "valid"), fields[1], fields[2], fields[3], fields[4],
            )
        }

        /** Build the first spendable branch from a finalized top-up anchor. */
        @JvmStatic
        fun initSpendV4(request: InitRequestV4): InitResultV4 {
            return withHeavyProofPermit("init spend") {
                var secretArchive: ByteArray? = null
                var terminal = true
                try {
                    requireProofBackend()
                    val borrowed = request.borrowForNative().also { secretArchive = it }
                    InitResultV4(callNativeLifecycle("init spend") {
                        nativeInitSpendV4(borrowed)
                    })
                } catch (failure: ProofWorkerBusyException) {
                    terminal = false
                    throw failure
                } finally {
                    SecretArchiveWiper.wipe(secretArchive)
                    if (terminal) request.close()
                }
            }
        }

        /** Prove one exact recipient output and optional independently spendable sender change. */
        @JvmStatic
        fun appendSpendV4(
            request: AppendRequestV4,
            recipientRequest: RecipientPaymentRequest,
            verifiedAtMilliseconds: Long,
        ): SplitResultV4 = withHeavyProofPermit("append spend") {
            var secretArchive: ByteArray? = null
            var terminal = true
            try {
                require(verifiedAtMilliseconds > 0) {
                    "verifiedAtMilliseconds must be positive"
                }
                requireProofBackend()
                val borrowed = request.borrowForNative().also { secretArchive = it }
                val resultArchive = callNativeLifecycle("append spend") {
                    nativeAppendSpendV4(
                        borrowed,
                        recipientRequest.noritoEncoded(),
                        verifiedAtMilliseconds,
                    )
                }
                transferChangeOpeningOwnership(request.takeChangeOpening()) { opening ->
                    SplitResultV4(resultArchive, opening)
                }
            } catch (failure: ProofWorkerBusyException) {
                terminal = false
                throw failure
            } finally {
                SecretArchiveWiper.wipe(secretArchive)
                if (terminal) request.close()
            }
        }

        /** Verify the recursive proof, exact split bindings, membership, and hop limit. */
        @JvmStatic
        fun verifySpendV4(request: VerifyRequestV4): VerifyResultV4 {
            return withHeavyProofPermit("verify spend") {
                requireProofBackend()
                VerifyResultV4(callNativeLifecycle("verify spend") {
                    nativeVerifySpendV4(request.borrowForNative())
                })
            }
        }

        /** Build a full or partial redemption and its optional proof-bound offline change. */
        @JvmStatic
        fun buildRedeemV4(request: RedeemRequestV5): RedeemBuildResultV4 =
            withHeavyProofPermit("build redeem") {
                var secretArchive: ByteArray? = null
                var terminal = true
                try {
                    requireProofBackend()
                    val borrowed = request.borrowForNative().also { secretArchive = it }
                    val resultArchive = callNativeLifecycle("build redeem") {
                        nativeBuildRedeemV4(borrowed)
                    }
                    transferChangeOpeningOwnership(request.takeChangeOpening()) { opening ->
                        RedeemBuildResultV4(resultArchive, opening)
                    }
                } catch (failure: ProofWorkerBusyException) {
                    terminal = false
                    throw failure
                } finally {
                    SecretArchiveWiper.wipe(secretArchive)
                    if (terminal) request.close()
                }
            }

        @JvmStatic
        fun newToriiClient(
            baseUri: URI,
            transport: TransportExecutor,
            localSigningContext: LocalSigningContext,
        ): ToriiClient = newToriiClient(baseUri, transport, localSigningContext, null)

        /** Build a Kagemusha Torii client with an optional per-request timeout. */
        @JvmStatic
        fun newToriiClient(
            baseUri: URI,
            transport: TransportExecutor,
            localSigningContext: LocalSigningContext,
            requestTimeout: Duration?,
        ): ToriiClient = ToriiClient(baseUri, transport, localSigningContext, requestTimeout)

        internal fun isExactBridgeAbi(abiVersion: Int): Boolean =
            abiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION

        internal fun detectExactNativeAvailability(
            loadLibrary: () -> Unit,
            abiVersion: () -> Int,
            contractRevision: () -> Int,
            symbolProbe: () -> Boolean,
        ): Boolean = try {
            loadLibrary()
            isExactBridgeAbi(abiVersion()) &&
                contractRevision() == REQUIRED_KAGEMUSHA_NATIVE_CONTRACT_REVISION &&
                symbolProbe()
        } catch (_: UnsatisfiedLinkError) {
            false
        } catch (_: SecurityException) {
            false
        } catch (_: RuntimeException) {
            false
        }

        private fun loadArtifactBridge(): Boolean =
            detectExactNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                abiVersion = { nativeBridgeAbiVersion() },
                contractRevision = { nativeKagemushaContractRevision() },
                symbolProbe = {
                    expectRejectedSymbolProbe {
                        nativeArtifactBeginV4(byteArrayOf(0), ByteArray(32), ByteArray(32))
                    }
                },
            )

        /*
         * The symbol exists in both lifecycle states. Before production promotion native rejects
         * the probe as unavailable; after promotion it rejects the malformed manifest. Either
         * typed rejection proves linkage. Only an absent JNI symbol must make the bridge false.
         */
        private fun expectRejectedSymbolProbe(probe: () -> Unit): Boolean = try {
            probe()
            false
        } catch (_: IllegalArgumentException) {
            true
        } catch (_: IllegalStateException) {
            true
        }

        internal fun detectProductionProofBackendCompilation(probe: () -> Unit): Boolean = try {
            probe()
            false
        } catch (_: IllegalArgumentException) {
            // Production promotion passed and native reached malformed-artifact validation.
            true
        } catch (_: IllegalStateException) {
            // The symbol is linked, but the default/candidate build rejected production use.
            false
        } catch (_: UnsatisfiedLinkError) {
            false
        } catch (_: SecurityException) {
            false
        }

        private fun requireArtifactBridge() {
            requireV4ArtifactBridge()
        }

        private fun requireV4ArtifactBridge() {
            check(artifactBridgeAvailable) {
                "$LIBRARY_NAME ABI $REQUIRED_NATIVE_BRIDGE_ABI_VERSION artifact streaming is unavailable"
            }
        }

        private fun requireProofBackend() {
            requireV4ProofBackend()
        }

        private fun requireV4ProofBackend() {
            check(isProofBackendAvailable()) {
                "$LIBRARY_NAME ABI $REQUIRED_NATIVE_BRIDGE_ABI_VERSION Kagemusha proof backend is unavailable"
            }
        }

        private inline fun callNativeLifecycle(label: String, call: () -> ByteArray?): ByteArray =
            try {
                checkNotNull(call()) { "native Kagemusha $label returned no archive" }
            } catch (failure: IllegalStateException) {
                if (failure.message.orEmpty().contains(NATIVE_BUSY_MESSAGE)) {
                    throw ProofWorkerBusyException(
                        "Kagemusha $label is busy; retry after the active proof completes",
                        failure,
                    )
                }
                throw failure
            } catch (failure: UnsatisfiedLinkError) {
                throw IllegalStateException("native Kagemusha $label entrypoint is unavailable", failure)
            }

        private fun utf8(value: String?, field: String): ByteArray {
            require(value != null && value.isNotEmpty() && value == value.trim()) {
                "$field must be canonical non-empty text"
            }
            return value.toByteArray(Charsets.UTF_8)
        }

        private fun requireChainDiscriminant(value: Int): Int {
            require(value in 0..0xffff) { "chainDiscriminant must fit in u16" }
            return value
        }

        private fun requiredBytes(value: ByteArray?, field: String): ByteArray {
            require(value != null && value.isNotEmpty()) { "$field must not be empty" }
            return value.copyOf()
        }

        private fun requireFieldCount(fields: Array<ByteArray>?, expected: Int, label: String) {
            check(fields != null && fields.size == expected) {
                "native Kagemusha $label returned invalid fields"
            }
        }

        private fun requireIosAppAttestAuthenticatorDataProjection(authenticatorData: ByteArray) {
            check(
                authenticatorData.size in IOS_APP_ATTEST_AUTHENTICATOR_DATA_MIN_BYTES..
                    IOS_APP_ATTEST_AUTHENTICATOR_DATA_MAX_BYTES,
            ) {
                "native Kagemusha App Attest finalization returned invalid authenticator data"
            }
            val flags = authenticatorData[32].toInt() and 0xff
            check(flags == IOS_APP_ATTEST_EXTENSION_DATA_FLAG) {
                "native Kagemusha App Attest finalization must return extension-bearing authenticator data"
            }
        }

        private fun amount(atomic: ByteArray, scale: ByteArray): KagemushaScaledAmount =
            KagemushaScaledAmount.fromAtomicUnits(
                atomic.toString(Charsets.US_ASCII),
                integer(scale, "scale"),
            )

        private fun integer(value: ByteArray, field: String): Int =
            value.toString(Charsets.US_ASCII).toIntOrNull()
                ?: error("native Kagemusha $field is invalid")

        private fun longInteger(value: ByteArray, field: String): Long =
            value.toString(Charsets.US_ASCII).toLongOrNull()
                ?: error("native Kagemusha $field is invalid")

        private fun outputMembershipSiblings(
            flattened: ByteArray,
            field: String,
        ): List<ByteArray> {
            check(flattened.size == CONFIDENTIAL_TREE_DEPTH * 32) {
                "native Kagemusha $field has an invalid sibling count"
            }
            return List(CONFIDENTIAL_TREE_DEPTH) { index ->
                flattened.copyOfRange(index * 32, (index + 1) * 32)
            }
        }

        private fun outputMembershipPathFromNativeProjection(
            fields: Array<ByteArray>,
            leafIndex: Int,
            siblingsIndex: Int,
            directionsIndex: Int,
            rootIndex: Int,
            field: String,
        ): OutputMembershipPath = try {
            OutputMembershipPath(
                leafIndex,
                outputMembershipSiblings(fields[siblingsIndex], "$field.siblings"),
                fields[directionsIndex],
                fields[rootIndex],
            )
        } catch (failure: IllegalArgumentException) {
            throw IllegalStateException("native Kagemusha $field is invalid", failure)
        }

        private fun outputMembershipLeafFromNativeProjection(
            fields: Array<ByteArray>,
            offset: Int,
            field: String,
        ): OutputMembershipLeafPaths? {
            val values = fields.sliceArray(offset until offset + 7)
            if (values.all { it.isEmpty() }) return null
            check(values.all { it.isNotEmpty() }) {
                "native Kagemusha $field is only partially present"
            }
            val leafIndex = integer(fields[offset], "$field.leafIndex")
            return OutputMembershipLeafPaths(
                outputMembershipPathFromNativeProjection(
                    fields,
                    leafIndex,
                    offset + 1,
                    offset + 2,
                    offset + 3,
                    "$field.updatePath",
                ),
                outputMembershipPathFromNativeProjection(
                    fields,
                    leafIndex,
                    offset + 4,
                    offset + 5,
                    offset + 6,
                    "$field.membershipPath",
                ),
            )
        }

        private fun outputMembershipPathsFromNativeProjection(
            fields: Array<ByteArray>,
        ): OutputMembershipPaths {
            requireFieldCount(fields, 21, "V4 output membership derivation")
            val recipient = outputMembershipLeafFromNativeProjection(fields, 3, "recipient")
            val change = outputMembershipLeafFromNativeProjection(fields, 10, "change")
            check((17..20).all { fields[it].isNotEmpty() }) {
                "native Kagemusha dummy output membership path is absent"
            }
            val dummyLeafIndex = integer(fields[17], "dummy.leafIndex")
            val dummy = outputMembershipPathFromNativeProjection(
                fields,
                dummyLeafIndex,
                18,
                19,
                20,
                "dummy.path",
            )
            return try {
                OutputMembershipPaths(
                    fields[1],
                    fields[2],
                    recipient,
                    change,
                    dummy,
                    fields[0],
                )
            } catch (failure: IllegalArgumentException) {
                throw IllegalStateException(
                    "native Kagemusha V4 output membership derivation is invalid",
                    failure,
                )
            }
        }

        private fun canonicalText(value: ByteArray, field: String): String {
            val text = value.toString(Charsets.UTF_8)
            check(text.isNotEmpty() && text == text.trim() && text.none(Char::isISOControl)) {
                "native Kagemusha $field is invalid"
            }
            return text
        }

        private fun bool(value: ByteArray, field: String): Boolean {
            check(value.size == 1 && (value[0] == 0.toByte() || value[0] == 1.toByte())) {
                "native Kagemusha $field is invalid"
            }
            return value[0] == 1.toByte()
        }

        private fun projectionVersion(value: ByteArray, field: String) {
            check(
                value.size == 4 &&
                    value[0] == 0.toByte() && value[1] == 0.toByte() &&
                    value[2] == 0.toByte() &&
                    (value[3].toInt() and 0xff) == EXACT_STATE_PROJECTION_VERSION,
            ) { "native Kagemusha $field version is unsupported" }
        }

        private fun projectionCount(value: ByteArray, field: String): Int {
            check(value.size == 4) { "native Kagemusha $field count is invalid" }
            val count = value.fold(0L) { result, octet ->
                (result shl 8) or (octet.toLong() and 0xff)
            }
            check(count in 1..MAXIMUM_BRANCH_CLAIMS.toLong()) {
                "native Kagemusha $field count is outside the exact-state limit"
            }
            return count.toInt()
        }

        private class ProjectionCursor(
            private val fields: Array<ByteArray>,
            private val label: String,
            start: Int = 0,
        ) {
            var index: Int = start
                private set

            fun next(field: String): ByteArray {
                check(index < fields.size) { "native Kagemusha $label omitted $field" }
                return fields[index++]
            }

            fun finish() {
                check(index == fields.size) { "native Kagemusha $label has trailing fields" }
            }
        }

        private fun branchProjection(cursor: ProjectionCursor): BranchProjection {
            val bundle = BundleV4(cursor.next("bundle"))
            val witness = NoteMembershipWitness(cursor.next("membershipWitness"))
            val commitment = cursor.next("commitment")
            val spendNullifier = cursor.next("spendNullifier")
            val amount = amount(cursor.next("atomicUnits"), cursor.next("scale"))
            val hopCount = integer(cursor.next("hopCount"), "hopCount")
            val proofStepCount = integer(cursor.next("proofStepCount"), "proofStepCount")
            val bundleDigest = cursor.next("bundleDigest")
            val artifactBinding = ArtifactBindingV4(cursor.next("artifactBinding"))
            val claimCount = projectionCount(cursor.next("branchClaimCount"), "branchClaim")
            val claims = List(claimCount) { BranchClaim(cursor.next("branchClaim[$it]")) }
            return BranchProjection(
                bundle, witness, commitment, spendNullifier, amount, hopCount,
                proofStepCount, bundleDigest, artifactBinding, claims,
            )
        }

        private fun requireManifest(value: ByteArray?): ByteArray {
            require(value != null && value.isNotEmpty() && value.size <= MAX_MANIFEST_BYTES) {
                "manifestNorito must contain 1..$MAX_MANIFEST_BYTES bytes"
            }
            return value.copyOf()
        }

        private fun requireDigest(value: ByteArray?, name: String): ByteArray {
            require(value != null && value.size == 32) { "$name must contain exactly 32 bytes" }
            require(value.any { it.toInt() != 0 }) { "$name must be non-zero" }
            return value.copyOf()
        }

        private fun requireMarkedDigest(value: ByteArray?, name: String): ByteArray =
            requireDigest(value, name).also { digest ->
                if (digest.last().toInt() and 1 == 0) {
                    digest.fill(0)
                    throw IllegalArgumentException("$name must preserve the Iroha hash marker")
                }
            }

        private data class CompactFieldRange(val start: Int, val end: Int) {
            val size: Int get() = end - start
        }

        private fun operationIdentityFromProjection(
            fields: Array<ByteArray>,
            offset: Int,
        ): OperationIdentity = OperationIdentity(
            requireMarkedDigest(fields[offset], "operationId"),
            requireMarkedDigest(fields[offset + 1], "requestAuthorityDigest"),
            requireMarkedDigest(fields[offset + 2], "canonicalRequestDigest"),
            operationKind(canonicalText(fields[offset + 3], "operationKind")),
            longInteger(fields[offset + 4], "issuedAtMilliseconds"),
            longInteger(fields[offset + 5], "expiresAtMilliseconds"),
        )

        private fun operationIdentityFromCanonicalRequest(
            archive: ByteArray,
            schema: String,
            kind: OperationKind,
            fieldCount: Int,
            operationIdFieldIndex: Int,
        ): OperationIdentity {
            val decoded = NoritoHeader.decode(archive, SchemaHash.hash16(schema))
            val payload = decoded.payload
            var canonicalAuthorityArchive: ByteArray? = null
            var operationId: ByteArray? = null
            var authorityDigest: ByteArray? = null
            var requestDigest: ByteArray? = null
            var nonce: ByteArray? = null
            return try {
                val requestFields = compactFields(
                    payload,
                    0,
                    payload.size,
                    fieldCount,
                    "$schema request",
                )
                val version = requestFields[0]
                require(version.size == 2 &&
                    (payload[version.start].toInt() and 0xff) == KAGEMUSHA_REQUEST_WIRE_VERSION_V4 &&
                    payload[version.start + 1].toInt() == 0) {
                    "$schema must carry wire version $KAGEMUSHA_REQUEST_WIRE_VERSION_V4"
                }
                val authorization = requestFields.last()
                val authorizationFields = compactFields(
                    payload,
                    authorization.start,
                    authorization.end,
                    10,
                    "$schema authorization",
                )
                val authority = authorizationFields[0]
                require(authority.size > 0) { "$schema authorization authority must not be empty" }
                requireCanonicalAuthorizationText(payload, authorizationFields[1], "device_id", 128)
                require(authorizationFields[2].size > 0) {
                    "$schema authorization asset_definition_id must not be empty"
                }

                val outerOperation = copyField(payload, requestFields[operationIdFieldIndex])
                operationId = copyField(payload, authorizationFields[3])
                val ownedOperationId = requireMarkedDigest(operationId, "operationId")
                operationId?.fill(0)
                operationId = ownedOperationId
                require(outerOperation.contentEquals(ownedOperationId)) {
                    "$schema authorization operation_id must equal the outer operation_id"
                }
                outerOperation.fill(0)

                val issuedAt = unsignedLong(payload, authorizationFields[4], "issued_at_ms")
                val expiresAt = unsignedLong(payload, authorizationFields[5], "expires_at_ms")
                require(issuedAt > 0) { "$schema authorization issued_at_ms must be positive" }
                require(expiresAt > issuedAt &&
                    expiresAt - issuedAt <= KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2) {
                    "$schema authorization lifetime is invalid"
                }

                nonce = copyField(payload, authorizationFields[6])
                require(nonce?.size == 32 && nonce?.any { it.toInt() != 0 } == true) {
                    "$schema authorization nonce must be exactly 32 non-zero bytes"
                }
                require(authorizationFields[7].size == 32) {
                    "$schema authorization payload_digest must be exactly 32 bytes"
                }
                val registrationHash = copyField(payload, authorizationFields[8])
                requireMarkedDigest(registrationHash, "registrationHash").fill(0)
                registrationHash.fill(0)
                require(authorizationFields[9].size > 0) {
                    "$schema authorization hardware_assertion must not be empty"
                }

                canonicalAuthorityArchive = canonicalAccountIdArchive(payload, authority)
                val expectedOperationId = deriveOperationIdV4(
                    checkNotNull(canonicalAuthorityArchive),
                    checkNotNull(nonce),
                )
                require(ownedOperationId.contentEquals(expectedOperationId)) {
                    "$schema authorization operation_id is not derived from authority and nonce"
                }
                expectedOperationId.fill(0)

                authorityDigest = markedBlake2b256(
                    KAGEMUSHA_OPERATION_AUTHORITY_DIGEST_DOMAIN_V4,
                    littleEndianLength(checkNotNull(canonicalAuthorityArchive).size),
                    checkNotNull(canonicalAuthorityArchive),
                )
                requestDigest = markedBlake2b256(
                    KAGEMUSHA_OPERATION_REQUEST_DIGEST_DOMAIN_V4,
                    when (kind) {
                        OperationKind.TOP_UP -> "top_up".toByteArray(StandardCharsets.UTF_8)
                        OperationKind.REDEEM -> "redeem".toByteArray(StandardCharsets.UTF_8)
                    },
                    littleEndianLength(archive.size),
                    archive,
                )
                OperationIdentity(
                    ownedOperationId,
                    checkNotNull(authorityDigest),
                    checkNotNull(requestDigest),
                    kind,
                    issuedAt,
                    expiresAt,
                )
            } finally {
                nonce?.fill(0)
                requestDigest?.fill(0)
                authorityDigest?.fill(0)
                operationId?.fill(0)
                canonicalAuthorityArchive?.fill(0)
                payload.fill(0)
            }
        }

        internal fun operationIdentityFromCanonicalRequestForTest(
            archive: ByteArray,
            kind: OperationKind,
        ): OperationIdentity = when (kind) {
            OperationKind.TOP_UP -> operationIdentityFromCanonicalRequest(
                archive,
                "iroha.torii.v1.offline.top_up.request",
                kind,
                8,
                6,
            )
            OperationKind.REDEEM -> operationIdentityFromCanonicalRequest(
                archive,
                "iroha.torii.v1.offline.redeem.request",
                kind,
                10,
                8,
            )
        }

        private fun deriveOperationIdV4(
            canonicalAuthorityArchive: ByteArray,
            nonce: ByteArray,
        ): ByteArray {
            require(nonce.size == 32 && nonce.any { it.toInt() != 0 }) {
                "nonce must be exactly 32 non-zero bytes"
            }
            return markedBlake2b256(
                KAGEMUSHA_OPERATION_ID_DOMAIN_V4,
                littleEndianLength(canonicalAuthorityArchive.size),
                canonicalAuthorityArchive,
                nonce,
            )
        }

        private fun canonicalAccountIdArchive(
            source: ByteArray,
            field: CompactFieldRange,
        ): ByteArray {
            val payload = copyField(source, field)
            return try {
                val header = NoritoHeader(
                    SchemaHash.hash16("iroha_data_model::account::model::AccountId"),
                    payload.size,
                    CRC64.compute(payload),
                    NoritoHeader.COMPACT_LEN,
                    NoritoHeader.COMPRESSION_NONE,
                )
                header.encode() + payload
            } finally {
                payload.fill(0)
            }
        }

        private fun markedBlake2b256(vararg chunks: ByteArray): ByteArray {
            val digest = Blake2bDigest(256)
            chunks.forEach { chunk -> digest.update(chunk, 0, chunk.size) }
            return ByteArray(32).also { output ->
                digest.doFinal(output, 0)
                output[output.lastIndex] = (output.last().toInt() or 1).toByte()
            }
        }

        private fun littleEndianLength(value: Int): ByteArray = ByteArray(Long.SIZE_BYTES).also {
            var remaining = value.toLong()
            for (index in it.indices) {
                it[index] = remaining.toByte()
                remaining = remaining ushr 8
            }
        }

        private fun compactFields(
            source: ByteArray,
            start: Int,
            end: Int,
            count: Int,
            context: String,
        ): List<CompactFieldRange> {
            require(start in 0..end && end <= source.size) { "$context bounds are invalid" }
            val fields = ArrayList<CompactFieldRange>(count)
            var cursor = start
            repeat(count) { index ->
                var length = 0L
                var shift = 0
                var octets = 0
                while (true) {
                    require(cursor < end && octets < 10) {
                        "$context field $index compact length is truncated or overflows u64"
                    }
                    val octet = source[cursor++].toInt() and 0xff
                    val chunk = octet and 0x7f
                    require(shift < 63 || chunk <= 1) {
                        "$context field $index compact length overflows u64"
                    }
                    length = length or (chunk.toLong() shl shift)
                    octets++
                    if (octet and 0x80 == 0) {
                        require(octets == 1 || chunk != 0) {
                            "$context field $index compact length is non-canonical"
                        }
                        break
                    }
                    shift += 7
                }
                require(length <= Int.MAX_VALUE.toLong() && length <= (end - cursor).toLong()) {
                    "$context field $index is truncated"
                }
                val fieldEnd = cursor + length.toInt()
                fields += CompactFieldRange(cursor, fieldEnd)
                cursor = fieldEnd
            }
            require(cursor == end) { "$context must contain exactly $count fields" }
            return fields
        }

        private fun copyField(source: ByteArray, field: CompactFieldRange): ByteArray =
            source.copyOfRange(field.start, field.end)

        private fun unsignedLong(
            source: ByteArray,
            field: CompactFieldRange,
            name: String,
        ): Long {
            require(field.size == Long.SIZE_BYTES &&
                source[field.end - 1].toInt() and 0x80 == 0) {
                "$name must be one positive signed-range u64"
            }
            var value = 0L
            for (index in 0 until Long.SIZE_BYTES) {
                value = value or
                    ((source[field.start + index].toLong() and 0xffL) shl (index * 8))
            }
            return value
        }

        private fun requireCanonicalAuthorizationText(
            source: ByteArray,
            field: CompactFieldRange,
            name: String,
            maximumBytes: Int,
        ) {
            require(field.size in 1..maximumBytes) { "$name has an invalid byte length" }
            val bytes = copyField(source, field)
            try {
                val text = String(bytes, StandardCharsets.UTF_8)
                require(text.toByteArray(StandardCharsets.UTF_8).contentEquals(bytes) &&
                    text == text.trim() && text.none(Char::isISOControl)) {
                    "$name must be canonical UTF-8 text"
                }
            } finally {
                bytes.fill(0)
            }
        }

        private fun requireFinalityCheckpointContext(value: ByteArray?, name: String): ByteArray =
            requireDigest(value, name).also { context ->
                if (context.last().toInt() and 1 != 1) {
                    context.fill(0)
                    throw IllegalArgumentException("$name must preserve the Iroha hash marker")
                }
            }

        private fun requireTransactionHash(value: ByteArray?, name: String): ByteArray =
            requireDigest(value, name).also { hash ->
                if (hash.last().toInt() and 1 != 1) {
                    hash.fill(0)
                    throw IllegalArgumentException("$name must preserve the Iroha hash marker")
                }
            }

        private fun requireBoundedBytes(
            value: ByteArray?,
            name: String,
            maximumBytes: Int,
        ): ByteArray {
            require(value != null && value.isNotEmpty() && value.size <= maximumBytes) {
                "$name must contain 1..$maximumBytes bytes"
            }
            return value.copyOf()
        }

        internal fun requireChunk(value: ByteArray?): ByteArray {
            require(value != null && value.isNotEmpty() && value.size <= MAX_ARTIFACT_CHUNK_BYTES) {
                "chunk must contain 1..$MAX_ARTIFACT_CHUNK_BYTES bytes"
            }
            return value.copyOf()
        }

        private fun requireCanonicalArchive(
            value: ByteArray?,
            schema: String,
            field: String,
            maximumBytes: Int,
        ): ByteArray {
            require(value != null && value.isNotEmpty() && value.size <= maximumBytes) {
                "$field must contain 1..$maximumBytes bytes"
            }
            val archive = value.copyOf()
            return try {
                val decoded = try {
                    NoritoHeader.decode(archive, SchemaHash.hash16(schema))
                } catch (failure: RuntimeException) {
                    throw IllegalArgumentException("$field must contain canonical $schema", failure)
                }
                val header = decoded.header
                require(
                    header.compression == NoritoHeader.COMPRESSION_NONE &&
                        header.flags == NoritoHeader.COMPACT_LEN &&
                        decoded.payload.isNotEmpty() &&
                        archive.size == NoritoHeader.HEADER_LENGTH +
                            peerArchivePadding(schema) + decoded.payload.size &&
                        header.encode().contentEquals(
                            archive.copyOfRange(0, NoritoHeader.HEADER_LENGTH),
                        ),
                ) { "$field must use canonical compact Norito framing" }
                header.validateChecksum(decoded.payload)
                archive
            } catch (failure: Throwable) {
                archive.fill(0)
                throw failure
            }
        }

        private fun peerArchivePadding(schema: String): Int = when (schema) {
            "iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2",
            "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2",
            "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4",
            "iroha.torii.v1.offline.top_up.request",
            "iroha.torii.v1.offline.redeem.request" -> 8
            "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2" -> 0
            else -> 0
        }

        private fun hex(digest: ByteArray): String = buildString(64) {
            for (octet in digest) append("%02x".format(octet.toInt() and 0xff))
        }

        private fun hasWellFormedUtf16(value: String): Boolean {
            var index = 0
            while (index < value.length) {
                val character = value[index]
                when {
                    Character.isHighSurrogate(character) -> {
                        if (
                            index + 1 >= value.length ||
                            !Character.isLowSurrogate(value[index + 1])
                        ) {
                            return false
                        }
                        index += 2
                    }
                    Character.isLowSurrogate(character) -> return false
                    else -> index++
                }
            }
            return true
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativeKagemushaContractRevision(): Int

        @JvmStatic
        private external fun nativePastaCycleV4BackendAvailable(): Boolean

        @JvmStatic
        private external fun nativeArtifactBeginV4(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            expectedArtifactSha256: ByteArray,
        ): Long

        @JvmStatic
        private external fun nativeArtifactWriteV4(handle: Long, chunk: ByteArray)

        @JvmStatic
        private external fun nativeArtifactFinalizeV4(handle: Long)

        @JvmStatic
        private external fun nativeArtifactCancelV4(handle: Long)

        @JvmStatic
        private external fun nativeArtifactSetInstallV4(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            trustedPolicyNorito: ByteArray,
            releaseAttestationNorito: ByteArray,
            internalValidationReceiptNorito: ByteArray,
            benchmarkEvidence: ByteArray,
            cryptographicReview: ByteArray,
            promotionRecordNorito: ByteArray,
            artifactHandles: LongArray,
        )

        @JvmStatic
        private external fun nativeArtifactSetIsInstalledV4(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
        ): Boolean

        @JvmStatic
        private external fun nativeInstalledManifestSha256V4(): ByteArray

        @JvmStatic
        private external fun nativeBuildArtifactBindingV4(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
        ): ByteArray

        @JvmStatic
        private external fun nativeArtifactSetUninstallV4(manifestSha256: ByteArray)

        @JvmStatic
        private external fun nativeInitSpendV4(requestNorito: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeAppendSpendV4(
            requestNorito: ByteArray,
            recipientRequestNorito: ByteArray,
            verifiedAtMilliseconds: Long,
        ): ByteArray?

        @JvmStatic
        private external fun nativeVerifySpendV4(requestNorito: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeBuildRedeemV4(requestNorito: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativePrepareRecipientRequestV2(
            networkId: ByteArray,
            chainDiscriminant: Int,
            asset: ByteArray,
            atomicUnits: ByteArray,
            scale: Int,
            recipient: ByteArray,
            receiverDeviceId: ByteArray,
            receiverPublicKey: ByteArray,
            requestId: ByteArray,
            issuedAtMilliseconds: Long,
            expiresAtMilliseconds: Long,
            spendKey: ByteArray,
            rho: ByteArray,
            diversifier: ByteArray,
        ): Array<ByteArray>

        @JvmStatic private external fun nativeCreateRecipientRequestV2(payload: ByteArray, signature: ByteArray): ByteArray
        @JvmStatic private external fun nativeVerifyRecipientRequestV2(request: ByteArray, verifiedAtMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeCreateRecipientLineageQueryV2(networkId: ByteArray, chainDiscriminant: Int, recipient: ByteArray, receiverDeviceId: ByteArray, asset: ByteArray, trustedCheckpointHeight: Long): ByteArray
        @JvmStatic private external fun nativeVerifyRecipientRegistrationLineageV2(request: ByteArray, lineage: ByteArray, verifiedAtMilliseconds: Long, trustedCheckpointHeight: Long, trustedCheckpointContextId: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeCreateRecipientReceiveOfferV2(request: ByteArray, lineage: ByteArray, publisherCheckpointEnvelope: ByteArray): ByteArray
        @JvmStatic private external fun nativeProjectRecipientReceiveOfferV2(offer: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeVerifyRecipientReceiveOfferV2(offer: ByteArray, verifiedAtMilliseconds: Long, trustedCheckpointHeight: Long, trustedCheckpointContextId: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildOutputMembershipFrontierV4(leafIndex: Int, flattenedSiblings: ByteArray, directions: ByteArray, root: ByteArray): ByteArray
        @JvmStatic private external fun nativeDeriveOutputMembershipPathsV4(frontier: ByteArray, recipientCommitment: ByteArray, changeCommitment: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeValidateSpendableBranchV4(bundle: ByteArray, provenance: ByteArray, membershipWitness: ByteArray, opening: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeBuildOutputMembershipPathsV4(initialRoot: ByteArray, finalRoot: ByteArray, recipientFields: Array<ByteArray>, changeFields: Array<ByteArray>, dummyFields: Array<ByteArray>): ByteArray
        @JvmStatic private external fun nativeBuildInitRequestV4(anchor: ByteArray, proof: ByteArray, roster: ByteArray, opening: ByteArray, outputMembership: ByteArray): ByteArray
        @JvmStatic private external fun nativeBuildTopUpProvenanceV4(bundle: ByteArray, roster: ByteArray, anchors: Array<ByteArray>, finalityProofs: Array<ByteArray>, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeValidateTopUpProvenanceV4(bundle: ByteArray, provenance: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeBuildAppendRequestV4(bundles: Array<ByteArray>, topUpProvenances: Array<ByteArray>, openings: Array<ByteArray>, witnesses: Array<ByteArray>, changeOpening: ByteArray, outputMembership: ByteArray, verifierCommitment: ByteArray, operationId: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeProjectPeerPaymentV4(payment: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectInitResultV4(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectSplitResultV4(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildVerifyRequestV4(bundle: ByteArray, recipientRequest: ByteArray, topUpProvenance: ByteArray, maximumHops: Int, blockHeight: Long, verifiedAtMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeProjectVerifyResultV4(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildRedeemRequestV5(bundle: ByteArray, topUpProvenance: ByteArray, opening: ByteArray, membershipWitness: ByteArray, recipient: ByteArray, chainDiscriminant: Int, atomicUnits: ByteArray, scale: Int, changeOpening: ByteArray, changeOutputMembership: ByteArray, verifierCommitment: ByteArray, nonce: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeProjectRedeemBuildResultV4(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePrepareAcknowledgementV2(request: ByteArray, payment: ByteArray, acceptedAtMilliseconds: Long): Array<ByteArray>
        @JvmStatic private external fun nativeCreateAcknowledgementV2(payload: ByteArray, signature: ByteArray, request: ByteArray, payment: ByteArray): ByteArray
        @JvmStatic private external fun nativeVerifyAcknowledgementV2(acknowledgement: ByteArray, request: ByteArray, payment: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePrepareAuthorizationV3(authority: ByteArray, chainDiscriminant: Int, deviceId: ByteArray, assetDefinitionId: ByteArray, issuedAtMilliseconds: Long, expiresAtMilliseconds: Long, nonce: ByteArray, payloadDigest: ByteArray, registrationHash: ByteArray, hardwareAssertionPlatform: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeFinalizeHardwareAuthorizationV3(preparation: ByteArray, authenticatorData: ByteArray, signatureDer: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeFinalizeIosAppAttestAuthorizationV3(preparation: ByteArray, assertionObject: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeFinalizeTopUpV5(unsigned: ByteArray, authorization: ByteArray): ByteArray
        @JvmStatic private external fun nativeFinalizeRedeemV5(buildResult: ByteArray, authorization: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePrepareTopUpV5(networkId: ByteArray, chainDiscriminant: Int, assetDefinition: ByteArray, payer: ByteArray, atomicUnits: ByteArray, scale: Int, nonce: ByteArray, spendKey: ByteArray, rho: ByteArray, diversifier: ByteArray, leafIndex: Int, flattenedSiblings: ByteArray, directions: ByteArray, root: ByteArray, shieldVerifierCommitment: ByteArray, artifactBinding: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectTopUpRequestIdentityV4(request: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectRedeemRequestIdentityV4(request: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectOperationReferenceV2(reference: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectOperationStatusV2(status: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBranchClaimsConflictV2(left: ByteArray, right: ByteArray): Boolean
        @JvmStatic private external fun nativePrepareRedemptionChangeV5(bundle: ByteArray, inputOpening: ByteArray, atomicUnits: ByteArray, scale: Int, recipient: ByteArray, chainDiscriminant: Int, nonce: ByteArray, entropy: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePreparePeerSplitChangeV4(bundles: Array<ByteArray>, inputOpenings: Array<ByteArray>, recipientRequest: ByteArray, atomicUnits: ByteArray, scale: Int, operationId: ByteArray, entropy: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePrepareNoteOpeningV2(spendKey: ByteArray, rho: ByteArray, diversifier: ByteArray): ByteArray
        @JvmStatic private external fun nativeProjectRecipientRequestV2(request: ByteArray): Array<ByteArray>
    }

    /** Null-safe zeroization shared by secret-bearing native request builders. */
    internal object SecretArchiveWiper {
        fun wipe(archive: ByteArray?) {
            archive?.fill(0)
        }

        fun wipeAll(archives: Array<out ByteArray?>?) {
            archives?.forEach(::wipe)
        }

        fun <T> withOpeningDigests(
            spendKey: ByteArray,
            spendKeyName: String,
            rho: ByteArray,
            rhoName: String,
            diversifier: ByteArray,
            diversifierName: String,
            observer: (ByteArray) -> Unit = {},
            action: (ByteArray, ByteArray, ByteArray) -> T,
        ): T {
            var spendKeyCopy: ByteArray? = null
            var rhoCopy: ByteArray? = null
            var diversifierCopy: ByteArray? = null
            return try {
                val ownedSpendKey = requireDigest(spendKey, spendKeyName)
                    .also {
                        spendKeyCopy = it
                        observer(it)
                    }
                val ownedRho = requireDigest(rho, rhoName)
                    .also {
                        rhoCopy = it
                        observer(it)
                    }
                val ownedDiversifier = requireDigest(diversifier, diversifierName)
                    .also {
                        diversifierCopy = it
                        observer(it)
                    }
                action(ownedSpendKey, ownedRho, ownedDiversifier)
            } finally {
                wipe(diversifierCopy)
                wipe(rhoCopy)
                wipe(spendKeyCopy)
            }
        }
    }

    /** Immutable canonical Norito archive; proof and accumulator bytes remain opaque. */
    abstract class CanonicalArchive internal constructor(
        archive: ByteArray,
        schema: String,
        field: String,
        maximumBytes: Int,
    ) {
        private val bytes = requireCanonicalArchive(archive, schema, field, maximumBytes)
        // Retain only the 32-bit collection bucket after zeroization, never a secret digest.
        private val equalityHashCode = bytes.contentHashCode()
        private var destroyed = false

        @Synchronized
        fun noritoEncoded(): ByteArray {
            check(!destroyed) { "canonical archive has been destroyed" }
            return bytes.copyOf()
        }

        /** Borrow one synchronized native-call copy without changing ownership. */
        @Synchronized
        internal fun borrowForNative(): ByteArray {
            check(!destroyed) { "canonical archive has been destroyed" }
            return bytes.copyOf()
        }

        @Synchronized
        internal fun consumeAndDestroy(): ByteArray {
            check(!destroyed) { "canonical archive has already been consumed" }
            val consumed = bytes.copyOf()
            bytes.fill(0)
            destroyed = true
            return consumed
        }

        @Synchronized
        internal fun destroy() {
            bytes.fill(0)
            destroyed = true
        }

        @Synchronized
        fun isDestroyed(): Boolean = destroyed

        final override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other == null || this::class != other::class) return false
            other as CanonicalArchive
            val identity = System.identityHashCode(this)
            val otherIdentity = System.identityHashCode(other)
            return when {
                identity < otherIdentity -> synchronized(this) {
                    synchronized(other) { liveContentEquals(other) }
                }
                identity > otherIdentity -> synchronized(other) {
                    synchronized(this) { liveContentEquals(other) }
                }
                else -> synchronized(EQUALITY_TIE_LOCK) {
                    synchronized(this) {
                        synchronized(other) { liveContentEquals(other) }
                    }
                }
            }
        }

        final override fun hashCode(): Int = equalityHashCode

        private fun liveContentEquals(other: CanonicalArchive): Boolean =
            !destroyed && !other.destroyed && bytes.contentEquals(other.bytes)

        private companion object {
            val EQUALITY_TIE_LOCK = Any()
        }
    }

    class RecipientPaymentRequest internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_data_model::offline::model::KagemushaRecipientPaymentRequestV2",
        "recipientPaymentRequest",
        MAX_PEER_ARCHIVE_BYTES_V2,
    )

    class RecipientLineageQueryV2 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_torii_shared::offline_api::OfflineRecipientLineageRequest",
        "recipientLineageQuery",
        MAX_PEER_ARCHIVE_BYTES_V2,
    )

    /** Portable proof material; it becomes trusted only through a V2 native verifier result. */
    class RecipientRegistrationLineage internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_torii_shared::offline_api::OfflineRecipientRegistrationLineage",
        "recipientRegistrationLineage",
        MAX_TORII_RECIPIENT_LINEAGE_RESPONSE_BYTES,
    )

    class RecipientReceiveOfferV2 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_torii_shared::offline_api::OfflineRecipientReceiveOfferV2",
        "recipientReceiveOffer",
        MAX_RECIPIENT_RECEIVE_OFFER_BYTES_V2,
    )

    class PeerPayment internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_data_model::offline::model::KagemushaRecursiveSpendPeerPaymentV4",
        "peerPayment",
        MAX_PEER_ARCHIVE_BYTES_V4,
    )

    class ReceiverAcknowledgement internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha_data_model::offline::model::KagemushaReceiverAcknowledgementV2",
        "receiverAcknowledgement",
        MAX_PEER_ARCHIVE_BYTES_V2,
    )

    /** Proof-bound output membership state carried atomically with an accepted branch. */
    class NoteMembershipWitness internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaNoteMembershipWitnessV2",
        "noteMembershipWitness",
        MAX_PEER_ARCHIVE_BYTES_V2,
    )

    /**
     * Encrypted local note opening; never send this archive to Torii or a peer.
     *
     * Use [close] (normally through `use`) as soon as ownership ends so the secret archive is
     * zeroized deterministically.
     */
    class NoteOpening internal constructor(archive: ByteArray) : CanonicalArchive(
            archive,
            "KagemushaNoteOpeningV2",
            "noteOpening",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        /** Zeroize this opening. Repeated closes are harmless. */
        override fun close() {
            destroy()
        }
    }

    private class ChangeOpeningOwner(changeOpening: NoteOpening?) : AutoCloseable {
        private var opening = changeOpening
        private var transferred = false
        private var closed = false

        @Synchronized
        fun take(): NoteOpening? {
            check(!closed) { "change-opening owner has been closed" }
            check(!transferred) { "change opening has already been transferred" }
            transferred = true
            val ownedOpening = opening
            opening = null
            return ownedOpening
        }

        @Synchronized
        override fun close() {
            if (closed) return
            opening?.destroy()
            opening = null
            closed = true
        }
    }

    /** Owns native-derived redemption change secrets until [takeOpening] moves the opening. */
    class RedemptionChangePreparationV4 internal constructor(
        opening: NoteOpening?,
        rho: ByteArray?,
        diversifier: ByteArray?,
        commitment: ByteArray?,
        spendNullifier: ByteArray?,
        amount: KagemushaScaledAmount?,
    ) : AutoCloseable {
        private var openingValue: NoteOpening? = null
        private val rhoValue: ByteArray
        private val diversifierValue: ByteArray
        private val commitmentValue: ByteArray
        private val spendNullifierValue: ByteArray
        private val amountValue: KagemushaScaledAmount
        private var closed = false

        init {
            val ownedOpening = requireNotNull(opening) { "opening must not be null" }
            var rhoCopy: ByteArray? = null
            var diversifierCopy: ByteArray? = null
            var commitmentCopy: ByteArray? = null
            var spendNullifierCopy: ByteArray? = null
            try {
                check(!ownedOpening.isDestroyed()) { "opening has already been destroyed" }
                val requiredAmount = requireNotNull(amount) { "amount must not be null" }
                val checkedRho = requireDigest(rho, "rho").also { rhoCopy = it }
                val checkedDiversifier = requireDigest(diversifier, "diversifier")
                    .also { diversifierCopy = it }
                val checkedCommitment = requireDigest(commitment, "commitment")
                    .also { commitmentCopy = it }
                val checkedSpendNullifier = requireDigest(spendNullifier, "spendNullifier")
                    .also { spendNullifierCopy = it }
                check(!checkedRho.contentEquals(checkedDiversifier)) {
                    "native Kagemusha redemption opening coordinates collide"
                }
                openingValue = ownedOpening
                rhoValue = checkedRho
                diversifierValue = checkedDiversifier
                commitmentValue = checkedCommitment
                spendNullifierValue = checkedSpendNullifier
                amountValue = requiredAmount
            } catch (failure: Throwable) {
                spendNullifierCopy?.fill(0)
                commitmentCopy?.fill(0)
                diversifierCopy?.fill(0)
                rhoCopy?.fill(0)
                ownedOpening.destroy()
                throw failure
            }
        }

        val amount: KagemushaScaledAmount
            @Synchronized get() {
                requireOpen()
                return amountValue
            }

        /** Move the opening to a request/result owner. This succeeds exactly once. */
        @Synchronized
        fun takeOpening(): NoteOpening {
            requireOpen()
            val ownedOpening = checkNotNull(openingValue) {
                "redemption change opening has already been transferred"
            }
            openingValue = null
            return ownedOpening
        }

        @Synchronized
        fun rho(): ByteArray = openCopy(rhoValue)

        @Synchronized
        fun diversifier(): ByteArray = openCopy(diversifierValue)

        @Synchronized
        fun commitment(): ByteArray = openCopy(commitmentValue)

        @Synchronized
        fun spendNullifier(): ByteArray = openCopy(spendNullifierValue)

        @Synchronized
        override fun close() {
            if (closed) return
            openingValue?.destroy()
            openingValue = null
            rhoValue.fill(0)
            diversifierValue.fill(0)
            commitmentValue.fill(0)
            spendNullifierValue.fill(0)
            closed = true
        }

        private fun openCopy(value: ByteArray): ByteArray {
            requireOpen()
            return value.copyOf()
        }

        private fun requireOpen() {
            check(!closed) { "redemption change preparation has been destroyed" }
        }
    }

    /** Owns native-derived ordinary peer-split change until [takeOpening] transfers it. */
    class PeerSplitChangePreparationV4 internal constructor(
        opening: NoteOpening,
        rho: ByteArray,
        diversifier: ByteArray,
        commitment: ByteArray,
        spendNullifier: ByteArray,
        val amount: KagemushaScaledAmount,
    ) : AutoCloseable {
        private var openingValue: NoteOpening? = null
        private val rhoValue: ByteArray
        private val diversifierValue: ByteArray
        private val commitmentValue: ByteArray
        private val spendNullifierValue: ByteArray
        private var closed = false

        init {
            var rhoCopy: ByteArray? = null
            var diversifierCopy: ByteArray? = null
            var commitmentCopy: ByteArray? = null
            var nullifierCopy: ByteArray? = null
            try {
                check(!opening.isDestroyed()) { "opening has already been destroyed" }
                val checkedRho = requireDigest(rho, "rho").also { rhoCopy = it }
                val checkedDiversifier = requireDigest(diversifier, "diversifier")
                    .also { diversifierCopy = it }
                val checkedCommitment = requireDigest(commitment, "commitment")
                    .also { commitmentCopy = it }
                val checkedNullifier = requireDigest(spendNullifier, "spendNullifier")
                    .also { nullifierCopy = it }
                check(!checkedRho.contentEquals(checkedDiversifier)) {
                    "native Kagemusha peer-split opening coordinates collide"
                }
                openingValue = opening
                rhoValue = checkedRho
                diversifierValue = checkedDiversifier
                commitmentValue = checkedCommitment
                spendNullifierValue = checkedNullifier
            } catch (failure: Throwable) {
                nullifierCopy?.fill(0)
                commitmentCopy?.fill(0)
                diversifierCopy?.fill(0)
                rhoCopy?.fill(0)
                opening.destroy()
                throw failure
            }
        }

        @Synchronized fun takeOpening(): NoteOpening {
            requireOpen()
            val owned = checkNotNull(openingValue) {
                "peer-split change opening has already been transferred"
            }
            openingValue = null
            return owned
        }

        @Synchronized fun rho(): ByteArray = copyOpen(rhoValue)
        @Synchronized fun diversifier(): ByteArray = copyOpen(diversifierValue)
        @Synchronized fun commitment(): ByteArray = copyOpen(commitmentValue)
        @Synchronized fun spendNullifier(): ByteArray = copyOpen(spendNullifierValue)

        @Synchronized override fun close() {
            if (closed) return
            openingValue?.destroy()
            openingValue = null
            rhoValue.fill(0)
            diversifierValue.fill(0)
            commitmentValue.fill(0)
            spendNullifierValue.fill(0)
            closed = true
        }

        private fun copyOpen(value: ByteArray): ByteArray {
            requireOpen()
            return value.copyOf()
        }

        private fun requireOpen() {
            check(!closed) { "peer-split change preparation has been destroyed" }
        }
    }

    class RecipientRequestPayload internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecipientPaymentRequestSigningPayloadV2",
        "recipientRequestPayload",
        MAX_PEER_ARCHIVE_BYTES_V2,
    )

    /** Opaque ABI-21 recursive state. */
    class BundleV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendBundleV4",
        "bundleV4",
        MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
    )

    /** Opaque current lineage claim; native comparison implements all overlap rules. */
    class BranchClaim internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendBranchClaimV2",
        "branchClaim",
        MAX_PEER_ARCHIVE_BYTES_V2,
    ) {
        fun conflictsWith(other: BranchClaim): Boolean {
            requireArtifactBridge()
            return nativeBranchClaimsConflictV2(noritoEncoded(), other.noritoEncoded())
        }
    }

    class ArtifactBindingV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendArtifactBindingV4",
        "artifactBinding",
        MAX_MANIFEST_BYTES,
    )

    class TopUpUnsigned internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpUnsignedV4",
        "topUpUnsigned",
        MAX_TORII_TOP_UP_REQUEST_BYTES_V4,
    )

    class TopUpRequest internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha.torii.v1.offline.top_up.request",
        "topUpRequest",
        MAX_TORII_TOP_UP_REQUEST_BYTES_V4,
    )

    /** Finalized ABI-21 top-up receipt with a V4 artifact binding. */
    class TopUpAnchorV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpAnchorV4",
        "topUpAnchorV4",
        MAX_TORII_PROOF_ARCHIVE_BYTES,
    )

    class TopUpFinalityProof internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaTopUpFinalityProofV2",
        "topUpFinalityProof",
        MAX_TORII_PROOF_ARCHIVE_BYTES,
    )

    class TopUpFinalityRosterArtifact internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaTopUpFinalityRosterArtifactV2",
        "topUpFinalityRosterArtifact",
        MAX_TORII_PROOF_ARCHIVE_BYTES,
    )

    /** Complete V4 origin plus its stable compact-finality proof. */
    class TopUpFinalityEvidenceV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpFinalityEvidenceV4",
        "topUpFinalityEvidenceV4",
        MAX_TORII_PROOF_ARCHIVE_BYTES,
    )

    /** Complete bounded origin-finality inventory required to spend or verify one V4 bundle. */
    class TopUpProvenanceV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpProvenanceV4",
        "topUpProvenanceV4",
        MAX_TOP_UP_PROVENANCE_ARCHIVE_BYTES_V4,
    )

    class RedeemSubmissionRequest internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha.torii.v1.offline.redeem.request",
        "redeemSubmissionRequest",
        MAX_TORII_REDEEM_REQUEST_BYTES_V4,
    )

    class RedeemUnsignedV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendRedeemUnsignedV4",
        "redeemUnsignedV4",
        MAX_TORII_REDEEM_REQUEST_BYTES_V4,
    )

    class RequestAuthorizationPreparationArchive internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRequestAuthorizationPreparationV3",
        "requestAuthorizationPreparation",
        MAX_REQUEST_AUTHORIZATION_BYTES,
    )

    class RequestAuthorization internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRequestAuthorizationV2",
        "requestAuthorization",
        MAX_REQUEST_AUTHORIZATION_BYTES,
    )

    class TopUpZeroPath(
        val leafIndex: Int,
        siblings: List<ByteArray>,
        directions: ByteArray,
        root: ByteArray,
    ) {
        private val siblingsValue: List<ByteArray>
        private val directionsValue: ByteArray
        private val rootValue: ByteArray

        init {
            require(leafIndex in 0 until TOP_UP_SHIELD_INSERTION_CAPACITY) {
                "leafIndex is outside the top-up shield insertion range"
            }
            require(siblings.size == CONFIDENTIAL_TREE_DEPTH && siblings.all { it.size == 32 }) {
                "siblings must contain exactly $CONFIDENTIAL_TREE_DEPTH 32-byte nodes"
            }
            require(directions.size == CONFIDENTIAL_TREE_DEPTH && directions.all { it.toInt() in 0..1 }) {
                "directions must contain exactly $CONFIDENTIAL_TREE_DEPTH binary values"
            }
            val encodedLeaf = directions.withIndex().fold(0) { value, (level, direction) ->
                value or (direction.toInt() shl level)
            }
            require(encodedLeaf == leafIndex) { "directions do not encode leafIndex" }
            siblingsValue = siblings.map(ByteArray::copyOf)
            directionsValue = directions.copyOf()
            rootValue = requireDigest(root, "root")
        }

        fun siblings(): List<ByteArray> = siblingsValue.map(ByteArray::copyOf)

        fun directions(): ByteArray = directionsValue.copyOf()

        fun root(): ByteArray = rootValue.copyOf()

        internal fun flattenedSiblings(): ByteArray =
            ByteArray(CONFIDENTIAL_TREE_DEPTH * 32).also { flattened ->
                siblingsValue.forEachIndexed { index, sibling ->
                    sibling.copyInto(flattened, index * 32)
                }
            }

        companion object {
            /** Convert only Torii's authoritative next-zero path; ordinary inclusion paths fail. */
            @JvmStatic
            fun from(response: ZkMerklePathResponse): TopUpZeroPath {
                require(response.treeDepth == CONFIDENTIAL_TREE_DEPTH) {
                    "Torii confidential tree depth does not match Kagemusha"
                }
                val path = response.requireNextZeroPath()
                return TopUpZeroPath(
                    path.leafIndex,
                    path.siblingBytes(),
                    path.directions,
                    path.rootBytes(),
                )
            }
        }
    }

    /** Canonical next-zero cursor persisted atomically with every restored ABI-21 branch. */
    class OutputMembershipFrontierV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "connect_norito_bridge::KagemushaOutputMembershipFrontierV4",
        "outputMembershipFrontierV4",
        MAX_OUTPUT_MEMBERSHIP_FRONTIER_ARCHIVE_BYTES_V4,
    )

    /** One authenticated confidential-tree path used by the V4 output-update witness. */
    class OutputMembershipPath(
        val leafIndex: Int,
        siblings: List<ByteArray>,
        directions: ByteArray,
        root: ByteArray,
    ) {
        private val siblingsValue: List<ByteArray>
        private val directionsValue: ByteArray
        private val rootValue: ByteArray

        init {
            require(leafIndex in 0 until (1 shl CONFIDENTIAL_TREE_DEPTH)) {
                "leafIndex is outside the confidential tree"
            }
            require(siblings.size == CONFIDENTIAL_TREE_DEPTH && siblings.all { it.size == 32 }) {
                "siblings must contain exactly $CONFIDENTIAL_TREE_DEPTH 32-byte nodes"
            }
            require(directions.size == CONFIDENTIAL_TREE_DEPTH && directions.all { it.toInt() in 0..1 }) {
                "directions must contain exactly $CONFIDENTIAL_TREE_DEPTH binary values"
            }
            val encodedLeaf = directions.withIndex().fold(0) { value, (level, direction) ->
                value or (direction.toInt() shl level)
            }
            require(encodedLeaf == leafIndex) { "directions do not encode leafIndex" }
            siblingsValue = siblings.map(ByteArray::copyOf)
            directionsValue = directions.copyOf()
            rootValue = requireDigest(root, "root")
        }

        fun siblings(): List<ByteArray> = siblingsValue.map(ByteArray::copyOf)

        fun directions(): ByteArray = directionsValue.copyOf()

        fun root(): ByteArray = rootValue.copyOf()

        internal fun flattenedSiblings(): ByteArray =
            ByteArray(CONFIDENTIAL_TREE_DEPTH * 32).also { flattened ->
                siblingsValue.forEachIndexed { index, sibling ->
                    sibling.copyInto(flattened, index * 32)
                }
            }

        companion object {
            /** Convert one validated Torii path entry without weakening its root/index binding. */
            @JvmStatic
            fun from(entry: ZkMerklePathEntry): OutputMembershipPath = OutputMembershipPath(
                entry.leafIndex,
                entry.siblingBytes(),
                entry.directions,
                entry.rootBytes(),
            )
        }
    }

    /** Insertion path plus membership path for one output at the operation's final root. */
    class OutputMembershipLeafPaths(
        val updatePath: OutputMembershipPath,
        val membershipPath: OutputMembershipPath,
    ) {
        val leafIndex: Int = updatePath.leafIndex

        init {
            require(membershipPath.leafIndex == leafIndex) {
                "updatePath and membershipPath must address the same leaf"
            }
        }

        internal fun nativeFields(): Array<ByteArray> = arrayOf(
            leafIndex.toString().toByteArray(StandardCharsets.UTF_8),
            updatePath.flattenedSiblings(),
            updatePath.directions(),
            updatePath.root(),
            membershipPath.flattenedSiblings(),
            membershipPath.directions(),
            membershipPath.root(),
        )
    }

    /** Complete V4 output-update witness; commitments are derived and bound by native code. */
    class OutputMembershipPaths internal constructor(
        initialRoot: ByteArray,
        finalRoot: ByteArray,
        val recipient: OutputMembershipLeafPaths?,
        val change: OutputMembershipLeafPaths?,
        val dummyPath: OutputMembershipPath,
        canonicalArchive: ByteArray?,
    ) {
        private val initialRootValue = requireDigest(initialRoot, "initialRoot")
        private val finalRootValue = requireDigest(finalRoot, "finalRoot")
        private val canonicalArchiveValue = canonicalArchive?.let {
            requireCanonicalArchive(
                it,
                "connect_norito_bridge::KagemushaOutputMembershipPathsV4",
                "outputMembershipPathsV4",
                MAX_OUTPUT_MEMBERSHIP_PATHS_ARCHIVE_BYTES_V4,
            )
        }

        constructor(
            initialRoot: ByteArray,
            finalRoot: ByteArray,
            recipient: OutputMembershipLeafPaths?,
            change: OutputMembershipLeafPaths?,
            dummyPath: OutputMembershipPath,
        ) : this(initialRoot, finalRoot, recipient, change, dummyPath, null)

        init {
            require(!initialRootValue.contentEquals(finalRootValue)) {
                "initialRoot and finalRoot must differ"
            }
            require(recipient != null || change != null) {
                "at least one output membership leaf is required"
            }
            require(dummyPath.root().contentEquals(finalRootValue)) {
                "dummyPath must be rooted at finalRoot"
            }
            recipient?.let {
                require(it.membershipPath.root().contentEquals(finalRootValue)) {
                    "recipient membershipPath must be rooted at finalRoot"
                }
            }
            change?.let {
                require(it.membershipPath.root().contentEquals(finalRootValue)) {
                    "change membershipPath must be rooted at finalRoot"
                }
            }
            if (recipient != null) {
                require(recipient.updatePath.root().contentEquals(initialRootValue)) {
                    "recipient updatePath must be rooted at initialRoot"
                }
            } else {
                require(change!!.updatePath.root().contentEquals(initialRootValue)) {
                    "change updatePath must be rooted at initialRoot"
                }
            }
            if (recipient != null && change != null) {
                require(recipient.leafIndex + 1 == change.leafIndex) {
                    "change output must immediately follow the recipient output"
                }
            }
            val lastOutputLeafIndex = change?.leafIndex ?: recipient!!.leafIndex
            require(dummyPath.leafIndex == lastOutputLeafIndex + 1) {
                "dummyPath must immediately follow the final output"
            }
            val occupied = listOfNotNull(recipient?.leafIndex, change?.leafIndex) + dummyPath.leafIndex
            require(occupied.distinct().size == occupied.size) {
                "output and dummy paths must address distinct leaves"
            }
        }

        fun initialRoot(): ByteArray = initialRootValue.copyOf()

        fun finalRoot(): ByteArray = finalRootValue.copyOf()

        internal fun nativeArchive(): ByteArray {
            canonicalArchiveValue?.let { return it.copyOf() }
            requireArtifactBridge()
            val recipientFields = recipient?.nativeFields() ?: emptyArray()
            val changeFields = change?.nativeFields() ?: emptyArray()
            val dummyFields = arrayOf(
                dummyPath.leafIndex.toString().toByteArray(StandardCharsets.UTF_8),
                dummyPath.flattenedSiblings(),
                dummyPath.directions(),
                dummyPath.root(),
            )
            return try {
                nativeBuildOutputMembershipPathsV4(
                    initialRootValue,
                    finalRootValue,
                    recipientFields,
                    changeFields,
                    dummyFields,
                )
            } finally {
                recipientFields.forEach { it.fill(0) }
                changeFields.forEach { it.fill(0) }
                dummyFields.forEach { it.fill(0) }
            }
        }
    }

    /** Local secret-bearing initialization input. Close it if it is not submitted. */
    class InitRequestV4 internal constructor(archive: ByteArray) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendInitLocalRequestV4",
            "initRequest",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        /** Zeroize an unconsumed initialization request. Repeated closes are harmless. */
        override fun close() {
            destroy()
        }
    }

    /** Local secret-bearing append input. Native code consumes and wipes its openings. */
    class AppendRequestV4 internal constructor(
        archive: ByteArray,
        changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendAppendLocalRequestV4",
            "appendRequest",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        private val changeOpeningOwner = ChangeOpeningOwner(changeOpening)

        @Synchronized
        internal fun takeChangeOpening(): NoteOpening? {
            check(!isDestroyed()) { "append request has been closed" }
            return changeOpeningOwner.take()
        }

        @Synchronized
        override fun close() {
            destroy()
            changeOpeningOwner.close()
        }
    }

    class VerifyRequestV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendVerifyLocalRequestV4",
        "verifyRequest",
        MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
    )

    /** Local secret-bearing redemption input. Native code consumes and wipes its openings. */
    class RedeemRequestV5 internal constructor(
        archive: ByteArray,
        changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendRedeemLocalRequestV5",
            "redeemRequest",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        private val changeOpeningOwner = ChangeOpeningOwner(changeOpening)

        @Synchronized
        internal fun takeChangeOpening(): NoteOpening? {
            check(!isDestroyed()) { "redeem request has been closed" }
            return changeOpeningOwner.take()
        }

        @Synchronized
        override fun close() {
            destroy()
            changeOpeningOwner.close()
        }
    }

    class InitResultV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendInitResultV4",
        "initResult",
        MAX_LOCAL_RESULT_ARCHIVE_BYTES,
    )

    class SplitResultV4 internal constructor(
        archive: ByteArray,
        changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
        "KagemushaRecursiveSpendSplitResultV4",
            "splitResult",
            MAX_LOCAL_RESULT_ARCHIVE_BYTES,
        ), AutoCloseable {
        private val changeOpeningOwner = ChangeOpeningOwner(changeOpening)

        @Synchronized
        internal fun takeChangeOpening(): NoteOpening? {
            check(!isDestroyed()) { "split result has been closed" }
            return changeOpeningOwner.take()
        }

        @Synchronized
        override fun close() {
            destroy()
            changeOpeningOwner.close()
        }
    }

    class VerifyResultV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendVerifyResultV4",
        "verifyResult",
        MAX_LOCAL_RESULT_ARCHIVE_BYTES,
    )

    class RedeemBuildResultV4 internal constructor(
        archive: ByteArray,
        changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
        "KagemushaRecursiveSpendRedeemBuildResultV4",
            "redeemBuildResult",
            MAX_LOCAL_RESULT_ARCHIVE_BYTES,
        ), AutoCloseable {
        private val changeOpeningOwner = ChangeOpeningOwner(changeOpening)

        @Synchronized
        internal fun takeChangeOpening(): NoteOpening? {
            check(!isDestroyed()) { "redeem result has been closed" }
            return changeOpeningOwner.take()
        }

        @Synchronized
        override fun close() {
            destroy()
            changeOpeningOwner.close()
        }
    }

    class RecipientRequestPreparation internal constructor(
        payload: RecipientRequestPayload,
        signingBytes: ByteArray,
        opening: NoteOpening,
        commitment: ByteArray,
        nullifier: ByteArray,
        amount: KagemushaScaledAmount,
    ) {
        internal val payload: RecipientRequestPayload
        private val signingBytesValue: ByteArray
        val opening: NoteOpening
        private val commitmentValue: ByteArray
        private val nullifierValue: ByteArray
        val amount: KagemushaScaledAmount

        init {
            var signingBytesCopy: ByteArray? = null
            var commitmentCopy: ByteArray? = null
            var nullifierCopy: ByteArray? = null
            try {
                val ownedSigningBytes = requiredBytes(signingBytes, "signingBytes")
                    .also { signingBytesCopy = it }
                val ownedCommitment = requireDigest(commitment, "commitment")
                    .also { commitmentCopy = it }
                val ownedNullifier = requireDigest(nullifier, "nullifier")
                    .also { nullifierCopy = it }
                this.payload = payload
                signingBytesValue = ownedSigningBytes
                this.opening = opening
                commitmentValue = ownedCommitment
                nullifierValue = ownedNullifier
                this.amount = amount
            } catch (failure: Throwable) {
                SecretArchiveWiper.wipe(nullifierCopy)
                SecretArchiveWiper.wipe(commitmentCopy)
                SecretArchiveWiper.wipe(signingBytesCopy)
                opening.close()
                throw failure
            }
        }

        fun signingBytes(): ByteArray = signingBytesValue.copyOf()
        fun commitment(): ByteArray = commitmentValue.copyOf()
        fun nullifier(): ByteArray = nullifierValue.copyOf()
    }

    class RequestAuthorizationPreparation internal constructor(
        internal val archive: RequestAuthorizationPreparationArchive,
        signingBytes: ByteArray,
        operationId: ByteArray,
        payloadDigest: ByteArray,
        registrationHash: ByteArray,
    ) {
        private val signingBytesValue = requiredBytes(signingBytes, "signingBytes")
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val payloadDigestValue = requireDigest(payloadDigest, "payloadDigest")
        private val registrationHashValue = requireDigest(registrationHash, "registrationHash")

        fun signingBytes(): ByteArray = signingBytesValue.copyOf()

        fun operationId(): ByteArray = operationIdValue.copyOf()

        fun payloadDigest(): ByteArray = payloadDigestValue.copyOf()

        fun registrationHash(): ByteArray = registrationHashValue.copyOf()
    }

    class TopUpPreparation internal constructor(
        unsigned: TopUpUnsigned,
        authorizationDigest: ByteArray,
        opening: NoteOpening,
        noteCommitment: ByteArray,
        spendNullifier: ByteArray,
        initialRoot: ByteArray,
        finalizedRoot: ByteArray,
        operationId: ByteArray,
        amount: KagemushaScaledAmount,
        leafIndex: Int,
    ) {
        val unsigned: TopUpUnsigned
        private val authorizationDigestValue: ByteArray
        val opening: NoteOpening
        private val noteCommitmentValue: ByteArray
        private val spendNullifierValue: ByteArray
        private val initialRootValue: ByteArray
        private val finalizedRootValue: ByteArray
        private val operationIdValue: ByteArray
        val amount: KagemushaScaledAmount
        val leafIndex: Int

        init {
            var authorizationDigestCopy: ByteArray? = null
            var noteCommitmentCopy: ByteArray? = null
            var spendNullifierCopy: ByteArray? = null
            var initialRootCopy: ByteArray? = null
            var finalizedRootCopy: ByteArray? = null
            var operationIdCopy: ByteArray? = null
            try {
                val ownedAuthorizationDigest = requireDigest(
                    authorizationDigest,
                    "authorizationDigest",
                ).also { authorizationDigestCopy = it }
                val ownedNoteCommitment = requireDigest(noteCommitment, "noteCommitment")
                    .also { noteCommitmentCopy = it }
                val ownedSpendNullifier = requireDigest(spendNullifier, "spendNullifier")
                    .also { spendNullifierCopy = it }
                val ownedInitialRoot = requireDigest(initialRoot, "initialRoot")
                    .also { initialRootCopy = it }
                val ownedFinalizedRoot = requireDigest(finalizedRoot, "finalizedRoot")
                    .also { finalizedRootCopy = it }
                val ownedOperationId = requireDigest(operationId, "operationId")
                    .also { operationIdCopy = it }
                this.unsigned = unsigned
                authorizationDigestValue = ownedAuthorizationDigest
                this.opening = opening
                noteCommitmentValue = ownedNoteCommitment
                spendNullifierValue = ownedSpendNullifier
                initialRootValue = ownedInitialRoot
                finalizedRootValue = ownedFinalizedRoot
                operationIdValue = ownedOperationId
                this.amount = amount
                this.leafIndex = leafIndex
            } catch (failure: Throwable) {
                SecretArchiveWiper.wipe(operationIdCopy)
                SecretArchiveWiper.wipe(finalizedRootCopy)
                SecretArchiveWiper.wipe(initialRootCopy)
                SecretArchiveWiper.wipe(spendNullifierCopy)
                SecretArchiveWiper.wipe(noteCommitmentCopy)
                SecretArchiveWiper.wipe(authorizationDigestCopy)
                opening.close()
                throw failure
            }
        }

        fun authorizationDigest(): ByteArray = authorizationDigestValue.copyOf()
        fun noteCommitment(): ByteArray = noteCommitmentValue.copyOf()
        fun spendNullifier(): ByteArray = spendNullifierValue.copyOf()
        fun initialRoot(): ByteArray = initialRootValue.copyOf()
        fun finalizedRoot(): ByteArray = finalizedRootValue.copyOf()
        fun operationId(): ByteArray = operationIdValue.copyOf()
    }

    class RedeemFinalization internal constructor(
        val request: RedeemSubmissionRequest,
        operationId: ByteArray,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")

        fun operationId(): ByteArray = operationIdValue.copyOf()
    }

    class VerifiedRecipientPaymentRequest internal constructor(
        val request: RecipientPaymentRequest,
        digest: ByteArray,
        val verifiedAtMilliseconds: Long,
        val projection: RecipientRequestProjection,
    ) {
        private val digestValue = requireDigest(digest, "requestDigest")
        init {
            check(digestValue.contentEquals(projection.digest())) {
                "verified request digest does not match its projection"
            }
        }
        fun digest(): ByteArray = digestValue.copyOf()
    }

    class FinalityCheckpointPromotionV2 internal constructor(bytes: ByteArray) {
        private val encodedValue: ByteArray
        val height: Long

        init {
            require(bytes.size == PROMOTED_FINALITY_CHECKPOINT_BYTES_V2) {
                "promoted checkpoint must contain exactly 40 bytes"
            }
            require(bytes[0].toInt() and 0x80 == 0) {
                "promoted checkpoint height exceeds the signed-64-bit client bound"
            }
            var parsedHeight = 0L
            repeat(8) { index ->
                parsedHeight = (parsedHeight shl 8) or (bytes[index].toLong() and 0xffL)
            }
            require(parsedHeight > 0) { "promoted checkpoint height must be positive" }
            val context = bytes.copyOfRange(8, bytes.size)
            try {
                requireFinalityCheckpointContext(context, "promotedCheckpointContextId")
                    .fill(0)
            } finally {
                context.fill(0)
            }
            encodedValue = bytes.copyOf()
            height = parsedHeight
        }

        fun encoded(): ByteArray = encodedValue.copyOf()

        fun contextId(): ByteArray = encodedValue.copyOfRange(8, encodedValue.size)
    }

    class VerifiedRecipientRegistrationLineageV2 internal constructor(
        val lineage: RecipientRegistrationLineage,
        val promotedCheckpoint: FinalityCheckpointPromotionV2,
    )

    class RecipientReceiveOfferProjectionV2 internal constructor(
        val request: RecipientPaymentRequest,
        val lineage: RecipientRegistrationLineage,
        publisherCheckpointEnvelope: ByteArray,
    ) {
        private val publisherEnvelopeValue = requireBoundedBytes(
            publisherCheckpointEnvelope,
            "publisherCheckpointEnvelope",
            MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1,
        )

        fun publisherCheckpointEnvelope(): ByteArray = publisherEnvelopeValue.copyOf()
    }

    class VerifiedRecipientReceiveOfferV2 internal constructor(
        val request: RecipientPaymentRequest,
        val lineage: RecipientRegistrationLineage,
        publisherCheckpointEnvelope: ByteArray,
        val promotedCheckpoint: FinalityCheckpointPromotionV2,
        val verifiedAtMilliseconds: Long,
    ) {
        private val publisherEnvelopeValue = requireBoundedBytes(
            publisherCheckpointEnvelope,
            "publisherCheckpointEnvelope",
            MAX_PUBLISHER_CHECKPOINT_ENVELOPE_BYTES_V1,
        )

        init {
            require(verifiedAtMilliseconds > 0)
        }

        fun publisherCheckpointEnvelope(): ByteArray = publisherEnvelopeValue.copyOf()
    }

    class RecipientRequestProjection internal constructor(
        networkId: NetworkId,
        val assetDefinitionId: String,
        val amount: KagemushaScaledAmount,
        val recipientAccountId: String,
        val receiverDeviceId: String,
        requestId: ByteArray,
        val issuedAtMilliseconds: Long,
        val expiresAtMilliseconds: Long,
        outputCommitment: ByteArray,
        outputNullifier: ByteArray,
        receiverKeyReference: ByteArray,
        receiverPublicKey: ByteArray,
        digest: ByteArray,
    ) {
        private val networkIdValue = networkId
        private val requestIdValue = requireDigest(requestId, "requestId")
        private val outputCommitmentValue = requireDigest(outputCommitment, "outputCommitment")
        private val outputNullifierValue = requireDigest(outputNullifier, "outputNullifier")
        private val receiverKeyReferenceValue = requireDigest(receiverKeyReference, "receiverKeyReference")
        private val receiverPublicKeyValue = KagemushaDevicePublicKeyV2(receiverPublicKey)
        private val digestValue = requireDigest(digest, "requestDigest")

        fun networkId(): NetworkId = networkIdValue
        fun requestId(): ByteArray = requestIdValue.copyOf()
        fun outputCommitment(): ByteArray = outputCommitmentValue.copyOf()
        fun outputNullifier(): ByteArray = outputNullifierValue.copyOf()
        fun receiverKeyReference(): ByteArray = receiverKeyReferenceValue.copyOf()
        fun receiverPublicKey(): KagemushaDevicePublicKeyV2 = receiverPublicKeyValue
        fun digest(): ByteArray = digestValue.copyOf()
    }

    open class BranchProjection internal constructor(
        val bundle: BundleV4,
        val membershipWitness: NoteMembershipWitness,
        commitment: ByteArray,
        spendNullifier: ByteArray,
        val amount: KagemushaScaledAmount,
        val hopCount: Int,
        val proofStepCount: Int,
        bundleDigest: ByteArray,
        val artifactBinding: ArtifactBindingV4,
        branchClaims: List<BranchClaim>,
    ) {
        private val commitmentValue = requireDigest(commitment, "commitment")
        private val spendNullifierValue = requireDigest(spendNullifier, "spendNullifier")
        private val bundleDigestValue = requireDigest(bundleDigest, "bundleDigest")
        val branchClaims: List<BranchClaim> = Collections.unmodifiableList(ArrayList(branchClaims))

        init {
            check(hopCount in 0..MAXIMUM_PEER_HOPS) { "native Kagemusha hop count is invalid" }
            check(proofStepCount in 1..MAXIMUM_PROOF_STEPS) {
                "native Kagemusha proof-step count is invalid"
            }
            check(this.branchClaims.size in 1..MAXIMUM_BRANCH_CLAIMS) {
                "native Kagemusha exact-state claims are invalid"
            }
        }

        fun commitment(): ByteArray = commitmentValue.copyOf()
        fun spendNullifier(): ByteArray = spendNullifierValue.copyOf()
        fun bundleDigest(): ByteArray = bundleDigestValue.copyOf()
        fun branchClaims(): List<BranchClaim> = branchClaims.toList()

        fun conflictsWith(other: BranchProjection): Boolean = branchClaims.any { left ->
            other.branchClaims.any { right -> left.conflictsWith(right) }
        }
    }

    /** Secret-bearing local state used only by the genuine ABI-21 builders. */
    class SpendableBranchV4 internal constructor(
        val bundle: BundleV4,
        val membershipWitness: NoteMembershipWitness,
        val opening: NoteOpening,
        val topUpProvenance: TopUpProvenanceV4,
        val frontier: OutputMembershipFrontierV4,
    ) : AutoCloseable {
        /** Destroy the locally held secret opening; public proof artifacts remain immutable. */
        override fun close() {
            opening.destroy()
        }
    }

    class PeerPaymentProjection internal constructor(
        val branch: BranchProjection,
        val topUpProvenance: TopUpProvenanceV4,
        operationId: ByteArray,
        requestDigest: ByteArray,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")

        fun operationId(): ByteArray = operationIdValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
    }

    class InitProjectionV4 internal constructor(
        val branch: BranchProjection,
        val topUpProvenance: TopUpProvenanceV4,
        publicStatementDigest: ByteArray,
    ) {
        private val publicStatementDigestValue =
            requireDigest(publicStatementDigest, "publicStatementDigest")

        val bundle: BundleV4 get() = branch.bundle
        fun publicStatementDigest(): ByteArray = publicStatementDigestValue.copyOf()
    }

    class SplitProjection internal constructor(
        val peerPayment: PeerPayment,
        val recipient: BranchProjection,
        val change: BranchProjection?,
        val recipientTopUpProvenance: TopUpProvenanceV4,
        val changeTopUpProvenance: TopUpProvenanceV4?,
        operationId: ByteArray,
        requestDigest: ByteArray,
        splitBindingDigest: ByteArray,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")
        private val splitBindingDigestValue = requireDigest(splitBindingDigest, "splitBindingDigest")

        fun operationId(): ByteArray = operationIdValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
        fun splitBindingDigest(): ByteArray = splitBindingDigestValue.copyOf()
    }

    class VerifyProjection internal constructor(
        val valid: Boolean,
        val chainAdmissible: Boolean,
        val lineageRedeemable: Boolean,
        val witnesslessRedemptionSupported: Boolean,
        commitment: ByteArray,
        spendNullifier: ByteArray,
        val amount: KagemushaScaledAmount,
        val hopCount: Int,
        val proofStepCount: Int,
        bundleDigest: ByteArray,
        val assetDefinitionId: String,
        val artifactBinding: ArtifactBindingV4,
        requestDigest: ByteArray,
        outputBindingDigest: ByteArray,
        val verifierBackend: String,
        val verifierName: String,
        val verifierCircuitId: String,
        val verifierActivationHeight: Long?,
        val verifierWithdrawalHeight: Long?,
        val verifiedAtBlockHeight: Long,
        val verifiedAtMilliseconds: Long,
        branchClaims: List<BranchClaim>,
    ) {
        private val commitmentValue = requireDigest(commitment, "commitment")
        private val spendNullifierValue = requireDigest(spendNullifier, "spendNullifier")
        private val bundleDigestValue = requireDigest(bundleDigest, "bundleDigest")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")
        private val outputBindingDigestValue = requireDigest(outputBindingDigest, "outputBindingDigest")
        val branchClaims: List<BranchClaim> = Collections.unmodifiableList(ArrayList(branchClaims))

        init {
            check(hopCount in 0..MAXIMUM_PEER_HOPS && proofStepCount in 1..MAXIMUM_PROOF_STEPS) {
                "native Kagemusha verified state counters are invalid"
            }
            check(verifiedAtBlockHeight > 0 && verifiedAtMilliseconds > 0) {
                "native Kagemusha verification snapshot is invalid"
            }
            check(this.branchClaims.size in 1..MAXIMUM_BRANCH_CLAIMS) {
                "native Kagemusha verified branch claim vector is invalid"
            }
        }

        fun commitment(): ByteArray = commitmentValue.copyOf()
        fun spendNullifier(): ByteArray = spendNullifierValue.copyOf()
        fun bundleDigest(): ByteArray = bundleDigestValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
        fun outputBindingDigest(): ByteArray = outputBindingDigestValue.copyOf()
        fun branchClaims(): List<BranchClaim> = branchClaims.toList()
    }

    class RedeemBuildProjection internal constructor(
        val unsigned: RedeemUnsignedV4,
        authorizationDigest: ByteArray,
        val change: BranchProjection?,
        val changeTopUpProvenance: TopUpProvenanceV4?,
        operationId: ByteArray,
    ) {
        private val authorizationDigestValue = requireDigest(authorizationDigest, "authorizationDigest")
        private val operationIdValue = requireDigest(operationId, "operationId")

        init {
            check((change == null) == (changeTopUpProvenance == null)) {
                "native Kagemusha redemption change provenance does not match change projection"
            }
        }

        fun authorizationDigest(): ByteArray = authorizationDigestValue.copyOf()
        fun operationId(): ByteArray = operationIdValue.copyOf()
    }

    class AcknowledgementPayload internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaReceiverAcknowledgementPayloadV2",
        "acknowledgementPayload",
        MAX_PEER_ARCHIVE_BYTES,
    )

    class AcknowledgementPreparation internal constructor(
        internal val payload: AcknowledgementPayload,
        signingBytes: ByteArray,
        operationId: ByteArray,
        requestDigest: ByteArray,
        bundleDigest: ByteArray,
        commitment: ByteArray,
    ) {
        private val signingBytesValue = requiredBytes(signingBytes, "signingBytes")
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")
        private val bundleDigestValue = requireDigest(bundleDigest, "bundleDigest")
        private val commitmentValue = requireDigest(commitment, "commitment")

        fun signingBytes(): ByteArray = signingBytesValue.copyOf()
        fun operationId(): ByteArray = operationIdValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
        fun bundleDigest(): ByteArray = bundleDigestValue.copyOf()
        fun commitment(): ByteArray = commitmentValue.copyOf()
    }

    /** Delivery-receipt evidence for an already-final sender cash handoff. */
    class AcknowledgementVerification internal constructor(
        val valid: Boolean,
        operationId: ByteArray,
        requestDigest: ByteArray,
        bundleDigest: ByteArray,
        acknowledgementDigest: ByteArray,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")
        private val bundleDigestValue = requireDigest(bundleDigest, "bundleDigest")
        private val acknowledgementDigestValue = requireDigest(acknowledgementDigest, "acknowledgementDigest")

        fun operationId(): ByteArray = operationIdValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
        fun bundleDigest(): ByteArray = bundleDigestValue.copyOf()
        fun acknowledgementDigest(): ByteArray = acknowledgementDigestValue.copyOf()
    }

    /** Asset-neutral offline protocol capability implemented by every app-api node. */
    class OfflineStatus internal constructor(
        val cashHandoffCapability: String,
        val requiredBridgeAbiVersion: Int,
        val maximumHops: Int,
        val ready: Boolean,
    ) {
        init {
            require(cashHandoffCapability == CASH_HANDOFF_CAPABILITY_V1) {
                "cashHandoffCapability must be the exact cash_handoff_v1 contract"
            }
            require(requiredBridgeAbiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION) {
                "requiredBridgeAbiVersion must be 23"
            }
            require(maximumHops == MAXIMUM_PEER_HOPS) {
                "maximumHops must match the cash_handoff_v1 bound"
            }
            require(ready) { "ready must be true for universal offline capability" }
        }

        internal companion object {
            private val fields = setOf(
                "cash_handoff_capability",
                "required_bridge_abi_version",
                "max_hops",
                "ready",
            )

            fun decode(payload: ByteArray): OfflineStatus {
                val parsed = JsonParser.parse(String(payload, StandardCharsets.UTF_8))
                check(parsed is Map<*, *>) { "offline capability response must be a JSON object" }
                check(parsed.keys == fields) {
                    "offline capability response must contain exactly the universal fields"
                }
                val capability = parsed["cash_handoff_capability"] as? String
                    ?: error("offline capability cash_handoff_capability must be a string")
                val abi = exactInt(parsed["required_bridge_abi_version"], "required_bridge_abi_version")
                val hops = exactInt(parsed["max_hops"], "max_hops")
                val ready = parsed["ready"] as? Boolean
                    ?: error("offline capability ready must be a boolean")
                return OfflineStatus(
                    cashHandoffCapability = capability,
                    requiredBridgeAbiVersion = abi,
                    maximumHops = hops,
                    ready = ready,
                )
            }

            private fun exactInt(value: Any?, field: String): Int {
                check(value is Long && value in Int.MIN_VALUE.toLong()..Int.MAX_VALUE.toLong()) {
                    "offline capability $field must be a signed 32-bit JSON integer"
                }
                return value.toInt()
            }
        }
    }

    class OperationReference internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "OfflineOperationReference",
        "operationReference",
        MAX_TORII_OPERATION_REFERENCE_BYTES,
    )

    class OperationStatus internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "OfflineOperationStatus",
        "operationStatus",
        MAX_TORII_OPERATION_STATUS_BYTES,
    )

    enum class OperationState { PENDING, APPLIED, REJECTED }

    enum class OperationKind { TOP_UP, REDEEM }

    /** Immutable identity of one canonical signed Kagemusha command. */
    class OperationIdentity internal constructor(
        operationId: ByteArray,
        requestAuthorityDigest: ByteArray,
        canonicalRequestDigest: ByteArray,
        val kind: OperationKind,
        val issuedAtMilliseconds: Long,
        val expiresAtMilliseconds: Long,
    ) {
        private val operationIdValue = requireMarkedDigest(operationId, "operationId")
        private val requestAuthorityDigestValue = requireMarkedDigest(
            requestAuthorityDigest,
            "requestAuthorityDigest",
        )
        private val canonicalRequestDigestValue = requireMarkedDigest(
            canonicalRequestDigest,
            "canonicalRequestDigest",
        )

        init {
            require(issuedAtMilliseconds > 0) { "issuedAtMilliseconds must be positive" }
            require(expiresAtMilliseconds > issuedAtMilliseconds) {
                "expiresAtMilliseconds must be later than issuedAtMilliseconds"
            }
            require(
                expiresAtMilliseconds - issuedAtMilliseconds <=
                    KAGEMUSHA_REQUEST_AUTHORIZATION_MAX_TTL_MS_V2,
            ) {
                "operation identity lifetime exceeds the authorization maximum"
            }
        }

        fun operationId(): ByteArray = operationIdValue.copyOf()

        fun requestAuthorityDigest(): ByteArray = requestAuthorityDigestValue.copyOf()

        fun canonicalRequestDigest(): ByteArray = canonicalRequestDigestValue.copyOf()

        fun operationIdHex(): String = hex(operationIdValue)

        override fun equals(other: Any?): Boolean =
            other is OperationIdentity &&
                operationIdValue.contentEquals(other.operationIdValue) &&
                requestAuthorityDigestValue.contentEquals(other.requestAuthorityDigestValue) &&
                canonicalRequestDigestValue.contentEquals(other.canonicalRequestDigestValue) &&
                kind == other.kind &&
                issuedAtMilliseconds == other.issuedAtMilliseconds &&
                expiresAtMilliseconds == other.expiresAtMilliseconds

        override fun hashCode(): Int {
            var result = operationIdValue.contentHashCode()
            result = 31 * result + requestAuthorityDigestValue.contentHashCode()
            result = 31 * result + canonicalRequestDigestValue.contentHashCode()
            result = 31 * result + kind.hashCode()
            result = 31 * result + issuedAtMilliseconds.hashCode()
            return 31 * result + expiresAtMilliseconds.hashCode()
        }
    }

    /**
     * Durable status-polling reference bound to the signed command.
     *
     * The complete signed identity remains fixed. Only a validated Pending response may advance
     * the transaction-hash cursor to a retry carrier.
     */
    class OperationHandle(
        val identity: OperationIdentity,
        transactionHash: ByteArray,
    ) {
        private var transactionHashValue = requireTransactionHash(
            transactionHash,
            "transactionHash",
        )
        private var terminalState: OperationState? = null

        @Synchronized fun transactionHash(): ByteArray = transactionHashValue.copyOf()

        @Synchronized internal fun acceptValidatedStatus(status: OperationStatusProjection) {
            check(terminalState == null || status.state == terminalState) {
                "Kagemusha Torii operation terminal state is immutable"
            }
            if (terminalState != null) return
            if (status.state == OperationState.PENDING) {
                val replacement = requireTransactionHash(status.transactionHash(), "transactionHash")
                transactionHashValue.fill(0)
                transactionHashValue = replacement
            } else {
                terminalState = status.state
            }
        }
    }

    class OperationReferenceProjection internal constructor(
        val identity: OperationIdentity,
        val state: OperationState,
        transactionHash: ByteArray,
        val statusUri: String,
    ) {
        private val transactionHashValue = requireTransactionHash(
            transactionHash,
            "transactionHash",
        )

        init {
            require(state == OperationState.PENDING) {
                "operation reference state must be pending"
            }
            require(statusUri == "/v1/offline/operations/${identity.operationIdHex()}") {
                "statusUri must match the canonical operation resource"
            }
        }

        fun transactionHash(): ByteArray = transactionHashValue.copyOf()
    }

    class OperationRejection(val code: String, val message: String) {
        init {
            require(
                code.length in 1..64 &&
                    (code[0] in 'a'..'z' || code[0] in '0'..'9') &&
                    code.all { it in 'a'..'z' || it in '0'..'9' || it == '_' },
            ) { "rejection code must use the stable lowercase code grammar" }
            require(
                message.isNotEmpty() &&
                    message == message.trim() &&
                    message.none(Char::isISOControl) &&
                    hasWellFormedUtf16(message) &&
                    message.codePointCount(0, message.length) <= 1_024 &&
                    message.toByteArray(Charsets.UTF_8).size <= 4_096,
            ) { "rejection message must be bounded canonical text" }
        }
    }

    class FinalizedTopUp internal constructor(
        val anchor: TopUpAnchorV4,
        val finalityProof: TopUpFinalityProof,
        val finalizedBlockHeight: Long,
    ) {
        init {
            require(finalizedBlockHeight > 0) { "finalizedBlockHeight must be positive" }
        }
    }

    class OperationStatusProjection internal constructor(
        val state: OperationState,
        val identity: OperationIdentity,
        transactionHash: ByteArray,
        val finalizedBlockHeight: Long?,
        val finalizedTopUp: FinalizedTopUp?,
        val rejection: OperationRejection?,
    ) {
        private val transactionHashValue = requireTransactionHash(transactionHash, "transactionHash")

        init {
            when (state) {
                OperationState.PENDING -> require(
                    finalizedBlockHeight == null &&
                        finalizedTopUp == null && rejection == null,
                ) { "pending operation status fields are invalid" }
                OperationState.APPLIED -> require(
                    finalizedBlockHeight != null && finalizedBlockHeight > 0 &&
                        rejection == null &&
                        ((identity.kind == OperationKind.TOP_UP && finalizedTopUp != null &&
                            finalizedTopUp.finalizedBlockHeight == finalizedBlockHeight) ||
                            (identity.kind == OperationKind.REDEEM && finalizedTopUp == null)),
                ) { "applied operation status fields are invalid" }
                OperationState.REJECTED -> require(
                    finalizedBlockHeight == null &&
                        finalizedTopUp == null &&
                        rejection != null,
                ) { "rejected operation status fields are invalid" }
            }
        }

        fun transactionHash(): ByteArray = transactionHashValue.copyOf()
    }

    /** Strict typed client for the five first-release Kagemusha Torii routes. */
    class ToriiClient internal constructor(
        baseUri: URI,
        private val transport: TransportExecutor,
        private val localSigningContext: LocalSigningContext,
        private val requestTimeout: Duration?,
        private val topUpRequestIdentityProjector: (TopUpRequest) -> OperationIdentity =
            { projectTopUpRequestIdentity(it) },
        private val redeemRequestIdentityProjector: (RedeemSubmissionRequest) -> OperationIdentity =
            { projectRedeemRequestIdentity(it) },
        private val operationReferenceProjector: (OperationReference) -> OperationReferenceProjection =
            { projectOperationReference(it) },
        private val operationStatusProjector: (OperationStatus) -> OperationStatusProjection =
            { projectOperationStatus(it) },
    ) {
        internal constructor(
            baseUri: URI,
            transport: TransportExecutor,
            localSigningContext: LocalSigningContext,
        ) : this(baseUri, transport, localSigningContext, null)

        companion object {
            const val CAPABILITY_PATH: String = "/v1/offline/readiness"
            const val TOP_UP_PATH: String = "/v1/offline/top-up"
            const val REDEEM_PATH: String = "/v1/offline/redeem"
            const val OPERATIONS_PATH: String = "/v1/offline/operations"
            const val RECEIVER_LINEAGE_PATH: String = "/v1/offline/receiver-lineage"
            const val JSON_MEDIA_TYPE: String = "application/json"
            const val NORITO_MEDIA_TYPE: String = "application/x-norito"

            private fun requireCommandResponseHeaders(
                response: TransportResponse,
                operationId: String,
            ) {
                val expectedLocation = "$OPERATIONS_PATH/$operationId"
                check(response.headers["Location"] == listOf(expectedLocation)) {
                    "Kagemusha Torii Location must match the canonical operation resource"
                }
                val retryAfterValues = response.headers["Retry-After"].orEmpty()
                check(retryAfterValues.size == 1) {
                    "Kagemusha Torii Retry-After must occur exactly once"
                }
                val retryAfter = retryAfterValues.single()
                check(
                    retryAfter.isNotEmpty() && retryAfter.length <= 20 &&
                        retryAfter.all { it in '0'..'9' },
                ) { "Kagemusha Torii Retry-After must be a positive u64 delay" }
                val significant = retryAfter.dropWhile { it == '0' }
                check(
                    significant.isNotEmpty() &&
                        (significant.length < 20 ||
                            significant == "18446744073709551615" ||
                            significant < "18446744073709551615"),
                ) { "Kagemusha Torii Retry-After must be a positive u64 delay" }
            }

            internal fun requireOperationReferenceMatches(
                reference: OperationReferenceProjection,
                identity: OperationIdentity,
            ): OperationHandle {
                check(reference.identity == identity) {
                    "Kagemusha Torii response identity must match the submitted request"
                }
                check(reference.statusUri == "$OPERATIONS_PATH/${identity.operationIdHex()}") {
                    "Kagemusha Torii response status URI must match the submitted operation"
                }
                return OperationHandle(
                    reference.identity,
                    reference.transactionHash(),
                )
            }

            internal fun requireOperationStatusMatches(
                status: OperationStatusProjection,
                handle: OperationHandle,
            ) {
                check(status.identity == handle.identity) {
                    "Kagemusha Torii status identity must match the accepted operation"
                }
                handle.acceptValidatedStatus(status)
            }

            private fun stripTrailingSlash(value: String): String = value.trimEnd('/')
        }

        private val baseUri: String

        init {
            require(
                baseUri.isAbsolute &&
                    !baseUri.isOpaque &&
                    !baseUri.host.isNullOrEmpty() &&
                    baseUri.rawQuery == null &&
                    baseUri.rawFragment == null &&
                    baseUri.rawUserInfo == null,
            ) { "baseUri must be an absolute credential-free HTTP URI" }
            require(baseUri.scheme.equals("http", true) || baseUri.scheme.equals("https", true)) {
                "baseUri must use HTTP or HTTPS"
            }
            require(requestTimeout == null || !requestTimeout.isNegative) {
                "requestTimeout must be non-negative"
            }
            this.baseUri = stripTrailingSlash(baseUri.toString())
        }

        fun getOfflineCapability(): CompletableFuture<OfflineStatus> {
            return execute(
                TransportRequest.builder()
                    .setMethod("GET")
                    .setUri(URI.create("$baseUri$CAPABILITY_PATH"))
                    .addHeader("Accept", JSON_MEDIA_TYPE)
                    .setTimeout(requestTimeout)
                    .setMaximumResponseBytes(MAX_TORII_READINESS_RESPONSE_BYTES.toLong())
                    .build(),
                200,
                JSON_MEDIA_TYPE,
            ).thenApply { OfflineStatus.decode(it.body) }
        }

        fun getRecipientRegistrationLineage(
            query: RecipientLineageQueryV2,
            canonicalAuth: ToriiCanonicalRequestAuth,
        ): CompletableFuture<RecipientRegistrationLineage> {
            val target = URI.create("$baseUri$RECEIVER_LINEAGE_PATH")
            val body = query.noritoEncoded()
            val timestampMs = canonicalAuth.timestampMs
            val nonce = canonicalAuth.nonce
            require((timestampMs == null) == (nonce == null)) {
                "timestampMs and nonce must be provided together"
            }
            val authHeaders = if (timestampMs == null) {
                CanonicalRequestSigner.buildHeaders(
                    localSigningContext.networkId(),
                    "POST",
                    target,
                    body,
                    canonicalAuth.accountId,
                    canonicalAuth.privateKey,
                )
            } else {
                CanonicalRequestSigner.buildHeaders(
                    localSigningContext.networkId(),
                    "POST",
                    target,
                    body,
                    canonicalAuth.accountId,
                    canonicalAuth.privateKey,
                    timestampMs,
                    nonce!!,
                )
            }
            val builder = TransportRequest.builder()
                .setMethod("POST")
                .setUri(target)
                .addHeader("Accept", NORITO_MEDIA_TYPE)
                .addHeader("Content-Type", NORITO_MEDIA_TYPE)
                .setBody(body)
                .setTimeout(requestTimeout)
                .setMaximumResponseBytes(MAX_TORII_RECIPIENT_LINEAGE_RESPONSE_BYTES.toLong())
            authHeaders.forEach { (name, value) -> builder.addHeader(name, value) }
            return execute(
                builder.build(),
                200,
            ).thenApply { RecipientRegistrationLineage(it.body) }
        }

        fun submitTopUp(request: TopUpRequest): CompletableFuture<OperationHandle> {
            val identity = topUpRequestIdentityProjector(request)
            check(identity.kind == OperationKind.TOP_UP) {
                "native top-up request identity has the wrong operation kind"
            }
            return submitCommand(
                TOP_UP_PATH,
                request.noritoEncoded(),
                identity,
            )
        }

        fun submitRedeem(request: RedeemSubmissionRequest): CompletableFuture<OperationHandle> {
            val identity = redeemRequestIdentityProjector(request)
            check(identity.kind == OperationKind.REDEEM) {
                "native redemption request identity has the wrong operation kind"
            }
            return submitCommand(
                REDEEM_PATH,
                request.noritoEncoded(),
                identity,
            )
        }

        fun getOperation(handle: OperationHandle): CompletableFuture<OperationStatus> {
            val id = handle.identity.operationIdHex()
            return execute(
                TransportRequest.builder()
                    .setMethod("GET")
                    .setUri(URI.create("$baseUri$OPERATIONS_PATH/$id"))
                    .addHeader("Accept", NORITO_MEDIA_TYPE)
                    .setTimeout(requestTimeout)
                    .setMaximumResponseBytes(MAX_TORII_OPERATION_STATUS_BYTES.toLong())
                    .build(),
                200,
            ).thenApply {
                val status = OperationStatus(it.body)
                requireOperationStatusMatches(operationStatusProjector(status), handle)
                status
            }
        }

        private fun submitCommand(
            path: String,
            request: ByteArray,
            identity: OperationIdentity,
        ): CompletableFuture<OperationHandle> {
            val id = identity.operationIdHex()
            return execute(
                TransportRequest.builder()
                    .setMethod("POST")
                    .setUri(URI.create("$baseUri$path"))
                    .addHeader("Accept", NORITO_MEDIA_TYPE)
                    .addHeader("Content-Type", NORITO_MEDIA_TYPE)
                    .addHeader("Idempotency-Key", id)
                    .setBody(request)
                    .setTimeout(requestTimeout)
                    .setMaximumResponseBytes(MAX_TORII_OPERATION_REFERENCE_BYTES.toLong())
                    .build(),
                202,
            ).thenApply {
                requireCommandResponseHeaders(it, id)
                val reference = OperationReference(it.body)
                requireOperationReferenceMatches(
                    operationReferenceProjector(reference),
                    identity,
                )
            }
        }

        private fun execute(
            request: TransportRequest,
            expectedStatus: Int,
            expectedMediaType: String = NORITO_MEDIA_TYPE,
        ): CompletableFuture<TransportResponse> = transport.execute(request).thenApply { response ->
            check(response.statusCode == expectedStatus) {
                "Kagemusha Torii request failed with HTTP ${response.statusCode}"
            }
            val contentTypes = response.headers["Content-Type"].orEmpty()
            check(contentTypes.size == 1 && contentTypes.single().equals(expectedMediaType, true)) {
                "Kagemusha Torii response must use $expectedMediaType"
            }
            response
        }
    }

    /** Owns one native artifact spool until installation or cancellation. */
    class ArtifactIngest internal constructor(initialHandle: Long) : AutoCloseable {
        private var handle = initialHandle
        private var finalized = false
        private var installClaimed = false

        @Synchronized
        fun write(chunk: ByteArray) {
            requireOpen(allowFinalized = false)
            nativeArtifactWriteV4(handle, requireChunk(chunk))
        }

        fun finish() {
            withHeavyProofPermit("artifact finalization") {
                synchronized(this) {
                    requireOpen(allowFinalized = false)
                    nativeArtifactFinalizeV4(handle)
                    finalized = true
                }
            }
        }

        @Synchronized
        fun isFinalized(): Boolean = finalized

        @Synchronized
        override fun close() {
            if (handle == 0L) return
            check(!installClaimed) { "artifact ingest is being installed" }
            nativeArtifactCancelV4(handle)
            handle = 0
            finalized = false
        }

        @Synchronized
        internal fun claimFinalizedHandle(): Long {
            check(handle != 0L && finalized && !installClaimed) {
                "artifact ingest is not installable"
            }
            installClaimed = true
            return handle
        }

        @Synchronized
        internal fun releaseInstallClaim(expectedHandle: Long) {
            if (handle == expectedHandle) installClaimed = false
        }

        @Synchronized
        internal fun relinquishInstalledHandle(expectedHandle: Long) {
            check(handle == expectedHandle && finalized && installClaimed) {
                "artifact install ownership mismatch"
            }
            handle = 0
            finalized = false
            installClaimed = false
        }

        private fun requireOpen(allowFinalized: Boolean) {
            check(handle != 0L) { "artifact ingest is closed" }
            check(allowFinalized || !finalized) { "artifact ingest is already finalized" }
            check(!installClaimed) { "artifact ingest is being installed" }
        }
    }

    /**
     * Locally trusted material required to authenticate one published Kagemusha release.
     *
     * The policy must be provisioned from the deployment trust root, not copied from the
     * downloaded release. Native authenticates the runner-signed internal-validation receipt,
     * verifies signer-role thresholds, and hashes both external evidence files before validating
     * the candidate-bound promotion record and consuming any finalized artifact handle.
     */
    class ReleaseAuthentication(
        trustedPolicyNorito: ByteArray,
        releaseAttestationNorito: ByteArray,
        internalValidationReceiptNorito: ByteArray,
        benchmarkEvidence: ByteArray,
        cryptographicReview: ByteArray,
        promotionRecordNorito: ByteArray,
    ) {
        internal val trustedPolicyNorito = requireBoundedBytes(
            trustedPolicyNorito,
            "trustedPolicyNorito",
            MAX_TRUSTED_RELEASE_POLICY_BYTES,
        )
        internal val releaseAttestationNorito = requireBoundedBytes(
            releaseAttestationNorito,
            "releaseAttestationNorito",
            MAX_RELEASE_ATTESTATION_BYTES,
        )
        internal val internalValidationReceiptNorito = requireBoundedBytes(
            internalValidationReceiptNorito,
            "internalValidationReceiptNorito",
            MAX_INTERNAL_VALIDATION_RECEIPT_BYTES,
        )
        internal val benchmarkEvidence = requireBoundedBytes(
            benchmarkEvidence,
            "benchmarkEvidence",
            MAX_RELEASE_EVIDENCE_BYTES,
        )
        internal val cryptographicReview = requireBoundedBytes(
            cryptographicReview,
            "cryptographicReview",
            MAX_CRYPTOGRAPHIC_REVIEW_BYTES,
        )
        internal val promotionRecordNorito = requireBoundedBytes(
            promotionRecordNorito,
            "promotionRecordNorito",
            MAX_PROMOTION_RECORD_BYTES,
        )
    }

    /** Coordinates one authenticated, atomic eight-artifact generation install. */
    class ArtifactInstallSession internal constructor(
        manifest: ByteArray,
        manifestDigest: ByteArray,
        releaseAuthentication: ReleaseAuthentication,
    ) : AutoCloseable {
        private val manifestNorito = manifest.copyOf()
        private val manifestSha256 = manifestDigest.copyOf()
        private val trustedPolicyNorito = releaseAuthentication.trustedPolicyNorito.copyOf()
        private val releaseAttestationNorito =
            releaseAuthentication.releaseAttestationNorito.copyOf()
        private val internalValidationReceiptNorito =
            releaseAuthentication.internalValidationReceiptNorito.copyOf()
        private val benchmarkEvidence = releaseAuthentication.benchmarkEvidence.copyOf()
        private val cryptographicReview = releaseAuthentication.cryptographicReview.copyOf()
        private val promotionRecordNorito = releaseAuthentication.promotionRecordNorito.copyOf()
        private val artifacts = linkedMapOf<ArtifactRoleV4, ArtifactIngest>()
        private val artifactDigests = mutableListOf<String>()
        private var installed = false
        private var closed = false

        @Synchronized
        fun beginArtifact(
            role: ArtifactRoleV4,
            expectedArtifactSha256: ByteArray,
        ): ArtifactIngest {
            requirePending()
            check(artifacts.size < ARTIFACT_COUNT) { "artifact set already has eight streams" }
            require(role == ArtifactRoleV4.entries[artifacts.size]) {
                "artifact role is not in canonical V4 order"
            }
            val digest = requireDigest(expectedArtifactSha256, "expectedArtifactSha256")
            val key = hex(digest)
            require(!artifactDigests.contains(key)) { "expectedArtifactSha256 is duplicated" }
            return beginArtifactIngest(manifestNorito, manifestSha256, digest)
                .also {
                    artifacts[role] = it
                    artifactDigests += key
                }
        }

        fun install() {
            withHeavyProofPermit("artifact install") {
                synchronized(this) {
                    requirePending()
                    check(artifacts.size == ARTIFACT_COUNT) {
                        "artifact set must contain exactly eight streams"
                    }
                    requireCanonicalV4ArtifactRoleInventory(artifacts.keys.toList())
                    val ordered = artifacts.values.toList()
                    val handles = LongArray(ARTIFACT_COUNT)
                    var claimed = 0
                    try {
                        while (claimed < ordered.size) {
                            handles[claimed] = ordered[claimed].claimFinalizedHandle()
                            claimed += 1
                        }
                        nativeArtifactSetInstallV4(
                            manifestNorito,
                            manifestSha256,
                            trustedPolicyNorito,
                            releaseAttestationNorito,
                            internalValidationReceiptNorito,
                            benchmarkEvidence,
                            cryptographicReview,
                            promotionRecordNorito,
                            handles,
                        )
                    } catch (failure: Throwable) {
                        repeat(claimed) { index ->
                            ordered[index].releaseInstallClaim(handles[index])
                        }
                        throw failure
                    }
                    ordered.forEachIndexed { index, ingest ->
                        ingest.relinquishInstalledHandle(handles[index])
                    }
                    artifacts.clear()
                    artifactDigests.clear()
                    installed = true
                }
            }
        }

        @Synchronized
        fun isInstalled(): Boolean =
            !closed && nativeArtifactSetIsInstalledV4(manifestNorito, manifestSha256)

        @Synchronized
        fun artifactBinding(): ArtifactBindingV4 {
            check(installed && !closed && isInstalled()) { "artifact set is not installed" }
            return ArtifactBindingV4(
                nativeBuildArtifactBindingV4(manifestNorito, manifestSha256),
            )
        }

        fun uninstall() {
            withHeavyProofPermit("artifact uninstall") {
                synchronized(this) {
                    if (!installed || closed) return@withHeavyProofPermit
                    nativeArtifactSetUninstallV4(manifestSha256)
                    installed = false
                    closed = true
                }
            }
        }

        @Synchronized
        override fun close() {
            if (closed || installed) return
            var firstFailure: RuntimeException? = null
            artifacts.values.forEach { ingest ->
                try {
                    ingest.close()
                } catch (failure: RuntimeException) {
                    if (firstFailure == null) firstFailure = failure
                }
            }
            artifacts.clear()
            artifactDigests.clear()
            closed = true
            firstFailure?.let { throw it }
        }

        private fun requirePending() {
            check(!closed && !installed) { "artifact install session is not pending" }
        }
    }
}
