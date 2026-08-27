package org.hyperledger.iroha.android.alias;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
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
import org.hyperledger.iroha.android.model.instructions.RegisterAccountWirePayloadEncoder;
import org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;

/** Fail-closed verification for first-release account-faucet prepare and submit results. */
public final class AccountFaucetPreparedVerifier {
  private AccountFaucetPreparedVerifier() {}

  /**
   * Authenticates one exact prepared faucet transaction against independent fee and faucet
   * policy expectations.
   */
  public static SignedTransaction requireValidPrepared(
      final AccountFaucetPreparedTransactionV1 prepared,
      final AccountFaucetClaimV1 claim,
      final TairaPublicResetMutationBindingV1 binding,
      final FeePaymentIntent expectedFeePayment,
      final AccountFaucetPolicyV1 policy,
      final NetworkId expectedNetworkId) {
    Objects.requireNonNull(prepared, "prepared");
    Objects.requireNonNull(claim, "claim");
    Objects.requireNonNull(binding, "binding");
    Objects.requireNonNull(expectedFeePayment, "expectedFeePayment");
    Objects.requireNonNull(policy, "policy");
    Objects.requireNonNull(expectedNetworkId, "expectedNetworkId");
    if (!prepared.binding().toJsonMap().equals(binding.toJsonMap())
        || !prepared.claim().toJsonMap().equals(claim.toJsonMap())) {
      throw new IllegalArgumentException(
          "prepared faucet envelope differs from the exact claim or binding");
    }
    if (!prepared.semanticHashHex().equals(claim.semanticHashHex())
        || !prepared.accountId().equals(claim.accountId())) {
      throw new IllegalArgumentException(
          "prepared faucet semantic identity differs from the exact claim");
    }
    if (!prepared.assetDefinitionId().equals(policy.assetDefinitionId())
        || !prepared.assetId().equals(policy.assetDefinitionId() + "#" + claim.accountId())
        || !prepared.amount().equals(policy.amount())) {
      throw new IllegalArgumentException(
          "prepared faucet asset definition, destination, or amount differs from trusted policy");
    }
    if (!expectedFeePayment.hasSamePayerAndGasBound(prepared.feePayment())) {
      throw new IllegalArgumentException(
          "prepared faucet fee intent changed payer, sponsor revision, or gas bound");
    }
    if (!AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
        policy.faucetAuthority(),
        PreparedTransactionSignatureV1.digest(
            PreparedTransactionSignatureV1.faucetPrepared(prepared)),
        prepared.serverSignature())) {
      throw new IllegalArgumentException(
          "prepared faucet server signature is invalid for the trusted authority");
    }

