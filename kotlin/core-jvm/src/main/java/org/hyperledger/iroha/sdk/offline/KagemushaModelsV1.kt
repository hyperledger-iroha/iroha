// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder

/*
 * JVM carrier convention: Int and Long preserve the raw bit patterns of Rust u32 and u64 fields.
 * A negative signed carrier therefore remains a valid upper-half unsigned value.
 */

/** Exact typed `AssetDefinitionId` payload used by Kagemusha V1. */
class KagemushaAssetDefinitionIdV1 private constructor(payload: ByteArray) {
    private val value = payload.copyOf()

    /** Return a defensive copy of the canonical bare Norito payload. */
    fun canonicalPayload(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        other is KagemushaAssetDefinitionIdV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()

    companion object {
        /** Parse a canonical asset-definition address. */
        @JvmStatic
        fun parse(literal: String): KagemushaAssetDefinitionIdV1 =
            KagemushaAssetDefinitionIdV1(
                TransferWirePayloadEncoder.encodeAssetDefinitionIdPayload(literal),
            )

        /** Parse an exact canonical bare Norito `AssetDefinitionId` payload. */
        @JvmStatic
        fun fromCanonicalPayload(payload: ByteArray): KagemushaAssetDefinitionIdV1 {
            requireFixedByteArrayPayload(payload, 16, "asset")
            return KagemushaAssetDefinitionIdV1(payload)
        }
    }
}

/** Exact typed universal `AccountId` payload used by Kagemusha V1. */
class KagemushaAccountIdV1 private constructor(payload: ByteArray) {
    private val value = payload.copyOf()

    /** Return a defensive copy of the canonical bare Norito payload. */
    fun canonicalPayload(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        other is KagemushaAccountIdV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()

    companion object {
        /** Parse a canonical I105 account literal. */
        @JvmStatic
        fun parse(literal: String): KagemushaAccountIdV1 =
            KagemushaAccountIdV1(TransferWirePayloadEncoder.encodeAccountIdPayload(literal))

        /** Parse an exact canonical bare Norito `AccountId` payload. */
        @JvmStatic
        fun fromCanonicalPayload(payload: ByteArray): KagemushaAccountIdV1 {
            require(payload.isNotEmpty() && payload.size <= 512) {
                "Kagemusha V1 account payload is empty or oversized"
            }
            val rendered = TransferWirePayloadEncoder.decodeAccountIdPayload(payload, 0)
            require(
                TransferWirePayloadEncoder.encodeAccountIdPayload(rendered).contentEquals(payload),
            ) { "Kagemusha V1 account payload is not canonical" }
            return KagemushaAccountIdV1(payload)
        }
    }
}

/** Canonical non-zero marked hash naming one asset-registration incarnation. */
class KagemushaAssetIncarnationV1(bytes: ByteArray) {
    private val value = raw32(bytes, "assetIncarnation")

    init {
        require(value[31].toInt() and 1 == 1) {
            "assetIncarnation must carry the canonical Iroha hash marker"
        }
    }

    /** Return the canonical marked hash bytes. */
    fun bytes(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        other is KagemushaAssetIncarnationV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()
}

/** Canonical uncompressed SEC1 P-256 Kagemusha V1 authority key. */
class KagemushaDevicePublicKeyV1(sec1Bytes: ByteArray) {
    private val value = KagemushaP256Codec.requireUncompressedPublicKey(sec1Bytes)

    /** Return the exact `04 || x || y` bytes. */
    fun sec1Bytes(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        other is KagemushaDevicePublicKeyV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()
}

/** Canonical fixed-width low-S P-256 Kagemusha V1 signature. */
class KagemushaDeviceSignatureV1(rawBytes: ByteArray) {
    private val value = KagemushaP256Codec.requireRawLowSSignature(rawBytes)

    /** Return the exact fixed-width `r || s` bytes. */
    fun rawBytes(): ByteArray = value.copyOf()

    /** Return the equivalent strict DER signature. */
    fun strictDer(): ByteArray = KagemushaP256Codec.strictDerFromRawLowS(value)

    override fun equals(other: Any?): Boolean =
        other is KagemushaDeviceSignatureV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()

    companion object {
        /** Parse strict DER and normalize a valid signature to low-S form. */
        @JvmStatic
        fun fromStrictDer(derBytes: ByteArray): KagemushaDeviceSignatureV1 =
            KagemushaDeviceSignatureV1(KagemushaP256Codec.rawLowSFromStrictDer(derBytes))
    }
}

/**
 * Canonical 32-byte nonzero X25519 wire shape used by recipient-only envelopes.
 *
 * This codec type deliberately performs no scalar multiplication or managed group validation.
 * The shared native Kagemusha core authenticates canonical X25519 elements during object and
 * complete-exchange validation before any monetary use.
 */
class KagemushaX25519PublicKeyV1(bytes: ByteArray) {
    private val value = requireX25519PublicKey(bytes)

