// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger

/** Native original selection; the caller supplies no credit material or authority. */
enum class KagemushaIncomingStageKindV1(@JvmField val code: Int) {
    RESERVE_MINT(0), STAGE_MINT(1), STAGE_PEER(2),
}

/** Public physical work and exact original proof retained by the actual native incoming owner. */
class KagemushaNativeIncomingFoldPreparationV1 constructor(
    @JvmField val kind: KagemushaPendingCreditKindV1,
    historyOperationId: ByteArray, creditId: ByteArray, canonicalHardwareStatement: ByteArray,
    proofStatementDigest: ByteArray, normalizedGuardDigest: ByteArray,
    rootSelectionSigningBytes: ByteArray, deviceKeyReference: ByteArray,
    @JvmField val hardwareEpochGeneration: BigInteger, hardwareEpochId: ByteArray,
    canonicalPairedProof: ByteArray,
) {
    private val history = incomingDigest(historyOperationId)
    private val credit = incomingDigest(creditId)
    private val statement = incomingBytes(canonicalHardwareStatement, 8192)
    private val proofDigest = incomingDigest(proofStatementDigest)
    private val guardDigest = incomingDigest(normalizedGuardDigest)
    private val signing = incomingBytes(rootSelectionSigningBytes, 32768)
    private val key = incomingDigest(deviceKeyReference)
    private val epoch = incomingDigest(hardwareEpochId)
    private val proof = incomingBytes(canonicalPairedProof, 8192)

    init {
        require(hardwareEpochGeneration.signum() > 0 && hardwareEpochGeneration.bitLength() <= 128)
        KagemushaNoritoV1.decodePairedProofShapeExact(proof)
    }

    fun historyOperationId(): ByteArray = history.copyOf()
    fun creditId(): ByteArray = credit.copyOf()
    fun canonicalHardwareStatement(): ByteArray = statement.copyOf()
    fun proofStatementDigest(): ByteArray = proofDigest.copyOf()
    fun normalizedGuardDigest(): ByteArray = guardDigest.copyOf()
    fun rootSelectionSigningBytes(): ByteArray = signing.copyOf()
    fun deviceKeyReference(): ByteArray = key.copyOf()
    fun hardwareEpochId(): ByteArray = epoch.copyOf()
    fun canonicalPairedProof(): ByteArray = proof.copyOf()

    /** Correlate only public selectors; native proof and hardware verification remain mandatory. */
    fun requireEvidence(evidence: KagemushaIncomingFoldEvidenceV1) {
        require(history.contentEquals(evidence.historyOperationId()) && proof.contentEquals(evidence.canonicalPairedProof())) {
            "incoming physical evidence substituted its retained history operation or proof"
        }
    }
}

/** Untrusted exact original physical Guard and DEVICE root-selection signature for native verification. */
class KagemushaIncomingFoldEvidenceV1(
    historyOperationId: ByteArray, canonicalPairedProof: ByteArray,
    canonicalHardwareTransitionCertificate: ByteArray, deviceRootSelectionSignature: ByteArray,
) {
    private val history = incomingDigest(historyOperationId)
    private val proof = incomingBytes(canonicalPairedProof, 8192)
    private val certificate = incomingBytes(canonicalHardwareTransitionCertificate, 96 * 1024)
    private val signature = deviceRootSelectionSignature.copyOf()

    init {
        KagemushaNoritoV1.decodePairedProofShapeExact(proof)
        KagemushaP256Codec.requireRawLowSSignature(signature)
    }

    fun historyOperationId(): ByteArray = history.copyOf()
    fun canonicalPairedProof(): ByteArray = proof.copyOf()
    fun canonicalHardwareTransitionCertificate(): ByteArray = certificate.copyOf()
    fun deviceRootSelectionSignature(): ByteArray = signature.copyOf()
}

/**
 * Qualified physical owner of the original incoming Guard and device root-selection signature.
 *
 * Implementations must durably retain or recover the exact originals under the native history ID
 * before returning. Repeated calls with the same work reuse them across process restart; changed
 * work must fail. An aggregate device reply, Core signature or software usage counter supplies
 * neither original. Native completion independently verifies every returned byte before funds.
 */
fun interface KagemushaIncomingFoldEvidenceProviderV1 {
    fun obtainOrRecoverOriginal(preparation: KagemushaNativeIncomingFoldPreparationV1): KagemushaIncomingFoldEvidenceV1
}

/** The required qualified physical incoming evidence owner is absent; no fold was dispatched. */
class KagemushaIncomingFoldEvidenceUnavailableV1 : IllegalStateException(
    "KAGEMUSHA qualified physical incoming Guard and device root-selection evidence are unavailable",
)

private fun incomingDigest(value: ByteArray): ByteArray = value.copyOf().also {
    require(it.size == 32 && it.any { b -> b != 0.toByte() }) { "invalid incoming identity" }
}

private fun incomingBytes(value: ByteArray, maximum: Int): ByteArray = value.copyOf().also {
    require(it.size in 1..maximum) { "invalid incoming original size" }
}