    final byte[] wire =
        PreparedTransactionSignatureV1.decodeLowerHex(prepared.signedTransactionWireHex());
    if (!PreparedTransactionSignatureV1.hexLower(sha256(wire))
        .equals(prepared.signedTransactionWireSha256())) {
      throw new IllegalArgumentException(
          "prepared faucet wire SHA-256 differs from the envelope");
    }
    final SignedTransaction transaction;
    try {
      transaction = SignedTransactionEncoder.decodeVersioned(wire);
      if (!java.util.Arrays.equals(SignedTransactionEncoder.encodeVersioned(transaction), wire)) {
        throw new IllegalArgumentException(
            "prepared faucet wire is not canonical fixed-V1 SignedTransaction");
      }
    } catch (final NoritoException error) {
      throw new IllegalArgumentException("prepared faucet wire is invalid", error);
    }
    if (!SignedTransactionHasher.hashHex(transaction).equals(prepared.transactionHashHex())) {
      throw new IllegalArgumentException(
          "prepared faucet transaction hash differs from the envelope");
    }
    final TransactionPayload payload = SignedTransactionEncoder.decodeCanonicalPayload(transaction);
    if (!AccountOnboardingReceiptVerifier.verifyAuthoritySignature(
        payload.authority(),
        IrohaHash.prehash(transaction.encodedPayload()),
        transaction.signature())) {
      throw new IllegalArgumentException("prepared faucet transaction signature is invalid");
    }
    if (!payload.networkId().equals(expectedNetworkId)
        || !AccountOnboardingReceiptVerifier.sameAccountIdentity(
            payload.authority(), policy.faucetAuthority())) {
      throw new IllegalArgumentException(
          "prepared faucet transaction network or authority was substituted");
    }
    if (!payload.feePayment().equals(prepared.feePayment())) {
      throw new IllegalArgumentException(
          "prepared faucet fee intent differs from the signed transaction");
    }
    final Map<String, JsonValue> expectedMetadata = new LinkedHashMap<>();
    expectedMetadata.put(
        "taira_public_reset_binding",
        JsonValue.parse(JsonEncoder.encode(binding.toJsonMap())));
    expectedMetadata.put(
        "taira_prepared_operation",
        JsonValue.string(AccountFaucetPreparedTransactionV1.OPERATION));
    expectedMetadata.put(
        "taira_prepared_semantic_hash", JsonValue.string(prepared.semanticHashHex()));
    if (!payload.metadata().equals(expectedMetadata)) {
      throw new IllegalArgumentException(
          "prepared faucet transaction metadata differs from the envelope");
    }
    requireExactInstructions(payload.executable(), claim, policy);
    return transaction;
  }

  /** Reconciles only a policy-authenticated prepared faucet envelope and its exact submit hash. */
  public static PreparedTransactionSubmitResponseV1 requireValidSubmitResponse(
      final PreparedTransactionSubmitResponseV1 response,
      final AccountFaucetPreparedTransactionV1 prepared,
      final FeePaymentIntent expectedFeePayment,
      final AccountFaucetPolicyV1 policy,
      final NetworkId expectedNetworkId,
      final int httpStatus) {
    requireValidPrepared(
        prepared,
        prepared.claim(),
        prepared.binding(),
        expectedFeePayment,
        policy,
        expectedNetworkId);
    if (httpStatus != 200 && httpStatus != 202) {
      throw new IllegalArgumentException("prepared faucet submit requires HTTP 200 or 202");
    }
    if (!response.binding().toJsonMap().equals(prepared.binding().toJsonMap())
        || !response.operation().equals(prepared.operation())
        || !response.transactionHashHex().equals(prepared.transactionHashHex())) {
      throw new IllegalArgumentException(
          "prepared faucet submit response is not bound to the exact envelope");
    }
    if (httpStatus == 202 && response.outcome() != PreparedTransactionOutcomeV1.PENDING) {
      throw new IllegalArgumentException("HTTP 202 prepared faucet submit must remain Pending");
    }
    return response;
  }

  private static void requireExactInstructions(
      final Executable executable,
      final AccountFaucetClaimV1 claim,
      final AccountFaucetPolicyV1 policy) {
    if (!executable.isInstructions()) {
      throw new IllegalArgumentException(
          "prepared faucet transaction must contain a direct instruction sequence");
    }
    final InstructionBox transfer =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            policy.assetDefinitionId() + "#" + policy.faucetAuthority(),
            policy.amount(),
            claim.accountId());
    final List<InstructionBox> instructions = executable.instructions();
    final boolean exactTransfer = instructions.size() == 1 && instructions.get(0).equals(transfer);
    final InstructionBox registration =
        RegisterAccountWirePayloadEncoder.encodeRegisterAccount(claim.accountId());
    final boolean exactRegisterThenTransfer =
        instructions.size() == 2
            && instructions.get(0).equals(registration)
            && instructions.get(1).equals(transfer);
    if (!exactTransfer && !exactRegisterThenTransfer) {
      throw new IllegalArgumentException(
          "prepared faucet transaction instructions differ from the exact claim and trusted policy");
    }
  }

  private static byte[] sha256(final byte[] value) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(value);
    } catch (final NoSuchAlgorithmException impossible) {
      throw new IllegalStateException("SHA-256 is unavailable", impossible);
    }
  }
}
