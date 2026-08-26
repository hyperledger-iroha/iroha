package org.hyperledger.iroha.android.nexus;

import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletionException;
import java.util.function.Supplier;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.IrohaClient;
import org.hyperledger.iroha.android.client.TransactionFinality;
import org.hyperledger.iroha.android.crypto.Ed25519PublicKeyAdmission;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder;
import org.hyperledger.iroha.android.norito.NoritoCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;

/** High-level SORA Nexus app facade for Connect wallet transfer flows. */
public final class NexusAppClient {

  public static final String SIGNATURE_ALGORITHM_ED25519 = "ed25519";

  private final NexusAppConfig config;
  private final NexusConnectTransport connectTransport;
  private final NoritoCodecAdapter codecAdapter;
  private final IrohaClient toriiClient;

  public NexusAppClient(final NexusAppConfig config) {
    this(
        config,
        null,
        new NoritoJavaCodecAdapter(
            java.util.Objects.requireNonNull(config, "config").chainDiscriminant()),
        null);
  }

  public NexusAppClient(
      final NexusAppConfig config,
      final NexusConnectTransport connectTransport,
      final NoritoCodecAdapter codecAdapter,
      final IrohaClient toriiClient) {
    this.config = java.util.Objects.requireNonNull(config, "config");
    this.connectTransport = connectTransport;
    this.codecAdapter =
        codecAdapter == null
            ? new NoritoJavaCodecAdapter(this.config.chainDiscriminant())
            : codecAdapter;
    this.toriiClient = toriiClient;
  }

  public NexusConnectSession startConnect() {
    return startConnect(new NexusConnectOptions());
  }

  public NexusConnectSession startConnect(final NexusConnectOptions options) {
    if (connectTransport == null) {
      throw new NexusAppError(
          "connect_transport_unavailable",
          "Connect transport is required to start a Nexus Connect session");
    }
    return connectTransport.startConnect(options == null ? new NexusConnectOptions() : options, config);
  }

  public NexusApprovedAccount awaitApproval(final NexusConnectSession session) {
    if (connectTransport == null) {
      throw new NexusAppError(
          "connect_transport_unavailable",
          "Connect transport is required to await wallet approval");
    }
    final NexusApprovedAccount approved = connectTransport.awaitApproval(session, config);
    if (approved.session() != null) {
      throw new NexusAppError(
          "approval_session_mismatch",
          "wallet approval must not replace the caller's Connect session");
    }
    if (approved.accountId().isBlank()) {
      throw new NexusAppError("approval_missing_account", "wallet approval did not include an account");
    }
    final String accountId =
        requireCanonicalAccountId(approved.accountId(), "wallet approval account");
    final String[] assertedAccounts = {
      config.authority(),
      session.approvedAccount(),
    };
    final String[] assertedContexts = {
      "configured authority", "Connect session approved account"
    };
    for (int i = 0; i < assertedAccounts.length; i++) {
      final String assertedAccount = assertedAccounts[i];
      if (assertedAccount != null) {
        requireCanonicalAccountId(assertedAccount, assertedContexts[i]);
      }
      if (assertedAccount != null && !assertedAccount.equals(accountId)) {
        throw new NexusAppError(
            "approval_account_mismatch",
            assertedContexts[i] + " does not match the wallet approval account");
      }
    }
    final byte[] publicKey =
        requireAccountSigningKey(
            accountId,
            "wallet approval account",
            approved.signingPublicKey(),
            session.signingPublicKey(),
            config.signingPublicKey());
    final NexusConnectSession approvedSession = session.withApproval(accountId, publicKey);
    return approved.withSessionAndKey(approvedSession, publicKey);
  }

