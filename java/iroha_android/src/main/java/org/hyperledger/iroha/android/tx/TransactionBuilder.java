package org.hyperledger.iroha.android.tx;

import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.crypto.Signer;
import org.hyperledger.iroha.android.crypto.keystore.KeyAttestation;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoCodecAdapter;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.tx.offline.OfflineEnvelopeOptions;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelope;
import org.hyperledger.iroha.android.tx.offline.OfflineTransactionBundle;

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
   * Encodes the payload with the Norito codec and signs it using the given alias. Keys are created on
   * demand by the {@link IrohaKeyManager}.
   */
  public SignedTransaction encodeAndSign(
      final TransactionPayload payload,
      final String alias,
      final IrohaKeyManager.KeySecurityPreference preference)
      throws NoritoException, KeyManagementException, SigningException {
    final Signer signer = keyManager.signerForAlias(alias, preference);
    return encodeAndSignInternal(payload, signer, alias);
  }

  /** Encodes the payload and signs it using the provided signer. */
  public SignedTransaction encodeAndSign(final TransactionPayload payload, final Signer signer)
      throws NoritoException, SigningException {
    return encodeAndSignInternal(payload, signer, null);
  }

  /**
   * Encodes, signs, and packages the transaction into an {@link OfflineSigningEnvelope}. The helper
   * executes {@link #encodeAndSign(TransactionPayload, String, IrohaKeyManager.KeySecurityPreference)}
   * internally before augmenting the resulting {@link SignedTransaction} with envelope metadata.
   *
   * @param payload transaction payload to encode/sign
   * @param alias key alias to sign with
   * @param preference security preference for key selection
   * @param options additional metadata for the envelope (timestamp, custom fields, optional key bundle)
   */
  public OfflineSigningEnvelope encodeAndSignEnvelope(
      final TransactionPayload payload,
      final String alias,
      final IrohaKeyManager.KeySecurityPreference preference,
      final OfflineEnvelopeOptions options)
      throws NoritoException, KeyManagementException, SigningException {
    final SignedTransaction transaction = encodeAndSign(payload, alias, preference);
    return envelopeFromSignedTransaction(transaction, alias, options);
  }

  /**
   * Encodes, signs, and packages the transaction into an {@link OfflineSigningEnvelope} using default
   * {@link OfflineEnvelopeOptions}.
   */
  public OfflineSigningEnvelope encodeAndSignEnvelope(
      final TransactionPayload payload,
      final String alias,
      final IrohaKeyManager.KeySecurityPreference preference)
      throws NoritoException, KeyManagementException, SigningException {
    return encodeAndSignEnvelope(payload, alias, preference, OfflineEnvelopeOptions.builder().build());
  }

  /**
   * Encodes and signs the transaction, returning both the offline envelope and a freshly generated
   * attestation bundle (when supported by the configured provider).
   *
   * @param payload transaction payload to encode/sign
   * @param alias key alias to sign with
   * @param preference security preference for key selection
   */
  public OfflineTransactionBundle encodeAndSignEnvelopeWithAttestation(
      final TransactionPayload payload,
      final String alias,
      final IrohaKeyManager.KeySecurityPreference preference,
      final OfflineEnvelopeOptions options,
      final byte[] attestationChallenge)
      throws NoritoException, KeyManagementException, SigningException {
    final OfflineSigningEnvelope envelope =
        encodeAndSignEnvelope(payload, alias, preference, options);
    final Optional<KeyAttestation> attestation =
        keyManager.generateAttestation(alias, attestationChallenge);
    return new OfflineTransactionBundle(envelope, attestation);
  }

  /** Variant that uses default {@link OfflineEnvelopeOptions}. */
  public OfflineTransactionBundle encodeAndSignEnvelopeWithAttestation(
      final TransactionPayload payload,
      final String alias,
      final IrohaKeyManager.KeySecurityPreference preference,
      final byte[] attestationChallenge)
      throws NoritoException, KeyManagementException, SigningException {
    return encodeAndSignEnvelopeWithAttestation(
        payload,
        alias,
        preference,
        OfflineEnvelopeOptions.builder().build(),
        attestationChallenge);
  }

  /** Variant that omits the attestation challenge (provider may choose defaults). */
  public OfflineTransactionBundle encodeAndSignEnvelopeWithAttestation(
      final TransactionPayload payload,
      final String alias,
      final IrohaKeyManager.KeySecurityPreference preference)
      throws NoritoException, KeyManagementException, SigningException {
    return encodeAndSignEnvelopeWithAttestation(
        payload,
        alias,
        preference,
        OfflineEnvelopeOptions.builder().build(),
        null);
  }

  private OfflineSigningEnvelope envelopeFromSignedTransaction(
      final SignedTransaction transaction,
      final String alias,
      final OfflineEnvelopeOptions options) {
    final OfflineSigningEnvelope.Builder builder =
        OfflineSigningEnvelope.builder()
            .setEncodedPayload(transaction.encodedPayload())
            .setSignature(transaction.signature())
            .setPublicKey(transaction.publicKey())
            .setSchemaName(transaction.schemaName())
            .setKeyAlias(transaction.keyAlias().orElse(alias))
            .setIssuedAtMs(options.issuedAtMs())
            .setMetadata(options.metadata());
    options.exportedKeyBundle().ifPresent(builder::setExportedKeyBundle);
    return builder.build();
  }

  private SignedTransaction encodeAndSignInternal(
      final TransactionPayload payload, final Signer signer, final String alias)
      throws NoritoException, SigningException {
    Objects.requireNonNull(payload, "payload");
    Objects.requireNonNull(signer, "signer");

    final byte[] encoded = codecAdapter.encodeTransaction(payload);
    final byte[] signature = signer.sign(encoded);
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
