package org.hyperledger.iroha.sdk.offline

import java.net.URI
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.util.ArrayList
import java.util.Collections
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.ZkMerklePathEntry
import org.hyperledger.iroha.sdk.client.ZkMerklePathResponse
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

/**
 * ABI-20 Kagemusha V4 artifact streaming and capability bridge.
 *
 * This is the sole first-release offline-cash surface. It authenticates the opaque eight-file proof
 * artifact set and validates exact typed request/payment/acknowledgement and proof-bound membership
 * archives. Proof execution remains fail-closed while the native backend reports unavailable.
 * V4 lifecycle results remain opaque until dedicated ABI-20 projections are available; no V2
 * result decoder is used as a substitute.
 */
class KagemushaRecursiveSpendProver private constructor() {
    /** Canonical ABI-20 artifact roles. Declaration order is part of the native contract. */
    enum class ArtifactRoleV4(val fileName: String) {
        STEP_EQ_PARAMETERS("step-eq.parameters.krv4"),
        STEP_EQ_PROVING_KEY("step-eq.proving-key.krv4"),
        STEP_EQ_VERIFYING_KEY("step-eq.verifying-key.krv4"),
        STEP_EQ_BOOTSTRAP_WITNESS("step-eq.bootstrap-witness.krv4"),
        STEP_EP_PARAMETERS("step-ep.parameters.krv4"),
        STEP_EP_PROVING_KEY("step-ep.proving-key.krv4"),
        STEP_EP_VERIFYING_KEY("step-ep.verifying-key.krv4"),
        STEP_EP_BOOTSTRAP_WITNESS("step-ep.bootstrap-witness.krv4"),
    }

    companion object {
        const val V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 20
        const val REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = V4_REQUIRED_NATIVE_BRIDGE_ABI_VERSION
        const val V4_ARTIFACT_MANIFEST_SCHEMA: String =
            "kagemusha.offline.recursive_spend.artifact_manifest.v4"
        const val ARTIFACT_MANIFEST_SCHEMA: String = V4_ARTIFACT_MANIFEST_SCHEMA
        val V4_ARTIFACT_FILES: List<String> = listOf(
            "step-eq.parameters.krv4",
            "step-eq.proving-key.krv4",
            "step-eq.verifying-key.krv4",
            "step-eq.bootstrap-witness.krv4",
            "step-ep.parameters.krv4",
            "step-ep.proving-key.krv4",
            "step-ep.verifying-key.krv4",
            "step-ep.bootstrap-witness.krv4",
        )
        val ARTIFACT_FILES: List<String> = V4_ARTIFACT_FILES
        const val V4_ARTIFACT_COUNT: Int = 8
        const val ARTIFACT_COUNT: Int = V4_ARTIFACT_COUNT
        const val MAX_MANIFEST_BYTES: Int = 1024 * 1024
        const val MAX_TRUSTED_RELEASE_POLICY_BYTES: Int = 64 * 1024
        const val MAX_RELEASE_ATTESTATION_BYTES: Int = 1024 * 1024
        const val MAX_RELEASE_EVIDENCE_BYTES: Int = 16 * 1024 * 1024
        const val MAX_PEER_TEXT_ENVELOPE_BYTES: Int = 12 * 1024
        const val MAX_PEER_TEXT_ARCHIVE_BYTES: Int = 9_211
        const val MAX_PEER_ARCHIVE_BYTES: Int = 32 * 1024
        const val MAX_LOCAL_REQUEST_ARCHIVE_BYTES: Int = 8 * 1024 * 1024
        const val MAX_LOCAL_RESULT_ARCHIVE_BYTES: Int = 64 * 1024
        const val MAX_TORII_REQUEST_BYTES: Int = 512 * 1024
        const val MAX_TORII_RESPONSE_BYTES: Int = 4 * 1024 * 1024
        const val MAXIMUM_INPUTS_PER_TRANSITION: Int = 2
        const val MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS: Int = MAXIMUM_INPUTS_PER_TRANSITION
        const val MAXIMUM_BRANCH_CLAIMS: Int = 2
        const val MAXIMUM_PEER_HOPS: Int = 8
        const val MAXIMUM_PROOF_STEPS: Int = 128
        const val CONFIDENTIAL_TREE_DEPTH: Int = 16

        private const val EXACT_STATE_PROJECTION_VERSION: Int = 1

        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val artifactBridgeAvailable = loadArtifactBridge()
        private val proofBackendAvailable = loadProofBackendCapability()

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

        @JvmStatic
        fun isProofBackendAvailable(): Boolean = proofBackendAvailable

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
        ): AppendRequestV4 = AppendRequestV4(archive, changeOpening)

        @JvmStatic
        fun decodeVerifyRequestV4(archive: ByteArray): VerifyRequestV4 = VerifyRequestV4(archive)

        @JvmStatic
        fun decodeTopUpAnchorV4(archive: ByteArray): TopUpAnchorV4 = TopUpAnchorV4(archive)

        @JvmStatic
        fun decodeBundleV4(archive: ByteArray): BundleV4 = BundleV4(archive)

        @JvmStatic
        fun decodeTopUpFinalityEvidenceV4(archive: ByteArray): TopUpFinalityEvidenceV4 =
            TopUpFinalityEvidenceV4(archive)

        /** Restore one secret-bearing V4 branch from independently persisted typed archives. */
        @JvmStatic
        fun restoreSpendableBranchV4(
            bundle: BundleV4,
            membershipWitness: NoteMembershipWitness,
            opening: NoteOpening,
        ): SpendableBranchV4 = SpendableBranchV4(bundle, membershipWitness, opening)

        @JvmStatic
        fun decodeRedeemRequestV4(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): RedeemRequestV4 = RedeemRequestV4(archive, changeOpening)

        @JvmStatic
        fun decodeInitResultV4(archive: ByteArray): InitResultV4 = InitResultV4(archive)

        @JvmStatic
        fun decodeSplitResultV4(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): SplitResultV4 = SplitResultV4(archive, changeOpening)

        @JvmStatic
        fun decodeVerifyResultV4(archive: ByteArray): VerifyResultV4 = VerifyResultV4(archive)