  public NexusTransferDraft buildTransferDraft(final NexusTransferInput input) {
    if (config.authority() != null) {
      requireCanonicalAccountId(config.authority(), "configured authority");
    }
    final String authority = input.authority() != null ? input.authority() : config.authority();
    if (authority == null || authority.isBlank()) {
      throw new NexusAppError("missing_authority", "transfer authority is required");
    }
    requireCanonicalAccountId(authority, "transfer authority");
    requireCanonicalAccountId(
        input.destinationAccountId(), "transfer destination account");
    requireCanonicalAccountId(
        sourceAssetOwner(input.sourceAssetId()), "transfer source asset owner");
    final byte[] signingPublicKey =
        requireAccountSigningKey(
            authority,
            "transfer authority",
            input.signingPublicKey(),
            config.signingPublicKey());
    final NexusTransferInput normalized = input.toBuilder()
        .authority(authority)
        .signingPublicKey(signingPublicKey)
        .build();
    final TransactionPayload payload = TransactionPayload.builder()
        .setNetworkId(config.networkId())
        .setAuthority(authority)
        .setCreationTimeMs(
            normalized.creationTimeMs() == null
                ? System.currentTimeMillis()
                : normalized.creationTimeMs())
        .setInstructions(
            List.of(
                TransferWirePayloadEncoder.encodeAssetTransfer(
                    normalized.sourceAssetId(),
                    normalized.quantity(),
                    normalized.destinationAccountId())))
        .setTimeToLiveMs(normalized.ttlMs())
        .setNonce(normalized.nonce())
        .setFeePayment(normalized.feePayment())
        .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
        .setMetadata(normalized.metadata())
        .build();
    final byte[] payloadBytes;
    try {
      payloadBytes = codecAdapter.encodeTransaction(payload);
    } catch (final NoritoException ex) {
      throw new NexusAppError("transaction_encode", "failed to encode Nexus transfer payload", ex);
    }
    final NexusSignableTransaction signable =
        new NexusSignableTransaction(
            payloadBytes,
            toHex(IrohaHash.prehash(payloadBytes)),
            authority,
            signingPublicKey,
            SIGNATURE_ALGORITHM_ED25519);
    return new NexusTransferDraft(normalized, signable);
  }

  public NexusWalletSignature requestSignature(
      final NexusConnectSession session, final NexusSignableTransaction signable) {
    if (connectTransport == null) {
      throw new NexusAppError(
          "connect_transport_unavailable",
          "Connect transport is required to request a wallet signature");
    }
    requireCanonicalAccountId(signable.authority(), "signable authority");
    if (config.authority() != null) {
      requireCanonicalAccountId(config.authority(), "configured authority");
    }
    if (session.approvedAccount() != null) {
      requireCanonicalAccountId(
          session.approvedAccount(), "Connect session approved account");
    }
    final String[] assertedAccounts = {config.authority(), session.approvedAccount()};
    final String[] assertedContexts = {
      "configured authority", "Connect session approved account"
    };
    for (int i = 0; i < assertedAccounts.length; i++) {
      if (assertedAccounts[i] != null && !assertedAccounts[i].equals(signable.authority())) {
        throw new NexusAppError(
            "approval_account_mismatch",
            assertedContexts[i] + " does not match the signable authority");
      }
    }
    requireAccountSigningKey(
        signable.authority(),
        "signable authority",
        signable.signingPublicKey(),
        session.signingPublicKey(),
        config.signingPublicKey());
    ensureEd25519(signable.signatureAlgorithm());
    final NexusWalletSignature signature =
        connectTransport.requestSignature(session, signable, config);
    ensureEd25519(signature.algorithm());
    validateEd25519Signature(signature.signature());
    return new NexusWalletSignature(signature.signature(), SIGNATURE_ALGORITHM_ED25519);
  }

  public NexusTransferReceipt finalizeAndSubmit(
      final NexusSignableTransaction signable, final NexusWalletSignature signature) {
    return finalizeAndSubmit(signable, signature, new NexusFinalizeOptions());
  }

  public NexusTransferReceipt finalizeAndSubmit(
      final NexusSignableTransaction signable,
      final NexusWalletSignature signature,
      final NexusFinalizeOptions options) {
    requireAccountSigningKey(
        signable.authority(),
        "signable authority",
        signable.signingPublicKey(),
        config.signingPublicKey());
    ensureEd25519(signable.signatureAlgorithm());
    ensureEd25519(signature.algorithm());
    validateEd25519PublicKey(signable.signingPublicKey());
    validateEd25519Signature(signature.signature());
    validateEd25519SignatureForPayload(
        signable.signingPublicKey(), signable.payloadBytes(), signature.signature());
    final SignedTransaction signed = SignedTransaction.builder()
        .setEncodedPayload(signable.payloadBytes())
        .setSignature(signature.signature())
        .setPublicKey(signable.signingPublicKey())
        .setSchemaName(codecAdapter.schemaName())
        .build();
    final String transactionHashHex = SignedTransactionHasher.hashHex(signed);
    if (toriiClient == null) {
      throw new NexusAppError(
          "torii_client_unavailable",
          "Torii client is required to submit a signed Nexus transfer");
    }
    final ClientResponse submission =
        joinClientFuture(
            "submit_failed",
            "failed to submit signed transfer to Torii",
            () -> toriiClient.submitTransaction(signed).join());
    if (submission.hashHex().isPresent()
        && !submission.hashHex().get().equals(transactionHashHex)) {
      throw new NexusAppError(
          "transaction_hash_mismatch",
          "Torii returned transaction hash "
              + submission.hashHex().get()
              + " but local hash is "
              + transactionHashHex);
    }
    final Map<String, Object> finalStatus =
        options == null || options.waitForFinalStatus()
            ? joinClientFuture(
                "status_wait_failed",
                "failed while waiting for Torii pipeline status",
                () ->
                    toriiClient
                        .waitForTransactionStatus(
                            transactionHashHex,
                            options == null ? null : options.pipelineStatusOptions())
                        .join())
            : null;
    if (finalStatus != null) {
      try {
        TransactionFinality.requireApplied(finalStatus, transactionHashHex);
      } catch (final IllegalStateException error) {
        throw new NexusAppError(
            "status_wait_non_applied",
            "Torii status waiter returned without authoritative Applied execution finality",
            error);
      }
    }
    return new NexusTransferReceipt(transactionHashHex, signed, submission, finalStatus);
  }

