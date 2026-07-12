package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.tx.SignedTransaction
import org.hyperledger.iroha.sdk.tx.TransactionBuilder

/**
 * Canonical request that registers one finalized Offline device attestation on chain.
 *
 * Platform attestation must already have completed and populated [registration]. The
 * transaction authority signs the resulting instruction; the assertion key remains
 * bound inside the attestation registration and is not used as the account signer.
 */
class RegisterOfflineDeviceAttestation(
    val chainId: String,
    val authority: String,
    val registration: AttestedOfflineNote.DeviceAttestationRegistration,
    val creationTimeMs: Long,
    val timeToLiveMs: Long? = null,
    val nonce: Int? = null,
    metadata: Map<String, JsonValue> = emptyMap(),
) {
    private val metadataSnapshot: Map<String, JsonValue> = metadata.toMap()

    init {
        require(chainId.isNotEmpty() && chainId == chainId.trim()) {
            "chainId must be exact non-empty text"
        }
        require(authority.isNotEmpty() && authority == authority.trim()) {
            "authority must be exact non-empty text"
        }
        require(creationTimeMs >= 0) { "creationTimeMs must be non-negative" }
        if (timeToLiveMs != null) require(timeToLiveMs > 0) {
            "timeToLiveMs must be positive when present"
        }
        if (nonce != null) require(nonce > 0) { "nonce must be positive when present" }
    }

    /** Exact native instruction carried by this transaction. */
    fun instruction(): InstructionBox =
        AttestedOfflineNote.registerDeviceAttestationInstruction(registration)

    /** Build the canonical transaction payload without exporting account key material. */
    fun transactionPayload(): TransactionPayload = TransactionPayload(
        chainId = chainId,
        authority = authority,
        creationTimeMs = creationTimeMs,
        executable = Executable.instructions(listOf(instruction())),
        timeToLiveMs = timeToLiveMs,
        nonce = nonce,
        metadata = metadataSnapshot,
    )

    /** Encode and sign with the SDK's canonical transaction builder. */
    fun encodeAndSign(builder: TransactionBuilder, signer: Signer): SignedTransaction =
        builder.encodeAndSign(transactionPayload(), signer)
}
