package org.hyperledger.iroha.android.alias;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.IrohaClient;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.tx.TransactionBuilder;

/** Safe local handoff from a verified alias plan to the ordinary transaction pipeline. */
public final class AliasPlanApply {
  private AliasPlanApply() {}

  /** Builds a transaction using the repository's canonical V1 alias codecs. */
  public static TransactionPayload buildTransactionPayload(
      final AliasSetupPlanRequestV1 request,
      final AliasTransactionPlanV1 plan,
      final FeePaymentIntent feePayment,
      final long creationTimeMs,
      final Long nonce,
      final Map<String, JsonValue> metadata) {
    return buildTransactionPayload(
        request,
        plan,
        DefaultAliasPlanBodyNoritoEncoder.INSTANCE,
        DefaultAliasEnsureInstructionFrameCodec.INSTANCE,
        feePayment,
        creationTimeMs,
        nonce,
        metadata);
  }

  /** Builds one ordinary transaction containing every exact planner frame. */
  public static TransactionPayload buildTransactionPayload(
      final AliasSetupPlanRequestV1 request,
      final AliasTransactionPlanV1 plan,
      final AliasPlanBodyNoritoEncoder bodyEncoder,
      final AliasEnsureInstructionFrameCodec frameCodec,
      final FeePaymentIntent feePayment,
      final long creationTimeMs,
      final Long nonce,
      final Map<String, JsonValue> metadata) {
    if (request == null
        || plan == null
        || bodyEncoder == null
        || frameCodec == null
        || feePayment == null) {
      throw new IllegalArgumentException("alias plan apply arguments must not be null");
    }
    if (creationTimeMs < 0) throw new IllegalArgumentException("creationTimeMs must not be negative");
    if (plan.body().validUntilMs() <= creationTimeMs) {
      throw new IllegalArgumentException("alias setup plan has expired");
    }
    final byte[] bodyBytes = bodyEncoder.encode(plan.body());
    if (bodyBytes == null || bodyBytes.length == 0) {
      throw new IllegalArgumentException("canonical alias plan body must not be empty");
    }
    AliasPlanVerifier.requireExecutableForRequest(
        request, plan, bodyBytes, frameCodec);
    final List<InstructionBox> instructions = new ArrayList<>();
    for (final AliasSetupModels.AliasFramedInstructionV1 frame : plan.body().instructions()) {
      instructions.add(InstructionBox.fromWirePayload(frame.wireId(), frame.framedPayload()));
    }
    return TransactionPayload.builder()
        .setChainId(plan.body().chainId())
        .setAuthority(plan.body().authority())
        .setCreationTimeMs(creationTimeMs)
        .setInstructions(instructions)
        .setTimeToLiveMs(plan.body().validUntilMs() - creationTimeMs)
        .setNonce(nonce)
        .setFeePayment(feePayment)
        .setMetadata(metadata == null ? Collections.emptyMap() : metadata)
        .build();
  }

  /** Locally signs a verified plan and submits it through the normal transaction endpoint. */
  public static CompletableFuture<ClientResponse> signAndSubmit(
      final IrohaClient client,
      final AliasSetupPlanRequestV1 request,
      final AliasTransactionPlanV1 plan,
      final AliasPlanBodyNoritoEncoder bodyEncoder,
      final AliasEnsureInstructionFrameCodec frameCodec,
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
            feePayment,
            creationTimeMs,
            nonce,
            metadata);
    return client.submitTransaction(transactionBuilder.encodeAndSign(payload, signer));
  }

  /** Verifies with canonical V1 codecs, signs locally, and submits normally. */
  public static CompletableFuture<ClientResponse> signAndSubmit(
      final IrohaClient client,
      final AliasSetupPlanRequestV1 request,
      final AliasTransactionPlanV1 plan,
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
        DefaultAliasPlanBodyNoritoEncoder.INSTANCE,
        DefaultAliasEnsureInstructionFrameCodec.INSTANCE,
        transactionBuilder,
        signer,
        feePayment,
        creationTimeMs,
        nonce,
        metadata);
  }
}
