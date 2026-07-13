package org.hyperledger.iroha.sdk.offline

import java.net.URI
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.ZkMerklePathResponse
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash

/**
 * ABI-19 Kagemusha V3 artifact streaming and capability bridge.
 *
 * This is the sole first-release offline-cash surface. It installs the opaque six-file proof
 * artifact set and validates exact typed request/payment/acknowledgement and proof-bound membership
 * archives. Proof execution remains fail-closed while the native backend reports unavailable.
 */
class KagemushaRecursiveSpendProver private constructor() {
    companion object {
        const val REQUIRED_NATIVE_BRIDGE_ABI_VERSION: Int = 19
        const val ARTIFACT_MANIFEST_SCHEMA: String =
            "kagemusha.offline.recursive_spend.artifact_manifest.v3"
        val ARTIFACT_FILES: List<String> = listOf(
            "step-eq.parameters.krv3",
            "step-eq.proving-key.krv3",
            "step-eq.verifying-key.krv3",
            "step-ep.parameters.krv3",
            "step-ep.proving-key.krv3",
            "step-ep.verifying-key.krv3",
        )
        const val ARTIFACT_COUNT: Int = 6
        const val MAX_MANIFEST_BYTES: Int = 1024 * 1024
        const val MAX_PEER_TEXT_ENVELOPE_BYTES: Int = 12 * 1024
        const val MAX_PEER_TEXT_ARCHIVE_BYTES: Int = 9_211
        const val MAX_PEER_ARCHIVE_BYTES: Int = 32 * 1024
        const val MAX_LOCAL_REQUEST_ARCHIVE_BYTES: Int = 8 * 1024 * 1024
        const val MAX_LOCAL_RESULT_ARCHIVE_BYTES: Int = 64 * 1024
        const val MAX_TORII_REQUEST_BYTES: Int = 512 * 1024
        const val MAX_TORII_RESPONSE_BYTES: Int = 4 * 1024 * 1024
        const val MAXIMUM_INPUTS_PER_TRANSITION: Int = 2
        // TODO: Extend the JVM convenience builder to construct two-input joins.
        const val MAXIMUM_LOCAL_APPEND_BUILDER_INPUTS: Int = 1
        const val MAXIMUM_PEER_HOPS: Int = 8
        const val CONFIDENTIAL_TREE_DEPTH: Int = 16

        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val artifactBridgeAvailable = loadArtifactBridge()
        private val proofBackendAvailable = loadProofBackendCapability()

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
            val handle = nativeArtifactBeginV3(manifest, manifestDigest, artifactDigest)
            check(handle > 0) { "native Kagemusha artifact ingest returned no handle" }
            return ArtifactIngest(handle)
        }

