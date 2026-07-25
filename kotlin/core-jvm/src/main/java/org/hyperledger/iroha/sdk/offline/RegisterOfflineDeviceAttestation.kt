// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.lang.Math.addExact
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.TransactionBuilder

/** Canonical one-instruction transaction for the ABI-21 device-attestation path. */
class RegisterOfflineDeviceAttestation(
    val chainId: String,
    val authority: String,
    val registration: DeviceAttestationRegistration,
    val creationTimeMs: Long,
    val timeToLiveMs: Long? = null,
    val nonce: Long? = null,
    val feePayment: FeePaymentIntent,
    metadata: Map<String, JsonValue> = emptyMap(),
) {
    private val metadataSnapshot = metadata.toMap()

    init {
        require(chainId.isNotEmpty() && chainId == chainId.trim()) {
            "chainId must be exact non-empty text"
        }
        require(authority.isNotEmpty() && authority == authority.trim()) {
            "authority must be exact non-empty text"
        }
        require(creationTimeMs >= 0) { "creationTimeMs must be non-negative" }
        if (timeToLiveMs != null) {
            require(timeToLiveMs > 0) { "timeToLiveMs must be positive when present" }
            val validUntil = try {
                addExact(creationTimeMs, timeToLiveMs)
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
        chainId = chainId,
        authority = authority,
        creationTimeMs = creationTimeMs,
        executable = Executable.instructions(listOf(instruction())),
        timeToLiveMs = timeToLiveMs,
        nonce = nonce,
        feePayment = feePayment,
        metadata = metadataSnapshot,
    )

    /** Reject a payload changed after construction, including an added instruction. */
    fun validateExactPayload(payload: TransactionPayload) {
        val expected = transactionPayload()
        val instructions = payload.executable as? Executable.Instructions
        require(
            payload.chainId == expected.chainId &&
                payload.authority == expected.authority &&
                payload.creationTimeMs == expected.creationTimeMs &&
                payload.timeToLiveMs == expected.timeToLiveMs &&
                payload.nonce == expected.nonce &&
                payload.feePayment == expected.feePayment &&
                payload.metadata == expected.metadata &&
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