        @JvmStatic
        fun decodeRedeemBuildResultV4(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): RedeemBuildResultV4 = RedeemBuildResultV4(archive, changeOpening)

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
        fun projectOperationStatus(status: OperationStatus): OperationStatusProjection {
            requireArtifactBridge()
            val fields = nativeProjectOperationStatusV2(status.noritoEncoded())
            requireFieldCount(fields, 10, "operation status projection")
            val state = when (canonicalText(fields[0], "operationState")) {
                "pending" -> OperationState.PENDING
                "applied" -> OperationState.APPLIED
                "rejected" -> OperationState.REJECTED
                else -> error("native Kagemusha operation state is invalid")
            }
            val kind = when (canonicalText(fields[1], "operationKind")) {
                "top_up" -> OperationKind.TOP_UP
                "redeem" -> OperationKind.REDEEM
                else -> error("native Kagemusha operation kind is invalid")
            }
            val heightOrSubmittedAt = fields[4].takeIf { it.isNotEmpty() }
                ?.let { longInteger(it, "operationHeightOrSubmittedAt") }
            val serverTime = fields[5].takeIf { it.isNotEmpty() }
                ?.let { longInteger(it, "serverTimeMilliseconds") }
            val finalizedTopUp = if (fields[6].isNotEmpty() || fields[7].isNotEmpty()) {
                check(state == OperationState.APPLIED && kind == OperationKind.TOP_UP &&
                    fields[6].isNotEmpty() && fields[7].isNotEmpty() &&
                    heightOrSubmittedAt != null && serverTime != null) {
                    "native Kagemusha finalized top-up fields are invalid"
                }
                FinalizedTopUp(
                    TopUpAnchor(fields[6]),
                    TopUpFinalityProof(fields[7]),
                    heightOrSubmittedAt,
                    serverTime,
                )
            } else {
                null
            }
            val rejection = if (fields[8].isNotEmpty() || fields[9].isNotEmpty()) {
                check(state == OperationState.REJECTED && fields[8].isNotEmpty() && fields[9].isNotEmpty()) {
                    "native Kagemusha rejection fields are invalid"
                }
                OperationRejection(
                    canonicalText(fields[8], "rejectionCode"),
                    canonicalText(fields[9], "rejectionMessage"),
                )
            } else {
                null
            }
            return OperationStatusProjection(
                state,
                kind,
                requireDigest(fields[2], "operationId"),
                requireDigest(fields[3], "transactionHash"),
                if (state == OperationState.PENDING) heightOrSubmittedAt else null,
                if (state == OperationState.APPLIED) heightOrSubmittedAt else null,
                serverTime,
                finalizedTopUp,
                rejection,
            )
        }

        /** Decode the authoritative, snapshot-bound Torii Kagemusha capability response. */
        @JvmStatic
        fun projectReadiness(readiness: Readiness): ReadinessProjection {
            requireArtifactBridge()
            val fields = nativeProjectReadinessV2(readiness.noritoEncoded())
            check(fields.size >= 15) { "native Kagemusha readiness projection returned invalid fields" }
            val blockerCount = integer(fields[14], "blockerCount")
            check(blockerCount >= 0 && fields.size == 15 + blockerCount * 2) {
                "native Kagemusha readiness projection returned invalid blockers"
            }
            val blockers = ArrayList<ReadinessBlocker>(blockerCount)
            repeat(blockerCount) { index ->
                blockers.add(
                    ReadinessBlocker(
                        canonicalText(fields[15 + index * 2], "blockerCode"),
                        canonicalText(fields[16 + index * 2], "blockerMessage"),
                    ),
                )
            }
            return ReadinessProjection(
                requiredBridgeAbiVersion = integer(fields[0], "requiredBridgeAbiVersion"),
                maximumHops = integer(fields[1], "maximumHops"),
                assetDefinitionId = canonicalText(fields[2], "assetDefinitionId"),
                assetScale = fields[3].takeIf { it.isNotEmpty() }?.let { integer(it, "assetScale") },
                evaluatedBlockHeight = longInteger(fields[4], "evaluatedBlockHeight"),
                evaluatedBlockHash = requireDigest(fields[5], "evaluatedBlockHash"),
                proofBackendAvailable = bool(fields[6], "proofBackendAvailable"),
                recursiveLineageSupported = bool(fields[7], "recursiveLineageSupported"),
                ready = bool(fields[8], "ready"),
                transferVerifier = activeVerifier(fields[9]),
                topUpShieldVerifier = activeVerifier(fields[10]),
                unshieldVerifier = activeVerifier(fields[11]),
                recursiveStepEqVerifier = activeVerifier(fields[12]),
                recursiveStepEpVerifier = activeVerifier(fields[13]),
                blockers = blockers,
            )
        }

        @JvmStatic
        fun prepareRequestAuthorization(
            authority: String,
            deviceId: String,
            operationId: ByteArray,
            issuedAtMilliseconds: Long,
            expiresAtMilliseconds: Long,
            nonce: ByteArray,
            payloadDigest: ByteArray,
            appAttestEvidence: ByteArray? = null,
        ): RequestAuthorizationPreparation {
            requireArtifactBridge()
            val fields = nativePrepareAuthorizationV2(
                utf8(authority, "authority"),
                utf8(deviceId, "deviceId"),
                requireDigest(operationId, "operationId"),
                issuedAtMilliseconds,
                expiresAtMilliseconds,
                requireDigest(nonce, "nonce"),
                requireDigest(payloadDigest, "payloadDigest"),
                appAttestEvidence?.copyOf() ?: ByteArray(0),
            )
            requireFieldCount(fields, 5, "authorization preparation")
            return RequestAuthorizationPreparation(
                RequestAuthorizationTemplate(fields[0]),
                fields[1],
                fields[2],
                fields[3],
                fields[4].takeIf { it.isNotEmpty() },
            )
        }

        @JvmStatic
        fun signRequestAuthorization(
            preparation: RequestAuthorizationPreparation,
            signature: ByteArray,
        ): RequestAuthorization {
            requireArtifactBridge()
            return RequestAuthorization(
                nativeCreateAuthorizationV2(
                    preparation.template.noritoEncoded(),
                    requiredBytes(signature, "signature"),
                ),
            )
        }

        @JvmStatic
        fun finalizeTopUp(
            unsigned: TopUpUnsigned,
            authorization: RequestAuthorization,
        ): TopUpRequest {
            requireArtifactBridge()
            return TopUpRequest(
                nativeFinalizeTopUpV2(unsigned.noritoEncoded(), authorization.noritoEncoded()),
            )
        }

        @JvmStatic
        fun finalizeTopUp(
            preparation: TopUpPreparation,
            authorization: RequestAuthorization,
        ): TopUpRequest = finalizeTopUp(preparation.unsigned, authorization)

        @JvmStatic
        fun prepareTopUp(
            chainId: String,
            assetDefinitionId: String,
            payerAccountId: String,
            amount: KagemushaScaledAmount,
            operationId: ByteArray,
            openingSpendKey: ByteArray,
            openingRho: ByteArray,
            openingDiversifier: ByteArray,
            zeroPath: TopUpZeroPath,
            shieldVerifierCommitment: ByteArray,
            artifactBinding: ArtifactBinding,
        ): TopUpPreparation {
            requireArtifactBridge()
            val spendKeyCopy = requireDigest(openingSpendKey, "openingSpendKey")
            val rhoCopy = requireDigest(openingRho, "openingRho")
            val diversifierCopy = requireDigest(openingDiversifier, "openingDiversifier")
            val fields = try {
                nativePrepareTopUpV2(
                    utf8(chainId, "chainId"),
                    utf8(assetDefinitionId, "assetDefinitionId"),
                    utf8(payerAccountId, "payerAccountId"),
                    utf8(amount.atomicUnits, "atomicUnits"),
                    amount.scale,
                    requireDigest(operationId, "operationId"),
                    spendKeyCopy,
                    rhoCopy,
                    diversifierCopy,
                    zeroPath.leafIndex,
                    zeroPath.flattenedSiblings(),
                    zeroPath.directions(),
                    zeroPath.root(),
                    requireDigest(shieldVerifierCommitment, "shieldVerifierCommitment"),
                    artifactBinding.noritoEncoded(),
                )
            } finally {
                spendKeyCopy.fill(0)
                rhoCopy.fill(0)
                diversifierCopy.fill(0)
            }
            requireFieldCount(fields, 11, "top-up preparation")
            return TopUpPreparation(
                TopUpUnsigned(fields[0]),
                fields[1],
                NoteOpening(fields[2]),
                fields[3],
                fields[4],
                fields[5],
                fields[6],
                fields[7],
                amount(fields[8], fields[9]),
                integer(fields[10], "leafIndex"),
            )
        }

