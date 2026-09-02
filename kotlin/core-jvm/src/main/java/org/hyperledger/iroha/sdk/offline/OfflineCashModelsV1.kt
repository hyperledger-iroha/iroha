// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder

/** Exact typed `AssetDefinitionId` payload used by Offline Cash V1. */
class OfflineCashAssetDefinitionIdV1 private constructor(payload: ByteArray) {
    private val value = payload.copyOf()

    /** Return a defensive copy of the canonical bare Norito payload. */
    fun canonicalPayload(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashAssetDefinitionIdV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()

    companion object {
        /** Parse a canonical asset-definition address. */
        @JvmStatic
        fun parse(literal: String): OfflineCashAssetDefinitionIdV1 =
            OfflineCashAssetDefinitionIdV1(
                TransferWirePayloadEncoder.encodeAssetDefinitionIdPayload(literal),
            )

        /** Parse an exact canonical bare Norito `AssetDefinitionId` payload. */
        @JvmStatic
        fun fromCanonicalPayload(payload: ByteArray): OfflineCashAssetDefinitionIdV1 {
            requireFixedByteArrayPayload(payload, 16, "asset")
            return OfflineCashAssetDefinitionIdV1(payload)
        }
    }
}

/** Exact typed universal `AccountId` payload used by Offline Cash V1. */
class OfflineCashAccountIdV1 private constructor(payload: ByteArray) {
    private val value = payload.copyOf()

    /** Return a defensive copy of the canonical bare Norito payload. */
    fun canonicalPayload(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashAccountIdV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()

    companion object {
        /** Parse a canonical I105 account literal. */
        @JvmStatic
        fun parse(literal: String): OfflineCashAccountIdV1 =
            OfflineCashAccountIdV1(TransferWirePayloadEncoder.encodeAccountIdPayload(literal))

        /** Parse an exact canonical bare Norito `AccountId` payload. */
        @JvmStatic
        fun fromCanonicalPayload(payload: ByteArray): OfflineCashAccountIdV1 {
            require(payload.isNotEmpty() && payload.size <= 512) {
                "Offline Cash V1 account payload is empty or oversized"
            }
            val rendered = TransferWirePayloadEncoder.decodeAccountIdPayload(payload, 0)
            require(
                TransferWirePayloadEncoder.encodeAccountIdPayload(rendered).contentEquals(payload),
            ) { "Offline Cash V1 account payload is not canonical" }
            return OfflineCashAccountIdV1(payload)
        }
    }
}

/** Canonical non-zero marked hash naming one asset-registration incarnation. */
class OfflineCashAssetIncarnationV1(bytes: ByteArray) {
    private val value = raw32(bytes, "assetIncarnation")

    init {
        require(value[31].toInt() and 1 == 1) {
            "assetIncarnation must carry the canonical Iroha hash marker"
        }
    }

    /** Return the canonical marked hash bytes. */
    fun bytes(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashAssetIncarnationV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()
}

/** Canonical uncompressed SEC1 P-256 Offline Cash V1 authority key. */
class OfflineCashDevicePublicKeyV1(sec1Bytes: ByteArray) {
    private val value = OfflineCashP256Codec.requireUncompressedPublicKey(sec1Bytes)

    /** Return the exact `04 || x || y` bytes. */
    fun sec1Bytes(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashDevicePublicKeyV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()
}

/** Canonical fixed-width low-S P-256 Offline Cash V1 signature. */
class OfflineCashDeviceSignatureV1(rawBytes: ByteArray) {
    private val value = OfflineCashP256Codec.requireRawLowSSignature(rawBytes)

    /** Return the exact fixed-width `r || s` bytes. */
    fun rawBytes(): ByteArray = value.copyOf()

    /** Return the equivalent strict DER signature. */
    fun strictDer(): ByteArray = OfflineCashP256Codec.strictDerFromRawLowS(value)

    override fun equals(other: Any?): Boolean =
        other is OfflineCashDeviceSignatureV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()

    companion object {
        /** Parse strict DER and normalize a valid signature to low-S form. */
        @JvmStatic
        fun fromStrictDer(derBytes: ByteArray): OfflineCashDeviceSignatureV1 =
            OfflineCashDeviceSignatureV1(OfflineCashP256Codec.rawLowSFromStrictDer(derBytes))
    }
}

/**
 * Canonical 32-byte nonzero X25519 wire shape used by recipient-only envelopes.
 *
 * This codec type deliberately performs no scalar multiplication or managed group validation.
 * The shared native Offline Cash core authenticates canonical X25519 elements during object and
 * complete-exchange validation before any monetary use.
 */
class OfflineCashX25519PublicKeyV1(bytes: ByteArray) {
    private val value = requireX25519PublicKey(bytes)

    /** Return the exact 32-byte Montgomery u-coordinate. */
    fun bytes(): ByteArray = value.copyOf()