  public NexusTransferReceipt transferWithWallet(
      final NexusConnectSession session, final NexusTransferInput input) {
    return transferWithWallet(session, input, new NexusFinalizeOptions());
  }

  public NexusTransferReceipt transferWithWallet(
      final NexusConnectSession session,
      final NexusTransferInput input,
      final NexusFinalizeOptions options) {
    final String[] accountInputs = {
      session.approvedAccount(), input.authority(), config.authority()
    };
    final String[] accountContexts = {
      "Connect session approved account", "transfer authority", "configured authority"
    };
    for (int i = 0; i < accountInputs.length; i++) {
      if (accountInputs[i] != null) {
        requireCanonicalAccountId(accountInputs[i], accountContexts[i]);
      }
    }
    final String authority = input.authority() != null
        ? input.authority()
        : session.approvedAccount() != null ? session.approvedAccount() : config.authority();
    if (authority == null || authority.isBlank()) {
      throw new NexusAppError("missing_authority", "transfer authority is required");
    }
    if (session.approvedAccount() != null
        && input.authority() != null
        && !session.approvedAccount().equals(input.authority())) {
      throw new NexusAppError(
          "approval_account_mismatch",
          "transfer authority does not match the approved wallet account");
    }
    final byte[] signingPublicKey =
        requireAccountSigningKey(
            authority,
            "transfer authority",
            input.signingPublicKey(),
            session.signingPublicKey(),
            config.signingPublicKey());
    final NexusTransferDraft draft =
        buildTransferDraft(input.toBuilder().authority(authority).signingPublicKey(signingPublicKey).build());
    final NexusWalletSignature walletSignature = requestSignature(session, draft.signable());
    return finalizeAndSubmit(draft.signable(), walletSignature, options);
  }

  private String requireCanonicalAccountId(final String value, final String context) {
    if (!value.equals(value.trim())) {
      throw new NexusAppError(
          "invalid_account_id",
          context
              + " must be an exact canonical I105 account for chain discriminant "
              + config.chainDiscriminant());
    }
    final AccountAddress address;
    try {
      address =
          AccountAddress.parseEncodedIgnoringCurveSupport(
              value, config.chainDiscriminant());
    } catch (final AccountAddress.AccountAddressException error) {
      throw new NexusAppError(
          "invalid_account_id",
          context
              + " must be an exact canonical I105 account for chain discriminant "
              + config.chainDiscriminant(),
          error);
    }
    final String canonical;
    try {
      canonical = address.toI105(config.chainDiscriminant());
    } catch (final AccountAddress.AccountAddressException error) {
      throw new NexusAppError(
          "invalid_account_id",
          context
              + " could not be rendered for chain discriminant "
              + config.chainDiscriminant(),
          error);
    }
    if (!canonical.equals(value)) {
      throw new NexusAppError(
          "invalid_account_id", context + " must use its exact canonical I105 representation");
    }
    return value;
  }

