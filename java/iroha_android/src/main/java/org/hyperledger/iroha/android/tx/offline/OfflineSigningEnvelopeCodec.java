package org.hyperledger.iroha.android.tx.offline;

import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.norito.NoritoCodec;

/**
 * Encodes and decodes {@link OfflineSigningEnvelope} instances using the shared Norito codec.
 */
public final class OfflineSigningEnvelopeCodec {

  private static final String DEFAULT_SCHEMA = "iroha.android.offline.Envelope.v1";
  private final String schemaName;
  private final OfflineSigningEnvelopeAdapter adapter = new OfflineSigningEnvelopeAdapter();

  public OfflineSigningEnvelopeCodec() {
    this(DEFAULT_SCHEMA);
  }

  public OfflineSigningEnvelopeCodec(final String schemaName) {
    this.schemaName = schemaName;
  }

  public byte[] encode(final OfflineSigningEnvelope envelope) throws NoritoException {
    try {
      return NoritoCodec.encode(envelope, schemaName, adapter);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to encode offline signing envelope", ex);
    }
  }

  public OfflineSigningEnvelope decode(final byte[] encoded) throws NoritoException {
    try {
      return NoritoCodec.decode(encoded, adapter, schemaName);
    } catch (final Exception ex) {
      throw new NoritoException("Failed to decode offline signing envelope", ex);
    }
  }

  public String schemaName() {
    return schemaName;
  }
}
