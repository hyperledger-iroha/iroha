package org.hyperledger.iroha.android.tx;

import java.util.Objects;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.android.crypto.Signer;
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