  private byte[] requireAccountSigningKey(
      final String accountId, final String context, final byte[]... sources) {
    final AccountAddress address;
    try {
      address =
          AccountAddress.parseEncodedIgnoringCurveSupport(
              requireCanonicalAccountId(accountId, context), config.chainDiscriminant());
    } catch (final NexusAppError error) {
      throw error;
    } catch (final AccountAddress.AccountAddressException error) {
      throw new NexusAppError(
          "missing_signing_public_key", context + " must encode one Ed25519 controller", error);
    }
    final AccountAddress.SingleKeyPayload controller;
    try {
      controller = address.singleKeyPayloadIgnoringCurveSupport().orElse(null);
    } catch (final AccountAddress.AccountAddressException error) {
      throw new NexusAppError(
          "missing_signing_public_key", context + " must encode one Ed25519 controller", error);
    }
    if (controller == null || controller.curveId() != 0x01) {
      throw new NexusAppError(
          "missing_signing_public_key", context + " must encode one Ed25519 controller");
    }
    final byte[] controllerKey = controller.publicKey();
    validateEd25519PublicKey(controllerKey);
    boolean supplied = false;
    for (final byte[] source : sources) {
      if (source == null) {
        continue;
      }
      supplied = true;
      validateEd25519PublicKey(source);
      if (!Arrays.equals(source, controllerKey)) {
        throw new NexusAppError(
            "approval_account_mismatch", "signing public key does not control " + context);
      }
    }
    if (!supplied) {
      throw new NexusAppError(
          "missing_signing_public_key", context + " did not provide a signing public key");
    }
    return Arrays.copyOf(controllerKey, controllerKey.length);
  }

  private static String sourceAssetOwner(final String sourceAssetId) {
    final String[] parts = sourceAssetId.split("#", -1);
    if (parts.length < 2 || parts.length > 3 || parts[1].isEmpty()) {
      throw new NexusAppError(
          "invalid_account_id",
          "transfer source asset must contain one canonical owner account");
    }
    if (parts.length == 3 && !isCanonicalDataspaceScope(parts[2])) {
      throw new NexusAppError(
          "invalid_account_id",
          "transfer source asset scope must be a canonical dataspace:<u64> suffix");
    }
    return parts[1];
  }

  private static boolean isCanonicalDataspaceScope(final String scope) {
    final String prefix = "dataspace:";
    if (!scope.startsWith(prefix)) {
      return false;
    }
    final String value = scope.substring(prefix.length());
    if (value.isEmpty() || (value.length() > 1 && value.charAt(0) == '0')) {
      return false;
    }
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      if (ch < '0' || ch > '9') {
        return false;
      }
    }
    final String maxU64 = "18446744073709551615";
    return value.length() < maxU64.length()
        || (value.length() == maxU64.length() && value.compareTo(maxU64) <= 0);
  }

  private static void ensureEd25519(final String algorithm) {
    if (!isPrintableAscii(algorithm)
        || !(SIGNATURE_ALGORITHM_ED25519.equals(algorithm) || "0".equals(algorithm))) {
      throw new NexusAppError(
          "unsupported_signature_algorithm",
          "Nexus App Facade V1 supports Ed25519 signatures only");
    }
  }

  private static boolean isPrintableAscii(final String value) {
    if (value == null) {
      return false;
    }
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      if (ch < 0x20 || ch > 0x7E) {
        return false;
      }
    }
    return true;
  }

  private static void validateEd25519PublicKey(final byte[] publicKey) {
    if (!Ed25519PublicKeyAdmission.isValid(publicKey)) {
      throw new NexusAppError(
          "invalid_signing_public_key",
          "Ed25519 signing public key must be a canonical point in the prime-order subgroup");
    }
  }

  private static void validateEd25519Signature(final byte[] signature) {
    if (signature == null || signature.length != 64) {
      throw new NexusAppError("invalid_signature", "Ed25519 signature must be 64 bytes");
    }
  }

  private static void validateEd25519SignatureForPayload(
      final byte[] publicKey, final byte[] payloadBytes, final byte[] signature) {
    final byte[] message = IrohaHash.prehash(payloadBytes);
    final boolean verified;
    try {
      final Ed25519Signer verifier = new Ed25519Signer();
      verifier.init(false, new Ed25519PublicKeyParameters(publicKey, 0));
      verifier.update(message, 0, message.length);
      verified = verifier.verifySignature(signature);
    } catch (final RuntimeException ex) {
      throw new NexusAppError(
          "invalid_signature",
          "Ed25519 signature does not verify for the signable payload",
          ex);
    }
    if (!verified) {
      throw new NexusAppError(
          "invalid_signature", "Ed25519 signature does not verify for the signable payload");
    }
  }

  private static <T> T joinClientFuture(
      final String code, final String message, final Supplier<T> supplier) {
    try {
      return supplier.get();
    } catch (final CompletionException ex) {
      final Throwable cause = ex.getCause() == null ? ex : ex.getCause();
      throw new NexusAppError(code, message + ": " + cause.getMessage(), cause);
    } catch (final RuntimeException ex) {
      throw new NexusAppError(code, message + ": " + ex.getMessage(), ex);
    }
  }

  private static String toHex(final byte[] data) {
    final StringBuilder builder = new StringBuilder(data.length * 2);
    for (final byte b : data) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }
}
