package org.hyperledger.iroha.sdk.alias

import java.util.concurrent.CompletableFuture
import org.hyperledger.iroha.sdk.client.ClientResponse
import org.hyperledger.iroha.sdk.client.IrohaClient
import org.hyperledger.iroha.sdk.core.model.Executable
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.core.model.JsonValue
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.model.TransactionPayload
import org.hyperledger.iroha.sdk.crypto.Signer
import org.hyperledger.iroha.sdk.tx.TransactionBuilder

/** Encodes a lifecycle plan body to the exact canonical Norito bytes committed by its hash. */
fun interface AliasLifecyclePlanBodyNoritoEncoder {
    /** Returns canonical Norito bytes for the supplied lifecycle body. */
    fun encode(body: AliasLifecycleTransactionPlanBodyV1): ByteArray
}

/** Safe local handoff from a verified lifecycle plan to the public transaction pipeline. */
object AliasLifecyclePlanApply {
    /** Builds a lifecycle transaction using the repository's canonical V1 alias codecs. */
    @JvmStatic
    @JvmOverloads
    fun buildTransactionPayload(
        request: AliasLifecyclePlanRequestV1,
        plan: AliasLifecycleTransactionPlanV1,
        networkId: NetworkId,
        chainDiscriminant: Int,
        feePayment: FeePaymentIntent,
        creationTimeMs: Long = System.currentTimeMillis(),
        nonce: Long? = null,
        metadata: Map<String, JsonValue> = emptyMap(),
    ): TransactionPayload = buildTransactionPayload(
        request,
        plan,
        DefaultAliasLifecyclePlanBodyNoritoEncoder,
        DefaultAliasLifecycleInstructionFrameCodec,
        networkId,
        chainDiscriminant,
        feePayment,
        creationTimeMs,
        nonce,
        metadata,
    )

    /** Builds one transaction containing the exact planner lifecycle frame. */
    @JvmStatic
    @JvmOverloads
    fun buildTransactionPayload(
        request: AliasLifecyclePlanRequestV1,
        plan: AliasLifecycleTransactionPlanV1,
        bodyEncoder: AliasLifecyclePlanBodyNoritoEncoder,
        frameCodec: AliasLifecycleInstructionFrameCodec,
        networkId: NetworkId,
        chainDiscriminant: Int,
        feePayment: FeePaymentIntent,
        creationTimeMs: Long = System.currentTimeMillis(),
        nonce: Long? = null,
        metadata: Map<String, JsonValue> = emptyMap(),
    ): TransactionPayload {
        require(creationTimeMs >= 0) { "creationTimeMs must not be negative" }
        require(plan.body.networkId == networkId) {
            "alias lifecycle plan NetworkId does not match the trusted transaction network"
        }
        require(plan.body.validUntilMs > creationTimeMs) { "alias lifecycle plan has expired" }
        require(plan.body.disposition == AliasLifecyclePlanDispositionV1.APPLY) {
            "alias lifecycle plan is an exact no-op and must not be submitted"
        }
        val bodyBytes = bodyEncoder.encode(plan.body)
        require(bodyBytes.isNotEmpty()) { "canonical alias lifecycle plan body must not be empty" }
        AliasPlanVerifier.requireLifecycleExecutableForRequest(
            request,
            plan,
            bodyBytes,
            frameCodec,
            chainDiscriminant,
        )
        val frame = requireNotNull(plan.body.instruction) {
            "executable alias lifecycle plan is missing its instruction"
        }
        return TransactionPayload(
            networkId = networkId,
            authority = plan.body.authority,
            creationTimeMs = creationTimeMs,
            executable = Executable.instructions(
                listOf(InstructionBox.fromWirePayload(frame.wireId, frame.framedPayload)),
            ),
            timeToLiveMs = plan.body.validUntilMs - creationTimeMs,
            nonce = nonce,
            feePayment = feePayment,
            metadata = metadata,
        )
    }

    /** Locally signs a verified lifecycle plan and submits it through the public endpoint. */
    @JvmStatic
    @JvmOverloads
    fun signAndSubmit(
        client: IrohaClient,
        request: AliasLifecyclePlanRequestV1,
        plan: AliasLifecycleTransactionPlanV1,
        networkId: NetworkId,
        bodyEncoder: AliasLifecyclePlanBodyNoritoEncoder,
        frameCodec: AliasLifecycleInstructionFrameCodec,
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

    /** Verifies with canonical V1 codecs, signs locally, and submits normally. */
    @JvmStatic
    @JvmOverloads
    fun signAndSubmit(
        client: IrohaClient,
        request: AliasLifecyclePlanRequestV1,
        plan: AliasLifecycleTransactionPlanV1,
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
        DefaultAliasLifecyclePlanBodyNoritoEncoder,
        DefaultAliasLifecycleInstructionFrameCodec,
        chainDiscriminant,
        transactionBuilder,
        signer,
        feePayment,
        creationTimeMs,
        nonce,
        metadata,
    )
}
