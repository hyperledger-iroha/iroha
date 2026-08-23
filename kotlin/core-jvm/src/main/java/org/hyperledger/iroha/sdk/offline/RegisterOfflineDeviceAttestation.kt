// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.lang.Math.addExact
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.TransactionAdmissionIntent
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.TransactionBuilder

private const val DEFAULT_REGISTRATION_TRANSACTION_TTL_MS = 100_000L

/** Canonical one-instruction transaction for the ABI-21 device-attestation path. */
class RegisterOfflineDeviceAttestation(
    val networkId: NetworkId,
    val authority: String,
    val registration: DeviceAttestationRegistration,
    val creationTimeMs: Long,
    timeToLiveMs: Long? = null,
    val nonce: Long? = null,
    val feePayment: FeePaymentIntent,
    metadata: Map<String, JsonValue> = emptyMap(),
) {
    val timeToLiveMs: Long? = timeToLiveMs ?: DEFAULT_REGISTRATION_TRANSACTION_TTL_MS
    private val metadataSnapshot = metadata.toMap()

    init {
        require(authority.isNotEmpty() && authority == authority.trim()) {
            "authority must be exact non-empty text"
        }
        require(creationTimeMs >= 0) { "creationTimeMs must be non-negative" }
        requireNotNull(this.timeToLiveMs).let { effectiveTimeToLiveMs ->
            require(effectiveTimeToLiveMs > 0) { "timeToLiveMs must be positive when present" }
            val validUntil = try {
                addExact(creationTimeMs, effectiveTimeToLiveMs)
            } catch (ex: ArithmeticException) {
                throw IllegalArgumentException("transaction lifetime overflows milliseconds", ex)
            }
            require(validUntil <= registration.expiresAtMs) {
                "transaction lifetime must not outlive the device attestation"
            }
        }
        // Reuse the canonical payload model's I105 and metadata validation immediately.
        transactionPayload()
    }

    /** Exact native instruction carried by this transaction. */
    fun instruction(): InstructionBox = OfflineDeviceAttestationCodec.instruction(registration)

    /** Build a payload containing exactly one registration instruction. */
    fun transactionPayload(): TransactionPayload = TransactionPayload(
        networkId = networkId,
        authority = authority,
        creationTimeMs = creationTimeMs,
        executable = Executable.instructions(listOf(instruction())),
        timeToLiveMs = timeToLiveMs,
        nonce = nonce,
        feePayment = feePayment,
        admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
        metadata = metadataSnapshot,
    )

    /** Reject a payload changed after construction, including an added instruction. */
    fun validateExactPayload(payload: TransactionPayload) {
        val expected = transactionPayload()
        val instructions = payload.executable as? Executable.Instructions
        require(
            payload.networkId == expected.networkId &&
                payload.authority == expected.authority &&
                payload.creationTimeMs == expected.creationTimeMs &&
                payload.timeToLiveMs == expected.timeToLiveMs &&
                payload.nonce == expected.nonce &&
                payload.feePayment == expected.feePayment &&
                payload.admissionIntent == expected.admissionIntent &&
                payload.metadata == expected.metadata &&
                payload.attachments == expected.attachments &&
                instructions != null &&
                instructions.instructions.size == 1 &&
                instructions.instructions[0] == instruction(),
        ) { "RegisterOfflineDeviceAttestation requires its exact one-instruction payload" }
    }

    /** Encode and sign with the canonical transaction builder. */
    fun encodeAndSign(builder: TransactionBuilder, signer: Signer): SignedTransaction {
        val payload = transactionPayload()
        validateExactPayload(payload)
        return builder.encodeAndSign(payload, signer)
    }

    companion object {
        /** Decode and validate one current instruction archive. */
        @JvmStatic
        fun decodeInstructionPayloadCanonical(
            archive: ByteArray,
            chainDiscriminant: Int,
        ): DeviceAttestationRegistration =
            OfflineDeviceAttestationCodec.decodeInstructionPayloadCanonical(
                archive,
                chainDiscriminant,
            )
    }
}