        @JvmStatic
        fun finalizeRedeemV4(
            buildResult: RedeemBuildResultV4,
            authorization: RequestAuthorization,
        ): RedeemFinalization {
            buildResult.noritoEncoded()
            authorization.noritoEncoded()
            requireV4ProofBackend()
            error("native Kagemusha V4 redeem finalization is unavailable")
        }

        @JvmStatic
        fun prepareRecipientPaymentRequest(
            chainId: String,
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
            val spendKeyCopy = requireDigest(spendKey, "spendKey")
            val rhoCopy = requireDigest(rho, "rho")
            val diversifierCopy = requireDigest(diversifier, "diversifier")
            val fields = try {
                nativePrepareRecipientRequestV2(
                    utf8(chainId, "chainId"),
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
                )
            } finally {
                spendKeyCopy.fill(0)
                rhoCopy.fill(0)
                diversifierCopy.fill(0)
            }
            requireFieldCount(fields, 5, "recipient request preparation")
            return RecipientRequestPreparation(
                RecipientRequestPayload(fields[0]),
                fields[1],
                NoteOpening(fields[2]),
                fields[3],
                fields[4],
                amount,
            )
        }

        /** Prepare one local-only opening for sender change or partial redemption change. */
        @JvmStatic
        fun prepareNoteOpening(
            spendKey: ByteArray,
            rho: ByteArray,
            diversifier: ByteArray,
        ): NoteOpening {
            requireArtifactBridge()
            val spendKeyCopy = requireDigest(spendKey, "spendKey")
            val rhoCopy = requireDigest(rho, "rho")
            val diversifierCopy = requireDigest(diversifier, "diversifier")
            return try {
                NoteOpening(nativePrepareNoteOpeningV2(spendKeyCopy, rhoCopy, diversifierCopy))
            } finally {
                spendKeyCopy.fill(0)
                rhoCopy.fill(0)
                diversifierCopy.fill(0)
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

        @JvmStatic
        fun projectRecipientPaymentRequest(
            request: RecipientPaymentRequest,
        ): RecipientRequestProjection {
            requireArtifactBridge()
            val fields = nativeProjectRecipientRequestV2(request.noritoEncoded())
            requireFieldCount(fields, 14, "recipient request projection")
            return RecipientRequestProjection(
                chainId = canonicalText(fields[0], "chainId"),
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
            val openingArchive = opening.noritoEncoded()
            val membershipArchive = outputMembershipPaths.nativeArchive()
            return try {
                InitRequestV4(nativeBuildInitRequestV4(
                    topUpAnchor.noritoEncoded(),
                    topUpFinalityProof.noritoEncoded(),
                    topUpFinalityRosterArtifact.noritoEncoded(),
                    openingArchive,
                    membershipArchive,
                ))
            } finally {
                openingArchive.fill(0)
                membershipArchive.fill(0)
            }
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
        ): AppendRequestV4 {
            require(inputs.size in 1..MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS) {
                "inputs must contain one or two spendable branches"
            }
            require(inputs.map { it.bundle }.distinct().size == inputs.size) {
                "inputs must refer to distinct V4 bundles"
            }
            require(outputMembershipPaths.recipient != null) {
                "append requires a recipient output path"
            }
            require((outputMembershipPaths.change != null) == (changeOpening != null)) {
                "change output membership must be present exactly when changeOpening is present"
            }
            requireArtifactBridge()
            requireV4ProofBackend()
            val bundles = inputs.map { it.bundle.noritoEncoded() }.toTypedArray()
            val openings = inputs.map { it.opening.noritoEncoded() }.toTypedArray()
            val witnesses = inputs.map { it.membershipWitness.noritoEncoded() }.toTypedArray()
            val change = changeOpening?.noritoEncoded() ?: byteArrayOf()
            val outputMembership = outputMembershipPaths.nativeArchive()
            val verifier = requireDigest(transferVerifierCommitment, "transferVerifierCommitment")
            val operation = requireDigest(operationId, "operationId")
            val archive = try {
                nativeBuildAppendRequestV4(
                    bundles,
                    openings,
                    witnesses,
                    change,
                    outputMembership,
                    verifier,
                    operation,
                    blockHeight,
                )
            } finally {
                bundles.forEach { it.fill(0) }
                openings.forEach { it.fill(0) }
                witnesses.forEach { it.fill(0) }
                change.fill(0)
                outputMembership.fill(0)
                verifier.fill(0)
                operation.fill(0)
            }
            return AppendRequestV4(archive, changeOpening)
        }

        @JvmStatic
        fun projectPeerPayment(payment: PeerPayment): PeerPaymentProjection {
            requireArtifactBridge()
            val fields = nativeProjectPeerPaymentV2(payment.noritoEncoded())
            val cursor = ProjectionCursor(fields, "peer payment projection")
            projectionVersion(cursor.next("version"), "peer payment projection")
            val operationId = requireDigest(cursor.next("operationId"), "operationId")
            val requestDigest = requireDigest(cursor.next("requestDigest"), "requestDigest")
            val projection = branchProjection(cursor)
            cursor.finish()
            val result = PeerPaymentProjection(projection, operationId, requestDigest)
            operationId.fill(0)
            requestDigest.fill(0)
            return result
        }

        @JvmStatic
        fun buildVerifyRequestV4(
            bundle: BundleV4,
            recipientRequest: RecipientPaymentRequest,
            topUpFinalityRosterArtifact: TopUpFinalityRosterArtifact,
            topUpFinalityEvidence: List<TopUpFinalityEvidenceV4>,
            maximumHops: Int,
            blockHeight: Long,
            verifiedAtMilliseconds: Long,
        ): VerifyRequestV4 {
            requireArtifactBridge()
            requireV4ProofBackend()
            val evidence = topUpFinalityEvidence.map { it.noritoEncoded() }.toTypedArray()
            return try {
                VerifyRequestV4(nativeBuildVerifyRequestV4(
                    bundle.noritoEncoded(),
                    recipientRequest.noritoEncoded(),
                    topUpFinalityRosterArtifact.noritoEncoded(),
                    evidence,
                    maximumHops,
                    blockHeight,
                    verifiedAtMilliseconds,
                ))
            } finally {
                evidence.forEach { it.fill(0) }
            }
        }

        @JvmStatic
        fun buildRedeemRequestV4(
            input: SpendableBranchV4,
            recipientAccountId: String,
            amount: KagemushaScaledAmount,
            changeOpening: NoteOpening?,
            changeOutputMembershipPaths: OutputMembershipPaths?,
            unshieldVerifierCommitment: ByteArray,
            operationId: ByteArray,
            blockHeight: Long,
        ): RedeemRequestV4 {
            requireArtifactBridge()
            requireV4ProofBackend()
            require((changeOpening != null) == (changeOutputMembershipPaths != null)) {
                "change output membership must be present exactly when changeOpening is present"
            }
            changeOutputMembershipPaths?.let {
                require(it.recipient == null && it.change != null) {
                    "redemption change requires exactly one change output path"
                }
            }
            val change = changeOpening?.noritoEncoded() ?: byteArrayOf()
            val outputMembership = changeOutputMembershipPaths?.nativeArchive() ?: byteArrayOf()
            val verifier = requireDigest(
                unshieldVerifierCommitment,
                "unshieldVerifierCommitment",
            )
            val operation = requireDigest(operationId, "operationId")
            val bundleArchive = input.bundle.noritoEncoded()
            val openingArchive = input.opening.noritoEncoded()
            val witnessArchive = input.membershipWitness.noritoEncoded()
            val recipient = utf8(recipientAccountId, "recipientAccountId")
            val atomicUnits = utf8(amount.atomicUnits, "atomicUnits")
            return try {
                RedeemRequestV4(
                    nativeBuildRedeemRequestV4(
                        bundleArchive, openingArchive, witnessArchive, recipient,
                        atomicUnits, amount.scale,
                        change, outputMembership, verifier, operation, blockHeight,
                    ),
                    changeOpening,
                )
            } finally {
                change.fill(0)
                outputMembership.fill(0)
                verifier.fill(0)
                operation.fill(0)
                bundleArchive.fill(0)
                openingArchive.fill(0)
                witnessArchive.fill(0)
                recipient.fill(0)
                atomicUnits.fill(0)
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
            requireProofBackend()
            return InitResultV4(callNativeLifecycle("init spend") {
                nativeInitSpendV4(request.noritoEncoded())
            })
        }

        /** Prove one exact recipient output and optional independently spendable sender change. */
        @JvmStatic
        fun appendSpendV4(
            request: AppendRequestV4,
            recipientRequest: RecipientPaymentRequest,
            verifiedAtMilliseconds: Long,
        ): SplitResultV4 {
            require(verifiedAtMilliseconds > 0) {
                "verifiedAtMilliseconds must be positive"
            }
            requireProofBackend()
            val changeOpening = request.changeOpening
            val secretArchive = request.consumeAndDestroy()
            return try {
                SplitResultV4(
                    callNativeLifecycle("append spend") {
                        nativeAppendSpendV4(
                            secretArchive,
                            recipientRequest.noritoEncoded(),
                            verifiedAtMilliseconds,
                        )
                    },
                    changeOpening,
                )
            } finally {
                secretArchive.fill(0)
            }
        }

        /** Verify the recursive proof, exact split bindings, membership, and hop limit. */
        @JvmStatic
        fun verifySpendV4(request: VerifyRequestV4): VerifyResultV4 {
            requireProofBackend()
            return VerifyResultV4(callNativeLifecycle("verify spend") {
                nativeVerifySpendV4(request.noritoEncoded())
            })
        }

        /** Build a full or partial redemption and its optional proof-bound offline change. */
        @JvmStatic
        fun buildRedeemV4(request: RedeemRequestV4): RedeemBuildResultV4 {
            requireProofBackend()
            val changeOpening = request.changeOpening
            val secretArchive = request.consumeAndDestroy()
            return try {
                RedeemBuildResultV4(
                    callNativeLifecycle("build redeem") {
                        nativeBuildRedeemV4(secretArchive)
                    },
                    changeOpening,
                )
            } finally {
                secretArchive.fill(0)
            }
        }

        @JvmStatic
        fun newToriiClient(baseUri: URI, transport: TransportExecutor): ToriiClient =
            ToriiClient(baseUri, transport)

        internal fun isExactBridgeAbi(abiVersion: Int): Boolean =
            abiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION

        internal fun detectExactNativeAvailability(
            loadLibrary: () -> Unit,
            abiVersion: () -> Int,
            symbolProbe: () -> Boolean,
        ): Boolean = try {
            loadLibrary()
            isExactBridgeAbi(abiVersion()) && symbolProbe()
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
                symbolProbe = {
                    expectIllegalArgumentProbe {
                        nativeArtifactBeginV4(byteArrayOf(0), ByteArray(32), ByteArray(32))
                    }
                },
            )

        private fun loadProofBackendCapability(): Boolean =
            detectExactNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                abiVersion = { nativeBridgeAbiVersion() },
                symbolProbe = { nativePastaCycleV4BackendAvailable() },
            )

        private fun expectIllegalArgumentProbe(probe: () -> Unit): Boolean = try {
            probe()
            false
        } catch (_: IllegalArgumentException) {
            true
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
            check(proofBackendAvailable) {
                "$LIBRARY_NAME ABI $REQUIRED_NATIVE_BRIDGE_ABI_VERSION Kagemusha proof backend is unavailable"
            }
        }

        private inline fun callNativeLifecycle(label: String, call: () -> ByteArray?): ByteArray =
            try {
                checkNotNull(call()) { "native Kagemusha $label returned no archive" }
            } catch (failure: UnsatisfiedLinkError) {
                throw IllegalStateException("native Kagemusha $label entrypoint is unavailable", failure)
            }

        private fun utf8(value: String?, field: String): ByteArray {
            require(value != null && value.isNotEmpty() && value == value.trim()) {
                "$field must be canonical non-empty text"
            }
            return value.toByteArray(Charsets.UTF_8)
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

        private fun canonicalText(value: ByteArray, field: String): String {
            val text = value.toString(Charsets.UTF_8)
            check(text.isNotEmpty() && text == text.trim() && text.none(Char::isISOControl)) {
                "native Kagemusha $field is invalid"
            }
            return text
        }

        private fun activeVerifier(archive: ByteArray): ActiveVerifier? {
            if (archive.isEmpty()) return null
            val fields = nativeProjectActiveVerifierV2(archive)
            requireFieldCount(fields, 9, "active verifier projection")
            return ActiveVerifier(
                backend = canonicalText(fields[0], "verifierBackend"),
                name = canonicalText(fields[1], "verifierName"),
                version = integer(fields[2], "verifierVersion"),
                circuitId = canonicalText(fields[3], "verifierCircuitId"),
                commitment = requireDigest(fields[4], "verifierCommitment"),
                publicInputsSchemaHash = requireDigest(fields[5], "publicInputsSchemaHash"),
                maximumProofBytes = integer(fields[6], "maximumProofBytes"),
                activationHeight = longInteger(fields[7], "activationHeight"),
                withdrawalHeight = fields[8].takeIf { it.isNotEmpty() }
                    ?.let { longInteger(it, "withdrawalHeight") },
            )
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
            val bundle = Bundle(cursor.next("bundle"))
            val witness = NoteMembershipWitness(cursor.next("membershipWitness"))
            val commitment = cursor.next("commitment")
            val spendNullifier = cursor.next("spendNullifier")
            val amount = amount(cursor.next("atomicUnits"), cursor.next("scale"))
            val hopCount = integer(cursor.next("hopCount"), "hopCount")
            val proofStepCount = integer(cursor.next("proofStepCount"), "proofStepCount")
            val bundleDigest = cursor.next("bundleDigest")
            val artifactBinding = ArtifactBinding(cursor.next("artifactBinding"))
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

        private fun requireChunk(value: ByteArray?): ByteArray {
            require(value != null && value.isNotEmpty()) { "chunk must not be empty" }
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
                    archive.size == NoritoHeader.HEADER_LENGTH + decoded.payload.size &&
                    header.encode().contentEquals(
                        archive.copyOfRange(0, NoritoHeader.HEADER_LENGTH),
                    ),
            ) { "$field must use canonical compact Norito framing" }
            header.validateChecksum(decoded.payload)
            return archive
        }

        private fun hex(digest: ByteArray): String = buildString(64) {
            for (octet in digest) append("%02x".format(octet.toInt() and 0xff))
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

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
            benchmarkEvidence: ByteArray,
            cryptographicReview: ByteArray,
            artifactHandles: LongArray,
        )

        @JvmStatic
        private external fun nativeArtifactSetIsInstalledV4(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
        ): Boolean

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
            chainId: ByteArray,
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
        @JvmStatic private external fun nativeBuildOutputMembershipPathsV4(initialRoot: ByteArray, finalRoot: ByteArray, recipientFields: Array<ByteArray>, changeFields: Array<ByteArray>, dummyFields: Array<ByteArray>): ByteArray
        @JvmStatic private external fun nativeBuildInitRequestV2(anchor: ByteArray, proof: ByteArray, roster: ByteArray): ByteArray
        @JvmStatic private external fun nativeBuildInitRequestV4(anchor: ByteArray, proof: ByteArray, roster: ByteArray, opening: ByteArray, outputMembership: ByteArray): ByteArray
        @JvmStatic private external fun nativeBuildAppendRequestV2(bundles: Array<ByteArray>, openings: Array<ByteArray>, witnesses: Array<ByteArray>, changeOpening: ByteArray, verifierCommitment: ByteArray, operationId: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeBuildAppendRequestV4(bundles: Array<ByteArray>, openings: Array<ByteArray>, witnesses: Array<ByteArray>, changeOpening: ByteArray, outputMembership: ByteArray, verifierCommitment: ByteArray, operationId: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeProjectPeerPaymentV2(payment: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectSplitResultV2(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildVerifyRequestV2(payment: ByteArray, request: ByteArray, maximumHops: Int, blockHeight: Long, verifiedAtMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeBuildVerifyRequestV4(bundle: ByteArray, recipientRequest: ByteArray, roster: ByteArray, finalityEvidence: Array<ByteArray>, maximumHops: Int, blockHeight: Long, verifiedAtMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeProjectVerifyResultV2(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildRedeemRequestV2(bundle: ByteArray, opening: ByteArray, witness: ByteArray, recipient: ByteArray, atomicUnits: ByteArray, scale: Int, changeOpening: ByteArray, verifierCommitment: ByteArray, operationId: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeBuildRedeemRequestV4(bundle: ByteArray, opening: ByteArray, membershipWitness: ByteArray, recipient: ByteArray, atomicUnits: ByteArray, scale: Int, changeOpening: ByteArray, changeOutputMembership: ByteArray, verifierCommitment: ByteArray, operationId: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeProjectRedeemBuildResultV2(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePrepareAcknowledgementV2(request: ByteArray, payment: ByteArray, acceptedAtMilliseconds: Long): Array<ByteArray>
        @JvmStatic private external fun nativeCreateAcknowledgementV2(payload: ByteArray, signature: ByteArray, request: ByteArray, payment: ByteArray): ByteArray
        @JvmStatic private external fun nativeVerifyAcknowledgementV2(acknowledgement: ByteArray, request: ByteArray, payment: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectReadinessV2(readiness: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectActiveVerifierV2(verifier: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeArtifactBindingV3(manifest: ByteArray, manifestSha256: ByteArray): ByteArray
        @JvmStatic private external fun nativePrepareAuthorizationV2(authority: ByteArray, deviceId: ByteArray, operationId: ByteArray, issuedAtMilliseconds: Long, expiresAtMilliseconds: Long, nonce: ByteArray, payloadDigest: ByteArray, appAttestEvidence: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeCreateAuthorizationV2(template: ByteArray, signature: ByteArray): ByteArray
        @JvmStatic private external fun nativeFinalizeTopUpV2(unsigned: ByteArray, authorization: ByteArray): ByteArray
        @JvmStatic private external fun nativeFinalizeRedeemV2(buildResult: ByteArray, authorization: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativePrepareTopUpV2(chainId: ByteArray, assetDefinition: ByteArray, payer: ByteArray, atomicUnits: ByteArray, scale: Int, operationId: ByteArray, spendKey: ByteArray, rho: ByteArray, diversifier: ByteArray, leafIndex: Int, flattenedSiblings: ByteArray, directions: ByteArray, root: ByteArray, shieldVerifierCommitment: ByteArray, artifactBinding: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectOperationStatusV2(status: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBranchClaimsConflictV2(left: ByteArray, right: ByteArray): Boolean
        @JvmStatic private external fun nativePrepareNoteOpeningV2(spendKey: ByteArray, rho: ByteArray, diversifier: ByteArray): ByteArray
        @JvmStatic private external fun nativeProjectRecipientRequestV2(request: ByteArray): Array<ByteArray>
    }

    /** Immutable canonical Norito archive; proof and accumulator bytes remain opaque. */
    abstract class CanonicalArchive internal constructor(
        archive: ByteArray,
        schema: String,
        field: String,
        maximumBytes: Int,
    ) {
        private val bytes = requireCanonicalArchive(archive, schema, field, maximumBytes)
        private var destroyed = false

        @Synchronized
        fun noritoEncoded(): ByteArray {
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

        final override fun equals(other: Any?): Boolean =
            other != null && this::class == other::class &&
                bytes.contentEquals((other as CanonicalArchive).bytes)

        final override fun hashCode(): Int = bytes.contentHashCode()
    }

    class RecipientPaymentRequest internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecipientPaymentRequestV2",
        "recipientPaymentRequest",
        MAX_PEER_ARCHIVE_BYTES,
    )

    class PeerPayment internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendPeerPaymentV2",
        "peerPayment",
        MAX_PEER_ARCHIVE_BYTES,
    )

    class ReceiverAcknowledgement internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaReceiverAcknowledgementV2",
        "receiverAcknowledgement",
        MAX_PEER_ARCHIVE_BYTES,
    )

    /** Proof-bound output membership state carried atomically with an accepted branch. */
    class NoteMembershipWitness internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaNoteMembershipWitnessV2",
        "noteMembershipWitness",
        MAX_PEER_ARCHIVE_BYTES,
    )

    class NoteOpening internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaNoteOpeningV2",
        "noteOpening",
        MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
    )

    class RecipientRequestPayload internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecipientPaymentRequestSigningPayloadV2",
        "recipientRequestPayload",
        MAX_PEER_ARCHIVE_BYTES,
    )

    class Bundle internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendBundleV2",
        "bundle",
        MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
    )

    /** Opaque ABI-20 recursive state; it is never decoded as a V2/V3 bundle. */
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
        MAX_PEER_ARCHIVE_BYTES,
    ) {
        fun conflictsWith(other: BranchClaim): Boolean {
            requireArtifactBridge()
            return nativeBranchClaimsConflictV2(noritoEncoded(), other.noritoEncoded())
        }
    }

    class ArtifactBinding internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendArtifactBindingV3",
        "artifactBinding",
        MAX_MANIFEST_BYTES,
    )

    class TopUpUnsigned internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpUnsignedV2",
        "topUpUnsigned",
        MAX_TORII_REQUEST_BYTES,
    )

    class TopUpRequest internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha.torii.v1.offline.top_up.request",
        "topUpRequest",
        MAX_TORII_REQUEST_BYTES,
    )

    class TopUpAnchor internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpAnchorV2",
        "topUpAnchor",
        MAX_TORII_RESPONSE_BYTES,
    )

    /** Finalized ABI-20 top-up receipt with a V4 artifact binding. */
    class TopUpAnchorV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpAnchorV4",
        "topUpAnchorV4",
        MAX_TORII_RESPONSE_BYTES,
    )

    class TopUpFinalityProof internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaTopUpFinalityProofV2",
        "topUpFinalityProof",
        MAX_TORII_RESPONSE_BYTES,
    )

    class TopUpFinalityRosterArtifact internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaTopUpFinalityRosterArtifactV2",
        "topUpFinalityRosterArtifact",
        MAX_TORII_RESPONSE_BYTES,
    )

    /** Complete V4 origin plus its stable compact-finality proof. */
    class TopUpFinalityEvidenceV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendTopUpFinalityEvidenceV4",
        "topUpFinalityEvidenceV4",
        MAX_TORII_RESPONSE_BYTES,
    )

    class RedeemSubmissionRequest internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "iroha.torii.v1.offline.redeem.request",
        "redeemSubmissionRequest",
        MAX_TORII_REQUEST_BYTES,
    )

    class RequestAuthorizationTemplate internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRequestAuthorizationV2",
        "requestAuthorizationTemplate",
        MAX_TORII_REQUEST_BYTES,
    )

    class RequestAuthorization internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRequestAuthorizationV2",
        "requestAuthorization",
        MAX_TORII_REQUEST_BYTES,
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
    class OutputMembershipPaths(
        initialRoot: ByteArray,
        finalRoot: ByteArray,
        val recipient: OutputMembershipLeafPaths?,
        val change: OutputMembershipLeafPaths?,
        val dummyPath: OutputMembershipPath,
    ) {
        private val initialRootValue = requireDigest(initialRoot, "initialRoot")
        private val finalRootValue = requireDigest(finalRoot, "finalRoot")

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
            val occupied = listOfNotNull(recipient?.leafIndex, change?.leafIndex) + dummyPath.leafIndex
            require(occupied.distinct().size == occupied.size) {
                "output and dummy paths must address distinct leaves"
            }
        }

        fun initialRoot(): ByteArray = initialRootValue.copyOf()

        fun finalRoot(): ByteArray = finalRootValue.copyOf()

        internal fun nativeArchive(): ByteArray {
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

    class InitRequestV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendInitLocalRequestV4",
        "initRequest",
        MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
    )

    /** Local secret-bearing append input. Native code consumes and wipes its openings. */
    class AppendRequestV4 internal constructor(
        archive: ByteArray,
        internal val changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendAppendLocalRequestV4",
            "appendRequest",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        override fun close() = destroy()
    }

    class VerifyRequestV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendVerifyLocalRequestV4",
        "verifyRequest",
        MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
    )

    /** Local secret-bearing redemption input. Native code consumes and wipes its openings. */
    class RedeemRequestV4 internal constructor(
        archive: ByteArray,
        internal val changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendRedeemLocalRequestV4",
            "redeemRequest",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        override fun close() = destroy()
    }

    class InitResultV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendInitResultV4",
        "initResult",
        MAX_LOCAL_RESULT_ARCHIVE_BYTES,
    )

    class SplitResultV4 internal constructor(
        archive: ByteArray,
        internal val changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
        "KagemushaRecursiveSpendSplitResultV4",
            "splitResult",
            MAX_LOCAL_RESULT_ARCHIVE_BYTES,
        )

    class VerifyResultV4 internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendVerifyResultV4",
        "verifyResult",
        MAX_LOCAL_RESULT_ARCHIVE_BYTES,
    )