    /** Return the exact 32-byte Montgomery u-coordinate. */
    fun bytes(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        other is KagemushaX25519PublicKeyV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()
}

/** Constant-size public commitment to one private aggregate balance. */
class KagemushaAggregateStateCommitmentV1(
    @JvmField val version: Int,
    releaseId: ByteArray,
    @JvmField val networkId: NetworkId,
    @JvmField val asset: KagemushaAssetDefinitionIdV1,
    @JvmField val assetIncarnation: KagemushaAssetIncarnationV1,
    @JvmField val scale: Int,
    liabilityPoolId: ByteArray,
    laneId: ByteArray,
    hardwareEpochId: ByteArray,
    keyReference: ByteArray,
    hardwarePolicyId: ByteArray,
    @JvmField val sequence: BigInteger,
    stateCommitment: ByteArray,
) {
    private val releaseIdValue = fixed32(releaseId, "releaseId")
    private val liabilityPoolIdValue = fixed32(liabilityPoolId, "liabilityPoolId")
    private val laneIdValue = fixed32(laneId, "laneId")
    private val hardwareEpochIdValue = fixed32(hardwareEpochId, "hardwareEpochId")
    private val keyReferenceValue = fixed32(keyReference, "keyReference")
    private val hardwarePolicyIdValue = fixed32(hardwarePolicyId, "hardwarePolicyId")
    private val stateCommitmentValue = fixed32(stateCommitment, "stateCommitment")

    init {
        requireHeader(version, networkId, scale, null)
        requireUnsigned128(sequence, "sequence")
    }

    fun releaseId(): ByteArray = releaseIdValue.copyOf()
    fun liabilityPoolId(): ByteArray = liabilityPoolIdValue.copyOf()
    fun laneId(): ByteArray = laneIdValue.copyOf()
    fun hardwareEpochId(): ByteArray = hardwareEpochIdValue.copyOf()
    fun keyReference(): ByteArray = keyReferenceValue.copyOf()
    fun hardwarePolicyId(): ByteArray = hardwarePolicyIdValue.copyOf()
    fun stateCommitment(): ByteArray = stateCommitmentValue.copyOf()
}

/** Pair of public Pasta-side commitments for one recursively proved state. */
class KagemushaPastaStateCommitmentV1(eq: ByteArray, ep: ByteArray) {
    private val eqValue = raw32(eq, "eq")
    private val epValue = raw32(ep, "ep")

