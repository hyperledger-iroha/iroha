package org.hyperledger.iroha.android.nexus;

import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletionException;
import java.util.function.Supplier;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.IrohaClient;
import org.hyperledger.iroha.android.crypto.IrohaHash;
import org.hyperledger.iroha.android.model.TransactionPayload;
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
    this(config, null, new NoritoJavaCodecAdapter(), null);
  }

  public NexusAppClient(
      final NexusAppConfig config,
      final NexusConnectTransport connectTransport,
      final NoritoCodecAdapter codecAdapter,
      final IrohaClient toriiClient) {
    this.config = java.util.Objects.requireNonNull(config, "config");
    this.connectTransport = connectTransport;
    this.codecAdapter = codecAdapter == null ? new NoritoJavaCodecAdapter() : codecAdapter;
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
    if (approved.accountId().isBlank()) {
      throw new NexusAppError("approval_missing_account", "wallet approval did not include an account");
    }
    final byte[] publicKey = firstKey(
        approved.signingPublicKey(), session.signingPublicKey(), config.signingPublicKey());
    if (publicKey == null || publicKey.length == 0) {
      throw new NexusAppError(
          "missing_signing_public_key", "wallet approval did not include a signing public key");
    }
    validateEd25519PublicKey(publicKey);
    final NexusConnectSession approvedSession = approved.session() == null
        ? session.withApproval(approved.accountId(), publicKey)
        : approved.session();
    return approved.withSessionAndKey(approvedSession, publicKey);
  }

  public NexusTransferDraft buildTransferDraft(final NexusTransferInput input) {
    final String authority = input.authority() != null ? input.authority() : config.authority();
    if (authority == null || authority.isBlank()) {
      throw new NexusAppError("missing_authority", "transfer authority is required");
    }
    final byte[] signingPublicKey = firstKey(input.signingPublicKey(), config.signingPublicKey());
    if (signingPublicKey == null) {
      throw new NexusAppError(
          "missing_signing_public_key",
          "signing public key is required for an externally signed transfer");
    }
    validateEd25519PublicKey(signingPublicKey);
    final NexusTransferInput normalized = input.toBuilder()
        .authority(authority)
        .signingPublicKey(signingPublicKey)
        .build();
    final TransactionPayload payload = TransactionPayload.builder()
        .setChainId(config.chainId())
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
        firstKey(input.signingPublicKey(), session.signingPublicKey(), config.signingPublicKey());
    if (signingPublicKey == null) {
      throw new NexusAppError(
          "missing_signing_public_key",
          "approved account did not provide a signing public key");
    }
    final NexusTransferDraft draft =
        buildTransferDraft(input.toBuilder().authority(authority).signingPublicKey(signingPublicKey).build());
    final NexusWalletSignature walletSignature = requestSignature(session, draft.signable());
    return finalizeAndSubmit(draft.signable(), walletSignature, options);
  }

  private static void ensureEd25519(final String algorithm) {
    if (!SIGNATURE_ALGORITHM_ED25519.equalsIgnoreCase(algorithm)) {
      throw new NexusAppError(
          "unsupported_signature_algorithm",
          "Nexus App Facade V1 supports Ed25519 signatures only");
    }
  }

  private static void validateEd25519PublicKey(final byte[] publicKey) {
    if (publicKey == null || publicKey.length != 32) {
      throw new NexusAppError(
          "invalid_signing_public_key", "Ed25519 signing public key must be 32 bytes");
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

  private static byte[] firstKey(final byte[]... keys) {
    for (final byte[] key : keys) {
      if (key != null && key.length > 0) {
        return Arrays.copyOf(key, key.length);
      }
    }
    return null;
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