    class RedeemBuildResultV4 internal constructor(
        archive: ByteArray,
        internal val changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
        "KagemushaRecursiveSpendRedeemBuildResultV4",
            "redeemBuildResult",
            MAX_LOCAL_RESULT_ARCHIVE_BYTES,
        )

    class RecipientRequestPreparation internal constructor(
        internal val payload: RecipientRequestPayload,
        signingBytes: ByteArray,
        val opening: NoteOpening,
        commitment: ByteArray,
        nullifier: ByteArray,
        val amount: KagemushaScaledAmount,
    ) {
        private val signingBytesValue = requiredBytes(signingBytes, "signingBytes")
        private val commitmentValue = requireDigest(commitment, "commitment")
        private val nullifierValue = requireDigest(nullifier, "nullifier")

        fun signingBytes(): ByteArray = signingBytesValue.copyOf()
        fun commitment(): ByteArray = commitmentValue.copyOf()
        fun nullifier(): ByteArray = nullifierValue.copyOf()
    }

    class RequestAuthorizationPreparation internal constructor(
        internal val template: RequestAuthorizationTemplate,
        signingBytes: ByteArray,
        operationId: ByteArray,
        payloadDigest: ByteArray,
        appAttestEvidenceSha256: ByteArray?,
    ) {
        private val signingBytesValue = requiredBytes(signingBytes, "signingBytes")
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val payloadDigestValue = requireDigest(payloadDigest, "payloadDigest")
        private val evidenceDigestValue = appAttestEvidenceSha256?.let {
            requireDigest(it, "appAttestEvidenceSha256")
        }

        fun signingBytes(): ByteArray = signingBytesValue.copyOf()

        fun operationId(): ByteArray = operationIdValue.copyOf()

        fun payloadDigest(): ByteArray = payloadDigestValue.copyOf()

        fun appAttestEvidenceSha256(): ByteArray? = evidenceDigestValue?.copyOf()
    }