    override fun equals(other: Any?): Boolean =
        other is OfflineCashX25519PublicKeyV1 && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()
}

/** Constant-size public commitment to one private aggregate balance. */
class OfflineCashAggregateStateCommitmentV1(
    @JvmField val version: Int,
    releaseId: ByteArray,
    @JvmField val networkId: NetworkId,
    @JvmField val asset: OfflineCashAssetDefinitionIdV1,
    @JvmField val assetIncarnation: OfflineCashAssetIncarnationV1,
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
class OfflineCashPastaStateCommitmentV1(eq: ByteArray, ep: ByteArray) {
    private val eqValue = raw32(eq, "eq")
    private val epValue = raw32(ep, "ep")

    fun eq(): ByteArray = eqValue.copyOf()
    fun ep(): ByteArray = epValue.copyOf()
    fun isZero(): Boolean = eqValue.all { it == 0.toByte() } && epValue.all { it == 0.toByte() }
}

/** Closed paired-Pasta proof with unlinkable statement-scoped history accumulators. */
class OfflineCashPairedProofV1(
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
        require(version == OfflineCashWireV1.WIRE_VERSION)
        require(!eqProtocolDigestValue.contentEquals(epProtocolDigestValue))
        require(!guardEqCredentialAuditValue.contentEquals(guardEpCredentialAuditValue))
        require(!eqDeferredAuditValue.contentEquals(epDeferredAuditValue))
        require(eqProofValue.size + epProofValue.size <= OfflineCashWireV1.MAXIMUM_CURRENT_PROOFS_BYTES)
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
enum class OfflineCashHardwarePlatformClassV1 {
    ANDROID_OEM_SERVICE,
    APPLE_OEM_SERVICE,
    DEDICATED_SECURE_ELEMENT,
    OTHER_QUALIFIED,
}

/** Governed non-forking hardware-service profile. */
class OfflineCashHardwareProfileV1(
    @JvmField val version: Int,
    @JvmField val protocolVersion: Int,
    hardwareProfileId: ByteArray,
    providerId: ByteArray,
    @JvmField val platformClass: OfflineCashHardwarePlatformClassV1,
    productClassDigest: ByteArray,
    firmwarePolicyDigest: ByteArray,
    enrollmentAttestationVerifierDigest: ByteArray,
    attestationTrustRootsDigest: ByteArray,
    allowedSuiteCommitment: ByteArray,
    @JvmField val policyEpoch: Long,
    @JvmField val governanceCredentialPublicKey: OfflineCashDevicePublicKeyV1,
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
        require(version == 1 && protocolVersion == 1 && policyEpoch > 0)
        require(capabilityMask == 0xffff) { "the complete Offline Cash V1 hardware capability mask is required" }
        require(validFromMs >= 0 && expiresAtMs > validFromMs)
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
class OfflineCashHardwareCredentialV1(
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
    @JvmField val devicePublicKey: OfflineCashDevicePublicKeyV1,
    deviceKeyReference: ByteArray,
    @JvmField val issuedAtMs: Long,
    @JvmField val expiresAtMs: Long,
    @JvmField val governanceSignature: OfflineCashDeviceSignatureV1,
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
        require(policyEpoch > 0 && hardwareEpochGeneration >= 0)
        require(issuedAtMs >= 0 && expiresAtMs > issuedAtMs)
    }

    fun credentialId(): ByteArray = credentialIdValue.copyOf()
    fun hardwareProfileId(): ByteArray = hardwareProfileIdValue.copyOf()
    fun suiteId(): ByteArray = suiteIdValue.copyOf()
    fun firmwarePolicyDigest(): ByteArray = firmwarePolicyDigestValue.copyOf()
    fun laneCommitment(): ByteArray = laneCommitmentValue.copyOf()
    fun hardwareEpochId(): ByteArray = hardwareEpochIdValue.copyOf()
    fun deviceKeyReference(): ByteArray = deviceKeyReferenceValue.copyOf()
}

/** Inclusive positive amount interval used by reusable requests. */
class OfflineCashAmountPolicyV1(
    @JvmField val minimumAmount: BigInteger,
    @JvmField val maximumAmount: BigInteger,
) {
    init {
        requireUnsigned128(minimumAmount, "minimumAmount")
        requireUnsigned128(maximumAmount, "maximumAmount")
        require(minimumAmount.signum() > 0 && minimumAmount <= maximumAmount)
    }

    fun contains(amount: BigInteger): Boolean = amount >= minimumAmount && amount <= maximumAmount
}

/** Closed reusable receiver-request policy. */
sealed class OfflineCashPaymentRequestModeV1 {
    /** Exactly one payment of [amount]. */
    class SingleExact(@JvmField val amount: BigInteger) : OfflineCashPaymentRequestModeV1() {
        init {
            requirePositiveU128(amount, "amount")
        }
    }

    /** Independently ticketed partial payments up to [totalAmount]. */
    class PartialUntilTotal(@JvmField val totalAmount: BigInteger) : OfflineCashPaymentRequestModeV1() {
        init {
            requirePositiveU128(totalAmount, "totalAmount")
        }
    }

    /** At most [maxPayments] payments, each admitted by [perPayment]. */
    class BoundedMultiPayment(
        @JvmField val maxPayments: Int,
        @JvmField val perPayment: OfflineCashAmountPolicyV1,
    ) : OfflineCashPaymentRequestModeV1() {
        init {
            require(maxPayments > 0)
        }
    }

    /** Unbounded cumulative count of independently ticketed payments. */
    class OpenReceive(@JvmField val perPayment: OfflineCashAmountPolicyV1) : OfflineCashPaymentRequestModeV1()

    /** Return whether one exact ticket amount satisfies the stateless policy. */
    fun acceptsExactAmount(amount: BigInteger): Boolean = when (this) {
        is SingleExact -> amount == this.amount
        is PartialUntilTotal -> amount.signum() > 0 && amount <= totalAmount
        is BoundedMultiPayment -> perPayment.contains(amount)
        is OpenReceive -> perPayment.contains(amount)
    }
}

/** Sender-selected one-use intent presented before receiver capacity is reserved. */
class OfflineCashAcceptanceIntentV1(
    @JvmField val version: Int,
    requestDigest: ByteArray,
    intentId: ByteArray,
    @JvmField val exactAmount: BigInteger,
    senderOneTimeCommitment: ByteArray,
) {
    private val requestDigestValue = fixed32(requestDigest, "requestDigest")
    private val intentIdValue = fixed32(intentId, "intentId")
    private val senderOneTimeCommitmentValue = fixed32(senderOneTimeCommitment, "senderOneTimeCommitment")

    init {
        require(version == 1)
        requirePositiveU128(exactAmount, "exactAmount")
    }

    fun requestDigest(): ByteArray = requestDigestValue.copyOf()
    fun intentId(): ByteArray = intentIdValue.copyOf()
    fun senderOneTimeCommitment(): ByteArray = senderOneTimeCommitmentValue.copyOf()
}

/** Release-wide public statement for pre-ticket sender authorization. */
class OfflineCashAcceptanceIntentAuthorizationStatementV1(
    @JvmField val version: Int,
    @JvmField val intent: OfflineCashAcceptanceIntentV1,
    releaseId: ByteArray,
    suiteId: ByteArray,
    vkDigest: ByteArray,
    artifactManifestDigest: ByteArray,
) {
    private val releaseIdValue = fixed32(releaseId, "releaseId")
    private val suiteIdValue = fixed32(suiteId, "suiteId")
    private val vkDigestValue = fixed32(vkDigest, "vkDigest")
    private val artifactManifestDigestValue = fixed32(artifactManifestDigest, "artifactManifestDigest")

    init {
        require(version == 1 && intent.version == version)
    }

    fun releaseId(): ByteArray = releaseIdValue.copyOf()
    fun suiteId(): ByteArray = suiteIdValue.copyOf()
    fun vkDigest(): ByteArray = vkDigestValue.copyOf()
    fun artifactManifestDigest(): ByteArray = artifactManifestDigestValue.copyOf()
}

/** Proof-bearing sender capability required before request budget or inbox capacity is consumed. */
class OfflineCashAcceptanceIntentAuthorizationV1(
    @JvmField val version: Int,
    @JvmField val statement: OfflineCashAcceptanceIntentAuthorizationStatementV1,
    @JvmField val proof: OfflineCashPairedProofV1,
) {
    init {
        require(version == 1 && statement.version == version && proof.version == version)
    }
}

/** Unlinkable public statement proving irreversible cancellation of one sender authorization. */
class OfflineCashNoCommitClosureStatementV1(
    @JvmField val version: Int,
    releaseId: ByteArray,
    suiteId: ByteArray,
    vkDigest: ByteArray,
    artifactManifestDigest: ByteArray,
    senderHardwareBindingCommitment: ByteArray,
    requestId: ByteArray,
    requestDigest: ByteArray,
    acceptanceTicketId: ByteArray,
    ticketDigest: ByteArray,
    intentAuthorizationDigest: ByteArray,
    intentDigest: ByteArray,
    @JvmField val exactAmount: BigInteger,
    senderOneTimeCommitment: ByteArray,
    recoveryId: ByteArray,
    cancellationNullifier: ByteArray,
    equivalentDeliverySlotCommitment: ByteArray,
) {
    private val releaseIdValue = fixed32(releaseId, "releaseId")
    private val suiteIdValue = fixed32(suiteId, "suiteId")
    private val vkDigestValue = fixed32(vkDigest, "vkDigest")
    private val artifactManifestDigestValue = fixed32(artifactManifestDigest, "artifactManifestDigest")
    private val senderHardwareBindingCommitmentValue =
        fixed32(senderHardwareBindingCommitment, "senderHardwareBindingCommitment")
    private val requestIdValue = fixed32(requestId, "requestId")
    private val requestDigestValue = fixed32(requestDigest, "requestDigest")
    private val acceptanceTicketIdValue = fixed32(acceptanceTicketId, "acceptanceTicketId")
    private val ticketDigestValue = fixed32(ticketDigest, "ticketDigest")
    private val intentAuthorizationDigestValue = fixed32(intentAuthorizationDigest, "intentAuthorizationDigest")
    private val intentDigestValue = fixed32(intentDigest, "intentDigest")
    private val senderOneTimeCommitmentValue = fixed32(senderOneTimeCommitment, "senderOneTimeCommitment")
    private val recoveryIdValue = fixed32(recoveryId, "recoveryId")
    private val cancellationNullifierValue = fixed32(cancellationNullifier, "cancellationNullifier")
    private val equivalentDeliverySlotCommitmentValue =
        fixed32(equivalentDeliverySlotCommitment, "equivalentDeliverySlotCommitment")

    init {
        require(version == OfflineCashWireV1.WIRE_VERSION)
        requirePositiveU128(exactAmount, "exactAmount")
    }

    fun releaseId(): ByteArray = releaseIdValue.copyOf()
    fun suiteId(): ByteArray = suiteIdValue.copyOf()
    fun vkDigest(): ByteArray = vkDigestValue.copyOf()
    fun artifactManifestDigest(): ByteArray = artifactManifestDigestValue.copyOf()
    fun senderHardwareBindingCommitment(): ByteArray = senderHardwareBindingCommitmentValue.copyOf()
    fun requestId(): ByteArray = requestIdValue.copyOf()
    fun requestDigest(): ByteArray = requestDigestValue.copyOf()
    fun acceptanceTicketId(): ByteArray = acceptanceTicketIdValue.copyOf()
    fun ticketDigest(): ByteArray = ticketDigestValue.copyOf()
    fun intentAuthorizationDigest(): ByteArray = intentAuthorizationDigestValue.copyOf()
    fun intentDigest(): ByteArray = intentDigestValue.copyOf()
    fun senderOneTimeCommitment(): ByteArray = senderOneTimeCommitmentValue.copyOf()
    fun recoveryId(): ByteArray = recoveryIdValue.copyOf()
    fun cancellationNullifier(): ByteArray = cancellationNullifierValue.copyOf()
    fun equivalentDeliverySlotCommitment(): ByteArray = equivalentDeliverySlotCommitmentValue.copyOf()
}

/** Complete bounded no-commit recovery envelope; recursive verification remains native-only. */
class OfflineCashNoCommitClosureV1(
    @JvmField val version: Int,
    @JvmField val statement: OfflineCashNoCommitClosureStatementV1,
    @JvmField val request: OfflineCashPaymentRequestV1,
    @JvmField val intentAuthorization: OfflineCashAcceptanceIntentAuthorizationV1,
    @JvmField val acceptanceTicket: OfflineCashAcceptanceTicketV1,
    @JvmField val proof: OfflineCashPairedProofV1,
) {
    init {
        require(version == OfflineCashWireV1.WIRE_VERSION)
        require(statement.version == version)
        require(request.version == version)
        require(intentAuthorization.version == version)
        require(acceptanceTicket.version == version)
        require(proof.version == version)
    }
}

/** One-use receiver-hardware capacity reservation issued after sender proof verification. */
class OfflineCashAcceptanceTicketV1(
    @JvmField val version: Int,
    @JvmField val networkId: NetworkId,
    requestId: ByteArray,
    requestDigest: ByteArray,
    acceptanceTicketId: ByteArray,
    @JvmField val asset: OfflineCashAssetDefinitionIdV1,
    @JvmField val assetIncarnation: OfflineCashAssetIncarnationV1,
    @JvmField val scale: Int,
    @JvmField val requestMode: OfflineCashPaymentRequestModeV1,
    intentDigest: ByteArray,
    @JvmField val exactAmount: BigInteger,
    @JvmField val reservedInboxBytes: Int,
    @JvmField val recipientOneTimeKey: OfflineCashX25519PublicKeyV1,
    hardwareProfileId: ByteArray,
    @JvmField val policyEpoch: Long,
    @JvmField val issuedAtMs: Long,
    @JvmField val expiresAtMs: Long,
    @JvmField val signature: OfflineCashDeviceSignatureV1,
) {
    private val requestIdValue = fixed32(requestId, "requestId")
    private val requestDigestValue = fixed32(requestDigest, "requestDigest")
    private val acceptanceTicketIdValue = fixed32(acceptanceTicketId, "acceptanceTicketId")
    private val intentDigestValue = fixed32(intentDigest, "intentDigest")
    private val hardwareProfileIdValue = fixed32(hardwareProfileId, "hardwareProfileId")

    init {
        requireHeader(version, networkId, scale, exactAmount)
        require(reservedInboxBytes >= OfflineCashWireV1.ACCEPTANCE_TICKET_MIN_RESERVED_INBOX_BYTES)
        require(policyEpoch > 0 && issuedAtMs >= 0 && expiresAtMs > issuedAtMs)
        require(requestMode.acceptsExactAmount(exactAmount))
    }

    fun requestId(): ByteArray = requestIdValue.copyOf()
    fun requestDigest(): ByteArray = requestDigestValue.copyOf()
    fun acceptanceTicketId(): ByteArray = acceptanceTicketIdValue.copyOf()
    fun intentDigest(): ByteArray = intentDigestValue.copyOf()
    fun hardwareProfileId(): ByteArray = hardwareProfileIdValue.copyOf()
}

/** Exact pre-ID peer-transfer context authenticated by encrypted-credit AAD. */
class OfflineCashPeerCreditContextV1(
    @JvmField val version: Int,
    requestDigest: ByteArray,
    acceptanceIntentDigest: ByteArray,
    acceptanceTicketDigest: ByteArray,
    lifecycleContextDigest: ByteArray,
    @JvmField val recipientOneTimeKey: OfflineCashX25519PublicKeyV1,
) {
    private val requestDigestValue = fixed32(requestDigest, "requestDigest")
    private val acceptanceIntentDigestValue = fixed32(acceptanceIntentDigest, "acceptanceIntentDigest")
    private val acceptanceTicketDigestValue = fixed32(acceptanceTicketDigest, "acceptanceTicketDigest")
    private val lifecycleContextDigestValue = fixed32(lifecycleContextDigest, "lifecycleContextDigest")

    init {
        require(version == OfflineCashWireV1.WIRE_VERSION)
    }

    fun requestDigest(): ByteArray = requestDigestValue.copyOf()
    fun acceptanceIntentDigest(): ByteArray = acceptanceIntentDigestValue.copyOf()
    fun acceptanceTicketDigest(): ByteArray = acceptanceTicketDigestValue.copyOf()
    fun lifecycleContextDigest(): ByteArray = lifecycleContextDigestValue.copyOf()
}

/** Exact fixed-size recipient-only plaintext protected by the encrypted credit envelope. */
class OfflineCashCreditOpeningV1(
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
enum class OfflineCashEncryptedCreditPurposeV1 { MINT, PEER }

/** Canonical associated data authenticated by each encrypted credit. */
class OfflineCashEncryptedCreditAadV1(
    @JvmField val version: Int,
    @JvmField val purpose: OfflineCashEncryptedCreditPurposeV1,
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
class OfflineCashEncryptedCreditEnvelopeV1(
    @JvmField val version: Int,
    @JvmField val ephemeralX25519PublicKey: OfflineCashX25519PublicKeyV1,
    nonce: ByteArray,
    ciphertextAndTag: ByteArray,
) {
    private val nonceValue = exactBytes(nonce, OfflineCashWireV1.XCHACHA20_POLY1305_NONCE_BYTES, "nonce")
    private val ciphertextAndTagValue = ciphertextAndTag.copyOf()

    init {
        require(version == 1)
        require(
            ciphertextAndTagValue.size == OfflineCashWireV1.ENCRYPTED_CREDIT_CIPHERTEXT_AND_TAG_BYTES,
        ) { "ciphertextAndTag must protect the exact fixed-size V1 credit opening" }
    }

    fun nonce(): ByteArray = nonceValue.copyOf()
    fun ciphertextAndTag(): ByteArray = ciphertextAndTagValue.copyOf()
}

/** Released monetary operation. */
enum class OfflineCashOperationKindV1 {
    BOOTSTRAP,
    MINT_FOLD,
    SEND_SPLIT,
    RECEIVE_FOLD_BATCH,
    REDEEM_SPLIT,
    SUITE_UPGRADE,
    ROTATE,
}

/** Complete history-independent lifecycle context for one released transition. */
class OfflineCashLifecycleBindingV1(
    @JvmField val version: Int,
    @JvmField val networkId: NetworkId,
    @JvmField val protocolVersion: Int,
    suiteId: ByteArray,
    vkDigest: ByteArray,
    releaseId: ByteArray,
    @JvmField val asset: OfflineCashAssetDefinitionIdV1,
    @JvmField val assetIncarnation: OfflineCashAssetIncarnationV1,
    @JvmField val scale: Int,
    liabilityPoolId: ByteArray,
    hardwareProfileId: ByteArray,
    @JvmField val policyEpoch: Long,
    @JvmField val operationKind: OfflineCashOperationKindV1,
    requestId: ByteArray,
    acceptanceTicketId: ByteArray,
    creditId: ByteArray,
    ciphertextDigest: ByteArray,
) {
    private val suiteIdValue = fixed32(suiteId, "suiteId")
    private val vkDigestValue = fixed32(vkDigest, "vkDigest")
    private val releaseIdValue = fixed32(releaseId, "releaseId")
    private val liabilityPoolIdValue = fixed32(liabilityPoolId, "liabilityPoolId")
    private val hardwareProfileIdValue = fixed32(hardwareProfileId, "hardwareProfileId")
    private val requestIdValue = raw32(requestId, "requestId")
    private val acceptanceTicketIdValue = raw32(acceptanceTicketId, "acceptanceTicketId")
    private val creditIdValue = raw32(creditId, "creditId")
    private val ciphertextDigestValue = raw32(ciphertextDigest, "ciphertextDigest")

    init {
        requireHeader(version, networkId, scale, null)
        require(protocolVersion == 1 && policyEpoch > 0)
        val requestFieldsPresent = listOf(requestIdValue, acceptanceTicketIdValue)
            .all { bytes -> bytes.any { it != 0.toByte() } }
        val requestFieldsAbsent = listOf(requestIdValue, acceptanceTicketIdValue)
            .all { bytes -> bytes.all { it == 0.toByte() } }
        val creditFieldsPresent = listOf(creditIdValue, ciphertextDigestValue)
            .all { bytes -> bytes.any { it != 0.toByte() } }
        val creditFieldsAbsent = listOf(creditIdValue, ciphertextDigestValue)
            .all { bytes -> bytes.all { it == 0.toByte() } }
        require(
            when (operationKind) {
                OfflineCashOperationKindV1.SEND_SPLIT -> requestFieldsPresent && creditFieldsPresent
                OfflineCashOperationKindV1.MINT_FOLD -> requestFieldsAbsent && creditFieldsPresent
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
    fun acceptanceTicketId(): ByteArray = acceptanceTicketIdValue.copyOf()
    fun creditId(): ByteArray = creditIdValue.copyOf()
    fun ciphertextDigest(): ByteArray = ciphertextDigestValue.copyOf()
}

/** Trusted-time or secure monotonic-lease commit evidence. */
sealed class OfflineCashCommitEvidenceV1 {
    /** Hiding trusted-time evidence commitment. */
    class TrustedTime(timeEvidenceCommitment: ByteArray) : OfflineCashCommitEvidenceV1() {
        private val value = fixed32(timeEvidenceCommitment, "timeEvidenceCommitment")
        fun timeEvidenceCommitment(): ByteArray = value.copyOf()
    }

    /** Hiding secure monotonic-lease evidence commitment. */
    class MonotonicLease(leaseEvidenceCommitment: ByteArray) : OfflineCashCommitEvidenceV1() {
        private val value = fixed32(leaseEvidenceCommitment, "leaseEvidenceCommitment")
        fun leaseEvidenceCommitment(): ByteArray = value.copyOf()
    }
}

/** Sender outbox capacity reserved before a predecessor can be consumed. */
class OfflineCashOutboxReservationV1(
    reservationId: ByteArray,
    @JvmField val operationKind: OfflineCashOperationKindV1,
    @JvmField val reservedOutboxBytes: Int,
    @JvmField val issuedAtMs: Long,
    @JvmField val expiresAtMs: Long,
) {
    private val reservationIdValue = fixed32(reservationId, "reservationId")

    init {
        require(reservedOutboxBytes > 0 && issuedAtMs >= 0 && expiresAtMs > issuedAtMs)
    }

    fun reservationId(): ByteArray = reservationIdValue.copyOf()
}

/** Self-free private terminal body committed before certificate identity derivation. */
class OfflineCashHardwareTerminalBodyV1(
    @JvmField val version: Int,
    candidateEnvelopeDigest: ByteArray,
    lifecycleBindingDigest: ByteArray,
    transitionNullifier: ByteArray,
    outboxReservationCommitment: ByteArray,
    @JvmField val commitEvidence: OfflineCashCommitEvidenceV1,
    hardwareProfileId: ByteArray,
    @JvmField val policyEpoch: Long,
    privateSuccessorCommitment: ByteArray,
    privateJournalCommitment: ByteArray,
    privateRecoveryCommitment: ByteArray,
) {
    private val candidateEnvelopeDigestValue = fixed32(candidateEnvelopeDigest, "candidateEnvelopeDigest")
    private val lifecycleBindingDigestValue = fixed32(lifecycleBindingDigest, "lifecycleBindingDigest")
    private val transitionNullifierValue = fixed32(transitionNullifier, "transitionNullifier")
    private val outboxReservationCommitmentValue = fixed32(outboxReservationCommitment, "outboxReservationCommitment")
    private val hardwareProfileIdValue = fixed32(hardwareProfileId, "hardwareProfileId")
    private val privateSuccessorCommitmentValue = fixed32(privateSuccessorCommitment, "privateSuccessorCommitment")
    private val privateJournalCommitmentValue = fixed32(privateJournalCommitment, "privateJournalCommitment")
    private val privateRecoveryCommitmentValue = fixed32(privateRecoveryCommitment, "privateRecoveryCommitment")

    init {
        require(version == 1 && policyEpoch > 0)
    }

    fun candidateEnvelopeDigest(): ByteArray = candidateEnvelopeDigestValue.copyOf()
    fun lifecycleBindingDigest(): ByteArray = lifecycleBindingDigestValue.copyOf()
    fun transitionNullifier(): ByteArray = transitionNullifierValue.copyOf()
    fun outboxReservationCommitment(): ByteArray = outboxReservationCommitmentValue.copyOf()
    fun hardwareProfileId(): ByteArray = hardwareProfileIdValue.copyOf()
    fun privateSuccessorCommitment(): ByteArray = privateSuccessorCommitmentValue.copyOf()
    fun privateJournalCommitment(): ByteArray = privateJournalCommitmentValue.copyOf()
    fun privateRecoveryCommitment(): ByteArray = privateRecoveryCommitmentValue.copyOf()
}

/** Recoverable hardware terminal certificate emitted by atomic commit. */
class OfflineCashCommitCertificateV1(
    @JvmField val version: Int,
    certificateId: ByteArray,
    candidateEnvelopeDigest: ByteArray,
    lifecycleBindingDigest: ByteArray,
    transitionNullifier: ByteArray,
    outboxReservationCommitment: ByteArray,
    @JvmField val commitEvidence: OfflineCashCommitEvidenceV1,
    hardwareProfileId: ByteArray,
    @JvmField val policyEpoch: Long,
    hardwareTerminalCommitment: ByteArray,
) {
    private val certificateIdValue = fixed32(certificateId, "certificateId")
    private val candidateEnvelopeDigestValue = fixed32(candidateEnvelopeDigest, "candidateEnvelopeDigest")
    private val lifecycleBindingDigestValue = fixed32(lifecycleBindingDigest, "lifecycleBindingDigest")
    private val transitionNullifierValue = fixed32(transitionNullifier, "transitionNullifier")
    private val outboxReservationCommitmentValue = fixed32(outboxReservationCommitment, "outboxReservationCommitment")
    private val hardwareProfileIdValue = fixed32(hardwareProfileId, "hardwareProfileId")
    private val hardwareTerminalCommitmentValue = fixed32(hardwareTerminalCommitment, "hardwareTerminalCommitment")

    init {
        require(version == 1 && policyEpoch > 0)
    }

    fun certificateId(): ByteArray = certificateIdValue.copyOf()
    fun candidateEnvelopeDigest(): ByteArray = candidateEnvelopeDigestValue.copyOf()
    fun lifecycleBindingDigest(): ByteArray = lifecycleBindingDigestValue.copyOf()
    fun transitionNullifier(): ByteArray = transitionNullifierValue.copyOf()
    fun outboxReservationCommitment(): ByteArray = outboxReservationCommitmentValue.copyOf()
    fun hardwareProfileId(): ByteArray = hardwareProfileIdValue.copyOf()
    fun hardwareTerminalCommitment(): ByteArray = hardwareTerminalCommitmentValue.copyOf()
}

/** Final paired proof that authenticates a prepared transition and terminal certificate. */
class OfflineCashCommitWrapperProofV1(
    @JvmField val version: Int,
    eqProtocolDigest: ByteArray,
    epProtocolDigest: ByteArray,
    semanticDigest: ByteArray,
    candidateEnvelopeDigest: ByteArray,
    commitCertificateDigest: ByteArray,
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
    private val candidateEnvelopeDigestValue = fixed32(candidateEnvelopeDigest, "candidateEnvelopeDigest")
    private val commitCertificateDigestValue = fixed32(commitCertificateDigest, "commitCertificateDigest")
    private val eqDeferredAuditValue = fixed32(eqDeferredAudit, "eqDeferredAudit")
    private val epDeferredAuditValue = fixed32(epDeferredAudit, "epDeferredAudit")
    private val eqProofValue = boundedProof(eqProof, "eqProof")
    private val epProofValue = boundedProof(epProof, "epProof")
    private val eqHistoryValue = exactHistory(eqHistory, "eqHistory")
    private val epHistoryValue = exactHistory(epHistory, "epHistory")

    init {
        require(version == 1)
        require(!eqProtocolDigestValue.contentEquals(epProtocolDigestValue))
        require(!eqDeferredAuditValue.contentEquals(epDeferredAuditValue))
        require(eqProofValue.size + epProofValue.size <= OfflineCashWireV1.MAXIMUM_CURRENT_PROOFS_BYTES)
        require(!eqHistoryValue.contentEquals(epHistoryValue))
    }

    fun eqProtocolDigest(): ByteArray = eqProtocolDigestValue.copyOf()
    fun epProtocolDigest(): ByteArray = epProtocolDigestValue.copyOf()
    fun semanticDigest(): ByteArray = semanticDigestValue.copyOf()
    fun candidateEnvelopeDigest(): ByteArray = candidateEnvelopeDigestValue.copyOf()
    fun commitCertificateDigest(): ByteArray = commitCertificateDigestValue.copyOf()
    fun eqDeferredAudit(): ByteArray = eqDeferredAuditValue.copyOf()
    fun epDeferredAudit(): ByteArray = epDeferredAuditValue.copyOf()
    fun eqProof(): ByteArray = eqProofValue.copyOf()
    fun epProof(): ByteArray = epProofValue.copyOf()
    fun eqHistory(): ByteArray = eqHistoryValue.copyOf()
    fun epHistory(): ByteArray = epHistoryValue.copyOf()
}

/** Receiver-created reusable request; every payment still requires a separate ticket. */
class OfflineCashPaymentRequestV1(
    @JvmField val version: Int,
    releaseId: ByteArray,
    @JvmField val networkId: NetworkId,
    @JvmField val asset: OfflineCashAssetDefinitionIdV1,
    @JvmField val assetIncarnation: OfflineCashAssetIncarnationV1,
    @JvmField val scale: Int,
    liabilityPoolId: ByteArray,
    @JvmField val recipient: OfflineCashAccountIdV1,
    @JvmField val requestMode: OfflineCashPaymentRequestModeV1,
    @JvmField val hardwareCredential: OfflineCashHardwareCredentialV1,
    requestId: ByteArray,
    @JvmField val issuedAtMs: Long,
    @JvmField val expiresAtMs: Long,
    @JvmField val signature: OfflineCashDeviceSignatureV1,
) {
    private val releaseIdValue = fixed32(releaseId, "releaseId")
    private val liabilityPoolIdValue = fixed32(liabilityPoolId, "liabilityPoolId")
    private val requestIdValue = fixed32(requestId, "requestId")

    init {
        requireHeader(version, networkId, scale, null)
        require(hardwareCredential.version == version && hardwareCredential.networkId == networkId)
        require(issuedAtMs >= 0 && expiresAtMs > issuedAtMs)
        require(expiresAtMs - issuedAtMs <= OfflineCashWireV1.REQUEST_MAX_TTL_MS)
    }

    fun releaseId(): ByteArray = releaseIdValue.copyOf()
    fun liabilityPoolId(): ByteArray = liabilityPoolIdValue.copyOf()
    fun requestId(): ByteArray = requestIdValue.copyOf()
}

/** Unlinkable public send statement with no public predecessor or successor. */
class OfflineCashTransferStatementV1(
    @JvmField val version: Int,
    @JvmField val lifecycle: OfflineCashLifecycleBindingV1,
    @JvmField val amount: BigInteger,
    transitionNullifier: ByteArray,
    requestDigest: ByteArray,
    acceptanceTicketDigest: ByteArray,
    @JvmField val recipientOneTimeKey: OfflineCashX25519PublicKeyV1,
    ciphertextCommitment: ByteArray,
    @JvmField val commitEvidence: OfflineCashCommitEvidenceV1,
) {
    private val transitionNullifierValue = fixed32(transitionNullifier, "transitionNullifier")
    private val requestDigestValue = fixed32(requestDigest, "requestDigest")
    private val acceptanceTicketDigestValue = fixed32(acceptanceTicketDigest, "acceptanceTicketDigest")
    private val ciphertextCommitmentValue = fixed32(ciphertextCommitment, "ciphertextCommitment")

    init {
        require(version == 1 && lifecycle.version == version)
        require(lifecycle.operationKind == OfflineCashOperationKindV1.SEND_SPLIT)
        requirePositiveU128(amount, "amount")
    }

    fun transitionNullifier(): ByteArray = transitionNullifierValue.copyOf()
    fun requestDigest(): ByteArray = requestDigestValue.copyOf()
    fun acceptanceTicketDigest(): ByteArray = acceptanceTicketDigestValue.copyOf()
    fun ciphertextCommitment(): ByteArray = ciphertextCommitmentValue.copyOf()
}

/** Sender response with the recoverable terminal certificate and final wrapper proof. */
class OfflineCashPaymentV1(
    @JvmField val version: Int,
    @JvmField val statement: OfflineCashTransferStatementV1,
    @JvmField val acceptanceIntent: OfflineCashAcceptanceIntentV1,
    @JvmField val acceptanceTicket: OfflineCashAcceptanceTicketV1,
    @JvmField val commitCertificate: OfflineCashCommitCertificateV1,
    @JvmField val proof: OfflineCashCommitWrapperProofV1,
    encryptedCredit: ByteArray,
    artifactManifestDigest: ByteArray,
) {
    private val encryptedCreditValue = boundedEncryptedCredit(encryptedCredit)
    private val artifactManifestDigestValue = fixed32(artifactManifestDigest, "artifactManifestDigest")

    init {
        require(version == 1 && statement.version == version && acceptanceIntent.version == version)
        require(acceptanceTicket.version == version && commitCertificate.version == version && proof.version == version)
    }

    fun encryptedCredit(): ByteArray = encryptedCreditValue.copyOf()
    fun artifactManifestDigest(): ByteArray = artifactManifestDigestValue.copyOf()
}

/** Durable secure-inbox receipt; it is not a receiver balance head. */
class OfflineCashInboxReceiptV1(
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
class OfflineCashAcknowledgementV1(
    @JvmField val version: Int,
    requestDigest: ByteArray,
    paymentDigest: ByteArray,
    @JvmField val inboxReceipt: OfflineCashInboxReceiptV1,
    @JvmField val signature: OfflineCashDeviceSignatureV1,
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
class OfflineCashMintAuthorizationContextV1(
    @JvmField val version: Int,
    operationId: ByteArray,
    releaseId: ByteArray,
    suiteId: ByteArray,
    vkDigest: ByteArray,
    artifactManifestDigest: ByteArray,
    @JvmField val networkId: NetworkId,
    @JvmField val asset: OfflineCashAssetDefinitionIdV1,
    @JvmField val assetIncarnation: OfflineCashAssetIncarnationV1,
    @JvmField val scale: Int,
    liabilityPoolId: ByteArray,
    @JvmField val amount: BigInteger,
    @JvmField val payer: OfflineCashAccountIdV1,
    @JvmField val recipient: OfflineCashAccountIdV1,
    hardwareCredentialId: ByteArray,
    hardwareProfileId: ByteArray,
    @JvmField val policyEpoch: Long,
    recipientCredentialCommitment: ByteArray,
    creditCommitment: ByteArray,
    @JvmField val recipientOneTimeKey: OfflineCashX25519PublicKeyV1,
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
        require(policyEpoch > 0)
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
class OfflineCashMintAuthorizationStatementV1(
    @JvmField val version: Int,
    @JvmField val context: OfflineCashMintAuthorizationContextV1,
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
class OfflineCashMintAuthorizationV1(
    @JvmField val version: Int,
    @JvmField val statement: OfflineCashMintAuthorizationStatementV1,
    @JvmField val proof: OfflineCashPairedProofV1,
) {
    init {
        require(version == 1 && statement.version == version && proof.version == version)
    }
}

/** Public reserve-backed foldable mint-credit statement. */
class OfflineCashMintCreditStatementV1(
    @JvmField val version: Int,
    @JvmField val lifecycle: OfflineCashLifecycleBindingV1,
    recipientCredentialCommitment: ByteArray,
    authorizationContextDigest: ByteArray,
    mintAuthorizationDigest: ByteArray,
    @JvmField val amount: BigInteger,
    issuanceCommitment: ByteArray,
    @JvmField val recipient: OfflineCashAccountIdV1,
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
        require(lifecycle.operationKind == OfflineCashOperationKindV1.MINT_FOLD)
        requirePositiveU128(amount, "amount")
        require(mintedAtMs > 0)
    }

    fun recipientCredentialCommitment(): ByteArray = recipientCredentialCommitmentValue.copyOf()
    fun authorizationContextDigest(): ByteArray = authorizationContextDigestValue.copyOf()
    fun mintAuthorizationDigest(): ByteArray = mintAuthorizationDigestValue.copyOf()
    fun issuanceCommitment(): ByteArray = issuanceCommitmentValue.copyOf()
    fun creditCommitment(): ByteArray = creditCommitmentValue.copyOf()
}

/** Finalized constant-size authenticated top-up credit. */
class OfflineCashMintCreditV1(
    @JvmField val version: Int,
    @JvmField val statement: OfflineCashMintCreditStatementV1,
    @JvmField val proof: OfflineCashPairedProofV1,
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
class OfflineCashRedemptionStatementV1(
    @JvmField val version: Int,
    @JvmField val lifecycle: OfflineCashLifecycleBindingV1,
    @JvmField val amount: BigInteger,
    @JvmField val beneficiary: OfflineCashAccountIdV1,
    terminalNullifier: ByteArray,
    redemptionCommitment: ByteArray,
    redemptionId: ByteArray,
    @JvmField val commitEvidence: OfflineCashCommitEvidenceV1,
) {
    private val terminalNullifierValue = fixed32(terminalNullifier, "terminalNullifier")
    private val redemptionCommitmentValue = fixed32(redemptionCommitment, "redemptionCommitment")
    private val redemptionIdValue = fixed32(redemptionId, "redemptionId")

    init {
        require(version == 1 && lifecycle.version == version)
        require(lifecycle.operationKind == OfflineCashOperationKindV1.REDEEM_SPLIT)
        requirePositiveU128(amount, "amount")
    }

    fun terminalNullifier(): ByteArray = terminalNullifierValue.copyOf()
    fun redemptionCommitment(): ByteArray = redemptionCommitmentValue.copyOf()
    fun redemptionId(): ByteArray = redemptionIdValue.copyOf()
}

/** Constant-size terminal voucher submitted for online redemption. */
class OfflineCashRedemptionVoucherV1(
    @JvmField val version: Int,
    @JvmField val statement: OfflineCashRedemptionStatementV1,
    @JvmField val commitCertificate: OfflineCashCommitCertificateV1,
    @JvmField val proof: OfflineCashCommitWrapperProofV1,
    artifactManifestDigest: ByteArray,
) {
    private val artifactManifestDigestValue = fixed32(artifactManifestDigest, "artifactManifestDigest")

    init {
        require(version == 1 && statement.version == version)
        require(commitCertificate.version == version && proof.version == version)
    }

    fun artifactManifestDigest(): ByteArray = artifactManifestDigestValue.copyOf()
}

internal val OFFLINE_CASH_UINT128_MAX: BigInteger = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE)

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
    require(value.signum() >= 0 && value <= OFFLINE_CASH_UINT128_MAX) {
        "$field must fit unsigned 128-bit arithmetic"
    }
}

internal fun requirePositiveU128(value: BigInteger, field: String) {
    requireUnsigned128(value, field)
    require(value.signum() > 0) { "$field must be positive" }
}

private fun requireHeader(version: Int, networkId: NetworkId, scale: Int, amount: BigInteger?) {
    require(version == OfflineCashWireV1.WIRE_VERSION) { "Offline Cash wire version must be 1" }
    require(networkId.bytes().any { it != 0.toByte() }) { "networkId is zero" }
    require(scale in 0..OfflineCashWireV1.MAXIMUM_ASSET_SCALE) { "asset scale is out of range" }
    amount?.let { requirePositiveU128(it, "amount") }
}

private fun boundedProof(bytes: ByteArray, field: String): ByteArray {
    val copy = bytes.copyOf()
    require(copy.isNotEmpty() && copy.size <= OfflineCashWireV1.MAXIMUM_PARITY_PROOF_BYTES) {
        "$field is empty or oversized"
    }
    return copy
}

private fun exactHistory(bytes: ByteArray, field: String): ByteArray {
    val copy = exactBytes(bytes, OfflineCashWireV1.HISTORY_ACCUMULATOR_BYTES, field)
    require(copy.any { it != 0.toByte() }) { "$field is zero" }
    return copy
}

private fun boundedEncryptedCredit(bytes: ByteArray): ByteArray {
    val copy = bytes.copyOf()
    require(copy.isNotEmpty() && copy.size <= OfflineCashWireV1.MAXIMUM_ENCRYPTED_CREDIT_BYTES) {
        "encryptedCredit is empty or oversized"
    }
    return copy
}

private fun requireX25519PublicKey(bytes: ByteArray): ByteArray {
    val copy = exactBytes(bytes, OfflineCashWireV1.X25519_PUBLIC_KEY_BYTES, "X25519 public key")
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
