package org.hyperledger.iroha.android.alias;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;

/** Fail-closed verification for onboarding prepare and exact-submit responses. */
public final class AccountOnboardingPreparedVerifier {
  private AccountOnboardingPreparedVerifier() {}

  /** Authenticates an exact prepared transaction for the receipt, binding, and expected fee intent. */
  public static SignedTransaction requireValidPrepared(
      final AccountOnboardingPreparedTransactionV1 prepared,
      final AccountOnboardingPlanRequestV1 request,
      final AccountOnboardingPlanReceiptV1 receipt,
      final TairaPublicResetMutationBindingV1 binding,
      final FeePaymentIntent expectedFeePayment,
      final NetworkId expectedNetworkId,
      final String expectedAuthority) {
    Objects.requireNonNull(prepared, "prepared");
    Objects.requireNonNull(expectedFeePayment, "expectedFeePayment");
    AccountOnboardingReceiptVerifier.requireValidForRequest(
        request, receipt, expectedNetworkId, expectedAuthority);
    if (!sameBinding(prepared.binding(), binding) || !sameReceipt(prepared.receipt(), receipt)) {
      throw new IllegalArgumentException(
          "prepared onboarding envelope differs from the exact receipt or binding");
    }
    if (!expectedFeePayment.hasSamePayerAndGasBound(prepared.feePayment())) {
      throw new IllegalArgumentException(
          "prepared onboarding fee intent changed payer, sponsor revision, or gas bound");
    }
    final byte[] receiptHash = AliasNameSupport.decodeHash(receipt.planHash());
    if (receiptHash == null
        || !prepared.semanticHashHex().equals(PreparedTransactionSignatureV1.hexLower(receiptHash))) {
      throw new IllegalArgumentException(
          "prepared onboarding semantic hash differs from the receipt");
    }
    if (!prepared.accountId().equals(receipt.body().request().accountId())
        || !prepared.alias().equals(receipt.body().request().alias())
        || !dispositionTransitionAllowed(
            receipt.body().resource().disposition(), prepared.disposition())) {
      throw new IllegalArgumentException(
          "prepared onboarding identity or disposition differs from the receipt");
    }
    if (!AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
        receipt.body().authority(),
        PreparedTransactionSignatureV1.digest(
            PreparedTransactionSignatureV1.onboardingPrepared(prepared)),
        prepared.serverSignature())) {
      throw new IllegalArgumentException("prepared onboarding server signature is invalid");
    }