    class TopUpPreparation internal constructor(
        val unsigned: TopUpUnsigned,
        authorizationDigest: ByteArray,
        val opening: NoteOpening,
        noteCommitment: ByteArray,
        spendNullifier: ByteArray,
        initialRoot: ByteArray,
        finalizedRoot: ByteArray,
        operationId: ByteArray,
        val amount: KagemushaScaledAmount,
        val leafIndex: Int,
    ) {
        private val authorizationDigestValue = requireDigest(authorizationDigest, "authorizationDigest")
        private val noteCommitmentValue = requireDigest(noteCommitment, "noteCommitment")
        private val spendNullifierValue = requireDigest(spendNullifier, "spendNullifier")
        private val initialRootValue = requireDigest(initialRoot, "initialRoot")
        private val finalizedRootValue = requireDigest(finalizedRoot, "finalizedRoot")
        private val operationIdValue = requireDigest(operationId, "operationId")

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

    class RecipientRequestProjection internal constructor(
        val chainId: String,
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
        private val requestIdValue = requireDigest(requestId, "requestId")
        private val outputCommitmentValue = requireDigest(outputCommitment, "outputCommitment")
        private val outputNullifierValue = requireDigest(outputNullifier, "outputNullifier")
        private val receiverKeyReferenceValue = requireDigest(receiverKeyReference, "receiverKeyReference")
        private val receiverPublicKeyValue = KagemushaDevicePublicKeyV2(receiverPublicKey)
        private val digestValue = requireDigest(digest, "requestDigest")

        fun requestId(): ByteArray = requestIdValue.copyOf()
        fun outputCommitment(): ByteArray = outputCommitmentValue.copyOf()
        fun outputNullifier(): ByteArray = outputNullifierValue.copyOf()
        fun receiverKeyReference(): ByteArray = receiverKeyReferenceValue.copyOf()
        fun receiverPublicKey(): KagemushaDevicePublicKeyV2 = receiverPublicKeyValue
        fun digest(): ByteArray = digestValue.copyOf()
    }

    open class BranchProjection internal constructor(
        val bundle: Bundle,
        val membershipWitness: NoteMembershipWitness,
        commitment: ByteArray,
        spendNullifier: ByteArray,
        val amount: KagemushaScaledAmount,
        val hopCount: Int,
        val proofStepCount: Int,
        bundleDigest: ByteArray,
        val artifactBinding: ArtifactBinding,
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

    class SpendableBranch internal constructor(
        bundle: Bundle,
        membershipWitness: NoteMembershipWitness,
        val opening: NoteOpening,
        commitment: ByteArray,
        spendNullifier: ByteArray,
        amount: KagemushaScaledAmount,
        hopCount: Int,
        proofStepCount: Int,
        bundleDigest: ByteArray,
        artifactBinding: ArtifactBinding,
        branchClaims: List<BranchClaim>,
    ) : BranchProjection(
        bundle, membershipWitness, commitment, spendNullifier, amount, hopCount,
        proofStepCount, bundleDigest, artifactBinding, branchClaims,
    )

    /** Secret-bearing local state used only by the genuine ABI-20 builders. */
    class SpendableBranchV4 internal constructor(
        val bundle: BundleV4,
        val membershipWitness: NoteMembershipWitness,
        val opening: NoteOpening,
    )

    class PeerPaymentProjection internal constructor(
        val branch: BranchProjection,
        operationId: ByteArray,
        requestDigest: ByteArray,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")

        fun operationId(): ByteArray = operationIdValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
    }

    class SplitProjection internal constructor(
        val peerPayment: PeerPayment,
        val recipient: BranchProjection,
        val change: BranchProjection?,
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
        val artifactBinding: ArtifactBinding,
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
        unsignedRequest: ByteArray,
        authorizationDigest: ByteArray,
        val change: BranchProjection?,
        operationId: ByteArray,
    ) {
        private val unsignedRequestValue = requiredBytes(unsignedRequest, "unsignedRequest")
        private val authorizationDigestValue = requireDigest(authorizationDigest, "authorizationDigest")
        private val operationIdValue = requireDigest(operationId, "operationId")

        fun unsignedRequest(): ByteArray = unsignedRequestValue.copyOf()
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

    class Readiness internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "OfflineReadiness",
        "readiness",
        MAX_TORII_RESPONSE_BYTES,
    )

    class ActiveVerifier internal constructor(
        val backend: String,
        val name: String,
        val version: Int,
        val circuitId: String,
        commitment: ByteArray,
        publicInputsSchemaHash: ByteArray,
        val maximumProofBytes: Int,
        val activationHeight: Long,
        val withdrawalHeight: Long?,
    ) {
        private val commitmentValue = requireDigest(commitment, "verifierCommitment")
        private val publicInputsSchemaHashValue =
            requireDigest(publicInputsSchemaHash, "publicInputsSchemaHash")

        fun commitment(): ByteArray = commitmentValue.copyOf()

        fun publicInputsSchemaHash(): ByteArray = publicInputsSchemaHashValue.copyOf()

        fun isActiveAt(blockHeight: Long): Boolean =
            blockHeight >= activationHeight &&
                (withdrawalHeight == null || blockHeight < withdrawalHeight)
    }

    data class ReadinessBlocker(val code: String, val message: String)

    class ReadinessProjection internal constructor(
        val requiredBridgeAbiVersion: Int,
        val maximumHops: Int,
        val assetDefinitionId: String,
        val assetScale: Int?,
        val evaluatedBlockHeight: Long,
        evaluatedBlockHash: ByteArray,
        val proofBackendAvailable: Boolean,
        val recursiveLineageSupported: Boolean,
        val ready: Boolean,
        val transferVerifier: ActiveVerifier?,
        val topUpShieldVerifier: ActiveVerifier?,
        val unshieldVerifier: ActiveVerifier?,
        val recursiveStepEqVerifier: ActiveVerifier?,
        val recursiveStepEpVerifier: ActiveVerifier?,
        val blockers: List<ReadinessBlocker>,
    ) {
        private val evaluatedBlockHashValue = requireDigest(evaluatedBlockHash, "evaluatedBlockHash")

        val bridgeCompatible: Boolean
            get() = requiredBridgeAbiVersion == REQUIRED_NATIVE_BRIDGE_ABI_VERSION

        /** Every role-specific verifier is present and active at the same committed snapshot. */
        val allVerifiersActive: Boolean
            get() = listOf(
                transferVerifier,
                topUpShieldVerifier,
                unshieldVerifier,
                recursiveStepEqVerifier,
                recursiveStepEpVerifier,
            ).all { verifier -> verifier?.isActiveAt(evaluatedBlockHeight) == true }

        /** Chain-side recursive artifact/verifier set readiness at the evaluated snapshot. */
        val chainArtifactSetReady: Boolean
            get() = proofBackendAvailable && recursiveLineageSupported &&
                allVerifiersActive

        /** Complete fail-closed SDK decision; local artifact installation is checked separately. */
        val offlineReady: Boolean
            get() = ready && bridgeCompatible && chainArtifactSetReady && assetScale != null &&
                assetScale in 0..KagemushaScaledAmount.MAXIMUM_SCALE &&
                evaluatedBlockHeight > 0 && maximumHops == MAXIMUM_PEER_HOPS &&
                isProofBackendAvailable() && blockers.isEmpty()

        fun evaluatedBlockHash(): ByteArray = evaluatedBlockHashValue.copyOf()
    }

    class OperationReference internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "OfflineOperationReference",
        "operationReference",
        MAX_TORII_RESPONSE_BYTES,
    )

    class OperationStatus internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "OfflineOperationStatus",
        "operationStatus",
        MAX_TORII_RESPONSE_BYTES,
    )

