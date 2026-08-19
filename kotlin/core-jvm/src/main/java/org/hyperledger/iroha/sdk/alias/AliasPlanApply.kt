package org.hyperledger.iroha.sdk.alias

import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.IrohaClient
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.TransactionAdmissionIntent
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.tx.TransactionBuilder

/** Encodes a typed plan body to the exact canonical Norito bytes committed by `plan_hash`. */
fun interface AliasPlanBodyNoritoEncoder {
    /** Returns canonical Norito bytes for the supplied body. */
    fun encode(body: AliasTransactionPlanBodyV1): ByteArray
}

/** Safe local handoff from a verified alias plan to the public transaction pipeline. */
object AliasPlanApply {
    /** Builds a transaction using the repository's canonical V1 alias codecs. */
    @JvmStatic
    @JvmOverloads
    fun buildTransactionPayload(
        request: AliasSetupPlanRequestV1,
        plan: AliasTransactionPlanV1,
        networkId: NetworkId,
        chainDiscriminant: Int,
        feePayment: FeePaymentIntent,
        creationTimeMs: Long = System.currentTimeMillis(),
        nonce: Long? = null,
        metadata: Map<String, JsonValue> = emptyMap(),
    ): TransactionPayload = buildTransactionPayload(
        request,
        plan,
        DefaultAliasPlanBodyNoritoEncoder,
        DefaultAliasEnsureInstructionFrameCodec,
        networkId,
        chainDiscriminant,
        feePayment,
        creationTimeMs,
        nonce,
        metadata,
    )

    /**
     * Builds one transaction containing every exact planner frame.
     *
     * No alias mutation endpoint is involved. The caller supplies only generic transaction fields;
     * authority and exact network are pinned by the signed planner response. The caller's trusted
     * genesis context must match that committed `NetworkId` before any transaction is built.
     */
    @JvmStatic
    @JvmOverloads
    fun buildTransactionPayload(
        request: AliasSetupPlanRequestV1,
        plan: AliasTransactionPlanV1,
        bodyEncoder: AliasPlanBodyNoritoEncoder,
        frameCodec: AliasEnsureInstructionFrameCodec,
        networkId: NetworkId,
        chainDiscriminant: Int,
        feePayment: FeePaymentIntent,
        creationTimeMs: Long = System.currentTimeMillis(),
        nonce: Long? = null,
        metadata: Map<String, JsonValue> = emptyMap(),
    ): TransactionPayload {
        require(creationTimeMs >= 0) { "creationTimeMs must not be negative" }
        require(plan.body.networkId == networkId) {
            "alias setup plan NetworkId does not match the trusted transaction network"
        }
        require(plan.body.validUntilMs > creationTimeMs) { "alias setup plan has expired" }
        val bodyBytes = bodyEncoder.encode(plan.body)
        require(bodyBytes.isNotEmpty()) { "canonical alias plan body must not be empty" }
        AliasPlanVerifier.requireExecutableForRequest(
            request,
            plan,
            bodyBytes,
            frameCodec,
            chainDiscriminant,
        )
        val instructions = plan.body.instructions.map { frame ->
            InstructionBox.fromWirePayload(frame.wireId, frame.framedPayload)
        }
        return TransactionPayload(
            networkId = networkId,
            authority = plan.body.authority,
            creationTimeMs = creationTimeMs,
            executable = Executable.instructions(instructions),
            timeToLiveMs = plan.body.validUntilMs - creationTimeMs,
            nonce = nonce,
            feePayment = feePayment,
            admissionIntent = TransactionAdmissionIntent.QUEUE_PLAN_SYNCED,
            metadata = metadata,
        )
    }

    /** Locally signs a verified plan and submits it through the public transaction endpoint. */
    @JvmStatic
    @JvmOverloads
    fun signAndSubmit(
        client: IrohaClient,
        request: AliasSetupPlanRequestV1,
        plan: AliasTransactionPlanV1,
        networkId: NetworkId,
        bodyEncoder: AliasPlanBodyNoritoEncoder,
        frameCodec: AliasEnsureInstructionFrameCodec,
        chainDiscriminant: Int,
        transactionBuilder: TransactionBuilder,
        signer: Signer,
        feePayment: FeePaymentIntent,
        creationTimeMs: Long = System.currentTimeMillis(),
        nonce: Long? = null,
        metadata: Map<String, JsonValue> = emptyMap(),
    ): CompletableFuture<ClientResponse> {
        val payload = buildTransactionPayload(
            request,
            plan,
            bodyEncoder,
            frameCodec,
            networkId,
            chainDiscriminant,
            feePayment,
            creationTimeMs,
            nonce,
            metadata,
        )
        return client.submitTransaction(transactionBuilder.encodeAndSign(payload, signer))
    }

    /** Verifies with the canonical V1 codecs, signs locally, and submits normally. */
    @JvmStatic
    @JvmOverloads
    fun signAndSubmit(
        client: IrohaClient,
        request: AliasSetupPlanRequestV1,
        plan: AliasTransactionPlanV1,
        networkId: NetworkId,
        chainDiscriminant: Int,
        transactionBuilder: TransactionBuilder,
        signer: Signer,
        feePayment: FeePaymentIntent,
        creationTimeMs: Long = System.currentTimeMillis(),
        nonce: Long? = null,
        metadata: Map<String, JsonValue> = emptyMap(),
    ): CompletableFuture<ClientResponse> = signAndSubmit(
        client,
        request,
        plan,
        networkId,
        DefaultAliasPlanBodyNoritoEncoder,
        DefaultAliasEnsureInstructionFrameCodec,
        chainDiscriminant,
        transactionBuilder,
        signer,
        feePayment,
        creationTimeMs,
        nonce,
        metadata,
    )
}
