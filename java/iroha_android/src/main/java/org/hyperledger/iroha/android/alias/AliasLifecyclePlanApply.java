package org.hyperledger.iroha.android.alias;

import java.util.Collections;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.IrohaClient;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.tx.TransactionBuilder;

/** Safe local handoff from a verified lifecycle plan to the ordinary transaction pipeline. */
public final class AliasLifecyclePlanApply {
  private AliasLifecyclePlanApply() {}

  /** Builds a lifecycle transaction using the repository's canonical V1 alias codecs. */
  public static TransactionPayload buildTransactionPayload(
      final AliasLifecyclePlanRequestV1 request,
      final AliasLifecycleTransactionPlanV1 plan,
      final NetworkId networkId,
      final int chainDiscriminant,
      final FeePaymentIntent feePayment,
      final long creationTimeMs,
      final Long nonce,
      final Map<String, JsonValue> metadata) {
    return buildTransactionPayload(
        request,
        plan,
        DefaultAliasLifecyclePlanBodyNoritoEncoder.INSTANCE,
        DefaultAliasLifecycleInstructionFrameCodec.INSTANCE,
        networkId,
        chainDiscriminant,
        feePayment,
        creationTimeMs,
        nonce,
        metadata);
  }

  /** Builds one ordinary transaction containing the exact planner lifecycle frame. */
  public static TransactionPayload buildTransactionPayload(
      final AliasLifecyclePlanRequestV1 request,
      final AliasLifecycleTransactionPlanV1 plan,
      final AliasLifecyclePlanBodyNoritoEncoder bodyEncoder,
      final AliasLifecycleInstructionFrameCodec frameCodec,
      final NetworkId networkId,
      final int chainDiscriminant,
      final FeePaymentIntent feePayment,
      final long creationTimeMs,
      final Long nonce,
      final Map<String, JsonValue> metadata) {
    if (request == null
        || plan == null
        || bodyEncoder == null
        || frameCodec == null
        || networkId == null
        || feePayment == null) {
      throw new IllegalArgumentException("alias lifecycle apply arguments must not be null");
    }
    if (creationTimeMs < 0) throw new IllegalArgumentException("creationTimeMs must not be negative");
    if (!plan.body().networkId().equals(networkId)) {
      throw new IllegalArgumentException(
          "alias lifecycle plan NetworkId does not match the trusted transaction network");
    }
    if (plan.body().validUntilMs() <= creationTimeMs) {
      throw new IllegalArgumentException("alias lifecycle plan has expired");
    }
    if (plan.body().disposition() != AliasLifecyclePlanDispositionV1.APPLY) {
      throw new IllegalArgumentException(
          "alias lifecycle plan is an exact no-op and must not be submitted");
    }
    final byte[] bodyBytes = bodyEncoder.encode(plan.body());
    if (bodyBytes == null || bodyBytes.length == 0) {
      throw new IllegalArgumentException("canonical alias lifecycle plan body must not be empty");
    }
    AliasPlanVerifier.requireLifecycleExecutableForRequest(
        request, plan, bodyBytes, frameCodec, chainDiscriminant);
    final AliasSetupModels.AliasFramedInstructionV1 frame = plan.body().instruction();
    if (frame == null) {
      throw new IllegalArgumentException("executable alias lifecycle plan is missing its instruction");
    }
    return TransactionPayload.builder()
        .setNetworkId(networkId)
        .setAuthority(plan.body().authority())
        .setCreationTimeMs(creationTimeMs)
        .setInstructions(
            Collections.singletonList(
                InstructionBox.fromWirePayload(frame.wireId(), frame.framedPayload())))
        .setTimeToLiveMs(plan.body().validUntilMs() - creationTimeMs)
        .setNonce(nonce)
        .setFeePayment(feePayment)
        .setMetadata(metadata == null ? Collections.emptyMap() : metadata)
        .build();
  }

  /** Locally signs a verified lifecycle plan and submits it through the normal endpoint. */
  public static CompletableFuture<ClientResponse> signAndSubmit(
      final IrohaClient client,
      final AliasLifecyclePlanRequestV1 request,
      final AliasLifecycleTransactionPlanV1 plan,
      final NetworkId networkId,
      final AliasLifecyclePlanBodyNoritoEncoder bodyEncoder,
      final AliasLifecycleInstructionFrameCodec frameCodec,
      final int chainDiscriminant,
      final TransactionBuilder transactionBuilder,
      final Signer signer,
      final FeePaymentIntent feePayment,
      final long creationTimeMs,
      final Long nonce,
      final Map<String, JsonValue> metadata)
      throws NoritoException, SigningException {
    if (client == null || transactionBuilder == null || signer == null) {
      throw new IllegalArgumentException("client, transactionBuilder, and signer must not be null");
    }
    final TransactionPayload payload =
        buildTransactionPayload(
            request,
            plan,
            bodyEncoder,
            frameCodec,
            networkId,
            chainDiscriminant,
            feePayment,
            creationTimeMs,
            nonce,
            metadata);
    return client.submitTransaction(transactionBuilder.encodeAndSign(payload, signer));
  }

  /** Verifies with canonical V1 codecs, signs locally, and submits normally. */
  public static CompletableFuture<ClientResponse> signAndSubmit(
      final IrohaClient client,
      final AliasLifecyclePlanRequestV1 request,
      final AliasLifecycleTransactionPlanV1 plan,
      final NetworkId networkId,
      final int chainDiscriminant,
      final TransactionBuilder transactionBuilder,
      final Signer signer,
      final FeePaymentIntent feePayment,
      final long creationTimeMs,
      final Long nonce,
      final Map<String, JsonValue> metadata)
      throws NoritoException, SigningException {
    return signAndSubmit(
        client,
        request,
        plan,
        networkId,
        DefaultAliasLifecyclePlanBodyNoritoEncoder.INSTANCE,
        DefaultAliasLifecycleInstructionFrameCodec.INSTANCE,
        chainDiscriminant,
        transactionBuilder,
        signer,
        feePayment,
        creationTimeMs,
        nonce,
        metadata);
  }
}