    enum class OperationState { PENDING, APPLIED, REJECTED }

    enum class OperationKind { TOP_UP, REDEEM }

    data class OperationRejection(val code: String, val message: String)

    class FinalizedTopUp internal constructor(
        val anchor: TopUpAnchor,
        val finalityProof: TopUpFinalityProof,
        val finalizedBlockHeight: Long,
        val serverTimeMilliseconds: Long,
    )

    class OperationStatusProjection internal constructor(
        val state: OperationState,
        val kind: OperationKind,
        operationId: ByteArray,
        transactionHash: ByteArray,
        val submittedAtMilliseconds: Long?,
        val finalizedBlockHeight: Long?,
        val serverTimeMilliseconds: Long?,
        val finalizedTopUp: FinalizedTopUp?,
        val rejection: OperationRejection?,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val transactionHashValue = requireDigest(transactionHash, "transactionHash")

        fun operationId(): ByteArray = operationIdValue.copyOf()

        fun transactionHash(): ByteArray = transactionHashValue.copyOf()
    }

    /** Strict typed client for the four first-release Kagemusha Torii routes. */
    class ToriiClient internal constructor(baseUri: URI, private val transport: TransportExecutor) {
        companion object {
            const val READINESS_PATH: String = "/v1/offline/readiness"
            const val TOP_UP_PATH: String = "/v1/offline/top-up"
            const val REDEEM_PATH: String = "/v1/offline/redeem"
            const val OPERATIONS_PATH: String = "/v1/offline/operations"
            const val NORITO_MEDIA_TYPE: String = "application/x-norito"

            private fun requireOperationId(value: String?): String {
                require(
                    value != null &&
                        value.length == 64 &&
                        value != "0".repeat(64) &&
                        value.all { it in '0'..'9' || it in 'a'..'f' },
                ) { "operationId must be non-zero lowercase 32-byte hex" }
                return value
            }

            private fun stripTrailingSlash(value: String): String = value.trimEnd('/')
        }