    fun eq(): ByteArray = eqValue.copyOf()
    fun ep(): ByteArray = epValue.copyOf()
    fun isZero(): Boolean = eqValue.all { it == 0.toByte() } && epValue.all { it == 0.toByte() }
}

/** Closed paired-Pasta proof with unlinkable statement-scoped history accumulators. */
class KagemushaPairedProofV1(
    @JvmField val version: Int,
    eqProtocolDigest: ByteArray,
    epProtocolDigest: ByteArray,
    semanticDigest: ByteArray,
    guardEqCredentialAudit: ByteArray,
    guardEpCredentialAudit: ByteArray,
    eqDeferredAudit: ByteArray,
    epDeferredAudit: ByteArray,
    eqProof: ByteArray,
    epProof: ByteArray,
    eqHistory: ByteArray,
    epHistory: ByteArray,
) {
    private val eqProtocolDigestValue = fixed32(eqProtocolDigest, "eqProtocolDigest")
    private val epProtocolDigestValue = fixed32(epProtocolDigest, "epProtocolDigest")
    private val semanticDigestValue = fixed32(semanticDigest, "semanticDigest")
    private val guardEqCredentialAuditValue = fixed32(guardEqCredentialAudit, "guardEqCredentialAudit")
    private val guardEpCredentialAuditValue = fixed32(guardEpCredentialAudit, "guardEpCredentialAudit")
    private val eqDeferredAuditValue = fixed32(eqDeferredAudit, "eqDeferredAudit")
    private val epDeferredAuditValue = fixed32(epDeferredAudit, "epDeferredAudit")
    private val eqProofValue = boundedProof(eqProof, "eqProof")
    private val epProofValue = boundedProof(epProof, "epProof")
    private val eqHistoryValue = exactHistory(eqHistory, "eqHistory")
    private val epHistoryValue = exactHistory(epHistory, "epHistory")

    init {
        require(version == KagemushaWireV1.WIRE_VERSION)
        require(!eqProtocolDigestValue.contentEquals(epProtocolDigestValue))
        require(!guardEqCredentialAuditValue.contentEquals(guardEpCredentialAuditValue))
        require(!eqDeferredAuditValue.contentEquals(epDeferredAuditValue))
        require(eqProofValue.size + epProofValue.size <= KagemushaWireV1.MAXIMUM_CURRENT_PROOFS_BYTES)
        require(!eqHistoryValue.contentEquals(epHistoryValue))
    }

    fun eqProtocolDigest(): ByteArray = eqProtocolDigestValue.copyOf()
    fun epProtocolDigest(): ByteArray = epProtocolDigestValue.copyOf()
    fun semanticDigest(): ByteArray = semanticDigestValue.copyOf()
    fun guardEqCredentialAudit(): ByteArray = guardEqCredentialAuditValue.copyOf()
    fun guardEpCredentialAudit(): ByteArray = guardEpCredentialAuditValue.copyOf()
    fun eqDeferredAudit(): ByteArray = eqDeferredAuditValue.copyOf()
    fun epDeferredAudit(): ByteArray = epDeferredAuditValue.copyOf()
    fun eqProof(): ByteArray = eqProofValue.copyOf()
    fun epProof(): ByteArray = epProofValue.copyOf()
    fun eqHistory(): ByteArray = eqHistoryValue.copyOf()
    fun epHistory(): ByteArray = epHistoryValue.copyOf()
}

/** Governed qualified hardware service class. */
enum class KagemushaHardwarePlatformClassV1 {
    ANDROID_OEM_SERVICE,
    APPLE_OEM_SERVICE,
    DEDICATED_SECURE_ELEMENT,
    OTHER_QUALIFIED,
}

/** Governed non-forking hardware-service profile. */
class KagemushaHardwareProfileV1(
    @JvmField val version: Int,
    @JvmField val protocolVersion: Int,
    hardwareProfileId: ByteArray,
    providerId: ByteArray,
    @JvmField val platformClass: KagemushaHardwarePlatformClassV1,
    productClassDigest: ByteArray,
    firmwarePolicyDigest: ByteArray,
    enrollmentAttestationVerifierDigest: ByteArray,
    attestationTrustRootsDigest: ByteArray,
    allowedSuiteCommitment: ByteArray,
    @JvmField val policyEpoch: Long,
    @JvmField val governanceCredentialPublicKey: KagemushaDevicePublicKeyV1,
    @JvmField val capabilityMask: Int,
    qualificationReportDigest: ByteArray,
    @JvmField val validFromMs: Long,
    @JvmField val expiresAtMs: Long,
) {
    private val hardwareProfileIdValue = fixed32(hardwareProfileId, "hardwareProfileId")
    private val providerIdValue = fixed32(providerId, "providerId")
    private val productClassDigestValue = fixed32(productClassDigest, "productClassDigest")
    private val firmwarePolicyDigestValue = fixed32(firmwarePolicyDigest, "firmwarePolicyDigest")
    private val enrollmentAttestationVerifierDigestValue =
        fixed32(enrollmentAttestationVerifierDigest, "enrollmentAttestationVerifierDigest")
    private val attestationTrustRootsDigestValue = fixed32(attestationTrustRootsDigest, "attestationTrustRootsDigest")
    private val allowedSuiteCommitmentValue = fixed32(allowedSuiteCommitment, "allowedSuiteCommitment")
    private val qualificationReportDigestValue = fixed32(qualificationReportDigest, "qualificationReportDigest")

    init {
        require(version == 1 && protocolVersion == 1 && policyEpoch != 0L)
        require(capabilityMask == 0xffff) { "the complete Kagemusha V1 hardware capability mask is required" }
        require(java.lang.Long.compareUnsigned(validFromMs, expiresAtMs) < 0)
    }

    fun hardwareProfileId(): ByteArray = hardwareProfileIdValue.copyOf()
    fun providerId(): ByteArray = providerIdValue.copyOf()
    fun productClassDigest(): ByteArray = productClassDigestValue.copyOf()
    fun firmwarePolicyDigest(): ByteArray = firmwarePolicyDigestValue.copyOf()
    fun enrollmentAttestationVerifierDigest(): ByteArray = enrollmentAttestationVerifierDigestValue.copyOf()
    fun attestationTrustRootsDigest(): ByteArray = attestationTrustRootsDigestValue.copyOf()
    fun allowedSuiteCommitment(): ByteArray = allowedSuiteCommitmentValue.copyOf()
    fun qualificationReportDigest(): ByteArray = qualificationReportDigestValue.copyOf()
}

/** Compact governed device credential consumed by recursive hardware guards. */
class KagemushaHardwareCredentialV1(
    @JvmField val version: Int,
    credentialId: ByteArray,
    @JvmField val networkId: NetworkId,
    hardwareProfileId: ByteArray,
    suiteId: ByteArray,
    firmwarePolicyDigest: ByteArray,
    @JvmField val policyEpoch: Long,
    laneCommitment: ByteArray,
    hardwareEpochId: ByteArray,
    @JvmField val hardwareEpochGeneration: Long,
    @JvmField val devicePublicKey: KagemushaDevicePublicKeyV1,
    deviceKeyReference: ByteArray,
    @JvmField val issuedAtMs: Long,
    @JvmField val expiresAtMs: Long,
    @JvmField val governanceSignature: KagemushaDeviceSignatureV1,
) {
    private val credentialIdValue = fixed32(credentialId, "credentialId")
    private val hardwareProfileIdValue = fixed32(hardwareProfileId, "hardwareProfileId")
    private val suiteIdValue = fixed32(suiteId, "suiteId")
    private val firmwarePolicyDigestValue = fixed32(firmwarePolicyDigest, "firmwarePolicyDigest")
    private val laneCommitmentValue = fixed32(laneCommitment, "laneCommitment")
    private val hardwareEpochIdValue = fixed32(hardwareEpochId, "hardwareEpochId")
    private val deviceKeyReferenceValue = fixed32(deviceKeyReference, "deviceKeyReference")

    init {
        require(version == 1 && networkId.bytes().any { it != 0.toByte() })
        require(policyEpoch != 0L)
        require(java.lang.Long.compareUnsigned(issuedAtMs, expiresAtMs) < 0)
    }

    fun credentialId(): ByteArray = credentialIdValue.copyOf()
    fun hardwareProfileId(): ByteArray = hardwareProfileIdValue.copyOf()
    fun suiteId(): ByteArray = suiteIdValue.copyOf()
    fun firmwarePolicyDigest(): ByteArray = firmwarePolicyDigestValue.copyOf()
    fun laneCommitment(): ByteArray = laneCommitmentValue.copyOf()
    fun hardwareEpochId(): ByteArray = hardwareEpochIdValue.copyOf()
    fun deviceKeyReference(): ByteArray = deviceKeyReferenceValue.copyOf()
}

/** Exact pre-ID peer-transfer context authenticated by encrypted-credit AAD. */
class KagemushaPeerCreditContextV1(
    @JvmField val version: Int,
    requestDigest: ByteArray,
    senderBeforeCommitment: KagemushaPastaStateCommitmentV1,
    senderAfterCommitment: KagemushaPastaStateCommitmentV1,
    recipientLaneId: ByteArray,
    @JvmField val recipientEncryptionKey: KagemushaX25519PublicKeyV1,
    @JvmField val committedAtMs: Long,
    hardwareTransitionCommitment: ByteArray,
    lifecycleContextDigest: ByteArray,
) {
    private val requestDigestValue = fixed32(requestDigest, "requestDigest")
    private val senderBeforeCommitmentValue = senderBeforeCommitment
    private val senderAfterCommitmentValue = senderAfterCommitment
    private val recipientLaneIdValue = fixed32(recipientLaneId, "recipientLaneId")
    private val hardwareTransitionCommitmentValue =
        fixed32(hardwareTransitionCommitment, "hardwareTransitionCommitment")
    private val lifecycleContextDigestValue = fixed32(lifecycleContextDigest, "lifecycleContextDigest")

    init {
        require(version == KagemushaWireV1.WIRE_VERSION)
    }

    fun requestDigest(): ByteArray = requestDigestValue.copyOf()
    fun senderBeforeCommitment(): KagemushaPastaStateCommitmentV1 = senderBeforeCommitmentValue
    fun senderAfterCommitment(): KagemushaPastaStateCommitmentV1 = senderAfterCommitmentValue
    fun recipientLaneId(): ByteArray = recipientLaneIdValue.copyOf()
    fun hardwareTransitionCommitment(): ByteArray = hardwareTransitionCommitmentValue.copyOf()
    fun lifecycleContextDigest(): ByteArray = lifecycleContextDigestValue.copyOf()
}

/** Exact fixed-size recipient-only plaintext protected by the encrypted credit envelope. */
class KagemushaCreditOpeningV1(
    @JvmField val version: Int,
    creditId: ByteArray,
    @JvmField val amount: BigInteger,
    creditCommitmentOpening: ByteArray,
    recipientBindingOpening: ByteArray,
    recoveryNonce: ByteArray,
) {
    private val creditIdValue = fixed32(creditId, "creditId")
    private val creditCommitmentOpeningValue = fixed32(creditCommitmentOpening, "creditCommitmentOpening")
    private val recipientBindingOpeningValue = fixed32(recipientBindingOpening, "recipientBindingOpening")
    private val recoveryNonceValue = fixed32(recoveryNonce, "recoveryNonce")

    init {
        require(version == 1)
        requirePositiveU128(amount, "amount")
    }

    fun creditId(): ByteArray = creditIdValue.copyOf()
    fun creditCommitmentOpening(): ByteArray = creditCommitmentOpeningValue.copyOf()
    fun recipientBindingOpening(): ByteArray = recipientBindingOpeningValue.copyOf()
    fun recoveryNonce(): ByteArray = recoveryNonceValue.copyOf()
}

/** Associated-data domain selector for an encrypted credit. */
enum class KagemushaEncryptedCreditPurposeV1 { MINT, PEER }

/** Canonical associated data authenticated by each encrypted credit. */
class KagemushaEncryptedCreditAadV1(
    @JvmField val version: Int,
    @JvmField val purpose: KagemushaEncryptedCreditPurposeV1,
    contextDigest: ByteArray,
    issuanceOrTransitionCommitment: ByteArray,
    creditId: ByteArray,
    @JvmField val amount: BigInteger,
) {
    private val contextDigestValue = fixed32(contextDigest, "contextDigest")
    private val issuanceOrTransitionCommitmentValue =
        fixed32(issuanceOrTransitionCommitment, "issuanceOrTransitionCommitment")
    private val creditIdValue = fixed32(creditId, "creditId")

    init {
        require(version == 1)
        requirePositiveU128(amount, "amount")
    }

    fun contextDigest(): ByteArray = contextDigestValue.copyOf()
    fun issuanceOrTransitionCommitment(): ByteArray = issuanceOrTransitionCommitmentValue.copyOf()
    fun creditId(): ByteArray = creditIdValue.copyOf()
}

/** X25519/HKDF-SHA256/XChaCha20-Poly1305 recipient-only envelope. */
class KagemushaEncryptedCreditEnvelopeV1(
    @JvmField val version: Int,
    @JvmField val ephemeralX25519PublicKey: KagemushaX25519PublicKeyV1,
    nonce: ByteArray,
    ciphertextAndTag: ByteArray,
) {
    private val nonceValue = exactBytes(nonce, KagemushaWireV1.XCHACHA20_POLY1305_NONCE_BYTES, "nonce")
    private val ciphertextAndTagValue = ciphertextAndTag.copyOf()

    init {
        require(version == 1)
        require(
            ciphertextAndTagValue.size == KagemushaWireV1.ENCRYPTED_CREDIT_CIPHERTEXT_AND_TAG_BYTES,
        ) { "ciphertextAndTag must protect the exact fixed-size V1 credit opening" }
    }

    fun nonce(): ByteArray = nonceValue.copyOf()
    fun ciphertextAndTag(): ByteArray = ciphertextAndTagValue.copyOf()
}

/** Released monetary operation. */
enum class KagemushaOperationKindV1 {
    BOOTSTRAP,
    MINT_FOLD,
    SEND_SPLIT,
    RECEIVE_FOLD,
    REDEEM_SPLIT,
    ROTATE,
}

/** Complete history-independent lifecycle context for one released transition. */
class KagemushaLifecycleBindingV1(
    @JvmField val version: Int,
    @JvmField val networkId: NetworkId,
    @JvmField val protocolVersion: Int,
    suiteId: ByteArray,
    vkDigest: ByteArray,
    releaseId: ByteArray,
    @JvmField val asset: KagemushaAssetDefinitionIdV1,
    @JvmField val assetIncarnation: KagemushaAssetIncarnationV1,
    @JvmField val scale: Int,
    liabilityPoolId: ByteArray,
    hardwareProfileId: ByteArray,
    @JvmField val policyEpoch: Long,
    @JvmField val operationKind: KagemushaOperationKindV1,
    requestId: ByteArray,
    creditId: ByteArray,
    ciphertextDigest: ByteArray,
) {
    private val suiteIdValue = fixed32(suiteId, "suiteId")
    private val vkDigestValue = fixed32(vkDigest, "vkDigest")
    private val releaseIdValue = fixed32(releaseId, "releaseId")
    private val liabilityPoolIdValue = fixed32(liabilityPoolId, "liabilityPoolId")
    private val hardwareProfileIdValue = fixed32(hardwareProfileId, "hardwareProfileId")
    private val requestIdValue = raw32(requestId, "requestId")
    private val creditIdValue = raw32(creditId, "creditId")
    private val ciphertextDigestValue = raw32(ciphertextDigest, "ciphertextDigest")

    init {
        requireHeader(version, networkId, scale, null)
        require(protocolVersion == 1 && policyEpoch != 0L)
        val requestFieldsPresent = requestIdValue.any { it != 0.toByte() }
        val requestFieldsAbsent = requestIdValue.all { it == 0.toByte() }
        val creditFieldsPresent = listOf(creditIdValue, ciphertextDigestValue)
            .all { bytes -> bytes.any { it != 0.toByte() } }
        val creditFieldsAbsent = listOf(creditIdValue, ciphertextDigestValue)
            .all { bytes -> bytes.all { it == 0.toByte() } }
        require(
            when (operationKind) {
                KagemushaOperationKindV1.SEND_SPLIT -> requestFieldsPresent && creditFieldsPresent
                KagemushaOperationKindV1.MINT_FOLD -> requestFieldsAbsent && creditFieldsPresent
                else -> requestFieldsAbsent && creditFieldsAbsent
            },
        )
    }

    fun suiteId(): ByteArray = suiteIdValue.copyOf()
    fun vkDigest(): ByteArray = vkDigestValue.copyOf()
    fun releaseId(): ByteArray = releaseIdValue.copyOf()
    fun liabilityPoolId(): ByteArray = liabilityPoolIdValue.copyOf()
    fun hardwareProfileId(): ByteArray = hardwareProfileIdValue.copyOf()
    fun requestId(): ByteArray = requestIdValue.copyOf()
    fun creditId(): ByteArray = creditIdValue.copyOf()
    fun ciphertextDigest(): ByteArray = ciphertextDigestValue.copyOf()
}

/** Receiver-created exact-amount request reusable by any number of valid payments. */
class KagemushaPaymentRequestV1(
    @JvmField val version: Int,
    releaseId: ByteArray,
    @JvmField val networkId: NetworkId,
    @JvmField val asset: KagemushaAssetDefinitionIdV1,
    @JvmField val assetIncarnation: KagemushaAssetIncarnationV1,
    @JvmField val scale: Int,
    liabilityPoolId: ByteArray,
    @JvmField val recipient: KagemushaAccountIdV1,
    recipientLaneId: ByteArray,
    @JvmField val recipientEncryptionKey: KagemushaX25519PublicKeyV1,
    @JvmField val amount: BigInteger,
    @JvmField val hardwareCredential: KagemushaHardwareCredentialV1,
    requestId: ByteArray,
    @JvmField val issuedAtMs: Long,
    @JvmField val expiresAtMs: Long,
    @JvmField val signature: KagemushaDeviceSignatureV1,
) {
    private val releaseIdValue = fixed32(releaseId, "releaseId")
    private val liabilityPoolIdValue = fixed32(liabilityPoolId, "liabilityPoolId")
    private val recipientLaneIdValue = fixed32(recipientLaneId, "recipientLaneId")
    private val requestIdValue = fixed32(requestId, "requestId")

    init {
        requireHeader(version, networkId, scale, amount)
        require(hardwareCredential.version == version && hardwareCredential.networkId == networkId)
        require(hardwareCredential.laneCommitment().contentEquals(recipientLaneIdValue))
        require(java.lang.Long.compareUnsigned(issuedAtMs, expiresAtMs) < 0)
        require(
            java.lang.Long.compareUnsigned(
                expiresAtMs - issuedAtMs,
                KagemushaWireV1.REQUEST_MAX_TTL_MS,
            ) <= 0,
        )
    }

    fun releaseId(): ByteArray = releaseIdValue.copyOf()
    fun liabilityPoolId(): ByteArray = liabilityPoolIdValue.copyOf()
    fun recipientLaneId(): ByteArray = recipientLaneIdValue.copyOf()
    fun requestId(): ByteArray = requestIdValue.copyOf()
}

/** Public send statement exposing only opaque predecessor and successor commitments. */
class KagemushaTransferStatementV1(
    @JvmField val version: Int,
    @JvmField val lifecycle: KagemushaLifecycleBindingV1,
    @JvmField val amount: BigInteger,
    transitionNullifier: ByteArray,
    requestDigest: ByteArray,
    senderBeforeCommitment: KagemushaPastaStateCommitmentV1,
    senderAfterCommitment: KagemushaPastaStateCommitmentV1,
    recipientLaneId: ByteArray,
    @JvmField val recipientEncryptionKey: KagemushaX25519PublicKeyV1,
    @JvmField val committedAtMs: Long,
    ciphertextCommitment: ByteArray,
    hardwareTransitionCommitment: ByteArray,
) {
    private val transitionNullifierValue = fixed32(transitionNullifier, "transitionNullifier")
    private val requestDigestValue = fixed32(requestDigest, "requestDigest")
    private val senderBeforeCommitmentValue = senderBeforeCommitment
    private val senderAfterCommitmentValue = senderAfterCommitment
    private val recipientLaneIdValue = fixed32(recipientLaneId, "recipientLaneId")
    private val ciphertextCommitmentValue = fixed32(ciphertextCommitment, "ciphertextCommitment")
    private val hardwareTransitionCommitmentValue =
        fixed32(hardwareTransitionCommitment, "hardwareTransitionCommitment")

    init {
        require(version == 1 && lifecycle.version == version)
        require(lifecycle.operationKind == KagemushaOperationKindV1.SEND_SPLIT)
        requirePositiveU128(amount, "amount")
    }

    fun transitionNullifier(): ByteArray = transitionNullifierValue.copyOf()
    fun requestDigest(): ByteArray = requestDigestValue.copyOf()
    fun senderBeforeCommitment(): KagemushaPastaStateCommitmentV1 = senderBeforeCommitmentValue
    fun senderAfterCommitment(): KagemushaPastaStateCommitmentV1 = senderAfterCommitmentValue
    fun recipientLaneId(): ByteArray = recipientLaneIdValue.copyOf()
    fun ciphertextCommitment(): ByteArray = ciphertextCommitmentValue.copyOf()
    fun hardwareTransitionCommitment(): ByteArray = hardwareTransitionCommitmentValue.copyOf()
}

/** Sender response with the recursively authenticated hardware transition and paired proof. */
class KagemushaPaymentV1(
    @JvmField val version: Int,
    @JvmField val statement: KagemushaTransferStatementV1,
    @JvmField val proof: KagemushaPairedProofV1,
    encryptedCredit: ByteArray,
) {
    private val encryptedCreditValue = boundedEncryptedCredit(encryptedCredit)

    init {
        require(version == 1 && statement.version == version && proof.version == version)
    }

    fun encryptedCredit(): ByteArray = encryptedCreditValue.copyOf()
}

/** Durable secure-inbox receipt; it is not a receiver balance head. */
class KagemushaInboxReceiptV1(
    @JvmField val version: Int,
    creditId: ByteArray,
    receiptCommitment: ByteArray,
) {
    private val creditIdValue = fixed32(creditId, "creditId")
    private val receiptCommitmentValue = fixed32(receiptCommitment, "receiptCommitment")

    init {
        require(version == 1)
    }

    fun creditId(): ByteArray = creditIdValue.copyOf()
    fun receiptCommitment(): ByteArray = receiptCommitmentValue.copyOf()
}

/** Receiver acknowledgement emitted only after durable inbox persistence. */
class KagemushaAcknowledgementV1(
    @JvmField val version: Int,
    requestDigest: ByteArray,
    paymentDigest: ByteArray,
    @JvmField val inboxReceipt: KagemushaInboxReceiptV1,
    @JvmField val signature: KagemushaDeviceSignatureV1,
) {
    private val requestDigestValue = fixed32(requestDigest, "requestDigest")
    private val paymentDigestValue = fixed32(paymentDigest, "paymentDigest")

    init {
        require(version == 1 && inboxReceipt.version == version)
    }

    fun requestDigest(): ByteArray = requestDigestValue.copyOf()
    fun paymentDigest(): ByteArray = paymentDigestValue.copyOf()
}

/** Pre-ID recipient context authorized before a reserve debit. */
class KagemushaMintAuthorizationContextV1(
    @JvmField val version: Int,
    operationId: ByteArray,
    releaseId: ByteArray,
    suiteId: ByteArray,
    vkDigest: ByteArray,
    artifactManifestDigest: ByteArray,
    @JvmField val networkId: NetworkId,
    @JvmField val asset: KagemushaAssetDefinitionIdV1,
    @JvmField val assetIncarnation: KagemushaAssetIncarnationV1,
    @JvmField val scale: Int,
    liabilityPoolId: ByteArray,
    @JvmField val amount: BigInteger,
    @JvmField val payer: KagemushaAccountIdV1,
    @JvmField val recipient: KagemushaAccountIdV1,
    hardwareCredentialId: ByteArray,
    hardwareProfileId: ByteArray,
    @JvmField val policyEpoch: Long,
    recipientCredentialCommitment: ByteArray,
    creditCommitment: ByteArray,
    @JvmField val recipientOneTimeKey: KagemushaX25519PublicKeyV1,
) {
    private val operationIdValue = fixed32(operationId, "operationId")
    private val releaseIdValue = fixed32(releaseId, "releaseId")
    private val suiteIdValue = fixed32(suiteId, "suiteId")
    private val vkDigestValue = fixed32(vkDigest, "vkDigest")
    private val artifactManifestDigestValue = fixed32(artifactManifestDigest, "artifactManifestDigest")
    private val liabilityPoolIdValue = fixed32(liabilityPoolId, "liabilityPoolId")
    private val hardwareCredentialIdValue = fixed32(hardwareCredentialId, "hardwareCredentialId")
    private val hardwareProfileIdValue = fixed32(hardwareProfileId, "hardwareProfileId")
    private val recipientCredentialCommitmentValue =
        fixed32(recipientCredentialCommitment, "recipientCredentialCommitment")
    private val creditCommitmentValue = fixed32(creditCommitment, "creditCommitment")

    init {
        requireHeader(version, networkId, scale, amount)
        require(policyEpoch != 0L)
    }

    fun operationId(): ByteArray = operationIdValue.copyOf()
    fun releaseId(): ByteArray = releaseIdValue.copyOf()
    fun suiteId(): ByteArray = suiteIdValue.copyOf()
    fun vkDigest(): ByteArray = vkDigestValue.copyOf()
    fun artifactManifestDigest(): ByteArray = artifactManifestDigestValue.copyOf()
    fun liabilityPoolId(): ByteArray = liabilityPoolIdValue.copyOf()
    fun hardwareCredentialId(): ByteArray = hardwareCredentialIdValue.copyOf()
    fun hardwareProfileId(): ByteArray = hardwareProfileIdValue.copyOf()
    fun recipientCredentialCommitment(): ByteArray = recipientCredentialCommitmentValue.copyOf()
    fun creditCommitment(): ByteArray = creditCommitmentValue.copyOf()
}

/** Exact post-encryption recipient mint-authorization statement. */
class KagemushaMintAuthorizationStatementV1(
    @JvmField val version: Int,
    @JvmField val context: KagemushaMintAuthorizationContextV1,
    issuanceCommitment: ByteArray,
    creditId: ByteArray,
    ciphertextDigest: ByteArray,
) {
    private val issuanceCommitmentValue = fixed32(issuanceCommitment, "issuanceCommitment")
    private val creditIdValue = fixed32(creditId, "creditId")
    private val ciphertextDigestValue = fixed32(ciphertextDigest, "ciphertextDigest")

    init {
        require(version == 1 && context.version == version)
    }

    fun issuanceCommitment(): ByteArray = issuanceCommitmentValue.copyOf()
    fun creditId(): ByteArray = creditIdValue.copyOf()
    fun ciphertextDigest(): ByteArray = ciphertextDigestValue.copyOf()
}

/** Release-pinned proof-bearing recipient authorization verified before reserve mutation. */
class KagemushaMintAuthorizationV1(
    @JvmField val version: Int,
    @JvmField val statement: KagemushaMintAuthorizationStatementV1,
    @JvmField val proof: KagemushaPairedProofV1,
) {
    init {
        require(version == 1 && statement.version == version && proof.version == version)
    }
}

/** Public reserve-backed foldable mint-credit statement. */
class KagemushaMintCreditStatementV1(
    @JvmField val version: Int,
    @JvmField val lifecycle: KagemushaLifecycleBindingV1,
    recipientCredentialCommitment: ByteArray,
    authorizationContextDigest: ByteArray,
    mintAuthorizationDigest: ByteArray,
    @JvmField val amount: BigInteger,
    issuanceCommitment: ByteArray,
    @JvmField val recipient: KagemushaAccountIdV1,
    creditCommitment: ByteArray,
    @JvmField val mintedAtMs: Long,
) {
    private val recipientCredentialCommitmentValue =
        fixed32(recipientCredentialCommitment, "recipientCredentialCommitment")
    private val authorizationContextDigestValue = fixed32(authorizationContextDigest, "authorizationContextDigest")
    private val mintAuthorizationDigestValue = fixed32(mintAuthorizationDigest, "mintAuthorizationDigest")
    private val issuanceCommitmentValue = fixed32(issuanceCommitment, "issuanceCommitment")
    private val creditCommitmentValue = fixed32(creditCommitment, "creditCommitment")

    init {
        require(version == 1 && lifecycle.version == version)
        require(lifecycle.operationKind == KagemushaOperationKindV1.MINT_FOLD)
        requirePositiveU128(amount, "amount")
        require(mintedAtMs != 0L)
    }

    fun recipientCredentialCommitment(): ByteArray = recipientCredentialCommitmentValue.copyOf()
    fun authorizationContextDigest(): ByteArray = authorizationContextDigestValue.copyOf()
    fun mintAuthorizationDigest(): ByteArray = mintAuthorizationDigestValue.copyOf()
    fun issuanceCommitment(): ByteArray = issuanceCommitmentValue.copyOf()
    fun creditCommitment(): ByteArray = creditCommitmentValue.copyOf()
}

/** Finalized constant-size authenticated top-up credit. */
class KagemushaMintCreditV1(
    @JvmField val version: Int,
    @JvmField val statement: KagemushaMintCreditStatementV1,
    @JvmField val proof: KagemushaPairedProofV1,
    finalityCertificateBinding: ByteArray,
    finalityAuthorityHead: ByteArray,
    finalityGenesisRosterId: ByteArray,
    finalityProofBindingDigest: ByteArray,
    encryptedCredit: ByteArray,
    artifactManifestDigest: ByteArray,
) {
    private val finalityCertificateBindingValue = fixed32(finalityCertificateBinding, "finalityCertificateBinding")
    private val finalityAuthorityHeadValue = fixed32(finalityAuthorityHead, "finalityAuthorityHead")
    private val finalityGenesisRosterIdValue = fixed32(finalityGenesisRosterId, "finalityGenesisRosterId")
    private val finalityProofBindingDigestValue = fixed32(finalityProofBindingDigest, "finalityProofBindingDigest")
    private val encryptedCreditValue = boundedEncryptedCredit(encryptedCredit)
    private val artifactManifestDigestValue = fixed32(artifactManifestDigest, "artifactManifestDigest")

    init {
        require(version == 1 && statement.version == version && proof.version == version)
    }

    fun finalityCertificateBinding(): ByteArray = finalityCertificateBindingValue.copyOf()
    fun finalityAuthorityHead(): ByteArray = finalityAuthorityHeadValue.copyOf()
    fun finalityGenesisRosterId(): ByteArray = finalityGenesisRosterIdValue.copyOf()
    fun finalityProofBindingDigest(): ByteArray = finalityProofBindingDigestValue.copyOf()
    fun encryptedCredit(): ByteArray = encryptedCreditValue.copyOf()
    fun artifactManifestDigest(): ByteArray = artifactManifestDigestValue.copyOf()
}

/** Unlinkable terminal transition converting aggregate cash to an online claim. */
class KagemushaRedemptionStatementV1(
    @JvmField val version: Int,
    @JvmField val lifecycle: KagemushaLifecycleBindingV1,
    @JvmField val amount: BigInteger,
    @JvmField val beneficiary: KagemushaAccountIdV1,
    terminalNullifier: ByteArray,
    senderBeforeCommitment: KagemushaPastaStateCommitmentV1,
    senderAfterCommitment: KagemushaPastaStateCommitmentV1,
    @JvmField val committedAtMs: Long,
    redemptionCommitment: ByteArray,
    redemptionId: ByteArray,
    hardwareTransitionCommitment: ByteArray,
) {
    private val terminalNullifierValue = fixed32(terminalNullifier, "terminalNullifier")
    private val senderBeforeCommitmentValue = senderBeforeCommitment
    private val senderAfterCommitmentValue = senderAfterCommitment
    private val redemptionCommitmentValue = fixed32(redemptionCommitment, "redemptionCommitment")
    private val redemptionIdValue = fixed32(redemptionId, "redemptionId")
    private val hardwareTransitionCommitmentValue =
        fixed32(hardwareTransitionCommitment, "hardwareTransitionCommitment")

    init {
        require(version == 1 && lifecycle.version == version)
        require(lifecycle.operationKind == KagemushaOperationKindV1.REDEEM_SPLIT)
        requirePositiveU128(amount, "amount")
    }

    fun terminalNullifier(): ByteArray = terminalNullifierValue.copyOf()
    fun senderBeforeCommitment(): KagemushaPastaStateCommitmentV1 = senderBeforeCommitmentValue
    fun senderAfterCommitment(): KagemushaPastaStateCommitmentV1 = senderAfterCommitmentValue
    fun redemptionCommitment(): ByteArray = redemptionCommitmentValue.copyOf()
    fun redemptionId(): ByteArray = redemptionIdValue.copyOf()
    fun hardwareTransitionCommitment(): ByteArray = hardwareTransitionCommitmentValue.copyOf()
}

/** Constant-size terminal voucher submitted for online redemption. */
class KagemushaRedemptionVoucherV1(
    @JvmField val version: Int,
    @JvmField val statement: KagemushaRedemptionStatementV1,
    @JvmField val proof: KagemushaPairedProofV1,
) {
    init {
        require(version == 1 && statement.version == version && proof.version == version)
    }
}

internal val KAGEMUSHA_UINT128_MAX: BigInteger = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)

internal fun fixed32(value: ByteArray, field: String): ByteArray {
    val copy = raw32(value, field)
    require(copy.any { it != 0.toByte() }) { "$field must be nonzero" }
    return copy
}

internal fun raw32(value: ByteArray, field: String): ByteArray = exactBytes(value, 32, field)

internal fun exactBytes(value: ByteArray, width: Int, field: String): ByteArray {
    val copy = value.copyOf()
    require(copy.size == width) { "$field must be exactly $width bytes" }
    return copy
}

internal fun requireUnsigned128(value: BigInteger, field: String) {
    require(value.signum() >= 0 && value <= KAGEMUSHA_UINT128_MAX) {
        "$field must fit unsigned 128-bit arithmetic"
    }
}

internal fun requirePositiveU128(value: BigInteger, field: String) {
    requireUnsigned128(value, field)
    require(value.signum() > 0) { "$field must be positive" }
}

private fun requireHeader(version: Int, networkId: NetworkId, scale: Int, amount: BigInteger?) {
    require(version == KagemushaWireV1.WIRE_VERSION) { "Kagemusha wire version must be 1" }
    require(networkId.bytes().any { it != 0.toByte() }) { "networkId is zero" }
    require(scale in 0..KagemushaWireV1.MAXIMUM_ASSET_SCALE) { "asset scale is out of range" }
    amount?.let { requirePositiveU128(it, "amount") }
}

private fun boundedProof(bytes: ByteArray, field: String): ByteArray {
    val copy = bytes.copyOf()
    require(copy.isNotEmpty() && copy.size <= KagemushaWireV1.MAXIMUM_PARITY_PROOF_BYTES) {
        "$field is empty or oversized"
    }
    return copy
}

private fun exactHistory(bytes: ByteArray, field: String): ByteArray {
    val copy = exactBytes(bytes, KagemushaWireV1.HISTORY_ACCUMULATOR_BYTES, field)
    require(copy.any { it != 0.toByte() }) { "$field is zero" }
    return copy
}

private fun boundedEncryptedCredit(bytes: ByteArray): ByteArray {
    val copy = bytes.copyOf()
    require(copy.isNotEmpty() && copy.size <= KagemushaWireV1.MAXIMUM_ENCRYPTED_CREDIT_BYTES) {
        "encryptedCredit is empty or oversized"
    }
    return copy
}

private fun requireX25519PublicKey(bytes: ByteArray): ByteArray {
    val copy = exactBytes(bytes, KagemushaWireV1.X25519_PUBLIC_KEY_BYTES, "X25519 public key")
    require(copy.any { it != 0.toByte() }) { "X25519 public key must be nonzero" }
    return copy
}

private fun requireFixedByteArrayPayload(payload: ByteArray, width: Int, field: String) {
    var offset = 0
    repeat(width) {
        require(offset < payload.size && payload[offset].toInt() == 1) {
            "$field is not a canonical fixed-byte-array payload"
        }
        offset += 2
    }
    require(offset == payload.size) { "$field has trailing bytes" }
}