    final byte[] wire =
        PreparedTransactionSignatureV1.decodeLowerHex(prepared.signedTransactionWireHex());
    if (!PreparedTransactionSignatureV1.hexLower(sha256(wire))
        .equals(prepared.signedTransactionWireSha256())) {
      throw new IllegalArgumentException(
          "prepared onboarding wire SHA-256 differs from the envelope");
    }
    final SignedTransaction transaction;
    try {
      transaction = SignedTransactionEncoder.decodeVersioned(wire);
      if (!java.util.Arrays.equals(SignedTransactionEncoder.encodeVersioned(transaction), wire)) {
        throw new IllegalArgumentException(
            "prepared onboarding wire is not canonical fixed-V1 SignedTransaction");
      }
    } catch (final NoritoException error) {
      throw new IllegalArgumentException("prepared onboarding wire is invalid", error);
    }
    if (!SignedTransactionHasher.hashHex(transaction).equals(prepared.transactionHashHex())) {
      throw new IllegalArgumentException(
          "prepared onboarding transaction hash differs from the envelope");
    }
    final TransactionPayload payload = SignedTransactionEncoder.decodeCanonicalPayload(transaction);
    if (!AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
        payload.authority(),
        IrohaHash.prehash(transaction.encodedPayload()),
        transaction.signature())) {
      throw new IllegalArgumentException("prepared onboarding transaction signature is invalid");
    }
    if (!payload.networkId().equals(expectedNetworkId)
        || !AccountOnboardingReceiptVerifier.sameAccountIdentity(
            payload.authority(), receipt.body().authority())) {
      throw new IllegalArgumentException(
          "prepared onboarding transaction network or authority was substituted");
    }
    if (!payload.feePayment().equals(prepared.feePayment())) {
      throw new IllegalArgumentException(
          "prepared onboarding fee intent differs from the signed transaction");
    }
    final Map<String, JsonValue> expectedMetadata = new LinkedHashMap<>();
    expectedMetadata.put(
        "taira_public_reset_binding",
        JsonValue.parse(JsonEncoder.encode(binding.toJsonMap())));
    expectedMetadata.put(
        "taira_prepared_operation", JsonValue.string(AccountOnboardingPreparedTransactionV1.OPERATION));
    expectedMetadata.put(
        "taira_prepared_semantic_hash", JsonValue.string(prepared.semanticHashHex()));
    if (!payload.metadata().equals(expectedMetadata)) {
      throw new IllegalArgumentException(
          "prepared onboarding transaction metadata differs from the envelope");
    }
    final Executable executable = payload.executable();
    if (!executable.isInstructions() || executable.instructions().isEmpty()) {
      throw new IllegalArgumentException(
          "prepared onboarding transaction must contain instructions");
    }
    final List<InstructionBox> planned = new ArrayList<>();
    for (final AliasSetupModels.AliasFramedInstructionV1 frame : receipt.body().instructions()) {
      planned.add(InstructionBox.fromWirePayload(frame.wireId(), frame.framedPayload()));
    }
    if (!orderedSubset(executable.instructions(), planned)) {
      throw new IllegalArgumentException(
          "prepared onboarding instructions are not an ordered subset of the signed receipt");
    }
    return transaction;
  }

  /** Authenticates a nonterminal result that still requires one fresh atomic observation. */
  public static AccountOnboardingProofRequiredPrepareResponseV1 requireValidProofRequired(
      final AccountOnboardingProofRequiredPrepareResponseV1 proofRequired,
      final AccountOnboardingPlanRequestV1 request,
      final AccountOnboardingPlanReceiptV1 receipt,
      final TairaPublicResetMutationBindingV1 binding,
      final NetworkId expectedNetworkId,
      final String expectedAuthority) {
    AccountOnboardingReceiptVerifier.requireValidForRequest(
        request, receipt, expectedNetworkId, expectedAuthority);
    final byte[] receiptHash = AliasNameSupport.decodeHash(receipt.planHash());
    if (receiptHash == null
        || !sameBinding(proofRequired.binding(), binding)
        || !AccountOnboardingProofRequiredPrepareResponseV1.OUTCOME.equals(
            proofRequired.outcome())
        || !AccountOnboardingProofRequiredPrepareResponseV1.PROOF_KIND.equals(
            proofRequired.proofKind())
        || !proofRequired.semanticHashHex().equals(
            PreparedTransactionSignatureV1.hexLower(receiptHash))
        || !proofRequired.accountId().equals(receipt.body().request().accountId())
        || !proofRequired.alias().equals(receipt.body().request().alias())
        || proofRequired.disposition() != AliasSetupModels.AliasPlanDispositionV1.NO_OP) {
      throw new IllegalArgumentException(
          "proof-required onboarding result differs from the exact receipt or binding");
    }
    if (!AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
        receipt.body().authority(),
        PreparedTransactionSignatureV1.digest(
            PreparedTransactionSignatureV1.onboardingProofRequired(proofRequired)),
        proofRequired.serverSignature())) {
      throw new IllegalArgumentException("proof-required onboarding server signature is invalid");
    }
    return proofRequired;
  }

  /** Reconciles only an independently fee-checked prepared envelope and its exact submit result. */
  public static PreparedTransactionSubmitResponseV1 requireValidSubmitResponse(
      final PreparedTransactionSubmitResponseV1 response,
      final AccountOnboardingPreparedTransactionV1 prepared,
      final FeePaymentIntent expectedFeePayment,
      final int httpStatus) {
    Objects.requireNonNull(expectedFeePayment, "expectedFeePayment");
    if (!expectedFeePayment.hasSamePayerAndGasBound(prepared.feePayment())) {
      throw new IllegalArgumentException(
          "prepared onboarding fee intent changed payer, sponsor revision, or gas bound");
    }
    if (httpStatus != 200 && httpStatus != 202) {
      throw new IllegalArgumentException(
          "prepared onboarding submit requires HTTP 200 or 202");
    }
    if (!sameBinding(response.binding(), prepared.binding())
        || !response.operation().equals(prepared.operation())
        || !response.transactionHashHex().equals(prepared.transactionHashHex())) {
      throw new IllegalArgumentException(
          "prepared onboarding submit response is not bound to the exact envelope");
    }
    if (httpStatus == 202 && response.outcome() != PreparedTransactionOutcomeV1.PENDING) {
      throw new IllegalArgumentException(
          "HTTP 202 prepared onboarding submit must remain Pending");
    }
    return response;
  }

  private static boolean dispositionTransitionAllowed(
      final AliasSetupModels.AliasPlanDispositionV1 planned,
      final AliasSetupModels.AliasPlanDispositionV1 live) {
    switch (planned) {
      case CREATE:
        return live == AliasSetupModels.AliasPlanDispositionV1.CREATE
            || live == AliasSetupModels.AliasPlanDispositionV1.REPAIR
            || live == AliasSetupModels.AliasPlanDispositionV1.NO_OP;
      case REPAIR:
        return live == AliasSetupModels.AliasPlanDispositionV1.REPAIR
            || live == AliasSetupModels.AliasPlanDispositionV1.NO_OP;
      case NO_OP:
        return live == AliasSetupModels.AliasPlanDispositionV1.NO_OP;
      case CONFLICT:
      default:
        return false;
    }
  }

  private static boolean orderedSubset(
      final List<InstructionBox> actual, final List<InstructionBox> planned) {
    int plannedIndex = 0;
    for (final InstructionBox instruction : actual) {
      while (plannedIndex < planned.size() && !planned.get(plannedIndex).equals(instruction)) {
        plannedIndex++;
      }
      if (plannedIndex == planned.size()) return false;
      plannedIndex++;
    }
    return true;
  }

  private static boolean sameBinding(
      final TairaPublicResetMutationBindingV1 left,
      final TairaPublicResetMutationBindingV1 right) {
    return left.toJsonMap().equals(right.toJsonMap());
  }

  private static boolean sameReceipt(
      final AccountOnboardingPlanReceiptV1 left,
      final AccountOnboardingPlanReceiptV1 right) {
    return left.toJsonMap().equals(right.toJsonMap());
  }

  private static byte[] sha256(final byte[] value) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(value);
    } catch (final NoSuchAlgorithmException impossible) {
      throw new IllegalStateException("SHA-256 is unavailable", impossible);
    }
  }
}