        private val baseUri: String

        init {
            require(
                baseUri.isAbsolute &&
                    baseUri.rawQuery == null &&
                    baseUri.rawFragment == null &&
                    baseUri.rawUserInfo == null,
            ) { "baseUri must be an absolute credential-free HTTP URI" }
            require(baseUri.scheme.equals("http", true) || baseUri.scheme.equals("https", true)) {
                "baseUri must use HTTP or HTTPS"
            }
            this.baseUri = stripTrailingSlash(baseUri.toString())
        }

        fun getReadiness(assetDefinitionId: String): CompletableFuture<Readiness> {
            require(assetDefinitionId.isNotEmpty() && assetDefinitionId == assetDefinitionId.trim()) {
                "assetDefinitionId must be canonical non-empty text"
            }
            val encoded = URLEncoder.encode(assetDefinitionId, StandardCharsets.UTF_8.name())
            return execute(
                TransportRequest.builder()
                    .setMethod("GET")
                    .setUri(URI.create("$baseUri$READINESS_PATH?asset_definition_id=$encoded"))
                    .addHeader("Accept", NORITO_MEDIA_TYPE)
                    .setMaximumResponseBytes(MAX_TORII_RESPONSE_BYTES.toLong())
                    .build(),
                200,
            ).thenApply { Readiness(it.body) }
        }

