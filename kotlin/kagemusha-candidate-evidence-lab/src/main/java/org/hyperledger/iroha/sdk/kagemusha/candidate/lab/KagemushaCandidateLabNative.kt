package org.hyperledger.iroha.sdk.kagemusha.candidate.lab

/**
 * Private JNI surface for an unsigned, clean candidate on a physical-device lab.
 *
 * This class is intentionally absent from every published SDK module. Its
 * native symbols exist only in a marker-bearing, feature-gated lab library.
 */
object KagemushaCandidateLabNative {
    const val REQUIRED_BRIDGE_ABI: Int = 21
    const val LIBRARY_NAME: String = "connect_norito_bridge_candidate_lab"

    init {
        System.loadLibrary(LIBRARY_NAME)
        check(nativeBridgeAbiVersion() == REQUIRED_BRIDGE_ABI) {
            "candidate lab requires exact native bridge ABI 21"
        }
    }

    @JvmStatic external fun nativeBridgeAbiVersion(): Int
    @JvmStatic external fun nativeProductionCapabilityObservedV4(): Boolean

    @JvmStatic external fun nativeArtifactBeginV4(
        candidateRecordNorito: ByteArray,
        candidateRecordSha256: ByteArray,
        artifactSha256: ByteArray,
    ): Long

    @JvmStatic external fun nativeArtifactWriteV4(handle: Long, chunk: ByteArray)
    @JvmStatic external fun nativeArtifactFinalizeV4(handle: Long)
    @JvmStatic external fun nativeArtifactCancelV4(handle: Long)
    @JvmStatic external fun nativeArtifactSetInstallV4(
        candidateRecordNorito: ByteArray,
        candidateRecordSha256: ByteArray,
        handles: LongArray,
    )
    @JvmStatic external fun nativeArtifactSetIsInstalledV4(
        candidateRecordNorito: ByteArray,
        candidateRecordSha256: ByteArray,
    ): Boolean
    @JvmStatic external fun nativeAcceptedIdentityV4(): Array<ByteArray>
    @JvmStatic external fun nativeArtifactSetUninstallV4(candidateRecordSha256: ByteArray)

    @JvmStatic external fun nativeBuildInitRequestV4(
        topUpAnchor: ByteArray,
        topUpFinalityProof: ByteArray,
        topUpFinalityRosterArtifact: ByteArray,
        opening: ByteArray,
        outputMembership: ByteArray,
    ): ByteArray

    @JvmStatic external fun nativeBuildAppendRequestV4(
        bundles: Array<ByteArray>,
        topUpProvenances: Array<ByteArray>,
        openings: Array<ByteArray>,
        membershipWitnesses: Array<ByteArray>,
        changeOpening: ByteArray,
        outputMembership: ByteArray,
        verifierCommitment: ByteArray,
        operationId: ByteArray,
        blockHeight: Long,
    ): ByteArray

    @JvmStatic external fun nativeBuildDuplicateInputAppendRequestV4(
        bundles: Array<ByteArray>,
        topUpProvenances: Array<ByteArray>,
        openings: Array<ByteArray>,
        membershipWitnesses: Array<ByteArray>,
        changeOpening: ByteArray,
        outputMembership: ByteArray,
        verifierCommitment: ByteArray,
        operationId: ByteArray,
        blockHeight: Long,
    ): ByteArray

    @JvmStatic external fun nativeBuildVerifyRequestV4(
        bundle: ByteArray,
        recipientRequest: ByteArray,
        topUpProvenance: ByteArray,
        maximumHops: Int,
        blockHeight: Long,
        verifiedAtMilliseconds: Long,
    ): ByteArray

    @JvmStatic external fun nativeBuildRedeemRequestV4(
        bundle: ByteArray,
        topUpProvenance: ByteArray,
        opening: ByteArray,
        membershipWitness: ByteArray,
        recipient: ByteArray,
        atomicUnits: ByteArray,
        scale: Int,
        changeOpening: ByteArray,
        changeOutputMembership: ByteArray,
        verifierCommitment: ByteArray,
        operationId: ByteArray,
        blockHeight: Long,
    ): ByteArray

    @JvmStatic external fun nativeValidateBranchV4(
        bundle: ByteArray,
        topUpProvenance: ByteArray,
        membershipWitness: ByteArray,
        opening: ByteArray,
        blockHeight: Long,
    ): ByteArray

    @JvmStatic external fun nativeInitV4(requestNorito: ByteArray): ByteArray
    @JvmStatic external fun nativeAppendV4(
        requestNorito: ByteArray,
        recipientRequestNorito: ByteArray,
        verifiedAtMilliseconds: Long,
    ): ByteArray
    @JvmStatic external fun nativeVerifyV4(requestNorito: ByteArray): ByteArray
    @JvmStatic external fun nativeRedeemV4(requestNorito: ByteArray): ByteArray

    @JvmStatic external fun nativeProjectInitResultV4(resultNorito: ByteArray): Array<ByteArray>
    @JvmStatic external fun nativeProjectSplitResultV4(resultNorito: ByteArray): Array<ByteArray>
    @JvmStatic external fun nativeProjectVerifyResultV4(resultNorito: ByteArray): Array<ByteArray>
    @JvmStatic external fun nativeProjectRedeemResultV4(resultNorito: ByteArray): Array<ByteArray>
}
