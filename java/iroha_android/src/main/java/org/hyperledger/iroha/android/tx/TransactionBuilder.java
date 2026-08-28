package org.hyperledger.iroha.android.tx;

import java.util.Objects;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.crypto.SignatureAdmission;
import org.hyperledger.iroha.android.crypto.SigningAlgorithm;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoException;

/**
 * Encodes transaction payloads via Norito and attaches signatures using keys managed by
 * {@link IrohaKeyManager}.
 */
public final class TransactionBuilder {

  private final NoritoCodecAdapter codecAdapter;
  private final IrohaKeyManager keyManager;

  public TransactionBuilder(final NoritoCodecAdapter codecAdapter, final IrohaKeyManager keyManager) {
    this.codecAdapter = Objects.requireNonNull(codecAdapter, "codecAdapter");
    this.keyManager = Objects.requireNonNull(keyManager, "keyManager");
  }

  /**
   * Encodes the payload for public Torii submission and signs it using the given alias. Keys are
   * created on demand by the {@link IrohaKeyManager}.
   *
   * <p>Public submission requires the signature-bound QueuePlan admission intent. The caller's
   * payload remains unchanged so direct codec users continue to produce ordinary transactions.
   */
  public SignedTransaction encodeAndSign(
      final TransactionPayload payload,
      final String alias,
      final IrohaKeyManager.KeySecurityPreference preference)
      throws NoritoException, KeyManagementException, SigningException {
    final Signer signer = keyManager.signerForAlias(alias, preference);
    return encodeAndSignInternal(withQueuePlanSyncedAdmission(payload), signer, alias);
  }

  /** Encodes a public-submission payload and signs it using the provided signer. */
  public SignedTransaction encodeAndSign(final TransactionPayload payload, final Signer signer)
      throws NoritoException, SigningException {
    return encodeAndSignInternal(withQueuePlanSyncedAdmission(payload), signer, null);
  }

  private static TransactionPayload withQueuePlanSyncedAdmission(
      final TransactionPayload payload) {
    Objects.requireNonNull(payload, "payload");
    return payload
        .toBuilder()
        .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
        .build();
  }

  private SignedTransaction encodeAndSignInternal(
      final TransactionPayload payload, final Signer signer, final String alias)
      throws NoritoException, SigningException {
    Objects.requireNonNull(payload, "payload");
    Objects.requireNonNull(signer, "signer");

    final SigningAlgorithm algorithm;
    try {
      algorithm = SigningAlgorithm.fromAlgorithmName(signer.algorithm());
    } catch (final IllegalArgumentException error) {
      throw new SigningException("Unsupported signer algorithm", error);
    }
    final byte[] encoded = codecAdapter.encodeTransaction(payload);
    final byte[] signature = signer.sign(encoded);
    if (!SignatureAdmission.isValid(algorithm, signature)) {
      final int expectedLength;
      if (algorithm == SigningAlgorithm.ED25519) {
        expectedLength = SignatureAdmission.ED25519_SIGNATURE_LENGTH;
      } else if (algorithm == SigningAlgorithm.ML_DSA) {
        expectedLength = SignatureAdmission.ML_DSA_65_SIGNATURE_LENGTH;
      } else {
        throw new SigningException(algorithm.providerName() + " signer returned no signature");
      }
      throw new SigningException(
          algorithm.providerName()
              + " signer returned a malformed signature; expected "
              + expectedLength
              + " nonzero bytes");
    }
    return SignedTransaction.builder()
        .setEncodedPayload(encoded)
        .setSignature(signature)
        .setPublicKey(signer.publicKey())
        .setSchemaName(codecAdapter.schemaName())
        .setKeyAlias(alias)
        .setBlsPublicKey(signer.blsPublicKey())
        .build();
  }
}