        fun submitTopUp(
            request: TopUpRequest,
            operationId: String,
        ): CompletableFuture<OperationReference> = submitCommand(
            TOP_UP_PATH,
            request.noritoEncoded(),
            operationId,
        )

        fun submitRedeem(
            request: RedeemSubmissionRequest,
            operationId: String,
        ): CompletableFuture<OperationReference> = submitCommand(
            REDEEM_PATH,
            request.noritoEncoded(),
            operationId,
        )

        fun getOperation(operationId: String): CompletableFuture<OperationStatus> {
            val id = requireOperationId(operationId)
            return execute(
                TransportRequest.builder()
                    .setMethod("GET")
                    .setUri(URI.create("$baseUri$OPERATIONS_PATH/$id"))
                    .addHeader("Accept", NORITO_MEDIA_TYPE)
                    .setMaximumResponseBytes(MAX_TORII_RESPONSE_BYTES.toLong())
                    .build(),
                200,
            ).thenApply { OperationStatus(it.body) }
        }

        private fun submitCommand(
            path: String,
            request: ByteArray,
            operationId: String,
        ): CompletableFuture<OperationReference> {
            val id = requireOperationId(operationId)
            return execute(
                TransportRequest.builder()
                    .setMethod("POST")
                    .setUri(URI.create("$baseUri$path"))
                    .addHeader("Accept", NORITO_MEDIA_TYPE)
                    .addHeader("Content-Type", NORITO_MEDIA_TYPE)
                    .addHeader("Idempotency-Key", id)
                    .setBody(request)
                    .setMaximumResponseBytes(MAX_TORII_RESPONSE_BYTES.toLong())
                    .build(),
                202,
            ).thenApply { OperationReference(it.body) }
        }

        private fun execute(
            request: TransportRequest,
            expectedStatus: Int,
        ): CompletableFuture<TransportResponse> = transport.execute(request).thenApply { response ->
            check(response.statusCode == expectedStatus) {
                "Kagemusha Torii request failed with HTTP ${response.statusCode}"
            }
            val contentTypes = response.headers["Content-Type"].orEmpty()
            check(contentTypes.size == 1 && contentTypes.single().equals(NORITO_MEDIA_TYPE, true)) {
                "Kagemusha Torii response must use $NORITO_MEDIA_TYPE"
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

        @Synchronized
        fun finish() {
            requireOpen(allowFinalized = false)
            nativeArtifactFinalizeV4(handle)
            finalized = true
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
     * downloaded release. Native verifies signer-role thresholds and hashes both evidence files
     * before consuming any finalized artifact handle.
     */
    class ReleaseAuthentication(
        trustedPolicyNorito: ByteArray,
        releaseAttestationNorito: ByteArray,
        benchmarkEvidence: ByteArray,
        cryptographicReview: ByteArray,
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
        internal val benchmarkEvidence = requireBoundedBytes(
            benchmarkEvidence,
            "benchmarkEvidence",
            MAX_RELEASE_EVIDENCE_BYTES,
        )
        internal val cryptographicReview = requireBoundedBytes(
            cryptographicReview,
            "cryptographicReview",
            MAX_RELEASE_EVIDENCE_BYTES,
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
        private val benchmarkEvidence = releaseAuthentication.benchmarkEvidence.copyOf()
        private val cryptographicReview = releaseAuthentication.cryptographicReview.copyOf()
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

        @Synchronized
        fun install() {
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
                    benchmarkEvidence,
                    cryptographicReview,
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

        @Synchronized
        fun isInstalled(): Boolean =
            !closed && nativeArtifactSetIsInstalledV4(manifestNorito, manifestSha256)

        @Synchronized
        fun artifactBinding(): ArtifactBinding {
            check(installed && !closed && isInstalled()) { "artifact set is not installed" }
            error("ABI20 V4 artifact binding projection is unavailable until the V4 registry is linked")
        }

        @Synchronized
        fun uninstall() {
            if (!installed || closed) return
            nativeArtifactSetUninstallV4(manifestSha256)
            installed = false
            closed = true
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