        @JvmStatic
        fun beginArtifactInstallSession(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
        ): ArtifactInstallSession {
            requireArtifactBridge()
            return ArtifactInstallSession(
                requireManifest(manifestNorito),
                requireDigest(manifestSha256, "manifestSha256"),
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
        fun decodeInitRequest(archive: ByteArray): InitRequest = InitRequest(archive)

        @JvmStatic
        fun decodeAppendRequest(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): AppendRequest = AppendRequest(archive, changeOpening)

        @JvmStatic
        fun decodeVerifyRequest(archive: ByteArray): VerifyRequest = VerifyRequest(archive)

        @JvmStatic
        fun decodeRedeemRequest(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): RedeemRequest = RedeemRequest(archive, changeOpening)

        @JvmStatic
        fun decodeInitResult(archive: ByteArray): InitResult = InitResult(archive)

        @JvmStatic
        fun decodeSplitResult(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): SplitResult = SplitResult(archive, changeOpening)

        @JvmStatic
        fun decodeVerifyResult(archive: ByteArray): VerifyResult = VerifyResult(archive)

        @JvmStatic
        fun decodeRedeemBuildResult(
            archive: ByteArray,
            changeOpening: NoteOpening?,
        ): RedeemBuildResult = RedeemBuildResult(archive, changeOpening)

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
        fun finalizeRedeem(
            buildResult: RedeemBuildResult,
            authorization: RequestAuthorization,
        ): RedeemFinalization {
            requireArtifactBridge()
            val fields = nativeFinalizeRedeemV2(
                buildResult.noritoEncoded(),
                authorization.noritoEncoded(),
            )
            requireFieldCount(fields, 2, "redeem finalization")
            return RedeemFinalization(
                RedeemSubmissionRequest(fields[0]),
                requireDigest(fields[1], "operationId"),
            )
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
        fun buildInitRequest(
            topUpAnchor: TopUpAnchor,
            topUpFinalityProof: TopUpFinalityProof,
            topUpFinalityRosterArtifact: TopUpFinalityRosterArtifact,
        ): InitRequest {
            requireArtifactBridge()
            return InitRequest(
                nativeBuildInitRequestV2(
                    topUpAnchor.noritoEncoded(),
                    topUpFinalityProof.noritoEncoded(),
                    topUpFinalityRosterArtifact.noritoEncoded(),
                ),
            )
        }

        @JvmStatic
        fun projectInitResult(
            request: InitRequest,
            result: InitResult,
            opening: NoteOpening,
        ): SpendableBranch {
            requireArtifactBridge()
            val fields = nativeProjectInitResultV2(request.noritoEncoded(), result.noritoEncoded())
            requireFieldCount(fields, 10, "init result projection")
            requireDigest(fields[7], "publicStatementDigest")
            val expectedAmount = amount(fields[4], fields[5])
            val expectedHopCount = integer(fields[6], "hopCount")
            val restored = restoreSpendableBranch(
                Bundle(fields[0]), NoteMembershipWitness(fields[1]), opening,
            )
            requireProjectedBranch(
                restored, fields[2], fields[3], expectedAmount, expectedHopCount, null,
                BranchClaim(fields[8]), fields[9],
                "init result",
            )
            return restored
        }

        /** Revalidate one encrypted persisted branch before making it spendable after restart. */
        @JvmStatic
        fun restoreSpendableBranch(
            bundleArchive: ByteArray,
            membershipWitnessArchive: ByteArray,
            openingArchive: ByteArray,
        ): SpendableBranch {
            val bundle = Bundle(bundleArchive)
            val witness = NoteMembershipWitness(membershipWitnessArchive)
            val opening = NoteOpening(openingArchive)
            return restoreSpendableBranch(bundle, witness, opening)
        }

        private fun restoreSpendableBranch(
            bundle: Bundle,
            witness: NoteMembershipWitness,
            opening: NoteOpening,
        ): SpendableBranch {
            requireProofBackend()
            val fields = nativeRestoreSpendableBranchV2(
                bundle.noritoEncoded(), witness.noritoEncoded(), opening.noritoEncoded(),
            )
            requireFieldCount(fields, 9, "branch restore")
            requireDigest(fields[5], "bundleDigest")
            return SpendableBranch(
                bundle,
                witness,
                opening,
                fields[0],
                fields[1],
                amount(fields[2], fields[3]),
                integer(fields[4], "hopCount"),
                fields[6].takeIf { it.isNotEmpty() },
                BranchClaim(fields[7]),
                fields[8],
            )
        }

        private fun requireProjectedBranch(
            branch: SpendableBranch,
            commitment: ByteArray,
            spendNullifier: ByteArray,
            amount: KagemushaScaledAmount,
            hopCount: Int,
            parentBranchClaimDigest: ByteArray?,
            branchClaim: BranchClaim,
            branchClaimDigest: ByteArray,
            field: String,
        ) {
            check(branch.commitment().contentEquals(requireDigest(commitment, "$field commitment")) &&
                branch.spendNullifier().contentEquals(
                    requireDigest(spendNullifier, "$field spend nullifier"),
                ) &&
                branch.amount == amount && branch.hopCount == hopCount &&
                nullableDigestEquals(branch.parentBranchClaimDigest(), parentBranchClaimDigest) &&
                branch.branchClaim.noritoEncoded().contentEquals(branchClaim.noritoEncoded()) &&
                branch.branchClaimDigest().contentEquals(
                    requireDigest(branchClaimDigest, "$field branch claim digest"),
                )
            ) { "$field does not match its proof-verified spendable branch" }
        }

        private fun nullableDigestEquals(actual: ByteArray?, expected: ByteArray?): Boolean {
            if (actual == null || expected == null) return actual == null && expected == null
            return actual.contentEquals(requireDigest(expected, "parentBranchClaimDigest"))
        }

        @JvmStatic
        fun buildAppendRequest(
            input: SpendableBranch,
            changeOpening: NoteOpening?,
            transferVerifierCommitment: ByteArray,
            operationId: ByteArray,
            blockHeight: Long,
        ): AppendRequest {
            requireArtifactBridge()
            return AppendRequest(
                nativeBuildAppendRequestV2(
                    input.bundle.noritoEncoded(),
                    input.opening.noritoEncoded(),
                    input.membershipWitness.noritoEncoded(),
                    changeOpening?.noritoEncoded() ?: byteArrayOf(),
                    requireDigest(transferVerifierCommitment, "transferVerifierCommitment"),
                    requireDigest(operationId, "operationId"),
                    blockHeight,
                ),
                changeOpening,
            )
        }

        @JvmStatic
        fun projectSplitResult(result: SplitResult): SplitProjection {
            requireArtifactBridge()
            val fields = nativeProjectSplitResultV2(result.noritoEncoded())
            requireFieldCount(fields, 23, "split result projection")
            val recipient = BranchProjection(
                Bundle(fields[1]), NoteMembershipWitness(fields[2]), fields[3], fields[4],
                amount(fields[5], fields[6]), integer(fields[7], "recipientHopCount"), fields[18],
                BranchClaim(fields[19]), fields[20],
            )
            val change = if (fields[10].isEmpty()) {
                null
            } else {
                val opening = checkNotNull(result.changeOpening) {
                    "split result contains change without its local opening"
                }
                val expectedAmount = amount(fields[14], fields[15])
                val expectedHopCount = integer(fields[16], "changeHopCount")
                val restored = restoreSpendableBranch(
                    Bundle(fields[10]), NoteMembershipWitness(fields[11]), opening,
                )
                requireProjectedBranch(
                    restored, fields[12], fields[13], expectedAmount, expectedHopCount, fields[18],
                    BranchClaim(fields[21]), fields[22],
                    "split change",
                )
                restored
            }
            return SplitProjection(
                PeerPayment(fields[0]), recipient, change,
                fields[8], fields[9], fields[17], fields[18],
            )
        }

        @JvmStatic
        fun projectPeerPayment(payment: PeerPayment): BranchProjection {
            requireArtifactBridge()
            val fields = nativeProjectPeerPaymentV2(payment.noritoEncoded())
            requireFieldCount(fields, 13, "peer payment projection")
            return BranchProjection(
                Bundle(fields[0]), NoteMembershipWitness(fields[1]), fields[2], fields[3],
                amount(fields[4], fields[5]), integer(fields[6], "hopCount"), fields[10],
                BranchClaim(fields[11]), fields[12],
            )
        }

        @JvmStatic
        fun buildVerifyRequest(
            payment: PeerPayment,
            recipientRequest: RecipientPaymentRequest,
            maximumHops: Int,
            blockHeight: Long,
            verifiedAtMilliseconds: Long,
        ): VerifyRequest {
            requireArtifactBridge()
            return VerifyRequest(
                nativeBuildVerifyRequestV2(
                    payment.noritoEncoded(), recipientRequest.noritoEncoded(), maximumHops,
                    blockHeight, verifiedAtMilliseconds,
                ),
            )
        }

        @JvmStatic
        fun projectVerifyResult(result: VerifyResult): VerifyProjection {
            requireArtifactBridge()
            val fields = nativeProjectVerifyResultV2(result.noritoEncoded())
            requireFieldCount(fields, 14, "verify result projection")
            return VerifyProjection(
                bool(fields[0], "valid"),
                bool(fields[1], "chainAdmissible"),
                bool(fields[2], "lineageRedeemable"),
                bool(fields[3], "witnesslessRedemptionSupported"),
                fields[4], fields[5], amount(fields[6], fields[7]), integer(fields[8], "hopCount"),
                fields[9], fields[10], fields[11], BranchClaim(fields[12]), fields[13],
            )
        }

        @JvmStatic
        fun buildRedeemRequest(
            input: SpendableBranch,
            recipientAccountId: String,
            amount: KagemushaScaledAmount,
            changeOpening: NoteOpening?,
            unshieldVerifierCommitment: ByteArray,
            operationId: ByteArray,
            blockHeight: Long,
        ): RedeemRequest {
            requireArtifactBridge()
            return RedeemRequest(
                nativeBuildRedeemRequestV2(
                    input.bundle.noritoEncoded(), input.opening.noritoEncoded(),
                    input.membershipWitness.noritoEncoded(),
                    utf8(recipientAccountId, "recipientAccountId"),
                    utf8(amount.atomicUnits, "atomicUnits"), amount.scale,
                    changeOpening?.noritoEncoded() ?: byteArrayOf(),
                    requireDigest(unshieldVerifierCommitment, "unshieldVerifierCommitment"),
                    requireDigest(operationId, "operationId"), blockHeight,
                ),
                changeOpening,
            )
        }

        @JvmStatic
        fun projectRedeemBuildResult(result: RedeemBuildResult): RedeemBuildProjection {
            requireArtifactBridge()
            val fields = nativeProjectRedeemBuildResultV2(result.noritoEncoded())
            requireFieldCount(fields, 13, "redeem build projection")
            val change = if (fields[2].isEmpty()) {
                null
            } else {
                val opening = checkNotNull(result.changeOpening) {
                    "redeem result contains change without its local opening"
                }
                val expectedAmount = amount(fields[6], fields[7])
                val expectedHopCount = integer(fields[8], "hopCount")
                val restored = restoreSpendableBranch(
                    Bundle(fields[2]), NoteMembershipWitness(fields[3]), opening,
                )
                requireProjectedBranch(
                    restored, fields[4], fields[5], expectedAmount, expectedHopCount, fields[10],
                    BranchClaim(fields[11]), fields[12],
                    "redemption change",
                )
                restored
            }
            return RedeemBuildProjection(fields[0], fields[1], change, fields[9])
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
        fun initSpend(request: InitRequest): InitResult {
            requireProofBackend()
            return InitResult(callNativeLifecycle("init spend") {
                nativeInitSpendV2(request.noritoEncoded())
            })
        }

        /** Prove one exact recipient output and optional independently spendable sender change. */
        @JvmStatic
        fun appendSpend(
            request: AppendRequest,
            recipientRequest: RecipientPaymentRequest,
            verifiedAtMilliseconds: Long,
        ): SplitResult {
            require(verifiedAtMilliseconds > 0) {
                "verifiedAtMilliseconds must be positive"
            }
            requireProofBackend()
            val changeOpening = request.changeOpening
            val secretArchive = request.consumeAndDestroy()
            return try {
                SplitResult(
                    callNativeLifecycle("append spend") {
                        nativeAppendSpendV2(
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
        fun verifySpend(request: VerifyRequest): VerifyResult {
            requireProofBackend()
            return VerifyResult(callNativeLifecycle("verify spend") {
                nativeVerifySpendV2(request.noritoEncoded())
            })
        }

        /** Build a full or partial redemption and its optional proof-bound offline change. */
        @JvmStatic
        fun buildRedeem(request: RedeemRequest): RedeemBuildResult {
            requireProofBackend()
            val changeOpening = request.changeOpening
            val secretArchive = request.consumeAndDestroy()
            return try {
                RedeemBuildResult(
                    callNativeLifecycle("build redeem") {
                        nativeBuildRedeemV2(secretArchive)
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
                        nativeArtifactBeginV3(byteArrayOf(0), ByteArray(32), ByteArray(32))
                    }
                },
            )

        private fun loadProofBackendCapability(): Boolean =
            detectExactNativeAvailability(
                loadLibrary = { System.loadLibrary(LIBRARY_NAME) },
                abiVersion = { nativeBridgeAbiVersion() },
                symbolProbe = { nativePastaCycleV3BackendAvailable() },
            )

        private fun expectIllegalArgumentProbe(probe: () -> Unit): Boolean = try {
            probe()
            false
        } catch (_: IllegalArgumentException) {
            true
        }

        private fun requireArtifactBridge() {
            check(artifactBridgeAvailable) {
                "$LIBRARY_NAME ABI $REQUIRED_NATIVE_BRIDGE_ABI_VERSION artifact streaming is unavailable"
            }
        }

        private fun requireProofBackend() {
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
        private external fun nativePastaCycleV3BackendAvailable(): Boolean

        @JvmStatic
        private external fun nativeArtifactBeginV3(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            expectedArtifactSha256: ByteArray,
        ): Long

        @JvmStatic
        private external fun nativeArtifactWriteV3(handle: Long, chunk: ByteArray)

        @JvmStatic
        private external fun nativeArtifactFinalizeV3(handle: Long)

        @JvmStatic
        private external fun nativeArtifactCancelV3(handle: Long)

        @JvmStatic
        private external fun nativeArtifactSetInstallV3(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
            artifactHandles: LongArray,
        )

        @JvmStatic
        private external fun nativeArtifactSetIsInstalledV3(
            manifestNorito: ByteArray,
            manifestSha256: ByteArray,
        ): Boolean

        @JvmStatic
        private external fun nativeArtifactSetUninstallV3(manifestSha256: ByteArray)

        @JvmStatic
        private external fun nativeInitSpendV2(requestNorito: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeAppendSpendV2(
            requestNorito: ByteArray,
            recipientRequestNorito: ByteArray,
            verifiedAtMilliseconds: Long,
        ): ByteArray?

        @JvmStatic
        private external fun nativeVerifySpendV2(requestNorito: ByteArray): ByteArray?

        @JvmStatic
        private external fun nativeBuildRedeemV2(requestNorito: ByteArray): ByteArray?

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
        @JvmStatic private external fun nativeBuildInitRequestV2(anchor: ByteArray, proof: ByteArray, roster: ByteArray): ByteArray
        @JvmStatic private external fun nativeProjectInitResultV2(request: ByteArray, result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildAppendRequestV2(bundle: ByteArray, opening: ByteArray, witness: ByteArray, changeOpening: ByteArray, verifierCommitment: ByteArray, operationId: ByteArray, blockHeight: Long): ByteArray
        @JvmStatic private external fun nativeProjectPeerPaymentV2(payment: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeProjectSplitResultV2(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildVerifyRequestV2(payment: ByteArray, request: ByteArray, maximumHops: Int, blockHeight: Long, verifiedAtMilliseconds: Long): ByteArray
        @JvmStatic private external fun nativeProjectVerifyResultV2(result: ByteArray): Array<ByteArray>
        @JvmStatic private external fun nativeBuildRedeemRequestV2(bundle: ByteArray, opening: ByteArray, witness: ByteArray, recipient: ByteArray, atomicUnits: ByteArray, scale: Int, changeOpening: ByteArray, verifierCommitment: ByteArray, operationId: ByteArray, blockHeight: Long): ByteArray
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
        @JvmStatic private external fun nativeRestoreSpendableBranchV2(bundle: ByteArray, witness: ByteArray, opening: ByteArray): Array<ByteArray>
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

    class InitRequest internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendInitRequestV2",
        "initRequest",
        MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
    )

    /** Local secret-bearing append input. Native code consumes and wipes its openings. */
    class AppendRequest internal constructor(
        archive: ByteArray,
        internal val changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendAppendLocalRequestV2",
            "appendRequest",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        override fun close() = destroy()
    }

    class VerifyRequest internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendVerifyRequestV2",
        "verifyRequest",
        MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
    )

    /** Local secret-bearing redemption input. Native code consumes and wipes its openings. */
    class RedeemRequest internal constructor(
        archive: ByteArray,
        internal val changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendRedeemLocalRequestV2",
            "redeemRequest",
            MAX_LOCAL_REQUEST_ARCHIVE_BYTES,
        ), AutoCloseable {
        override fun close() = destroy()
    }

    class InitResult internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendInitResultV2",
        "initResult",
        MAX_LOCAL_RESULT_ARCHIVE_BYTES,
    )

    class SplitResult internal constructor(
        archive: ByteArray,
        internal val changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendSplitResultV2",
            "splitResult",
            MAX_LOCAL_RESULT_ARCHIVE_BYTES,
        )

    class VerifyResult internal constructor(archive: ByteArray) : CanonicalArchive(
        archive,
        "KagemushaRecursiveSpendVerifyResultV2",
        "verifyResult",
        MAX_LOCAL_RESULT_ARCHIVE_BYTES,
    )

    class RedeemBuildResult internal constructor(
        archive: ByteArray,
        internal val changeOpening: NoteOpening? = null,
    ) : CanonicalArchive(
            archive,
            "KagemushaRecursiveSpendRedeemBuildResultV2",
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
        parentBranchClaimDigest: ByteArray?,
        val branchClaim: BranchClaim,
        branchClaimDigest: ByteArray,
    ) {
        private val commitmentValue = requireDigest(commitment, "commitment")
        private val spendNullifierValue = requireDigest(spendNullifier, "spendNullifier")
        private val parentBranchClaimDigestValue = parentBranchClaimDigest?.let {
            requireDigest(it, "parentBranchClaimDigest")
        }
        private val branchClaimDigestValue = requireDigest(branchClaimDigest, "branchClaimDigest")

        init {
            check(hopCount in 0..MAXIMUM_PEER_HOPS) { "native Kagemusha hop count is invalid" }
        }

        fun commitment(): ByteArray = commitmentValue.copyOf()
        fun spendNullifier(): ByteArray = spendNullifierValue.copyOf()
        fun parentBranchClaimDigest(): ByteArray? = parentBranchClaimDigestValue?.copyOf()
        fun branchClaimDigest(): ByteArray = branchClaimDigestValue.copyOf()

        fun conflictsWith(other: BranchProjection): Boolean = branchClaim.conflictsWith(other.branchClaim)
    }

    class SpendableBranch internal constructor(
        bundle: Bundle,
        membershipWitness: NoteMembershipWitness,
        val opening: NoteOpening,
        commitment: ByteArray,
        spendNullifier: ByteArray,
        amount: KagemushaScaledAmount,
        hopCount: Int,
        parentBranchClaimDigest: ByteArray?,
        branchClaim: BranchClaim,
        branchClaimDigest: ByteArray,
    ) : BranchProjection(
        bundle, membershipWitness, commitment, spendNullifier, amount, hopCount,
        parentBranchClaimDigest, branchClaim, branchClaimDigest,
    )

    class SplitProjection internal constructor(
        val peerPayment: PeerPayment,
        val recipient: BranchProjection,
        val change: SpendableBranch?,
        operationId: ByteArray,
        requestDigest: ByteArray,
        splitBindingDigest: ByteArray,
        parentBranchClaimDigest: ByteArray,
    ) {
        private val operationIdValue = requireDigest(operationId, "operationId")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")
        private val splitBindingDigestValue = requireDigest(splitBindingDigest, "splitBindingDigest")
        private val parentBranchClaimDigestValue =
            requireDigest(parentBranchClaimDigest, "parentBranchClaimDigest")

        fun operationId(): ByteArray = operationIdValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
        fun splitBindingDigest(): ByteArray = splitBindingDigestValue.copyOf()
        fun parentBranchClaimDigest(): ByteArray = parentBranchClaimDigestValue.copyOf()
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
        bundleDigest: ByteArray,
        requestDigest: ByteArray,
        outputBindingDigest: ByteArray,
        val branchClaim: BranchClaim,
        branchClaimDigest: ByteArray,
    ) {
        private val commitmentValue = requireDigest(commitment, "commitment")
        private val spendNullifierValue = requireDigest(spendNullifier, "spendNullifier")
        private val bundleDigestValue = requireDigest(bundleDigest, "bundleDigest")
        private val requestDigestValue = requireDigest(requestDigest, "requestDigest")
        private val outputBindingDigestValue = requireDigest(outputBindingDigest, "outputBindingDigest")
        private val branchClaimDigestValue = requireDigest(branchClaimDigest, "branchClaimDigest")

        fun commitment(): ByteArray = commitmentValue.copyOf()
        fun spendNullifier(): ByteArray = spendNullifierValue.copyOf()
        fun bundleDigest(): ByteArray = bundleDigestValue.copyOf()
        fun requestDigest(): ByteArray = requestDigestValue.copyOf()
        fun outputBindingDigest(): ByteArray = outputBindingDigestValue.copyOf()
        fun branchClaimDigest(): ByteArray = branchClaimDigestValue.copyOf()
    }

    class RedeemBuildProjection internal constructor(
        unsignedRequest: ByteArray,
        authorizationDigest: ByteArray,
        val change: SpendableBranch?,
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
            nativeArtifactWriteV3(handle, requireChunk(chunk))
        }

        @Synchronized
        fun finish() {
            requireOpen(allowFinalized = false)
            nativeArtifactFinalizeV3(handle)
            finalized = true
        }

        @Synchronized
        fun isFinalized(): Boolean = finalized

        @Synchronized
        override fun close() {
            if (handle == 0L) return
            check(!installClaimed) { "artifact ingest is being installed" }
            nativeArtifactCancelV3(handle)
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

    /** Coordinates one atomic six-artifact generation install. */
    class ArtifactInstallSession internal constructor(
        manifest: ByteArray,
        manifestDigest: ByteArray,
    ) : AutoCloseable {
        private val manifestNorito = manifest.copyOf()
        private val manifestSha256 = manifestDigest.copyOf()
        private val artifacts = linkedMapOf<String, ArtifactIngest>()
        private var installed = false
        private var closed = false

        @Synchronized
        fun beginArtifact(expectedArtifactSha256: ByteArray): ArtifactIngest {
            requirePending()
            check(artifacts.size < ARTIFACT_COUNT) { "artifact set already has six streams" }
            val digest = requireDigest(expectedArtifactSha256, "expectedArtifactSha256")
            val key = hex(digest)
            require(!artifacts.containsKey(key)) { "expectedArtifactSha256 is duplicated" }
            return beginArtifactIngest(manifestNorito, manifestSha256, digest)
                .also { artifacts[key] = it }
        }

        @Synchronized
        fun install() {
            requirePending()
            check(artifacts.size == ARTIFACT_COUNT) {
                "artifact set must contain exactly six streams"
            }
            val ordered = artifacts.values.toList()
            val handles = LongArray(ARTIFACT_COUNT)
            var claimed = 0
            try {
                while (claimed < ordered.size) {
                    handles[claimed] = ordered[claimed].claimFinalizedHandle()
                    claimed += 1
                }
                nativeArtifactSetInstallV3(manifestNorito, manifestSha256, handles)
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
            installed = true
        }

        @Synchronized
        fun isInstalled(): Boolean =
            !closed && nativeArtifactSetIsInstalledV3(manifestNorito, manifestSha256)

        @Synchronized
        fun artifactBinding(): ArtifactBinding {
            check(installed && !closed && isInstalled()) { "artifact set is not installed" }
            return ArtifactBinding(nativeArtifactBindingV3(manifestNorito, manifestSha256))
        }

        @Synchronized
        fun uninstall() {
            if (!installed || closed) return
            nativeArtifactSetUninstallV3(manifestSha256)
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
            closed = true
            firstFailure?.let { throw it }
        }

        private fun requirePending() {
            check(!closed && !installed) { "artifact install session is not pending" }
        }
    }
}
